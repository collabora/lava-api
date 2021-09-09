pub mod device;
pub mod job;
mod paginator;
mod queryset;
pub mod tag;
pub mod worker;

use futures::stream::TryStreamExt;
use log::debug;
use reqwest::{header, redirect::Policy, Client};
use std::collections::HashMap;
use std::convert::TryInto;
use tokio::sync::RwLock;
use url::Url;

use device::Devices;
use job::JobsBuilder;
use paginator::{PaginationError, Paginator};
use tag::Tag;
use thiserror::Error;
use worker::Worker;

#[derive(Error, Debug)]
pub enum LavaError {
    #[error("Could not parse url")]
    ParseUrlError(#[from] url::ParseError),
    #[error("Invalid token format")]
    InvalidToken(#[from] header::InvalidHeaderValue),
    #[error("Failed to build reqwest client")]
    ReqwestError(#[from] reqwest::Error),
}

#[derive(Debug)]
pub struct Lava {
    client: Client,
    base: Url,
    tags: RwLock<HashMap<u32, Tag>>,
}

impl Lava {
    pub fn new(url: &str, token: Option<String>) -> Result<Lava, LavaError> {
        let host: Url = url.parse()?;
        let base = host.join("api/v0.2/")?;
        let tags = RwLock::new(HashMap::new());
        let mut headers = header::HeaderMap::new();

        if let Some(t) = token {
            headers.insert(
                reqwest::header::AUTHORIZATION,
                format!("Token {}", t).try_into()?,
            );
        }

        // Force redirect policy none as that will drop sensitive headers; in
        // particular tokens
        let client = Client::builder()
            .redirect(Policy::none())
            .default_headers(headers)
            .build()?;

        Ok(Lava { client, base, tags })
    }

    pub async fn refresh_tags(&self) -> Result<(), PaginationError> {
        debug!("Refreshing tags cache");
        let mut tags = self.tags.write().await;
        let url = self.base.join("tags/")?;
        let mut new_tags: Paginator<Tag> = Paginator::new(self.client.clone(), url);
        while let Some(t) = new_tags.try_next().await? {
            tags.insert(t.id, t);
        }

        Ok(())
    }

    pub async fn tag(&self, tag: u32) -> Option<Tag> {
        debug!("Checking for tag id: {}", tag);
        {
            let tags = self.tags.read().await;
            if let Some(t) = tags.get(&tag) {
                return Some(t.clone());
            }
        }
        let _ = self.refresh_tags().await;

        let tags = self.tags.read().await;
        tags.get(&tag).cloned()
    }

    pub async fn tags(&self) -> Result<Vec<Tag>, PaginationError> {
        self.refresh_tags().await?;
        let tags = self.tags.read().await;
        Ok(tags.values().cloned().collect())
    }

    pub fn devices(&self) -> Devices {
        Devices::new(self)
    }

    pub fn jobs(&self) -> JobsBuilder {
        JobsBuilder::new(self)
    }

    pub fn workers(&self) -> Paginator<Worker> {
        let url = self
            .base
            .join("workers/")
            .expect("Failed to append to base url");
        Paginator::new(self.client.clone(), url)
    }
}
