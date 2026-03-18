use axum::extract::State;
use axum::Json;

use tgcryptfs_deadman::config::DeadmanConfig;

use crate::error::ApiError;
use crate::server::state::AppState;

/// GET /api/v1/deadman/status
pub async fn status(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    let status = state.inner.deadman.status().await;

    Ok(Json(serde_json::json!({
        "configured": status.configured,
        "armed": status.armed,
        "check_interval_secs": status.check_interval_secs,
        "grace_period_secs": status.grace_period_secs,
        "trigger_count": status.trigger_count,
        "config_path": status.config_path,
    })))
}

/// POST /api/v1/deadman/arm
pub async fn arm(
    State(state): State<AppState>,
    body: Option<Json<serde_json::Value>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Load config from body or default location
    let config = if let Some(Json(body)) = body {
        if let Some(config_json) = body.get("config") {
            serde_json::from_value::<DeadmanConfig>(config_json.clone())
                .map_err(|e| ApiError::InvalidArgument(format!("invalid config: {e}")))?
        } else {
            load_config_from_disk()?
        }
    } else {
        load_config_from_disk()?
    };

    state.inner.deadman.arm(config).await?;

    Ok(Json(serde_json::json!({
        "status": "armed",
        "message": "Deadman switch armed. Daemon running in background.",
    })))
}

/// POST /api/v1/deadman/disarm
pub async fn disarm(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    state.inner.deadman.disarm().await?;

    Ok(Json(serde_json::json!({
        "status": "disarmed",
        "message": "Deadman switch disarmed. Daemon will stop at next check.",
    })))
}

fn load_config_from_disk() -> Result<DeadmanConfig, ApiError> {
    let config_path = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("tgcryptfs")
        .join("deadman.json");

    if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| ApiError::Internal(format!("read deadman config: {e}")))?;
        serde_json::from_str(&content)
            .map_err(|e| ApiError::Internal(format!("parse deadman config: {e}")))
    } else {
        Ok(DeadmanConfig::default())
    }
}
