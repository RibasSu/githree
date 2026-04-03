mod common;

use std::fs;

use axum::body::to_bytes;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use githree::config::AppConfig;
use githree::error::AppError;
use githree::git::{self, RepoInfo};
use githree::registry::RepoRegistry;
use tempfile::tempdir;

#[test]
fn app_config_loads_default_file_and_helpers_work() {
    let cfg = AppConfig::load().expect("load default config");
    assert!(!cfg.bind_addr().is_empty());
    assert!(!cfg.repos_dir().as_os_str().is_empty());
    assert!(!cfg.registry_file().as_os_str().is_empty());
    assert!(!cfg.static_dir().as_os_str().is_empty());
}

#[tokio::test]
async fn app_error_into_response_maps_status_and_code() {
    let cases = vec![
        (
            AppError::NotFound("missing".to_string()),
            StatusCode::NOT_FOUND,
            "NOT_FOUND",
        ),
        (
            AppError::InvalidRequest("bad".to_string()),
            StatusCode::BAD_REQUEST,
            "INVALID_REQUEST",
        ),
        (
            AppError::CloneError("clone failed".to_string()),
            StatusCode::BAD_GATEWAY,
            "CLONE_ERROR",
        ),
        (
            AppError::Forbidden("forbidden".to_string()),
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
        ),
        (
            AppError::GitError("git failed".to_string()),
            StatusCode::INTERNAL_SERVER_ERROR,
            "GIT_ERROR",
        ),
        (
            AppError::IoError("io failed".to_string()),
            StatusCode::INTERNAL_SERVER_ERROR,
            "IO_ERROR",
        ),
    ];

    for (error, expected_status, expected_code) in cases {
        let response = error.into_response();
        assert_eq!(response.status(), expected_status);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read response body");
        let json: serde_json::Value = serde_json::from_slice(&body).expect("parse json body");
        assert_eq!(json["code"], expected_code);
        assert!(json["error"].as_str().unwrap_or_default().contains(':'));
    }
}

#[test]
fn app_error_from_conversions_cover_io_git_and_json() {
    let io_error = std::io::Error::other("io conversion");
    let mapped_io: AppError = io_error.into();
    match mapped_io {
        AppError::IoError(message) => assert!(message.contains("io conversion")),
        other => panic!("expected IoError, got {other:?}"),
    }

    let mapped_git: AppError = git2::Error::from_str("git conversion").into();
    match mapped_git {
        AppError::GitError(message) => assert!(message.contains("git conversion")),
        other => panic!("expected GitError, got {other:?}"),
    }

    let bad_json =
        serde_json::from_str::<serde_json::Value>("{").expect_err("invalid json must fail");
    let mapped_json: AppError = bad_json.into();
    match mapped_json {
        AppError::InvalidRequest(message) => assert!(!message.is_empty()),
        other => panic!("expected InvalidRequest, got {other:?}"),
    }
}

#[tokio::test]
async fn registry_round_trip_and_not_found_paths() {
    let temp = tempdir().expect("tempdir");
    let path = temp.path().join("repos.json");
    let registry = RepoRegistry::new(path.clone())
        .await
        .expect("registry init");

    let listed = registry.list().await.expect("list empty repos");
    assert!(listed.is_empty());

    let repo = RepoInfo {
        name: "example".to_string(),
        url: "https://github.com/example/repo.git".to_string(),
        description: Some("desc".to_string()),
        default_branch: "main".to_string(),
        last_fetched: None,
        size_kb: 1,
        source: "github".to_string(),
    };

    registry.upsert(repo.clone()).await.expect("upsert repo");
    let loaded = registry.get("example").await.expect("get repo");
    assert_eq!(loaded.name, repo.name);

    registry.remove("example").await.expect("remove repo");
    let err = registry
        .get("example")
        .await
        .expect_err("repo should not exist");
    match err {
        AppError::NotFound(msg) => assert!(msg.contains("example")),
        other => panic!("expected NotFound, got {other:?}"),
    }

    let remove_err = registry
        .remove("missing")
        .await
        .expect_err("missing repo should error");
    match remove_err {
        AppError::NotFound(msg) => assert!(msg.contains("missing")),
        other => panic!("expected NotFound, got {other:?}"),
    }

    fs::write(&path, "{not-json}").expect("write malformed registry file");
    let malformed = registry
        .list()
        .await
        .expect_err("invalid registry should error");
    match malformed {
        AppError::InvalidRequest(_) => {}
        other => panic!("expected InvalidRequest, got {other:?}"),
    }
}

#[test]
fn git_utilities_cover_name_source_language_and_path_helpers() {
    assert_eq!(
        git::detect_repo_source("https://github.com/org/repo.git"),
        "github"
    );
    assert_eq!(
        git::detect_repo_source("https://gitlab.com/org/repo.git"),
        "gitlab"
    );
    assert_eq!(
        git::detect_repo_source("https://gitea.example/org/repo.git"),
        "generic"
    );

    let derived =
        git::derive_repo_name("https://github.com/Org/My.Repo.git", None).expect("derive from URL");
    assert_eq!(derived, "my-repo");

    let alias = git::derive_repo_name("https://example.com/foo", Some("My Alias"))
        .expect("derive from alias");
    assert_eq!(alias, "my-alias");

    let bad = git::sanitize_name("---").expect_err("all separators must fail");
    match bad {
        AppError::InvalidRequest(msg) => assert!(msg.contains("usable")),
        other => panic!("expected InvalidRequest, got {other:?}"),
    }

    let empty_name = git::sanitize_name("   ").expect_err("empty name must fail");
    match empty_name {
        AppError::InvalidRequest(msg) => assert!(msg.contains("cannot be empty")),
        other => panic!("expected InvalidRequest, got {other:?}"),
    }

    let derive_fail = git::derive_repo_name("", None).expect_err("invalid URL should fail");
    match derive_fail {
        AppError::InvalidRequest(msg) => {
            assert!(
                msg.contains("could not derive repository name") || msg.contains("cannot be empty"),
                "unexpected derive error message: {msg}"
            );
        }
        other => panic!("expected InvalidRequest, got {other:?}"),
    }

    assert_eq!(git::detect_language("src/main.rs"), "rust");
    assert_eq!(git::detect_language("pkg/index.ts"), "typescript");
    assert_eq!(git::detect_language("frontend/App.svelte"), "svelte");
    assert_eq!(git::detect_language("docs/README.md"), "markdown");
    assert_eq!(git::detect_language(".gitignore"), "text");
    assert_eq!(git::detect_language("Dockerfile"), "docker");
    assert_eq!(git::detect_language("deploy/Containerfile"), "docker");

    let path = git::repo_disk_path(std::path::Path::new("/tmp/repos"), "demo");
    assert!(path.ends_with("repos/demo"));
}

#[tokio::test]
async fn state_initializes_cache() {
    let temp = tempdir().expect("tempdir");
    let state = common::test_state(temp.path()).await;
    let key = "sample|main|".to_string();
    state.tree_cache.insert(key.clone(), vec![]).await;
    let cached = state.tree_cache.get(&key).await;
    assert!(cached.is_some());

    let language_key = "sample|main".to_string();
    state
        .language_cache
        .insert(
            language_key.clone(),
            vec![git::LanguageStat {
                language: "rust".to_string(),
                bytes: 128,
                percentage: 100.0,
            }],
        )
        .await;
    let cached_languages = state.language_cache.get(&language_key).await;
    assert!(cached_languages.is_some());
}
