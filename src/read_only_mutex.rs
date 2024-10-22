use std::ops::Deref;

use tokio::sync::{Mutex, MutexGuard};

#[derive(Debug)]
/// Type of Mutex that only allows read-only access to the inner item
pub struct ReadOnlyMutex<T>(Mutex<T>);

impl<T> ReadOnlyMutex<T> {
    pub fn new(path: T) -> Self {
        Self(Mutex::new(path))
    }
    pub async fn lock(&self) -> ReadOnlyGuard<'_, T> {
        ReadOnlyGuard(self.0.lock().await)
    }
}

pub struct ReadOnlyGuard<'m, T>(MutexGuard<'m, T>);

impl<'m, T> Deref for ReadOnlyGuard<'m, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
