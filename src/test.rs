use chrono::{DateTime, Utc};
use serde::de::Visitor;
use serde::{Deserialize, Deserializer};
use serde_with::DeserializeFromStr;
use std::fmt;
use strum::{Display, EnumString};

// From lava/lava_results_app/models.py in TestCase::RESULT_CHOICES
#[derive(Copy, DeserializeFromStr, Clone, Debug, Display, EnumString, PartialEq)]
#[strum(serialize_all = "snake_case")]
pub enum PassFail {
    Pass,
    Fail,
    Skip,
    Unknown,
}

// From lava/lava_common/exceptions.py as the error_type fields of the classes
#[derive(Copy, DeserializeFromStr, Clone, Debug, Display, EnumString, PartialEq)]
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

// From lava/lava_results_app/models.py in TestCase
#[derive(Clone, Debug, Deserialize)]
pub struct TestCase {
    pub id: u64,
    pub name: String,
    // Renamed in the v02 api from "units" (in the model) to "unit"
    pub unit: String,
    pub result: PassFail,
    pub measurement: Option<String>,
    #[serde(deserialize_with = "nested_yaml")]
    pub metadata: Option<Metadata>,
    pub suite: u64,
    pub start_log_line: Option<u32>,
    pub end_log_line: Option<u32>,
    pub test_set: Option<u64>,
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
    use super::*;

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
        assert_eq!(tc.id, 207021205u64);
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
        assert_eq!(tc.suite, 10892144u64);
        assert_eq!(tc.test_set, None);
    }
}
