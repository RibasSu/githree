use std::env;
use std::path::{Path, PathBuf};

use config::{Config, File};
use serde::Deserialize;

use crate::error::AppError;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub storage: StorageConfig,
    #[serde(default)]
    pub git: GitConfig,
    #[serde(default)]
    pub fetch: FetchConfig,
    #[serde(default)]
    pub repos: ReposConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StorageConfig {
    pub repos_dir: String,
    pub registry_file: String,
    pub static_dir: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitConfig {
    pub clone_timeout_secs: u64,
    pub fetch_on_request: bool,
    pub ssh_private_key_path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FetchConfig {
    pub enabled: bool,
    pub interval_minutes: u64,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ReposConfig {
    #[serde(default)]
    pub credentials: Vec<RepoCredential>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RepoCredential {
    pub host: String,
    pub username: String,
    pub password: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            storage: StorageConfig::default(),
            git: GitConfig::default(),
            fetch: FetchConfig::default(),
            repos: ReposConfig::default(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 3001,
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            repos_dir: "./data/repos".to_string(),
            registry_file: "./data/repos.json".to_string(),
            static_dir: "./frontend/build".to_string(),
        }
    }
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            clone_timeout_secs: 120,
            fetch_on_request: true,
            ssh_private_key_path: "~/.ssh/id_rsa".to_string(),
        }
    }
}

impl Default for FetchConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_minutes: 30,
        }
    }
}

impl AppConfig {
    pub fn load() -> Result<Self, AppError> {
        let config_path =
            env::var("GITHREE_CONFIG").unwrap_or_else(|_| "config/default.toml".to_string());
        let settings = Config::builder()
            .add_source(File::from(Path::new(&config_path)).required(false))
            .build()
            .map_err(|e| AppError::InvalidRequest(format!("failed to load config: {e}")))?;

        let mut cfg: AppConfig = settings
            .try_deserialize()
            .map_err(|e| AppError::InvalidRequest(format!("invalid config: {e}")))?;

        cfg.storage.repos_dir = expand_tilde(&cfg.storage.repos_dir);
        cfg.storage.registry_file = expand_tilde(&cfg.storage.registry_file);
        cfg.storage.static_dir = expand_tilde(&cfg.storage.static_dir);
        cfg.git.ssh_private_key_path = expand_tilde(&cfg.git.ssh_private_key_path);
        Ok(cfg)
    }

    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.server.host, self.server.port)
    }

    pub fn repos_dir(&self) -> PathBuf {
        PathBuf::from(&self.storage.repos_dir)
    }

    pub fn registry_file(&self) -> PathBuf {
        PathBuf::from(&self.storage.registry_file)
    }

    pub fn static_dir(&self) -> PathBuf {
        PathBuf::from(&self.storage.static_dir)
    }
}

fn expand_tilde(path: &str) -> String {
    if let Some(stripped) = path.strip_prefix("~/") {
        if let Ok(home) = env::var("HOME") {
            return PathBuf::from(home)
                .join(stripped)
                .to_string_lossy()
                .to_string();
        }
    }
    path.to_string()
}
