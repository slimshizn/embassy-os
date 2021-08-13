use chrono::{DateTime, Utc};
use clap::ArgMatches;
use rpc_toolkit::command;
use tokio::process::Command;
use tokio::sync::RwLock;

use crate::context::RpcContext;
use crate::{Error, ErrorKind, ResultExt};

pub const SYSTEMD_UNIT: &'static str = "embassyd";

fn parse_datetime(text: &str, _matches: &ArgMatches) -> Result<DateTime<Utc>, Error> {
    text.parse().with_kind(ErrorKind::ParseTimestamp)
}

#[command(rpc_only)]
pub async fn logs(
    #[context] _ctx: RpcContext,
    #[arg(parse(crate::system::parse_datetime))] before: Option<DateTime<Utc>>,
    #[arg] limit: Option<usize>,
) -> Result<Vec<(String, String)>, Error> {
    let before = before.unwrap_or(Utc::now());
    let limit = limit.unwrap_or(50);
    // Journalctl has unexpected behavior where "until" does not play well with "lines" unless the output is reversed.
    // Keep this in mind if you are changing the code below
    let out = Command::new("journalctl")
        .args(&[
            "-u",
            SYSTEMD_UNIT,
            &format!(
                "-U\"{} {} UTC\"",
                before.date().naive_utc(),
                before.time().format("%H:%M:%S")
            ),
            "--output=short-iso",
            "--no-hostname",
            "--utc",
            "--reverse",
            &format!("-n{}", limit),
        ])
        .output()
        .await?
        .stdout;
    let out_string = String::from_utf8(out)?;
    let lines = out_string.lines();
    let mut split_lines = lines
        .skip(1) // ditch the journalctl header
        .map(|s| {
            // split the timestamp off from the log line
            let (ts, l) = s.split_once(" ").unwrap();
            (ts.to_owned(), l.to_owned())
        })
        .collect::<Vec<(String, String)>>();
    // reverse output again because we reversed it above
    split_lines.reverse();
    Ok(split_lines)
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct Celsius(f64);
impl fmt::Display for Celsius {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:.1}°C", self.0)
    }
}
#[derive(serde::Serialize, Clone, Debug)]
pub struct Percentage(f64);
#[derive(serde::Serialize, Clone, Debug)]
pub struct MebiBytes(f64);
#[derive(serde::Serialize, Clone, Debug)]
pub struct GigaBytes(f64);

#[derive(serde::Serialize, Clone, Debug)]
pub struct MetricsGeneral {
    temperature: Celsius,
}
#[derive(serde::Serialize, Clone, Debug)]
pub struct MetricsMemory {
    percentage_used: Percentage,
    total: MebiBytes,
    available: MebiBytes,
    used: MebiBytes,
    swap_total: MebiBytes,
    swap_free: MebiBytes,
    swap_used: MebiBytes,
}
#[derive(serde::Serialize, Clone, Debug)]
pub struct MetricsCpu {
    user_space: Percentage,
    kernel_space: Percentage,
    wait: Percentage,
    idle: Percentage,
    usage: Percentage,
}
#[derive(serde::Serialize, Clone, Debug)]
pub struct MetricsDisk {
    size: GigaBytes,
    used: GigaBytes,
    available: GigaBytes,
    used_percentage: Percentage,
}
#[derive(serde::Serialize, Clone, Debug)]
pub struct Metrics {
    general: MetricsGeneral,
    memory: MetricsMemory,
    cpu: MetricsCpu,
    disk: MetricsDisk,
}

#[command(rpc_only)]
pub async fn metrics(#[context] ctx: RpcContext) -> Result<Metrics, Error> {
    match ctx.metrics_cache.read().await.clone() {
        None => Err(Error {
            source: anyhow::anyhow!("No Metrics Found"),
            kind: ErrorKind::NotFound,
            revision: None,
        }),
        Some(metrics_val) => Ok(metrics_val),
    }
}

pub async fn launch_metrics_task(cache: &RwLock<Option<Metrics>>) {
    // fetch init temp
    let init_temp;
    loop {
        match get_temp().await {
            Ok(a) => {
                init_temp = a;
                break;
            }
            Err(e) => {
                log::error!("Could not get initial temperature: {}", e);
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
    // fetch init cpu
    let init_cpu;
    let proc_stat;
    loop {
        match get_proc_stat().await {
            Ok(mut ps) => match get_cpu_info(&mut ps).await {
                Ok(mc) => {
                    proc_stat = ps;
                    init_cpu = mc;
                    break;
                }
                Err(e) => {
                    log::error!("Could not get initial cpu info: {}", e);
                }
            },
            Err(e) => {
                log::error!("Could not get initial proc stat: {}", e);
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
    // fetch init memory
    let init_mem;
    loop {
        match get_mem_info().await {
            Ok(a) => {
                init_mem = a;
                break;
            }
            Err(e) => {
                log::error!("Could not get initial mem info: {}", e);
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
    // fetch init disk usage
    let init_disk;
    loop {
        match get_disk_info().await {
            Ok(a) => {
                init_disk = a;
                break;
            }
            Err(e) => {
                log::error!("Could not get initial disk info: {}", e);
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
    {
        // lock for writing
        let mut guard = cache.write().await;
        // write
        *guard = Some(Metrics {
            general: MetricsGeneral {
                temperature: init_temp,
            },
            memory: init_mem,
            cpu: init_cpu,
            disk: init_disk,
        })
    }
    // launch persistent temp task
    let temp_task = launch_temp_task(cache);
    // launch persistent cpu task
    let cpu_task = launch_cpu_task(cache, proc_stat);
    // launch persistent mem task
    let mem_task = launch_mem_task(cache);
    // launch persistent disk task
    let disk_task = launch_disk_task(cache);
    tokio::join!(
        temp_task,
        cpu_task,
        mem_task,
        disk_task,
    );
}

async fn launch_temp_task(cache: &RwLock<Option<Metrics>>) {
    loop {
        match get_temp().await {
            Ok(a) => {
                let mut lock = cache.write().await;
                (*lock).as_mut().unwrap().general.temperature = a
            }
            Err(e) => {
                log::error!("Could not get new temperature: {}", e);
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
    }
}

async fn launch_cpu_task(cache: &RwLock<Option<Metrics>>, mut init: ProcStat) {
    loop {
        // read /proc/stat, diff against previous metrics, compute cpu load
        match get_cpu_info(&mut init).await {
            Ok(info) => {
                let mut lock = cache.write().await;
                (*lock).as_mut().unwrap().cpu = info;
            }
            Err(e) => {
                log::error!("Could not get new CPU Metrics: {}", e);
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
    }
}

async fn launch_mem_task(cache: &RwLock<Option<Metrics>>) {
    loop {
        // read /proc/meminfo
        match get_mem_info().await {
            Ok(a) => {
                let mut lock = cache.write().await;
                (*lock).as_mut().unwrap().memory = a;
            }
            Err(e) => {
                log::error!("Could not get new Memory Metrics: {}", e);
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
    }
}
async fn launch_disk_task(cache: &RwLock<Option<Metrics>>) {
    loop {
        // run df and capture output
        match get_disk_info().await {
            Ok(a) => {
                let mut lock = cache.write().await;
                (*lock).as_mut().unwrap().disk = a;
            }
            Err(e) => {
                log::error!("Could not get new Disk Metrics: {}", e);
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }
}

async fn get_temp() -> Result<Celsius, Error> {
    let milli = tokio::fs::read_to_string("/sys/class/thermal/thermal_zone0/temp")
        .await?
        .trim()
        .parse::<f64>()?;
    Ok(Celsius(milli / 1000.0))
}

#[derive(Debug, Clone)]
pub struct ProcStat {
    user: u64,
    nice: u64,
    system: u64,
    idle: u64,
    iowait: u64,
    irq: u64,
    softirq: u64,
    // below are only applicable to virtualized environments
    // steal: u64,
    // guest: u64,
    // guest_nice: u64,
}
impl ProcStat {
    fn total(&self) -> u64 {
        self.user + self.nice + self.system + self.idle + self.iowait + self.irq + self.softirq
    }
    fn user(&self) -> u64 {
        self.user + self.nice
    }
    fn system(&self) -> u64 {
        self.system + self.irq + self.softirq
    }
    fn used(&self) -> u64 {
        self.user() + self.system()
    }
}

async fn get_proc_stat() -> Result<ProcStat, Error> {
    use tokio::io::AsyncBufReadExt;
    let mut cpu_line = String::new();
    let _n = tokio::io::BufReader::new(tokio::fs::File::open("/proc/stat").await?)
        .read_line(&mut cpu_line)
        .await?;
    let stats: Vec<u64> = cpu_line
        .split_whitespace()
        .skip(1)
        .map(|s| {
            s.parse::<u64>().map_err(|e| Error::new(
                anyhow::anyhow!("Invalid /proc/stat column value: {}", e),
                ErrorKind::ParseSysInfo
            ))
        })
        .collect::<Result<Vec<u64>, Error>>()?;

    if stats.len() < 10 {
        Err(Error {
            source: anyhow::anyhow!(
                "Columns missing from /proc/stat. Need 10, found {}",
                stats.len()
            ),
            kind: ErrorKind::ParseSysInfo,
            revision: None,
        })
    } else {
        Ok(ProcStat {
            user: stats[0],
            nice: stats[1],
            system: stats[2],
            idle: stats[3],
            iowait: stats[4],
            irq: stats[5],
            softirq: stats[6],
        })
    }
}

async fn get_cpu_info(last: &mut ProcStat) -> Result<MetricsCpu, Error> {
    let new = get_proc_stat().await?;
    let total_old = last.total();
    let total_new = new.total();
    let total_diff = total_new - total_old;
    let res = MetricsCpu {
        user_space: Percentage((new.user() - last.user()) as f64 / total_diff as f64),
        kernel_space: Percentage((new.system() - last.system()) as f64 / total_diff as f64),
        idle: Percentage((new.idle - last.idle) as f64 / total_diff as f64),
        wait: Percentage((new.iowait - last.iowait) as f64 / total_diff as f64),
        usage: Percentage((new.used() - last.used()) as f64 / total_diff as f64),
    };
    *last = new;
    Ok(res)
}

pub struct MemInfo {
    mem_total: u64,
    mem_free: u64,
    mem_available: u64,
    buffers: u64,
    cached: u64,
    slab: u64,
    swap_total: u64,
    swap_free: u64,
}
async fn get_mem_info() -> Result<MetricsMemory, Error> {
    let contents = tokio::fs::read_to_string("/proc/meminfo").await?;
    let mut mem_info = MemInfo {
        mem_total: 0,
        mem_free: 0,
        mem_available: 0,
        buffers: 0,
        cached: 0,
        slab: 0,
        swap_total: 0,
        swap_free: 0,
    };
    let mut counter = 0;
    for entry in contents.lines() {
        if entry.starts_with("MemTotal") {
            mem_info.mem_total = entry.split_whitespace().skip(1).next().unwrap().parse()?;
            counter += 1;
        } else if entry.starts_with("MemFree") {
            mem_info.mem_free = entry.split_whitespace().skip(1).next().unwrap().parse()?;
            counter += 1;
        } else if entry.starts_with("MemAvailable") {
            mem_info.mem_available = entry.split_whitespace().skip(1).next().unwrap().parse()?;
            counter += 1;
        } else if entry.starts_with("Buffers") {
            mem_info.buffers = entry.split_whitespace().skip(1).next().unwrap().parse()?;
            counter += 1;
        } else if entry.starts_with("Cached") {
            mem_info.cached = entry.split_whitespace().skip(1).next().unwrap().parse()?;
            counter += 1;
        } else if entry.starts_with("Slab") {
            mem_info.slab = entry.split_whitespace().skip(1).next().unwrap().parse()?;
            counter += 1;
        } else if entry.starts_with("SwapTotal") {
            mem_info.swap_total = entry.split_whitespace().skip(1).next().unwrap().parse()?;
            counter += 1;
        } else if entry.starts_with("SwapFree") {
            mem_info.swap_free = entry.split_whitespace().skip(1).next().unwrap().parse()?;
            counter += 1;
        }
    }
    if counter != 8 {
        Err(Error {
            source: anyhow::anyhow!("Invalid Output from /proc/meminfo: {}", contents),
            kind: ErrorKind::ParseSysInfo,
            revision: None,
        })
    } else {
        let total = MebiBytes(mem_info.mem_total as f64 / 1024.0);
        let available = MebiBytes(mem_info.mem_available as f64 / 1024.0);
        let used = MebiBytes(
            (mem_info.mem_total
                - mem_info.mem_free
                - mem_info.buffers
                - mem_info.cached
                - mem_info.slab) as f64
                / 1024.0,
        );
        let swap_total = MebiBytes(mem_info.swap_total as f64 / 1024.0);
        let swap_free = MebiBytes(mem_info.swap_free as f64 / 1024.0);
        let swap_used = MebiBytes((mem_info.swap_total - mem_info.swap_free) as f64 / 1024.0);
        let percentage_used = Percentage(used.0 / total.0 * 100.0);
        Ok(MetricsMemory {
            percentage_used,
            total,
            available,
            used,
            swap_total,
            swap_free,
            swap_used,
        })
    }
}

async fn get_disk_info() -> Result<MetricsDisk, Error> {
    tokio::task::spawn_blocking(move || {
        let fs_res = nix::sys::statfs::statfs("/").map_err(|e| Error {
            source: anyhow::anyhow!("statfs panicked: {}", e),
            kind: ErrorKind::ParseSysInfo,
            revision: None,
        })?;
        let block_size = fs_res.block_size() as u64;
        let blocks = fs_res.blocks();
        let blocks_available = fs_res.blocks_available();
        fn to_gigs(n: u64) -> GigaBytes {
            GigaBytes(n as f64 / (1u64 << 30) as f64)
        }
        let size = to_gigs(blocks * block_size);
        let used = to_gigs(block_size * (blocks - blocks_available));
        let available = to_gigs(block_size * blocks_available);
        let used_percentage = Percentage(used.0 / size.0 * 100.0);
        Ok(MetricsDisk {
            size,
            used,
            available,
            used_percentage,
        })
    })
    .await
    .map_err(|e| Error {
        source: anyhow::anyhow!("statfs panicked: {}", e),
        kind: ErrorKind::ParseSysInfo,
        revision: None,
    })?
}

#[test]
pub fn test_datetime_output() {
    println!(
        "{} {} UTC",
        Utc::now().date().naive_utc(),
        Utc::now().time().format("%H:%M:%S")
    )
}

#[tokio::test]
pub async fn test_get_temp() {
    println!("{}", get_temp().await.unwrap())
}

#[tokio::test]
pub async fn test_get_proc_stat() {
    println!("{:?}", get_proc_stat().await.unwrap())
}

#[tokio::test]
pub async fn test_get_mem_info() {
    println!("{:?}", get_mem_info().await.unwrap())
}

#[tokio::test]
pub async fn test_get_disk_usage() {
    println!("{:?}", get_disk_info().await.unwrap())
}
