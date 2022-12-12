use boulder::{BuildableWithPersianRug, GeneratableWithPersianRug};
use boulder::{Inc, Some as GSome, Time};
use chrono::{DateTime, Duration, Utc};
use django_query::{
    filtering::FilterableWithPersianRug, row::IntoRowWithPersianRug,
    sorting::SortableWithPersianRug,
};
use persian_rug::{contextual, Context, Proxy};
use strum::{Display, EnumString};

use crate::devices::Device;
use crate::devicetypes::{Alias, Architecture, BitWidth, Core, DeviceType, ProcessorFamily};
use crate::tags::Tag;
use crate::users::{Group, User};
use crate::workers::Worker;

/// A job from the LAVA API
// Filters from lava/lava_rest_app/filters.py
// FIXME: the model contains
// - is_public
// - target_group
// - sub_id
// That don't seem to appear in query output
#[derive(
    Clone,
    Debug,
    FilterableWithPersianRug,
    SortableWithPersianRug,
    IntoRowWithPersianRug,
    BuildableWithPersianRug,
    GeneratableWithPersianRug,
)]
#[django(
    persian_rug(
        context=C,
        access(
            Alias<C>,
            Architecture<C>,
            BitWidth<C>,
            Core<C>,
            Device<C>,
            DeviceType<C>,
            Group<C>,
            Job<C>,
            ProcessorFamily<C>,
            Tag<C>,
            User<C>,
            Worker<C>
        )
    )
)]
#[boulder(
    persian_rug(
        context=C,
        access(
            Alias<C>,
            Architecture<C>,
            BitWidth<C>,
            Core<C>,
            Device<C>,
            DeviceType<C>,
            Group<C>,
            Job<C>,
            ProcessorFamily<C>,
            Tag<C>,
            User<C>,
            Worker<C>
        )
    )
)]
#[contextual(C)]
pub struct Job<C: Context + 'static> {
    #[boulder(generator=Inc(0))]
    #[django(op(lt, gt, in), sort)]
    pub id: i64,
    #[boulder(buildable_with_persian_rug, generatable_with_persian_rug)]
    #[django(traverse, foreign_key = "username")]
    pub submitter: Proxy<User<C>>,
    #[boulder(generatable_with_persian_rug, sequence = 3usize)]
    #[django(traverse, foreign_key = "id")]
    pub viewing_groups: Vec<Proxy<Group<C>>>,
    // FIXME: verify: is this really mandatory?
    #[boulder(default = "Example job description")]
    #[django(op(in, contains, icontains, startswith, endswith))]
    pub description: String,
    #[boulder(default = true)]
    pub health_check: bool,
    #[boulder(buildable_with_persian_rug, generatable_with_persian_rug)]
    #[django(traverse, foreign_key = "name")]
    pub requested_device_type: Option<Proxy<DeviceType<C>>>,
    #[boulder(generatable_with_persian_rug, sequence = 4usize)]
    #[django(traverse, foreign_key = "id")]
    pub tags: Vec<Proxy<Tag<C>>>,
    #[boulder(buildable_with_persian_rug, generatable_with_persian_rug)]
    #[django(traverse, foreign_key = "hostname")]
    pub actual_device: Option<Proxy<Device<C>>>,
    #[boulder(default=Some(DateTime::parse_from_rfc3339("2022-03-17T17:00:00-00:00").unwrap().with_timezone(&Utc)),
              generator=GSome(Time::new(DateTime::parse_from_rfc3339("2022-03-17T17:00:00-00:00").unwrap().with_timezone(&Utc),
                                  Duration::minutes(1))))]
    #[django(op(gt, lt, isnull), sort)]
    pub submit_time: Option<DateTime<Utc>>,
    #[django(op(gt, lt, isnull), sort)]
    pub start_time: Option<DateTime<Utc>>,
    #[django(op(gt, lt, isnull), sort)]
    pub end_time: Option<DateTime<Utc>>,
    #[boulder(default=State::Submitted)]
    #[django(op(iexact, in))]
    pub state: State,
    #[boulder(default=Health::Unknown)]
    #[django(op(iexact, in))]
    pub health: Health,
    #[django(op(in, lt, gt, lte, gte))]
    pub priority: i64,
    #[boulder(default = "Example job definition")]
    #[django(op(in, contains, icontains, startswith, endswith))]
    pub definition: String,
    #[boulder(default = "Example job original definition")]
    #[django(op(in, contains, icontains, startswith, endswith))]
    pub original_definition: String,
    #[boulder(default = "Example job multinode definition")]
    #[django(op(in, contains, icontains, startswith, endswith))]
    pub multinode_definition: String,
    #[django(traverse, foreign_key = "id")]
    pub failure_tags: Vec<Proxy<Tag<C>>>,
    #[django(op(in, contains, icontains, startswith, endswith, isnull))]
    pub failure_comment: Option<String>,
}

/// The health (i.e. completion type) of a [`Job`] in the LAVA API
#[derive(Copy, Clone, Debug, PartialEq, Eq, EnumString, Display)]
pub enum Health {
    Unknown,
    Complete,
    Incomplete,
    Canceled,
}

impl django_query::filtering::ops::Scalar for Health {}
impl django_query::row::StringCellValue for Health {}

/// The state (i.e. progress) of a [`Job`] in the LAVA API
#[derive(Copy, Clone, Debug, PartialEq, Eq, EnumString, Display)]
pub enum State {
    Submitted,
    Scheduling,
    Scheduled,
    Running,
    Canceling,
    Finished,
}

impl django_query::filtering::ops::Scalar for State {}
impl django_query::row::StringCellValue for State {}

#[cfg(test)]
mod tests {
    use super::*;

    use anyhow::Result;
    use boulder::Repeat;
    use boulder::{
        BuildableWithPersianRug, BuilderWithPersianRug, GeneratorWithPersianRugIterator,
    };
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
    async fn test_jobs() {
        let mut p = crate::state::SharedState::new();
        {
            let m = p.mutate();

            let (submitter, m) = Proxy::<User<_>>::builder().username("fred").build(m);
            let (device_type, mut m) = Proxy::<DeviceType<_>>::builder().name("big one").build(m);
            m.add(Job {
                id: 1,
                submitter,
                viewing_groups: Vec::new(),
                description: "A job submitted by Fred".to_string(),
                health_check: false,
                requested_device_type: Some(device_type),
                tags: Vec::new(),
                actual_device: None,
                submit_time: Some(Utc::now()),
                start_time: None,
                end_time: None,
                state: State::Scheduled,
                health: Health::Unknown,
                priority: 1,
                definition: "/bin/some_stuff".to_string(),
                original_definition: "/usr/bin/other_stuff".to_string(),
                multinode_definition: String::new(),
                failure_tags: Vec::new(),
                failure_comment: None,
            });

            let (submitter, m) = Proxy::<User<_>>::builder().username("jane").build(m);
            let (device_type, mut m) = Proxy::<DeviceType<_>>::builder().name("anything").build(m);
            m.add(Job {
                id: 2,
                submitter,
                viewing_groups: Vec::new(),
                description: "A job submitted by Jane".to_string(),
                health_check: false,
                requested_device_type: Some(device_type),
                tags: Vec::new(),
                actual_device: None,
                submit_time: Some(Utc::now()),
                start_time: None,
                end_time: None,
                state: State::Submitted,
                health: Health::Incomplete,
                priority: 1,
                definition: "/bin/some_stuff".to_string(),
                original_definition: "/usr/bin/other_stuff".to_string(),
                multinode_definition: String::new(),
                failure_tags: Vec::new(),
                failure_comment: None,
            });
        }

        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/v0.2/jobs/"))
            .respond_with(p.endpoint::<Job<_>>(Some(&server.uri()), None))
            .mount(&server)
            .await;

        let jobs = make_request(server.uri(), "jobs/")
            .await
            .expect("failed to query jobs");

        assert_eq!(jobs["results"][0]["id"], json!(1));
        assert_eq!(jobs["results"][1]["id"], json!(2));
        assert_eq!(jobs["results"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_job_builder() {
        let mut p = crate::state::SharedState::new();
        {
            let m = p.mutate();

            let (submitter, m) = Proxy::<User<_>>::builder().username("fred").build(m);
            let (device_type, m) = Proxy::<DeviceType<_>>::builder().name("big one").build(m);
            let (job, mut m) = Job::builder()
                .id(1)
                .submitter(submitter)
                .requested_device_type(device_type)
                .state(State::Scheduled)
                .start_time(None)
                .build(m);
            m.add(job);

            let (submitter, m) = Proxy::<User<_>>::builder().username("jane").build(m);
            let (device_type, m) = Proxy::<DeviceType<_>>::builder().name("anything").build(m);
            let (job, mut m) = Job::builder()
                .id(2)
                .submitter(submitter)
                .requested_device_type(device_type)
                .state(State::Submitted)
                .start_time(None)
                .build(m);
            m.add(job);
        }

        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/v0.2/jobs/"))
            .respond_with(p.endpoint::<Job<_>>(Some(&server.uri()), None))
            .mount(&server)
            .await;

        let jobs = make_request(server.uri(), "jobs/")
            .await
            .expect("failed to query jobs");

        assert_eq!(jobs["results"][0]["id"], json!(1));
        assert_eq!(jobs["results"][1]["id"], json!(2));
        assert_eq!(jobs["results"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_job_stream() {
        let mut p = crate::state::SharedState::new();
        {
            let m = p.mutate();
            let (user1, m) = Proxy::<User<_>>::builder().username("fred").build(m);
            let (user2, m) = Proxy::<User<_>>::builder().username("jane").build(m);
            let _ = GeneratorWithPersianRugIterator::new(
                Proxy::<Job<crate::state::State>>::generator().submitter(Repeat!(user1, user2)),
                m,
            )
            .take(2)
            .collect::<Vec<_>>();
        }

        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/v0.2/jobs/"))
            .respond_with(p.endpoint::<Job<_>>(Some(&server.uri()), None))
            .mount(&server)
            .await;

        let jobs = make_request(server.uri(), "jobs/")
            .await
            .expect("failed to query jobs");

        assert_eq!(jobs["results"][0]["id"], json!(0));
        assert_eq!(jobs["results"][1]["id"], json!(1));
        assert_eq!(jobs["results"].as_array().unwrap().len(), 2);
    }

    #[test(tokio::test)]
    async fn test_output() {
        let mut p = crate::state::SharedState::new();
        {
            let m = p.mutate();

            let gen = Proxy::<Job<crate::state::State>>::generator()
                .state(|| State::Finished)
                .health(|| Health::Complete)
                .submit_time(GSome(Time::new(
                    DateTime::parse_from_rfc3339("2022-04-11T05:00:00-00:00")
                        .unwrap()
                        .with_timezone(&Utc),
                    Duration::minutes(5),
                )))
                .start_time(GSome(Time::new(
                    DateTime::parse_from_rfc3339("2022-04-11T05:30:00-00:00")
                        .unwrap()
                        .with_timezone(&Utc),
                    Duration::minutes(5),
                )))
                .end_time(GSome(Time::new(
                    DateTime::parse_from_rfc3339("2022-04-11T06:00:00-00:00")
                        .unwrap()
                        .with_timezone(&Utc),
                    Duration::minutes(5),
                )))
                .tags(Vec::new)
                .viewing_groups(Vec::new)
                .multinode_definition(String::new)
                .health_check(Repeat!(false, true))
                .priority(Repeat!(0, 50));

            let _ = GeneratorWithPersianRugIterator::new(gen, m)
                .take(4)
                .collect::<Vec<_>>();
        }

        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/v0.2/jobs/"))
            .respond_with(p.endpoint::<Job<_>>(Some(&server.uri()), None))
            .mount(&server)
            .await;

        let body = make_request(server.uri(), "jobs/?limit=2")
            .await
            .expect("failed to query jobs");

        let next = format!("{}/api/v0.2/jobs/?limit=2&offset=2", server.uri());

        assert_eq!(
            body,
            serde_json::json! {
                {
                    "count": 4,
                    "next": next,
                    "previous": null,
                    "results": [
                        {
                            "id": 0,
                            "submitter": "test-user-1",
                            "viewing_groups": [

                            ],
                            "description": "Example job description",
                            "health_check": false,
                            "requested_device_type": "test-device-type-0",
                            "tags": [

                            ],
                            "actual_device": "test-device-0",
                            "submit_time": "2022-04-11T05:00:00.000000Z",
                            "start_time": "2022-04-11T05:30:00.000000Z",
                            "end_time": "2022-04-11T06:00:00.000000Z",
                            "state": "Finished",
                            "health": "Complete",
                            "priority": 0,
                            "definition": "Example job definition",
                            "original_definition": "Example job original definition",
                            "multinode_definition": "",
                            "failure_tags": [

                            ],
                            "failure_comment": null
                        },
                        {
                            "id": 1,
                            "submitter": "test-user-2",
                            "viewing_groups": [

                            ],
                            "description": "Example job description",
                            "health_check": true,
                            "requested_device_type": "test-device-type-1",
                            "tags": [

                            ],
                            "actual_device": "test-device-1",
                            "submit_time": "2022-04-11T05:05:00.000000Z",
                            "start_time": "2022-04-11T05:35:00.000000Z",
                            "end_time": "2022-04-11T06:05:00.000000Z",
                            "state": "Finished",
                            "health": "Complete",
                            "priority": 50,
                            "definition": "Example job definition",
                            "original_definition": "Example job original definition",
                            "multinode_definition": "",
                            "failure_tags": [

                            ],
                            "failure_comment": null
                        },
                    ]
                }
            }
        );
    }
}
