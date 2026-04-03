use std::fs;
use std::path::{Path, PathBuf};

use git2::{build::RepoBuilder, Cred, CredentialType, FetchOptions, RemoteCallbacks, Repository};
use tracing::info;
use url::Url;

use crate::config::{AppConfig, RepoCredential};
use crate::error::AppError;

pub fn clone_repo(
    url: &str,
    local_path: &Path,
    config: &AppConfig,
) -> Result<Repository, AppError> {
    if let Some(parent) = local_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(remote_callbacks(config, url));

    let mut builder = RepoBuilder::new();
    builder.bare(true);
    builder.fetch_options(fetch_options);

    info!(%url, path = %local_path.display(), "cloning repository");
    builder
        .clone(url, local_path)
        .map_err(|err| AppError::CloneError(err.to_string()))
}

pub fn fetch_repo(local_path: &Path, url: &str, config: &AppConfig) -> Result<(), AppError> {
    let repo = Repository::open_bare(local_path)?;
    let mut remote = repo
        .find_remote("origin")
        .or_else(|_| repo.remote_anonymous(url))?;

    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(remote_callbacks(config, url));
    remote
        .fetch(&[] as &[&str], Some(&mut fetch_options), None)
        .map_err(|err| AppError::GitError(err.to_string()))?;
    Ok(())
}

pub fn open_bare_repo(local_path: &Path) -> Result<Repository, AppError> {
    Repository::open_bare(local_path).map_err(AppError::from)
}

pub fn default_branch(repo: &Repository) -> Result<String, AppError> {
    let reference = repo
        .find_reference("refs/remotes/origin/HEAD")
        .or_else(|_| repo.head())?;

    let shorthand = reference
        .symbolic_target()
        .or_else(|| reference.name())
        .and_then(|name| name.rsplit('/').next())
        .unwrap_or("main");
    Ok(shorthand.to_string())
}

pub fn repo_size_kb(local_path: &Path) -> Result<u64, AppError> {
    let mut total: u64 = 0;
    let mut stack = vec![PathBuf::from(local_path)];

    while let Some(path) = stack.pop() {
        let entries = fs::read_dir(path)?;
        for entry in entries {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if metadata.is_dir() {
                stack.push(entry.path());
            } else {
                total = total.saturating_add(metadata.len());
            }
        }
    }

    Ok(total / 1024)
}

fn remote_callbacks(config: &AppConfig, original_url: &str) -> RemoteCallbacks<'static> {
    let mut callbacks = RemoteCallbacks::new();
    let ssh_private_key = config.git.ssh_private_key_path.clone();
    let credential = find_credential_for_url(original_url, config);

    callbacks.credentials(move |url, username_from_url, allowed| {
        if allowed.contains(CredentialType::USER_PASS_PLAINTEXT) {
            if let Some(cred) = credential_for_callback(url, &credential) {
                return Cred::userpass_plaintext(&cred.username, &cred.password);
            }
        }

        if allowed.contains(CredentialType::SSH_KEY) {
            let username = username_from_url.unwrap_or("git");
            let key_path = PathBuf::from(&ssh_private_key);
            if key_path.exists() {
                return Cred::ssh_key(username, None, &key_path, None);
            }
            return Cred::ssh_key_from_agent(username);
        }

        if allowed.contains(CredentialType::USERNAME) {
            let username = username_from_url.unwrap_or("git");
            return Cred::username(username);
        }

        if allowed.contains(CredentialType::DEFAULT) {
            return Cred::default();
        }

        Err(git2::Error::from_str("unsupported credential type"))
    });

    callbacks
}

fn find_credential_for_url(url: &str, config: &AppConfig) -> Option<RepoCredential> {
    let host = extract_host(url)?;
    config
        .repos
        .credentials
        .iter()
        .find(|cred| cred.host.eq_ignore_ascii_case(&host))
        .cloned()
}

fn credential_for_callback(
    url: &str,
    configured: &Option<RepoCredential>,
) -> Option<RepoCredential> {
    let requested_host = extract_host(url)?;
    let credential = configured.as_ref()?;
    if credential.host.eq_ignore_ascii_case(&requested_host) {
        return Some(credential.clone());
    }
    None
}

fn extract_host(url: &str) -> Option<String> {
    if let Ok(parsed) = Url::parse(url) {
        return parsed.host_str().map(|host| host.to_string());
    }

    // ssh-style URL: git@host:owner/repo.git
    let at_split = url.split('@').nth(1)?;
    let host = at_split.split(':').next()?;
    Some(host.to_string())
}
