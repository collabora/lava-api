use boulder::{BuildableWithPersianRug, GeneratableWithPersianRug};
use boulder::{Inc, Pattern, Some as GSome};
use django_query::{
    filtering::FilterableWithPersianRug, row::IntoRowWithPersianRug,
    sorting::SortableWithPersianRug,
};
use persian_rug::{contextual, Context, Proxy};

/// A user in the LAVA API
// FIXME: Deriving sort and IntoRowWithPersianRug needlessly
#[derive(
    Clone,
    Debug,
    FilterableWithPersianRug,
    SortableWithPersianRug,
    IntoRowWithPersianRug,
    BuildableWithPersianRug,
    GeneratableWithPersianRug,
)]
#[django(persian_rug(context=C, access(User<C>, Group<C>)))]
#[boulder(persian_rug(context=C, access(User<C>, Group<C>)))]
#[contextual(C)]
pub struct User<C: Context + 'static> {
    #[boulder(generator=Inc(0))]
    #[django(sort)]
    pub id: i64,
    #[boulder(buildable_with_persian_rug, generatable_with_persian_rug)]
    #[django(traverse, foreign_key = "id")]
    pub group: Option<Proxy<Group<C>>>,
    #[boulder(default="test-username",
              generator=Pattern!("test-user-{}", Inc(1)))]
    #[django(op(in, contains, icontains, startswith, endswith))]
    pub username: String,
    #[boulder(default=Some("test@test.com".to_string()),
              generator=GSome(Pattern!("test-user-{}@example.com", Inc(1))))]
    #[django(op(in, contains, icontains, startswith, endswith))]
    pub email: Option<String>,
}

/// A group in the LAVA API
// FIXME: Deriving sort and IntoRowWithPersianRug needlessly
#[derive(
    Clone,
    Debug,
    FilterableWithPersianRug,
    SortableWithPersianRug,
    IntoRowWithPersianRug,
    BuildableWithPersianRug,
    GeneratableWithPersianRug,
)]
#[django(persian_rug(context=C, access(Group<C>)))]
#[boulder(persian_rug(context=C, access(Group<C>)))]
#[contextual(C)]
pub struct Group<C: Context + 'static> {
    #[django(exclude)]
    _marker: core::marker::PhantomData<C>,
    #[boulder(generator=Inc(0))]
    #[django(sort)]
    pub id: i64,
    #[boulder(default="test-group",
              generator=Pattern!("test-group-{}", Inc(1)))]
    #[django(op(in, contains, icontains, startswith, endswith))]
    pub name: String,
}
