use serde::Deserialize;

#[derive(Clone, Deserialize, Debug, PartialEq, Eq)]
pub struct Tag {
    pub id: u32,
    pub name: String,
    pub description: Option<String>,
}
