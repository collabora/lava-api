use futures::future::BoxFuture;
use futures::FutureExt;
use futures::stream::Stream;
use log::debug;
use reqwest::Client;
use serde::{de::DeserializeOwned, Deserialize};
use std::collections::VecDeque;
use std::pin::Pin;
use std::task::{Context, Poll};
use thiserror::Error;
use url::Url;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PaginationError {
    #[error("http request failed: {0}")]
    ReqWest(#[from] reqwest::Error),
    #[error("HTTP redirect without location")]
    RedirectMissing,
    #[error("HTTP redirect not valid utf-8")]
    RedirectInvalidUTF8,
    #[error("Too many redirects")]
    TooManyRedirects,
    #[error("Failed to parse next uri: {0}")]
    ParseNextError(#[from] url::ParseError),
}

#[derive(Deserialize, Debug)]
struct PaginatedReply<T> {
    count: u32,
    next: Option<String>,
    results: VecDeque<T>,
}

enum State<T> {
    Data(PaginatedReply<T>),
    Next(BoxFuture<'static, Result<PaginatedReply<T>, PaginationError>>),
    Failed,
}

pub struct Paginator<T> {
    client: Client,
    current: Url,
    next: State<T>,
    count: Option<u32>,
}

impl<T> Paginator<T>
where
    T: DeserializeOwned + 'static,
{
    pub fn new(client: Client, base: &Url, function: &str) -> Self {
        let url = base.join(function).expect("Failed to append to base url");
        let next = State::Next(
            Self::get(
                client.clone(),
                url.clone(),
            )
            .boxed(),
        );

        Paginator { client, current: url, next, count: None }
    }

    async fn get(client: Client, uri: Url) -> Result<PaginatedReply<T>, PaginationError>
    where
        T: DeserializeOwned,
    {
        let mut redirects: u8 = 0;
        let mut u = uri.clone();
        let response = loop {
            let response = client.get(u.clone()).send().await?;

            if !response.status().is_redirection() {
                break response;
            }

            if redirects > 9 {
                return Err(PaginationError::TooManyRedirects);
            }

            redirects += 1;
            if let Some(location) = response.headers().get("location") {
                let redirect = std::str::from_utf8(location.as_bytes())
                    .or(Err(PaginationError::RedirectInvalidUTF8))?;

                debug!("Redirecting from {:?} to {:?}", u, location);
                u = u.join(redirect)?;
                // Prevent https to http downgrade as we might have a token in
                // the request
                if uri.scheme() == "https" && u.scheme() == "http" {
                    u.set_scheme("https").unwrap();
                }
            } else {
                return Err(PaginationError::RedirectMissing);
            }
        };

        response
            .error_for_status()?
            .json()
            .await
            .map_err(|e| e.into())
    }

    fn next_data(&mut self) -> Result<Option<T>, PaginationError> {
        if let State::Data(d) = &mut self.next {
            self.count = Some(d.count);
            if let Some(data) = d.results.pop_front() {
                return Ok(Some(data));
            }

            if let Some(n) = &d.next {
                let u : Result<Url, _> = n.parse();
                match u {
                    Ok(u) => {
                        self.next = State::Next(Self::get(self.client.clone(), u.clone()).boxed());
                        self.current = u;
                    },
                    Err(e) => {
                        self.next = State::Failed;
                        return Err(e.into());
                    }
                }
            }
        }
        Ok(None)
    }

    pub fn reported_items(&self) -> Option<u32> {
        self.count
    }
}

impl<T> Stream for Paginator<T>
where
    T: DeserializeOwned + Unpin + 'static,
{
    type Item = Result<T, PaginationError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let me = self.get_mut();
        if let Some(data) = me.next_data()? {
            return Poll::Ready(Some(Ok(data)));
        }

        if let State::Next(n) = &mut me.next {
            match n.as_mut().poll(cx) {
                Poll::Ready(r) => {
                    match r {
                        Ok(r) => me.next = State::Data(r),
                        Err(e) => {
                            me.next = State::Next(Self::get(me.client.clone(), me.current.clone()).boxed());
                            return Poll::Ready(Some(Err(e)))
                        },
                    }
                    if let Some(data) = me.next_data()? {
                        Poll::Ready(Some(Ok(data)))
                    } else {
                        Poll::Pending
                    }
                }
                _ => Poll::Pending,
            }
        } else {
            Poll::Ready(None)
        }
    }
}
