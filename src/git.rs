use std::{ops::Deref, path::{Path, PathBuf}};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
/// Wrapper type for Path to use in a lock for the git repository
pub struct ReadOnlyPath(PathBuf);

impl ReadOnlyPath {
    pub fn new(path: PathBuf) -> Self {
        Self(path)
    }
}

impl Deref for ReadOnlyPath {
    type Target = Path;
    fn deref(&self) -> &Self::Target {
        self.0.as_path()
    }
}
