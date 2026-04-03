use std::path::PathBuf;

use tokio::task::spawn_blocking;
use tracing::warn;

use crate::error::AppError;
use crate::git;
use crate::state::AppState;

pub async fn ensure_repo_ready(
    state: AppState,
    repo_name: &str,
    local_path: PathBuf,
    url: String,
) -> Result<(), AppError> {
    if !local_path.exists() {
        let config = state.config.clone();
        let clone_path = local_path.clone();
        let clone_url = url.clone();
        spawn_blocking(move || {
            git::clone::clone_repo(&clone_url, &clone_path, &config).map(|_| ())
        })
        .await
        .map_err(join_error)??;
    }

    if !state.config.git.fetch_on_request {
        return Ok(());
    }

    let fetch_key = format!("{repo_name}|{url}");
    if state.fetch_guard_cache.get(&fetch_key).await.is_some() {
        return Ok(());
    }

    state.fetch_guard_cache.insert(fetch_key, ()).await;
    let config = state.config.clone();
    let repo_name = repo_name.to_string();
    tokio::spawn(async move {
        match spawn_blocking(move || git::clone::fetch_repo(&local_path, &url, &config)).await {
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                warn!(repo = %repo_name, error = %err, "on-request background fetch failed")
            }
            Err(err) => {
                warn!(repo = %repo_name, error = %err, "on-request background fetch join failed")
            }
        }
    });

    Ok(())
}

pub fn join_error(err: tokio::task::JoinError) -> AppError {
    AppError::IoError(format!("blocking task join error: {err}"))
}
