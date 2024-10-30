use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    sync::Arc,
};

use axum::{
    extract::Path,
    http::StatusCode,
    routing::{get, put},
    Router,
};
use crate_file::get_crate_file;
use crate_name::CrateName;
use publish::publish_handler;
use read_only_mutex::ReadOnlyMutex;
use semver::Version;
use serde::Deserialize;
use sqlx::{Pool, Postgres};
use tokio::net::TcpListener;

mod crate_file;
mod crate_name;
mod feature_name;
mod index;
mod middleware;
mod non_empty_strings;
mod postgres;
mod publish;
mod read_only_mutex;

const IP_ENV_VARIABLE: &str = "REGISTRY_SERVER_IP";
const PORT_ENV_VARIABLE: &str = "REGISTRY_SERVER_PORT";
const REPOSITORY_ENV_VARIABLE: &str = "REGISTRY_SERVER_REPOSITORY_PATH";
const POSTGRES_CONNECTION_STRING_VAR: &str = "REGISTRY_SERVER_DATABASE_URL";

#[derive(Clone, Debug)]
struct ServerState {
    git_repository_path: Arc<ReadOnlyMutex<PathBuf>>,
    database_connection_pool: Arc<Pool<Postgres>>,
}

#[tokio::main]
async fn main() {
    let ip_from_env: IpAddr = std::env::var(IP_ENV_VARIABLE).unwrap().parse().unwrap();
    let port_from_env: u16 = std::env::var(PORT_ENV_VARIABLE).unwrap().parse().unwrap();
    let database_url_from_env = std::env::var(POSTGRES_CONNECTION_STRING_VAR).unwrap();
    let tcp_connector = TcpListener::bind(SocketAddr::from((ip_from_env, port_from_env)))
        .await
        .unwrap();
    let database_connection_pool = Arc::new(Pool::connect_lazy(&database_url_from_env).unwrap());
    let git_repository_from_env = std::env::var(REPOSITORY_ENV_VARIABLE).unwrap();
    let git_repository_path = PathBuf::from(git_repository_from_env)
        .canonicalize()
        .unwrap();
    let state = ServerState {
        git_repository_path: Arc::new(ReadOnlyMutex::new(git_repository_path)),
        database_connection_pool,
    };
    let router: Router = Router::new()
        .route("/api/v1/crates/new", put(publish_handler))
        .route(
            "/api/v1/crates/:crate_name/:version/download",
            get(download_handler),
        )
        .layer(axum::middleware::from_fn(
            middleware::convert_errors_to_json,
        ))
        .with_state(state);
    axum::serve(tcp_connector, router).await.unwrap()
}

#[derive(Debug, Deserialize)]
struct DownloadPath {
    crate_name: CrateName,
    version: Version,
}

async fn download_handler(
    Path(DownloadPath {
        crate_name,
        version,
    }): Path<DownloadPath>,
) -> Result<Vec<u8>, (StatusCode, &'static str)> {
    get_crate_file(version, &crate_name)
        .await
        .map_err(|e| match e {
            e if e.kind() == std::io::ErrorKind::NotFound => {
                (StatusCode::NOT_FOUND, "crate or version doesn't exist")
            }
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "couldn't get crate file for you",
            ),
        })
}
