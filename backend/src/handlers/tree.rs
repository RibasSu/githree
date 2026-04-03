use axum::{extract::Path, extract::Query, extract::State, Json};
use serde::Deserialize;
use tokio::task::spawn_blocking;
use tracing::instrument;

use crate::error::AppError;
use crate::git::{self, TreeEntry};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct TreeQuery {
    #[serde(rename = "ref")]
    pub ref_name: Option<String>,
    pub path: Option<String>,
}

#[instrument(skip(state, query), fields(repo_name = %name))]
pub async fn get_tree(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(query): Query<TreeQuery>,
) -> Result<Json<Vec<TreeEntry>>, AppError> {
    let repo = state.registry.get(&name).await?;
    let ref_name = query
        .ref_name
        .unwrap_or_else(|| repo.default_branch.clone());
    let path = query.path.unwrap_or_default();
    let local_path = git::repo_disk_path(&state.config.repos_dir(), &name);

    maybe_fetch_repo(state.clone(), local_path.clone(), repo.url.clone()).await?;

    let key = format!("{name}|{ref_name}|{path}");
    if let Some(cached) = state.tree_cache.get(&key).await {
        return Ok(Json(cached));
    }

    let entries = spawn_blocking(move || git::browse::list_tree(&local_path, &ref_name, &path))
        .await
        .map_err(join_error)??;
    state.tree_cache.insert(key, entries.clone()).await;
    Ok(Json(entries))
}

async fn maybe_fetch_repo(
    state: AppState,
    local_path: std::path::PathBuf,
    url: String,
) -> Result<(), AppError> {
    if state.config.git.fetch_on_request == false {
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
