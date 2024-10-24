use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    sync::Arc,
};

use axum::{
    body::{to_bytes, Body},
    http::{header::CONTENT_LENGTH, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::put,
    Router,
};
use publish_metadata::PublishMetadata;
use read_only_mutex::ReadOnlyMutex;
use tokio::net::TcpListener;

mod crate_name;
mod feature_name;
mod middleware;
mod read_only_mutex;

const IP_ENV_VARIABLE: &str = "REGISTRY_SERVER_IP";
const PORT_ENV_VARIABLE: &str = "REGISTRY_SERVER_PORT";
const REPOSITORY_ENV_VARIABLE: &str = "REGISTRY_SERVER_REPOSITORY_PATH";

#[derive(Clone, Debug)]
struct ServerState {
    _git_repository_path: Arc<ReadOnlyMutex<PathBuf>>,
}

#[tokio::main]
async fn main() {
    let ip_from_env: IpAddr = std::env::var(IP_ENV_VARIABLE).unwrap().parse().unwrap();
    let port_from_env: u16 = std::env::var(PORT_ENV_VARIABLE).unwrap().parse().unwrap();
    let tcp_connector = TcpListener::bind(SocketAddr::from((ip_from_env, port_from_env)))
        .await
        .unwrap();
    let git_repository_from_env = std::env::var(REPOSITORY_ENV_VARIABLE).unwrap();
    let state = ServerState {
        _git_repository_path: Arc::new(ReadOnlyMutex::new(git_repository_from_env.into())),
    };
    let router: Router = Router::new()
        .route("/api/v1/crates/new", put(publish_handler))
        .layer(axum::middleware::from_fn(
            middleware::convert_errors_to_json,
        ))
        .with_state(state);
    axum::serve(tcp_connector, router).await.unwrap()
}

#[derive(Debug)]
enum PublishError {
    UnexpectedEOF,
    InvalidMetadata(serde_json::Error),
}
impl PublishError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::UnexpectedEOF | Self::InvalidMetadata(_) => StatusCode::BAD_REQUEST,
        }
    }
}
impl IntoResponse for PublishError {
    fn into_response(self) -> axum::response::Response {
        (self.status_code(), self.to_string()).into_response()
    }
}
impl std::error::Error for PublishError {}
impl Display for PublishError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::UnexpectedEOF => f.write_str("Unexpected end of data stream."),
            Self::InvalidMetadata(e) => write!(f, "Invalid metadata: {e}"),
        }
    }
}

async fn publish_handler(headers: HeaderMap, body: Body) -> Result<String, Response> {
    let content_length: Option<usize> = headers
        .get(CONTENT_LENGTH)
        .map(|b| b.to_str().unwrap().parse().unwrap());
    let body_bytes = to_bytes(body, content_length.unwrap_or(usize::MAX))
        .await
        .unwrap();
    let (metadata_length_bytes, rest) = body_bytes
        .split_first_chunk::<4>()
        .ok_or(PublishError::UnexpectedEOF.into_response())?;
    let metadata_length = u32::from_le_bytes(*metadata_length_bytes) as usize;
    let (metadata_bytes, request_body_rest) = rest
        .split_at_checked(metadata_length)
        .ok_or(PublishError::UnexpectedEOF.into_response())?;
    let (file_length_bytes, file_content) = request_body_rest
        .split_first_chunk()
        .ok_or(PublishError::UnexpectedEOF.into_response())?;
    if (u32::from_le_bytes(*file_length_bytes) as usize) < file_content.len() {
        return Err(PublishError::UnexpectedEOF.into_response());
    }
    let metadata = serde_json::from_slice::<PublishMetadata>(metadata_bytes)
        .map_err(|e| PublishError::InvalidMetadata(e).into_response())?;
    eprintln!("{metadata:#?}");
    Err((StatusCode::SERVICE_UNAVAILABLE, "still building").into_response())
}

mod publish_metadata {
    use std::collections::BTreeMap;

    use semver::{Version, VersionReq};
    use serde::Deserialize;

    use crate::{crate_name::CrateName, feature_name::FeatureName};

    #[derive(Debug, Deserialize)]
    pub struct PublishMetadata {
        name: CrateName,
        vers: Version,
        deps: Vec<DependencyMetadata>,
        features: BTreeMap<FeatureName, Vec<String>>,
        authors: Vec<String>,
        description: Option<String>,
        documentation: Option<String>,
        homepage: Option<String>,
        readme: Option<String>,
        readme_file: Option<String>,
        keywords: Vec<String>,
        categories: Vec<String>,
        license: Option<String>,
        license_file: Option<String>,
        repository: Option<String>,
        badges: BTreeMap<String, BTreeMap<String, String>>,
        links: Option<String>,
        rust_version: Option<String>,
    }
    #[derive(Debug, Deserialize)]
    pub struct DependencyMetadata {
        name: CrateName,
        version_req: VersionReq,
        features: Vec<FeatureName>,
        optional: bool,
        default_features: bool,
        target: Option<String>,
        kind: DependencyKind,
        registry: Option<String>,
        explicit_name_in_toml: Option<CrateName>,
    }
    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "lowercase")]
    pub enum DependencyKind {
        Dev,
        Build,
        Normal,
    }
}
