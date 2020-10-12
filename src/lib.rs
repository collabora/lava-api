pub mod device;
mod paginator;
pub mod tag;
pub mod worker;

use reqwest::Client;
use std::collections::HashMap;
use tokio::stream::StreamExt;
use tokio::sync::RwLock;
use url::Url;

use device::Devices;
use paginator::{PaginationError, Paginator};
use tag::Tag;
use worker::Worker;

#[derive(Debug)]
pub struct Lava {
    client: Client,
    base: Url,
    tags: RwLock<HashMap<u32, Tag>>,
}

impl Lava {
    pub fn new(url: &str) -> Result<Lava, url::ParseError> {
        let host: Url = url.parse()?;
        let base = host.join("api/v0.2/")?;
        let tags = RwLock::new(HashMap::new());

        Ok(Lava {
            client: Client::new(),
            base,
            tags,
        })
    }

    pub async fn refresh_tags(&self) -> Result<(), PaginationError> {
        let mut tags = self.tags.write().await;
        let mut new_tags: Paginator<Tag> = Paginator::new(self.client.clone(), &self.base, "tags/");
        while let Some(t) = new_tags.try_next().await? {
            tags.insert(t.id, t);
        }

        Ok(())
    }

    pub async fn tag(&self, tag: u32) -> Option<Tag> {
        {
            let tags = self.tags.read().await;
            if let Some(t) = tags.get(&tag) {
                return Some(t.clone());
            }
        }
        let _ = self.refresh_tags().await;

        let tags = self.tags.read().await;
        return tags.get(&tag).cloned();
    }

    pub async fn tags(&self) -> Result<Vec<Tag>, PaginationError> {
        self.refresh_tags().await?;
        let tags = self.tags.read().await;
        Ok(tags.values().map(|t| t.clone()).collect())
    }

    pub fn devices(&self) -> Devices {
        Devices::new(self, self.client.clone(), &self.base)
    }

    pub fn workers(&self) -> Paginator<Worker> {
        Paginator::new(self.client.clone(), &self.base, "workers")
    }
}
