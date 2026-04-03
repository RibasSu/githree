use axum::Router;
use axum::routing::{delete, get, post};
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};

use crate::handlers;
use crate::state::AppState;

pub fn build_router(state: AppState) -> Router {
    let static_dir = state.config.static_dir();
    let index_file = static_dir.join("index.html");

    let api_router = Router::new()
        .route("/settings", get(handlers::repos::get_settings))
        .route(
            "/repos",
            post(handlers::repos::add_repo).get(handlers::repos::list_repos),
        )
        .route("/repos/{name}", delete(handlers::repos::delete_repo))
        .route("/repos/{name}/fetch", post(handlers::repos::fetch_repo))
        .route("/repos/{name}/refs", get(handlers::refs::get_refs))
        .route(
            "/repos/{name}/languages",
            get(handlers::languages::get_languages),
        )
        .route("/repos/{name}/tree", get(handlers::tree::get_tree))
        .route("/repos/{name}/blob", get(handlers::blob::get_blob))
        .route("/repos/{name}/raw", get(handlers::blob::get_raw))
        .route("/repos/{name}/readme", get(handlers::blob::get_readme))
        .route(
            "/repos/{name}/commits",
            get(handlers::commits::list_commits),
        )
        .route(
            "/repos/{name}/commit/{hash}",
            get(handlers::commits::get_commit_detail),
        )
        .route("/repos/{name}/archive", get(handlers::archive::get_archive));

    let app = Router::new()
        .nest("/api", api_router)
        .layer(CorsLayer::permissive())
        .with_state(state);

    if static_dir.exists() {
        return app.fallback_service(
            ServeDir::new(static_dir).not_found_service(ServeFile::new(index_file)),
        );
    }

    app
}
