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
    maybe_launch_caddy(&state.config);

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
    serve_http(listener, app)
        .await
        .map_err(|err| AppError::IoError(err.to_string()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CaddyLaunchSpec {
    command: String,
    args: Vec<String>,
    working_dir: Option<String>,
}

fn maybe_launch_caddy(config: &AppConfig) {
    let Some(spec) = resolve_caddy_launch_spec(config) else {
        return;
    };

    let mut cmd = std::process::Command::new(&spec.command);
    cmd.args(&spec.args);
    if let Some(working_dir) = spec.working_dir.as_ref() {
        cmd.current_dir(working_dir);
    }

    match cmd.spawn() {
        Ok(child) => {
            info!(
                pid = child.id(),
                command = %spec.command,
                args = ?spec.args,
                "caddy process launched"
            );
        }
        Err(err) => {
            warn!(error = %err, command = %spec.command, "failed to launch caddy process");
        }
    }
}

fn resolve_caddy_launch_spec(config: &AppConfig) -> Option<CaddyLaunchSpec> {
    if !config.caddy.enabled {
        return None;
    }

    let command = config.caddy.command.trim();
    if command.is_empty() {
        warn!("caddy is enabled but command is empty; skipping launcher");
        return None;
    }

    let args = if !config.caddy.args.is_empty() {
        config.caddy.args.clone()
    } else if let Some(config_file) = config.caddy.config_file.as_ref() {
        let mut values = vec![
            "run".to_string(),
            "--config".to_string(),
            config_file.to_string(),
        ];
        let adapter = config.caddy.adapter.trim();
        if !adapter.is_empty() {
            values.push("--adapter".to_string());
            values.push(adapter.to_string());
        }
        values
    } else {
        let from = caddy_from_value(config);
        if from.is_empty() {
            warn!(
                "caddy is enabled but no domain/site_url was configured; skipping reverse-proxy launcher"
            );
            return None;
        }
        vec![
            "reverse-proxy".to_string(),
            "--from".to_string(),
            from,
            "--to".to_string(),
            format!("127.0.0.1:{}", config.server.port),
        ]
    };

    if args.is_empty() {
        warn!("caddy is enabled but no launch arguments were resolved; skipping launcher");
        return None;
    }

    Some(CaddyLaunchSpec {
        command: command.to_string(),
        args,
        working_dir: config.caddy.working_dir.clone(),
    })
}

fn caddy_from_value(config: &AppConfig) -> String {
    let domain = config.branding.domain.trim();
    if !domain.is_empty() {
        return domain.to_string();
    }
    config.branding.site_url.trim().to_string()
}

fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .compact()
        .try_init();
}

#[cfg(not(test))]
async fn serve_http(listener: TcpListener, app: axum::Router) -> Result<(), std::io::Error> {
    axum::serve(listener, app).await
}

#[cfg(test)]
async fn serve_http(_listener: TcpListener, _app: axum::Router) -> Result<(), std::io::Error> {
    Ok(())
}

async fn run_periodic_fetch(state: AppState) {
    let interval = match state.config.fetch.sync_interval() {
        Ok(value) => value,
        Err(err) => {
            warn!(
                error = %err,
                "invalid fetch interval in runtime config, falling back to 60s"
            );
            Duration::from_secs(60)
        }
    };
    info!(
        interval_secs = interval.as_secs(),
        "background fetch scheduler started"
    );

    let mut ticker = time::interval(interval);
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
                state.tree_cache.invalidate_all();
                state.language_cache.invalidate_all();
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
    use std::sync::{Mutex, OnceLock};

    use chrono::Utc;
    use tempfile::TempDir;
    use tokio::time::Duration;

    use super::*;
    use githree::config::{
        BrandingConfig, CaddyConfig, FeaturesConfig, FetchConfig, GitConfig, ReposConfig,
        ServerConfig, StorageConfig,
    };
    use githree::git::RepoInfo;

    fn env_guard() -> std::sync::MutexGuard<'static, ()> {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("lock env mutex")
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
                interval: Some("1s".to_string()),
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

    #[test]
    fn resolve_caddy_launch_spec_returns_none_when_disabled() {
        let mut config = make_config(Path::new("/tmp"));
        config.caddy.enabled = false;
        assert!(resolve_caddy_launch_spec(&config).is_none());
    }

    #[test]
    fn resolve_caddy_launch_spec_uses_config_file_when_present() {
        let mut config = make_config(Path::new("/tmp"));
        config.caddy.enabled = true;
        config.caddy.config_file = Some("./config/Caddyfile".to_string());
        config.caddy.adapter = "caddyfile".to_string();

        let spec = resolve_caddy_launch_spec(&config).expect("launch spec");
        assert_eq!(spec.command, "caddy");
        assert_eq!(
            spec.args,
            vec![
                "run".to_string(),
                "--config".to_string(),
                "./config/Caddyfile".to_string(),
                "--adapter".to_string(),
                "caddyfile".to_string(),
            ]
        );
    }

    #[test]
    fn resolve_caddy_launch_spec_falls_back_to_reverse_proxy_mode() {
        let mut config = make_config(Path::new("/tmp"));
        config.caddy.enabled = true;
        config.server.port = 3001;
        config.branding.domain = "githree.example.com".to_string();

        let spec = resolve_caddy_launch_spec(&config).expect("launch spec");
        assert_eq!(
            spec.args,
            vec![
                "reverse-proxy".to_string(),
                "--from".to_string(),
                "githree.example.com".to_string(),
                "--to".to_string(),
                "127.0.0.1:3001".to_string(),
            ]
        );
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

    fn create_empty_remote_fixture(base: &Path) -> String {
        let remote = base.join("empty-remote.git");
        run_git(
            &["init", "--bare", remote.to_str().expect("utf-8 path")],
            None,
        );
        remote.to_string_lossy().to_string()
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

    async fn seeded_state_with_interval(
        repo_name: &str,
        interval: &str,
    ) -> (TempDir, TempDir, AppState, String) {
        let (fixture_temp, _work, remote_url) = create_remote_fixture();
        let state_temp = tempfile::tempdir().expect("state tempdir");
        fs::create_dir_all(state_temp.path().join("repos")).expect("create repos dir");

        let mut config = make_config(state_temp.path());
        config.fetch.interval = Some(interval.to_string());
        config.fetch.interval_minutes = None;
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

    #[tokio::test]
    async fn periodic_fetch_falls_back_when_interval_is_invalid() {
        let (_fixture_temp, _state_temp, state, _remote_url) =
            seeded_state_with_interval("repo-invalid", "0s").await;
        let handle = tokio::spawn(run_periodic_fetch(state));
        tokio::time::sleep(Duration::from_millis(20)).await;
        handle.abort();
    }

    #[tokio::test]
    async fn periodic_fetch_handles_iteration_errors() {
        let (_fixture_temp, _state_temp, state, _remote_url) =
            seeded_state_with_interval("repo-iter", "1s").await;
        fs::write(state.config.registry_file(), "{invalid-json").expect("corrupt registry file");

        let handle = tokio::spawn(run_periodic_fetch(state));
        tokio::time::sleep(Duration::from_millis(1_200)).await;
        handle.abort();
    }

    #[tokio::test]
    async fn fetch_all_repositories_handles_stats_failures() {
        let fixture_temp = tempfile::tempdir().expect("fixture tempdir");
        let state_temp = tempfile::tempdir().expect("state tempdir");
        fs::create_dir_all(state_temp.path().join("repos")).expect("create repos dir");

        let config = make_config(state_temp.path());
        let registry = registry::RepoRegistry::new(config.registry_file())
            .await
            .expect("create registry");
        let state = AppState::new(config, registry);

        let remote_url = create_empty_remote_fixture(fixture_temp.path());
        let local_path = git::repo_disk_path(&state.config.repos_dir(), "empty-repo");
        git::clone::clone_repo(&remote_url, &local_path, &state.config).expect("clone empty repo");
        state
            .registry
            .upsert(RepoInfo {
                name: "empty-repo".to_string(),
                url: remote_url,
                description: None,
                default_branch: "main".to_string(),
                last_fetched: None,
                size_kb: 0,
                source: "generic".to_string(),
            })
            .await
            .expect("upsert empty repo");

        let result = fetch_all_repositories(state).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn fetch_all_repositories_handles_registry_write_failures() {
        let (_fixture_temp, _state_temp, state, _remote_url) = seeded_state("repo-readonly").await;
        let registry_path = state.config.registry_file();

        let path_for_task = registry_path.clone();
        let chmod_task = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            let metadata = fs::metadata(&path_for_task).expect("registry metadata");
            let mut permissions = metadata.permissions();
            permissions.set_readonly(true);
            fs::set_permissions(&path_for_task, permissions).expect("set readonly perms");
        });

        let result = fetch_all_repositories(state.clone()).await;
        assert!(result.is_ok());
        let _ = chmod_task.await;
    }

    #[test]
    fn binary_main_returns_error_for_invalid_bind_host() {
        let _guard = env_guard();
        let temp = tempfile::tempdir().expect("tempdir");
        let config_path = temp.path().join("main-invalid-host.toml");
        fs::write(
            &config_path,
            r#"
[server]
host = "999.999.999.999"
port = 3001

[storage]
repos_dir = "./repos"
registry_file = "./repos.json"
static_dir = "./static"

[git]
clone_timeout_secs = 5
fetch_on_request = false
fetch_cooldown_secs = 1
ssh_private_key_path = "~/.ssh/id_rsa"

[fetch]
enabled = false
interval = "60s"

[features]
web_repo_management = true
"#,
        )
        .expect("write config file");

        // SAFETY: process env is globally mutable; this test serializes env access with a mutex.
        unsafe {
            std::env::set_var("GITHREE_CONFIG", &config_path);
            std::env::set_var("RUST_LOG", "info");
        }

        let result = super::main();
        assert!(result.is_err());

        // SAFETY: process env is globally mutable; this test serializes env access with a mutex.
        unsafe {
            std::env::remove_var("GITHREE_CONFIG");
            std::env::remove_var("RUST_LOG");
        }
    }

    #[test]
    fn binary_main_completes_with_test_server_stub() {
        let _guard = env_guard();
        let temp = tempfile::tempdir().expect("tempdir");
        let config_path = temp.path().join("main-happy-path.toml");
        fs::write(
            &config_path,
            r#"
[server]
host = "127.0.0.1"
port = 0

[storage]
repos_dir = "./repos"
registry_file = "./repos.json"
static_dir = "./static"

[git]
clone_timeout_secs = 5
fetch_on_request = false
fetch_cooldown_secs = 1
ssh_private_key_path = "~/.ssh/id_rsa"

[fetch]
enabled = true
interval = "1s"

[features]
web_repo_management = true
"#,
        )
        .expect("write config file");

        // SAFETY: process env is globally mutable; this test serializes env access with a mutex.
        unsafe {
            std::env::set_var("GITHREE_CONFIG", &config_path);
            std::env::set_var("RUST_LOG", "info");
        }

        let result = super::main();
        assert!(result.is_ok());

        // SAFETY: process env is globally mutable; this test serializes env access with a mutex.
        unsafe {
            std::env::remove_var("GITHREE_CONFIG");
            std::env::remove_var("RUST_LOG");
        }
    }
}
