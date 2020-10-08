use serde::Deserialize;
use std::convert::TryFrom;
use thiserror::Error;

#[derive(Copy, Deserialize, Clone, Debug, PartialEq)]
#[serde(try_from = "&str")]
pub enum Health {
    Unknown,
    Maintenance,
    Good,
    Bad,
    Looping,
    Retired,
}

#[derive(Clone, Debug, Error)]
#[error("Failed to convert into Health")]
pub struct TryFromHealthError {}

impl TryFrom<&str> for Health {
    type Error = TryFromHealthError;
    fn try_from(v: &str) -> Result<Self, Self::Error> {
        match v {
            "Unknown" => Ok(Health::Unknown),
            "Maintenance" => Ok(Health::Maintenance),
            "Good" => Ok(Health::Good),
            "Bad" => Ok(Health::Bad),
            "Looping" => Ok(Health::Looping),
            "Retired" => Ok(Health::Retired),
            _ => Err(TryFromHealthError {}),
        }
    }
}

#[derive(Clone, Deserialize, Debug)]
pub struct Device {
    pub hostname: String,
    pub worker_host: String,
    pub device_type: String,
    pub description: Option<String>,
    pub health: Health,
}
