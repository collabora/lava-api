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

#[cfg(test)]
mod tests {
    use super::*;

    use boulder::{
        GeneratableWithPersianRug, GeneratorWithPersianRug, GeneratorWithPersianRugIterator,
    };
    use boulder::{Inc, Pattern, Repeat, Some as GSome, Time};
    use chrono::{DateTime, Duration, Utc};
    use persian_rug::Proxy;
    use rust_decimal_macros::dec;
    use test_log::test;

    use crate::testcases::Decimal;
    use crate::{TestCase, TestSuite};

    #[test(tokio::test)]
    async fn test_read() {
        let mut p = SharedState::new();
        {
            let m = p.mutate();

            let (suite, m) = Proxy::<TestSuite<State>>::generator().generate(m);

            let gen = Proxy::<TestCase<State>>::generator()
                .name(Pattern!("example-case-{}", Inc(0)))
                .unit(Repeat!("", "seconds"))
                .result(|| PassFail::Pass)
                .measurement(Repeat!(None, Some(Decimal(dec!(0.1000000000)))))
            // We hard code this here because serde_yaml isn't configurable enough to match the surface form
            // We check the metadata generator separately
                .metadata(GSome(Repeat!(
                    "case: example-case-0\ndefinition: example-definition-0\nresult: pass\n",
                    "case: example-case-1\ndefinition: example-definition-1\nduration: '0.10'\nextra: example-extra-data\nlevel: 1.1.1\nnamespace: example-namespace\nresult: pass\n"
                )))
                .logged(Time::new(
                    DateTime::parse_from_rfc3339("2022-04-11T16:00:00-00:00")
                        .unwrap()
                        .with_timezone(&Utc),
                    Duration::minutes(30),
                ))
                .suite(move || suite)
                .test_set(|| None)
                .resource_uri(Pattern!("example-resource-uri-{}", Inc(0)));

            let _ = GeneratorWithPersianRugIterator::new(gen, m)
                .take(4)
                .collect::<Vec<_>>();
        }

        let server = wiremock::MockServer::start().await;

        let ep = junit_endpoint(p);

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/v0.2/jobs/0/junit/"))
            .respond_with(ep)
            .mount(&server)
            .await;

        let body = reqwest::get(&format!("{}/api/v0.2/jobs/0/junit/", server.uri()))
            .await
            .expect("error getting junit")
            .bytes()
            .await
            .expect("error parsing utf-8 for junit");

        let suites =
            junit_parser::from_reader(std::io::Cursor::new(body)).expect("failed to parse junit");
        assert_eq!(suites.suites.len(), 1);
        for suite in suites.suites.iter() {
            assert_eq!(suite.cases.len(), 4);
            for (i, case) in suite.cases.iter().enumerate() {
                assert!(case.status.is_success());
                assert_eq!(case.time, if i % 2 == 0 { 0.0f64 } else { 0.1f64 });
                assert_eq!(case.name, format!("example-case-{}", i))
            }
        }
    }
}
