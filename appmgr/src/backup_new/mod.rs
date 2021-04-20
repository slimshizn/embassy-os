use emver::Version;
use serde::{Deserialize, Serialize};

use crate::action::ActionImplementation;
use crate::s9pk::manifest::PackageId;
use crate::volume::Volumes;
use crate::{Error, ResultExt};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BackupAction {
    Custom(ActionImplementation),
    Preset(ActionImplementation),
}
impl BackupAction {
    pub async fn execute(
        &self,
        pkg_id: &PackageId,
        pkg_version: &Version,
        volumes: &Volumes,
    ) -> Result<(), Error> {
        match self {
            BackupAction::Custom(action) => action,
            BackupAction::Preset(action) => action,
        }
        .execute(pkg_id, pkg_version, volumes, None)
        .await?
        .map_err(|e| anyhow::anyhow!("{}", e))
        .with_kind(crate::ErrorKind::Backup)?;
        Ok(())
    }

    pub async fn install(&self, pkg_id: &PackageId, pkg_version: &Version) -> Result<(), Error> {
        // docker tag start9/presets/${self.image} start9/${pkg_id}/${self.image}:${pkg_version}
        todo!()
    }
}
