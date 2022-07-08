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

#[cfg(test)]
mod tests {
    use crate::Lava;
    use boulder::{Buildable, Builder};
    use futures::TryStreamExt;
    use lava_api_mock::{LavaMock, PaginationLimits, PopulationParams, SharedState, State, Worker};
    use persian_rug::Accessor;
    use std::collections::BTreeMap;
    use test_log::test;

    /// Stream 51 workers with a page limit of 2 from the server
    #[test(tokio::test)]
    async fn test_basic() {
        let state =
            SharedState::new_populated(PopulationParams::builder().workers(51usize).build());
        let server = LavaMock::new(
            state.clone(),
            PaginationLimits::builder().workers(Some(2)).build(),
        )
        .await;

        let mut map = BTreeMap::new();
        let start = state.access();
        for d in start.get_iter::<Worker<State>>() {
            map.insert(d.hostname.clone(), d.clone());
        }

        let lava = Lava::new(&server.uri(), None).expect("failed to make lava server");

        let mut lw = lava.workers();

        let mut seen = BTreeMap::new();
        while let Some(worker) = lw.try_next().await.expect("failed to get worker") {
            assert!(!seen.contains_key(&worker.hostname));
            assert!(map.contains_key(&worker.hostname));
            let wk = map.get(&worker.hostname).unwrap();
            assert_eq!(worker.hostname, wk.hostname);
            assert_eq!(worker.state.to_string(), wk.state.to_string());
            assert_eq!(worker.health.to_string(), wk.health.to_string());

            seen.insert(worker.hostname.clone(), worker.clone());
        }
        assert_eq!(seen.len(), 51);
    }
}
