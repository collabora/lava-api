use chrono::{DateTime, Utc};
use strum::{Display, EnumString};

use boulder::{BuildableWithPersianRug, GeneratableWithPersianRug};
use boulder::{Inc, Pattern};
use django_query::filtering::{ops::Scalar, FilterableWithPersianRug};
use django_query::{row::IntoRowWithPersianRug, sorting::SortableWithPersianRug};

use persian_rug::{contextual, Context};

/// A worker in the LAVA API
#[derive(
    Clone,
    Debug,
    IntoRowWithPersianRug,
    FilterableWithPersianRug,
    SortableWithPersianRug,
    BuildableWithPersianRug,
    GeneratableWithPersianRug,
)]
#[boulder(persian_rug(context=C, access(Worker<C>)))]
#[django(persian_rug(context=C, access(Worker<C>)))]
#[contextual(C)]
pub struct Worker<C: Context + 'static> {
    #[django(exclude)]
    _marker: core::marker::PhantomData<C>,
    #[boulder(generator=Pattern!("a-test-worker-{}", Inc(1)))]
    #[django(sort, op(in, contains, icontains, startswith, endswith))]
    pub hostname: String,
    #[boulder(default=Some("A test worker".to_string()))]
    #[django(sort, op(in, contains, icontains, startswith, endswith))]
    pub description: Option<String>,
    #[boulder(default=DateTime::parse_from_rfc3339("2022-03-17T17:00:00-00:00").unwrap().with_timezone(&Utc))]
    #[django(sort, op(lt, gt))]
    pub last_ping: Option<DateTime<Utc>>,
    #[boulder(default=State::Online)]
    #[django(sort)]
    pub state: State,
    #[boulder(default=Health::Active)]
    #[django(sort)]
    pub health: Health,
    #[boulder(default = 100)]
    #[django(unfilterable)]
    pub job_limit: i64,
    #[boulder(default=Some("1.0".to_string()))]
    #[django(unfilterable)]
    pub version: Option<String>,
    #[boulder(default=Some("1.0".to_string()))]
    #[django(unfilterable)]
    pub master_version_notified: Option<String>,
}

/// The health (i.e. status) of a [`Worker`] in the LAVA API
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Display, EnumString)]
pub enum Health {
    Active,
    Maintenance,
    Retired,
}

impl Scalar for Health {}
impl django_query::row::StringCellValue for Health {}

/// The (power) state of a [`Worker`] in the LAVA API
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Display, EnumString)]
pub enum State {
    Online,
    Offline,
}

impl Scalar for State {}
impl django_query::row::StringCellValue for State {}

#[cfg(test)]
mod test {
    use super::*;
    use crate::SharedState;

    use anyhow::Result;
    use boulder::BuilderWithPersianRug;
    use boulder::GeneratorWithPersianRugIterator;
    use boulder::{Repeat, Some as GSome, Time};
    use chrono::{DateTime, Duration, Utc};
    use persian_rug::Proxy;
    use serde_json::json;
    use test_log::test;

    async fn make_request<T, U>(server_uri: T, endpoint: U) -> Result<serde_json::Value>
    where
        T: AsRef<str>,
        U: AsRef<str>,
    {
        let url = format!("{}/api/v0.2/{}", server_uri.as_ref(), endpoint.as_ref());
        Ok(reqwest::get(&url).await?.json().await?)
    }

    #[tokio::test]
    async fn test_workers() {
        let mut p = SharedState::new();
        {
            let mut m = p.mutate();

            m.add(Worker {
                _marker: Default::default(),
                hostname: "test2".to_string(),
                health: Health::Active,
                state: State::Online,
                description: Some("description of worker".to_string()),
                last_ping: Some(Utc::now()),
                job_limit: 0,
                version: None,
                master_version_notified: None,
            });

            m.add(Worker {
                _marker: Default::default(),
                hostname: "test1".to_string(),
                health: Health::Maintenance,
                state: State::Offline,
                description: Some("description of worker".to_string()),
                last_ping: Some(Utc::now() - chrono::Duration::seconds(10)),
                job_limit: 0,
                version: None,
                master_version_notified: None,
            });
        }

        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/v0.2/workers/"))
            .respond_with(p.endpoint::<Worker<_>>(Some(&server.uri()), None))
            .mount(&server)
            .await;

        let workers = make_request(server.uri(), "workers/")
            .await
            .expect("failed to query workers");

        assert_eq!(workers["results"][0]["hostname"], json!("test2"));
        assert_eq!(workers["results"][1]["hostname"], json!("test1"));
        assert_eq!(workers["results"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_worker_builder() {
        let mut p = SharedState::new();
        {
            let m = p.mutate();

            let (w, mut m) = Worker::builder().hostname("test2").build(m);
            m.add(w);
            let (w, mut m) = Worker::builder().hostname("test1").build(m);
            m.add(w);
        }

        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/v0.2/workers/"))
            .respond_with(p.endpoint::<Worker<_>>(Some(&server.uri()), None))
            .mount(&server)
            .await;

        let workers = make_request(server.uri(), "workers/")
            .await
            .expect("failed to query workers");

        assert_eq!(workers["results"][0]["hostname"], json!("test2"));
        assert_eq!(workers["results"][1]["hostname"], json!("test1"));
        assert_eq!(workers["results"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_worker_stream() {
        let mut p = SharedState::new();
        {
            let m = p.mutate();

            let _ = GeneratorWithPersianRugIterator::new(
                Proxy::<Worker<crate::state::State>>::generator()
                    .state(Repeat!(State::Offline, State::Online)),
                m,
            )
            .take(2)
            .collect::<Vec<_>>();
        }

        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/v0.2/workers/"))
            .respond_with(p.endpoint::<Worker<_>>(Some(&server.uri()), None))
            .mount(&server)
            .await;

        let workers = make_request(server.uri(), "workers/")
            .await
            .expect("failed to query workers");

        assert_eq!(workers["results"][0]["hostname"], json!("a-test-worker-1"));
        assert_eq!(workers["results"][1]["hostname"], json!("a-test-worker-2"));
        assert_eq!(workers["results"].as_array().unwrap().len(), 2);
    }

    #[test(tokio::test)]
    async fn test_output() {
        let mut p = SharedState::new();
        {
            let m = p.mutate();

            let gen = Proxy::<Worker<crate::state::State>>::generator()
                .state(Repeat!(State::Online, State::Offline))
                .health(Repeat!(Health::Active, Health::Retired))
                .last_ping(GSome(Time::new(
                    DateTime::parse_from_rfc3339("2022-04-11T21:00:00-00:00")
                        .unwrap()
                        .with_timezone(&Utc),
                    Duration::minutes(30),
                )))
                .job_limit(|| 0)
                .master_version_notified(|| None);

            let _ = GeneratorWithPersianRugIterator::new(gen, m)
                .take(5)
                .collect::<Vec<_>>();
        }

        let server = wiremock::MockServer::start().await;

        let ep = p.endpoint::<Worker<_>>(Some(&server.uri()), None);

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/v0.2/workers/"))
            .respond_with(ep)
            .mount(&server)
            .await;

        let body = make_request(server.uri(), "workers/?limit=2&offset=1")
            .await
            .expect("failed to query workers");

        let next = format!("{}/api/v0.2/workers/?limit=2&offset=3", server.uri());
        let prev = format!("{}/api/v0.2/workers/?limit=2", server.uri());

        assert_eq!(
            body,
            serde_json::json! {
                {
                    "count": 5,
                    "next": next,
                    "previous": prev,
                    "results": [
                        {
                            "hostname": "a-test-worker-2",
                            "health": "Retired",
                            "state": "Offline",
                            "description": "A test worker",
                            "last_ping": "2022-04-11T21:30:00.000000Z",
                            "job_limit": 0,
                            "version": "1.0",
                            "master_version_notified": null
                        },
                        {
                            "hostname": "a-test-worker-3",
                            "health": "Active",
                            "state": "Online",
                            "description": "A test worker",
                            "last_ping": "2022-04-11T22:00:00.000000Z",
                            "job_limit": 0,
                            "version": "1.0",
                            "master_version_notified": null
                        }
                    ]
                }
            }
        );
    }
}
