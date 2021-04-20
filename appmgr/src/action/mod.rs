use std::borrow::Cow;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

use emver::Version;
use hashlink::LinkedHashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::{Config, ConfigSpec};
use crate::id::ImageId;
use crate::s9pk::manifest::PackageId;
use crate::util::{Invoke, IpPool};
use crate::volume::{VolumeId, Volumes};
use crate::{Error, ResultExt};

#[derive(Debug, Deserialize, Serialize)]
pub struct Action {
    pub implementation: ActionImplementation,
    pub input_spec: Option<ConfigSpec>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "kebab-case")]
#[serde(tag = "type")]
pub enum ActionImplementation {
    Docker(DockerAction),
}
impl ActionImplementation {
    pub async fn execute(
        &self,
        pkg_id: &PackageId,
        pkg_version: &Version,
        volumes: &Volumes,
        input: Option<Config>,
    ) -> Result<Result<Value, String>, Error> {
        match self {
            ActionImplementation::Docker(action) => {
                action.execute(pkg_id, pkg_version, volumes, input).await
            }
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "kebab-case")]
pub enum DockerIOFormat {
    Json,
    Yaml,
    Cbor,
    Toml,
}
impl std::fmt::Display for DockerIOFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use DockerIOFormat::*;
        match self {
            Json => write!(f, "JSON"),
            Yaml => write!(f, "YAML"),
            Cbor => write!(f, "CBOR"),
            Toml => write!(f, "TOML"),
        }
    }
}
impl DockerIOFormat {
    pub fn to_vec<T: Serialize>(&self, value: &T) -> Result<Vec<u8>, Error> {
        match self {
            DockerIOFormat::Json => {
                serde_json::to_vec(value).with_kind(crate::ErrorKind::Serialization)
            }
            DockerIOFormat::Yaml => {
                serde_yaml::to_vec(value).with_kind(crate::ErrorKind::Serialization)
            }
            DockerIOFormat::Cbor => {
                serde_cbor::to_vec(value).with_kind(crate::ErrorKind::Serialization)
            }
            DockerIOFormat::Toml => {
                serde_toml::to_vec(value).with_kind(crate::ErrorKind::Serialization)
            }
        }
    }
    pub fn from_slice<T: for<'de> Deserialize<'de>>(&self, slice: &[u8]) -> Result<T, Error> {
        match self {
            DockerIOFormat::Json => {
                serde_json::from_slice(slice).with_kind(crate::ErrorKind::Deserialization)
            }
            DockerIOFormat::Yaml => {
                serde_yaml::from_slice(slice).with_kind(crate::ErrorKind::Deserialization)
            }
            DockerIOFormat::Cbor => {
                serde_cbor::from_slice(slice).with_kind(crate::ErrorKind::Deserialization)
            }
            DockerIOFormat::Toml => {
                serde_toml::from_slice(slice).with_kind(crate::ErrorKind::Deserialization)
            }
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DockerAction {
    pub image: ImageId,
    pub entrypoint: String,
    pub args: Vec<String>,
    pub mounts: LinkedHashMap<VolumeId, PathBuf>,
    pub io_format: Option<DockerIOFormat>,
    pub shm_size_mb: Option<usize>, // TODO: use postfix sizing? like 1k vs 1m vs 1g
}
impl DockerAction {
    pub async fn create(
        &self,
        pkg_id: &PackageId,
        pkg_version: &Version,
        volumes: &Volumes,
        ip_pool: &mut IpPool,
    ) -> Result<(), Error> {
        tokio::process::Command::new("docker")
            .arg("create")
            .arg("--net")
            .arg("start9")
            .arg("--name")
            .arg("--ip")
            .arg(format!(
                "{}",
                ip_pool
                    .get()
                    .ok_or_else(|| anyhow::anyhow!("No available IP addresses"))
                    .with_kind(crate::ErrorKind::Network)?,
            ))
            .arg(self.container_name(pkg_id))
            .args(self.docker_args(pkg_id, pkg_version, volumes))
            .invoke(crate::ErrorKind::Docker)
            .await?;
        Ok(())
    }

    pub async fn execute(
        &self,
        pkg_id: &PackageId,
        pkg_version: &Version,
        volumes: &Volumes,
        input: Option<Config>,
    ) -> Result<Result<Value, String>, Error> {
        let mut cmd = tokio::process::Command::new("docker");
        cmd.arg("run")
            .arg("--rm")
            .args(self.docker_args(pkg_id, pkg_version, volumes));
        let input_buf = if let (Some(input), Some(format)) = (&input, &self.io_format) {
            cmd.stdin(std::process::Stdio::piped());
            Some(format.to_vec(input)?)
        } else {
            None
        };
        let mut handle = cmd.spawn().with_kind(crate::ErrorKind::Docker)?;
        if let (Some(input), Some(stdin)) = (&input_buf, &mut handle.stdin) {
            use tokio::io::AsyncWriteExt;
            stdin
                .write_all(input)
                .await
                .with_kind(crate::ErrorKind::Docker)?;
        }
        let res = handle
            .wait_with_output()
            .await
            .with_kind(crate::ErrorKind::Docker)?;
        Ok(if res.status.success() {
            Ok(if let Some(format) = &self.io_format {
                match format.from_slice(&res.stdout) {
                    Ok(a) => a,
                    Err(e) => {
                        log::warn!(
                            "Failed to deserialize stdout from {}: {}, falling back to UTF-8 string.",
                            format,
                            e
                        );
                        String::from_utf8(res.stdout)?.into()
                    }
                }
            } else {
                String::from_utf8(res.stdout)?.into()
            })
        } else {
            Err(String::from_utf8(res.stderr)?)
        })
    }

    fn container_name(&self, pkg_id: &PackageId) -> String {
        format!("{}_{}", pkg_id, self.image)
    }

    fn docker_args<'a>(
        &'a self,
        pkg_id: &PackageId,
        pkg_version: &Version,
        volumes: &Volumes,
    ) -> Vec<Cow<'a, OsStr>> {
        let mut res = Vec::with_capacity(
            (2 * self.mounts.len()) // --mount <MOUNT_ARG>
                + (2 * self.shm_size_mb.is_some() as usize) // --shm-size <SHM_SIZE>
                + 3 // --entrypoint <ENTRYPOINT> <IMAGE>
                + self.args.len(), // [ARG...]
        );
        for (volume_id, dst) in &self.mounts {
            let src = if let Some(path) = volumes.get_path_for(pkg_id, volume_id) {
                path
            } else {
                continue;
            };
            res.push(OsStr::new("--mount").into());
            res.push(
                OsString::from(format!(
                    "type=bind,src={},dst={}",
                    src.display(),
                    dst.display()
                ))
                .into(),
            );
        }
        if let Some(shm_size_mb) = self.shm_size_mb {
            res.push(OsStr::new("--shm-size").into());
            res.push(OsString::from(format!("{}m", shm_size_mb)).into());
        }
        res.push(OsStr::new("--entrypoint").into());
        res.push(OsStr::new(&self.entrypoint).into());
        res.push(OsString::from(self.image.for_package(pkg_id, pkg_version)).into());
        res.extend(self.args.iter().map(|s| OsStr::new(s).into()));

        res
    }
}
