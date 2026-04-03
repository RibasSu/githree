use std::path::PathBuf;

use axum::{Json, extract::Path, extract::State, http::StatusCode};
use chrono::Utc;
use tokio::task::spawn_blocking;
use tokio::time::{Duration, timeout};
use tracing::{info, instrument};

use crate::error::AppError;
use crate::git::{self, AddRepoRequest, RepoInfo};
use crate::state::AppState;

#[instrument(skip(state, payload), fields(repo_url = %payload.url))]
pub async fn add_repo(
    State(state): State<AppState>,
    Json(payload): Json<AddRepoRequest>,
) -> Result<Json<RepoInfo>, AppError> {
    if payload.url.trim().is_empty() {
        return Err(AppError::InvalidRequest(
            "repository URL cannot be empty".to_string(),
        ));
    }

    let name = git::derive_repo_name(&payload.url, payload.name.as_deref())?;
    let local_path = git::repo_disk_path(&state.config.repos_dir(), &name);

    clone_or_fetch_repo(
        state.clone(),
        local_path.clone(),
        payload.url.clone(),
        local_path.exists(),
    )
    .await?;

    let info = build_repo_info(name, payload.url, local_path).await?;
    let saved = state.registry.upsert(info).await?;
    state.tree_cache.invalidate_all();
    state.fetch_guard_cache.invalidate_all();
    Ok(Json(saved))
}

#[instrument(skip(state))]
pub async fn list_repos(State(state): State<AppState>) -> Result<Json<Vec<RepoInfo>>, AppError> {
    let repos = state.registry.list().await?;
    Ok(Json(repos))
}

#[instrument(skip(state), fields(repo_name = %name))]
pub async fn delete_repo(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<StatusCode, AppError> {
    let local_path = git::repo_disk_path(&state.config.repos_dir(), &name);
    state.registry.remove(&name).await?;

    if local_path.exists() {
        let path = local_path.clone();
        spawn_blocking(move || std::fs::remove_dir_all(path))
            .await
            .map_err(join_error)??;
    }
    state.tree_cache.invalidate_all();
    state.fetch_guard_cache.invalidate_all();

    Ok(StatusCode::NO_CONTENT)
}

#[instrument(skip(state), fields(repo_name = %name))]
pub async fn fetch_repo(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<RepoInfo>, AppError> {
    let existing = state.registry.get(&name).await?;
    let local_path = git::repo_disk_path(&state.config.repos_dir(), &name);

    clone_or_fetch_repo(
        state.clone(),
        local_path.clone(),
        existing.url.clone(),
        true,
    )
    .await?;

    let mut updated = build_repo_info(name, existing.url.clone(), local_path).await?;
    updated.description = existing.description.clone();
    let saved = state.registry.upsert(updated).await?;
    state.tree_cache.invalidate_all();
    state.fetch_guard_cache.invalidate_all();
    Ok(Json(saved))
}

async fn clone_or_fetch_repo(
    state: AppState,
    local_path: PathBuf,
    url: String,
    already_exists: bool,
) -> Result<(), AppError> {
    let timeout_duration = Duration::from_secs(state.config.git.clone_timeout_secs);
    let config = state.config.clone();
    let local_path_for_task = local_path.clone();
    let url_for_task = url.clone();
    let task = spawn_blocking(move || {
        if already_exists {
            git::clone::fetch_repo(&local_path_for_task, &url_for_task, &config)
        } else {
            git::clone::clone_repo(&url_for_task, &local_path_for_task, &config).map(|_| ())
        }
    });

    let outcome = timeout(timeout_duration, task).await.map_err(|_| {
        AppError::CloneError(format!(
            "git operation timed out after {}s",
            state.config.git.clone_timeout_secs
        ))
    })?;
    outcome.map_err(join_error)?
}

async fn build_repo_info(
    name: String,
    url: String,
    local_path: PathBuf,
) -> Result<RepoInfo, AppError> {
    let (default_branch, size_kb) = spawn_blocking(move || {
        let repo = git::clone::open_bare_repo(&local_path)?;
        let default_branch = git::clone::default_branch(&repo)?;
        let size_kb = git::clone::repo_size_kb(&local_path)?;
        Ok::<(String, u64), AppError>((default_branch, size_kb))
    })
    .await
    .map_err(join_error)??;

    let source = git::detect_repo_source(&url);
    info!(repo_name = %name, %source, "repository registered");

    Ok(RepoInfo {
        name,
        url,
        description: None,
        default_branch,
        last_fetched: Some(Utc::now()),
        size_kb,
        source,
    })
}

fn join_error(err: tokio::task::JoinError) -> AppError {
    AppError::IoError(format!("blocking task join error: {err}"))
}
