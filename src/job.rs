use chrono::{DateTime, Utc};
use futures::future::BoxFuture;
use futures::stream::{self, Stream, StreamExt};
use futures::FutureExt;
use serde::Deserialize;
use std::convert::TryFrom;
use std::fmt;
use std::pin::Pin;
use std::task::{Context, Poll};
use strum::{EnumIter, IntoEnumIterator};
use thiserror::Error;

use crate::paginator::{PaginationError, Paginator};
use crate::queryset::{QuerySet, QuerySetMember};
use crate::tag::Tag;
use crate::Lava;

#[derive(Copy, Deserialize, Clone, Debug, Hash, PartialEq, Eq, EnumIter)]
#[serde(try_from = "&str")]
pub enum State {
    Submitted,
    Scheduling,
    Scheduled,
    Running,
    Canceling,
    Finished,
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            State::Submitted => write!(f, "Submitted"),
            State::Scheduling => write!(f, "Scheduling"),
            State::Scheduled => write!(f, "Scheduled"),
            State::Running => write!(f, "Running"),
            State::Canceling => write!(f, "Canceling"),
            State::Finished => write!(f, "Finished"),
        }
    }
}

#[derive(Clone, Debug, Error)]
#[error("Failed to convert into State")]
pub struct TryFromStateError {}

impl TryFrom<&str> for State {
    type Error = TryFromStateError;
    fn try_from(v: &str) -> Result<Self, Self::Error> {
        match v {
            "Submitted" => Ok(State::Submitted),
            "Scheduling" => Ok(State::Scheduling),
            "Scheduled" => Ok(State::Scheduled),
            "Running" => Ok(State::Running),
            "Canceling" => Ok(State::Canceling),
            "Finished" => Ok(State::Finished),
            _ => Err(TryFromStateError {}),
        }
    }
}

impl QuerySetMember for State {
    type Iter = StateIter;
    fn all() -> Self::Iter {
        Self::iter()
    }
}

#[derive(Copy, Deserialize, Clone, Debug, PartialEq, Eq, Hash, EnumIter)]
#[serde(try_from = "&str")]
pub enum Health {
    Unknown,
    Complete,
    Incomplete,
    Canceled,
}

impl fmt::Display for Health {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Health::Unknown => write!(f, "Unknown"),
            Health::Complete => write!(f, "Complete"),
            Health::Incomplete => write!(f, "Incomplete"),
            Health::Canceled => write!(f, "Canceled"),
        }
    }
}

#[derive(Clone, Debug, Error)]
#[error("Failed to convert into Health")]
pub struct TryFromHealthError {}

impl TryFrom<&str> for Health {
    type Error = TryFromHealthError;
    fn try_from(v: &str) -> Result<Self, Self::Error> {
        match v {
            "Unknown" => Ok(Health::Unknown),
            "Complete" => Ok(Health::Complete),
            "Incomplete" => Ok(Health::Incomplete),
            "Canceled" => Ok(Health::Canceled),
            _ => Err(TryFromHealthError {}),
        }
    }
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
    viewing_groups: Vec<u64>,
    description: String,
    health_check: bool,
    requested_device_type: String,
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

#[derive(Clone, Debug)]
pub struct Job {
    pub id: i64,
    pub submitter: String,
    pub viewing_groups: Vec<u64>,
    pub description: String,
    pub health_check: bool,
    pub requested_device_type: String,
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
        let mut url = self.lava.base.join("jobs/").expect("Failed to append to base url");
        url.query_pairs_mut()
            .append_pair("ordering", &format!("{}{}", match self.ascending { true => "", false => "-"}, self.ordering));
        if let Some(pair) = self.states.query() {
            url.query_pairs_mut().append_pair(&pair.0, &pair.1);
        }
        if let Some(limit) = self.limit {
            url.query_pairs_mut().append_pair("limit", &limit.to_string());
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
