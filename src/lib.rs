pub mod device;
mod paginator;
pub mod worker;

use reqwest::Client;
use url::Url;

use device::Device;
use paginator::Paginator;
use worker::Worker;

pub struct Lava {
    client: Client,
    base: Url,
}

impl Lava {
    pub fn new(url: &str) -> Result<Lava, url::ParseError> {
        let host: Url = url.parse()?;
        let base = host.join("api/v0.2/")?;

        Ok(Lava {
            client: Client::new(),
            base,
        })
    }

    pub fn devices(&self) -> Paginator<Device> {
        Paginator::new(self.client.clone(), &self.base, "devices")
    }

    pub fn workers(&self) -> Paginator<Worker> {
        Paginator::new(self.client.clone(), &self.base, "workers")
    }
}
