//! Retrieve test data

use chrono::{DateTime, Utc};
use serde::de::Visitor;
use serde::{Deserialize, Deserializer};
use serde_with::DeserializeFromStr;
use std::fmt;
use strum::{Display, EnumString};

/// The result of running a [`TestCase`], as stored by LAVA
// From lava/lava_results_app/models.py in TestCase::RESULT_CHOICES
#[derive(Copy, DeserializeFromStr, Clone, Debug, Display, EnumString, PartialEq, Eq)]
#[strum(serialize_all = "snake_case")]
pub enum PassFail {
    Pass,
    Fail,
    Skip,
    Unknown,
}

/// The type of an error that occurred running a test
// From lava/lava_common/exceptions.py as the error_type fields of the classes
#[derive(Copy, DeserializeFromStr, Clone, Debug, Display, EnumString, PartialEq, Eq)]
pub enum ErrorType {
    None,
    Infrastructure,
    Configuration,
    Bug,
    Canceled,
    Job,
    Test,
    #[strum(serialize = "LAVATimeout")]
    LavaTimeout,
    MultinodeTimeout,
    ObjectNotPersisted,
    #[strum(serialize = "Unexisting permission codename.")]
    UnexistingPermissionCodename,
}

/// The metadata available for a [`TestCase`] from the LAVA API
// This structure is an amalgam of things handed to
// - lava/lava_common/log.py YAMLLogger::results
// particularly by
// - lava/lava_dispatcher/action.py Action::log_action_results
// - lava/lava_dispatcher/job.py Job::validate
// - lava/lava/dispatcher/lava-run main
// In particular the failure case is defined in lava-run.
// These results are then propagated back to
// - lava/lava_scheduler_app/views.py internal_v1_jobs_logs
// And then from there to
// - lava/lava_results_app/dbutils.py map_scanned_results
#[derive(Clone, Debug, Deserialize)]
pub struct Metadata {
    // These three fields are present or the results would have been
    // rejected earlier by map_scanned_results.
    pub definition: String,
    pub case: String,
    pub result: PassFail,

    // Success case
    pub namespace: Option<String>,
    pub level: Option<String>,
    // This is just a float formatted with "%0.2f"
    pub duration: Option<String>,
    pub extra: Option<String>,

    // Failure case
    // These are not present in the success case, and are added by
    // lava-run based on the contents of the fault thrown.
    pub error_msg: Option<String>,
    pub error_type: Option<ErrorType>,
}

/// The data available for a test case for a [`Job`](crate::job::Job)
/// from the LAVA API
// From lava/lava_results_app/models.py in TestCase
#[derive(Clone, Debug, Deserialize)]
pub struct TestCase {
    pub id: i64,
    pub name: String,
    // Renamed in the v02 api from "units" (in the model) to "unit"
    pub unit: String,
    pub result: PassFail,
    pub measurement: Option<String>,
    #[serde(deserialize_with = "nested_yaml")]
    pub metadata: Option<Metadata>,
    pub suite: i64,
    pub start_log_line: Option<u32>,
    pub end_log_line: Option<u32>,
    pub test_set: Option<i64>,
    pub logged: DateTime<Utc>,
    // from v02 api
    pub resource_uri: String,
}

fn nested_yaml<'de, D, T>(deser: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: for<'de3> Deserialize<'de3>,
{
    struct StrVisitor<U> {
        _marker: core::marker::PhantomData<U>,
    }

    impl<U> Default for StrVisitor<U> {
        fn default() -> Self {
            Self {
                _marker: Default::default(),
            }
        }
    }

    impl<'de, U> Visitor<'de> for StrVisitor<U>
    where
        U: for<'de2> Deserialize<'de2>,
    {
        type Value = U;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("nested YAML")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            serde_yaml::from_str(value).map_err(|e| serde::de::Error::custom(e))
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            serde_yaml::from_str(&value).map_err(|e| serde::de::Error::custom(e))
        }
    }

    deser.deserialize_str(StrVisitor::default())
}

#[cfg(test)]
mod tests {
    use super::{ErrorType, Metadata, PassFail, TestCase};

    use crate::Lava;
    use boulder::{Buildable, Builder};
    use futures::TryStreamExt;
    use lava_api_mock::{Job, LavaMock, PaginationLimits, PopulationParams, SharedState, State};
    use persian_rug::Accessor;
    use std::collections::BTreeMap;
    use test_log::test;

    #[test]
    fn test_meta() {
        let yaml = r#"
case: http-download
definition: lava
duration: '0.35'
extra: /var/lib/lava-server/default/media/job-output/2022/02/28/5790643/metadata/lava-http-download-1.2.1.yaml
level: 1.2.1
namespace: common
result: pass"#;
        let meta: Metadata = serde_yaml::from_str(yaml).expect("failed to deserialize metadata");
        assert_eq!(meta.case, "http-download");
        assert_eq!(meta.definition, "lava");
        assert_eq!(meta.duration, Some("0.35".to_string()));
        assert_eq!(meta.extra, Some("/var/lib/lava-server/default/media/job-output/2022/02/28/5790643/metadata/lava-http-download-1.2.1.yaml".to_string()));
        assert_eq!(meta.level, Some("1.2.1".to_string()));
        assert_eq!(meta.namespace, Some("common".to_string()));
        assert_eq!(meta.result, PassFail::Pass);
        assert_eq!(meta.error_msg, None);
        assert_eq!(meta.error_type, None);

        let yaml = r#"
case: job
definition: lava
error_msg: bootloader-interrupt timed out after 30 seconds
error_type: Infrastructure
result: fail
"#;
        let meta: Metadata = serde_yaml::from_str(yaml).expect("failed to deserialize metadata");
        assert_eq!(meta.case, "job");
        assert_eq!(meta.definition, "lava");
        assert_eq!(meta.duration, None);
        assert_eq!(meta.extra, None);
        assert_eq!(meta.level, None);
        assert_eq!(meta.namespace, None);
        assert_eq!(meta.result, PassFail::Fail);
        assert_eq!(
            meta.error_msg,
            Some("bootloader-interrupt timed out after 30 seconds".to_string())
        );
        assert_eq!(meta.error_type, Some(ErrorType::Infrastructure));
    }

    #[test]
    fn test_test_case() {
        let json = r#"
{
  "id": 207021205,
  "result": "pass",
  "resource_uri": "http://lava.collabora.co.uk/api/v0.2/jobs/5790643/suites/10892144/tests/207021205/",
  "unit": "seconds",
  "name": "http-download",
  "measurement": "0.2600000000",
  "metadata": "case: http-download\ndefinition: lava\nduration: '0.26'\nextra: /var/lib/lava-server/default/media/job-output/2022/02/28/5790643/metadata/lava-http-download-1.1.1.yaml\nlevel: 1.1.1\nnamespace: common\nresult: pass\n",
  "start_log_line": null,
  "end_log_line": null,
  "logged": "2022-02-28T19:29:01.998922Z",
  "suite": 10892144,
  "test_set": null
}"#;
        let tc: TestCase = serde_json::from_str(json).expect("failed to deserialize testcase");
        assert_eq!(tc.id, 207021205i64);
        assert_eq!(tc.result, PassFail::Pass);
        assert_eq!(
            tc.resource_uri,
            "http://lava.collabora.co.uk/api/v0.2/jobs/5790643/suites/10892144/tests/207021205/"
        );
        assert_eq!(tc.unit, "seconds");
        assert_eq!(tc.name, "http-download");
        assert_eq!(tc.measurement, Some("0.2600000000".to_string()));
        assert!(tc.metadata.is_some());
        if let Some(ref meta) = tc.metadata {
            assert_eq!(meta.case, "http-download");
            assert_eq!(meta.definition, "lava");
            assert_eq!(meta.duration, Some("0.26".to_string()));
            assert_eq!(meta.extra, Some("/var/lib/lava-server/default/media/job-output/2022/02/28/5790643/metadata/lava-http-download-1.1.1.yaml".to_string()));
            assert_eq!(meta.level, Some("1.1.1".to_string()));
            assert_eq!(meta.namespace, Some("common".to_string()));
            assert_eq!(meta.result, PassFail::Pass);
        }
        assert_eq!(tc.start_log_line, None);
        assert_eq!(tc.end_log_line, None);
        assert_eq!(
            tc.logged,
            chrono::DateTime::parse_from_rfc3339("2022-02-28T19:29:01.998922Z")
                .expect("parsing date")
        );
        assert_eq!(tc.suite, 10892144i64);
        assert_eq!(tc.test_set, None);
    }

    /// Stream 20 tests each from 3 jobs with a page limit of 6 from
    /// the server checking that they are all accounted for (that
    /// pagination is handled properly)
    #[test(tokio::test)]
    async fn test_basic() {
        let pop = PopulationParams::builder()
            .jobs(3usize)
            .test_suites(6usize)
            .test_cases(20usize)
            .build();
        let state = SharedState::new_populated(pop);
        let server = LavaMock::new(
            state.clone(),
            PaginationLimits::builder().test_cases(Some(6)).build(),
        )
        .await;

        let mut map = BTreeMap::new();
        let start = state.access();
        for t in start.get_iter::<lava_api_mock::TestCase<State>>() {
            map.insert(t.id, t.clone());
        }

        let lava = Lava::new(&server.uri(), None).expect("failed to make lava server");

        let mut seen = BTreeMap::new();

        for job in start.get_iter::<Job<State>>() {
            let mut lt = lava.test_cases(job.id);

            while let Some(test) = lt.try_next().await.expect("failed to get test") {
                assert!(!seen.contains_key(&test.id));
                assert!(map.contains_key(&test.id));
                let tt = map.get(&test.id).unwrap();
                assert_eq!(test.id, tt.id);
                assert_eq!(test.name, tt.name);
                assert_eq!(test.unit, tt.unit);
                assert_eq!(test.result.to_string(), tt.result.to_string());
                assert_eq!(
                    test.measurement,
                    tt.measurement.as_ref().map(|m| m.to_string())
                );
                assert_eq!(test.suite, start.get(&tt.suite).id);
                assert_eq!(job.id, start.get(&start.get(&tt.suite).job).id);
                assert_eq!(test.start_log_line, tt.start_log_line);
                assert_eq!(test.end_log_line, tt.end_log_line);
                assert_eq!(
                    test.test_set,
                    tt.test_set.as_ref().map(|t| start.get(&t).id)
                );
                assert_eq!(test.logged, tt.logged);
                assert_eq!(test.resource_uri, tt.resource_uri);

                seen.insert(test.id, test.clone());
            }
        }
        assert_eq!(seen.len(), 60);
    }
}
