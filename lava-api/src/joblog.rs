use std::collections::HashMap;
use std::fmt;
use std::pin::Pin;
use std::task::Poll;
use std::time::Duration;

use bytes::{Bytes, BytesMut};
use chrono::{DateTime, NaiveDateTime, Utc};
use futures::future::BoxFuture;
use futures::stream::BoxStream;
use futures::{prelude::*, ready};
use reqwest::{Response, StatusCode, Url};
use serde::{Deserialize, Deserializer};
use thiserror::Error;

use crate::Lava;

#[derive(Debug)]
pub struct JobLogBuilder<'a> {
    lava: &'a Lava,
    id: i64,
    start: u64,
    end: u64,
}

impl<'a> JobLogBuilder<'a> {
    pub fn new(lava: &'a Lava, id: i64) -> Self {
        Self {
            lava,
            id,
            start: 0,
            end: 0,
        }
    }

    pub fn start(mut self, start: u64) -> Self {
        self.start = start;
        self
    }

    pub fn end(mut self, end: u64) -> Self {
        self.end = end;
        self
    }

    pub fn raw(self) -> JobLogRaw<'a> {
        JobLogRaw::new(self.lava, self.id, self.start, self.end)
    }

    pub fn log(self) -> JobLog<'a> {
        JobLog::new(self.lava, self.id, self.start, self.end)
    }
}

#[derive(Debug, Error)]
pub enum JobLogError {
    #[error("Request failed: {0}")]
    RequestError(#[from] reqwest::Error),
    #[error("Parse error: {0} - {1}")]
    ParseError(String, serde_norway::Error),
    #[error("No data available")]
    NoData,
}

enum LogRequest {
    Initial,
    Request(BoxFuture<'static, reqwest::Result<Response>>),
    Stream(BoxStream<'static, reqwest::Result<Bytes>>),
    Done,
}

impl fmt::Debug for LogRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let fmt = match self {
            LogRequest::Initial => "Initial",
            LogRequest::Request(_) => "Request",
            LogRequest::Stream(_) => "Stream",
            LogRequest::Done => "Done",
        };
        f.write_str(fmt)
    }
}

#[derive(Debug)]
pub struct JobLogRaw<'a> {
    lava: &'a Lava,
    id: i64,
    start: u64,
    end: u64,
    state: LogRequest,
}

impl<'a> JobLogRaw<'a> {
    fn new(lava: &'a Lava, id: i64, start: u64, end: u64) -> Self {
        Self {
            lava,
            id,
            start,
            end,
            state: LogRequest::Initial,
        }
    }

    fn url(&self) -> Url {
        let mut url = self.lava.base.clone();
        url.path_segments_mut()
            .unwrap()
            .pop_if_empty()
            .push("jobs")
            .push(&self.id.to_string())
            .push("logs")
            .push("");

        if self.start != 0 {
            url.query_pairs_mut()
                .append_pair("start", &self.start.to_string());
        }

        if self.end != 0 {
            url.query_pairs_mut()
                .append_pair("end", &self.end.to_string());
        }
        url
    }
}

impl Stream for JobLogRaw<'_> {
    type Item = Result<Bytes, JobLogError>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let me = self.get_mut();
        loop {
            match me.state {
                LogRequest::Initial => {
                    let u = me.url();
                    let r = me.lava.client.get(u).send();
                    me.state = LogRequest::Request(r.boxed());
                }
                LogRequest::Request(ref mut r) => match ready!(r.as_mut().poll(cx)) {
                    Ok(r) => match r.error_for_status() {
                        Ok(r) => me.state = LogRequest::Stream(r.bytes_stream().boxed()),
                        Err(e) => {
                            me.state = LogRequest::Done;
                            let e = match e.status() {
                                Some(StatusCode::NOT_FOUND) => JobLogError::NoData,
                                _ => e.into(),
                            };
                            return Poll::Ready(Some(Err(e)));
                        }
                    },
                    Err(e) => return Poll::Ready(Some(Err(e.into()))),
                },
                LogRequest::Stream(ref mut stream) => match ready!(stream.as_mut().poll_next(cx)) {
                    Some(Err(e)) => return Poll::Ready(Some(Err(e.into()))),
                    Some(Ok(b)) => {
                        return Poll::Ready(Some(Ok(b)));
                    }
                    None => {
                        me.state = LogRequest::Done;
                        return Poll::Ready(None);
                    }
                },
                LogRequest::Done => return Poll::Ready(None),
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

fn deserialize_duration<'de, D>(d: D) -> Result<Option<Duration>, D::Error>
where
    D: Deserializer<'de>,
{
    let duration = String::deserialize(d)?
        .parse()
        .map_err(serde::de::Error::custom)?;
    Ok(Some(Duration::from_secs_f64(duration)))
}

#[derive(Debug, Clone, Deserialize)]
pub struct JobResult {
    pub case: String,
    pub definition: String,
    pub namespace: Option<String>,
    pub level: Option<String>,
    pub result: String,
    #[serde(default, deserialize_with = "deserialize_duration")]
    pub duration: Option<Duration>,
    #[serde(default)]
    pub extra: HashMap<String, serde_norway::Value>,
}

#[derive(Debug, Clone)]
pub enum JobLogMsg {
    Msg(String),
    Msgs(Vec<String>),
    Result(JobResult),
}

impl<'de> Deserialize<'de> for JobLogMsg {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Deserialize as serde_norway::Value first to avoid serde's ContentVisitor,
        // which does not support YAML tagged values (e.g. `! "188250"`) that LAVA
        // sometimes emits in the `extra` field.
        let value = serde_norway::Value::deserialize(deserializer)?;
        match value {
            serde_norway::Value::String(s) => Ok(JobLogMsg::Msg(s)),
            serde_norway::Value::Sequence(seq) => {
                let msgs = seq
                    .into_iter()
                    .map(|v| match v {
                        serde_norway::Value::String(s) => Ok(s),
                        _ => Err(serde::de::Error::custom("expected string in msg sequence")),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(JobLogMsg::Msgs(msgs))
            }
            serde_norway::Value::Mapping(_) => {
                let result = serde_norway::from_value(value).map_err(serde::de::Error::custom)?;
                Ok(JobLogMsg::Result(result))
            }
            _ => Err(serde::de::Error::custom(
                "expected string, sequence, or mapping for msg",
            )),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobLogLevel {
    Debug,
    Info,
    Warning,
    Error,
    Results,
    Target,
    Input,
    Feedback,
    Exception,
}

fn job_deserialize_dt<'de, D>(d: D) -> Result<NaiveDateTime, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum DorN {
        D(DateTime<Utc>),
        N(NaiveDateTime),
    }
    match DorN::deserialize(d)? {
        DorN::D(d) => Ok(d.naive_utc()),
        DorN::N(n) => Ok(n),
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct JobLogEntry {
    #[serde(deserialize_with = "job_deserialize_dt")]
    pub dt: NaiveDateTime,
    pub lvl: JobLogLevel,
    pub ns: Option<String>,
    pub msg: JobLogMsg,
}

#[derive(Debug)]
pub struct JobLog<'a> {
    buf: Vec<Bytes>,
    from_buf: bool,
    raw: JobLogRaw<'a>,
}

impl<'a> JobLog<'a> {
    fn new(lava: &'a Lava, id: i64, start: u64, end: u64) -> Self {
        let raw = JobLogRaw::new(lava, id, start, end);
        Self {
            buf: Vec::new(),
            from_buf: false,
            raw,
        }
    }
}

impl Stream for JobLog<'_> {
    type Item = Result<JobLogEntry, JobLogError>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let me = self.get_mut();
        loop {
            if me.from_buf {
                let last = me.buf.last().unwrap();
                if let Some(eol) = last.iter().position(|e| e == &b'\n') {
                    let line = if me.buf.len() == 1 {
                        if last.len() - 1 == eol {
                            me.from_buf = false;
                            me.buf.pop().unwrap()
                        } else {
                            let b = me.buf.get_mut(0).unwrap();
                            b.split_to(eol + 1)
                        }
                    } else {
                        let mut buf = BytesMut::new();
                        for b in me.buf.drain(0..me.buf.len() - 1) {
                            buf.extend_from_slice(b.as_ref());
                        }

                        let last = me.buf.last().unwrap();
                        if last.len() == eol {
                            me.from_buf = false;
                            buf.extend_from_slice(me.buf.pop().unwrap().as_ref());
                        } else {
                            let b = me.buf.get_mut(0).unwrap();
                            buf.extend_from_slice(b.split_to(eol + 1).as_ref());
                        }
                        buf.into()
                    };
                    let l = line.slice(1..);
                    let entry = serde_norway::from_slice(l.as_ref()).map_err(|e| {
                        let s = String::from_utf8_lossy(l.as_ref());
                        JobLogError::ParseError(s.into_owned(), e)
                    });
                    return Poll::Ready(Some(entry));
                } else {
                    me.from_buf = false;
                }
            } else {
                match ready!(Pin::new(&mut me.raw).poll_next(cx)) {
                    Some(Err(e)) => return Poll::Ready(Some(Err(e))),
                    Some(Ok(b)) => {
                        me.from_buf = true;
                        me.buf.push(b);
                    }
                    None => return Poll::Ready(None),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_log_entry_deserialisation() {
        let entry0: JobLogEntry = serde_json::from_str(
            r#"{"dt": "2026-06-02T12:16:27.730458", "lvl": "results", "msg": {"definition": "lava", "case": "job", "result": "pass"}}"#,
        )
        .unwrap();
        let entry1: JobLogEntry = serde_json::from_str(
            r#"{"dt": "2026-06-02T12:16:27.730458+00:00", "lvl": "results", "msg": {"definition": "lava", "case": "job", "result": "pass"}}"#,
        )
        .unwrap();
        assert_eq!(entry0.dt, entry1.dt);
    }

    #[test]
    fn test_joblog_entry_with_tagged_extra() {
        // Regression test for https://github.com/collabora/lava-api/issues/31
        // LAVA sometimes emits tagged YAML values (e.g. `! "188250"`) in the
        // `extra` field of a results entry. The bare `!` is a YAML non-specific
        // tag and should not prevent deserialization.
        let yaml = r#"{"dt": "2026-04-30T16:19:53.641343", "lvl": "results", "msg": {"definition": "lava", "namespace": "common", "case": "http-download", "level": "1.4.1", "duration": "0.00", "result": "pass", "extra": {"label": "dtb", "size": ! "188250", "sha256sum": "9f4c38d2218c38be09a03812ab9176d521b8fb624b8e9ecff69b4125d260122b"}}}"#;
        let entry: JobLogEntry =
            serde_norway::from_str(yaml).expect("failed to parse entry with tagged extra value");
        assert!(matches!(entry.lvl, JobLogLevel::Results));
        let JobLogMsg::Result(result) = entry.msg else {
            panic!("expected Result variant, got {:?}", entry.msg);
        };
        assert_eq!(result.case, "http-download");
        assert_eq!(result.definition, "lava");
        assert_eq!(result.result, "pass");
        assert!(result.extra.contains_key("size"));
        assert!(result.extra.contains_key("sha256sum"));
    }

    #[test]
    fn test_joblog_msg_string() {
        let yaml = r#"{"dt": "2022-01-01T00:00:00", "lvl": "info", "msg": "hello world"}"#;
        let entry: JobLogEntry = serde_norway::from_str(yaml).expect("failed to parse string msg");
        assert!(matches!(entry.msg, JobLogMsg::Msg(ref s) if s == "hello world"));
    }

    #[test]
    fn test_joblog_msg_sequence() {
        let yaml = r#"{"dt": "2022-01-01T00:00:00", "lvl": "target", "msg": ["line1", "line2"]}"#;
        let entry: JobLogEntry =
            serde_norway::from_str(yaml).expect("failed to parse sequence msg");
        assert!(matches!(entry.msg, JobLogMsg::Msgs(ref v) if v == &["line1", "line2"]));
    }
}
