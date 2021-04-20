use std::path::{Path, PathBuf};

use hashlink::LinkedHashMap;
use serde::{Deserialize, Deserializer, Serialize};

use crate::id::{Id, InterfaceId};
use crate::s9pk::manifest::PackageId;

pub const APP_DATA_DIR: &'static str = "/mnt/embassy-os/app-data";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct VolumeId<S: AsRef<str> = String>(Id<S>);
impl<S: AsRef<str>> std::fmt::Display for VolumeId<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.0)
    }
}
impl<S: AsRef<str>> AsRef<str> for VolumeId<S> {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}
impl<S: AsRef<str>> AsRef<Path> for VolumeId<S> {
    fn as_ref(&self) -> &Path {
        self.0.as_ref().as_ref()
    }
}
impl<'de, S> Deserialize<'de> for VolumeId<S>
where
    S: AsRef<str>,
    Id<S>: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(VolumeId(Deserialize::deserialize(deserializer)?))
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Volumes(LinkedHashMap<VolumeId, Volume>); // TODO
impl Volumes {
    pub fn get_path_for(&self, pkg_id: &PackageId, volume_id: &VolumeId) -> Option<PathBuf> {
        self.0
            .get(volume_id)
            .map(|volume| volume.path_for(pkg_id, volume_id))
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "kebab-case")]
pub enum Volume {
    #[serde(rename_all = "kebab-case")]
    Data,
    #[serde(rename_all = "kebab-case")]
    Pointer {
        package_id: PackageId,
        volume_id: VolumeId,
        path: PathBuf,
        read_only: bool,
    },
    Certificate {
        interface_id: InterfaceId,
    },
    HiddenService {
        interface_id: InterfaceId,
    },
}
impl Volume {
    pub fn path_for(&self, pkg_id: &PackageId, volume_id: &VolumeId) -> PathBuf {
        match self {
            Volume::Data => Path::new(APP_DATA_DIR)
                .join(pkg_id)
                .join("volumes")
                .join(volume_id),
            Volume::Pointer {
                package_id,
                volume_id,
                path,
                ..
            } => Path::new(APP_DATA_DIR)
                .join(package_id)
                .join("volumes")
                .join(volume_id)
                .join(path),
            Volume::Certificate { interface_id } => Path::new(APP_DATA_DIR)
                .join(pkg_id)
                .join("certificates")
                .join(interface_id),
            Volume::HiddenService { interface_id } => Path::new(APP_DATA_DIR)
                .join(pkg_id)
                .join("hidden-services")
                .join(interface_id),
        }
    }
    pub fn read_only(&self) -> bool {
        match self {
            Volume::Data => false,
            Volume::Pointer { read_only, .. } => *read_only,
            Volume::Certificate { .. } => true,
            Volume::HiddenService { .. } => true,
        }
    }
}
