use std::collections::BTreeMap;

use junit_report::{Duration, Report, ReportBuilder, TestCaseBuilder, TestSuiteBuilder};
use persian_rug::Accessor;
use regex::Regex;
use rust_decimal::prelude::ToPrimitive;
use wiremock::{Request, Respond, ResponseTemplate};

use crate::{PassFail, SharedState, State};

fn get_duration(tc: &crate::TestCase<State>) -> Option<Duration> {
    tc.measurement.as_ref().map(|m| {
        Duration::seconds_f64(
            //<crate::testcases::Decimal as Into<rust_decimal::Decimal>>::into(m)
            m.to_f64().unwrap()
                * match tc.unit.as_ref() {
                    "seconds" => 1f64,
                    "hours" => 3600f64,
                    _ => unimplemented!("testcase unit not handled"),
                },
        )
    })
}

fn create_junit(job_id: i64, data: &SharedState) -> Report {
    let data = data.access();
    let mut m = BTreeMap::new();

    for testcase in data.get_iter::<crate::TestCase<State>>() {
        let suite = data.get(&testcase.suite);
        let job = data.get(&suite.job);
        if job.id == job_id {
            let (ty, msg) = match testcase.metadata.as_ref() {
                Some(meta) => {
                    let m: crate::Metadata = serde_yaml::from_str(meta).unwrap();
                    (
                        m.error_type.unwrap_or_default(),
                        m.error_msg.unwrap_or_default(),
                    )
                }
                None => Default::default(),
            };

            let tc = match testcase.result {
                PassFail::Pass => TestCaseBuilder::success(
                    &testcase.name,
                    get_duration(testcase).unwrap_or(Duration::seconds(0)),
                ),
                PassFail::Fail => TestCaseBuilder::failure(
                    &testcase.name,
                    get_duration(testcase).unwrap_or(Duration::seconds(0)),
                    &ty,
                    &msg,
                ),
                PassFail::Skip => TestCaseBuilder::skipped(&testcase.name),
                PassFail::Unknown => TestCaseBuilder::error(
                    &testcase.name,
                    get_duration(testcase).unwrap_or(Duration::seconds(0)),
                    &ty,
                    &msg,
                ),
            };
            m.entry(testcase.suite)
                .or_insert_with(|| TestSuiteBuilder::new(&suite.name))
                .add_testcase(tc.build());
        }
    }

    let mut rb = ReportBuilder::new();
    for (_, v) in m.into_iter() {
        rb.add_testsuite(v.build());
    }
    rb.build()
}

pub struct JunitEndpoint {
    data: SharedState,
}

impl Respond for JunitEndpoint {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let rr = Regex::new(r"/api/v0.2/jobs/(?P<parent>[0-9]+)/junit/").unwrap();
        if let Some(captures) = rr.captures(request.url.as_str()) {
            println!("Got capture {:?}", captures.get(1).unwrap().as_str());
            let job_id = captures.get(1).unwrap().as_str().parse::<i64>().unwrap();
            let r = create_junit(job_id, &self.data);
            let mut v = Vec::new();
            r.write_xml(&mut v).expect("failed to write junit xml");
            ResponseTemplate::new(200).set_body_bytes(v)
        } else {
            ResponseTemplate::new(404)
        }
    }
}

pub fn junit_endpoint(data: SharedState) -> JunitEndpoint {
    JunitEndpoint { data }
}
