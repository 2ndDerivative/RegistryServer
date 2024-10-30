use std::collections::BTreeMap;

use semver::{Version, VersionReq};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{
    crate_name::CrateName,
    feature_name::FeatureName,
    publish::{self, DependencyKind, Metadata, RustVersionReq},
};

pub fn build_version_metadata(metadata: &Metadata, crate_file: &[u8]) -> VersionMetadata {
    let mut hasher = Sha256::new();
    hasher.update(crate_file);
    let hash_res = hasher.finalize();
    let cksum = format!("{hash_res:x}");
    let vers = metadata.vers.clone();
    let name = metadata.name.clone();
    let links = metadata.links.clone();
    let rust_version = metadata.rust_version.clone();
    let deps = metadata
        .deps
        .clone()
        .into_iter()
        .map(
            |publish::DependencyMetadata {
                 name,
                 version_req: req,
                 features,
                 optional,
                 default_features,
                 target,
                 kind,
                 registry,
                 explicit_name_in_toml,
             }| {
                let (name, package) = match (name, explicit_name_in_toml) {
                    (original, Some(renamed_name)) => (renamed_name, Some(original)),
                    (original, None) => (original, None),
                };
                VersionDependencyMetadata {
                    name,
                    req,
                    features,
                    optional,
                    default_features,
                    target,
                    kind,
                    registry,
                    package,
                }
            },
        )
        .collect();
    let features = metadata.features.clone();
    VersionMetadata {
        name,
        vers,
        deps,
        cksum,
        features,
        yanked: false,
        links,
        v: 2,
        features2: BTreeMap::new(),
        rust_version,
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct VersionMetadata {
    pub(crate) name: CrateName,
    pub(crate) vers: Version,
    pub(crate) deps: Vec<VersionDependencyMetadata>,
    pub(crate) cksum: String,
    pub(crate) features: BTreeMap<FeatureName, Vec<String>>,
    pub(crate) yanked: bool,
    pub(crate) links: Option<String>,
    pub(crate) v: u32,
    pub(crate) features2: BTreeMap<FeatureName, Vec<String>>,
    pub(crate) rust_version: Option<RustVersionReq>,
}

#[derive(Clone, Debug, Serialize)]
pub struct VersionDependencyMetadata {
    pub(crate) name: CrateName,
    pub(crate) req: VersionReq,
    pub(crate) features: Vec<FeatureName>,
    pub(crate) optional: bool,
    pub(crate) default_features: bool,
    pub(crate) target: Option<String>,
    pub(crate) kind: DependencyKind,
    pub(crate) registry: Option<String>,
    pub(crate) package: Option<CrateName>,
}
