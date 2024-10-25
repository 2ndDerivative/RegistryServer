use std::collections::BTreeMap;

use semver::{Version, VersionReq};
use serde::Deserialize;

use crate::{crate_name::CrateName, non_empty_strings::{Description, Keyword}, feature_name::FeatureName};

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
