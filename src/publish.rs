use std::{collections::BTreeMap, fmt::{Display, Formatter, Result as FmtResult}};

use axum::{body::{to_bytes, Body}, extract::State, http::StatusCode, response::{IntoResponse, Response}};
use semver::{Version, VersionReq};
use serde::Deserialize;

use crate::{crate_file::create_crate_file, crate_name::CrateName, feature_name::FeatureName, non_empty_strings::{Description, Keyword}, postgres::{crate_exists_or_normalized, CrateExists}, ServerState};

pub async fn publish_handler(
    State(ServerState { database_connection_pool, ..}): State<ServerState>,
    body: Body
) -> Result<String, Response> {
    let body_bytes = to_bytes(body, usize::MAX)
        .await
        .map_err(|_| StatusCode::PAYLOAD_TOO_LARGE.into_response())?;
    let (metadata, file_content) = extract_request_body(&body_bytes).map_err(IntoResponse::into_response)?;
    let publish_kind = match dbg!(crate_exists_or_normalized(&metadata.name, &database_connection_pool)
        .await
        .map_err(|_e| StatusCode::INTERNAL_SERVER_ERROR.into_response())?) {
        CrateExists::NoButNormalized => return Err((StatusCode::BAD_REQUEST, "Crate exists under different -_ usage or capitalization").into_response()),
        // Add crate to database, assign new owner
        CrateExists::No => PublishKind::NewCrate,
        // Check if person is owner, if newer version update crate data
        CrateExists::Yes => PublishKind::NewVersionForExistingCrate,
    };
    eprintln!("{metadata:#?}");
    create_crate_file(file_content, metadata.vers, &metadata.name)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response())?;
    Err((StatusCode::SERVICE_UNAVAILABLE, "still building").into_response())
}

#[derive(Clone, Copy, Debug)]
enum PublishKind {
    NewCrate,
    NewVersionForExistingCrate,
}

fn extract_request_body(bytes: &[u8]) -> Result<(Metadata, &[u8]), PublishError> {
    let (metadata_length_bytes, rest) = bytes
        .split_first_chunk::<4>()
        .ok_or(PublishError::UnexpectedEOF)?;
    let metadata_length = u32::from_le_bytes(*metadata_length_bytes) as usize;
    let (metadata_bytes, request_body_rest) = rest
        .split_at_checked(metadata_length)
        .ok_or(PublishError::UnexpectedEOF)?;
    let (file_length_bytes, file_content) = request_body_rest
        .split_first_chunk::<4>()
        .ok_or(PublishError::UnexpectedEOF)?;
    if (u32::from_le_bytes(*file_length_bytes) as usize) < file_content.len() {
        return Err(PublishError::UnexpectedEOF)
    }
    let metadata = serde_json::from_slice::<Metadata>(metadata_bytes)
        .map_err(PublishError::InvalidMetadata)?;
    Ok((metadata, file_content))
}

#[derive(Debug)]
pub enum PublishError {
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

#[derive(Debug, Deserialize)]
pub struct Metadata {
    name: CrateName,
    vers: Version,
    deps: Vec<DependencyMetadata>,
    features: BTreeMap<FeatureName, Vec<String>>,
    authors: Vec<String>,
    /// This implementation doesn't accept empty descriptions
    description: Description,
    documentation: Option<String>,
    homepage: Option<String>,
    readme: Option<String>,
    readme_file: Option<String>,
    /// Free user-controlled strings, should maybe be restricted to be non-empty
    keywords: Vec<Keyword>,
    /// Categories the server may choose. should probably be matched to a database or sth
    categories: Vec<String>,
    /// NAME of the license
    license: Option<String>,
    /// FILE WITH CONTENT of the license
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
