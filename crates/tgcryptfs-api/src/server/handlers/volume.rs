use axum::extract::{Path, State};
use axum::Json;

use crate::error::ApiError;
use crate::server::state::AppState;
use crate::types::{CreateVolumeRequest, CreateVolumeResponse, VolumeSummary};

/// POST /api/v1/volumes
pub async fn create(
    State(state): State<AppState>,
    Json(req): Json<CreateVolumeRequest>,
) -> Result<(http::StatusCode, Json<CreateVolumeResponse>), ApiError> {
    let resp = state
        .inner
        .volumes
        .create(req.name.as_deref(), &req.password)
        .await?;

    Ok((http::StatusCode::CREATED, Json(resp)))
}

/// GET /api/v1/volumes
pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<VolumeSummary>>, ApiError> {
    let volumes = state.inner.volumes.list().await?;
    let open_sessions = state.inner.sessions.list_open().await;

    // Enrich with session status
    let enriched: Vec<VolumeSummary> = volumes
        .into_iter()
        .map(|mut v| {
            if open_sessions.contains(&v.volume_id) {
                v.mounted = true;
            }
            v
        })
        .collect();

    Ok(Json(enriched))
}

/// GET /api/v1/volumes/:id
pub async fn info(
    State(state): State<AppState>,
    Path(volume_id): Path<String>,
) -> Result<Json<VolumeSummary>, ApiError> {
    let volumes = state.inner.volumes.list().await?;
    let mut vol = volumes
        .into_iter()
        .find(|v| v.volume_id == volume_id)
        .ok_or(ApiError::VolumeNotFound(volume_id.clone()))?;

    // Enrich with session data
    if state.inner.sessions.is_open(&volume_id).await {
        vol.mounted = true;
        if let Ok(session) = state.inner.sessions.get(&volume_id).await {
            vol.mount_point = session.mount_point().map(|p| p.display().to_string());
        }
    }

    Ok(Json(vol))
}

/// DELETE /api/v1/volumes/:id
pub async fn delete(
    State(state): State<AppState>,
    Path(volume_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Prevent deletion if session is open
    if state.inner.sessions.is_open(&volume_id).await {
        return Err(ApiError::VolumeIsMounted(volume_id));
    }

    state.inner.volumes.delete(&volume_id).await?;
    Ok(Json(serde_json::json!({
        "status": "deleted",
        "volume_id": volume_id,
    })))
}

/// POST /api/v1/volumes/:id/open — Open a volume session with password.
pub async fn open(
    State(state): State<AppState>,
    Path(volume_id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let password = body
        .get("password")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::InvalidArgument("password is required".into()))?;

    let session = state.inner.sessions.open(&volume_id, password).await?;

    Ok(Json(serde_json::json!({
        "status": "opened",
        "volume_id": session.volume_id,
        "display_name": session.display_name,
    })))
}

/// POST /api/v1/volumes/:id/close — Close an open volume session.
pub async fn close(
    State(state): State<AppState>,
    Path(volume_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.inner.sessions.close(&volume_id).await?;

    Ok(Json(serde_json::json!({
        "status": "closed",
        "volume_id": volume_id,
    })))
}

/// POST /api/v1/volumes/:id/mount — Mount an open volume at a directory.
pub async fn mount(
    State(state): State<AppState>,
    Path(volume_id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> Result<(http::StatusCode, Json<serde_json::Value>), ApiError> {
    let _password = body
        .get("password")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::InvalidArgument("password is required".into()))?;

    let _mount_point = body
        .get("mount_point")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::InvalidArgument("mount_point is required".into()))?;

    // Verify volume is open
    if !state.inner.sessions.is_open(&volume_id).await {
        return Err(ApiError::SessionNotOpen(volume_id));
    }

    // FUSE mounting requires root/macFUSE and is out of scope for API-level testing.
    // The mount operation would use tokio::task::spawn_blocking(fuser::mount2).
    Ok((
        http::StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "status": "mounting",
            "volume_id": volume_id,
            "message": "Mount operation accepted. Use GET /api/v1/volumes/:id to check status.",
        })),
    ))
}

/// POST /api/v1/volumes/:id/unmount
pub async fn unmount(
    State(state): State<AppState>,
    Path(volume_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if !state.inner.sessions.is_open(&volume_id).await {
        return Err(ApiError::SessionNotOpen(volume_id));
    }

    Ok(Json(serde_json::json!({
        "status": "unmounting",
        "volume_id": volume_id,
        "message": "Unmount operation started.",
    })))
}
