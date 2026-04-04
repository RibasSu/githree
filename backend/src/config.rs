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
    #[serde(default)]
    pub branding: BrandingConfig,
    #[serde(default)]
    pub caddy: CaddyConfig,
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

#[derive(Debug, Clone, Deserialize)]
pub struct FeaturesConfig {
    #[serde(default)]
    pub web_repo_management: bool,
    #[serde(default = "default_show_repo_controls")]
    pub show_repo_controls: bool,
}

fn default_show_repo_controls() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct BrandingConfig {
    pub app_name: String,
    pub logo_url: String,
    pub site_url: String,
    pub domain: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct CaddyConfig {
    pub enabled: bool,
    pub command: String,
    #[serde(default)]
    pub config_file: Option<String>,
    pub adapter: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub working_dir: Option<String>,
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

impl Default for FeaturesConfig {
    fn default() -> Self {
        Self {
            web_repo_management: false,
            show_repo_controls: true,
        }
    }
}

impl Default for BrandingConfig {
    fn default() -> Self {
        Self {
            app_name: "Githree".to_string(),
            logo_url: "/logo.svg".to_string(),
            site_url: "https://githree.org".to_string(),
            domain: "githree.org".to_string(),
        }
    }
}

impl Default for CaddyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            command: "caddy".to_string(),
            config_file: None,
            adapter: "caddyfile".to_string(),
            args: vec![],
            working_dir: None,
        }
    }
}

impl AppConfig {
    pub fn load() -> Result<Self, AppError> {
        let config_path = resolve_config_path();
        let settings = Config::builder()
            .add_source(File::from(config_path.as_path()).required(false))
            .build()
            .map_err(|e| AppError::InvalidRequest(format!("failed to load config: {e}")))?;

        let mut cfg: AppConfig = settings
            .try_deserialize()
            .map_err(|e| AppError::InvalidRequest(format!("invalid config: {e}")))?;

        let config_dir = resolve_config_dir(config_path.as_path());

        cfg.storage.repos_dir = resolve_path(&cfg.storage.repos_dir, config_dir.as_deref());
        cfg.storage.registry_file = resolve_path(&cfg.storage.registry_file, config_dir.as_deref());
        cfg.storage.static_dir = resolve_path(&cfg.storage.static_dir, config_dir.as_deref());
        cfg.git.ssh_private_key_path = resolve_path(&cfg.git.ssh_private_key_path, None);
        if let Some(config_file) = cfg.caddy.config_file.as_ref() {
            cfg.caddy.config_file = Some(resolve_path(config_file, config_dir.as_deref()));
        }
        if let Some(working_dir) = cfg.caddy.working_dir.as_ref() {
            cfg.caddy.working_dir = Some(resolve_path(working_dir, config_dir.as_deref()));
        }
        if let Ok(value) = env::var("GITHREE_WEB_REPO_MANAGEMENT") {
            cfg.features.web_repo_management =
                parse_bool_env_for("GITHREE_WEB_REPO_MANAGEMENT", &value)?;
        }
        if let Ok(value) = env::var("GITHREE_SHOW_REPO_CONTROLS") {
            cfg.features.show_repo_controls =
                parse_bool_env_for("GITHREE_SHOW_REPO_CONTROLS", &value)?;
        }
        if let Ok(value) = env::var("GITHREE_APP_NAME") {
            cfg.branding.app_name = value;
        }
        if let Ok(value) = env::var("GITHREE_LOGO_URL") {
            cfg.branding.logo_url = value;
        }
        if let Ok(value) = env::var("GITHREE_SITE_URL") {
            cfg.branding.site_url = value;
        }
        if let Ok(value) = env::var("GITHREE_DOMAIN") {
            cfg.branding.domain = value;
        }
        if let Ok(value) = env::var("GITHREE_CADDY_ENABLED") {
            cfg.caddy.enabled = parse_bool_env_for("GITHREE_CADDY_ENABLED", &value)?;
        }
        if let Ok(value) = env::var("GITHREE_CADDY_COMMAND") {
            cfg.caddy.command = value;
        }
        if let Ok(value) = env::var("GITHREE_CADDY_CONFIG_FILE") {
            let trimmed = value.trim();
            cfg.caddy.config_file = if trimmed.is_empty() {
                None
            } else {
                Some(resolve_path(trimmed, config_dir.as_deref()))
            };
        }
        if let Ok(value) = env::var("GITHREE_CADDY_WORKING_DIR") {
            let trimmed = value.trim();
            cfg.caddy.working_dir = if trimmed.is_empty() {
                None
            } else {
                Some(resolve_path(trimmed, config_dir.as_deref()))
            };
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

fn parse_bool_env_for(var_name: &str, value: &str) -> Result<bool, AppError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        other => Err(AppError::InvalidRequest(format!(
            "invalid boolean for {var_name}: {other}"
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

fn resolve_config_path() -> PathBuf {
    if let Ok(path) = env::var("GITHREE_CONFIG") {
        return PathBuf::from(expand_tilde(&path));
    }

    let preferred = PathBuf::from("config/default.toml");
    if preferred.exists() {
        return preferred;
    }

    let fallback = PathBuf::from("../config/default.toml");
    if fallback.exists() {
        return fallback;
    }

    preferred
}

fn resolve_config_dir(config_path: &Path) -> Option<PathBuf> {
    if !config_path.exists() {
        return None;
    }
    let parent = config_path.parent()?;
    if parent.is_absolute() {
        return Some(parent.to_path_buf());
    }
    env::current_dir().ok().map(|cwd| cwd.join(parent))
}

fn resolve_path(path: &str, base_dir: Option<&Path>) -> String {
    let expanded = expand_tilde(path);
    let path_buf = PathBuf::from(&expanded);
    if path_buf.is_absolute() {
        return expanded;
    }
    if let Some(base) = base_dir {
        return base.join(path_buf).to_string_lossy().to_string();
    }
    expanded
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
    use std::fs;
    use std::sync::{Mutex, OnceLock};
    use tempfile::tempdir;

    use super::*;

    fn env_guard() -> std::sync::MutexGuard<'static, ()> {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("lock env mutex")
    }

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

    #[test]
    fn parse_sync_interval_covers_overflow_and_parse_errors() {
        // This value overflows u64 parsing.
        assert!(parse_sync_interval("184467440737095516160s").is_err());
        // This value parses as u64 but overflows conversion to seconds.
        assert!(parse_sync_interval("3074457345618258603h").is_err());
    }

    #[test]
    fn parse_bool_env_accepts_true_false_and_rejects_invalid() {
        assert!(parse_bool_env_for("GITHREE_WEB_REPO_MANAGEMENT", "true").expect("parse true"));
        assert!(parse_bool_env_for("GITHREE_WEB_REPO_MANAGEMENT", " YES ").expect("parse yes"));
        assert!(!parse_bool_env_for("GITHREE_WEB_REPO_MANAGEMENT", "false").expect("parse false"));
        assert!(!parse_bool_env_for("GITHREE_WEB_REPO_MANAGEMENT", "off").expect("parse off"));
        assert!(parse_bool_env_for("GITHREE_WEB_REPO_MANAGEMENT", "definitely-not-bool").is_err());
    }

    #[test]
    fn sync_interval_supports_minutes_field_and_defaults() {
        let minutes_cfg = FetchConfig {
            enabled: true,
            interval: None,
            interval_minutes: Some(2),
        };
        assert_eq!(
            minutes_cfg.sync_interval().expect("minutes interval"),
            Duration::from_secs(120)
        );

        let default_cfg = FetchConfig {
            enabled: true,
            interval: None,
            interval_minutes: None,
        };
        assert_eq!(
            default_cfg.sync_interval().expect("default interval"),
            Duration::from_secs(60)
        );

        let zero_cfg = FetchConfig {
            enabled: true,
            interval: None,
            interval_minutes: Some(0),
        };
        assert!(zero_cfg.sync_interval().is_err());

        let overflow_cfg = FetchConfig {
            enabled: true,
            interval: None,
            interval_minutes: Some(u64::MAX),
        };
        assert!(overflow_cfg.sync_interval().is_err());
    }

    #[test]
    fn app_config_load_honors_env_overrides() {
        let _guard = env_guard();
        let temp = tempdir().expect("tempdir");
        let config_path = temp.path().join("githree.toml");
        fs::write(
            &config_path,
            r#"
[server]
host = "127.0.0.1"
port = 3111

[storage]
repos_dir = "./repos"
registry_file = "./repos.json"
static_dir = "./static"

[git]
clone_timeout_secs = 10
fetch_on_request = true
fetch_cooldown_secs = 1
ssh_private_key_path = "~/.ssh/id_rsa"

[fetch]
enabled = true
interval = "10s"
interval_minutes = 5

[features]
web_repo_management = true
show_repo_controls = true

[branding]
app_name = "Githree Config"
logo_url = "/brand/logo.svg"
site_url = "https://config.example.com"
domain = "config.example.com"

[caddy]
enabled = false
command = "caddy"
config_file = "./config/Caddyfile"
adapter = "caddyfile"
args = []
"#,
        )
        .expect("write config file");

        // SAFETY: process env is globally mutable; this test serializes env access with a mutex.
        unsafe {
            env::set_var("GITHREE_CONFIG", &config_path);
            env::set_var("GITHREE_WEB_REPO_MANAGEMENT", "off");
            env::set_var("GITHREE_SHOW_REPO_CONTROLS", "off");
            env::set_var("GITHREE_FETCH_INTERVAL", "2m");
            env::set_var("GITHREE_APP_NAME", "Githree Env");
            env::set_var("GITHREE_CADDY_ENABLED", "on");
            env::set_var("GITHREE_CADDY_WORKING_DIR", "~/githree");
        }

        let loaded = AppConfig::load().expect("load config");
        assert!(!loaded.features.web_repo_management);
        assert!(!loaded.features.show_repo_controls);
        assert_eq!(
            loaded.fetch.sync_interval().expect("sync interval"),
            Duration::from_secs(120)
        );
        assert_eq!(loaded.branding.app_name, "Githree Env");
        assert!(loaded.caddy.enabled);
        assert!(loaded.caddy.working_dir.is_some());
        let caddy_working_dir = loaded.caddy.working_dir.clone().expect("working dir");
        assert!(
            !caddy_working_dir.contains("~/"),
            "working dir should expand HOME: {caddy_working_dir}"
        );

        // SAFETY: process env is globally mutable; this test serializes env access with a mutex.
        unsafe {
            env::remove_var("GITHREE_CONFIG");
            env::remove_var("GITHREE_WEB_REPO_MANAGEMENT");
            env::remove_var("GITHREE_SHOW_REPO_CONTROLS");
            env::remove_var("GITHREE_FETCH_INTERVAL");
            env::remove_var("GITHREE_APP_NAME");
            env::remove_var("GITHREE_CADDY_ENABLED");
            env::remove_var("GITHREE_CADDY_WORKING_DIR");
        }
    }

    #[test]
    fn app_config_load_resolves_storage_paths_relative_to_config_file() {
        let _guard = env_guard();
        let temp = tempdir().expect("tempdir");
        let config_dir = temp.path().join("nested/config");
        fs::create_dir_all(&config_dir).expect("create config directory");
        let config_path = config_dir.join("githree.toml");
        fs::write(
            &config_path,
            r#"
[server]
host = "127.0.0.1"
port = 3111

[storage]
repos_dir = "../runtime/repos"
registry_file = "../runtime/repos.json"
static_dir = "../runtime/static"

[git]
clone_timeout_secs = 10
fetch_on_request = true
fetch_cooldown_secs = 1
ssh_private_key_path = "~/.ssh/id_rsa"

[fetch]
enabled = false
interval = "60s"

[caddy]
enabled = true
command = "caddy"
config_file = "./Caddyfile"
adapter = "caddyfile"
"#,
        )
        .expect("write config file");

        // SAFETY: process env is globally mutable; this test serializes env access with a mutex.
        unsafe {
            env::set_var("GITHREE_CONFIG", &config_path);
        }

        let loaded = AppConfig::load().expect("load config");
        assert_eq!(
            loaded.storage.repos_dir,
            config_dir
                .join("../runtime/repos")
                .to_string_lossy()
                .to_string()
        );
        assert_eq!(
            loaded.storage.registry_file,
            config_dir
                .join("../runtime/repos.json")
                .to_string_lossy()
                .to_string()
        );
        assert_eq!(
            loaded.storage.static_dir,
            config_dir
                .join("../runtime/static")
                .to_string_lossy()
                .to_string()
        );
        assert_eq!(
            loaded.caddy.config_file,
            Some(config_dir.join("./Caddyfile").to_string_lossy().to_string())
        );

        // SAFETY: process env is globally mutable; this test serializes env access with a mutex.
        unsafe {
            env::remove_var("GITHREE_CONFIG");
        }
    }

    #[test]
    fn resolve_path_preserves_absolute_and_expands_relative() {
        let base = PathBuf::from("/tmp/githree-config");
        let resolved = resolve_path("./data/repos", Some(base.as_path()));
        assert_eq!(
            resolved,
            base.join("./data/repos").to_string_lossy().to_string()
        );

        let absolute = resolve_path("/var/lib/githree/repos", Some(base.as_path()));
        assert_eq!(absolute, "/var/lib/githree/repos".to_string());
    }

    #[test]
    fn app_config_load_rejects_invalid_boolean_override() {
        let _guard = env_guard();
        let temp = tempdir().expect("tempdir");
        let config_path = temp.path().join("githree.toml");
        fs::write(
            &config_path,
            r#"
[server]
host = "127.0.0.1"
port = 3111
"#,
        )
        .expect("write config file");

        // SAFETY: process env is globally mutable; this test serializes env access with a mutex.
        unsafe {
            env::set_var("GITHREE_CONFIG", &config_path);
            env::set_var("GITHREE_WEB_REPO_MANAGEMENT", "invalid");
        }

        let err = AppConfig::load().expect_err("invalid bool must fail");
        assert!(
            matches!(
                err,
                AppError::InvalidRequest(message)
                    if message.contains("GITHREE_WEB_REPO_MANAGEMENT")
            ),
            "unexpected error type from invalid boolean override"
        );

        // SAFETY: process env is globally mutable; this test serializes env access with a mutex.
        unsafe {
            env::remove_var("GITHREE_CONFIG");
            env::remove_var("GITHREE_WEB_REPO_MANAGEMENT");
        }
    }
}
