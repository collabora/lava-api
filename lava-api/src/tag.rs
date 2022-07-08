//! Retrieve tags

use serde::Deserialize;

/// Metadata for a tag on the LAVA server
#[derive(Clone, Deserialize, Debug, PartialEq, Eq)]
pub struct Tag {
    /// The unique id of the tag
    pub id: u32,
    /// The name of the tag
    pub name: String,
    /// An optional description for this tag
    pub description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::Tag;
    use crate::Lava;

    use boulder::{Buildable, Builder};
    use lava_api_mock::{
        LavaMock, PaginationLimits, PopulationParams, SharedState, State, Tag as MockTag,
    };
    use persian_rug::{Accessor, Context};
    use std::collections::BTreeMap;
    use test_log::test;

    impl Tag {
        pub fn from_mock<'b, B, C>(tag: &MockTag<C>, _context: B) -> Tag
        where
            B: 'b + Accessor<Context = C>,
            C: Context + 'static,
        {
            Self {
                id: tag.id,
                name: tag.name.clone(),
                description: tag.description.clone(),
            }
        }
    }

    /// Stream 49 tags with a page limit of 5 from the server
    #[test(tokio::test)]
    async fn test_basic() {
        let state = SharedState::new_populated(PopulationParams::builder().tags(49usize).build());
        let server = LavaMock::new(
            state.clone(),
            PaginationLimits::builder().workers(Some(5)).build(),
        )
        .await;

        let mut map = BTreeMap::new();
        let start = state.access();
        for t in start.get_iter::<MockTag<State>>() {
            map.insert(t.id, t.clone());
        }

        let lava = Lava::new(&server.uri(), None).expect("failed to make lava server");

        let tags = lava.tags().await.expect("failed to get tags");

        let mut seen = BTreeMap::new();
        for tag in tags {
            assert!(!seen.contains_key(&tag.id));
            assert!(map.contains_key(&tag.id));
            let tk = map.get(&tag.id).unwrap();
            assert_eq!(tag.id, tk.id);
            assert_eq!(tag.name, tk.name);
            assert_eq!(tag.description, tk.description);

            seen.insert(tag.id, tag.clone());
        }
        assert_eq!(seen.len(), 49);
    }
}
