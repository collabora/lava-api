use anyhow::Error;
use lava_api::device;
use lava_api::job;
use lava_api::worker::{self, Worker};
use lava_api::Lava;
use structopt::StructOpt;
use futures::stream::TryStreamExt;

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

    println!("\nQueued Jobs:");
    let mut jobs = l.jobs().state(job::State::Submitted).query();
    let mut num = 10;
    while let Some(w) = jobs.try_next().await? {
        println!(" 💤️  [{}]  {}", w.id, w.description);
        num = num - 1;
        if num == 0 {
            match jobs.reported_items() {
                Some(n) => println!("\n…and {} more jobs", n-10),
                None => println!("\n…and an unknown amount of jobs")
            };
            break;
        }
    }



    Ok(())
}
