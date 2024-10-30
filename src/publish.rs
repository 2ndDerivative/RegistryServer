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
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Transaction};

use crate::{
    crate_file::create_crate_file,
    crate_name::CrateName,
    feature_name::FeatureName,
    index::add_file_to_index,
    non_empty_strings::{Description, Keyword},
    postgres::{
        add_crate, add_keywords, add_version, crate_exists_or_normalized, delete_category_entries,
        delete_keywords, get_bad_categories, get_versions, insert_categories, CrateExists,
    },
    ServerState,
};

pub async fn publish_handler(
    State(ServerState {
        database_connection_pool,
        git_repository_path,
    }): State<ServerState>,
    body: Body,
) -> Result<Json<SuccessfulPublish>, Response> {
    let mut other_warnings = Vec::new();
    let body_bytes = to_bytes(body, usize::MAX)
        .await
        .map_err(|_| (StatusCode::PAYLOAD_TOO_LARGE, "payload too large").into_response())?;
    let (crate_metadata, file_content) =
        extract_request_body(&body_bytes).map_err(IntoResponse::into_response)?;
    let mut transaction = database_connection_pool
        .begin()
        .await
        .map_err(|_e| internal_server_error("couldn't start transaction"))?;
    let publish_kind = match crate_exists_or_normalized(&crate_metadata.name, &mut transaction)
        .await
        .inspect_err(|e| eprintln!("Failed to check if crate exists: {e}"))
        .map_err(|_e| internal_server_error("couldn't check if crate exists"))?
    {
        CrateExists::NoButNormalized => {
            return Err(bad_request(
                "Crate exists under different -_ usage or capitalization",
            ))
        }
        // Add crate to database, assign new owner
        CrateExists::No => PublishKind::NewCrate,
        // Check if person is owner, if newer version update crate data
        // TODO Check if it's a newer version
        CrateExists::Yes => {
            let max = get_versions(&crate_metadata.name, &mut transaction)
                .await
                .map_err(|_e| internal_server_error("cannot get versions of crate"))?
                .into_iter()
                .max();
            if max.is_none_or(|max| max < crate_metadata.vers) {
                PublishKind::NewVersionForExistingCrate
            } else {
                PublishKind::OldVersionForExistingCrate
            }
        }
    };

    let mut invalid_categories = Vec::new();
    match publish_kind {
        // Clean adding of new crate possible
        PublishKind::NewCrate => {
            add_crate(&crate_metadata, &mut *transaction)
                .await
                .map_err(|_e| internal_server_error("adding crate to db failed"))?;
            invalid_categories
                .extend(add_keywords_and_categories(&crate_metadata, &mut transaction).await?);
        }
        // Old categories need to be deleted before
        PublishKind::NewVersionForExistingCrate => {
            delete_keywords(&crate_metadata.name, &mut transaction)
                .await
                .inspect_err(|e| eprintln!("Deleting keywords failed: {e}"))
                .map_err(|_e| internal_server_error("removing old keywords failed"))?;
            delete_category_entries(&crate_metadata.name, &mut transaction)
                .await
                .inspect_err(|e| eprintln!("Deleting category entries failed: {e}"))
                .map_err(|_e| internal_server_error("removing old categories failed"))?;
            invalid_categories
                .extend(add_keywords_and_categories(&crate_metadata, &mut transaction).await?);
        }
        // Categories and keywords are ignored
        PublishKind::OldVersionForExistingCrate => {
            other_warnings.push(String::from("Newer version for this crate is already in the registry. Categories and keywords will not be overwritten."));
        }
    };
    create_crate_file(
        file_content,
        crate_metadata.vers.clone(),
        &crate_metadata.name,
    )
    .await
    .map_err(|e| internal_server_error(e.to_string()))?;
    let cksum = hash_file_content(file_content);
    add_version(&crate_metadata, &cksum, &mut transaction)
        .await
        .inspect_err(|e| eprintln!("failed to add crate version to db: {e}"))
        .map_err(|_e| internal_server_error("failed to add crate version to database"))?;
    if let Err(e) = add_file_to_index(&crate_metadata, file_content, &git_repository_path).await {
        eprintln!("Failed to add file to index: {e}");
        return Err(internal_server_error("failed to add file to index"));
    };
    transaction
        .commit()
        .await
        .map_err(|_e| internal_server_error("committing to database failed"))?;
    Ok(Json(SuccessfulPublish {
        warnings: PublishWarnings {
            invalid_categories,
            invalid_badges: Vec::new(),
            other: other_warnings,
        },
    }))
}

fn hash_file_content(file: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(file);
    let hash_res = hasher.finalize();
    format!("{hash_res:x}")
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

fn bad_request(s: impl Into<String>) -> Response {
    (StatusCode::BAD_REQUEST, s.into()).into_response()
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
    eprintln!("Received metadata: {metadata:#?}");
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
    pub(crate) rust_version: Option<RustVersionReq>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct DependencyMetadata {
    pub(crate) name: CrateName,
    pub(crate) version_req: VersionReq,
    pub(crate) features: Vec<FeatureName>,
    pub(crate) optional: bool,
    pub(crate) default_features: bool,
    pub(crate) target: Option<String>,
    pub(crate) kind: DependencyKind,
    pub(crate) registry: Option<String>,
    pub(crate) explicit_name_in_toml: Option<CrateName>,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DependencyKind {
    Dev,
    Build,
    Normal,
}

#[derive(Clone, Debug, Serialize)]
/// A semver version requirement without comparators
pub struct RustVersionReq(VersionReq);
impl RustVersionReq {
    pub fn new(v: VersionReq) -> Option<Self> {
        if v.comparators.is_empty() {
            None
        } else {
            Some(Self(v))
        }
    }
}
impl<'de> Deserialize<'de> for RustVersionReq {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let vr = VersionReq::deserialize(deserializer)?;
        match Self::new(vr) {
            Some(rv) => Ok(rv),
            None => Err(serde::de::Error::custom(
                "rust version requirement can't have comparators",
            )),
        }
    }
}
impl Display for RustVersionReq {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
