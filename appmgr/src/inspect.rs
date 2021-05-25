use std::path::Path;

use futures::stream::StreamExt;
use tokio_tar as tar;

use crate::config::{ConfigRuleEntry, ConfigSpec};
use crate::manifest::{Manifest, ManifestLatest};
use crate::util::from_cbor_async_reader;
use crate::version::VersionT;
use crate::{Error, ResultExt as _};

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct AppInfoFull {
    #[serde(flatten)]
    pub info: AppInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manifest: Option<ManifestLatest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<AppConfig>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct AppInfo {
    pub title: String,
    pub version: emver::Version,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct AppConfig {
    pub spec: ConfigSpec,
    pub rules: Vec<ConfigRuleEntry>,
}

#[command(
    about = "Inspects a package",
    subcommands(info_full, print_instructions)
)]
pub async fn disks<T>(#[context] ctx: T) -> Result<T, RpcError> {
    Ok(ctx)
}

#[command(about = "Prints information about a package", rename = "info")]
pub async fn info_full<P: AsRef<Path>>(
    #[arg(help = "Path to the s9pk file to inspect")] path: P,
    #[arg(rename = "include-manifest", short = "m", long = "include-manifest")] with_manifest: bool,
    #[arg(rename = "include-config", short = "c", long = "include-config")] with_config: bool,
) -> Result<AppInfoFull, Error> {
    let p = path.as_ref();
    log::info!("Opening file.");
    let r = tokio::fs::File::open(p)
        .await
        .with_ctx(|_| (crate::ErrorKind::Filesystem, p.display().to_string()))?;
    log::info!("Extracting archive.");
    let mut pkg = tar::Archive::new(r);
    let mut entries = pkg.entries()?;
    log::info!("Opening manifest from archive.");
    let manifest = entries
        .next()
        .await
        .ok_or(crate::install::Error::CorruptedPkgFile("missing manifest"))??;
    crate::ensure_code!(
        manifest.path()?.to_str() == Some("manifest.cbor"),
        crate::ErrorKind::ParseS9pk,
        "Package File Invalid or Corrupted"
    );
    log::trace!("Deserializing manifest.");
    let manifest: Manifest = from_cbor_async_reader(manifest).await?;
    let manifest = manifest.into_latest()?;
    crate::ensure_code!(
        crate::version::Current::new()
            .semver()
            .satisfies(&manifest.os_version_required),
        crate::ErrorKind::VersionIncompatible,
        "AppMgr Version Not Compatible: needs {}",
        manifest.os_version_required
    );
    Ok(AppInfoFull {
        info: AppInfo {
            title: manifest.title.clone(),
            version: manifest.version.clone(),
        },
        manifest: if with_manifest { Some(manifest) } else { None },
        config: if with_config {
            log::info!("Opening config spec from archive.");
            let spec = entries
                .next()
                .await
                .ok_or(crate::install::Error::CorruptedPkgFile(
                    "missing config spec",
                ))??;
            crate::ensure_code!(
                spec.path()?.to_str() == Some("config_spec.cbor"),
                crate::ErrorKind::ParseS9pk,
                "Package File Invalid or Corrupted"
            );
            log::trace!("Deserializing config spec.");
            let spec = from_cbor_async_reader(spec).await?;
            log::info!("Opening config rules from archive.");
            let rules = entries
                .next()
                .await
                .ok_or(crate::install::Error::CorruptedPkgFile(
                    "missing config rules",
                ))??;
            crate::ensure_code!(
                rules.path()?.to_str() == Some("config_rules.cbor"),
                crate::ErrorKind::ParseS9pk,
                "Package File Invalid or Corrupted"
            );
            log::trace!("Deserializing config rules.");
            let rules = from_cbor_async_reader(rules).await?;
            Some(AppConfig { spec, rules })
        } else {
            None
        },
    })
}

#[command(
    help = "Prints instructions for an installed package",
    rename = "instructions"
)]
pub async fn print_instructions<P: AsRef<Path>>(
    #[arg(help = "Path to the s9pk file to inspect")] path: P,
) -> Result<(), Error> {
    let p = path.as_ref();
    log::info!("Opening file.");
    let r = tokio::fs::File::open(p)
        .await
        .with_ctx(|_| (crate::ErrorKind::Filesystem, p.display().to_string()))?;
    log::info!("Extracting archive.");
    let mut pkg = tar::Archive::new(r);
    let mut entries = pkg.entries()?;
    log::info!("Opening manifest from archive.");
    let manifest = entries
        .next()
        .await
        .ok_or(crate::install::Error::CorruptedPkgFile("missing manifest"))??;
    crate::ensure_code!(
        manifest.path()?.to_str() == Some("manifest.cbor"),
        crate::ErrorKind::ParseS9pk,
        "Package File Invalid or Corrupted"
    );
    log::trace!("Deserializing manifest.");
    let manifest: Manifest = from_cbor_async_reader(manifest).await?;
    let manifest = manifest.into_latest()?;
    crate::ensure_code!(
        crate::version::Current::new()
            .semver()
            .satisfies(&manifest.os_version_required),
        crate::ErrorKind::VersionIncompatible,
        "AppMgr Version Not Compatible: needs {}",
        manifest.os_version_required
    );
    entries
        .next()
        .await
        .ok_or(crate::install::Error::CorruptedPkgFile(
            "missing config spec",
        ))??;
    entries
        .next()
        .await
        .ok_or(crate::install::Error::CorruptedPkgFile(
            "missing config rules",
        ))??;

    if manifest.has_instructions {
        use tokio::io::AsyncWriteExt;

        let mut instructions =
            entries
                .next()
                .await
                .ok_or(crate::install::Error::CorruptedPkgFile(
                    "missing instructions",
                ))??;

        let mut stdout = tokio::io::stdout();
        tokio::io::copy(&mut instructions, &mut stdout)
            .await
            .with_kind(crate::ErrorKind::Filesystem)?;
        stdout
            .flush()
            .await
            .with_kind(crate::ErrorKind::Filesystem)?;
        stdout
            .shutdown()
            .await
            .with_kind(crate::ErrorKind::Filesystem)?;
    } else {
        return Err(anyhow::anyhow!("No instructions for {}", p.display()))
            .with_kind(crate::ErrorKind::NotFound);
    }

    Ok(())
}
