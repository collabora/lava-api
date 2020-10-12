use anyhow::Error;
use lava_api::device;
use lava_api::worker::{self, Worker};
use lava_api::Lava;
use tokio::stream::StreamExt;

fn device_health_to_emoji(health: device::Health) -> &'static str {
    use device::Health::*;
    match health {
        Unknown => "❓",
        Maintenance => "🔨",
        Good => "💚",
        Bad => "💢",
        Looping => "➿",
        Retired => "⚰️",
    }
}

fn worker_to_emoji(w: &Worker) -> &'static str {
    use worker::Health::*;
    use worker::State::*;
    match w.health {
        Active => match w.state {
            Online => "💚",
            Offline => "💢",
        },
        Maintenance => "🔨",
        Retired => "⚰️",
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let l = Lava::new("https://lava.collabora.com").unwrap();

    let mut devices = l.devices();

    println!("Devices:");
    while let Some(d) = devices.try_next().await? {
        println!(
            " {}  {} on {} tags {:?}",
            device_health_to_emoji(d.health),
            d.hostname,
            d.worker_host,
            d.tags,
        );
    }

    println!("\nWorkers:");
    let mut workers = l.workers();
    while let Some(w) = workers.try_next().await? {
        println!(" {}  {}", worker_to_emoji(&w), w.hostname);
    }

    Ok(())
}
