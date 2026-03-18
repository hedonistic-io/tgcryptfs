use axum::extract::State;
use axum::Json;

use crate::server::state::AppState;
use crate::service::cache::CacheService;
use crate::types::SystemStatus;

/// GET /api/v1/status
pub async fn status(State(state): State<AppState>) -> Json<SystemStatus> {
    let volumes = state.inner.volumes.list().await.unwrap_or_default();
    let open_sessions = state.inner.sessions.list_open().await;

    let mounted_count = open_sessions.len();
    let total_count = volumes.len();

    let cache_stats = CacheService::aggregate_stats(&state.inner.sessions).await;
    let deadman_armed = state.inner.deadman.is_armed().await;

    let status = state.inner.system.status(
        state.inner.auth.has_session(),
        mounted_count,
        total_count,
        cache_stats.total_entries,
        cache_stats.total_size_bytes,
        deadman_armed,
    );

    Json(status)
}

/// GET /api/v1/version
pub async fn version(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "version": state.inner.system.version(),
    }))
}
