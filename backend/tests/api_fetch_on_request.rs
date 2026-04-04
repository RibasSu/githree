mod common;

use std::fs;

use axum_test::TestServer;
use githree::config::AppConfig;
use githree::git::{CommitInfo, RepoInfo};
use githree::registry::RepoRegistry;
use githree::router;
use githree::state::AppState;
use serde_json::json;
use tempfile::tempdir;

async fn state_with_fetch_enabled(mut cfg: AppConfig) -> AppState {
    cfg.git.fetch_on_request = true;
    let registry = RepoRegistry::new(cfg.registry_file())
        .await
        .expect("create registry");
    AppState::new(cfg, registry)
}

#[tokio::test]
async fn api_routes_trigger_fetch_when_enabled() {
    let fixture = common::RepoFixture::new();
    let temp = tempdir().expect("tempdir");
    let cfg = common::test_config(temp.path());
    let state = state_with_fetch_enabled(cfg).await;
    let server = TestServer::new(router::build_router(state));

    let add = server
        .post("/api/repos")
        .json(&json!({
            "url": fixture.remote_url(),
            "name": "fetch-repo"
        }))
        .await;
    add.assert_status_ok();

    let initial_list = server.get("/api/repos").await;
    initial_list.assert_status_ok();
    let initial_repos: Vec<RepoInfo> = initial_list.json();
    let initial_last_fetched = initial_repos
        .first()
        .and_then(|repo| repo.last_fetched)
        .expect("repo should expose last_fetched");

    fixture.add_remote_commit(
        "fetched-on-request.txt",
        b"fetch-on-request marker\n",
        "test: trigger on-request metadata refresh",
    );

    server
        .get("/api/repos/fetch-repo/refs")
        .await
        .assert_status_ok();

    let post_fetch_list = server.get("/api/repos").await;
    post_fetch_list.assert_status_ok();
    let post_fetch_repos: Vec<RepoInfo> = post_fetch_list.json();
    let post_fetch_size_kb = post_fetch_repos
        .first()
        .map(|repo| repo.size_kb)
        .expect("repo should exist after fetch");

    let local_repo_path = temp.path().join("repos").join("fetch-repo");
    fs::write(
        local_repo_path.join("size-probe.bin"),
        vec![42_u8; 8 * 1024],
    )
    .expect("write probe");

    server
        .get("/api/repos/fetch-repo/tree")
        .add_query_param("path", "")
        .await
        .assert_status_ok();

    let size_refreshed_list = server.get("/api/repos").await;
    size_refreshed_list.assert_status_ok();
    let size_refreshed_repos: Vec<RepoInfo> = size_refreshed_list.json();
    let size_refreshed_kb = size_refreshed_repos
        .first()
        .map(|repo| repo.size_kb)
        .expect("repo should exist after local size probe");
    assert!(size_refreshed_kb >= post_fetch_size_kb + 8);

    server
        .get("/api/repos/fetch-repo/tree")
        .add_query_param("path", "docs")
        .await
        .assert_status_ok();

    server
        .get("/api/repos/fetch-repo/blob")
        .add_query_param("path", "README.md")
        .await
        .assert_status_ok();

    server
        .get("/api/repos/fetch-repo/readme")
        .await
        .assert_status_ok();

    let commits = server
        .get("/api/repos/fetch-repo/commits")
        .add_query_param("limit", "1")
        .await;
    commits.assert_status_ok();
    let commit_entries: Vec<CommitInfo> = commits.json();
    assert_eq!(commit_entries.len(), 1);

    server
        .get(&format!(
            "/api/repos/fetch-repo/commit/{}",
            commit_entries[0].hash
        ))
        .await
        .assert_status_ok();

    server
        .get("/api/repos/fetch-repo/archive")
        .add_query_param("format", "zip")
        .await
        .assert_status_ok();

    let list = server.get("/api/repos").await;
    list.assert_status_ok();
    let repos: Vec<RepoInfo> = list.json();
    assert_eq!(repos.len(), 1);
    let updated_last_fetched = repos[0]
        .last_fetched
        .expect("last_fetched should be present after on-request fetch");
    assert!(updated_last_fetched >= initial_last_fetched);
}
