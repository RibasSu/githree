mod common;

use axum_test::TestServer;
use githree::git::{
    BlobResponse, CommitDetail, CommitInfo, LanguageStat, ReadmeResponse, RefsResponse, RepoInfo,
    TreeEntry,
};
use githree::registry::RepoRegistry;
use githree::router;
use githree::state::AppState;
use serde_json::json;
use tempfile::tempdir;

#[tokio::test]
async fn api_repository_lifecycle_and_browsing_routes_work() {
    let fixture = common::RepoFixture::new();
    let temp = tempdir().expect("tempdir");
    let state = common::test_state(temp.path()).await;
    let server = TestServer::new(router::build_router(state)).expect("create test server");

    let empty = server.get("/api/repos").await;
    empty.assert_status_ok();
    let empty_repos: Vec<RepoInfo> = empty.json();
    assert!(empty_repos.is_empty());

    let add = server
        .post("/api/repos")
        .json(&json!({
            "url": fixture.remote_url(),
            "name": "sample-repo"
        }))
        .await;
    add.assert_status_ok();
    let added: RepoInfo = add.json();
    assert_eq!(added.name, "sample-repo");
    assert_eq!(added.source, "generic");

    let list_after_add = server.get("/api/repos").await;
    list_after_add.assert_status_ok();
    let repos: Vec<RepoInfo> = list_after_add.json();
    assert_eq!(repos.len(), 1);

    let refs = server.get("/api/repos/sample-repo/refs").await;
    refs.assert_status_ok();
    let refs_payload: RefsResponse = refs.json();
    assert!(refs_payload.branches.iter().any(|branch| branch == "main"));

    let languages = server
        .get("/api/repos/sample-repo/languages")
        .add_query_param("ref", "main")
        .await;
    languages.assert_status_ok();
    let language_payload: Vec<LanguageStat> = languages.json();
    assert!(!language_payload.is_empty());
    assert!(
        language_payload
            .iter()
            .any(|entry| entry.language == "rust" && entry.bytes > 0)
    );

    let tree = server
        .get("/api/repos/sample-repo/tree")
        .add_query_param("ref", "main")
        .await;
    tree.assert_status_ok();
    let tree_entries: Vec<TreeEntry> = tree.json();
    assert!(tree_entries.iter().any(|entry| entry.name == "README.md"));

    let cached_tree = server
        .get("/api/repos/sample-repo/tree")
        .add_query_param("ref", "main")
        .await;
    cached_tree.assert_status_ok();
    let cached_tree_entries: Vec<TreeEntry> = cached_tree.json();
    assert_eq!(cached_tree_entries.len(), tree_entries.len());

    let blob = server
        .get("/api/repos/sample-repo/blob")
        .add_query_param("ref", "main")
        .add_query_param("path", "README.md")
        .await;
    blob.assert_status_ok();
    let blob_payload: BlobResponse = blob.json();
    assert_eq!(blob_payload.encoding, "utf8");
    assert!(blob_payload.content.contains("Sample Repo"));
    assert!(!blob_payload.is_truncated);

    let binary_blob = server
        .get("/api/repos/sample-repo/blob")
        .add_query_param("path", "binary.bin")
        .await;
    binary_blob.assert_status_ok();
    let binary_payload: BlobResponse = binary_blob.json();
    assert!(binary_payload.is_binary);

    let raw = server
        .get("/api/repos/sample-repo/raw")
        .add_query_param("path", "README.md")
        .await;
    raw.assert_status_ok();
    assert!(raw.contains_header("content-disposition"));
    assert!(raw.contains_header("content-type"));
    assert!(!raw.as_bytes().is_empty());

    let readme = server
        .get("/api/repos/sample-repo/readme")
        .add_query_param("ref", "main")
        .await;
    readme.assert_status_ok();
    let readme_payload: ReadmeResponse = readme.json();
    assert_eq!(readme_payload.filename, "README.md");

    let commits = server
        .get("/api/repos/sample-repo/commits")
        .add_query_param("ref", "main")
        .add_query_param("limit", "2")
        .await;
    commits.assert_status_ok();
    let commit_entries: Vec<CommitInfo> = commits.json();
    assert_eq!(commit_entries.len(), 2);

    let commit_detail = server
        .get(&format!(
            "/api/repos/sample-repo/commit/{}",
            commit_entries[0].hash
        ))
        .await;
    commit_detail.assert_status_ok();
    let detail_payload: CommitDetail = commit_detail.json();
    assert_eq!(detail_payload.commit.hash, commit_entries[0].hash);

    let archive_tgz = server
        .get("/api/repos/sample-repo/archive")
        .add_query_param("ref", "main")
        .add_query_param("format", "tar.gz")
        .await;
    archive_tgz.assert_status_ok();
    assert!(archive_tgz.contains_header("content-disposition"));
    assert!(!archive_tgz.as_bytes().is_empty());

    let archive_zip = server
        .get("/api/repos/sample-repo/archive")
        .add_query_param("format", "zip")
        .await;
    archive_zip.assert_status_ok();
    assert!(!archive_zip.as_bytes().is_empty());

    let archive_default = server.get("/api/repos/sample-repo/archive").await;
    archive_default.assert_status_ok();
    assert!(!archive_default.as_bytes().is_empty());

    let new_hash = fixture.add_remote_commit(
        "fetched-through-api.txt",
        b"new file from remote update\n",
        "feat: remote update for fetch endpoint",
    );

    let fetched = server.post("/api/repos/sample-repo/fetch").await;
    fetched.assert_status_ok();
    let fetched_repo: RepoInfo = fetched.json();
    assert_eq!(fetched_repo.name, "sample-repo");

    let fetched_tree = server
        .get("/api/repos/sample-repo/tree")
        .add_query_param("ref", "refs/remotes/origin/main")
        .await;
    fetched_tree.assert_status_ok();
    let fetched_entries: Vec<TreeEntry> = fetched_tree.json();
    assert!(
        fetched_entries
            .iter()
            .any(|entry| entry.name == "fetched-through-api.txt")
    );

    let fetched_commit = server
        .get(&format!("/api/repos/sample-repo/commit/{new_hash}"))
        .await;
    fetched_commit.assert_status_ok();

    let delete = server.delete("/api/repos/sample-repo").await;
    delete.assert_status(axum::http::StatusCode::NO_CONTENT);

    let after_delete = server.get("/api/repos").await;
    let final_repos: Vec<RepoInfo> = after_delete.json();
    assert!(final_repos.is_empty());
}

#[tokio::test]
async fn api_can_disable_web_repo_management() {
    let fixture = common::RepoFixture::new();
    let temp = tempdir().expect("tempdir");
    let mut cfg = common::test_config(temp.path());
    cfg.features.web_repo_management = false;
    let registry = RepoRegistry::new(cfg.registry_file())
        .await
        .expect("create repo registry");
    let state = AppState::new(cfg, registry);
    let server = TestServer::new(router::build_router(state)).expect("create test server");

    let settings = server.get("/api/settings").await;
    settings.assert_status_ok();
    let payload: serde_json::Value = settings.json();
    assert_eq!(payload["web_repo_management"], false);
    assert_eq!(payload["app_name"], "Githree");
    assert_eq!(payload["caddy_enabled"], false);

    let add = server
        .post("/api/repos")
        .json(&json!({
            "url": fixture.remote_url(),
            "name": "disabled-repo"
        }))
        .await;
    add.assert_status(axum::http::StatusCode::FORBIDDEN);
    let add_json: serde_json::Value = add.json();
    assert_eq!(add_json["code"], "FORBIDDEN");
}

#[tokio::test]
async fn api_error_paths_return_expected_codes() {
    let fixture = common::RepoFixture::new();
    let temp = tempdir().expect("tempdir");
    let state = common::test_state(temp.path()).await;
    let server = TestServer::new(router::build_router(state)).expect("create test server");

    let bad_add = server
        .post("/api/repos")
        .json(&json!({ "url": "   " }))
        .await;
    bad_add.assert_status_bad_request();
    let bad_add_json: serde_json::Value = bad_add.json();
    assert_eq!(bad_add_json["code"], "INVALID_REQUEST");

    let missing_repo_tree = server.get("/api/repos/unknown/tree").await;
    missing_repo_tree.assert_status_not_found();

    let add = server
        .post("/api/repos")
        .json(&json!({
            "url": fixture.remote_url(),
            "name": "errors-repo"
        }))
        .await;
    add.assert_status_ok();

    let bad_archive = server
        .get("/api/repos/errors-repo/archive")
        .add_query_param("format", "rar")
        .await;
    bad_archive.assert_status_bad_request();
    let bad_archive_json: serde_json::Value = bad_archive.json();
    assert_eq!(bad_archive_json["code"], "INVALID_REQUEST");

    let missing_blob = server
        .get("/api/repos/errors-repo/blob")
        .add_query_param("path", "missing.file")
        .await;
    missing_blob.assert_status_not_found();

    let delete_missing = server.delete("/api/repos/not-there").await;
    delete_missing.assert_status_not_found();
}
