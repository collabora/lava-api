use serde::Deserialize;
use std::convert::TryFrom;
use thiserror::Error;

#[derive(Copy, Deserialize, Clone, Debug, PartialEq)]
#[serde(try_from = "&str")]
pub enum Health {
    Active,
    Maintenance,
    Retired,
}

#[derive(Clone, Debug, Error)]
#[error("Failed to convert into State")]
pub struct TryFromHealthError {}

impl TryFrom<&str> for Health {
    type Error = TryFromStateError;
    fn try_from(v: &str) -> Result<Self, Self::Error> {
        match v {
            "Active" => Ok(Health::Active),
            "Maintenance" => Ok(Health::Maintenance),
            "Retired" => Ok(Health::Retired),
            _ => Err(TryFromStateError {}),
        }
    }
}

#[derive(Copy, Deserialize, Clone, Debug, PartialEq)]
#[serde(try_from = "&str")]
pub enum State {
    Online,
    Offline,
}

#[derive(Clone, Debug, Error)]
#[error("Failed to convert into State")]
pub struct TryFromStateError {}

impl TryFrom<&str> for State {
    type Error = TryFromStateError;
    fn try_from(v: &str) -> Result<Self, Self::Error> {
        match v {
            "Online" => Ok(State::Online),
            "Offline" => Ok(State::Offline),
            _ => Err(TryFromStateError {}),
        }
    }
}

#[derive(Clone, Deserialize, Debug)]
pub struct Worker {
    pub hostname: String,
    pub state: State,
    pub health: Health,
}
