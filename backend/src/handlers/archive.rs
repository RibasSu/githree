use std::path::PathBuf;
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::HeaderValue;
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::response::IntoResponse;
use serde::Deserialize;
use tokio::fs::File;
use tokio::task::spawn_blocking;
use tokio_util::io::ReaderStream;
use tracing::instrument;

use crate::error::AppError;
use crate::git::{self, ArchiveResponse};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ArchiveQuery {
    #[serde(rename = "ref")]
    pub ref_name: Option<String>,
    pub format: Option<String>,
}

#[instrument(skip(state, query), fields(repo_name = %name))]
pub async fn get_archive(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(query): Query<ArchiveQuery>,
) -> Result<impl IntoResponse, AppError> {
    let repo = state.registry.get(&name).await?;
    let ref_name = query
        .ref_name
        .unwrap_or_else(|| repo.default_branch.clone());
    let format = query.format.unwrap_or_else(|| "tar.gz".to_string());
    let local_path = git::repo_disk_path(&state.config.repos_dir(), &name);

    maybe_fetch_repo(state.clone(), local_path.clone(), repo.url.clone()).await?;

    let archive = spawn_blocking(move || {
        git::archive::create_archive(&local_path, &name, &ref_name, &format)
    })
    .await
    .map_err(join_error)??;
    stream_archive(archive).await
}

async fn stream_archive(archive: ArchiveResponse) -> Result<impl IntoResponse, AppError> {
    let file = File::open(&archive.path).await?;
    let stream = ReaderStream::new(file);
    let content_type = HeaderValue::from_str(&archive.content_type)
        .map_err(|err| AppError::InvalidRequest(format!("invalid content type: {err}")))?;
    let content_disposition =
        HeaderValue::from_str(&format!("attachment; filename=\"{}\"", archive.file_name)).map_err(
            |err| AppError::InvalidRequest(format!("invalid content disposition: {err}")),
        )?;

    cleanup_archive_later(archive.path.clone());
    Ok((
        [
            (CONTENT_TYPE, content_type),
            (CONTENT_DISPOSITION, content_disposition),
        ],
        axum::body::Body::from_stream(stream),
    ))
}

fn cleanup_archive_later(path: PathBuf) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(120)).await;
        let _ = tokio::fs::remove_file(path).await;
    });
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
