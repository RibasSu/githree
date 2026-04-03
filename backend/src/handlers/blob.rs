use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::HeaderValue;
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::response::IntoResponse;
use serde::Deserialize;
use tokio::task::spawn_blocking;
use tracing::instrument;

use crate::error::AppError;
use crate::git::{self, BlobResponse, ReadmeResponse};
use crate::handlers::sync::{ensure_repo_ready, join_error};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct BlobQuery {
    #[serde(rename = "ref")]
    pub ref_name: Option<String>,
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct ReadmeQuery {
    #[serde(rename = "ref")]
    pub ref_name: Option<String>,
}

#[instrument(skip(state, query), fields(repo_name = %name))]
pub async fn get_blob(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(query): Query<BlobQuery>,
) -> Result<Json<BlobResponse>, AppError> {
    let repo = state.registry.get(&name).await?;
    let ref_name = query
        .ref_name
        .unwrap_or_else(|| repo.default_branch.clone());
    let local_path = git::repo_disk_path(&state.config.repos_dir(), &name);
    ensure_repo_ready(state.clone(), &name, local_path.clone(), repo.url.clone()).await?;

    let path = query.path.clone();
    let blob = spawn_blocking(move || git::browse::read_blob(&local_path, &ref_name, &path))
        .await
        .map_err(join_error)??;
    Ok(Json(blob))
}

#[instrument(skip(state, query), fields(repo_name = %name))]
pub async fn get_readme(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(query): Query<ReadmeQuery>,
) -> Result<Json<ReadmeResponse>, AppError> {
    let repo = state.registry.get(&name).await?;
    let ref_name = query
        .ref_name
        .unwrap_or_else(|| repo.default_branch.clone());
    let local_path = git::repo_disk_path(&state.config.repos_dir(), &name);
    ensure_repo_ready(state.clone(), &name, local_path.clone(), repo.url.clone()).await?;

    let readme = spawn_blocking(move || git::browse::read_readme(&local_path, &ref_name))
        .await
        .map_err(join_error)??;
    Ok(Json(readme))
}

#[instrument(skip(state, query), fields(repo_name = %name))]
pub async fn get_raw(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(query): Query<BlobQuery>,
) -> Result<impl IntoResponse, AppError> {
    let repo = state.registry.get(&name).await?;
    let ref_name = query
        .ref_name
        .unwrap_or_else(|| repo.default_branch.clone());
    let local_path = git::repo_disk_path(&state.config.repos_dir(), &name);
    ensure_repo_ready(state.clone(), &name, local_path.clone(), repo.url.clone()).await?;

    let path = query.path.clone();
    let raw = spawn_blocking(move || git::browse::read_raw(&local_path, &ref_name, &path))
        .await
        .map_err(join_error)??;

    let content_type = HeaderValue::from_str(&raw.mime)
        .map_err(|err| AppError::InvalidRequest(format!("invalid MIME type: {err}")))?;
    let disposition = HeaderValue::from_str(&format!("attachment; filename=\"{}\"", raw.file_name))
        .map_err(|err| {
            AppError::InvalidRequest(format!("invalid file name in content disposition: {err}"))
        })?;

    Ok((
        [
            (CONTENT_TYPE, content_type),
            (CONTENT_DISPOSITION, disposition),
        ],
        Bytes::from(raw.content),
    ))
}
