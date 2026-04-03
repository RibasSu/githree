pub mod archive;
pub mod browse;
pub mod clone;
pub mod refs;

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoInfo {
    pub name: String,
    pub url: String,
    pub description: Option<String>,
    pub default_branch: String,
    pub last_fetched: Option<DateTime<Utc>>,
    pub size_kb: u64,
    pub source: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AddRepoRequest {
    pub url: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefsResponse {
    pub branches: Vec<String>,
    pub tags: Vec<String>,
    pub default_branch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageStat {
    pub language: String,
    pub bytes: u64,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeEntry {
    pub name: String,
    pub path: String,
    pub entry_type: String,
    pub oid: String,
    pub size: Option<u64>,
    pub mode: u32,
    pub last_commit: Option<CommitInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobResponse {
    pub content: String,
    pub encoding: String,
    pub size: usize,
    pub language: String,
    pub is_binary: bool,
    pub mime: Option<String>,
    pub is_truncated: bool,
    pub truncated_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadmeResponse {
    pub content: String,
    pub filename: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitInfo {
    pub hash: String,
    pub short_hash: String,
    pub author_name: String,
    pub author_email: String,
    pub authored_at: DateTime<Utc>,
    pub message: String,
    pub message_short: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffStats {
    pub files_changed: usize,
    pub insertions: usize,
    pub deletions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffLine {
    pub old_lineno: Option<u32>,
    pub new_lineno: Option<u32>,
    pub content: String,
    pub line_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffHunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDiff {
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub status: String,
    pub hunks: Vec<DiffHunk>,
    pub is_binary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitDetail {
    pub commit: CommitInfo,
    pub parents: Vec<String>,
    pub stats: DiffStats,
    pub diffs: Vec<FileDiff>,
    pub is_truncated: bool,
    pub truncated_reason: Option<String>,
    pub displayed_file_count: usize,
    pub displayed_line_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveResponse {
    pub content_type: String,
    pub file_name: String,
    pub path: PathBuf,
}

pub fn detect_repo_source(url: &str) -> String {
    if url.contains("github.com") {
        return "github".to_string();
    }
    if url.contains("gitlab.com") {
        return "gitlab".to_string();
    }
    "generic".to_string()
}

pub fn derive_repo_name(url: &str, alias: Option<&str>) -> Result<String, AppError> {
    if let Some(alias) = alias {
        return sanitize_name(alias);
    }

    let normalized = url.trim_end_matches('/').trim_end_matches(".git");
    let raw_name = normalized.rsplit(['/', ':']).next().unwrap_or_default();
    sanitize_name(raw_name)
}

pub fn sanitize_name(value: &str) -> Result<String, AppError> {
    let lowered = value.trim().to_lowercase();
    if lowered.is_empty() {
        return Err(AppError::InvalidRequest(
            "repository name cannot be empty".to_string(),
        ));
    }

    let sanitized: String = lowered
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect();

    let compact = sanitized
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");

    if compact.is_empty() {
        return Err(AppError::InvalidRequest(
            "repository name contains no usable characters".to_string(),
        ));
    }

    Ok(compact)
}

pub fn repo_disk_path(repos_dir: &Path, name: &str) -> PathBuf {
    repos_dir.join(name)
}

pub fn detect_language(path: &str) -> String {
    let lowered = path.to_ascii_lowercase();
    let path_ref = Path::new(&lowered);

    let file_name = path_ref
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default();

    if matches!(file_name, "dockerfile" | "containerfile")
        || file_name.ends_with(".dockerfile")
        || file_name.ends_with(".containerfile")
    {
        return "docker".to_string();
    }

    let ext = path_ref
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase());

    match ext.as_deref() {
        Some("rs") => "rust",
        Some("js") => "javascript",
        Some("ts") => "typescript",
        Some("tsx") => "tsx",
        Some("jsx") => "jsx",
        Some("svelte") => "svelte",
        Some("py") => "python",
        Some("go") => "go",
        Some("java") => "java",
        Some("c") => "c",
        Some("cpp") | Some("cc") | Some("cxx") => "cpp",
        Some("h") | Some("hpp") => "cpp",
        Some("json") => "json",
        Some("toml") => "toml",
        Some("yaml") | Some("yml") => "yaml",
        Some("md") => "markdown",
        Some("html") => "html",
        Some("css") => "css",
        Some("sh") => "bash",
        Some("sql") => "sql",
        Some("rb") => "ruby",
        Some("php") => "php",
        Some("swift") => "swift",
        Some("kt") => "kotlin",
        Some("dart") => "dart",
        Some("xml") => "xml",
        Some("dockerfile") | Some("containerfile") => "docker",
        Some("lock") => "text",
        _ => "text",
    }
    .to_string()
}
