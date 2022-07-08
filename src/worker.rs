use serde::Deserialize;
use serde_with::DeserializeFromStr;
use strum::{Display, EnumString};

#[derive(Copy, Clone, Debug, DeserializeFromStr, Display, EnumString, PartialEq)]
pub enum Health {
    Active,
    Maintenance,
    Retired,
}

#[derive(Copy, Clone, Debug, DeserializeFromStr, Display, EnumString, PartialEq)]
pub enum State {
    Online,
    Offline,
}

#[derive(Clone, Deserialize, Debug)]
pub struct Worker {
    pub hostname: String,
    pub state: State,
    pub health: Health,
}
