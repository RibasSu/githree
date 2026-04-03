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
use crate::handlers::sync::{ensure_repo_ready, join_error};
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

    ensure_repo_ready(state.clone(), &name, local_path.clone(), repo.url.clone()).await?;

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
    let content_type = content_type_header(&archive.content_type)?;
    let content_disposition = content_disposition_header(&archive.file_name)?;

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
    cleanup_archive_after(path, Duration::from_secs(120));
}

fn cleanup_archive_after(path: PathBuf, delay: Duration) {
    tokio::spawn(async move {
        tokio::time::sleep(delay).await;
        let _ = tokio::fs::remove_file(path).await;
    });
}

fn content_type_header(value: &str) -> Result<HeaderValue, AppError> {
    HeaderValue::from_str(value)
        .map_err(|err| AppError::InvalidRequest(format!("invalid content type: {err}")))
}

fn content_disposition_header(file_name: &str) -> Result<HeaderValue, AppError> {
    HeaderValue::from_str(&format!("attachment; filename=\"{}\"", file_name))
        .map_err(|err| AppError::InvalidRequest(format!("invalid content disposition: {err}")))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[tokio::test]
    async fn stream_archive_rejects_invalid_headers() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("archive.bin");
        fs::write(&path, b"archive-bytes").expect("write archive file");

        let bad_type = ArchiveResponse {
            content_type: "text/plain\nx".to_string(),
            file_name: "ok.zip".to_string(),
            path: path.clone(),
        };
        let bad_type_result = stream_archive(bad_type).await;
        assert!(matches!(
            bad_type_result,
            Err(AppError::InvalidRequest(message)) if message.contains("invalid content type")
        ));

        let bad_disposition = ArchiveResponse {
            content_type: "application/zip".to_string(),
            file_name: "bad\nname.zip".to_string(),
            path,
        };
        let bad_disposition_result = stream_archive(bad_disposition).await;
        assert!(matches!(
            bad_disposition_result,
            Err(AppError::InvalidRequest(message))
                if message.contains("invalid content disposition")
        ));
    }

    #[tokio::test]
    async fn cleanup_archive_after_removes_file() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("to-remove.tar.gz");
        fs::write(&path, b"temp archive").expect("write temp archive");
        assert!(path.exists());

        cleanup_archive_after(path.clone(), Duration::from_millis(10));
        tokio::time::sleep(Duration::from_millis(40)).await;

        assert!(!path.exists());
    }
}
