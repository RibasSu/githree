use axum::Json;
use axum::extract::{Path, State};
use tokio::task::spawn_blocking;
use tracing::instrument;

use crate::error::AppError;
use crate::git::{self, RefsResponse};
use crate::handlers::sync::{ensure_repo_ready, join_error};
use crate::state::AppState;

#[instrument(skip(state), fields(repo_name = %name))]
pub async fn get_refs(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<RefsResponse>, AppError> {
    let repo = state.registry.get(&name).await?;
    let local_path = git::repo_disk_path(&state.config.repos_dir(), &name);

    ensure_repo_ready(state.clone(), &name, local_path.clone(), repo.url.clone()).await?;

    let refs = spawn_blocking(move || git::refs::list_refs(&local_path))
        .await
        .map_err(join_error)??;
    Ok(Json(refs))
}
