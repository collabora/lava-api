use boulder::{BuildableWithPersianRug, GeneratableWithPersianRug};
use boulder::{Inc, Pattern, Some as GSome};
use django_query::{
    filtering::FilterableWithPersianRug, row::IntoRowWithPersianRug,
    sorting::SortableWithPersianRug,
};
use persian_rug::{contextual, Context, Proxy};
use strum::{Display, EnumString};

use crate::{
    Alias, Architecture, BitWidth, Core, DeviceType, Group, Job, ProcessorFamily, Tag, User, Worker,
};

/// A device from the LAVA API.
#[derive(
    Clone,
    Debug,
    FilterableWithPersianRug,
    SortableWithPersianRug,
    IntoRowWithPersianRug,
    BuildableWithPersianRug,
    GeneratableWithPersianRug,
)]
#[boulder(
    persian_rug(
        context=C,
        access(
            Device<C>,
            DeviceType<C>,
            Alias<C>,
            Architecture<C>,
            BitWidth<C>,
            Core<C>,
            ProcessorFamily<C>,
            User<C>,
            Group<C>,
            Worker<C>,
            Job<C>,
            Tag<C>
        )
    )
)]
#[django(
    persian_rug(
        context=C,
        access(
            Device<C>,
            DeviceType<C>,
            Alias<C>,
            Architecture<C>,
            BitWidth<C>,
            Core<C>,
            ProcessorFamily<C>,
            User<C>,
            Group<C>,
            Worker<C>,
            Job<C>,
            Tag<C>
        )
    )
)]
#[contextual(C)]
pub struct Device<C: Context + 'static> {
    #[boulder(default="test-device",
              generator=Pattern!("test-device-{}", Inc(0)))]
    #[django(sort, op(in, contains, icontains, startswith, endswith))]
    pub hostname: String,
    #[boulder(buildable_with_persian_rug, generatable_with_persian_rug)]
    #[django(sort("name"), traverse, foreign_key = "name")]
    pub device_type: Proxy<DeviceType<C>>,
    #[boulder(default=Some("1.0".to_string()))]
    #[django(sort, op(in, contains, icontains, startswith, endswith))]
    pub device_version: Option<String>,
    #[boulder(buildable_with_persian_rug, generatable_with_persian_rug)]
    #[django(sort("id"), traverse, foreign_key = "id")]
    pub physical_owner: Option<Proxy<User<C>>>,
    #[boulder(buildable_with_persian_rug, generatable_with_persian_rug)]
    #[django(sort("id"), traverse, foreign_key = "id")]
    pub physical_group: Option<Proxy<Group<C>>>,
    #[boulder(default=Some("Test description for device.".to_string()),
              generator=GSome(Pattern!("Test description {}", Inc(0))))]
    #[django(sort, op(in, contains, icontains, startswith, endswith))]
    pub description: Option<String>,

    /* The API docs claim `tags` is a sort field, but testing this
    reveals it's simply ignored. There's no obvious way to sort by
    a vector of key ID, and the output is definitely not sorted.
    */
    #[boulder(generatable_with_persian_rug, sequence = 3usize)]
    #[django(traverse, foreign_key = "id")]
    pub tags: Vec<Proxy<Tag<C>>>,

    #[django(sort)]
    #[boulder(default=State::Idle)]
    pub state: State,
    #[django(sort)]
    #[boulder(default=Health::Good)]
    pub health: Health,
    #[boulder(buildable_with_persian_rug, generatable_with_persian_rug)]
    #[django(sort("hostname"), traverse, foreign_key = "hostname")]
    pub worker_host: Proxy<Worker<C>>,

    #[django(unfilterable)]
    pub is_synced: bool,
    #[django(unfilterable, foreign_key = "id")]
    pub last_health_report_job: Option<Proxy<Job<C>>>,
}

/// The health status of a [`Device`] from the LAVA API.
#[derive(Clone, Debug, PartialEq, Eq, EnumString, PartialOrd, Ord, Display)]
pub enum Health {
    Unknown,
    Maintenance,
    Good,
    Bad,
    Looping,
    Retired,
}

impl django_query::filtering::ops::Scalar for Health {}
impl django_query::row::StringCellValue for Health {}

/// The state of a [`Device`] from the LAVA API.
#[derive(Clone, Debug, PartialEq, Eq, EnumString, PartialOrd, Ord, Display)]
pub enum State {
    Idle,
    Reserved,
    Running,
}

impl django_query::filtering::ops::Scalar for State {}
impl django_query::row::StringCellValue for State {}

#[cfg(test)]
mod test {
    use super::*;
    use crate::state::{self, SharedState};

    use anyhow::Result;
    use boulder::BuilderWithPersianRug;
    use boulder::GeneratorWithPersianRugIterator;
    use boulder::Repeat;
    use serde_json::{json, Value};
    use test_log::test;

    async fn make_request<T, U>(server_uri: T, endpoint: U) -> Result<Value>
    where
        T: AsRef<str>,
        U: AsRef<str>,
    {
        let url = format!("{}/api/v0.2/{}", server_uri.as_ref(), endpoint.as_ref());
        Ok(reqwest::get(&url).await?.json().await?)
    }

    #[tokio::test]
    async fn test_devices() {
        let mut p = SharedState::new();
        {
            let m = p.mutate();

            let (worker, m) = Proxy::<Worker<_>>::builder().hostname("worker1").build(m);
            let (device_type, m) = Proxy::<DeviceType<_>>::builder().name("type1").build(m);
            let (_, m) = Proxy::<Device<_>>::builder()
                .hostname("test1")
                .worker_host(worker)
                .device_type(device_type)
                .description(Some("description of device".to_string()))
                .health(Health::Good)
                .build(m);

            let (worker, m) = Proxy::<Worker<_>>::builder().hostname("worker2").build(m);
            let (device_type, m) = Proxy::<DeviceType<_>>::builder().name("type2").build(m);
            let _ = Proxy::<Device<state::State>>::builder()
                .hostname("test2")
                .worker_host(worker)
                .device_type(device_type)
                .description(Some("description of device".to_string()))
                .health(Health::Bad)
                .build(m);
        }

        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/v0.2/devices/"))
            .respond_with(p.endpoint::<Device<state::State>>(Some(&server.uri()), None))
            .mount(&server)
            .await;

        let devices = make_request(server.uri(), "devices/")
            .await
            .expect("failed to query devices");

        assert_eq!(devices["results"][0]["hostname"], json!("test1"));
        assert_eq!(devices["results"][1]["hostname"], json!("test2"));
        assert_eq!(devices["results"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_device_builder() {
        let mut p = SharedState::new();
        {
            let m = p.mutate();
            let (worker, m) = Proxy::<Worker<_>>::builder().hostname("worker1").build(m);
            let (_, m) = Proxy::<Device<state::State>>::builder()
                .hostname("test1")
                .worker_host(worker)
                .build(m);
            let (worker, m) = Proxy::<Worker<_>>::builder().hostname("worker2").build(m);
            let _ = Proxy::<Device<state::State>>::builder()
                .hostname("test2")
                .worker_host(worker)
                .build(m);
        }

        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/v0.2/devices/"))
            .respond_with(p.endpoint::<Device<state::State>>(Some(&server.uri()), None))
            .mount(&server)
            .await;

        let devices = make_request(server.uri(), "devices/")
            .await
            .expect("failed to query devices");

        assert_eq!(devices["results"][0]["hostname"], json!("test1"));
        assert_eq!(devices["results"][1]["hostname"], json!("test2"));
        assert_eq!(devices["results"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_device_stream() {
        let mut p = SharedState::new();
        {
            let m = p.mutate();
            let devices = Proxy::<Device<_>>::generator().hostname(Repeat!("w1", "w2"));
            let _ = GeneratorWithPersianRugIterator::new(devices, m)
                .take(2)
                .collect::<Vec<_>>();
        }

        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/v0.2/devices/"))
            .respond_with(p.endpoint::<Device<state::State>>(Some(&server.uri()), None))
            .mount(&server)
            .await;

        let devices = make_request(server.uri(), "devices/")
            .await
            .expect("failed to query devices");

        assert_eq!(devices["results"][0]["hostname"], json!("w1"));
        assert_eq!(devices["results"][1]["hostname"], json!("w2"));
        assert_eq!(devices["results"].as_array().unwrap().len(), 2);
    }

    #[test(tokio::test)]
    async fn test_output() {
        let mut p = SharedState::new();
        {
            let m = p.mutate();
            // let mut tag_gen = Proxy::<Tag<_>>::generator();

            let gen = Proxy::<Device<_>>::generator()
                .health(Repeat!(Health::Maintenance, Health::Good))
                .description(|| None)
                .device_version(|| None)
                .physical_owner(|| None)
                .physical_group(|| None)
                .last_health_report_job(|| None); // GSome(Proxy::<Job<state::State>>::generator())
                                                  //gen.tags(move || (&mut tag_gen).into_iter().take(2).collect::<Vec<_>>());

            let _ = GeneratorWithPersianRugIterator::new(gen, m)
                .take(4)
                .collect::<Vec<_>>();
        }

        let server = wiremock::MockServer::start().await;
        let ep = p.endpoint::<Device<_>>(Some(&server.uri()), Some(2));

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/v0.2/devices/"))
            .respond_with(ep)
            .mount(&server)
            .await;

        let body: serde_json::Value =
            reqwest::get(&format!("{}/api/v0.2/devices/?limit=2", server.uri()))
                .await
                .expect("error getting devices")
                .json()
                .await
                .expect("error parsing devices");

        let next = format!("{}/api/v0.2/devices/?limit=2&offset=2", server.uri());

        assert_eq!(
            body,
            serde_json::json! {
                {
                    "count": 4,
                    "next": next,
                    "previous": null,
                    "results": [
                        {
                            "hostname": "test-device-0",
                            "device_type": "test-device-type-0",
                            "device_version": null,
                            "physical_owner": null,
                            "physical_group": null,
                            "description": null,
                            "tags": [
                                0,
                                1,
                                2
                            ],
                            "state": "Idle",
                            "health": "Maintenance",
                            "last_health_report_job": null,
                            "worker_host": "a-test-worker-1",
                            "is_synced": false
                        },
                        {
                            "hostname": "test-device-1",
                            "device_type": "test-device-type-1",
                            "device_version": null,
                            "physical_owner": null,
                            "physical_group": null,
                            "description": null,
                            "tags": [
                                3,
                                4,
                                5
                            ],
                            "state": "Idle",
                            "health": "Good",
                            "last_health_report_job": null,
                            "worker_host": "a-test-worker-2",
                            "is_synced": false
                        },
                    ]
                }
            }
        );
    }
}
