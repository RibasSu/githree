use std::path::PathBuf;

use axum::{Json, extract::Path, extract::State, http::StatusCode};
use chrono::Utc;
use serde::Serialize;
use tokio::task::spawn_blocking;
use tokio::time::{Duration, timeout};
use tracing::{info, instrument};

use crate::error::AppError;
use crate::git::{self, AddRepoRequest, RepoInfo};
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct SettingsResponse {
    pub web_repo_management: bool,
    pub repos_dir: String,
    pub registry_file: String,
    pub app_name: String,
    pub logo_url: String,
    pub site_url: String,
    pub domain: String,
    pub caddy_enabled: bool,
}

#[instrument(skip(state))]
pub async fn get_settings(
    State(state): State<AppState>,
) -> Result<Json<SettingsResponse>, AppError> {
    Ok(Json(SettingsResponse {
        web_repo_management: state.config.features.web_repo_management,
        repos_dir: state.config.storage.repos_dir.clone(),
        registry_file: state.config.storage.registry_file.clone(),
        app_name: state.config.branding.app_name.clone(),
        logo_url: state.config.branding.logo_url.clone(),
        site_url: state.config.branding.site_url.clone(),
        domain: state.config.branding.domain.clone(),
        caddy_enabled: state.config.caddy.enabled,
    }))
}

#[instrument(skip(state, payload), fields(repo_url = %payload.url))]
pub async fn add_repo(
    State(state): State<AppState>,
    Json(payload): Json<AddRepoRequest>,
) -> Result<Json<RepoInfo>, AppError> {
    ensure_management_enabled(&state)?;

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
    state.language_cache.invalidate_all();
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
    ensure_management_enabled(&state)?;

    let local_path = git::repo_disk_path(&state.config.repos_dir(), &name);
    state.registry.remove(&name).await?;

    if local_path.exists() {
        let path = local_path.clone();
        spawn_blocking(move || std::fs::remove_dir_all(path))
            .await
            .map_err(join_error)??;
    }
    state.tree_cache.invalidate_all();
    state.language_cache.invalidate_all();
    state.fetch_guard_cache.invalidate_all();

    Ok(StatusCode::NO_CONTENT)
}

#[instrument(skip(state), fields(repo_name = %name))]
pub async fn fetch_repo(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<RepoInfo>, AppError> {
    ensure_management_enabled(&state)?;

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
    state.language_cache.invalidate_all();
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

fn ensure_management_enabled(state: &AppState) -> Result<(), AppError> {
    if state.config.features.web_repo_management {
        return Ok(());
    }
    Err(AppError::Forbidden(
        "repository management via web API is disabled".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::process::Command;

    use crate::config::{
        AppConfig, BrandingConfig, CaddyConfig, FeaturesConfig, FetchConfig, GitConfig,
        ReposConfig, ServerConfig, StorageConfig,
    };
    use crate::registry::RepoRegistry;

    use super::*;

    fn make_config(base: &Path) -> AppConfig {
        AppConfig {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 0,
            },
            storage: StorageConfig {
                repos_dir: base.join("repos").to_string_lossy().to_string(),
                registry_file: base.join("repos.json").to_string_lossy().to_string(),
                static_dir: base.join("static").to_string_lossy().to_string(),
            },
            git: GitConfig {
                clone_timeout_secs: 10,
                fetch_on_request: false,
                fetch_cooldown_secs: 20,
                ssh_private_key_path: "~/.ssh/id_rsa".to_string(),
            },
            fetch: FetchConfig {
                enabled: false,
                interval: Some("60s".to_string()),
                interval_minutes: None,
            },
            repos: ReposConfig::default(),
            features: FeaturesConfig {
                web_repo_management: true,
            },
            branding: BrandingConfig::default(),
            caddy: CaddyConfig::default(),
        }
    }

    fn run_git(args: &[&str], cwd: Option<&Path>) {
        let mut cmd = Command::new("git");
        cmd.args(args);
        if let Some(path) = cwd {
            cmd.current_dir(path);
        }
        let output = cmd.output().expect("run git command");
        assert!(output.status.success());
    }

    fn create_remote_fixture(base: &Path) -> (String, std::path::PathBuf) {
        let work = base.join("work");
        let remote = base.join("remote.git");
        run_git(
            &[
                "init",
                "--initial-branch=main",
                work.to_str().expect("utf-8 path"),
            ],
            None,
        );
        run_git(&["config", "user.name", "Repos Test"], Some(&work));
        run_git(
            &["config", "user.email", "repos-test@example.com"],
            Some(&work),
        );
        fs::write(work.join("README.md"), "repos test\n").expect("write readme");
        run_git(&["add", "."], Some(&work));
        run_git(&["commit", "-m", "init"], Some(&work));
        run_git(
            &[
                "clone",
                "--bare",
                work.to_str().expect("utf-8 path"),
                remote.to_str().expect("utf-8 path"),
            ],
            None,
        );
        (remote.to_string_lossy().to_string(), remote)
    }

    #[tokio::test]
    async fn clone_or_fetch_repo_timeout_maps_to_clone_error() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(temp.path().join("repos")).expect("create repos dir");

        let mut config = make_config(temp.path());
        config.git.clone_timeout_secs = 0;
        let registry = RepoRegistry::new(config.registry_file())
            .await
            .expect("create registry");
        let state = AppState::new(config, registry);

        let local_path = state.config.repos_dir().join("timeout-repo");
        let err = clone_or_fetch_repo(
            state,
            local_path,
            "https://example.com/never-cloned.git".to_string(),
            false,
        )
        .await
        .expect_err("timeout must fail");
        match err {
            AppError::CloneError(message) => assert!(message.contains("timed out")),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn build_repo_info_reports_git_errors_for_invalid_repo_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        let err = build_repo_info(
            "invalid".to_string(),
            "https://example.com/repo.git".to_string(),
            temp.path().join("missing-repo-path"),
        )
        .await
        .expect_err("missing repo should fail");
        match err {
            AppError::GitError(_) | AppError::IoError(_) => {}
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn clone_or_fetch_and_build_repo_info_work_for_valid_repo() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(temp.path().join("repos")).expect("create repos dir");
        let (remote_url, _) = create_remote_fixture(temp.path());

        let config = make_config(temp.path());
        let registry = RepoRegistry::new(config.registry_file())
            .await
            .expect("create registry");
        let state = AppState::new(config, registry);

        let local_path = state.config.repos_dir().join("ok-repo");
        clone_or_fetch_repo(state.clone(), local_path.clone(), remote_url.clone(), false)
            .await
            .expect("clone repo");

        let info = build_repo_info("ok-repo".to_string(), remote_url, local_path)
            .await
            .expect("build repo info");
        assert_eq!(info.name, "ok-repo");
        assert!(!info.default_branch.is_empty());
    }

    #[tokio::test]
    async fn join_error_maps_to_io_error() {
        let panic_join_error = tokio::spawn(async { panic!("panic for join_error test") })
            .await
            .expect_err("must return join error");
        match super::join_error(panic_join_error) {
            AppError::IoError(message) => assert!(message.contains("blocking task join error")),
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
