use std::sync::Arc;

use moka::future::Cache;

use crate::config::AppConfig;
use crate::git::TreeEntry;
use crate::registry::RepoRegistry;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub registry: Arc<RepoRegistry>,
    pub tree_cache: Cache<String, Vec<TreeEntry>>,
}

impl AppState {
    pub fn new(config: AppConfig, registry: Arc<RepoRegistry>) -> Self {
        Self {
            config: Arc::new(config),
            registry,
            tree_cache: Cache::builder()
                .time_to_live(std::time::Duration::from_secs(60))
                .max_capacity(10_000)
                .build(),
        }
    }
}
