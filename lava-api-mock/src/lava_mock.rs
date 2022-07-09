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

        let mock = LavaMock::new(s, Default::default()).await;

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
