use std::path::PathBuf;

use semver::{BuildMetadata, Version};
use tokio::{fs::{create_dir_all, OpenOptions}, io::{AsyncReadExt, AsyncWriteExt}};

use crate::crate_name::CrateName;

const CRATE_BASE_FILE_PATH: &str = "./target/test_filesystem/download_files/";

fn crate_directory_path(crate_name: &CrateName) -> PathBuf {
    PathBuf::from(CRATE_BASE_FILE_PATH)
        .join(crate_name.normalized())
}
fn crate_file_path(crate_name: &CrateName, Version { major, minor, patch, pre, .. }: Version) -> PathBuf {
    let version_no_build = Version { major, minor, patch, pre, build: BuildMetadata::EMPTY };
    crate_directory_path(crate_name)
        .join(version_no_build.to_string())
}

pub async fn create_crate_file(file_content: &[u8], version: Version, crate_name: &CrateName) -> Result<(), std::io::Error> {
    create_dir_all(&crate_directory_path(crate_name)).await?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(crate_file_path(crate_name, version)).await?;
    file.write_all(file_content).await
}
pub async fn get_crate_file(version: Version, crate_name: &CrateName) -> Result<Vec<u8>, std::io::Error> {
    let mut buf = Vec::new();
    OpenOptions::new()
        .read(true)
        .open(crate_file_path(crate_name, version))
        .await?
        .read_to_end(&mut buf).await?;
    Ok(buf)
}
