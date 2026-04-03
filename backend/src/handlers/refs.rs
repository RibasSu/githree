use axum::extract::{Path, State};
use axum::Json;
use tokio::task::spawn_blocking;
use tracing::instrument;

use crate::error::AppError;
use crate::git::{self, RefsResponse};
use crate::state::AppState;

#[instrument(skip(state), fields(repo_name = %name))]
pub async fn get_refs(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<RefsResponse>, AppError> {
    let repo = state.registry.get(&name).await?;
    let local_path = git::repo_disk_path(&state.config.repos_dir(), &name);

    if state.config.git.fetch_on_request {
        let config = state.config.clone();
        let url = repo.url.clone();
        let fetch_path = local_path.clone();
        spawn_blocking(move || git::clone::fetch_repo(&fetch_path, &url, &config))
            .await
            .map_err(join_error)??;
    }

    let refs = spawn_blocking(move || git::refs::list_refs(&local_path))
        .await
        .map_err(join_error)??;
    Ok(Json(refs))
}

fn join_error(err: tokio::task::JoinError) -> AppError {
    AppError::IoError(format!("blocking task join error: {err}"))
}
