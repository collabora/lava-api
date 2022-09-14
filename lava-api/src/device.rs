//! Retrieve devices

use futures::future::BoxFuture;
use futures::FutureExt;
use futures::{stream, stream::Stream, stream::StreamExt};
use serde::Deserialize;
use serde_with::DeserializeFromStr;
use std::convert::{Infallible, TryFrom, TryInto};
use std::pin::Pin;
use std::task::{Context, Poll};
use strum::{Display, EnumString};

use crate::paginator::{PaginationError, Paginator};
use crate::tag::Tag;
use crate::Lava;

/// The current status of a [`Device`]
#[derive(Clone, Copy, Debug, DeserializeFromStr, Display, EnumString, Eq, PartialEq)]
pub enum Health {
    Unknown,
    Maintenance,
    Good,
    Bad,
    Looping,
    Retired,
}

#[derive(Clone, Deserialize, Debug)]
struct LavaDevice {
    hostname: String,
    worker_host: String,
    device_type: String,
    description: Option<String>,
    health: Health,
    pub tags: Vec<u32>,
}

/// A subset of the data available for a device from the LAVA API.
///
/// Note that [`tags`](Device::tags) have been resolved into [`Tag`]
/// objects, rather than tag ids.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    pub hostname: String,
    pub worker_host: String,
    pub device_type: String,
    pub description: Option<String>,
    pub health: Health,
    pub tags: Vec<Tag>,
}

enum State<'a> {
    Paging,
    Transforming(BoxFuture<'a, Device>),
}

/// A [`Stream`] that yields all the [`Device`] instances on a LAVA
/// server.
pub struct Devices<'a> {
    lava: &'a Lava,
    paginator: Paginator<LavaDevice>,
    state: State<'a>,
}

impl<'a> Devices<'a> {
    /// Create a new stream, using the given [`Lava`] proxy.
    ///
    /// Note that due to pagination, the dataset returned is not
    /// guaranteed to be self-consistent, and the odds of
    /// self-consistency decrease the longer it takes to iterate over
    /// the stream. It is therefore advisable to extract whatever data
    /// is required immediately after the creation of this object.
    pub fn new(lava: &'a Lava) -> Self {
        let url = lava
            .base
            .join("devices/?ordering=hostname")
            .expect("Failed to append to base url");
        let paginator = Paginator::new(lava.client.clone(), url);
        Self {
            lava,
            paginator,
            state: State::Paging,
        }
    }
}

async fn transform_device(device: LavaDevice, lava: &Lava) -> Device {
    let t = stream::iter(device.tags.iter());
    let tags = t
        .filter_map(|i| async move { lava.tag(*i).await })
        .collect()
        .await;

    Device {
        hostname: device.hostname,
        worker_host: device.worker_host,
        device_type: device.device_type,
        description: device.description,
        health: device.health,
        tags,
    }
}

impl<'a> Stream for Devices<'a> {
    type Item = Result<Device, PaginationError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let me = self.get_mut();

        loop {
            return match &mut me.state {
                State::Paging => {
                    let p = Pin::new(&mut me.paginator);
                    match p.poll_next(cx) {
                        Poll::Ready(None) => Poll::Ready(None),
                        Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
                        Poll::Ready(Some(Ok(d))) => {
                            me.state = State::Transforming(transform_device(d, me.lava).boxed());
                            continue;
                        }
                        Poll::Pending => Poll::Pending,
                    }
                }
                State::Transforming(fut) => match fut.as_mut().poll(cx) {
                    Poll::Ready(d) => {
                        me.state = State::Paging;
                        Poll::Ready(Some(Ok(d)))
                    }
                    Poll::Pending => Poll::Pending,
                },
            };
        }
    }
}

impl TryFrom<lava_api_mock::DeviceHealth> for Health {
    type Error = Infallible;
    fn try_from(dev: lava_api_mock::DeviceHealth) -> Result<Health, Self::Error> {
        use Health::*;
        match dev {
            lava_api_mock::DeviceHealth::Unknown => Ok(Unknown),
            lava_api_mock::DeviceHealth::Maintenance => Ok(Maintenance),
            lava_api_mock::DeviceHealth::Good => Ok(Good),
            lava_api_mock::DeviceHealth::Bad => Ok(Bad),
            lava_api_mock::DeviceHealth::Looping => Ok(Looping),
            lava_api_mock::DeviceHealth::Retired => Ok(Retired),
        }
    }
}

impl Device {
    #[persian_rug::constraints(context = C, access(lava_api_mock::Tag<C>, lava_api_mock::DeviceType<C>, lava_api_mock::Worker<C>))]
    pub fn from_mock<'b, B, C>(dev: &lava_api_mock::Device<C>, context: B) -> Device
    where
        B: 'b + persian_rug::Accessor<Context = C>,
        C: persian_rug::Context + 'static,
    {
        Self {
            hostname: dev.hostname.clone(),
            worker_host: context.get(&dev.worker_host).hostname.clone(),
            device_type: context.get(&dev.device_type).name.clone(),
            description: dev.description.clone(),
            health: dev.health.clone().try_into().unwrap(),
            tags: dev
                .tags
                .iter()
                .map(|t| Tag::from_mock(context.get(t), context.clone()))
                .collect::<Vec<_>>(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Lava;

    use boulder::{Buildable, Builder};
    use chrono::Utc;
    use futures::TryStreamExt;
    use lava_api_mock::{
        create_mock, PaginationLimits, PopulationParams, Server, SharedState, State,
    };
    use persian_rug::Accessor;
    use std::collections::{BTreeMap, BTreeSet};
    use std::iter::FromIterator;
    use test_log::test;

    /// Stream 50 devices with a page limit of 5 from the server
    /// checking that we correctly reconstruct their tags and that
    /// they are all accounted for (that pagination is handled
    /// properly)
    #[test(tokio::test)]
    async fn test_basic() {
        let state =
            SharedState::new_populated(PopulationParams::builder().devices(50usize).build());
        let server = Server::new(
            state.clone(),
            PaginationLimits::builder().devices(Some(5)).build(),
        )
        .await;

        let mut map = BTreeMap::new();
        let start = state.access();

        for device in start.get_iter::<lava_api_mock::Device<State>>() {
            map.insert(device.hostname.clone(), device);
        }

        let lava = Lava::new(&server.uri(), None).expect("failed to make lava server");

        let mut ld = lava.devices();

        let mut seen = BTreeMap::new();
        while let Some(device) = ld.try_next().await.expect("failed to get device") {
            assert!(!seen.contains_key(&device.hostname));
            assert!(map.contains_key(&device.hostname));
            let dev = map.get(&device.hostname).unwrap();
            assert_eq!(device.hostname, dev.hostname);
            assert_eq!(device.worker_host, start.get(&dev.worker_host).hostname);
            assert_eq!(device.device_type, start.get(&dev.device_type).name);
            assert_eq!(device.description, dev.description);
            assert_eq!(device.health.to_string(), dev.health.to_string());

            assert_eq!(device.tags.len(), dev.tags.len());
            for i in 0..device.tags.len() {
                assert_eq!(device.tags[i].id, start.get(&dev.tags[i]).id);
                assert_eq!(device.tags[i].name, start.get(&dev.tags[i]).name);
                assert_eq!(
                    device.tags[i].description,
                    start.get(&dev.tags[i]).description
                );
            }

            seen.insert(device.hostname.clone(), device.clone());
        }
        assert_eq!(seen.len(), 50);
    }

    #[test(tokio::test)]
    async fn test_basic_mock() {
        let (mut p, _clock) = create_mock(Utc::now()).await;

        let devices = BTreeSet::from_iter(p.generate_devices(50).into_iter());

        let lava = Lava::new(&p.uri(), None).expect("failed to make lava server");

        let mut ld = lava.devices();

        let mut seen = BTreeSet::new();
        while let Some(device) = ld.try_next().await.expect("failed to get device") {
            assert!(!seen.contains(&device.hostname));
            assert!(devices.contains(&device.hostname));

            p.with_device(&device.hostname, |dev| {
                assert_eq!(device.hostname, dev.hostname);
                p.with_proxy(&dev.worker_host, |h| {
                    assert_eq!(device.worker_host, h.hostname);
                });
                p.with_proxy(&dev.device_type, |dt| {
                    assert_eq!(device.device_type, dt.name);
                });
                assert_eq!(device.description, dev.description);
                assert_eq!(device.health.to_string(), dev.health.to_string());

                assert_eq!(device.tags.len(), dev.tags.len());
                for i in 0..device.tags.len() {
                    p.with_proxy(&dev.tags[i], |t| {
                        assert_eq!(device.tags[i].id, t.id);
                        assert_eq!(device.tags[i].name, t.name);
                        assert_eq!(device.tags[i].description, t.description);
                    });
                }
            });

            seen.insert(device.hostname.clone());
        }
        assert_eq!(seen.len(), 50);
    }
}
