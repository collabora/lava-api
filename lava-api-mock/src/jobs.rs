use boulder::{BuildableWithPersianRug, GeneratableWithPersianRug};
use boulder::{Inc, Some as GSome, Time};
use chrono::{DateTime, Duration, Utc};
use django_query::{
    filtering::FilterableWithPersianRug, row::IntoRowWithPersianRug,
    sorting::SortableWithPersianRug,
};
use persian_rug::{contextual, Context, Proxy};
use strum::{Display, EnumString};

use crate::devices::Device;
use crate::devicetypes::{Alias, Architecture, BitWidth, Core, DeviceType, ProcessorFamily};
use crate::tags::Tag;
use crate::users::{Group, User};
use crate::workers::Worker;

// Filters from lava/lava_rest_app/filters.py
// FIXME: the model contains
// - is_public
// - target_group
// - sub_id
// That don't seem to appear in query output
#[derive(
    Clone,
    Debug,
    FilterableWithPersianRug,
    SortableWithPersianRug,
    IntoRowWithPersianRug,
    BuildableWithPersianRug,
    GeneratableWithPersianRug,
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
            Worker<C>
        )
    )
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
            Worker<C>
        )
    )
)]
#[contextual(C)]
pub struct Job<C: Context + 'static> {
    #[boulder(generator=Inc(0))]
    #[django(op(lt, gt, in), sort)]
    pub id: i64,
    #[boulder(buildable_with_persian_rug, generatable_with_persian_rug)]
    #[django(traverse, foreign_key = "username")]
    pub submitter: Proxy<User<C>>,
    #[boulder(generatable_with_persian_rug, sequence = 3usize)]
    #[django(traverse, foreign_key = "id")]
    pub viewing_groups: Vec<Proxy<Group<C>>>,
    // FIXME: verify: is this really mandatory?
    #[boulder(default = "Example job description")]
    #[django(op(in, contains, icontains, startswith, endswith))]
    pub description: String,
    #[boulder(default = true)]
    pub health_check: bool,
    #[boulder(buildable_with_persian_rug, generatable_with_persian_rug)]
    #[django(traverse, foreign_key = "name")]
    pub requested_device_type: Option<Proxy<DeviceType<C>>>,
    #[boulder(generatable_with_persian_rug, sequence = 4usize)]
    #[django(traverse, foreign_key = "id")]
    pub tags: Vec<Proxy<Tag<C>>>,
    #[boulder(buildable_with_persian_rug, generatable_with_persian_rug)]
    #[django(traverse, foreign_key = "hostname")]
    pub actual_device: Option<Proxy<Device<C>>>,
    #[boulder(default=Some(DateTime::parse_from_rfc3339("2022-03-17T17:00:00-00:00").unwrap().with_timezone(&Utc)),
              generator=GSome(Time::new(DateTime::parse_from_rfc3339("2022-03-17T17:00:00-00:00").unwrap().with_timezone(&Utc),
                                  Duration::minutes(1))))]
    #[django(op(gt, lt, isnull), sort)]
    pub submit_time: Option<DateTime<Utc>>,
    #[django(op(gt, lt, isnull), sort)]
    pub start_time: Option<DateTime<Utc>>,
    #[django(op(gt, lt, isnull), sort)]
    pub end_time: Option<DateTime<Utc>>,
    #[boulder(default=State::Submitted)]
    #[django(op(iexact, in))]
    pub state: State,
    #[boulder(default=Health::Unknown)]
    #[django(op(iexact, in))]
    pub health: Health,
    #[django(op(in, lt, gt, lte, gte))]
    pub priority: i64,
    #[boulder(default = "Example job definition")]
    #[django(op(in, contains, icontains, startswith, endswith))]
    pub definition: String,
    #[boulder(default = "Example job original definition")]
    #[django(op(in, contains, icontains, startswith, endswith))]
    pub original_definition: String,
    #[boulder(default = "Example job multinode definition")]
    #[django(op(in, contains, icontains, startswith, endswith))]
    pub multinode_definition: String,
    #[django(traverse, foreign_key = "id")]
    pub failure_tags: Vec<Proxy<Tag<C>>>,
    #[django(op(in, contains, icontains, startswith, endswith, isnull))]
    pub failure_comment: Option<String>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, EnumString, Display)]
pub enum Health {
    Unknown,
    Complete,
    Incomplete,
    Canceled,
}

impl django_query::filtering::ops::Scalar for Health {}
impl django_query::row::StringCellValue for Health {}

#[derive(Copy, Clone, Debug, PartialEq, Eq, EnumString, Display)]
pub enum State {
    Submitted,
    Scheduling,
    Scheduled,
    Running,
    Canceling,
    Finished,
}

impl django_query::filtering::ops::Scalar for State {}
impl django_query::row::StringCellValue for State {}
