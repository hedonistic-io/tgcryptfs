use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rusqlite::Connection;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use tgcryptfs_core::crypto::keys::SymmetricKey;
use tgcryptfs_core::volume::{config::VolumeConfig, manager};
use tgcryptfs_store::migrations::initialize_database;
use tgcryptfs_store::opaque_schema::OpaqueSchema;
use tgcryptfs_store::schema::logical_tables;

use crate::error::{ApiError, Result};

/// An open volume session with live database connection and derived keys.
///
/// Not auto-derived Debug because `Connection` and `JoinHandle` don't implement it.
pub struct VolumeSession {
    pub volume_id: String,
    pub display_name: String,
    pub config: VolumeConfig,
    pub conn: Arc<std::sync::Mutex<Connection>>,
    pub schema: OpaqueSchema,
    pub meta_key: SymmetricKey,
    pub data_key: SymmetricKey,
    pub mount_handle: std::sync::Mutex<Option<JoinHandle<()>>>,
    pub mount_point: std::sync::Mutex<Option<PathBuf>>,
}

impl std::fmt::Debug for VolumeSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VolumeSession")
            .field("volume_id", &self.volume_id)
            .field("display_name", &self.display_name)
            .field("is_mounted", &self.is_mounted())
            .finish()
    }
}

impl VolumeSession {
    /// Check if this session's volume is currently mounted.
    pub fn is_mounted(&self) -> bool {
        self.mount_handle
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|h| !h.is_finished())
    }

    /// Get the current mount point, if mounted.
    pub fn mount_point(&self) -> Option<PathBuf> {
        self.mount_point.lock().unwrap().clone()
    }
}

/// Manages open volume sessions.
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<String, Arc<VolumeSession>>>>,
    base_dir: PathBuf,
}

impl SessionManager {
    /// Create a new session manager.
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            base_dir,
        }
    }

    /// Open a volume session by decrypting with the given password.
    pub async fn open(&self, volume_id: &str, password: &str) -> Result<Arc<VolumeSession>> {
        // Check if already open
        {
            let sessions = self.sessions.read().await;
            if sessions.contains_key(volume_id) {
                return Err(ApiError::SessionAlreadyOpen(volume_id.to_string()));
            }
        }

        // Open the volume (CPU-bound key derivation)
        let vid = volume_id.to_string();
        let pwd = password.to_string();
        let base = self.base_dir.clone();

        let result =
            tokio::task::spawn_blocking(move || manager::open_volume(&vid, pwd.as_bytes(), &base))
                .await
                .map_err(|e| ApiError::Internal(format!("spawn_blocking: {e}")))?
                .map_err(|e| ApiError::Crypto(format!("open volume: {e}")))?;

        // Open database and initialize schema
        let schema_key = result.hierarchy.schema.clone();
        let meta_key = result.hierarchy.meta.clone();
        let data_key = result.hierarchy.data.clone();
        let db_path = result.paths.db_path.clone();

        let conn = tokio::task::spawn_blocking(move || {
            let conn = Connection::open(&db_path)
                .map_err(|e| ApiError::Storage(format!("open db: {e}")))?;
            let schema = OpaqueSchema::new(schema_key, &logical_tables());
            initialize_database(&conn, &schema)
                .map_err(|e| ApiError::Storage(format!("init db: {e}")))?;
            Ok::<_, ApiError>((conn, schema))
        })
        .await
        .map_err(|e| ApiError::Internal(format!("spawn_blocking: {e}")))??;

        let session = Arc::new(VolumeSession {
            volume_id: volume_id.to_string(),
            display_name: result.config.display_name.clone(),
            config: result.config,
            conn: Arc::new(std::sync::Mutex::new(conn.0)),
            schema: conn.1,
            meta_key,
            data_key,
            mount_handle: std::sync::Mutex::new(None),
            mount_point: std::sync::Mutex::new(None),
        });

        self.sessions
            .write()
            .await
            .insert(volume_id.to_string(), Arc::clone(&session));

        tracing::info!(volume_id = %volume_id, "session opened");
        Ok(session)
    }

    /// Close a volume session, unmounting if needed.
    pub async fn close(&self, volume_id: &str) -> Result<()> {
        let session = self
            .sessions
            .write()
            .await
            .remove(volume_id)
            .ok_or_else(|| ApiError::SessionNotOpen(volume_id.to_string()))?;

        // If mounted, abort the mount handle
        let handle = session.mount_handle.lock().unwrap().take();
        if let Some(h) = handle {
            h.abort();
            tracing::info!(volume_id = %volume_id, "mount task aborted");
        }

        tracing::info!(volume_id = %volume_id, "session closed");
        Ok(())
    }

    /// Get an open session by volume ID.
    pub async fn get(&self, volume_id: &str) -> Result<Arc<VolumeSession>> {
        self.sessions
            .read()
            .await
            .get(volume_id)
            .cloned()
            .ok_or_else(|| ApiError::SessionNotOpen(volume_id.to_string()))
    }

    /// List all open session volume IDs.
    pub async fn list_open(&self) -> Vec<String> {
        self.sessions.read().await.keys().cloned().collect()
    }

    /// Check if a volume has an open session.
    pub async fn is_open(&self, volume_id: &str) -> bool {
        self.sessions.read().await.contains_key(volume_id)
    }

    /// Get the base directory.
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_manager(dir: &TempDir) -> SessionManager {
        SessionManager::new(dir.path().to_path_buf())
    }

    fn create_test_volume(dir: &Path) -> (String, String) {
        let result = manager::create_volume(Some("test-vol"), b"password123", dir).unwrap();
        (result.config.volume_id.to_string(), "password123".into())
    }

    #[tokio::test]
    async fn open_and_get_session() {
        let dir = TempDir::new().unwrap();
        let mgr = make_manager(&dir);
        let (vid, pwd) = create_test_volume(dir.path());

        let session = mgr.open(&vid, &pwd).await.unwrap();
        assert_eq!(session.volume_id, vid);
        assert_eq!(session.display_name, "test-vol");
        assert!(!session.is_mounted());

        let got = mgr.get(&vid).await.unwrap();
        assert_eq!(got.volume_id, vid);
    }

    #[tokio::test]
    async fn open_twice_errors() {
        let dir = TempDir::new().unwrap();
        let mgr = make_manager(&dir);
        let (vid, pwd) = create_test_volume(dir.path());

        mgr.open(&vid, &pwd).await.unwrap();
        let err = mgr.open(&vid, &pwd).await.unwrap_err();
        assert!(matches!(err, ApiError::SessionAlreadyOpen(_)));
    }

    #[tokio::test]
    async fn close_session() {
        let dir = TempDir::new().unwrap();
        let mgr = make_manager(&dir);
        let (vid, pwd) = create_test_volume(dir.path());

        mgr.open(&vid, &pwd).await.unwrap();
        assert!(mgr.is_open(&vid).await);

        mgr.close(&vid).await.unwrap();
        assert!(!mgr.is_open(&vid).await);
    }

    #[tokio::test]
    async fn close_nonexistent_errors() {
        let dir = TempDir::new().unwrap();
        let mgr = make_manager(&dir);

        let err = mgr.close("nonexistent").await.unwrap_err();
        assert!(matches!(err, ApiError::SessionNotOpen(_)));
    }

    #[tokio::test]
    async fn get_nonexistent_errors() {
        let dir = TempDir::new().unwrap();
        let mgr = make_manager(&dir);

        let err = mgr.get("nonexistent").await.unwrap_err();
        assert!(matches!(err, ApiError::SessionNotOpen(_)));
    }

    #[tokio::test]
    async fn list_open_sessions() {
        let dir = TempDir::new().unwrap();
        let mgr = make_manager(&dir);

        assert!(mgr.list_open().await.is_empty());

        let (vid, pwd) = create_test_volume(dir.path());
        mgr.open(&vid, &pwd).await.unwrap();

        let open = mgr.list_open().await;
        assert_eq!(open.len(), 1);
        assert_eq!(open[0], vid);
    }

    #[tokio::test]
    async fn open_produces_session_with_keys() {
        // Note: open_volume derives keys from password without validation;
        // wrong password produces wrong keys, detected only at decrypt time.
        let dir = TempDir::new().unwrap();
        let mgr = make_manager(&dir);
        let (vid, pwd) = create_test_volume(dir.path());

        let session = mgr.open(&vid, &pwd).await.unwrap();
        // Verify the session has valid keys (non-zero)
        assert_ne!(session.meta_key.as_bytes(), &[0u8; 32]);
        assert_ne!(session.data_key.as_bytes(), &[0u8; 32]);
    }

    #[tokio::test]
    async fn open_nonexistent_volume() {
        let dir = TempDir::new().unwrap();
        let mgr = make_manager(&dir);

        let err = mgr.open("nonexistent", "password").await.unwrap_err();
        assert!(matches!(err, ApiError::Crypto(_)));
    }

    #[tokio::test]
    async fn reopen_after_close() {
        let dir = TempDir::new().unwrap();
        let mgr = make_manager(&dir);
        let (vid, pwd) = create_test_volume(dir.path());

        mgr.open(&vid, &pwd).await.unwrap();
        mgr.close(&vid).await.unwrap();

        // Should be able to reopen
        let session = mgr.open(&vid, &pwd).await.unwrap();
        assert_eq!(session.volume_id, vid);
    }
}
