use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use futures::StreamExt;
use futures::stream::TryStreamExt;
use lava_api::Lava;
use lava_api::device;
use lava_api::job;
use lava_api::joblog::JobLogError;
use lava_api::worker::{self, Worker};
use structopt::StructOpt;
use tokio::time::sleep;

use chrono::{DateTime, Utc};

fn format_duration(since: DateTime<Utc>) -> String {
    let delta = Utc::now() - since;
    let days = delta.num_days();
    let hours = delta.num_hours() % 24;
    let mins = delta.num_minutes() % 60;

    if days > 0 {
        format!("{days}d {hours}h {mins}m")
    } else if delta.num_hours() > 0 {
        format!("{}h {}m", delta.num_hours(), mins)
    } else {
        format!("{}m", delta.num_minutes())
    }
}

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

async fn bad_devices(lava: &Lava) -> Result<()> {
    use std::collections::HashMap;

    #[derive(Debug)]
    struct BadDeviceRow {
        hostname: String,
        worker_host: String,
        description: Option<String>,
        tags: String,
        bad_since: Option<DateTime<Utc>>,
    }

    let mut rows = Vec::new();
    let mut devices = lava.devices();

    while let Some(d) = devices.try_next().await? {
        if d.health == device::Health::Bad {
            rows.push(BadDeviceRow {
                hostname: d.hostname,
                worker_host: d.worker_host,
                description: d.description,
                tags: d
                    .tags
                    .into_iter()
                    .map(|t| t.name)
                    .collect::<Vec<_>>()
                    .join(", "),
                bad_since: None,
            });
        }
    }

    if rows.is_empty() {
        println!("No bad devices found.");
        return Ok(());
    }

    let mut earliest_failed_healthcheck: HashMap<String, DateTime<Utc>> = HashMap::new();

    // Create a set of bad device hostnames
    let bad_device_hostnames: std::collections::HashSet<String> =
        rows.iter().map(|r| r.hostname.clone()).collect();

    // Filter for incomplete health checks on the server-side, ordered by oldest job to find when
    // the device first failed a health check.
    let mut jobs = lava
        .jobs()
        .state(job::State::Finished)
        .health(job::Health::Incomplete)
        .ordering(job::Ordering::EndTime, true)
        .query();

    let num_bad_devices = rows.len();

    while let Some(j) = jobs.try_next().await? {
        if j.health_check
            && let (Some(device), Some(end_time)) = (j.actual_device.as_ref(), j.end_time)
        {
            // Only track devices in the "bad devices" list
            if bad_device_hostnames.contains(device) {
                earliest_failed_healthcheck
                    .entry(device.clone())
                    .or_insert(end_time);

                // Stop when the earliest failure is found
                if earliest_failed_healthcheck.len() == num_bad_devices {
                    break;
                }
            }
        }
    }

    for row in &mut rows {
        row.bad_since = earliest_failed_healthcheck.get(&row.hostname).cloned();
    }

    // Sort devices with unknown durations first.
    rows.sort_by_key(|row| row.bad_since);

    println!("Bad Devices:");

    for row in &rows {
        match row.bad_since {
            Some(ts) => {
                println!(
                    " {} {} on {} [since {}]",
                    device_health_to_emoji(device::Health::Bad),
                    row.hostname,
                    row.worker_host,
                    format_duration(ts)
                );
            }
            None => {
                println!(
                    " {} {} on {} [bad duration unknown]",
                    device_health_to_emoji(device::Health::Bad),
                    row.hostname,
                    row.worker_host,
                );
            }
        }

        if !row.tags.is_empty() {
            println!("     Tags: {}", row.tags);
        }
        if let Some(desc) = &row.description {
            println!("     Description: {}", desc);
        }
    }

    println!("\nTotal bad devices: {}", rows.len());

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

async fn healthchecks(lava: &Lava, opts: HealthChecksCmd) -> Result<()> {
    println!("\nHealthcheck summary for Device: {}", opts.device);

    let mut finished_jobs = lava.jobs().state(job::State::Finished).query();

    let mut passed = 0;
    let mut failed = 0;
    let mut canceled = 0;
    let mut unknown = 0;

    while let Some(j) = finished_jobs.try_next().await? {
        if j.health_check && j.actual_device.as_deref() == Some(&opts.device) {
            match j.health {
                job::Health::Complete => passed += 1,
                job::Health::Incomplete => {
                    failed += 1;
                    if opts.show_failed {
                        println!("  ❌ Job [{}] failed", j.id);
                    }
                }
                job::Health::Canceled => canceled += 1,
                job::Health::Unknown => unknown += 1,
            }
        }
    }

    let total = passed + failed + canceled + unknown;
    println!("Total healthchecks ran: {}", total);
    println!("✅ Passed: {}", passed);
    println!("❌ Failed: {}", failed);
    if canceled > 0 {
        println!("⊘ Canceled: {}", canceled);
    }
    if unknown > 0 {
        println!("❓ Unknown: {}", unknown);
    }
    if total > 0 {
        let pass_rate = (passed as f64 / total as f64) * 100.0;
        println!("Pass rate: {:.1}%", pass_rate);
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
struct HealthChecksCmd {
    #[structopt(short, long)]
    device: String,
    #[structopt(short, long)]
    /// Show individual failed healthcheck jobs
    show_failed: bool,
}

#[derive(StructOpt, Debug)]
enum Command {
    /// List devices
    Devices,
    /// List bad devices
    BadDevices,
    /// Show a job log
    Log(LogCmd),
    /// Submit a job
    Submit(SubmitCmd),
    /// List jobs
    Jobs(JobsCmd),
    /// List workers
    Workers,
    /// Analyze healthcheck results for a device
    Healthchecks(HealthChecksCmd),
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
        Command::BadDevices => bad_devices(&l).await?,
        Command::Submit(s) => submit(&l, s).await?,
        Command::Log(opts) => log(&l, opts).await?,
        Command::Jobs(j) => jobs(&l, j).await?,
        Command::Workers => workers(&l).await?,
        Command::Healthchecks(h) => healthchecks(&l, h).await?,
    }

    Ok(())
}
