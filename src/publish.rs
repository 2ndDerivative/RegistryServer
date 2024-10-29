use std::{
    collections::{BTreeMap, HashSet},
    fmt::{Display, Formatter, Result as FmtResult},
};

use axum::{
    body::{to_bytes, Body},
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};

use crate::{
    crate_file::create_crate_file,
    crate_name::CrateName,
    feature_name::FeatureName,
    non_empty_strings::{Description, Keyword},
    postgres::{
        add_crate, add_keywords, crate_exists_or_normalized, delete_category_entries,
        delete_keywords, get_bad_categories, insert_categories, CrateExists,
    },
    ServerState,
};

pub async fn publish_handler(
    State(ServerState {
        database_connection_pool,
        ..
    }): State<ServerState>,
    body: Body,
) -> Result<Json<PublishWarnings>, Response> {
    let mut other_warnings = Vec::new();
    let body_bytes = to_bytes(body, usize::MAX)
        .await
        .map_err(|_| (StatusCode::PAYLOAD_TOO_LARGE, "payload too large").into_response())?;
    let (metadata, file_content) =
        extract_request_body(&body_bytes).map_err(IntoResponse::into_response)?;
    let publish_kind = match crate_exists_or_normalized(&metadata.name, &database_connection_pool)
        .await
        .inspect_err(|e| eprintln!("Failed to check if crate exists: {e}"))
        .map_err(|_e| internal_server_error("couldn't check if crate exists"))?
    {
        CrateExists::NoButNormalized => {
            return Err((
                StatusCode::BAD_REQUEST,
                "Crate exists under different -_ usage or capitalization",
            )
                .into_response())
        }
        // Add crate to database, assign new owner
        CrateExists::No => PublishKind::NewCrate,
        // Check if person is owner, if newer version update crate data
        // TODO Check if it's a newer version
        CrateExists::Yes => PublishKind::NewVersionForExistingCrate,
    };
    let mut transaction = database_connection_pool
        .begin()
        .await
        .map_err(|_e| internal_server_error("couldn't start transaction"))?;
    let mut invalid_categories = Vec::new();
    match publish_kind {
        // Clean adding of new crate possible
        PublishKind::NewCrate => {
            add_crate(&metadata, &mut *transaction)
                .await
                .map_err(|_e| internal_server_error("adding crate to db failed"))?;
            invalid_categories
                .extend(add_keywords_and_categories(&metadata, &mut transaction).await?);
        }
        // Old categories need to be deleted before
        PublishKind::NewVersionForExistingCrate => {
            delete_keywords(&metadata.name, &mut transaction)
                .await
                .inspect_err(|e| eprintln!("Deleting keywords failed: {e}"))
                .map_err(|_e| internal_server_error("removing old keywords failed"))?;
            delete_category_entries(&metadata.name, &mut transaction)
                .await
                .inspect_err(|e| eprintln!("Deleting category entries failed: {e}"))
                .map_err(|_e| internal_server_error("removing old categories failed"))?;
            invalid_categories
                .extend(add_keywords_and_categories(&metadata, &mut transaction).await?);
        }
        // Categories and keywords are ignored
        PublishKind::OldVersionForExistingCrate => {
            other_warnings.push(String::from("Newer version for this crate is already in the registry. Categories and keywords will not be overwritten."));
        }
    };
    eprintln!("Invalid categories: {invalid_categories:#?}");
    create_crate_file(file_content, metadata.vers, &metadata.name)
        .await
        .map_err(|e| internal_server_error(e.to_string()))?;
    transaction
        .commit()
        .await
        .map_err(|_e| internal_server_error("committing to database failed"))?;
    Err((StatusCode::SERVICE_UNAVAILABLE, "still building").into_response())
}

async fn add_keywords_and_categories(
    metadata: &Metadata,
    transaction: &mut Transaction<'_, Postgres>,
) -> Result<HashSet<String>, Response> {
    let invalid_categories = get_bad_categories(metadata, transaction)
        .await
        .map_err(|_e| internal_server_error("Failed to check categories"))?;
    insert_categories(
        metadata
            .categories
            .difference(&invalid_categories)
            .cloned()
            .collect(),
        &metadata.name,
        transaction,
    )
    .await
    .map_err(|_e| internal_server_error("Failed to insert categories"))?;
    add_keywords(metadata, transaction)
        .await
        .inspect_err(|e| eprintln!("Couldn't insert keywords: {e}"))
        .map_err(|_e| internal_server_error("Couldn't add keywords"))?;
    Ok(invalid_categories)
}

fn internal_server_error(s: impl Into<String>) -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, s.into()).into_response()
}

#[derive(Debug, Serialize)]
pub struct SuccessfulPublish {
    warnings: PublishWarnings,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct PublishWarnings {
    invalid_categories: Vec<String>,
    invalid_badges: Vec<String>,
    other: Vec<String>,
}

#[derive(Clone, Copy, Debug)]
#[allow(clippy::enum_variant_names)]
enum PublishKind {
    NewCrate,
    NewVersionForExistingCrate,
    OldVersionForExistingCrate,
}

fn extract_request_body(bytes: &[u8]) -> Result<(Metadata, &[u8]), BodyError> {
    let (metadata_length_bytes, rest) = bytes
        .split_first_chunk::<4>()
        .ok_or(BodyError::UnexpectedEOF)?;
    let metadata_length = u32::from_le_bytes(*metadata_length_bytes) as usize;
    let (metadata_bytes, request_body_rest) = rest
        .split_at_checked(metadata_length)
        .ok_or(BodyError::UnexpectedEOF)?;
    let (file_length_bytes, file_content) = request_body_rest
        .split_first_chunk::<4>()
        .ok_or(BodyError::UnexpectedEOF)?;
    if (u32::from_le_bytes(*file_length_bytes) as usize) < file_content.len() {
        return Err(BodyError::UnexpectedEOF);
    }
    let metadata =
        serde_json::from_slice::<Metadata>(metadata_bytes).map_err(BodyError::InvalidMetadata)?;
    Ok((metadata, file_content))
}

#[derive(Debug)]
pub enum BodyError {
    UnexpectedEOF,
    InvalidMetadata(serde_json::Error),
}
impl BodyError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::UnexpectedEOF | Self::InvalidMetadata(_) => StatusCode::BAD_REQUEST,
        }
    }
}
impl IntoResponse for BodyError {
    fn into_response(self) -> axum::response::Response {
        (self.status_code(), self.to_string()).into_response()
    }
}
impl std::error::Error for BodyError {}
impl Display for BodyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::UnexpectedEOF => f.write_str("Unexpected end of data stream."),
            Self::InvalidMetadata(e) => write!(f, "Invalid metadata: {e}"),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Metadata {
    pub(crate) name: CrateName,
    pub(crate) vers: Version,
    pub(crate) deps: Vec<DependencyMetadata>,
    pub(crate) features: BTreeMap<FeatureName, Vec<String>>,
    pub(crate) authors: Vec<String>,
    /// This implementation doesn't accept empty descriptions
    pub(crate) description: Description,
    pub(crate) documentation: Option<String>,
    pub(crate) homepage: Option<String>,
    pub(crate) readme: Option<String>,
    pub(crate) readme_file: Option<String>,
    /// Free user-controlled strings, should maybe be restricted to be non-empty
    pub(crate) keywords: HashSet<Keyword>,
    /// Categories the server may choose. should probably be matched to a database or sth
    pub(crate) categories: HashSet<String>,
    /// NAME of the license
    pub(crate) license: Option<String>,
    /// FILE WITH CONTENT of the license
    pub(crate) license_file: Option<String>,
    pub(crate) repository: Option<String>,
    pub(crate) badges: BTreeMap<String, BTreeMap<String, String>>,
    pub(crate) links: Option<String>,
    pub(crate) rust_version: Option<String>,
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
