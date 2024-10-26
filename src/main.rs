use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    sync::Arc,
};

use axum::{
    routing::put,
    Router,
};
use publish::publish_handler;
use read_only_mutex::ReadOnlyMutex;
use sqlx::{Pool, Postgres};
use tokio::net::TcpListener;

mod crate_file;
mod crate_name;
mod feature_name;
mod middleware;
mod non_empty_strings;
mod postgres;
mod publish;
mod read_only_mutex;

const IP_ENV_VARIABLE: &str = "REGISTRY_SERVER_IP";
const PORT_ENV_VARIABLE: &str = "REGISTRY_SERVER_PORT";
const REPOSITORY_ENV_VARIABLE: &str = "REGISTRY_SERVER_REPOSITORY_PATH";

#[derive(Clone, Debug)]
struct ServerState {
    _git_repository_path: Arc<ReadOnlyMutex<PathBuf>>,
    database_connection_pool: Arc<Pool<Postgres>>,
}

#[tokio::main]
async fn main() {
    let ip_from_env: IpAddr = std::env::var(IP_ENV_VARIABLE).unwrap().parse().unwrap();
    let port_from_env: u16 = std::env::var(PORT_ENV_VARIABLE).unwrap().parse().unwrap();
    let tcp_connector = TcpListener::bind(SocketAddr::from((ip_from_env, port_from_env)))
        .await
        .unwrap();
    let database_connection_pool = Arc::new(
        Pool::connect_lazy("postgres://registry_server:no_expiry@localhost:5432/cargo_registry").unwrap()
    );
    let git_repository_from_env = std::env::var(REPOSITORY_ENV_VARIABLE).unwrap();
    let state = ServerState {
        _git_repository_path: Arc::new(ReadOnlyMutex::new(git_repository_from_env.into())),
        database_connection_pool
    };
    let router: Router = Router::new()
        .route("/api/v1/crates/new", put(publish_handler))
        .layer(axum::middleware::from_fn(
            middleware::convert_errors_to_json,
        ))
        .with_state(state);
    axum::serve(tcp_connector, router).await.unwrap()
}
