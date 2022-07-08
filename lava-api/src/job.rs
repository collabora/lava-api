use chrono::{DateTime, Utc};
use futures::future::BoxFuture;
use futures::stream::{self, Stream, StreamExt};
use futures::FutureExt;
use serde::Deserialize;
use serde_with::DeserializeFromStr;
use std::fmt;
use std::pin::Pin;
use std::task::{Context, Poll};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

use crate::paginator::{PaginationError, Paginator};
use crate::queryset::{QuerySet, QuerySetMember};
use crate::tag::Tag;
use crate::Lava;

#[derive(
    Copy, Clone, Debug, Hash, PartialEq, Eq, EnumIter, Display, EnumString, DeserializeFromStr,
)]
pub enum State {
    Submitted,
    Scheduling,
    Scheduled,
    Running,
    Canceling,
    Finished,
}

impl QuerySetMember for State {
    type Iter = StateIter;
    fn all() -> Self::Iter {
        Self::iter()
    }
}

#[derive(
    Copy, Clone, Debug, PartialEq, Eq, Hash, EnumIter, EnumString, Display, DeserializeFromStr,
)]
pub enum Health {
    Unknown,
    Complete,
    Incomplete,
    Canceled,
}

impl QuerySetMember for Health {
    type Iter = HealthIter;
    fn all() -> Self::Iter {
        Self::iter()
    }
}

pub enum Ordering {
    Id,
    StartTime,
    EndTime,
    SubmitTime,
}

impl fmt::Display for Ordering {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ordering::Id => write!(f, "id"),
            Ordering::StartTime => write!(f, "start_time"),
            Ordering::EndTime => write!(f, "end_time"),
            Ordering::SubmitTime => write!(f, "submit_time"),
        }
    }
}

#[derive(Clone, Deserialize, Debug)]
struct LavaJob {
    id: i64,
    submitter: String,
    viewing_groups: Vec<i64>,
    description: String,
    health_check: bool,
    requested_device_type: Option<String>,
    tags: Vec<u32>,
    actual_device: Option<String>,
    submit_time: DateTime<Utc>,
    start_time: Option<DateTime<Utc>>,
    end_time: Option<DateTime<Utc>>,
    state: State,
    health: Health,
    priority: i64,
    definition: String,
    original_definition: String,
    multinode_definition: String,
    failure_tags: Vec<u32>,
    failure_comment: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Job {
    pub id: i64,
    pub submitter: String,
    pub viewing_groups: Vec<i64>,
    pub description: String,
    pub health_check: bool,
    pub requested_device_type: Option<String>,
    pub tags: Vec<Tag>,
    pub actual_device: Option<String>,
    pub submit_time: DateTime<Utc>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub state: State,
    pub health: Health,
    pub priority: i64,
    pub definition: String,
    pub original_definition: String,
    pub multinode_definition: String,
    pub failure_tags: Vec<Tag>,
    pub failure_comment: Option<String>,
}

enum PagingState<'a> {
    Paging,
    Transforming(BoxFuture<'a, Job>),
}

pub struct Jobs<'a> {
    lava: &'a Lava,
    paginator: Paginator<LavaJob>,
    state: PagingState<'a>,
}

impl<'a> Jobs<'a> {
    pub fn reported_items(&self) -> Option<u32> {
        self.paginator.reported_items()
    }
}

pub struct JobsBuilder<'a> {
    lava: &'a Lava,
    states: QuerySet<State>,
    healths: QuerySet<Health>,
    limit: Option<u32>,
    ordering: Ordering,
    id_after: Option<i64>,
    started_after: Option<DateTime<Utc>>,
    submitted_after: Option<DateTime<Utc>>,
    ended_after: Option<DateTime<Utc>>,
    ascending: bool,
}

impl<'a> JobsBuilder<'a> {
    pub fn new(lava: &'a Lava) -> Self {
        Self {
            lava,
            states: QuerySet::new(String::from("state")),
            healths: QuerySet::new(String::from("health")),
            limit: None,
            ordering: Ordering::Id,
            id_after: None,
            started_after: None,
            submitted_after: None,
            ended_after: None,
            ascending: true,
        }
    }

    /// Return jobs in this state.
    pub fn state(mut self, state: State) -> Self {
        self.states.include(state);
        self
    }

    /// Exclude jobs in this state.
    pub fn state_not(mut self, state: State) -> Self {
        self.states.exclude(&state);
        self
    }

    /// Set the number of jobs retrieved at a time while the query is
    /// running. The query will be processed transparently as a
    /// sequence of requests that return all matching responses. This
    /// setting governs the size of each of the (otherwise
    /// transparent) requests, so this number is really a page size.
    ///
    /// Note that you will see artifacts on queries that are split
    /// into many requests, especially when responses are slow. This
    /// makes setting the limit much smaller than the response size
    /// unattractive when accurate data is required. However, the
    /// server will need to return records in chunks of this size,
    /// regardless of how many are consumed from the response stream,
    /// which makes setting the limit much higher than the response
    /// size wasteful. In practice, it is probably best to set this
    /// limit to the expected response size for most use cases.
    ///
    /// Artifacts occur when paging occurs, because paging is entirely
    /// client side. Each page contains a section of the query
    /// begining with the job at some multiple of the limit count into
    /// the result set.  However the result set is evolving while the
    /// paging is occurring, and this is not currently compensated
    /// for, which leads to jobs being returned multiple times at the
    /// boundaries between pages - or even omitted depending on the
    /// query. In general, query sets that can shrink are not safe to
    /// use with paging, because results can be lost rather than
    /// duplicated.
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Return jobs with this health.
    pub fn health(mut self, health: Health) -> Self {
        self.healths.include(health);
        self
    }

    /// Exclude jobs with this health.
    pub fn health_not(mut self, health: Health) -> Self {
        self.healths.exclude(&health);
        self
    }

    /// Return only jobs whose id is strictly greater than `id`.
    pub fn id_after(mut self, id: i64) -> Self {
        self.id_after = Some(id);
        self
    }

    /// Return only jobs whose start time is strictly after the given
    /// instant.
    pub fn started_after(mut self, when: chrono::DateTime<Utc>) -> Self {
        self.started_after = Some(when);
        self
    }

    /// Return only jobs whose submission time is strictly after the
    /// given instant.
    pub fn submitted_after(mut self, when: chrono::DateTime<Utc>) -> Self {
        self.submitted_after = Some(when);
        self
    }

    /// Return only jobs which ended strictly after the given instant.
    pub fn ended_after(mut self, when: chrono::DateTime<Utc>) -> Self {
        self.ended_after = Some(when);
        self
    }

    /// Order returned jobs by the given key.
    pub fn ordering(mut self, ordering: Ordering, ascending: bool) -> Self {
        self.ordering = ordering;
        self.ascending = ascending;
        self
    }

    pub fn query(self) -> Jobs<'a> {
        let mut url = self
            .lava
            .base
            .join("jobs/")
            .expect("Failed to append to base url");
        url.query_pairs_mut().append_pair(
            "ordering",
            &format!(
                "{}{}",
                match self.ascending {
                    true => "",
                    false => "-",
                },
                self.ordering
            ),
        );
        if let Some(pair) = self.states.query() {
            url.query_pairs_mut().append_pair(&pair.0, &pair.1);
        }
        if let Some(limit) = self.limit {
            url.query_pairs_mut()
                .append_pair("limit", &limit.to_string());
        };
        if let Some(pair) = self.healths.query() {
            url.query_pairs_mut().append_pair(&pair.0, &pair.1);
        }
        if let Some(id_after) = self.id_after {
            url.query_pairs_mut()
                .append_pair("id__gt", &id_after.to_string());
        };
        if let Some(started_after) = self.started_after {
            url.query_pairs_mut()
                .append_pair("start_time__gt", &started_after.to_rfc3339());
        };
        if let Some(submitted_after) = self.submitted_after {
            url.query_pairs_mut()
                .append_pair("submit_time__gt", &submitted_after.to_rfc3339());
        };
        if let Some(ended_after) = self.ended_after {
            url.query_pairs_mut()
                .append_pair("end_time__gt", &ended_after.to_rfc3339());
        };

        let paginator = Paginator::new(self.lava.client.clone(), url);
        Jobs {
            lava: self.lava,
            paginator,
            state: PagingState::Paging,
        }
    }
}

async fn transform_job(job: LavaJob, lava: &Lava) -> Job {
    let t = stream::iter(job.tags.iter());
    let tags = t
        .filter_map(|i| async move { lava.tag(*i).await })
        .collect()
        .await;

    let t = stream::iter(job.failure_tags.iter());
    let failure_tags = t
        .filter_map(|i| async move { lava.tag(*i).await })
        .collect()
        .await;

    Job {
        id: job.id,
        submitter: job.submitter,
        viewing_groups: job.viewing_groups,
        description: job.description,
        health_check: job.health_check,
        requested_device_type: job.requested_device_type,
        tags,
        actual_device: job.actual_device,
        submit_time: job.submit_time,
        start_time: job.start_time,
        end_time: job.end_time,
        state: job.state,
        health: job.health,
        priority: job.priority,
        definition: job.definition,
        original_definition: job.original_definition,
        multinode_definition: job.multinode_definition,
        failure_tags,
        failure_comment: job.failure_comment,
    }
}

impl<'a> Stream for Jobs<'a> {
    type Item = Result<Job, PaginationError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let me = self.get_mut();

        loop {
            return match &mut me.state {
                PagingState::Paging => {
                    let p = Pin::new(&mut me.paginator);
                    match p.poll_next(cx) {
                        Poll::Ready(None) => Poll::Ready(None),
                        Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
                        Poll::Ready(Some(Ok(d))) => {
                            me.state = PagingState::Transforming(transform_job(d, me.lava).boxed());
                            continue;
                        }
                        Poll::Pending => Poll::Pending,
                    }
                }
                PagingState::Transforming(fut) => match fut.as_mut().poll(cx) {
                    Poll::Ready(d) => {
                        me.state = PagingState::Paging;
                        Poll::Ready(Some(Ok(d)))
                    }
                    Poll::Pending => Poll::Pending,
                },
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Health, Job, Ordering, State, Tag};
    use crate::Lava;

    use boulder::{
        Buildable, Builder, GeneratableWithPersianRug, GeneratorWithPersianRugMutIterator, Repeat,
        Some as GSome, SubsetsFromPersianRug, Time,
    };
    use chrono::{DateTime, Duration, Utc};
    use futures::TryStreamExt;
    use lava_api_mock::{
        Device as MockDevice, DeviceType as MockDeviceType, Group as MockGroup, Job as MockJob,
        JobHealth as MockJobHealth, JobState as MockJobState, LavaMock, PaginationLimits,
        PopulationParams, SharedState, Tag as MockTag, User as MockUser,
    };
    use persian_rug::{Accessor, Context, Proxy};
    use std::collections::BTreeMap;
    use std::convert::{Infallible, TryFrom, TryInto};
    use std::str::FromStr;
    use test_log::test;

    impl Job {
        #[persian_rug::constraints(
            context = C,
            access(
                MockUser<C>,
                MockGroup<C>,
                MockTag<C>,
                MockDevice<C>,
                MockDeviceType<C>
            )
        )]
        pub fn from_mock<'b, B, C>(job: &MockJob<C>, context: B) -> Job
        where
            B: 'b + Accessor<Context = C>,
            C: Context + 'static,
        {
            Self {
                id: job.id,
                submitter: context.get(&job.submitter).username.clone(),
                viewing_groups: job
                    .viewing_groups
                    .iter()
                    .map(|g| context.get(g).id)
                    .collect::<Vec<_>>(),
                description: job.description.clone(),
                health_check: job.health_check,
                requested_device_type: job
                    .requested_device_type
                    .map(|d| context.get(&d).name.to_string()),
                tags: job
                    .tags
                    .iter()
                    .map(|t| Tag::from_mock(context.get(t), context.clone()))
                    .collect::<Vec<_>>(),
                actual_device: job
                    .actual_device
                    .as_ref()
                    .map(|d| context.get(d).hostname.to_string()),
                submit_time: job.submit_time.unwrap(),
                start_time: job.start_time,
                end_time: job.end_time,
                state: job.state.try_into().unwrap(),
                health: job.health.try_into().unwrap(),
                priority: job.priority,
                definition: job.definition.clone(),
                original_definition: job.original_definition.clone(),
                multinode_definition: job.multinode_definition.clone(),
                failure_tags: job
                    .failure_tags
                    .iter()
                    .map(|t| Tag::from_mock(context.get(t), context.clone()))
                    .collect::<Vec<_>>(),
                failure_comment: job.failure_comment.clone(),
            }
        }
    }

    impl TryFrom<MockJobState> for State {
        type Error = Infallible;
        fn try_from(state: MockJobState) -> Result<State, Self::Error> {
            use State::*;

            match state {
                MockJobState::Submitted => Ok(Submitted),
                MockJobState::Scheduling => Ok(Scheduling),
                MockJobState::Scheduled => Ok(Scheduled),
                MockJobState::Running => Ok(Running),
                MockJobState::Canceling => Ok(Canceling),
                MockJobState::Finished => Ok(Finished),
            }
        }
    }

    impl TryFrom<MockJobHealth> for Health {
        type Error = Infallible;
        fn try_from(health: MockJobHealth) -> Result<Health, Self::Error> {
            use Health::*;

            match health {
                MockJobHealth::Unknown => Ok(Unknown),
                MockJobHealth::Complete => Ok(Complete),
                MockJobHealth::Incomplete => Ok(Incomplete),
                MockJobHealth::Canceled => Ok(Canceled),
            }
        }
    }

    #[test]
    fn test_display() {
        assert_eq!(State::Submitted.to_string(), "Submitted");
        assert_eq!(State::Scheduling.to_string(), "Scheduling");
        assert_eq!(State::Scheduled.to_string(), "Scheduled");
        assert_eq!(State::Running.to_string(), "Running");
        assert_eq!(State::Canceling.to_string(), "Canceling");
        assert_eq!(State::Finished.to_string(), "Finished");

        assert_eq!(Health::Unknown.to_string(), "Unknown");
        assert_eq!(Health::Complete.to_string(), "Complete");
        assert_eq!(Health::Incomplete.to_string(), "Incomplete");
        assert_eq!(Health::Canceled.to_string(), "Canceled");
    }

    #[test]
    fn test_from_str() {
        assert_eq!(Ok(State::Submitted), State::from_str("Submitted"));
        assert_eq!(Ok(State::Scheduling), State::from_str("Scheduling"));
        assert_eq!(Ok(State::Scheduled), State::from_str("Scheduled"));
        assert_eq!(Ok(State::Running), State::from_str("Running"));
        assert_eq!(Ok(State::Canceling), State::from_str("Canceling"));
        assert_eq!(Ok(State::Finished), State::from_str("Finished"));
        assert_eq!(
            Err(strum::ParseError::VariantNotFound),
            State::from_str("womble")
        );

        assert_eq!(Ok(Health::Unknown), Health::from_str("Unknown"));
        assert_eq!(Ok(Health::Complete), Health::from_str("Complete"));
        assert_eq!(Ok(Health::Incomplete), Health::from_str("Incomplete"));
        assert_eq!(Ok(Health::Canceled), Health::from_str("Canceled"));
        assert_eq!(
            Err(strum::ParseError::VariantNotFound),
            Health::from_str("")
        );
    }

    /// Stream 50 jobs with a page limit of 7 from the server
    /// checking that we correctly reconstruct their tags and that
    /// they are all accounted for (that pagination is handled
    /// properly)
    #[test(tokio::test)]
    async fn test_basic() {
        let state = SharedState::new_populated(PopulationParams::builder().jobs(50usize).build());
        let server = LavaMock::new(
            state.clone(),
            PaginationLimits::builder().jobs(Some(7)).build(),
        )
        .await;

        let mut map = BTreeMap::new();
        let start = state.access();
        for j in start.get_iter::<lava_api_mock::Job<lava_api_mock::State>>() {
            map.insert(j.id, j);
        }

        let lava = Lava::new(&server.uri(), None).expect("failed to make lava server");

        let mut lj = lava.jobs().query();

        let mut seen = BTreeMap::new();
        while let Some(job) = lj.try_next().await.expect("failed to get job") {
            assert!(!seen.contains_key(&job.id));
            assert!(map.contains_key(&job.id));
            let jj = map.get(&job.id).unwrap();
            assert_eq!(job.submitter, start.get(&jj.submitter).username);
            assert_eq!(job.viewing_groups.len(), jj.viewing_groups.len());
            for i in 0..job.viewing_groups.len() {
                assert_eq!(job.viewing_groups[i], start.get(&jj.viewing_groups[i]).id);
            }
            assert_eq!(job.description, jj.description);
            assert_eq!(job.health_check, jj.health_check);
            assert_eq!(
                job.requested_device_type.as_ref(),
                jj.requested_device_type
                    .as_ref()
                    .map(|t| &start.get(&t).name)
            );

            assert_eq!(job.tags.len(), jj.tags.len());
            for i in 0..job.tags.len() {
                assert_eq!(job.tags[i].id, start.get(&jj.tags[i]).id);
                assert_eq!(job.tags[i].name, start.get(&jj.tags[i]).name);
                assert_eq!(job.tags[i].description, start.get(&jj.tags[i]).description);
            }

            assert_eq!(
                job.actual_device.as_ref(),
                jj.actual_device.as_ref().map(|t| &start.get(&t).hostname)
            );
            assert_eq!(Some(job.submit_time), jj.submit_time);
            assert_eq!(job.start_time, jj.start_time);
            assert_eq!(job.end_time, jj.end_time);
            assert_eq!(job.state.to_string(), jj.state.to_string());
            assert_eq!(job.health.to_string(), jj.health.to_string());
            assert_eq!(job.priority, jj.priority);
            assert_eq!(job.definition, jj.definition);
            assert_eq!(job.original_definition, jj.original_definition);
            assert_eq!(job.multinode_definition, jj.multinode_definition);

            assert_eq!(job.failure_tags.len(), jj.failure_tags.len());
            for i in 0..job.failure_tags.len() {
                assert_eq!(job.viewing_groups[i], start.get(&jj.viewing_groups[i]).id);
            }
            assert_eq!(job.failure_comment, jj.failure_comment);

            seen.insert(job.id, job.clone());
        }
        assert_eq!(seen.len(), 50);
    }

    /// Stream 50 jobs with a page limit of 7 from the server
    /// checking that we correctly reconstruct their tags and that
    /// they are all accounted for (that pagination is handled
    /// properly)
    #[test(tokio::test)]
    async fn test_jobs_builder() {
        let mut server = lava_api_mock::LavaMock::new(
            SharedState::new_populated(
                PopulationParams::builder()
                    .tags(5usize)
                    .jobs(0usize)
                    .build(),
            ),
            PaginationLimits::builder().jobs(Some(7)).build(),
        )
        .await;

        let base_date = DateTime::parse_from_rfc3339("2022-04-10T16:30:00+01:00")
            .unwrap()
            .with_timezone(&Utc);

        let mut gen = Proxy::<lava_api_mock::Job<lava_api_mock::State>>::generator()
            .tags(SubsetsFromPersianRug::new())
            .health(Repeat!(
                MockJobHealth::Complete,
                MockJobHealth::Incomplete,
                MockJobHealth::Canceled,
                MockJobHealth::Unknown
            ))
            .state(Repeat!(
                MockJobState::Submitted,
                MockJobState::Scheduling,
                MockJobState::Scheduled,
                MockJobState::Running,
                MockJobState::Canceling,
                MockJobState::Finished
            ))
            .submit_time(GSome(Time::new(
                base_date - Duration::minutes(1),
                Duration::minutes(-1),
            )))
            .start_time(GSome(Time::new(base_date.clone(), Duration::minutes(-1))))
            .end_time(GSome(Time::new(base_date.clone(), Duration::seconds(-30))));

        let _ = GeneratorWithPersianRugMutIterator::new(&mut gen, server.state_mut())
            .take(50)
            .collect::<Vec<_>>();

        let lava = Lava::new(&server.uri(), None).expect("failed to make lava server");

        let mut lj = lava.jobs().state(State::Running).query();

        let mut count = 0;
        while let Some(job) = lj.try_next().await.expect("failed to get job") {
            assert_eq!(job.state, State::Running);
            count += 1;
        }
        assert_eq!(count, 8);

        let mut lj = lava.jobs().state_not(State::Canceling).query();
        let mut count = 0;
        while let Some(job) = lj.try_next().await.expect("failed to get job") {
            assert_ne!(job.state, State::Canceling);
            count += 1;
        }
        assert_eq!(count, 42);

        let mut lj = lava.jobs().health(Health::Incomplete).query();

        let mut count = 0;
        while let Some(job) = lj.try_next().await.expect("failed to get job") {
            assert_eq!(job.health, Health::Incomplete);
            count += 1;
        }
        assert_eq!(count, 13);

        let mut lj = lava.jobs().health_not(Health::Canceled).query();
        let mut count = 0;
        while let Some(job) = lj.try_next().await.expect("failed to get job") {
            assert_ne!(job.health, Health::Canceled);
            count += 1;
        }
        assert_eq!(count, 38);

        let mut lj = lava.jobs().id_after(9i64).query();
        let mut count = 0;
        while let Some(job) = lj.try_next().await.expect("failed to get job") {
            assert!(job.id > 9i64);
            count += 1;
        }
        assert_eq!(count, 40);

        let job_35_start = DateTime::parse_from_rfc3339("2022-04-10T15:55:00+01:00")
            .unwrap()
            .with_timezone(&Utc);

        let mut lj = lava.jobs().started_after(job_35_start).query();
        let mut count = 0;
        while let Some(job) = lj.try_next().await.expect("failed to get job") {
            assert!(job.start_time.is_some() && job.start_time.unwrap() > job_35_start);
            count += 1;
        }
        assert_eq!(count, 35);

        let job_19_submit = DateTime::parse_from_rfc3339("2022-04-10T16:10:00+01:00")
            .unwrap()
            .with_timezone(&Utc);

        let mut lj = lava.jobs().submitted_after(job_19_submit).query();
        let mut count = 0;
        while let Some(job) = lj.try_next().await.expect("failed to get job") {
            assert!(job.submit_time > job_19_submit);
            count += 1;
        }
        assert_eq!(count, 19);

        let job_25_end = DateTime::parse_from_rfc3339("2022-04-10T16:17:30+01:00")
            .unwrap()
            .with_timezone(&Utc);

        let mut lj = lava.jobs().ended_after(job_25_end).query();
        let mut count = 0;
        while let Some(job) = lj.try_next().await.expect("failed to get job") {
            assert!(job.end_time.is_some() && job.end_time.unwrap() > job_25_end);
            count += 1;
        }
        assert_eq!(count, 25);

        let mut lj = lava.jobs().ordering(Ordering::SubmitTime, false).query();
        let mut count = 0;
        let mut prev = None;
        while let Some(job) = lj.try_next().await.expect("failed to get job") {
            if let Some(dt) = prev {
                assert!(job.submit_time < dt);
            }
            prev = Some(job.submit_time);
            count += 1;
        }
        assert_eq!(count, 50);

        let mut lj = lava.jobs().ordering(Ordering::SubmitTime, true).query();
        let mut count = 0;
        let mut prev = None;
        while let Some(job) = lj.try_next().await.expect("failed to get job") {
            if let Some(dt) = prev {
                assert!(job.submit_time > dt);
            }
            prev = Some(job.submit_time);
            count += 1;
        }
        assert_eq!(count, 50);

        let mut lj = lava.jobs().ordering(Ordering::StartTime, false).query();
        let mut count = 0;
        let mut prev = None;
        while let Some(job) = lj.try_next().await.expect("failed to get job") {
            if let Some(dt) = prev {
                assert!(job.start_time < dt);
            }
            prev = Some(job.start_time);
            count += 1;
        }
        assert_eq!(count, 50);

        let mut lj = lava.jobs().ordering(Ordering::StartTime, true).query();
        let mut count = 0;
        let mut prev = None;
        while let Some(job) = lj.try_next().await.expect("failed to get job") {
            if let Some(dt) = prev {
                assert!(job.start_time > dt);
            }
            prev = Some(job.start_time);
            count += 1;
        }
        assert_eq!(count, 50);
    }
}
