use std::path::PathBuf;

use emver::Version;
use hashlink::LinkedHashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::{Config, ConfigSpec};
use crate::id::{ImageId, VolumeId};
use crate::s9pk::manifest::PackageId;
use crate::Error;

#[derive(Debug, Deserialize, Serialize)]
pub enum Volumes {} // TODO

#[derive(Debug, Deserialize, Serialize)]
pub struct Action {
    pub implementation: ActionImplementation,
    pub volumes: Volumes,
    pub input_spec: Option<ConfigSpec>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "kebab-case")]
#[serde(tag = "type")]
pub enum ActionImplementation {
    Docker(DockerAction),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "kebab-case")]
pub enum DockerIOFormat {
    Json,
    Yaml,
    Cbor,
    Toml,
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
    pub async fn execute(
        &self,
        pkg_id: &PackageId,
        pkg_version: &Version,
        input: Option<Config>,
    ) -> Result<Result<Value, (i32, Value)>, Error> {
        let mut cmd = tokio::process::Command::new("docker");
        cmd.arg("run")
            .arg("--rm")
            .args(self.mount_args())
            .arg("--entrypoint")
            .arg(&self.entrypoint)
            .arg(self.image.for_package(pkg_id, pkg_version));

        todo!()
    }

    pub fn mount_args(&self) -> Vec<String> {
        todo!()
    }
}
