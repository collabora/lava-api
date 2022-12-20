use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use futures::stream::TryStreamExt;
use futures::StreamExt;
use lava_api::device;
use lava_api::job;
use lava_api::joblog::JobLogError;
use lava_api::worker::{self, Worker};
use lava_api::Lava;
use structopt::StructOpt;
use tokio::time::sleep;

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

async fn devices(lava: &Lava) -> Result<()> {
    let mut devices = lava.devices();
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
    Ok(())
}

async fn log(lava: &Lava, opts: LogCmd) -> Result<()> {
    println!("Job log:");
    let mut log = lava.log(opts.job).log();

    while let Some(entry) = log.try_next().await? {
        println!("{:?}", entry);
    }
    Ok(())
}

async fn jobs(lava: &Lava, opts: JobsCmd) -> Result<()> {
    println!("\nQueued Jobs:");
    let mut jobs = lava
        .jobs()
        .limit(opts.limit)
        .state(job::State::Submitted)
        .query();
    let mut num = opts.limit;
    while let Some(w) = jobs.try_next().await? {
        println!(" 💤️  [{}]  {}", w.id, w.description);
        num -= 1;
        if num == 0 {
            match jobs.reported_items() {
                Some(n) => println!("\n…and {} more jobs", n - 10),
                None => println!("\n…and an unknown amount of jobs"),
            };
            break;
        }
    }
    Ok(())
}

async fn submit(lava: &Lava, opts: SubmitCmd) -> Result<()> {
    let mut job = File::open(opts.job).context("Failed to open job file")?;
    let mut definition = String::new();
    job.read_to_string(&mut definition)
        .context("Failed to read job")?;

    let mut ids = lava.submit_job(&definition).await?;
    println!("Submitted job(s): {:?}", ids);
    let id = ids.pop().ok_or_else(|| anyhow!("No job id"))?;
    if opts.follow {
        // TODO support following more then 1 job
        let builder = lava.jobs().id(id);
        let mut offset = 0;
        loop {
            let mut jobs = builder.clone().query();
            match jobs.try_next().await {
                Ok(Some(job)) => {
                    //if job.state == job::State::Running {
                    let mut log = lava.log(job.id).start(offset).log();
                    while let Some(entry) = log.next().await {
                        match entry {
                            Ok(entry) => {
                                println!("{:?}: {:?}", entry.dt, entry.msg);
                                offset += 1;
                            }
                            Err(JobLogError::NoData) => (),
                            Err(JobLogError::ParseError(s, e)) => {
                                println!("Couldn't parse {} - {}", s.trim_end(), e);
                                offset += 1;
                            }
                            Err(e) => return Err(e.into()),
                        }
                    }
                    //}
                    if job.state == job::State::Finished {
                        break;
                    }
                }
                Ok(None) => bail!("Job not found"),
                Err(e) => {
                    println!("Failed to check status: {:?}", e);
                }
            }

            sleep(Duration::from_secs(10)).await;
        }
    }
    Ok(())
}

async fn workers(lava: &Lava) -> Result<()> {
    println!("\nWorkers:");
    let mut workers = lava.workers();
    while let Some(w) = workers.try_next().await? {
        println!(" {}  {}", worker_to_emoji(&w), w.hostname);
    }
    Ok(())
}

#[derive(StructOpt, Debug)]
struct SubmitCmd {
    #[structopt(short, long)]
    follow: bool,
    job: PathBuf,
}

#[derive(StructOpt, Debug)]
struct LogCmd {
    #[structopt(short, long)]
    _follow: bool,
    job: i64,
}

#[derive(StructOpt, Debug)]
struct JobsCmd {
    #[structopt(short, long, default_value = "10")]
    limit: u32,
}

#[derive(StructOpt, Debug)]
enum Command {
    /// List devices
    Devices,
    /// Show a job log
    Log(LogCmd),
    /// Submit a job
    Submit(SubmitCmd),
    /// List jobs
    Jobs(JobsCmd),
    /// List workers
    Workers,
}

#[derive(StructOpt, Debug)]
struct Opt {
    #[structopt(short, long, default_value = "https://lava.collabora.co.uk")]
    url: String,
    #[structopt(short, long)]
    token: Option<String>,
    #[structopt(subcommand)]
    command: Command,
}

#[tokio::main]
async fn main() -> Result<()> {
    let env = env_logger::Env::default()
        .filter_or("LAVA_LOG", "lava_monitor=info")
        .write_style("LAVA_WRITE_STYLE");
    env_logger::init_from_env(env);

    let opts = Opt::from_args();
    let l = Lava::new(&opts.url, opts.token)?;

    match opts.command {
        Command::Devices => devices(&l).await?,
        Command::Submit(s) => submit(&l, s).await?,
        Command::Log(opts) => log(&l, opts).await?,
        Command::Jobs(j) => jobs(&l, j).await?,
        Command::Workers => workers(&l).await?,
    }

    Ok(())
}
