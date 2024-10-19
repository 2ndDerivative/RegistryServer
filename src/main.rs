use std::{net::{IpAddr, SocketAddr}, path::PathBuf, sync::Arc};

use axum::Router;
use tokio::{net::TcpListener, sync::Mutex};

mod middleware;

const IP_ENV_VARIABLE: &str = "REGISTRY_SERVER_IP";
const PORT_ENV_VARIABLE: &str = "REGISTRY_SERVER_PORT";
const REPOSITORY_ENV_VARIABLE: &str = "REGISTRY_SERVER_REPOSITORY_PATH";

#[derive(Clone, Debug)]
struct ServerState {
    _git_repository_path: Arc<Mutex<PathBuf>>
}

#[tokio::main]
async fn main() {
    let ip_from_env: IpAddr = std::env::var(IP_ENV_VARIABLE).unwrap().parse().unwrap();
    let port_from_env: u16 = std::env::var(PORT_ENV_VARIABLE).unwrap().parse().unwrap();
    let tcp_connector = TcpListener::bind(SocketAddr::from((ip_from_env, port_from_env))).await.unwrap();
    let git_repository_path = PathBuf::from(std::env::var(REPOSITORY_ENV_VARIABLE).unwrap());
    let state = ServerState { _git_repository_path: Arc::new(Mutex::new(git_repository_path)) };
    let router: Router = Router::new()
        .layer(axum::middleware::from_fn(middleware::convert_errors_to_json))
        .with_state(state);
    axum::serve(tcp_connector, router).await.unwrap()
}
