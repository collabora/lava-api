use chrono::{DateTime, Utc};
use futures::future::BoxFuture;
use futures::stream::{self, Stream, StreamExt};
use futures::FutureExt;
use serde::Deserialize;
use std::convert::TryFrom;
use std::fmt;
use std::pin::Pin;
use std::task::{Context, Poll};
use thiserror::Error;

use crate::paginator::{PaginationError, Paginator};
use crate::tag::Tag;
use crate::Lava;

#[derive(Copy, Deserialize, Clone, Debug, PartialEq)]
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

#[derive(Copy, Deserialize, Clone, Debug, PartialEq)]
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

#[derive(Clone, Deserialize, Debug)]
struct LavaJob {
    id: i64,
    submitter: String,
    viewing_groups: Vec<String>,
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
    pub viewing_groups: Vec<String>,
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
    state: Option<State>,
    health: Option<Health>,
    limit: Option<u32>,
}

impl<'a> JobsBuilder<'a> {
    pub fn new(lava: &'a Lava) -> Self {
        Self {
            lava,
            state: None,
            health: None,
            limit: None,
        }
    }

    pub fn state(mut self, state: State) -> Self {
        self.state = Some(state);
        self
    }

    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn health(mut self, health: Health) -> Self {
        self.health = Some(health);
        self
    }

    pub fn query(self) -> Jobs<'a> {
        let mut query = String::from("jobs/?ordering=id");
        if let Some(state) = self.state {
            query.push_str(format!(";state={}", state).as_str())
        };
        if let Some(limit) = self.limit {
            query.push_str(format!(";limit={}", limit).as_str())
        };
        if let Some(health) = self.health {
            query.push_str(format!(";health={}", health).as_str())
        };
        let paginator = Paginator::new(self.lava.client.clone(), &self.lava.base, &query);
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
