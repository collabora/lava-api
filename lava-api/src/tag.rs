use serde::Deserialize;

#[derive(Clone, Deserialize, Debug, PartialEq, Eq)]
pub struct Tag {
    pub id: u32,
    pub name: String,
    pub description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::Tag;

    use lava_api_mock::Tag as MockTag;
    use persian_rug::{Accessor, Context};

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
}
