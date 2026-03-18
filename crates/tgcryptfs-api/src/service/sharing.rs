use std::sync::Arc;

use tgcryptfs_sharing::access::{AccessLevel, ShareRecord};
use tgcryptfs_sharing::invite::Invite;
use tgcryptfs_store::sharing_store::SharingStore;

use crate::error::{ApiError, Result};
use crate::service::session::VolumeSession;

/// Get the current Unix timestamp in seconds.
fn unix_now() -> Result<i64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .map_err(|e| ApiError::Internal(format!("system clock error: {e}")))
}

/// Service wrapping sharing operations through an open volume session.
pub struct SharingService;

impl SharingService {
    /// List all shares for an open volume session.
    pub fn list_shares(session: &Arc<VolumeSession>) -> Result<Vec<ShareRecord>> {
        let conn = session
            .conn
            .lock()
            .map_err(|e| ApiError::Internal(format!("lock poisoned: {e}")))?;
        let store = SharingStore::new(&conn, &session.schema, &session.meta_key);
        store
            .list_shares(&session.volume_id)
            .map_err(|e| ApiError::Storage(format!("list shares: {e}")))
    }

    /// Create a new share in an open volume session.
    pub fn create_share(
        session: &Arc<VolumeSession>,
        user_id: &str,
        access_level: AccessLevel,
    ) -> Result<ShareRecord> {
        let now = unix_now()?;

        let share = ShareRecord {
            user_id: user_id.to_string(),
            telegram_user_id: 0,
            display_name: user_id.to_string(),
            access_level,
            wrapped_key: Vec::new(), // populated during key exchange
            granted_at: now,
            active: true,
        };

        let conn = session
            .conn
            .lock()
            .map_err(|e| ApiError::Internal(format!("lock poisoned: {e}")))?;
        let store = SharingStore::new(&conn, &session.schema, &session.meta_key);
        store
            .insert_share(&session.volume_id, &share)
            .map_err(|e| ApiError::Storage(format!("create share: {e}")))?;

        Ok(share)
    }

    /// Revoke a share by user ID.
    pub fn revoke_share(session: &Arc<VolumeSession>, user_id: &str) -> Result<()> {
        let conn = session
            .conn
            .lock()
            .map_err(|e| ApiError::Internal(format!("lock poisoned: {e}")))?;
        let store = SharingStore::new(&conn, &session.schema, &session.meta_key);
        store
            .revoke_share(user_id)
            .map_err(|e| ApiError::Storage(format!("revoke share: {e}")))
    }

    /// Create an invite for the volume.
    pub fn create_invite(
        session: &Arc<VolumeSession>,
        access_level: AccessLevel,
        max_uses: u32,
        expires_at: i64,
    ) -> Result<Invite> {
        let invite = Invite::new(
            session.volume_id.clone(),
            "api".into(),
            access_level,
            expires_at,
            max_uses,
        );

        let conn = session
            .conn
            .lock()
            .map_err(|e| ApiError::Internal(format!("lock poisoned: {e}")))?;
        let store = SharingStore::new(&conn, &session.schema, &session.meta_key);
        store
            .insert_invite(&invite)
            .map_err(|e| ApiError::Storage(format!("create invite: {e}")))?;

        Ok(invite)
    }

    /// Accept an invite by ID — use the invite and create a share.
    pub fn accept_invite(
        session: &Arc<VolumeSession>,
        invite_id: &str,
        user_id: &str,
    ) -> Result<ShareRecord> {
        let conn = session
            .conn
            .lock()
            .map_err(|e| ApiError::Internal(format!("lock poisoned: {e}")))?;
        let store = SharingStore::new(&conn, &session.schema, &session.meta_key);

        let invite = store
            .use_invite(invite_id)
            .map_err(|e| ApiError::Storage(format!("use invite: {e}")))?
            .ok_or_else(|| {
                ApiError::InvalidArgument(format!("invite not found or expired: {invite_id}"))
            })?;

        let now = unix_now()?;

        let share = ShareRecord {
            user_id: user_id.to_string(),
            telegram_user_id: 0,
            display_name: user_id.to_string(),
            access_level: invite.access_level,
            wrapped_key: Vec::new(),
            granted_at: now,
            active: true,
        };

        store
            .insert_share(&session.volume_id, &share)
            .map_err(|e| ApiError::Storage(format!("create share from invite: {e}")))?;

        Ok(share)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::session::SessionManager;
    use tempfile::TempDir;
    use tgcryptfs_core::volume::manager;

    fn setup() -> (TempDir, SessionManager, String) {
        let dir = TempDir::new().unwrap();
        let result =
            manager::create_volume(Some("share-test"), b"password123", dir.path()).unwrap();
        let vid = result.config.volume_id.to_string();
        let mgr = SessionManager::new(dir.path().to_path_buf());
        (dir, mgr, vid)
    }

    #[tokio::test]
    async fn create_and_list_shares() {
        let (_dir, mgr, vid) = setup();
        let session = mgr.open(&vid, "password123").await.unwrap();

        SharingService::create_share(&session, "alice", AccessLevel::ReadOnly).unwrap();
        SharingService::create_share(&session, "bob", AccessLevel::ReadWrite).unwrap();

        let shares = SharingService::list_shares(&session).unwrap();
        assert_eq!(shares.len(), 2);
    }

    #[tokio::test]
    async fn revoke_share() {
        let (_dir, mgr, vid) = setup();
        let session = mgr.open(&vid, "password123").await.unwrap();

        SharingService::create_share(&session, "alice", AccessLevel::ReadOnly).unwrap();
        SharingService::revoke_share(&session, "alice").unwrap();

        let shares = SharingService::list_shares(&session).unwrap();
        // revoke_share sets active=false; list_shares may still return the record
        // but it should be marked inactive
        let active: Vec<_> = shares.iter().filter(|s| s.active).collect();
        assert!(active.is_empty());
    }

    #[tokio::test]
    async fn create_and_accept_invite() {
        let (_dir, mgr, vid) = setup();
        let session = mgr.open(&vid, "password123").await.unwrap();

        let invite = SharingService::create_invite(&session, AccessLevel::ReadOnly, 5, 0).unwrap();

        let share = SharingService::accept_invite(&session, &invite.invite_id, "charlie").unwrap();
        assert_eq!(share.user_id, "charlie");
        assert_eq!(share.access_level, AccessLevel::ReadOnly);
    }

    #[tokio::test]
    async fn empty_shares_list() {
        let (_dir, mgr, vid) = setup();
        let session = mgr.open(&vid, "password123").await.unwrap();

        let shares = SharingService::list_shares(&session).unwrap();
        assert!(shares.is_empty());
    }

    #[tokio::test]
    async fn accept_nonexistent_invite_fails() {
        let (_dir, mgr, vid) = setup();
        let session = mgr.open(&vid, "password123").await.unwrap();

        let err = SharingService::accept_invite(&session, "nonexistent", "user").unwrap_err();
        assert!(matches!(err, ApiError::InvalidArgument(_)));
    }
}
