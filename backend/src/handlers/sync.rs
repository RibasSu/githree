use std::path::PathBuf;

use tokio::task::spawn_blocking;
use tracing::warn;

use crate::error::AppError;
use crate::git;
use crate::state::AppState;

pub async fn ensure_repo_ready(
    state: AppState,
    repo_name: &str,
    local_path: PathBuf,
    url: String,
) -> Result<(), AppError> {
    if !local_path.exists() {
        let config = state.config.clone();
        let clone_path = local_path.clone();
        let clone_url = url.clone();
        spawn_blocking(move || {
            git::clone::clone_repo(&clone_url, &clone_path, &config).map(|_| ())
        })
        .await
        .map_err(join_error)??;
    }

    if !state.config.git.fetch_on_request {
        return Ok(());
    }

    let fetch_key = format!("{repo_name}|{url}");
    if state.fetch_guard_cache.get(&fetch_key).await.is_some() {
        return Ok(());
    }

    state.fetch_guard_cache.insert(fetch_key, ()).await;
    let config = state.config.clone();
    let repo_name = repo_name.to_string();
    let outcome = spawn_blocking(move || git::clone::fetch_repo(&local_path, &url, &config)).await;
    handle_fetch_outcome(&state, &repo_name, outcome);

    Ok(())
}

fn handle_fetch_outcome(
    state: &AppState,
    repo_name: &str,
    outcome: Result<Result<(), AppError>, tokio::task::JoinError>,
) {
    match outcome {
        Ok(Ok(())) => {
            state.tree_cache.invalidate_all();
            state.language_cache.invalidate_all();
        }
        Ok(Err(err)) => {
            warn!(repo = %repo_name, error = %err, "on-request fetch failed");
        }
        Err(err) => {
            warn!(repo = %repo_name, error = %err, "on-request fetch join failed");
        }
    }
}

pub fn join_error(err: tokio::task::JoinError) -> AppError {
    AppError::IoError(format!("blocking task join error: {err}"))
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

    fn run_git(args: &[&str], cwd: Option<&Path>) {
        let mut cmd = Command::new("git");
        cmd.args(args);
        if let Some(path) = cwd {
            cmd.current_dir(path);
        }
        let output = cmd.output().expect("run git command");
        assert!(output.status.success());
    }

    fn make_config(base: &Path, fetch_on_request: bool) -> AppConfig {
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
                fetch_on_request,
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
        run_git(&["config", "user.name", "Sync Test"], Some(&work));
        run_git(
            &["config", "user.email", "sync-test@example.com"],
            Some(&work),
        );
        fs::write(work.join("README.md"), "sync test\n").expect("write readme");
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
    async fn ensure_repo_ready_clones_when_missing() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(temp.path().join("repos")).expect("create repos dir");
        let (remote_url, _) = create_remote_fixture(temp.path());

        let config = make_config(temp.path(), false);
        let registry = RepoRegistry::new(config.registry_file())
            .await
            .expect("create registry");
        let state = AppState::new(config, registry);

        let local_path = state.config.repos_dir().join("clone-target");
        assert!(!local_path.exists());
        ensure_repo_ready(
            state,
            "clone-target",
            local_path.clone(),
            remote_url.clone(),
        )
        .await
        .expect("ensure repo ready");
        assert!(local_path.exists());
    }

    #[tokio::test]
    async fn ensure_repo_ready_swallow_fetch_errors() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(temp.path().join("repos")).expect("create repos dir");
        let config = make_config(temp.path(), true);
        let registry = RepoRegistry::new(config.registry_file())
            .await
            .expect("create registry");
        let state = AppState::new(config, registry);

        let local_path = state.config.repos_dir().join("broken-local.git");
        run_git(
            &["init", "--bare", local_path.to_str().expect("utf-8 path")],
            None,
        );

        ensure_repo_ready(
            state,
            "broken",
            local_path,
            "file:///definitely/missing/repo.git".to_string(),
        )
        .await
        .expect("fetch errors are swallowed");
    }

    #[tokio::test]
    async fn handle_fetch_outcome_covers_join_error_branch() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(temp.path().join("repos")).expect("create repos dir");
        let config = make_config(temp.path(), true);
        let registry = RepoRegistry::new(config.registry_file())
            .await
            .expect("create registry");
        let state = AppState::new(config, registry);

        let join_error = tokio::spawn(async { panic!("boom") })
            .await
            .expect_err("must produce join error");
        handle_fetch_outcome(&state, "join-error", Err(join_error));
    }

    #[tokio::test]
    async fn join_error_maps_to_io_error() {
        let panic_join_error = tokio::spawn(async { panic!("panic for join error mapping") })
            .await
            .expect_err("must produce join error");
        match super::join_error(panic_join_error) {
            AppError::IoError(message) => assert!(message.contains("blocking task join error")),
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
