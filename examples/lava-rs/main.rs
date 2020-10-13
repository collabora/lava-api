use anyhow::Error;
use lava_api::device;
use lava_api::worker::{self, Worker};
use lava_api::Lava;
use structopt::StructOpt;
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

#[derive(StructOpt)]
struct Opt {
    #[structopt(short, long, default_value = "https://lava.collabora.com")]
    url: String,
    #[structopt(short, long)]
    token: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let env = env_logger::Env::default()
        .filter_or("LAVA_LOG", "lava_monitor=info")
        .write_style("LAVA_WRITE_STYLE");
    env_logger::init_from_env(env);

    let opts = Opt::from_args();
    let l = Lava::new(&opts.url, opts.token).unwrap();

    let mut devices = l.devices();
    println!("Devices:");
    while let Some(d) = devices.try_next().await? {
        println!(
            " {}  {} on {} tags {}",
            device_health_to_emoji(d.health),
            d.hostname,
            d.worker_host,
            d.tags
                .iter()
                .map(|t| t.name.as_str())
                .collect::<Vec<&str>>()
                .join(", "),
        );
    }

    println!("\nWorkers:");
    let mut workers = l.workers();
    while let Some(w) = workers.try_next().await? {
        println!(" {}  {}", worker_to_emoji(&w), w.hostname);
    }

    Ok(())
}
