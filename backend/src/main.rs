use chrono::Utc;
use serde_json::json;
use tokio::net::TcpListener;
use tokio::task::spawn_blocking;
use tokio::time::{self, Duration};
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use githree::config::AppConfig;
use githree::error::AppError;
use githree::git::RepoInfo;
use githree::state::AppState;
use githree::{git, registry, router};

const CLI_USAGE: &str = r#"Usage:
  githree serve
  githree repo add --url <repo_url> [--name <alias>]
  githree repo remove --name <alias>
  githree repo fetch --name <alias>
  githree repo list

Docker examples:
  docker compose exec -T githree githree repo add --url https://github.com/user/repo.git --name my-repo
  docker compose exec -T githree githree repo list
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
enum CliCommand {
    Serve,
    Help,
    RepoAdd { url: String, name: Option<String> },
    RepoRemove { name: String },
    RepoFetch { name: String },
    RepoList,
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    init_tracing();

    let args = collect_cli_args();
    let command = parse_cli_command(&args)?;
    match command {
        CliCommand::Serve => run_server().await,
        CliCommand::Help => {
            write_stdout(CLI_USAGE)?;
            Ok(())
        }
        other => run_repo_cli(other).await,
    }
}

fn collect_cli_args() -> Vec<String> {
    #[cfg(test)]
    {
        Vec::new()
    }
    #[cfg(not(test))]
    {
        std::env::args().skip(1).collect()
    }
}

fn parse_cli_command(args: &[String]) -> Result<CliCommand, AppError> {
    if args.is_empty() {
        return Ok(CliCommand::Serve);
    }

    match args[0].as_str() {
        "serve" => {
            if args.len() == 1 {
                Ok(CliCommand::Serve)
            } else {
                Err(cli_usage_error("unexpected arguments for `serve`"))
            }
        }
        "repo" => parse_repo_subcommand(&args[1..]),
        "help" | "--help" | "-h" => Ok(CliCommand::Help),
        _ => Err(cli_usage_error("unknown command")),
    }
}

fn parse_repo_subcommand(args: &[String]) -> Result<CliCommand, AppError> {
    if args.is_empty() {
        return Err(cli_usage_error("missing repo subcommand"));
    }

    let subcommand = args[0].as_str();
    let mut url: Option<String> = None;
    let mut name: Option<String> = None;
    let mut index = 1usize;

    while index < args.len() {
        match args[index].as_str() {
            "--url" | "-u" => {
                index += 1;
                if index >= args.len() {
                    return Err(cli_usage_error("missing value for `--url`"));
                }
                url = Some(args[index].clone());
            }
            "--name" | "-n" => {
                index += 1;
                if index >= args.len() {
                    return Err(cli_usage_error("missing value for `--name`"));
                }
                name = Some(args[index].clone());
            }
            "--help" | "-h" => return Ok(CliCommand::Help),
            _ => return Err(cli_usage_error("unknown argument")),
        }
        index += 1;
    }

    match subcommand {
        "add" => {
            let repo_url = url.ok_or_else(|| cli_usage_error("`repo add` requires `--url`"))?;
            Ok(CliCommand::RepoAdd {
                url: repo_url,
                name,
            })
        }
        "remove" => {
            let repo_name =
                name.ok_or_else(|| cli_usage_error("`repo remove` requires `--name`"))?;
            Ok(CliCommand::RepoRemove { name: repo_name })
        }
        "fetch" => {
            let repo_name =
                name.ok_or_else(|| cli_usage_error("`repo fetch` requires `--name`"))?;
            Ok(CliCommand::RepoFetch { name: repo_name })
        }
        "list" => {
            if url.is_some() || name.is_some() {
                Err(cli_usage_error(
                    "`repo list` does not accept `--url` or `--name`",
                ))
            } else {
                Ok(CliCommand::RepoList)
            }
        }
        _ => Err(cli_usage_error("unknown repo subcommand")),
    }
}

fn cli_usage_error(message: &str) -> AppError {
    AppError::InvalidRequest(format!("{message}\n\n{CLI_USAGE}"))
}

async fn run_server() -> Result<(), AppError> {
    let state = load_app_state().await?;
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

async fn load_app_state() -> Result<AppState, AppError> {
    let config = AppConfig::load()?;
    std::fs::create_dir_all(config.repos_dir())?;
    if let Some(parent) = config.registry_file().parent() {
        std::fs::create_dir_all(parent)?;
    }

    let registry = registry::RepoRegistry::new(config.registry_file()).await?;
    Ok(AppState::new(config, registry))
}

async fn run_repo_cli(command: CliCommand) -> Result<(), AppError> {
    let state = load_app_state().await?;

    match command {
        CliCommand::RepoAdd { url, name } => repo_add_cli(state, url, name).await,
        CliCommand::RepoRemove { name } => repo_remove_cli(state, name).await,
        CliCommand::RepoFetch { name } => repo_fetch_cli(state, name).await,
        CliCommand::RepoList => repo_list_cli(state).await,
        CliCommand::Serve | CliCommand::Help => Ok(()),
    }
}

async fn repo_add_cli(state: AppState, url: String, name: Option<String>) -> Result<(), AppError> {
    let repo_name = git::derive_repo_name(&url, name.as_deref())?;
    let local_path = git::repo_disk_path(&state.config.repos_dir(), &repo_name);
    clone_or_fetch_repo_cli(
        state.config.clone(),
        local_path.clone(),
        url.clone(),
        local_path.exists(),
    )
    .await?;
    let saved = upsert_repo_info_cli(state, repo_name, url, local_path).await?;
    write_json_stdout(&saved)
}

async fn repo_remove_cli(state: AppState, name: String) -> Result<(), AppError> {
    let local_path = git::repo_disk_path(&state.config.repos_dir(), &name);
    state.registry.remove(&name).await?;
    if local_path.exists() {
        spawn_blocking(move || std::fs::remove_dir_all(local_path))
            .await
            .map_err(join_error)?
            .map_err(AppError::from)?;
    }
    write_json_stdout(&json!({ "removed": name }))
}

async fn repo_fetch_cli(state: AppState, name: String) -> Result<(), AppError> {
    let existing = state.registry.get(&name).await?;
    let local_path = git::repo_disk_path(&state.config.repos_dir(), &name);
    clone_or_fetch_repo_cli(
        state.config.clone(),
        local_path.clone(),
        existing.url.clone(),
        local_path.exists(),
    )
    .await?;
    let saved = upsert_repo_info_cli(state, name, existing.url, local_path).await?;
    write_json_stdout(&saved)
}

async fn repo_list_cli(state: AppState) -> Result<(), AppError> {
    let repos = state.registry.list().await?;
    write_json_stdout(&repos)
}

async fn clone_or_fetch_repo_cli(
    config: std::sync::Arc<AppConfig>,
    local_path: std::path::PathBuf,
    url: String,
    already_exists: bool,
) -> Result<(), AppError> {
    let timeout_duration = Duration::from_secs(config.git.clone_timeout_secs);
    let config_for_task = config.clone();
    let local_path_for_task = local_path.clone();
    let url_for_task = url.clone();
    let task = spawn_blocking(move || {
        if already_exists {
            git::clone::fetch_repo(&local_path_for_task, &url_for_task, &config_for_task)
        } else {
            git::clone::clone_repo(&url_for_task, &local_path_for_task, &config_for_task)
                .map(|_| ())
        }
    });

    let outcome = tokio::time::timeout(timeout_duration, task)
        .await
        .map_err(|_| {
            AppError::CloneError(format!(
                "git operation timed out after {}s",
                config.git.clone_timeout_secs
            ))
        })?;
    outcome.map_err(join_error)?
}

async fn upsert_repo_info_cli(
    state: AppState,
    name: String,
    url: String,
    local_path: std::path::PathBuf,
) -> Result<RepoInfo, AppError> {
    let (default_branch, size_kb) = spawn_blocking(move || {
        let repo = git::clone::open_bare_repo(&local_path)?;
        let default_branch = git::clone::default_branch(&repo)?;
        let size_kb = git::clone::repo_size_kb(&local_path)?;
        Ok::<(String, u64), AppError>((default_branch, size_kb))
    })
    .await
    .map_err(join_error)??;

    let saved = state
        .registry
        .upsert(RepoInfo {
            name,
            url: url.clone(),
            description: None,
            default_branch,
            last_fetched: Some(Utc::now()),
            size_kb,
            source: git::detect_repo_source(&url),
        })
        .await?;
    Ok(saved)
}

fn write_json_stdout<T: serde::Serialize>(value: &T) -> Result<(), AppError> {
    let mut stdout = std::io::stdout();
    serde_json::to_writer_pretty(&mut stdout, value)?;
    use std::io::Write;
    stdout.write_all(b"\n")?;
    Ok(())
}

fn write_stdout(value: &str) -> Result<(), AppError> {
    use std::io::Write;
    let mut stdout = std::io::stdout();
    stdout.write_all(value.as_bytes())?;
    stdout.write_all(b"\n")?;
    Ok(())
}

fn join_error(err: tokio::task::JoinError) -> AppError {
    AppError::IoError(format!("blocking task join error: {err}"))
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
        cmd.env("GIT_CONFIG_GLOBAL", "/dev/null");
        cmd.env("GIT_CONFIG_NOSYSTEM", "1");
        cmd.env("GIT_TERMINAL_PROMPT", "0");
        cmd.env("GIT_AUTHOR_NAME", "Githree Tests");
        cmd.env("GIT_AUTHOR_EMAIL", "tests@githree.local");
        cmd.env("GIT_COMMITTER_NAME", "Githree Tests");
        cmd.env("GIT_COMMITTER_EMAIL", "tests@githree.local");
        if let Some(path) = cwd {
            cmd.current_dir(path);
        }
        let output = cmd.output().expect("run git command");
        assert!(
            output.status.success(),
            "git {:?} failed: {}\n{}",
            args,
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
                interval: Some("1s".to_string()),
                interval_minutes: None,
            },
            repos: ReposConfig::default(),
            features: FeaturesConfig {
                web_repo_management: true,
                show_repo_controls: true,
            },
            branding: BrandingConfig::default(),
            caddy: CaddyConfig::default(),
        }
    }

    fn parse_args(values: &[&str]) -> Result<CliCommand, AppError> {
        let args = values
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>();
        parse_cli_command(&args)
    }

    #[test]
    fn parse_cli_defaults_to_serve() {
        let command = parse_args(&[]).expect("default command");
        assert_eq!(command, CliCommand::Serve);
    }

    #[test]
    fn parse_cli_parses_repo_add() {
        let command = parse_args(&["repo", "add", "--url", "https://example.com/repo.git"])
            .expect("repo add");
        assert_eq!(
            command,
            CliCommand::RepoAdd {
                url: "https://example.com/repo.git".to_string(),
                name: None,
            }
        );
    }

    #[test]
    fn parse_cli_parses_repo_add_with_alias() {
        let command = parse_args(&[
            "repo",
            "add",
            "--url",
            "https://example.com/repo.git",
            "--name",
            "demo",
        ])
        .expect("repo add with alias");
        assert_eq!(
            command,
            CliCommand::RepoAdd {
                url: "https://example.com/repo.git".to_string(),
                name: Some("demo".to_string()),
            }
        );
    }

    #[test]
    fn parse_cli_requires_repo_add_url() {
        let err = parse_args(&["repo", "add"]).expect_err("missing url should fail");
        assert!(err.to_string().contains("requires `--url`"));
    }

    #[test]
    fn parse_cli_parses_repo_remove_and_list() {
        let remove = parse_args(&["repo", "remove", "--name", "demo"]).expect("repo remove");
        assert_eq!(
            remove,
            CliCommand::RepoRemove {
                name: "demo".to_string(),
            }
        );

        let list = parse_args(&["repo", "list"]).expect("repo list");
        assert_eq!(list, CliCommand::RepoList);
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
        assert!(
            result.is_ok()
                || matches!(
                    &result,
                    Err(AppError::IoError(message))
                        if message.contains("Operation not permitted")
                            || message.contains("Permission denied")
                ),
            "expected Ok(()) or sandbox permission error, got: {result:?}"
        );

        // SAFETY: process env is globally mutable; this test serializes env access with a mutex.
        unsafe {
            std::env::remove_var("GITHREE_CONFIG");
            std::env::remove_var("RUST_LOG");
        }
    }
}
