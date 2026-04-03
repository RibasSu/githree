use std::fs;
use std::path::{Path, PathBuf};

use git2::{Cred, CredentialType, FetchOptions, RemoteCallbacks, Repository, build::RepoBuilder};
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
        if allowed.contains(CredentialType::USER_PASS_PLAINTEXT)
            && let Some(cred) = credential_for_callback(url, &credential)
        {
            return Cred::userpass_plaintext(&cred.username, &cred.password);
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::process::Command;

    use tempfile::tempdir;

    use super::*;
    use crate::config::{FetchConfig, GitConfig, ReposConfig, ServerConfig, StorageConfig};

    fn test_config_with_credentials() -> AppConfig {
        AppConfig {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 0,
            },
            storage: StorageConfig {
                repos_dir: "./data/repos".to_string(),
                registry_file: "./data/repos.json".to_string(),
                static_dir: "./static".to_string(),
            },
            git: GitConfig {
                clone_timeout_secs: 5,
                fetch_on_request: false,
                fetch_cooldown_secs: 20,
                ssh_private_key_path: "~/.ssh/id_rsa".to_string(),
            },
            fetch: FetchConfig {
                enabled: false,
                interval_minutes: 30,
            },
            repos: ReposConfig {
                credentials: vec![RepoCredential {
                    host: "gitlab.example.com".to_string(),
                    username: "bot".to_string(),
                    password: "token".to_string(),
                }],
            },
        }
    }

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

    #[test]
    fn extract_host_handles_https_and_ssh_urls() {
        assert_eq!(
            extract_host("https://github.com/org/repo.git").as_deref(),
            Some("github.com")
        );
        assert_eq!(
            extract_host("git@gitlab.example.com:team/repo.git").as_deref(),
            Some("gitlab.example.com")
        );
        assert!(extract_host("not-a-valid-url").is_none());
    }

    #[test]
    fn credential_matching_uses_case_insensitive_host() {
        let config = test_config_with_credentials();
        let found = find_credential_for_url("https://GitLab.Example.com/team/repo.git", &config)
            .expect("credential should match by host");
        assert_eq!(found.username, "bot");

        let requested = credential_for_callback(
            "https://gitlab.example.com/team/repo.git",
            &Some(found.clone()),
        )
        .expect("callback credential should match");
        assert_eq!(requested.password, "token");

        let mismatch =
            credential_for_callback("https://github.com/org/repo.git", &Some(found.clone()));
        assert!(mismatch.is_none());
    }

    #[test]
    fn repo_size_kb_scans_nested_directories() {
        let temp = tempdir().expect("tempdir");
        fs::create_dir_all(temp.path().join("nested")).expect("create nested dir");
        fs::write(temp.path().join("a.bin"), vec![0_u8; 600]).expect("write top level file");
        fs::write(temp.path().join("nested").join("b.bin"), vec![0_u8; 2048])
            .expect("write nested file");

        let size_kb = repo_size_kb(temp.path()).expect("repo size");
        assert!(size_kb >= 2);
    }

    #[test]
    fn clone_and_fetch_return_useful_errors_for_missing_repositories() {
        let config = test_config_with_credentials();
        let temp = tempdir().expect("tempdir");
        let clone_path = temp.path().join("missing-clone.git");

        let clone_error = match clone_repo(
            "file:///definitely/does/not/exist/repo.git",
            &clone_path,
            &config,
        ) {
            Ok(_) => panic!("clone should fail"),
            Err(err) => err,
        };
        match clone_error {
            AppError::CloneError(_) => {}
            other => panic!("expected CloneError, got {other:?}"),
        }

        let fetch_error = fetch_repo(&clone_path, "https://example.com/repo.git", &config)
            .expect_err("fetch should fail");
        match fetch_error {
            AppError::GitError(_) => {}
            other => panic!("expected GitError, got {other:?}"),
        }
    }

    #[test]
    fn default_branch_uses_head_when_origin_head_ref_is_missing() {
        let temp = tempdir().expect("tempdir");
        let work = temp.path().join("work");
        let bare = temp.path().join("bare.git");

        run_git(
            &[
                "init",
                "--initial-branch=main",
                work.to_str().expect("utf-8 path"),
            ],
            None,
        );
        run_git(&["config", "user.name", "Tester"], Some(&work));
        run_git(&["config", "user.email", "tester@example.com"], Some(&work));
        fs::write(work.join("README.md"), "# hello\n").expect("write readme");
        run_git(&["add", "."], Some(&work));
        run_git(&["commit", "-m", "init"], Some(&work));
        run_git(
            &[
                "clone",
                "--bare",
                work.to_str().expect("utf-8 path"),
                bare.to_str().expect("utf-8 path"),
            ],
            None,
        );

        let repo = open_bare_repo(&bare).expect("open bare repo");
        let branch = default_branch(&repo).expect("default branch");
        assert_eq!(branch, "main");
    }
}
