use std::time::Duration;

use embassy::context::{EitherContext, RpcContext};
use embassy::util::daemon;
use embassy::{Error, ErrorKind};
use futures::TryFutureExt;
use rpc_toolkit::hyper::StatusCode;
use rpc_toolkit::rpc_server;

fn status_fn(code: i32) -> StatusCode {
    match code {
        -32700 => StatusCode::BAD_REQUEST,
        -32600 => StatusCode::BAD_REQUEST,
        -32601 => StatusCode::BAD_REQUEST,
        -32602 => StatusCode::BAD_REQUEST,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn inner_main() -> Result<(), Error> {
    let ctx = EitherContext::Rpc(RpcContext::init().await?);
    let server = rpc_server!(embassy::main_api, ctx, status_fn);
    let status_daemon = daemon(move || async move { todo!() }, Duration::from_secs(5));
    let health_daemon = daemon(move || async move { todo!() }, Duration::from_secs(5));
    futures::try_join!(
        server.map_err(|e| Error::new(e, ErrorKind::Network)),
        status_daemon
            .map_err(|e| Error::new(e.context("Status Daemon panicked!"), ErrorKind::Unknown)),
        health_daemon
            .map_err(|e| Error::new(e.context("Health Daemon panicked!"), ErrorKind::Unknown)),
    )?;
    Ok(())
}

fn main() {
    let rt = tokio::runtime::Runtime::new().expect("failed to initialize runtime");
    match rt.block_on(inner_main()) {
        Ok(_) => (),
        Err(e) => {
            drop(rt);
            eprintln!("{}", e.source);
            log::debug!("{:?}", e.source);
            drop(e.source);
            std::process::exit(e.kind as i32)
        }
    }
}
