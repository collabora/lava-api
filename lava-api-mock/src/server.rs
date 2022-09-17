use crate::state::{SharedState, State};
use crate::{Alias, Device, DeviceType, Job, Tag, TestCase, TestSuite, Worker};

use boulder::Buildable;
use clone_replace::MutateGuard;
use django_query::mock::{nested_endpoint_matches, NestedEndpointParams};
use std::sync::Arc;

/// Pagination limits for constructing a [`Server`] instance.
///
/// A running Lava instance allows the default pagination of endpoints
/// to be customised, and specifying default pagination can be
/// important for checking it is properly handled in clients that do
/// not usually specify their pagination directly.
///
/// Each member is an [`Option`], with `None` meaning that no
/// pagination is applied, otherwise `Some(n)` means that a maximum of
/// `n` results for objects of this type will be returned. The default
/// object provides no pagination for anything.
#[derive(Buildable, Clone, Default)]
pub struct PaginationLimits {
    aliases: Option<usize>,
    test_cases: Option<usize>,
    test_suites: Option<usize>,
    jobs: Option<usize>,
    device_types: Option<usize>,
    devices: Option<usize>,
    tags: Option<usize>,
    workers: Option<usize>,
}

impl PaginationLimits {
    /// Create a new [`PaginationLimits`]
    ///
    /// The created object will not ever trigger pagination by default
    /// for any endpoint.
    pub fn new() -> Self {
        Default::default()
    }
}

/// A mock server that provides access to a [`SharedState`].
///
/// This provides the following endpoints from the v0.2 Lava REST API:
/// - `/api/v0.2/aliases/`
/// - `/api/v0.2/devices/`
/// - `/api/v0.2/devicetypes/`
/// - `/api/v0.2/jobs/`
/// - `/api/v0.2/tags/`
/// - `/api/v0.2/workers/`
///
/// It also provides the following nested endpoints for jobs:
/// - `/api/v0.2/jobs/<id>/tests/`
/// - `/api/v0.2/jobs/<id>/suites/`
///
/// You can use [`uri`](Server::uri) to find the initial portion
/// of the URL for your test instance.
///
/// The mock object does not support the Lava mutation endpoints, but
/// you can mutate the provided [`SharedState`] directly for testing.
/// There are two ways to do this:
/// - You can keep a clone of the [`SharedState`] you pass in and obtain
///   a [`MutateGuard`] with [`mutate`](SharedState::mutate).
/// - You can call [`state_mut`](Server::state_mut) to get a [`MutateGuard`]
///   for the enclosed [`SharedState`] directly.
pub struct Server {
    server: wiremock::MockServer,
    state: SharedState,
}

impl Server {
    /// Create and start a new [`Server`]
    ///
    /// Here `p` is the [`SharedState`] becomes the underlying data
    /// source for the mock, and `limits` are the default pagination
    /// limits as a [`PaginationLimits`] object, which are applied
    /// when the client does not give any.
    pub async fn new(p: SharedState, limits: PaginationLimits) -> Server {
        let s = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/v0.2/aliases/"))
            .respond_with(p.endpoint::<Alias<State>>(Some(&s.uri()), limits.aliases))
            .mount(&s)
            .await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(nested_endpoint_matches("/api/v0.2", "jobs", "tests"))
            .respond_with(p.nested_endpoint::<TestCase<State>>(
                NestedEndpointParams {
                    root: "/api/v0.2",
                    parent: "jobs",
                    child: "tests",
                    parent_query: "suite__job__id",
                    base_uri: Some(&s.uri()),
                },
                limits.test_cases,
            ))
            .mount(&s)
            .await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path_regex(
                r"^/api/v0.2/jobs/\d+/suites/$",
            ))
            .respond_with(p.nested_endpoint::<TestSuite<State>>(
                NestedEndpointParams {
                    root: "/api/v0.2",
                    parent: "jobs",
                    child: "suites",
                    parent_query: "suite__job__id",
                    base_uri: Some(&s.uri()),
                },
                limits.test_suites,
            ))
            .mount(&s)
            .await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/v0.2/jobs/"))
            .respond_with(p.endpoint::<Job<State>>(Some(&s.uri()), limits.jobs))
            .mount(&s)
            .await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/v0.2/devicetypes/"))
            .respond_with(p.endpoint::<DeviceType<State>>(Some(&s.uri()), limits.device_types))
            .mount(&s)
            .await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/v0.2/devices/"))
            .respond_with(p.endpoint::<Device<State>>(Some(&s.uri()), limits.devices))
            .mount(&s)
            .await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/v0.2/tags/"))
            .respond_with(p.endpoint::<Tag<State>>(Some(&s.uri()), limits.tags))
            .mount(&s)
            .await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/v0.2/workers/"))
            .respond_with(p.endpoint::<Worker<State>>(Some(&s.uri()), limits.workers))
            .mount(&s)
            .await;

        Server {
            server: s,
            state: p,
        }
    }

    /// Create and start a default new [`Server`].
    ///
    /// This mock will have a default [`SharedState`] and default
    /// [`PaginationLimits`]. This gives a mock object with an empty
    /// data store, and no default pagination (so if the client does
    /// not request pagination, all matching data will be returned).
    pub async fn start() -> Self {
        Self::new(Default::default(), Default::default()).await
    }

    /// Return the URI of the server.
    ///
    /// This object is based on a [`wiremock`] server, and as such it
    /// will usually be bound to an ephemeral port.
    pub fn uri(&self) -> String {
        self.server.uri()
    }

    /// Read a read-only view of the current state of the data store.
    ///
    /// Note that the data store is not currently prevented from
    /// evolving while this snapshot is held, because the underlying
    /// synchronisation mechanism is a
    /// [`CloneReplace`](clone_replace::CloneReplace).
    pub fn state(&self) -> Arc<State> {
        self.state.access()
    }

    /// Read a mutable view of the current state of the data store.
    ///
    /// Note that the data store is not currently prevented from
    /// evolving while this snapshot is held, because the underlying
    /// synchronisation mechanism is a
    /// [`CloneReplace`](clone_replace::CloneReplace). Other writers
    /// are not prevented from acting on the data store, and their
    /// changes will be lost when this guard is flushed. Note that
    /// changes from a [`MutateGuard`] only take effect when the guard
    /// is dropped.
    pub fn state_mut(&mut self) -> MutateGuard<State> {
        self.state.mutate()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::{devicetypes::DeviceType, Device, Job, JobState};

    use anyhow::Result;
    use boulder::{
        BuildableWithPersianRug, BuilderWithPersianRug, GeneratableWithPersianRug,
        TryRepeatFromPersianRug,
    };
    use boulder::{GeneratorToGeneratorWithPersianRugWrapper, GeneratorWithPersianRugMutIterator};
    use chrono::Utc;
    use persian_rug::Proxy;
    use rand::{Rng, SeedableRng};
    use serde_json::Value;

    async fn make_request<T, U>(server_uri: T, endpoint: U) -> Result<Value>
    where
        T: AsRef<str>,
        U: AsRef<str>,
    {
        let url = format!("{}/api/v0.2/{}", server_uri.as_ref(), endpoint.as_ref());
        Ok(reqwest::get(&url).await?.json().await?)
    }

    #[tokio::test]
    async fn test() {
        let mut s = SharedState::new();

        let mut rng = rand::rngs::StdRng::seed_from_u64(0xdeadbeef);
        let device_types = ["device-type-1", "device-type-2"]
            .into_iter()
            .map(|name| {
                Proxy::<DeviceType<State>>::builder()
                    .name(name)
                    .build(s.mutate())
                    .0
            })
            .collect::<Vec<_>>();

        let types = device_types.clone();
        let mut devices = Proxy::<Device<State>>::generator().device_type(
            GeneratorToGeneratorWithPersianRugWrapper::new(move || {
                types[rng.gen_range(0..types.len())]
            }),
        );

        let _ = GeneratorWithPersianRugMutIterator::new(&mut devices, s.mutate())
            .take(90)
            .collect::<Vec<_>>();

        let mut rng = rand::rngs::StdRng::seed_from_u64(0xdeadbeef);

        let types = device_types.clone();
        let mut jobs = Proxy::<Job<State>>::generator()
            .actual_device(TryRepeatFromPersianRug::new())
            .state(GeneratorToGeneratorWithPersianRugWrapper::new(|| {
                JobState::Submitted
            }))
            .submit_time(GeneratorToGeneratorWithPersianRugWrapper::new(|| {
                Some(Utc::now())
            }))
            .requested_device_type(GeneratorToGeneratorWithPersianRugWrapper::new(move || {
                Some(types[rng.gen_range(0..types.len())])
            }));

        let _ = GeneratorWithPersianRugMutIterator::new(&mut jobs, s.mutate())
            .take(500)
            .collect::<Vec<_>>();

        let mock = Server::new(s, Default::default()).await;

        let devices = make_request(mock.uri(), "devices/")
            .await
            .expect("failed to query devices");

        assert_eq!(devices["results"].as_array().unwrap().len(), 90);

        let jobs = make_request(mock.uri(), "jobs/")
            .await
            .expect("failed to query jobs");

        assert_eq!(jobs["results"].as_array().unwrap().len(), 500);
    }
}
