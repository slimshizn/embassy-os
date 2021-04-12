use emver::VersionRange;
use tokio_compat_02::FutureExt;

use crate::apps::AppConfig;
use crate::manifest::ManifestLatest;
use crate::{Error, ResultExt as _};

pub async fn manifest(id: &str, version: &VersionRange) -> Result<ManifestLatest, Error> {
    let manifest: ManifestLatest = reqwest::get(&format!(
        "{}/manifest/{}?spec={}",
        &*crate::APP_REGISTRY_URL,
        id,
        version
    ))
    .compat()
    .await
    .with_kind(crate::ErrorKind::Network)?
    .error_for_status()
    .with_kind(crate::ErrorKind::Registry)?
    .json()
    .await
    .with_kind(crate::ErrorKind::Deserialization)?;
    Ok(manifest)
}

pub async fn version(id: &str, version: &VersionRange) -> Result<emver::Version, Error> {
    #[derive(serde::Deserialize)]
    struct VersionRes {
        version: emver::Version,
    }

    let version: VersionRes = reqwest::get(&format!(
        "{}/version/{}?spec={}",
        &*crate::APP_REGISTRY_URL,
        id,
        version
    ))
    .compat()
    .await
    .with_kind(crate::ErrorKind::Network)?
    .error_for_status()
    .with_kind(crate::ErrorKind::Registry)?
    .json()
    .await
    .with_kind(crate::ErrorKind::Deserialization)?;
    Ok(version.version)
}

pub async fn config(id: &str, version: &VersionRange) -> Result<AppConfig, Error> {
    let config: crate::inspect::AppConfig = reqwest::get(&format!(
        "{}/config/{}?spec={}",
        &*crate::APP_REGISTRY_URL,
        id,
        version
    ))
    .compat()
    .await
    .with_kind(crate::ErrorKind::Network)?
    .error_for_status()
    .with_kind(crate::ErrorKind::Registry)?
    .json()
    .await
    .with_kind(crate::ErrorKind::Deserialization)?;
    Ok(AppConfig {
        config: None,
        spec: config.spec,
        rules: config.rules,
    })
}
