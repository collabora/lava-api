use boulder::{BuildableWithPersianRug, GeneratableWithPersianRug};
use boulder::{Inc, Pattern, Some as GSome};
use django_query::{
    filtering::FilterableWithPersianRug, row::IntoRowWithPersianRug,
    sorting::SortableWithPersianRug,
};
use persian_rug::{contextual, Context, Proxy};
use strum::{Display, EnumString};

use crate::{
    Alias, Architecture, BitWidth, Core, DeviceType, Group, Job, ProcessorFamily, Tag, User, Worker,
};

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
            Device<C>,
            DeviceType<C>,
            Alias<C>,
            Architecture<C>,
            BitWidth<C>,
            Core<C>,
            ProcessorFamily<C>,
            User<C>,
            Group<C>,
            Worker<C>,
            Job<C>,
            Tag<C>
        )
    )
)]
#[django(
    persian_rug(
        context=C,
        access(
            Device<C>,
            DeviceType<C>,
            Alias<C>,
            Architecture<C>,
            BitWidth<C>,
            Core<C>,
            ProcessorFamily<C>,
            User<C>,
            Group<C>,
            Worker<C>,
            Job<C>,
            Tag<C>
        )
    )
)]
#[contextual(C)]
pub struct Device<C: Context + 'static> {
    #[boulder(default="test-device",
              generator=Pattern!("test-device-{}", Inc(0)))]
    #[django(sort, op(in, contains, icontains, startswith, endswith))]
    pub hostname: String,
    #[boulder(buildable_with_persian_rug, generatable_with_persian_rug)]
    #[django(sort("name"), traverse, foreign_key = "name")]
    pub device_type: Proxy<DeviceType<C>>,
    #[boulder(default=Some("1.0".to_string()))]
    #[django(sort, op(in, contains, icontains, startswith, endswith))]
    pub device_version: Option<String>,
    #[boulder(buildable_with_persian_rug, generatable_with_persian_rug)]
    #[django(sort("id"), traverse, foreign_key = "id")]
    pub physical_owner: Option<Proxy<User<C>>>,
    #[boulder(buildable_with_persian_rug, generatable_with_persian_rug)]
    #[django(sort("id"), traverse, foreign_key = "id")]
    pub physical_group: Option<Proxy<Group<C>>>,
    #[boulder(default=Some("Test description for device.".to_string()),
              generator=GSome(Pattern!("Test description {}", Inc(0))))]
    #[django(sort, op(in, contains, icontains, startswith, endswith))]
    pub description: Option<String>,

    /* The API docs claim `tags` is a sort field, but testing this
    reveals it's simply ignored. There's no obvious way to sort by
    a vector of key ID, and the output is definitely not sorted.
    */
    #[boulder(generatable_with_persian_rug, sequence = 3usize)]
    #[django(traverse, foreign_key = "id")]
    pub tags: Vec<Proxy<Tag<C>>>,

    #[django(sort)]
    #[boulder(default=State::Idle)]
    pub state: State,
    #[django(sort)]
    #[boulder(default=Health::Good)]
    pub health: Health,
    #[boulder(buildable_with_persian_rug, generatable_with_persian_rug)]
    #[django(sort("hostname"), traverse, foreign_key = "hostname")]
    pub worker_host: Proxy<Worker<C>>,

    #[django(unfilterable)]
    pub is_synced: bool,
    #[django(unfilterable, foreign_key = "id")]
    pub last_health_report_job: Option<Proxy<Job<C>>>,
}

#[derive(Clone, Debug, PartialEq, Eq, EnumString, PartialOrd, Ord, Display)]
pub enum Health {
    Unknown,
    Maintenance,
    Good,
    Bad,
    Looping,
    Retired,
}

impl django_query::filtering::ops::Scalar for Health {}
impl django_query::row::StringCellValue for Health {}

#[derive(Clone, Debug, PartialEq, Eq, EnumString, PartialOrd, Ord, Display)]
pub enum State {
    Idle,
    Reserved,
    Running,
}

impl django_query::filtering::ops::Scalar for State {}
impl django_query::row::StringCellValue for State {}
