use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use fd_lock::RwLock;
use tokio::sync::Mutex;
use tokio::task::spawn_blocking;

use crate::error::AppError;
use crate::git::RepoInfo;

#[derive(Debug)]
pub struct RepoRegistry {
    path: PathBuf,
    process_lock: Mutex<()>,
}

impl RepoRegistry {
    pub async fn new(path: PathBuf) -> Result<Arc<Self>, AppError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        if !Path::new(&path).exists() {
            fs::write(&path, "[]")?;
        }

        Ok(Arc::new(Self {
            path,
            process_lock: Mutex::new(()),
        }))
    }

    pub async fn list(&self) -> Result<Vec<RepoInfo>, AppError> {
        let _guard = self.process_lock.lock().await;
        let path = self.path.clone();
        let entries = spawn_blocking(move || read_all_sync(&path))
            .await
            .map_err(join_error)??;
        Ok(entries)
    }

    pub async fn get(&self, name: &str) -> Result<RepoInfo, AppError> {
        let repos = self.list().await?;
        repos
            .into_iter()
            .find(|repo| repo.name == name)
            .ok_or_else(|| AppError::NotFound(format!("repository '{name}' not found")))
    }

    pub async fn upsert(&self, repo_info: RepoInfo) -> Result<RepoInfo, AppError> {
        let _guard = self.process_lock.lock().await;
        let path = self.path.clone();
        spawn_blocking(move || {
            let mut repos = read_all_sync(&path)?;
            if let Some(existing) = repos.iter_mut().find(|repo| repo.name == repo_info.name) {
                *existing = repo_info.clone();
            } else {
                repos.push(repo_info.clone());
            }
            repos.sort_by(|a, b| a.name.cmp(&b.name));
            write_all_sync(&path, &repos)?;
            Ok::<RepoInfo, AppError>(repo_info)
        })
        .await
        .map_err(join_error)?
    }

    pub async fn remove(&self, name: &str) -> Result<(), AppError> {
        let _guard = self.process_lock.lock().await;
        let path = self.path.clone();
        let name = name.to_string();
        spawn_blocking(move || {
            let mut repos = read_all_sync(&path)?;
            let before = repos.len();
            repos.retain(|repo| repo.name != name);
            if before == repos.len() {
                return Err(AppError::NotFound(format!("repository '{name}' not found")));
            }
            write_all_sync(&path, &repos)?;
            Ok::<(), AppError>(())
        })
        .await
        .map_err(join_error)?
    }
}

fn read_all_sync(path: &Path) -> Result<Vec<RepoInfo>, AppError> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    let mut lock = RwLock::new(file);
    let mut guard = lock.write().map_err(|e| AppError::IoError(e.to_string()))?;
    guard.seek(SeekFrom::Start(0))?;
    let mut raw = String::new();
    guard.read_to_string(&mut raw)?;
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(&raw).map_err(AppError::from)
}

fn write_all_sync(path: &Path, repos: &[RepoInfo]) -> Result<(), AppError> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    let mut lock = RwLock::new(file);
    let mut guard = lock.write().map_err(|e| AppError::IoError(e.to_string()))?;
    guard.seek(SeekFrom::Start(0))?;
    guard.set_len(0)?;
    let serialized = serde_json::to_string_pretty(repos)?;
    guard.write_all(serialized.as_bytes())?;
    guard.flush()?;
    Ok(())
}

fn join_error(err: tokio::task::JoinError) -> AppError {
    AppError::IoError(format!("blocking task join error: {err}"))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::*;

    #[tokio::test]
    async fn new_handles_existing_file_and_empty_content() {
        let temp = tempfile::tempdir().expect("tempdir");
        let registry_path = temp.path().join("repos.json");
        fs::write(&registry_path, "").expect("write empty registry file");

        let registry = RepoRegistry::new(registry_path)
            .await
            .expect("create registry with existing file");
        let entries = registry.list().await.expect("list entries from empty file");
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn new_with_invalid_empty_path_fails() {
        let err = RepoRegistry::new(PathBuf::new())
            .await
            .expect_err("empty path should fail");
        match err {
            AppError::IoError(_) => {}
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn join_error_maps_to_io_error() {
        let panic_join_error = tokio::spawn(async { panic!("registry join panic") })
            .await
            .expect_err("must return join error");
        match super::join_error(panic_join_error) {
            AppError::IoError(message) => assert!(message.contains("blocking task join error")),
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
