use boulder::{
    Buildable, BuildableWithPersianRug, Builder, Generatable, GeneratableWithPersianRug, Generator,
};
use boulder::{Cycle, Inc, Pattern, Repeat, Some as GSome, Time};
use chrono::{DateTime, Duration, Utc};
use core::fmt::{Display, Formatter};
use core::ops::{Deref, DerefMut};
use core::str::FromStr;
use django_query::{
    filtering::FilterableWithPersianRug, row::IntoRowWithPersianRug,
    sorting::SortableWithPersianRug,
};
use rust_decimal_macros::dec;
use serde::Serialize;
use serde_with::SerializeDisplay;
use strum::{Display, EnumString};

use crate::devices::Device;
use crate::devicetypes::{Alias, Architecture, BitWidth, Core, DeviceType, ProcessorFamily};
use crate::jobs::Job;
use crate::tags::Tag;
use crate::users::{Group, User};
use crate::workers::Worker;

use persian_rug::{contextual, Context, Proxy};

/// A representation of the metadata for a test case.
#[derive(Clone, Debug, Serialize, Buildable, Generatable)]
pub struct Metadata {
    #[boulder(default = "lava")]
    pub definition: String,
    #[boulder(default = "example-stage")]
    pub case: String,
    #[boulder(default=PassFail::Pass, generator=Repeat!(PassFail::Pass, PassFail::Fail))]
    pub result: PassFail,
    #[boulder(default=Some("common".to_string()), generator=Repeat!(Some("common".to_string()), None))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[boulder(default=Some("1.1".to_string()), generator=Repeat!(Some("1.1".to_string()), None))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
    #[boulder(default=Decimal(dec!(1.234)), generator=Repeat!(Some(dec!(1.234).into()), None))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<Decimal>,
    #[boulder(default=Some("example-definition.yaml".to_string()),
              generator=Repeat!(Some("example-definition.yaml".to_string()), None))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<String>,

    #[boulder(generator=Repeat!(None, Some("example error message".to_string())))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_msg: Option<String>,
    #[boulder(generator=Repeat!(None, Some("Infrastructure".to_string())))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_type: Option<String>,
}

/// A suite of tests from the LAVA API.
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
            Worker<C>,
            TestSuite<C>
        )
    )
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
            Worker<C>,
            TestSuite<C>
        )
    )
)]
#[contextual(C)]
pub struct TestSuite<C: Context + 'static> {
    #[boulder(generator=Inc(0))]
    #[django(sort, op(lt, gt))]
    pub id: i64,
    #[boulder(buildable_with_persian_rug, generatable_with_persian_rug)]
    #[django(traverse, foreign_key = "id")]
    pub job: Proxy<Job<C>>,
    #[boulder(default="Example suite name", generator=Pattern!("Example suite {}", Inc(1i32)))]
    #[django(sort, op(in, contains, icontains, startswith, endswith))]
    pub name: String,
    // from v02 api
    pub resource_uri: Option<String>,
}

/// A set of tests from the LAVA API.
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
            Worker<C>,
            TestSet<C>,
            TestSuite<C>
        )
    )
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
            Worker<C>,
            TestSet<C>,
            TestSuite<C>
        )
    )
)]
#[contextual(C)]
pub struct TestSet<C: Context + 'static> {
    #[boulder(generator=Inc(0))]
    #[django(sort, op(lt, gt))]
    pub id: i64,
    #[boulder(default=Some("Example test set".to_string()), generator=GSome(Pattern!("Example set {}", Inc(1i32))))]
    #[django(sort, op(in, contains, icontains, startswith, endswith))]
    pub name: Option<String>,
    #[boulder(buildable_with_persian_rug, generatable_with_persian_rug)]
    #[django(traverse, foreign_key = "id")]
    pub suite: Proxy<TestSuite<C>>,
}

/// A test from the LAVA API.
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
            Worker<C>,
            TestCase<C>,
            TestSet<C>,
            TestSuite<C>
        )
    )
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
            Worker<C>,
            TestCase<C>,
            TestSet<C>,
            TestSuite<C>
        )
    )
)]
#[contextual(C)]
pub struct TestCase<C: Context + 'static> {
    #[boulder(generator=Inc(0))]
    #[django(sort, op(lt, gt, in))]
    pub id: i64,
    #[boulder(default="An example test case", generator=Pattern!("Test case {}", Inc(0usize)))]
    #[django(sort, op(in, contains, icontains, startswith, endswith))]
    pub name: String,
    // Renamed in the v02 api from "units" (in the model) to "unit"
    #[boulder(default="seconds", generator=Cycle::new(vec!["seconds".to_string(), "hours".to_string()].into_iter()))]
    #[django(sort, op(in, contains, icontains, startswith, endswith))]
    pub unit: String,
    #[boulder(default=PassFail::Pass)]
    #[django(sort)]
    pub result: PassFail,
    // FIXME: better default
    #[django(sort, op(lt, lte, gt, gte))]
    pub measurement: Option<Decimal>,
    // Has to be a string because of the filtering and sorting
    #[boulder(default=Some(serde_yaml::to_string(&Metadata::builder().build()).unwrap()),
              generator=GSome(MetadataGenerator::new()))]
    #[django(sort, op(in, contains, icontains, startswith, endswith))]
    pub metadata: Option<String>,
    #[boulder(buildable_with_persian_rug, generatable_with_persian_rug)]
    #[django(traverse, foreign_key = "id")]
    pub suite: Proxy<TestSuite<C>>,
    #[django(sort, op(lt, lte, gt, gte))]
    pub start_log_line: Option<u32>,
    #[django(sort, op(lt, lte, gt, gte))]
    pub end_log_line: Option<u32>,
    #[boulder(buildable_with_persian_rug, generatable_with_persian_rug)]
    #[django(traverse, foreign_key = "id")]
    pub test_set: Option<Proxy<TestSet<C>>>,
    #[boulder(default=DateTime::parse_from_rfc3339("2022-03-26T21:00:00-00:00").unwrap().with_timezone(&Utc),
              generator=Time::new(DateTime::parse_from_rfc3339("2022-03-26T21:00:00-00:00").unwrap().with_timezone(&Utc),
                                  Duration::minutes(1)))]
    #[django(sort, op(lt, lte, gt, gte))]
    pub logged: DateTime<Utc>,
    // from v02 api
    pub resource_uri: String,
}

/// A test result from the LAVA API
#[derive(
    Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, EnumString, Display, SerializeDisplay,
)]
#[strum(serialize_all = "snake_case")]
pub enum PassFail {
    Fail,
    Pass,
    Skip,
    Unknown,
}

impl django_query::filtering::ops::Scalar for PassFail {}
impl django_query::row::StringCellValue for PassFail {}

#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd, Serialize)]
pub struct Decimal(rust_decimal::Decimal);

impl Deref for Decimal {
    type Target = rust_decimal::Decimal;
    fn deref(&self) -> &rust_decimal::Decimal {
        &self.0
    }
}

impl DerefMut for Decimal {
    fn deref_mut(&mut self) -> &mut rust_decimal::Decimal {
        &mut self.0
    }
}

impl FromStr for Decimal {
    type Err = <rust_decimal::Decimal as FromStr>::Err;
    fn from_str(value: &str) -> Result<Decimal, Self::Err> {
        Ok(Self(rust_decimal::Decimal::from_str(value)?))
    }
}

impl Display for Decimal {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        self.0.fmt(f)
    }
}

impl From<rust_decimal::Decimal> for Decimal {
    fn from(val: rust_decimal::Decimal) -> Decimal {
        Self(val)
    }
}

impl From<Decimal> for rust_decimal::Decimal {
    fn from(val: Decimal) -> rust_decimal::Decimal {
        val.0
    }
}

impl django_query::filtering::ops::Scalar for Decimal {}
impl django_query::row::StringCellValue for Decimal {}

/// YAML encoded [`Metadata`] objects.
pub struct MetadataGenerator(<Metadata as Generatable>::Generator);

impl MetadataGenerator {
    /// Create a new generator
    ///
    /// Note that this generator can only contain a default
    /// [`Metadata`] generator at present. It's convenient only when
    /// you aren't particularly interested in the actual data, you
    /// just need something parseable.
    pub fn new() -> Self {
        Self(Metadata::generator())
    }
}

impl Generator for MetadataGenerator {
    type Output = String;
    fn generate(&mut self) -> Self::Output {
        serde_yaml::to_string(&self.0.generate()).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SharedState, State};

    use boulder::GeneratorWithPersianRugIterator;
    use boulder::{BuilderWithPersianRug, GeneratableWithPersianRug};
    use django_query::row::{CellValue, IntoRowWithContext, Serializer};
    use persian_rug::Proxy;
    use serde_json::Number;
    use test_log::test;

    #[test]
    fn test_builder() {
        let mut p = SharedState::new();
        let tc = {
            let m = p.mutate();

            let (tc, _) = TestCase::builder().build(m);
            tc
        };
        let map = TestCase::get_serializer(p.access()).to_row(&tc);
        assert_eq!(map["id"], CellValue::Number(Number::from(0)));
        assert_eq!(
            map["name"],
            CellValue::String("An example test case".to_string())
        );
        assert_eq!(map["unit"], CellValue::String("seconds".to_string()));
        assert_eq!(map["result"], CellValue::String("pass".to_string()));
        assert_eq!(map["measurement"], CellValue::Null);
        // assert_eq!(map["metadata"], CellValue::String(""));
        assert_eq!(map["suite"], CellValue::Number(Number::from(0)));
        assert_eq!(map["start_log_line"], CellValue::Null);
        assert_eq!(map["end_log_line"], CellValue::Null);
        assert_eq!(map["test_set"], CellValue::Number(Number::from(0)));
    }

    #[test]
    fn test_generator() {
        let mut p = SharedState::new();
        let gen = TestCase::<State>::generator();

        let tcs = GeneratorWithPersianRugIterator::new(gen, p.mutate())
            .take(5)
            .collect::<Vec<_>>();

        let ser = TestCase::get_serializer(p.access());
        for (i, tc) in tcs.iter().enumerate() {
            let map = ser.to_row(tc);
            let units = ["seconds".to_string(), "hours".to_string()];
            assert_eq!(map["id"], CellValue::Number(Number::from(i)));
            assert_eq!(map["name"], CellValue::String(format!("Test case {}", i)));
            assert_eq!(map["unit"], CellValue::String(units[i % 2].clone()));
            assert_eq!(map["result"], CellValue::String("pass".to_string()));
            assert_eq!(map["measurement"], CellValue::Null);
            // assert_eq!(map["metadata"], CellValue::String(""));
            assert_eq!(map["suite"], CellValue::Number(Number::from(i)));
            assert_eq!(map["start_log_line"], CellValue::Null);
            assert_eq!(map["end_log_line"], CellValue::Null);
            assert_eq!(map["test_set"], CellValue::Number(Number::from(i)));
        }
    }

    #[test]
    fn test_metadata_output() {
        let mut mgen = MetadataGenerator(
            Metadata::generator()
                .case(Pattern!("example-case-{}", Inc(0)))
                .definition(Pattern!("example-definition-{}", Inc(0)))
                .result(|| PassFail::Pass)
                .level(Repeat!(None, Some("1.1.1".to_string())))
                .extra(Repeat!(None, Some("example-extra-data".to_string())))
                .namespace(Repeat!(None, Some("example-namespace".to_string())))
                .duration(Repeat!(None, Some(Decimal(dec!(0.10)))))
                .error_msg(|| None)
                .error_type(|| None),
        );

        let cases = vec![
            "case: example-case-0\ndefinition: example-definition-0\nresult: pass\n",
            "case: example-case-1\ndefinition: example-definition-1\nduration: '0.10'\nextra: example-extra-data\nlevel: 1.1.1\nnamespace: example-namespace\nresult: pass\n",
        ];

        for case in cases {
            let control: serde_yaml::Value =
                serde_yaml::from_str(case).expect("failed to parse control input");
            let test: serde_yaml::Value =
                serde_yaml::from_str(&mgen.generate()).expect("failed to generate test data");
            assert_eq!(test, control);
        }
    }

    #[test(tokio::test)]
    async fn test_output() {
        let mut p = SharedState::new();
        {
            let m = p.mutate();

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
                .suite(Proxy::<TestSuite<State>>::generator())
                .test_set(|| None)
                .resource_uri(Pattern!("example-resource-uri-{}", Inc(0)));

            let _ = GeneratorWithPersianRugIterator::new(gen, m)
                .take(4)
                .collect::<Vec<_>>();
        }

        let server = wiremock::MockServer::start().await;

        let ep = p.endpoint::<TestCase<State>>(Some(&server.uri()), None);

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/v0.2/jobs/0/tests/"))
            .respond_with(ep)
            .mount(&server)
            .await;

        let body: serde_json::Value =
            reqwest::get(&format!("{}/api/v0.2/jobs/0/tests/?limit=2", server.uri()))
                .await
                .expect("error getting tests")
                .json()
                .await
                .expect("error parsing tests");

        let next = format!("{}/api/v0.2/jobs/0/tests/?limit=2&offset=2", server.uri());

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
                            "result": "pass",
                            "resource_uri": "example-resource-uri-0",
                            "unit": "",
                            "name": "example-case-0",
                            "measurement": null,
                            "metadata": "case: example-case-0\ndefinition: example-definition-0\nresult: pass\n",
                            "start_log_line": null,
                            "end_log_line": null,
                            "logged": "2022-04-11T16:00:00.000000Z",
                            "suite": 0,
                            "test_set": null
                        },
                        {
                            "id": 1,
                            "result": "pass",
                            "resource_uri": "example-resource-uri-1",
                            "unit": "seconds",
                            "name": "example-case-1",
                            "measurement": "0.1000000000",
                            "metadata": "case: example-case-1\ndefinition: example-definition-1\nduration: '0.10'\nextra: example-extra-data\nlevel: 1.1.1\nnamespace: example-namespace\nresult: pass\n",
                            "start_log_line": null,
                            "end_log_line": null,
                            "logged": "2022-04-11T16:30:00.000000Z",
                            "suite": 1,
                            "test_set": null
                        }
                    ]
                }
            }
        );
    }
}
