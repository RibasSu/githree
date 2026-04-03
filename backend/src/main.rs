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
