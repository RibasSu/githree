use std::env;
use std::path::{Path, PathBuf};
use std::time::Duration;

use config::{Config, File};
use serde::Deserialize;

use crate::error::AppError;

#[derive(Debug, Clone, Deserialize, Default)]
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
    #[serde(default)]
    pub features: FeaturesConfig,
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
    pub fetch_cooldown_secs: u64,
    pub ssh_private_key_path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FetchConfig {
    pub enabled: bool,
    #[serde(default)]
    pub interval: Option<String>,
    #[serde(default)]
    pub interval_minutes: Option<u64>,
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

#[derive(Debug, Clone, Deserialize, Default)]
pub struct FeaturesConfig {
    pub web_repo_management: bool,
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
            static_dir: "./static".to_string(),
        }
    }
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            clone_timeout_secs: 120,
            fetch_on_request: true,
            fetch_cooldown_secs: 20,
            ssh_private_key_path: "~/.ssh/id_rsa".to_string(),
        }
    }
}

impl Default for FetchConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interval: Some("60s".to_string()),
            interval_minutes: None,
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
        if let Ok(value) = env::var("GITHREE_WEB_REPO_MANAGEMENT") {
            cfg.features.web_repo_management = parse_bool_env(&value)?;
        }
        if let Ok(value) = env::var("GITHREE_FETCH_INTERVAL") {
            cfg.fetch.interval = Some(value);
        }
        cfg.fetch.sync_interval()?;
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

impl FetchConfig {
    pub fn sync_interval(&self) -> Result<Duration, AppError> {
        if let Some(value) = self.interval.as_ref() {
            return parse_sync_interval(value);
        }
        if let Some(minutes) = self.interval_minutes {
            if minutes == 0 {
                return Err(AppError::InvalidRequest(
                    "fetch interval_minutes must be greater than 0".to_string(),
                ));
            }
            let seconds = minutes.checked_mul(60).ok_or_else(|| {
                AppError::InvalidRequest("fetch interval_minutes is too large".to_string())
            })?;
            return Ok(Duration::from_secs(seconds));
        }
        Ok(Duration::from_secs(60))
    }
}

fn parse_bool_env(value: &str) -> Result<bool, AppError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        other => Err(AppError::InvalidRequest(format!(
            "invalid boolean for GITHREE_WEB_REPO_MANAGEMENT: {other}"
        ))),
    }
}

fn parse_sync_interval(value: &str) -> Result<Duration, AppError> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Err(AppError::InvalidRequest(
            "fetch interval cannot be empty".to_string(),
        ));
    }

    let split_index = normalized
        .find(|ch: char| !ch.is_ascii_digit())
        .unwrap_or(normalized.len());
    let (amount_raw, unit_raw) = normalized.split_at(split_index);
    if amount_raw.is_empty() {
        return Err(AppError::InvalidRequest(
            "fetch interval must start with a positive integer".to_string(),
        ));
    }

    let amount = amount_raw.parse::<u64>().map_err(|_| {
        AppError::InvalidRequest("fetch interval has an invalid numeric value".to_string())
    })?;
    if amount == 0 {
        return Err(AppError::InvalidRequest(
            "fetch interval must be greater than 0".to_string(),
        ));
    }

    let unit = unit_raw.trim();
    let seconds = match unit {
        "" | "s" | "sec" | "secs" | "second" | "seconds" => amount,
        "m" | "min" | "mins" | "minute" | "minutes" => amount
            .checked_mul(60)
            .ok_or_else(|| AppError::InvalidRequest("fetch interval is too large".to_string()))?,
        "h" | "hr" | "hrs" | "hour" | "hours" => amount
            .checked_mul(3_600)
            .ok_or_else(|| AppError::InvalidRequest("fetch interval is too large".to_string()))?,
        other => {
            return Err(AppError::InvalidRequest(format!(
                "invalid fetch interval unit '{other}'. Use s, m, or h (for example: 60s, 5m, 1h)"
            )));
        }
    };
    Ok(Duration::from_secs(seconds))
}

fn expand_tilde(path: &str) -> String {
    if let Some(stripped) = path.strip_prefix("~/")
        && let Ok(home) = env::var("HOME")
    {
        return PathBuf::from(home)
            .join(stripped)
            .to_string_lossy()
            .to_string();
    }
    path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fetch_interval_parses_seconds_minutes_and_hours() {
        assert_eq!(
            parse_sync_interval("60s").expect("parse 60s"),
            Duration::from_secs(60)
        );
        assert_eq!(
            parse_sync_interval("5m").expect("parse 5m"),
            Duration::from_secs(300)
        );
        assert_eq!(
            parse_sync_interval("2h").expect("parse 2h"),
            Duration::from_secs(7_200)
        );
        assert_eq!(
            parse_sync_interval("90").expect("parse bare seconds"),
            Duration::from_secs(90)
        );
    }

    #[test]
    fn fetch_interval_rejects_invalid_values() {
        assert!(parse_sync_interval("").is_err());
        assert!(parse_sync_interval("0s").is_err());
        assert!(parse_sync_interval("x10m").is_err());
        assert!(parse_sync_interval("10d").is_err());
    }
}
