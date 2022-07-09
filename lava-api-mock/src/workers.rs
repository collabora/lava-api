use chrono::{DateTime, Utc};
use strum::{Display, EnumString};

use boulder::{BuildableWithPersianRug, GeneratableWithPersianRug};
use boulder::{Inc, Pattern};
use django_query::filtering::{ops::Scalar, FilterableWithPersianRug};
use django_query::{row::IntoRowWithPersianRug, sorting::SortableWithPersianRug};

use persian_rug::{contextual, Context};

#[derive(
    Clone,
    Debug,
    IntoRowWithPersianRug,
    FilterableWithPersianRug,
    SortableWithPersianRug,
    BuildableWithPersianRug,
    GeneratableWithPersianRug,
)]
#[boulder(persian_rug(context=C, access(Worker<C>)))]
#[django(persian_rug(context=C, access(Worker<C>)))]
#[contextual(C)]
pub struct Worker<C: Context + 'static> {
    #[django(exclude)]
    _marker: core::marker::PhantomData<C>,
    #[boulder(generator=Pattern!("a-test-worker-{}", Inc(1)))]
    #[django(sort, op(in, contains, icontains, startswith, endswith))]
    pub hostname: String,
    #[boulder(default=Some("A test worker".to_string()))]
    #[django(sort, op(in, contains, icontains, startswith, endswith))]
    pub description: Option<String>,
    #[boulder(default=DateTime::parse_from_rfc3339("2022-03-17T17:00:00-00:00").unwrap().with_timezone(&Utc))]
    #[django(sort, op(lt, gt))]
    pub last_ping: Option<DateTime<Utc>>,
    #[boulder(default=State::Online)]
    #[django(sort)]
    pub state: State,
    #[boulder(default=Health::Active)]
    #[django(sort)]
    pub health: Health,
    #[boulder(default = 100)]
    #[django(unfilterable)]
    pub job_limit: i64,
    #[boulder(default=Some("1.0".to_string()))]
    #[django(unfilterable)]
    pub version: Option<String>,
    #[boulder(default=Some("1.0".to_string()))]
    #[django(unfilterable)]
    pub master_version_notified: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Display, EnumString)]
pub enum Health {
    Active,
    Maintenance,
    Retired,
}

impl Scalar for Health {}
impl django_query::row::StringCellValue for Health {}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Display, EnumString)]
pub enum State {
    Online,
    Offline,
}

impl Scalar for State {}
impl django_query::row::StringCellValue for State {}
