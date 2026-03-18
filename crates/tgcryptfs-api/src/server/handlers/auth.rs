use axum::extract::State;
use axum::Json;

use crate::error::ApiError;
use crate::server::state::AppState;

/// POST /api/v1/auth/session — Start login flow.
///
/// In the current implementation, this checks for an existing session.
/// Full multi-step auth (phone -> code -> 2FA) requires a connected
/// Telegram client, which is initiated via the CLI `tgcryptfs auth login`.
pub async fn login_start(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if state.inner.auth.has_session() {
        return Ok(Json(serde_json::json!({
            "status": "authenticated",
        })));
    }

    Ok(Json(serde_json::json!({
        "status": "not_authenticated",
        "message": "Use the CLI `tgcryptfs auth login` for interactive authentication",
    })))
}

/// GET /api/v1/auth/status
pub async fn status(State(state): State<AppState>) -> Json<serde_json::Value> {
    let authenticated = state.inner.auth.has_session();
    Json(serde_json::json!({
        "authenticated": authenticated,
    }))
}

/// DELETE /api/v1/auth/session — Logout.
pub async fn logout(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    state.inner.auth.remove_session()?;
    Ok(Json(serde_json::json!({
        "status": "logged_out",
    })))
}
