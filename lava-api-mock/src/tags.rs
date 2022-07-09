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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SharedState, State};

    use boulder::GeneratorWithPersianRugIterator;
    use persian_rug::Proxy;
    use test_log::test;

    #[test(tokio::test)]
    async fn test_output() {
        let mut p = SharedState::new();
        {
            let m = p.mutate();

            let gen = Proxy::<Tag<State>>::generator()
                .id(Inc(1))
                .description(|| None);

            let _ = GeneratorWithPersianRugIterator::new(gen, m)
                .take(4)
                .collect::<Vec<_>>();
        }

        let server = wiremock::MockServer::start().await;

        let ep = p.endpoint::<Tag<_>>(Some(&server.uri()), None);

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/v0.2/tags/"))
            .respond_with(ep)
            .mount(&server)
            .await;

        let body: serde_json::Value =
            reqwest::get(&format!("{}/api/v0.2/tags/?limit=2", server.uri()))
                .await
                .expect("error getting tags")
                .json()
                .await
                .expect("error parsing tags/");

        let next = format!("{}/api/v0.2/tags/?limit=2&offset=2", server.uri());

        assert_eq!(
            body,
            serde_json::json! {
                {
                    "count": 4,
                    "next": next,
                    "previous": null,
                    "results": [
                        {
                            "id": 1,
                            "name": "test-tag-0",
                            "description": null
                        },
                        {
                            "id": 2,
                            "name": "test-tag-1",
                            "description": null
                        }
                    ]
                }
            }
        );
    }
}
