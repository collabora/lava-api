use crate::state::{SharedState, State};
use crate::{Alias, Device, DeviceType, Job, Tag, TestCase, TestSuite, Worker};

use boulder::Buildable;
use clone_replace::MutateGuard;
use django_query::mock::{nested_endpoint_matches, NestedEndpointParams};
use std::sync::Arc;

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
    pub fn new() -> Self {
        Default::default()
    }
}

pub struct LavaMock {
    server: wiremock::MockServer,
    state: SharedState,
}

impl LavaMock {
    pub async fn new(p: SharedState, limits: PaginationLimits) -> LavaMock {
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

        LavaMock {
            server: s,
            state: p,
        }
    }

    pub async fn start() -> Self {
        Self::new(Default::default(), Default::default()).await
    }

    pub fn uri(&self) -> String {
        self.server.uri()
    }

    pub fn state(&self) -> Arc<State> {
        self.state.access()
    }

    pub fn state_mut(&mut self) -> MutateGuard<State> {
        self.state.mutate()
    }
}
