use std::sync::Arc;

use moka::future::Cache;

use crate::config::AppConfig;
use crate::git::{LanguageStat, TreeEntry};
use crate::registry::RepoRegistry;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub registry: Arc<RepoRegistry>,
    pub tree_cache: Cache<String, Vec<TreeEntry>>,
    pub language_cache: Cache<String, Vec<LanguageStat>>,
    pub fetch_guard_cache: Cache<String, ()>,
}

impl AppState {
    pub fn new(config: AppConfig, registry: Arc<RepoRegistry>) -> Self {
        let fetch_ttl = std::time::Duration::from_secs(config.git.fetch_cooldown_secs.max(1));
        Self {
            config: Arc::new(config),
            registry,
            tree_cache: Cache::builder()
                .time_to_live(std::time::Duration::from_secs(60))
                .max_capacity(10_000)
                .build(),
            language_cache: Cache::builder()
                .time_to_live(std::time::Duration::from_secs(60))
                .max_capacity(10_000)
                .build(),
            fetch_guard_cache: Cache::builder()
                .time_to_live(fetch_ttl)
                .max_capacity(10_000)
                .build(),
        }
    }
}
