use axum::extract::{Path, State};
use axum::Json;

use tgcryptfs_sharing::access::AccessLevel;

use crate::error::ApiError;
use crate::server::state::AppState;
use crate::service::sharing::SharingService;

/// GET /api/v1/shares/volume/:volume_id
pub async fn list(
    State(state): State<AppState>,
    Path(volume_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = state.inner.sessions.get(&volume_id).await?;
    let shares = SharingService::list_shares(&session)?;

    let share_list: Vec<serde_json::Value> = shares
        .iter()
        .map(|s| {
            serde_json::json!({
                "user_id": s.user_id,
                "display_name": s.display_name,
                "access_level": format!("{:?}", s.access_level),
                "granted_at": s.granted_at,
                "active": s.active,
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "volume_id": volume_id,
        "shares": share_list,
    })))
}

/// POST /api/v1/shares
pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> Result<(http::StatusCode, Json<serde_json::Value>), ApiError> {
    let volume_id = body
        .get("volume_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::InvalidArgument("volume_id is required".into()))?;

    let user_id = body
        .get("user_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::InvalidArgument("user_id is required".into()))?;

    let access_str = body
        .get("access_level")
        .and_then(|v| v.as_str())
        .unwrap_or("read-only");

    let access_level = match access_str {
        "read-only" | "ReadOnly" => AccessLevel::ReadOnly,
        "read-write" | "ReadWrite" => AccessLevel::ReadWrite,
        "admin" | "Admin" => AccessLevel::Admin,
        _ => {
            return Err(ApiError::InvalidArgument(format!(
                "invalid access level: {access_str}"
            )))
        }
    };

    let session = state.inner.sessions.get(volume_id).await?;
    let share = SharingService::create_share(&session, user_id, access_level)?;

    Ok((
        http::StatusCode::CREATED,
        Json(serde_json::json!({
            "status": "created",
            "volume_id": volume_id,
            "user_id": share.user_id,
            "access_level": format!("{:?}", share.access_level),
            "granted_at": share.granted_at,
        })),
    ))
}

/// DELETE /api/v1/shares/:id — revoke a share by user ID (passed as path param).
pub async fn revoke(
    State(state): State<AppState>,
    Path(share_user_id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let volume_id = body
        .get("volume_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::InvalidArgument("volume_id is required in body".into()))?;

    let session = state.inner.sessions.get(volume_id).await?;
    SharingService::revoke_share(&session, &share_user_id)?;

    Ok(Json(serde_json::json!({
        "status": "revoked",
        "user_id": share_user_id,
        "volume_id": volume_id,
    })))
}

/// POST /api/v1/invites
pub async fn create_invite(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> Result<(http::StatusCode, Json<serde_json::Value>), ApiError> {
    let volume_id = body
        .get("volume_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::InvalidArgument("volume_id is required".into()))?;

    let access_str = body
        .get("access_level")
        .and_then(|v| v.as_str())
        .unwrap_or("read-only");

    let access_level = match access_str {
        "read-only" | "ReadOnly" => AccessLevel::ReadOnly,
        "read-write" | "ReadWrite" => AccessLevel::ReadWrite,
        "admin" | "Admin" => AccessLevel::Admin,
        _ => {
            return Err(ApiError::InvalidArgument(format!(
                "invalid access level: {access_str}"
            )))
        }
    };

    let max_uses = body
        .get("max_uses")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0) as u32;

    let expires_at = body
        .get("expires_at")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(0);

    let session = state.inner.sessions.get(volume_id).await?;
    let invite = SharingService::create_invite(&session, access_level, max_uses, expires_at)?;

    Ok((
        http::StatusCode::CREATED,
        Json(serde_json::json!({
            "invite_id": invite.invite_id,
            "volume_id": invite.volume_id,
            "access_level": format!("{:?}", invite.access_level),
            "max_uses": invite.max_uses,
            "expires_at": invite.expires_at,
        })),
    ))
}

/// POST /api/v1/invites/:code/accept
pub async fn accept_invite(
    State(state): State<AppState>,
    Path(invite_id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let volume_id = body
        .get("volume_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::InvalidArgument("volume_id is required".into()))?;

    let user_id = body
        .get("user_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::InvalidArgument("user_id is required".into()))?;

    let session = state.inner.sessions.get(volume_id).await?;
    let share = SharingService::accept_invite(&session, &invite_id, user_id)?;

    Ok(Json(serde_json::json!({
        "status": "accepted",
        "invite_id": invite_id,
        "user_id": share.user_id,
        "access_level": format!("{:?}", share.access_level),
    })))
}
