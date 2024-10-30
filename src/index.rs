use std::{fmt::Display, path::{Path, PathBuf}};

use tokio::{fs::{create_dir_all, OpenOptions}, io::AsyncWriteExt, process::Command};

use crate::{publish::Metadata, read_only_mutex::ReadOnlyMutex};
use json::{build_version_metadata, VersionMetadata};
mod json;

pub async fn add_file_to_index(crate_metadata: &Metadata, file_content: &[u8], repository: &ReadOnlyMutex<PathBuf>) -> Result<(), AddToIndexError> {
    let version_metadata = build_version_metadata(crate_metadata, file_content);
    let repository = repository.lock().await;
    add_version_to_index_file(&version_metadata, &repository).await?;
    let commit_message = format!("ADD CRATE: [{}] version: {}", version_metadata.name.original_str(), version_metadata.vers);
    commit_to_index(&repository, &index_file_path(&version_metadata, &repository), &commit_message).await.unwrap();
    Ok(())
}
#[derive(Debug)]
pub enum AddToIndexError {
    CreateDirectoryInIndex(std::io::Error),
    OpenIndexFile(std::io::Error),
    SerializeJson(serde_json::Error),
    WriteIndexFile(std::io::Error),
    GitReset(std::io::Error),
    CanonicalizeFilePath(std::io::Error),
    GitAdd(std::io::Error),
    GitCommit(std::io::Error),
}
impl std::error::Error for AddToIndexError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::OpenIndexFile(io) | Self::WriteIndexFile(io) | Self::GitReset(io)
            | Self::CanonicalizeFilePath(io) | Self::GitAdd(io) | Self::GitCommit(io)
            | Self::CreateDirectoryInIndex(io) => Some(io),
            Self::SerializeJson(json) => Some(json),
        }
    }
}
impl Display for AddToIndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CreateDirectoryInIndex(io) => write!(f, "failed to create directory in index: {io}"),
            Self::OpenIndexFile(io) => write!(f, "failed to open index file: {io}"),
            Self::SerializeJson(json) => write!(f, "failed to serialize json: {json}"),
            Self::WriteIndexFile(io) => write!(f, "failed to write to index file: {io}"),
            Self::GitReset(io) => write!(f, "failed to run \"git reset\": {io}"),
            Self::CanonicalizeFilePath(io) => write!(f, "failed to canonicalize file path: {io}"),
            Self::GitAdd(ga) => write!(f, "failed to run \"git add\": {ga}"),
            Self::GitCommit(commit) => write!(f, "failed to commit to index: {commit}"),
        }
    }
}

fn index_file_path(index: &VersionMetadata, repository_path: &Path) -> PathBuf {
    let name = index.name.original_str();
    let mut chars = name.chars();
    let first_letter = chars.next().unwrap();
    let Some(second_letter) = chars.next() else {
        return repository_path.join("1").join(name);
    };
    let Some(third_letter) = chars.next() else {
        return repository_path.join("2").join(name);
    };
    let Some(fourth_letter) = chars.next() else {
        return repository_path.join("3").join(first_letter.to_string()).join(name);
    };
    repository_path
        .join(format!{"{first_letter}{second_letter}"})
        .join(format!("{third_letter}{fourth_letter}"))
        .join(name)
}

async fn add_version_to_index_file(index: &VersionMetadata, repository_path: &Path) -> Result<(), AddToIndexError> {
    let index_file_path = index_file_path(index, repository_path);
    create_dir_all(index_file_path.parent().expect("an index file path shouldn't be parentless"))
        .await
        .map_err(AddToIndexError::CreateDirectoryInIndex)?;
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(index_file_path)
        .await
        .map_err(AddToIndexError::OpenIndexFile)?;
    let json = serde_json::to_string(&index)
        .map_err(AddToIndexError::SerializeJson)?;
    file.write_all(json.as_bytes()).await.map_err(AddToIndexError::WriteIndexFile)?;
    file.write_all(b"\n").await.map_err(AddToIndexError::WriteIndexFile)?;
    Ok(())
}

async fn commit_to_index(repository_path: &Path, file_path: &Path, commit_message: &str) -> Result<(), AddToIndexError> {
    Command::new("git")
        .arg("reset")
        .arg("-q")
        .arg("HEAD")
        .current_dir(repository_path)
        .status()
        .await
        .map_err(AddToIndexError::GitReset)?;
    Command::new("git")
        .arg("add")
        .arg(
            file_path
            .canonicalize()
            .map_err(AddToIndexError::CanonicalizeFilePath)?
        )
        .current_dir(repository_path)
        .status()
        .await
        .map_err(AddToIndexError::GitAdd)?;
    Command::new("git")
        .arg("commit")
        .arg("--no-gpg-sign")
        .arg("-m")
        .arg(commit_message)
        .current_dir(repository_path)
        .status()
        .await
        .map_err(AddToIndexError::GitCommit)?;
    Ok(())
}
