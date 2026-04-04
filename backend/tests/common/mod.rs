#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::Utc;
use githree::config::{
    AppConfig, BrandingConfig, CaddyConfig, FeaturesConfig, FetchConfig, GitConfig, ReposConfig,
    ServerConfig, StorageConfig,
};
use githree::git;
use githree::git::RepoInfo;
use githree::registry::RepoRegistry;
use githree::state::AppState;
use tempfile::TempDir;

pub struct RepoFixture {
    _temp: TempDir,
    pub root: PathBuf,
    pub work_dir: PathBuf,
    pub remote_bare: PathBuf,
}

impl RepoFixture {
    pub fn new() -> Self {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().to_path_buf();
        let work_dir = root.join("work");
        let remote_bare = root.join("remote.git");

        run_git(
            &["init", "--initial-branch=main", path_to_str(&work_dir)],
            None,
        );
        run_git(&["config", "user.name", "Githree Tester"], Some(&work_dir));
        run_git(
            &["config", "user.email", "tester@githree.local"],
            Some(&work_dir),
        );

        fs::create_dir_all(work_dir.join("src")).expect("create src dir");
        fs::create_dir_all(work_dir.join("docs")).expect("create docs dir");

        fs::write(
            work_dir.join("README.md"),
            "# Sample Repo\n\nThis repository powers backend tests.\n",
        )
        .expect("write README");
        fs::write(
            work_dir.join("src/main.rs"),
            "fn main() { println!(\"hi\"); }\n",
        )
        .expect("write main.rs");
        fs::write(work_dir.join("docs/guide.md"), "guide v1\n").expect("write guide");
        fs::write(work_dir.join("text.txt"), "line1\nline2\n").expect("write text file");
        fs::write(work_dir.join("binary.bin"), [0_u8, 159, 146, 150, 1, 2, 3])
            .expect("write binary file");
        run_git(&["add", "."], Some(&work_dir));
        run_git(&["commit", "-m", "feat: initial files"], Some(&work_dir));

        fs::write(
            work_dir.join("src/main.rs"),
            "fn main() {\n    println!(\"hello from main\");\n}\n",
        )
        .expect("update main.rs");
        fs::write(work_dir.join("docs/guide.md"), "guide v2\n").expect("update guide");
        run_git(&["add", "."], Some(&work_dir));
        run_git(
            &["commit", "-m", "feat: update source and docs"],
            Some(&work_dir),
        );

        run_git(
            &["mv", "docs/guide.md", "docs/renamed-guide.md"],
            Some(&work_dir),
        );
        fs::write(work_dir.join("docs/renamed-guide.md"), "guide v3\n")
            .expect("update renamed guide");
        run_git(&["commit", "-am", "chore: rename guide"], Some(&work_dir));

        run_git(&["rm", "text.txt"], Some(&work_dir));
        run_git(
            &["commit", "-m", "chore: remove text file"],
            Some(&work_dir),
        );

        run_git(&["tag", "--no-sign", "v1.0.0", "HEAD~1"], Some(&work_dir));

        run_git(&["checkout", "-b", "feature-x"], Some(&work_dir));
        fs::write(work_dir.join("feature.txt"), "feature branch\n").expect("write feature file");
        run_git(&["add", "feature.txt"], Some(&work_dir));
        run_git(
            &["commit", "-m", "feat: feature branch commit"],
            Some(&work_dir),
        );
        run_git(&["checkout", "main"], Some(&work_dir));

        run_git(
            &[
                "clone",
                "--bare",
                path_to_str(&work_dir),
                path_to_str(&remote_bare),
            ],
            None,
        );

        run_git(
            &["remote", "add", "origin", path_to_str(&remote_bare)],
            Some(&work_dir),
        );
        run_git(&["push", "--all", "origin"], Some(&work_dir));
        run_git(&["push", "--tags", "origin"], Some(&work_dir));

        Self {
            _temp: temp,
            root,
            work_dir,
            remote_bare,
        }
    }

    pub fn remote_url(&self) -> String {
        path_to_str(&self.remote_bare).to_string()
    }

    pub fn add_remote_commit(&self, file: &str, content: &[u8], message: &str) -> String {
        run_git(&["checkout", "main"], Some(&self.work_dir));
        let target = self.work_dir.join(file);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).expect("create parent directories");
        }
        fs::write(&target, content).expect("write remote commit file");
        run_git(&["add", file], Some(&self.work_dir));
        run_git(&["commit", "-m", message], Some(&self.work_dir));
        run_git(&["push", "origin", "main"], Some(&self.work_dir));
        git_stdout(&["rev-parse", "HEAD"], Some(&self.work_dir))
            .trim()
            .to_string()
    }

    pub fn resolve_ref(&self, reference: &str) -> String {
        git_stdout(&["rev-parse", reference], Some(&self.work_dir))
            .trim()
            .to_string()
    }
}

pub fn test_config(base: &Path) -> AppConfig {
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
            enabled: false,
            interval: Some("60s".to_string()),
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

pub async fn test_state(base: &Path) -> AppState {
    fs::create_dir_all(base.join("repos")).expect("create repos dir");
    let config = test_config(base);
    let registry = RepoRegistry::new(PathBuf::from(&config.storage.registry_file))
        .await
        .expect("create repo registry");
    AppState::new(config, registry)
}

pub async fn clone_and_register_repo(
    state: &AppState,
    name: &str,
    url: &str,
) -> Result<RepoInfo, githree::error::AppError> {
    let local_path = git::repo_disk_path(&state.config.repos_dir(), name);
    git::clone::clone_repo(url, &local_path, &state.config)?;
    let repo = git::clone::open_bare_repo(&local_path)?;
    let info = RepoInfo {
        name: name.to_string(),
        url: url.to_string(),
        description: Some("test repo".to_string()),
        default_branch: git::clone::default_branch(&repo)?,
        last_fetched: Some(Utc::now()),
        size_kb: git::clone::repo_size_kb(&local_path)?,
        source: git::detect_repo_source(url),
    };
    state.registry.upsert(info.clone()).await?;
    Ok(info)
}

pub fn run_git(args: &[&str], cwd: Option<&Path>) {
    let mut cmd = Command::new("git");
    cmd.args(["-c", "commit.gpgsign=false", "-c", "tag.gpgsign=false"]);
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
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

pub fn git_stdout(args: &[&str], cwd: Option<&Path>) -> String {
    let mut cmd = Command::new("git");
    cmd.args(["-c", "commit.gpgsign=false", "-c", "tag.gpgsign=false"]);
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let output = cmd.output().expect("run git command");
    assert!(
        output.status.success(),
        "git command failed: git {}\nstdout:\n{}\nstderr:\n{}",
        args.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

pub fn path_to_str(path: &Path) -> &str {
    path.to_str().expect("utf-8 path")
}
