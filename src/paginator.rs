use futures::future::BoxFuture;
use futures::FutureExt;
use reqwest::Client;
use serde::{de::DeserializeOwned, Deserialize};
use std::collections::VecDeque;
use std::pin::Pin;
use std::task::{Context, Poll};
use thiserror::Error;
use tokio::stream::Stream;
use url::Url;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PaginationError {
    #[error("http request failed: {0}")]
    ReqWest(#[from] reqwest::Error),
    #[error("Failed to parse next uri: {0}")]
    ParseNextError(#[from] url::ParseError),
}

#[derive(Deserialize, Debug)]
struct PaginatedReply<T> {
    count: i32,
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
    next: State<T>,
}

impl<T> Paginator<T>
where
    T: DeserializeOwned + 'static,
{
    pub fn new(client: Client, base: &Url, function: &str) -> Self {
        let next = State::Next(
            Self::get(
                client.clone(),
                base.join(function).expect("Failed to append to base url"),
            )
            .boxed(),
        );

        Paginator { client, next }
    }

    async fn get(client: Client, uri: Url) -> Result<PaginatedReply<T>, PaginationError>
    where
        T: DeserializeOwned,
    {
        client
            .get(uri)
            .send()
            .await?
            .json()
            .await
            .map_err(|e| e.into())
    }

    fn next_data(&mut self) -> Result<Option<T>, PaginationError> {
        if let State::Data(d) = &mut self.next {
            if let Some(data) = d.results.pop_front() {
                return Ok(Some(data));
            }

            if let Some(n) = &d.next {
                let u = n.parse();
                match u {
                    Ok(u) => self.next = State::Next(Self::get(self.client.clone(), u).boxed()),
                    Err(e) => {
                        self.next = State::Failed;
                        return Err(e.into());
                    }
                }
            }
        }
        Ok(None)
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
                        Err(e) => return Poll::Ready(Some(Err(e))),
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
