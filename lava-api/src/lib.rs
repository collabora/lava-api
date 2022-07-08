//! Provide access to the data held by a
//! [LAVA](https://docs.lavasoftware.org/lava/) server via the data
//! export REST interface.
//!
//! # Overview
//!
//! The central object in this crate is a [`Lava`], which represents
//! a local proxy for a LAVA server. The coverage of the data exposed
//! by LAVA is not complete. It is however possible to readback
//! - jobs
//! - test results
//! - devices
//! - workers
//! - tags (which apply to both jobs and devices)
//!
//! Pagination is handled transparently, but you will likely want to
//! use [`TryStreamExt`] to iterate over returned streams of objects,
//! since this crate is async and built on the [`tokio`] runtime.
//!
//! Example:
//! ```rust
//! use futures::stream::TryStreamExt;
//! # use lava_api_mock::{LavaMock, PaginationLimits, PopulationParams, SharedState};
//! use lava_api::Lava;
//! #
//! # tokio_test::block_on( async {
//! # let limits = PaginationLimits::new();
//! # let population = PopulationParams::new();
//! # let mock = LavaMock::new(SharedState::new_populated(population), limits).await;
//! # let service_uri = mock.uri();
//! # let lava_token = None;
//!
//! let lava = Lava::new(&service_uri, lava_token).expect("failed to create Lava object");
//!
//! // Read back the device data from the server
//! let mut ld = lava.devices();
//! while let Some(device) = ld
//!     .try_next()
//!     .await
//!     .expect("failed to read devices from server")
//! {
//!     println!("Got device {:?}", device);
//! }
//! # });
//! ```
pub mod device;
pub mod job;
mod paginator;
mod queryset;
pub mod tag;
pub mod test;
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
use test::TestCase;
use thiserror::Error;
use worker::Worker;

/// Errors in construction of a [`Lava`] instance
#[derive(Error, Debug)]
pub enum LavaError {
    #[error("Could not parse url")]
    ParseUrlError(#[from] url::ParseError),
    #[error("Invalid token format")]
    InvalidToken(#[from] header::InvalidHeaderValue),
    #[error("Failed to build reqwest client")]
    ReqwestError(#[from] reqwest::Error),
}

/// A local proxy for a LAVA server
///
/// This provides convenient access to some of the data
/// stored on a LAVA server, including jobs, devices, tags and
/// workers.
#[derive(Debug)]
pub struct Lava {
    client: Client,
    base: Url,
    tags: RwLock<HashMap<u32, Tag>>,
}

impl Lava {
    /// Create a new Lava proxy
    ///
    /// Here `url` is the address of the server, and `token` is an
    /// optional LAVA security token used to validate access.
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

    /// Refresh the tag cache
    ///
    /// Tags are cached to make lookup cheaper, and because the number
    /// of jobs can be very large: resolving tags individually for
    /// each job would be extremely slow. The cache has to be
    /// periodically refreshed to account for changes.
    ///
    /// Note that tags are automatically refreshed by calling
    /// [`tag`](Self::tag) or [`tags`](Self::tags), but not by calling
    /// [`devices`](Self::devices) or [`jobs`](Self::jobs).
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

    /// Retrieve the [`Tag`] for the given tag id.
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

    /// Retrieve all the tags from the server
    ///
    /// The returned data is not a stream, but a flat vector when the
    /// method succeeds. This also updates the tag cache.
    pub async fn tags(&self) -> Result<Vec<Tag>, PaginationError> {
        self.refresh_tags().await?;
        let tags = self.tags.read().await;
        Ok(tags.values().cloned().collect())
    }

    /// Obtain a [`Stream`](futures::stream::Stream) of all the
    /// [`Device`](device::Device) instances on the server.
    pub fn devices(&self) -> Devices {
        Devices::new(self)
    }

    /// Obtain a customisable query object for [`Job`](job::Job)
    /// instances on the server.
    ///
    /// The returned [`JobsBuilder`] can be used first to select the
    /// subset of jobs that will be returned, and then after that is
    /// complete to obtain a stream of matching jobs. The default
    /// query is the same as that for [`JobsBuilder::new`].
    pub fn jobs(&self) -> JobsBuilder {
        JobsBuilder::new(self)
    }

    /// Obtain a [`Stream`](futures::stream::Stream) of all the
    /// [`Worker`] instances on the server.
    pub fn workers(&self) -> Paginator<Worker> {
        let url = self
            .base
            .join("workers/")
            .expect("Failed to append to base url");
        Paginator::new(self.client.clone(), url)
    }

    /// Obtain a [`Stream`](futures::stream::Stream) of all the
    /// [`TestCase`] instances for a given job id.
    pub fn test_cases(&self, job_id: i64) -> Paginator<TestCase> {
        let url = self
            .base
            .join("jobs/")
            .and_then(|x| x.join(&format!("{}/", job_id)))
            .and_then(|x| x.join("tests/"))
            .expect("Failed to build test case url");
        Paginator::new(self.client.clone(), url)
    }
}
