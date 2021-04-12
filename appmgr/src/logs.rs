use std::borrow::Cow;
use std::ffi::{OsStr, OsString};
use std::path::Path;

use futures::stream::{StreamExt, TryStreamExt};
use tokio_stream::wrappers::LinesStream;

use crate::util::PersistencePath;
use crate::{Error, ResultExt as _};

#[derive(Clone, Copy, Debug, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Level {
    Error,
    Warn,
    Success,
    Info,
}
impl std::fmt::Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Level::Error => write!(f, "ERROR"),
            Level::Warn => write!(f, "WARN"),
            Level::Success => write!(f, "SUCCESS"),
            Level::Info => write!(f, "INFO"),
        }
    }
}
impl std::str::FromStr for Level {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ERROR" => Ok(Level::Error),
            "WARN" => Ok(Level::Warn),
            "SUCCESS" => Ok(Level::Success),
            "INFO" => Ok(Level::Info),
            _ => Err(todo!()),
        }
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Notification {
    pub time: i64,
    pub level: Level,
    pub code: usize,
    pub title: String,
    pub message: String,
}
impl std::fmt::Display for Notification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}:{}:{}",
            self.level,
            self.code,
            self.title.replace(":", "\u{A789}"),
            self.message.replace("\n", "\u{2026}")
        )
    }
}
impl std::str::FromStr for Notification {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        todo!()
    }
}

pub struct LogOptions<A: AsRef<str>, B: AsRef<str>> {
    pub details: bool,
    pub follow: bool,
    pub since: Option<A>,
    pub until: Option<B>,
    pub tail: Option<usize>,
    pub timestamps: bool,
}

pub async fn logs<A: AsRef<str>, B: AsRef<str>>(
    name: &str,
    options: LogOptions<A, B>,
) -> Result<(), Error> {
    let mut args = vec![Cow::Borrowed(OsStr::new("logs"))];
    if options.details {
        args.push(Cow::Borrowed(OsStr::new("--details")));
    }
    if options.follow {
        args.push(Cow::Borrowed(OsStr::new("-f")));
    }
    if let Some(since) = options.since.as_ref() {
        args.push(Cow::Borrowed(OsStr::new("--since")));
        args.push(Cow::Borrowed(OsStr::new(since.as_ref())));
    }
    if let Some(until) = options.until.as_ref() {
        args.push(Cow::Borrowed(OsStr::new("--until")));
        args.push(Cow::Borrowed(OsStr::new(until.as_ref())));
    }
    if let Some(tail) = options.tail {
        args.push(Cow::Borrowed(OsStr::new("--tail")));
        args.push(Cow::Owned(OsString::from(format!("{}", tail))));
    }
    if options.timestamps {
        args.push(Cow::Borrowed(OsStr::new("-t")));
    }
    args.push(Cow::Borrowed(OsStr::new(name)));
    crate::ensure_code!(
        std::process::Command::new("docker")
            .args(args.into_iter())
            .status()?
            .success(),
        crate::ErrorKind::Docker,
        "Failed to Collect Logs from Docker"
    );
    Ok(())
}

pub async fn notifications(id: &str) -> Result<Vec<Notification>, Error> {
    let p = PersistencePath::from_ref("notifications").join(id).tmp();
    if let Some(parent) = p.parent() {
        if !parent.exists() {
            tokio::fs::create_dir_all(parent).await?;
        }
    }
    match tokio::fs::rename(
        Path::new(crate::VOLUMES)
            .join(id)
            .join("start9")
            .join("notifications.log"),
        &p,
    )
    .await
    {
        Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        a => a,
    }?;
    let f = tokio::fs::File::open(&p)
        .await
        .with_ctx(|_| (crate::ErrorKind::Filesystem, p.display().to_string()))?;
    LinesStream::new(tokio::io::AsyncBufReadExt::lines(
        tokio::io::BufReader::new(f),
    ))
    .map(|a| a.map_err(From::from).and_then(|a| a.parse()))
    .try_collect()
    .await
}

pub async fn stats(id: &str) -> Result<serde_yaml::Value, Error> {
    let p = PersistencePath::from_ref("stats").join(id).tmp();
    if let Some(parent) = p.parent() {
        if !parent.exists() {
            tokio::fs::create_dir_all(parent).await?;
        }
    }
    match tokio::fs::copy(
        Path::new(crate::VOLUMES)
            .join(id)
            .join("start9")
            .join("stats.yaml"),
        &p,
    )
    .await
    {
        Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(serde_yaml::Value::Null)
        }
        a => a,
    }?;
    let f = tokio::fs::File::open(&p)
        .await
        .with_ctx(|e| (crate::ErrorKind::Filesystem, p.display().to_string()))?;
    crate::util::from_yaml_async_reader(f).await
}
