use rpc_toolkit::hyper::StatusCode;
use rpc_toolkit::rpc_server;

fn status_fn(code: i32) -> StatusCode {
    match code {
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

#[tokio::main]
async fn main() {
    // let seed =
    // let server = rpc_server!(embassy::main_api, seed);
}
