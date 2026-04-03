use std::path::PathBuf;

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::Deserialize;
use tokio::task::spawn_blocking;
use tracing::instrument;

use crate::error::AppError;
use crate::git::{self, CommitDetail, CommitInfo};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct CommitsQuery {
    #[serde(rename = "ref")]
    pub ref_name: Option<String>,
    pub path: Option<String>,
    pub skip: Option<usize>,
    pub limit: Option<usize>,
}

#[instrument(skip(state, query), fields(repo_name = %name))]
pub async fn list_commits(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(query): Query<CommitsQuery>,
) -> Result<Json<Vec<CommitInfo>>, AppError> {
    let repo = state.registry.get(&name).await?;
    let ref_name = query
        .ref_name
        .unwrap_or_else(|| repo.default_branch.clone());
    let path_filter = query.path.clone();
    let skip = query.skip.unwrap_or(0);
    let limit = query.limit.unwrap_or(30).min(200);
    let local_path = git::repo_disk_path(&state.config.repos_dir(), &name);

    maybe_fetch_repo(state.clone(), local_path.clone(), repo.url.clone()).await?;

    let commits = spawn_blocking(move || {
        git::browse::commit_history(&local_path, &ref_name, path_filter.as_deref(), skip, limit)
    })
    .await
    .map_err(join_error)??;
    Ok(Json(commits))
}

#[instrument(skip(state), fields(repo_name = %name, commit_hash = %hash))]
pub async fn get_commit_detail(
    State(state): State<AppState>,
    Path((name, hash)): Path<(String, String)>,
) -> Result<Json<CommitDetail>, AppError> {
    let repo = state.registry.get(&name).await?;
    let local_path = git::repo_disk_path(&state.config.repos_dir(), &name);
    maybe_fetch_repo(state.clone(), local_path.clone(), repo.url.clone()).await?;

    let detail = spawn_blocking(move || git::browse::commit_detail(&local_path, &hash))
        .await
        .map_err(join_error)??;
    Ok(Json(detail))
}

async fn maybe_fetch_repo(
    state: AppState,
    local_path: PathBuf,
    url: String,
) -> Result<(), AppError> {
    if !state.config.git.fetch_on_request {
        return Ok(());
    }
    let config = state.config.clone();
    spawn_blocking(move || git::clone::fetch_repo(&local_path, &url, &config))
        .await
        .map_err(join_error)?
}

fn join_error(err: tokio::task::JoinError) -> AppError {
    AppError::IoError(format!("blocking task join error: {err}"))
}
