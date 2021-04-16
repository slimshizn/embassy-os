use std::path::Path;

use emver::Version;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::action::Action;
use crate::dependencies::Dependencies;
use crate::id::Id;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct PackageId<S: AsRef<str> = String>(Id<S>);
impl<S: AsRef<str>> std::fmt::Display for PackageId<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.0)
    }
}
impl<S: AsRef<str>> AsRef<str> for PackageId<S> {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}
impl<S: AsRef<str>> AsRef<Path> for PackageId<S> {
    fn as_ref(&self) -> &Path {
        self.0.as_ref().as_ref()
    }
}
impl<'de, S> Deserialize<'de> for PackageId<S>
where
    S: AsRef<str>,
    Id<S>: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        Ok(PackageId(Deserialize::deserialize(deserializer)?))
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct Manifest {
    pub id: PackageId,
    pub title: String,
    pub version: Version,
    pub release_notes: String,
    pub license: String, // type of license
    pub wrapper_repo: Url,
    pub upstream_repo: Url,
    pub support_page: Option<Url>,
    pub marketing_page: Option<Url>,
    pub main: Action,
    #[serde(default)]
    pub alerts: Alerts,
    // #[serde(default = "current_version")]
    pub min_os_version: Version,
    // #[serde(default)]
    pub interfaces: Interfaces,
    // #[serde(default)]
    pub backup: BackupStrategy,
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
