mod common;

use std::fs;

use axum_test::TestServer;
use githree::registry::RepoRegistry;
use githree::router;
use githree::state::AppState;
use tempfile::tempdir;

#[tokio::test]
async fn router_serves_static_index_when_static_dir_exists() {
    let temp = tempdir().expect("tempdir");
    let static_dir = temp.path().join("static");
    fs::create_dir_all(&static_dir).expect("create static dir");
    fs::write(
        static_dir.join("index.html"),
        "<!doctype html><html><body>githree-ui</body></html>",
    )
    .expect("write static index");

    let mut cfg = common::test_config(temp.path());
    cfg.storage.static_dir = static_dir.to_string_lossy().to_string();

    let registry = RepoRegistry::new(cfg.registry_file())
        .await
        .expect("create registry");
    let state = AppState::new(cfg, registry);

    let server = TestServer::new(router::build_router(state));

    let root = server.get("/").await;
    root.assert_status_ok();
    assert!(root.text().contains("githree-ui"));

    let spa_fallback = server.get("/some/unknown/route").await;
    spa_fallback.assert_status(axum::http::StatusCode::NOT_FOUND);
    assert!(spa_fallback.text().contains("githree-ui"));

    let api = server.get("/api/repos").await;
    api.assert_status_ok();
}
