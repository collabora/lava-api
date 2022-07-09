use boulder::{BuildableWithPersianRug, GeneratableWithPersianRug};
use boulder::{Inc, Pattern};
use django_query::{
    filtering::FilterableWithPersianRug, row::IntoRowWithPersianRug,
    sorting::SortableWithPersianRug,
};

use persian_rug::{contextual, Context};

#[derive(
    Debug,
    Clone,
    FilterableWithPersianRug,
    SortableWithPersianRug,
    IntoRowWithPersianRug,
    BuildableWithPersianRug,
    GeneratableWithPersianRug,
)]
#[django(persian_rug(context = C, access(Tag<C>)))]
#[boulder(persian_rug(context = C, access(Tag<C>)))]
#[contextual(C)]
pub struct Tag<C: Context + 'static> {
    #[django(exclude)]
    _marker: core::marker::PhantomData<C>,
    #[boulder(generator=Inc(0u32))]
    pub id: u32,
    #[boulder(default="test-tag", generator=Pattern!("test-tag-{}", Inc(0)))]
    #[django(sort, op(in, contains, icontains, startswith, endswith))]
    pub name: String,
    #[boulder(default=Some("An example tag description".to_string()))]
    #[django(sort, op(in, contains, icontains, startswith, endswith))]
    pub description: Option<String>,
}
