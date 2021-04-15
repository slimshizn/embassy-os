use emver::Version;
use serde::{Deserialize, Serialize};

use super::id::PackageId;
use crate::dependencies::Dependencies;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct Manifest {
    pub id: PackageId,
    pub version: Version,
    pub title: String,
    pub release_notes: String,
    #[serde(default)]
    pub alerts: Alerts,
    // #[serde(default = "current_version")]
    pub min_os_version: Version,
    // #[serde(default)]
    pub interfaces: Interfaces,
    // #[serde(default)]
    pub backup: BackupStrategy,
    // #[serde(default)]
    pub volumes: VolumeConfig,
    // #[serde(default)]
    pub actions: Actions,
    // #[serde(default)]
    pub permissions: Permissions,
    // #[serde(default)]
    pub dependencies: Dependencies,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Interfaces {} // TODO
#[derive(Debug, Deserialize, Serialize)]
pub enum BackupStrategy {} // TODO
#[derive(Debug, Deserialize, Serialize)]
pub enum VolumeConfig {} // TODO
#[derive(Debug, Deserialize, Serialize)]
pub enum Actions {} // TODO
#[derive(Debug, Deserialize, Serialize)]
pub enum Permissions {} // TODO

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct Alerts {
    pub install: Option<String>,
    pub uninstall: Option<String>,
    pub restore: Option<String>,
    pub start: Option<String>,
}
