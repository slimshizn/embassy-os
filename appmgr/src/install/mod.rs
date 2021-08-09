use std::fmt::Display;
use std::io::SeekFrom;
use std::path::Path;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use anyhow::anyhow;
use emver::VersionRange;
use futures::TryStreamExt;
use http::HeaderMap;
use indexmap::{IndexMap, IndexSet};
use patch_db::json_ptr::JsonPointer;
use patch_db::{
    DbHandle, HasModel, MapModel, Model, ModelData, OptionModel, PatchDbHandle, Revision,
};
use reqwest::Response;
use rpc_toolkit::command;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncRead, AsyncSeek, AsyncSeekExt, AsyncWrite, AsyncWriteExt};

use self::progress::{InstallProgress, InstallProgressTracker};
use crate::context::{EitherContext, ExtendedContext, RpcContext};
use crate::db::model::{
    CurrentDependencyInfo, InstalledPackageDataEntry, PackageDataEntry, StaticFiles,
};
use crate::s9pk::manifest::{Manifest, PackageId};
use crate::s9pk::reader::S9pkReader;
use crate::status::{DependencyErrors, MainStatus, Status};
use crate::util::{display_none, AsyncFileExt, Version};
use crate::{Error, ResultExt};

pub mod progress;

pub const PKG_CACHE: &'static str = "/mnt/embassy-os/cache/packages";
pub const PKG_PUBLIC_DIR: &'static str = "/mnt/embassy-os/public/package-data";

#[command(display(display_none))]
pub async fn install(#[context] ctx: EitherContext, #[arg] id: String) -> Result<(), Error> {
    let rpc_ctx = ctx.to_rpc().unwrap();
    let (pkg_id, version_str) = if let Some(split) = id.split_once("@") {
        split
    } else {
        (id.as_str(), "*")
    };
    let version: VersionRange = version_str.parse()?;
    let reg_url = rpc_ctx.package_registry_url().await?;
    let (man_res, s9pk) = tokio::try_join!(
        reqwest::get(format!(
            "{}/package/manifest/{}?version={}",
            reg_url, pkg_id, version
        )),
        reqwest::get(format!(
            "{}/package/{}.s9pk?version={}",
            reg_url, pkg_id, version
        ))
    )
    .with_kind(crate::ErrorKind::Registry)?;
    let man = man_res.json().await.with_kind(crate::ErrorKind::Registry)?;
    tokio::spawn(async move {
        if let Err(e) = download_install_s9pk(&rpc_ctx, &man, s9pk).await {
            log::error!("Install of {}@{} Failed: {}", man.id, man.version, e);
        }
    });

    Ok(())
}

pub async fn download_install_s9pk(
    ctx: &RpcContext,
    temp_manifest: &Manifest,
    s9pk: Response,
) -> Result<(), Error> {
    let pkg_id = &temp_manifest.id;
    let version = &temp_manifest.version;

    let pkg_cache_dir = Path::new(PKG_CACHE).join(pkg_id).join(version.as_str());
    tokio::fs::create_dir_all(&pkg_cache_dir).await?;
    let pkg_cache = AsRef::<Path>::as_ref(pkg_id).with_extension("s9pk");

    let pkg_data_entry = crate::db::DatabaseModel::new()
        .package_data()
        .idx_model(pkg_id);

    let res = (|| async {
        let progress = InstallProgress::new(s9pk.content_length());
        let static_files = StaticFiles::remote(pkg_id, version, temp_manifest.assets.icon_type());
        let mut db_handle = ctx.db.handle();
        let mut pde = pkg_data_entry.get_mut(&mut db_handle).await?;
        match pde.take() {
            Some(PackageDataEntry::Installed {
                installed,
                manifest,
                static_files,
            }) => {
                *pde = Some(PackageDataEntry::Updating {
                    install_progress: progress.clone(),
                    static_files,
                    installed,
                    manifest,
                })
            }
            None => {
                *pde = Some(PackageDataEntry::Installing {
                    install_progress: progress.clone(),
                    static_files,
                    manifest: temp_manifest.clone(),
                })
            }
            _ => {
                return Err(Error::new(
                    anyhow!("Cannot install over an app in a transient state"),
                    crate::ErrorKind::InvalidRequest,
                ))
            }
        }
        pde.save(&mut db_handle).await?;
        drop(db_handle);
        let progress_model = pkg_data_entry.and_then(|pde| pde.install_progress());

        async fn check_cache(
            pkg_id: &PackageId,
            version: &Version,
            pkg_cache: &Path,
            headers: &HeaderMap,
            progress: &Arc<InstallProgress>,
            model: OptionModel<InstallProgress>,
            ctx: &RpcContext,
        ) -> Option<S9pkReader<InstallProgressTracker<File>>> {
            fn warn_ok<T, E: Display>(
                pkg_id: &PackageId,
                version: &Version,
                res: Result<T, E>,
            ) -> Option<T> {
                match res {
                    Ok(a) => Some(a),
                    Err(e) => {
                        log::warn!(
                            "Install {}@{}: Could not open cache: {}",
                            pkg_id,
                            version,
                            e
                        );
                        None
                    }
                }
            }
            let hash = headers.get("x-s9pk-hash")?;
            let file = warn_ok(pkg_id, version, File::maybe_open(&pkg_cache).await)??;
            let progress_reader = InstallProgressTracker::new(file, progress.clone());
            let rdr = warn_ok(
                pkg_id,
                version,
                progress
                    .track_read_during(model, &ctx.db, || S9pkReader::from_reader(progress_reader))
                    .await,
            )?;
            if hash.as_bytes() == rdr.hash_str().as_bytes() {
                Some(rdr)
            } else {
                None
            }
        }
        let cached = check_cache(
            pkg_id,
            version,
            &pkg_cache,
            s9pk.headers(),
            &progress,
            progress_model.clone(),
            &ctx,
        )
        .await;

        let mut s9pk_reader = if let Some(cached) = cached {
            cached
        } else {
            File::delete(&pkg_cache).await?;
            let mut dst = OpenOptions::new()
                .create(true)
                .write(true)
                .read(true)
                .open(&pkg_cache)
                .await?;

            progress
                .track_download_during(progress_model.clone(), &ctx.db, || async {
                    let mut progress_writer =
                        InstallProgressTracker::new(&mut dst, progress.clone());
                    tokio::io::copy(
                        &mut tokio_util::io::StreamReader::new(s9pk.bytes_stream().map_err(|e| {
                            std::io::Error::new(
                                if e.is_connect() {
                                    std::io::ErrorKind::ConnectionRefused
                                } else if e.is_timeout() {
                                    std::io::ErrorKind::TimedOut
                                } else {
                                    std::io::ErrorKind::Other
                                },
                                e,
                            )
                        })),
                        &mut progress_writer,
                    )
                    .await?;
                    progress.download_complete();
                    Ok(())
                })
                .await?;

            dst.seek(SeekFrom::Start(0)).await?;

            let progress_reader = InstallProgressTracker::new(dst, progress.clone());
            let rdr = progress
                .track_read_during(progress_model.clone(), &ctx.db, || {
                    S9pkReader::from_reader(progress_reader)
                })
                .await?;
            rdr
        };
        install_s9pk(&ctx, pkg_id, version, &mut s9pk_reader, progress).await?;

        Ok(())
    })()
    .await;

    if let Err(e) = res {
        let mut handle = ctx.db.handle();
        let mut broken = crate::db::DatabaseModel::new()
            .broken_packages()
            .get_mut(&mut handle)
            .await?;
        broken.push(pkg_id.clone());
        broken.save(&mut handle).await?;
        Err(e)
    } else {
        Ok(())
    }
}

pub async fn install_s9pk<R: AsyncRead + AsyncSeek + Unpin>(
    ctx: &RpcContext,
    pkg_id: &PackageId,
    version: &Version,
    rdr: &mut S9pkReader<InstallProgressTracker<R>>,
    progress: Arc<InstallProgress>,
) -> Result<(), Error> {
    rdr.validate().await?;
    rdr.validated();
    let model = crate::db::DatabaseModel::new()
        .package_data()
        .idx_model(pkg_id)
        .check(&mut ctx.db.handle())
        .await?
        .ok_or_else(|| {
            Error::new(
                anyhow!("PackageDataEntry does not exist"),
                crate::ErrorKind::Database,
            )
        })?;
    let progress_model = model.clone().install_progress();

    log::info!("Install {}@{}: Unpacking Manifest", pkg_id, version);
    let manifest = progress
        .track_read_during(progress_model.clone(), &ctx.db, || rdr.manifest())
        .await?;
    log::info!("Install {}@{}: Unpacked Manifest", pkg_id, version);

    let public_dir_path = Path::new(PKG_PUBLIC_DIR)
        .join(pkg_id)
        .join(version.as_str());
    tokio::fs::create_dir_all(&public_dir_path).await?;

    log::info!("Install {}@{}: Unpacking LICENSE.md", pkg_id, version);
    progress
        .track_read_during(progress_model.clone(), &ctx.db, || async {
            let license_path = public_dir_path.join("LICENSE.md");
            let mut dst = File::create(&license_path).await?;
            tokio::io::copy(&mut rdr.license().await?, &mut dst).await?;
            dst.sync_all().await?;
            Ok(())
        })
        .await?;
    log::info!("Install {}@{}: Unpacked LICENSE.md", pkg_id, version);

    log::info!("Install {}@{}: Unpacking INSTRUCTIONS.md", pkg_id, version);
    progress
        .track_read_during(progress_model.clone(), &ctx.db, || async {
            let instructions_path = public_dir_path.join("INSTRUCTIONS.md");
            let mut dst = File::create(&instructions_path).await?;
            tokio::io::copy(&mut rdr.instructions().await?, &mut dst).await?;
            dst.sync_all().await?;
            Ok(())
        })
        .await?;
    log::info!("Install {}@{}: Unpacked INSTRUCTIONS.md", pkg_id, version);

    let icon_path = Path::new("icon").with_extension(&manifest.assets.icon_type());
    log::info!(
        "Install {}@{}: Unpacking {}",
        pkg_id,
        version,
        icon_path.display()
    );
    progress
        .track_read_during(progress_model.clone(), &ctx.db, || async {
            let icon_path = public_dir_path.join(&icon_path);
            let mut dst = File::create(&icon_path).await?;
            tokio::io::copy(&mut rdr.icon().await?, &mut dst).await?;
            dst.sync_all().await?;
            Ok(())
        })
        .await?;
    log::info!(
        "Install {}@{}: Unpacked {}",
        pkg_id,
        version,
        icon_path.display()
    );

    log::info!("Install {}@{}: Unpacking Docker Images", pkg_id, version);
    progress
        .track_read_during(progress_model.clone(), &ctx.db, || async {
            let mut load = tokio::process::Command::new("docker")
                .arg("load")
                .stdin(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;
            let mut dst = load.stdin.take().ok_or_else(|| {
                Error::new(
                    anyhow!("Could not write to stdin of docker load"),
                    crate::ErrorKind::Docker,
                )
            })?;
            tokio::io::copy(&mut rdr.docker_images().await?, &mut dst).await?;
            dst.flush().await?;
            dst.shutdown().await?;
            drop(dst);
            let res = load.wait_with_output().await?;
            if !res.status.success() {
                Err(Error::new(
                    anyhow!(
                        "{}",
                        String::from_utf8(res.stderr)
                            .unwrap_or_else(|e| format!("Could not parse stderr: {}", e))
                    ),
                    crate::ErrorKind::Docker,
                ))
            } else {
                Ok(())
            }
        })
        .await?;
    log::info!("Install {}@{}: Unpacked Docker Images", pkg_id, version,);

    progress.unpack_complete.store(true, Ordering::SeqCst);

    progress_model.put(&mut ctx.db.handle(), &progress).await?;

    let mut handle = ctx.db.handle();
    let mut tx = handle.begin().await?;
    let mut sql_tx = ctx.secret_store.begin().await?;

    log::info!("Install {}@{}: Creating volumes", pkg_id, version);
    manifest.volumes.install(pkg_id, version).await?;
    log::info!("Install {}@{}: Created volumes", pkg_id, version);

    log::info!("Install {}@{}: Installing interfaces", pkg_id, version);
    let interface_addresses = manifest.interfaces.install(&mut sql_tx, pkg_id).await?;
    log::info!("Install {}@{}: Installed interfaces", pkg_id, version);

    log::info!("Install {}@{}: Creating manager", pkg_id, version);
    ctx.managers
        .add(
            ctx.docker.clone(),
            ctx.net_controller.clone(),
            manifest.clone(),
            manifest.interfaces.tor_keys(&mut sql_tx, pkg_id).await?,
        )
        .await?;
    log::info!("Install {}@{}: Created manager", pkg_id, version);

    let static_files = StaticFiles::local(pkg_id, version, manifest.assets.icon_type());
    let current_dependencies = manifest
        .dependencies
        .0
        .iter()
        .filter_map(|(id, info)| {
            if info.optional.is_none() {
                Some((id.clone(), CurrentDependencyInfo::default()))
            } else {
                None
            }
        })
        .collect();
    let installed = InstalledPackageDataEntry {
        status: Status {
            configured: manifest.config.is_none(),
            main: MainStatus::Stopped,
            dependency_errors: DependencyErrors::init(&mut tx, &manifest, &current_dependencies)
                .await?,
        },
        manifest: manifest.clone(),
        system_pointers: Vec::new(),
        current_dependents: {
            // search required dependencies
            let mut deps = IndexMap::new();
            for package in crate::db::DatabaseModel::new()
                .package_data()
                .keys(&mut tx, true)
                .await?
            {
                if let Some(dep) = crate::db::DatabaseModel::new()
                    .package_data()
                    .idx_model(&package)
                    .expect(&mut tx)
                    .await?
                    .installed()
                    .and_then(|i| i.current_dependencies().idx_model(pkg_id))
                    .get(&mut tx, true)
                    .await?
                    .to_owned()
                {
                    deps.insert(package, dep);
                }
            }
            deps
        },
        current_dependencies,
        interface_addresses,
    };
    let mut pde = model.get_mut(&mut tx).await?;
    let prev = std::mem::replace(
        &mut *pde,
        PackageDataEntry::Installed {
            installed,
            manifest: manifest.clone(),
            static_files,
        },
    );
    pde.save(&mut tx).await?;
    if let PackageDataEntry::Updating {
        installed: prev,
        manifest: prev_manifest,
        ..
    } = prev
    {
        let mut configured = prev.status.configured;
        if let Some(res) = prev_manifest
            .migrations
            .to(
                version,
                pkg_id,
                &prev_manifest.version,
                &prev_manifest.volumes,
            )
            .await?
        {
            configured &= res.configured;
        }
        // cleanup(pkg_id, Some(prev)).await?;
        if let Some(res) = manifest
            .migrations
            .from(&prev_manifest.version, pkg_id, version, &manifest.volumes)
            .await?
        {
            configured &= res.configured;
        }
        if configured {
            crate::config::configure(
                &mut tx,
                &ctx.docker,
                pkg_id,
                None,
                &None,
                false,
                &mut IndexMap::new(),
                &mut IndexMap::new(),
            )
            .await?;
            todo!("set as running if viable");
        }
    }

    sql_tx.commit().await?;
    tx.commit(None).await?;

    log::info!("Install {}@{}: Complete", pkg_id, version);

    Ok(())
}