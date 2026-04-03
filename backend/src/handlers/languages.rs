use axum::{Json, extract::Path, extract::Query, extract::State};
use serde::Deserialize;
use tokio::task::spawn_blocking;
use tracing::instrument;

use crate::error::AppError;
use crate::git::{self, LanguageStat};
use crate::handlers::sync::{ensure_repo_ready, join_error};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct LanguagesQuery {
    #[serde(rename = "ref")]
    pub ref_name: Option<String>,
}

#[instrument(skip(state, query), fields(repo_name = %name))]
pub async fn get_languages(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(query): Query<LanguagesQuery>,
) -> Result<Json<Vec<LanguageStat>>, AppError> {
    let repo = state.registry.get(&name).await?;
    let ref_name = query
        .ref_name
        .unwrap_or_else(|| repo.default_branch.clone());
    let local_path = git::repo_disk_path(&state.config.repos_dir(), &name);

    ensure_repo_ready(state.clone(), &name, local_path.clone(), repo.url.clone()).await?;

    let cache_key = format!("{name}|{ref_name}");
    if let Some(cached) = state.language_cache.get(&cache_key).await {
        return Ok(Json(cached));
    }

    let stats = spawn_blocking(move || git::browse::language_stats(&local_path, &ref_name))
        .await
        .map_err(join_error)??;
    state.language_cache.insert(cache_key, stats.clone()).await;
    Ok(Json(stats))
}
