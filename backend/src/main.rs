use chrono::Utc;
use tokio::net::TcpListener;
use tokio::task::spawn_blocking;
use tokio::time::{self, Duration};
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use githree::config::AppConfig;
use githree::error::AppError;
use githree::state::AppState;
use githree::{git, registry, router};

#[tokio::main]
async fn main() -> Result<(), AppError> {
    init_tracing();

    let config = AppConfig::load()?;
    std::fs::create_dir_all(config.repos_dir())?;
    if let Some(parent) = config.registry_file().parent() {
        std::fs::create_dir_all(parent)?;
    }

    let registry = registry::RepoRegistry::new(config.registry_file()).await?;
    let state = AppState::new(config, registry);

    if state.config.fetch.enabled {
        let background_state = state.clone();
        tokio::spawn(async move {
            run_periodic_fetch(background_state).await;
        });
    }

    let app = router::build_router(state.clone());
    let addr = state.config.bind_addr();
    let listener = TcpListener::bind(&addr).await?;
    info!(%addr, "githree server running");
    axum::serve(listener, app)
        .await
        .map_err(|err| AppError::IoError(err.to_string()))
}

fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .compact()
        .init();
}

async fn run_periodic_fetch(state: AppState) {
    let interval_minutes = state.config.fetch.interval_minutes.max(1);
    info!(interval_minutes, "background fetch scheduler started");

    let mut ticker = time::interval(Duration::from_secs(interval_minutes * 60));
    ticker.tick().await;

    loop {
        ticker.tick().await;
        if let Err(err) = fetch_all_repositories(state.clone()).await {
            warn!(error = %err, "background fetch iteration failed");
        }
    }
}

async fn fetch_all_repositories(state: AppState) -> Result<(), AppError> {
    let repos = state.registry.list().await?;
    for mut repo in repos {
        let local_path = git::repo_disk_path(&state.config.repos_dir(), &repo.name);
        let url = repo.url.clone();
        let config = state.config.clone();

        let result =
            spawn_blocking(move || git::clone::fetch_repo(&local_path, &url, &config)).await;
        match result {
            Ok(Ok(())) => {
                repo.last_fetched = Some(Utc::now());
                let path_for_stats = git::repo_disk_path(&state.config.repos_dir(), &repo.name);
                let stats_result = spawn_blocking(move || {
                    let opened_repo = git::clone::open_bare_repo(&path_for_stats)?;
                    let default_branch = git::clone::default_branch(&opened_repo)?;
                    let size_kb = git::clone::repo_size_kb(&path_for_stats)?;
                    Ok::<(String, u64), AppError>((default_branch, size_kb))
                })
                .await;

                match stats_result {
                    Ok(Ok((default_branch, size_kb))) => {
                        repo.default_branch = default_branch;
                        repo.size_kb = size_kb;
                    }
                    Ok(Err(err)) => {
                        warn!(repo = %repo.name, error = %err, "failed to refresh repo stats")
                    }
                    Err(err) => warn!(repo = %repo.name, error = %err, "stats task join error"),
                }

                if let Err(err) = state.registry.upsert(repo).await {
                    warn!(error = %err, "failed to update repository fetch timestamp");
                }
            }
            Ok(Err(err)) => warn!(error = %err, "failed background fetch for repo"),
            Err(err) => error!(error = %err, "background fetch task join error"),
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use chrono::Utc;
    use tempfile::TempDir;
    use tokio::time::Duration;

    use super::*;
    use githree::config::{
        FeaturesConfig, FetchConfig, GitConfig, ReposConfig, ServerConfig, StorageConfig,
    };
    use githree::git::RepoInfo;

    fn run_git(args: &[&str], cwd: Option<&Path>) {
        let mut cmd = Command::new("git");
        cmd.args(args);
        if let Some(path) = cwd {
            cmd.current_dir(path);
        }
        let output = cmd.output().expect("run git command");
        assert!(
            output.status.success(),
            "git command failed: git {}\nstdout:\n{}\nstderr:\n{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

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
                enabled: true,
                interval_minutes: 1,
            },
            repos: ReposConfig::default(),
            features: FeaturesConfig {
                web_repo_management: true,
            },
        }
    }

    fn create_remote_fixture() -> (TempDir, PathBuf, String) {
        let temp = tempfile::tempdir().expect("tempdir");
        let work = temp.path().join("work");
        let remote = temp.path().join("remote.git");

        run_git(
            &[
                "init",
                "--initial-branch=main",
                work.to_str().expect("utf-8 path"),
            ],
            None,
        );
        run_git(&["config", "user.name", "Main Test"], Some(&work));
        run_git(
            &["config", "user.email", "main-test@example.com"],
            Some(&work),
        );
        fs::write(work.join("README.md"), "# main test\n").expect("write readme");
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
        run_git(
            &[
                "remote",
                "add",
                "origin",
                remote.to_str().expect("utf-8 path"),
            ],
            Some(&work),
        );
        run_git(&["push", "--all", "origin"], Some(&work));

        (temp, work, remote.to_string_lossy().to_string())
    }

    async fn seeded_state(repo_name: &str) -> (TempDir, TempDir, AppState, String) {
        let (fixture_temp, _work, remote_url) = create_remote_fixture();
        let state_temp = tempfile::tempdir().expect("state tempdir");
        fs::create_dir_all(state_temp.path().join("repos")).expect("create repos dir");

        let config = make_config(state_temp.path());
        let registry = registry::RepoRegistry::new(config.registry_file())
            .await
            .expect("create registry");
        let state = AppState::new(config, registry);

        let local_path = git::repo_disk_path(&state.config.repos_dir(), repo_name);
        git::clone::clone_repo(&remote_url, &local_path, &state.config).expect("clone repository");
        let repo = git::clone::open_bare_repo(&local_path).expect("open bare repo");

        let info = RepoInfo {
            name: repo_name.to_string(),
            url: remote_url.clone(),
            description: None,
            default_branch: git::clone::default_branch(&repo).expect("default branch"),
            last_fetched: Some(Utc::now()),
            size_kb: git::clone::repo_size_kb(&local_path).expect("repo size"),
            source: git::detect_repo_source(&remote_url),
        };
        state
            .registry
            .upsert(info)
            .await
            .expect("save repo in registry");

        (fixture_temp, state_temp, state, remote_url)
    }

    #[tokio::test]
    async fn fetch_all_repositories_updates_registered_repos() {
        let (_fixture_temp, _state_temp, state, _remote_url) = seeded_state("repo-a").await;
        fetch_all_repositories(state.clone())
            .await
            .expect("fetch all repositories");

        let repos = state.registry.list().await.expect("list repos");
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].name, "repo-a");
        assert!(repos[0].last_fetched.is_some());
    }

    #[tokio::test]
    async fn fetch_all_repositories_survives_repo_fetch_failures() {
        let (_fixture_temp, _state_temp, state, _remote_url) = seeded_state("repo-b").await;
        state
            .registry
            .upsert(RepoInfo {
                name: "broken".to_string(),
                url: "file:///definitely/missing/repo.git".to_string(),
                description: None,
                default_branch: "main".to_string(),
                last_fetched: None,
                size_kb: 0,
                source: "generic".to_string(),
            })
            .await
            .expect("upsert broken repo");

        let result = fetch_all_repositories(state.clone()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn periodic_fetch_scheduler_runs_without_panicking() {
        let (_fixture_temp, _state_temp, state, _remote_url) = seeded_state("repo-c").await;
        let handle = tokio::spawn(run_periodic_fetch(state));
        tokio::time::sleep(Duration::from_millis(20)).await;
        tokio::task::yield_now().await;
        handle.abort();
    }
}
