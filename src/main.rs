use std::{net::{IpAddr, SocketAddr}, path::PathBuf, sync::Arc};

use axum::Router;
use read_only_mutex::ReadOnlyMutex;
use tokio::net::TcpListener;

mod read_only_mutex;
mod middleware;

const IP_ENV_VARIABLE: &str = "REGISTRY_SERVER_IP";
const PORT_ENV_VARIABLE: &str = "REGISTRY_SERVER_PORT";
const REPOSITORY_ENV_VARIABLE: &str = "REGISTRY_SERVER_REPOSITORY_PATH";

#[derive(Clone, Debug)]
struct ServerState {
    git_repository_path: Arc<ReadOnlyMutex<PathBuf>>,
}

#[tokio::main]
async fn main() {
    let ip_from_env: IpAddr = std::env::var(IP_ENV_VARIABLE).unwrap().parse().unwrap();
    let port_from_env: u16 = std::env::var(PORT_ENV_VARIABLE).unwrap().parse().unwrap();
    let tcp_connector = TcpListener::bind(SocketAddr::from((ip_from_env, port_from_env))).await.unwrap();
    let git_repository_from_env = std::env::var(REPOSITORY_ENV_VARIABLE).unwrap();
        let state = ServerState {
            git_repository_path: Arc::new(ReadOnlyMutex::new(git_repository_from_env.into())) };
    let router: Router = Router::new()
        .layer(axum::middleware::from_fn(middleware::convert_errors_to_json))
        .with_state(state);
    axum::serve(tcp_connector, router).await.unwrap()
}
