use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use patch_db::PatchDb;
use rpc_toolkit::url::Host;
use rpc_toolkit::Context;
use serde::Deserialize;
use tokio::fs::File;

use crate::util::from_yaml_async_reader;
use crate::ResultExt;

#[derive(Debug, Default, Deserialize)]
pub struct RpcContextConfig {
    pub bind: Option<SocketAddr>,
    pub db: Option<PathBuf>,
}
pub struct RpcContextSeed {
    pub bind: SocketAddr,
    pub db: PatchDb,
}

#[derive(Clone)]
pub struct RpcContext(Arc<RpcContextSeed>);
impl RpcContext {
    pub async fn init() -> Result<Self, crate::Error> {
        let cfg_path = Path::new(crate::CONFIG_PATH);
        let base = if cfg_path.exists() {
            from_yaml_async_reader(
                File::open(cfg_path)
                    .await
                    .with_ctx(|_| (crate::ErrorKind::Filesystem, cfg_path.display().to_string()))?,
            )
            .await?
        } else {
            RpcContextConfig::default()
        };
        Ok(RpcContext(Arc::new(RpcContextSeed {
            bind: base.bind.unwrap_or(([127, 0, 0, 1], 5960).into()),
            db: PatchDb::open(
                base.db
                    .unwrap_or_else(|| Path::new("/mnt/embassy-os").to_owned()),
            )
            .await?,
        })))
    }
}
impl Context for RpcContext {
    fn host(&self) -> Host<&str> {
        match self.0.bind.ip() {
            IpAddr::V4(a) => Host::Ipv4(a),
            IpAddr::V6(a) => Host::Ipv6(a),
        }
    }
    fn port(&self) -> u16 {
        self.0.bind.port()
    }
}
