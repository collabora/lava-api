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

impl Tag {
    pub fn from_mock<'b, B, C>(tag: &lava_api_mock::Tag<C>, _context: B) -> Tag
    where
        B: 'b + persian_rug::Accessor<Context = C>,
        C: persian_rug::Context + 'static,
    {
        Self {
            id: tag.id,
            name: tag.name.clone(),
            description: tag.description.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Lava;

    use boulder::{Buildable, Builder};
    use chrono::Utc;
    use lava_api_mock::{
        create_mock, PaginationLimits, PopulationParams, Server, SharedState, State, Tag as MockTag,
    };
    use persian_rug::Accessor;
    use std::collections::{BTreeMap, BTreeSet};
    use std::iter::FromIterator;
    use test_log::test;

    /// Stream 49 tags with a page limit of 5 from the server
    #[test(tokio::test)]
    async fn test_basic() {
        let state = SharedState::new_populated(PopulationParams::builder().tags(49usize).build());
        let server = Server::new(
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

    #[test(tokio::test)]
    async fn test_basic_mock() {
        let (mut p, _clock) = create_mock(Utc::now()).await;

        let tag_names = BTreeSet::from_iter(p.generate_tags(49).into_iter());

        let lava = Lava::new(&p.uri(), None).expect("failed to make lava server");

        let tags = lava.tags().await.expect("failed to get tags");

        let mut seen = BTreeSet::new();
        for tag in tags {
            assert!(!seen.contains(&tag.id));
            assert!(tag_names.contains(&tag.name));

            p.with_tag(&tag.name, |tk| {
                assert_eq!(tag.id, tk.id);
                assert_eq!(tag.name, tk.name);
                assert_eq!(tag.description, tk.description);
            });

            seen.insert(tag.id);
        }
        assert_eq!(seen.len(), 49);
    }
}
