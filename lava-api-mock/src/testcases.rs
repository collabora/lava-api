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

pub struct MetadataGenerator(<Metadata as Generatable>::Generator);

impl MetadataGenerator {
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
