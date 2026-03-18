use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::RwLock;

use tgcryptfs_core::volume::manager;

use crate::error::{ApiError, Result};
use crate::types::{CreateVolumeResponse, VolumeSummary};

/// Tracks a mounted volume's runtime state.
struct MountedVolume {
    _volume_id: String,
    _display_name: String,
    mount_point: PathBuf,
}

/// Volume management service.
pub struct VolumeService {
    base_dir: PathBuf,
    mounted: Arc<RwLock<HashMap<String, MountedVolume>>>,
}

impl VolumeService {
    /// Create a new volume service with the given base directory.
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            base_dir,
            mounted: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new volume.
    pub async fn create(&self, name: Option<&str>, password: &str) -> Result<CreateVolumeResponse> {
        if password.len() < 8 {
            return Err(ApiError::InvalidArgument(
                "password must be at least 8 characters".into(),
            ));
        }

        let result = manager::create_volume(name, password.as_bytes(), &self.base_dir)
            .map_err(|e| ApiError::Crypto(format!("create volume: {e}")))?;

        let volume_id = result.config.volume_id.to_string();
        let display_name = result.config.display_name.clone();

        // Encode root key as sentence reference
        let wordlists =
            core::array::from_fn(tgcryptfs_core::sentence::wordlists::placeholder_wordlist);
        let sentence = tgcryptfs_core::sentence::encode::encode_ref_string(
            result.hierarchy.root.as_bytes(),
            &wordlists,
        )
        .map_err(|e| ApiError::Crypto(format!("encode sentence: {e}")))?;

        tracing::info!(volume_id = %volume_id, name = %display_name, "volume created");

        Ok(CreateVolumeResponse {
            volume_id,
            display_name,
            sentence_ref: sentence,
        })
    }

    /// Open an existing volume with password.
    pub async fn open(&self, volume_id: &str, password: &str) -> Result<()> {
        let _result = manager::open_volume(volume_id, password.as_bytes(), &self.base_dir)
            .map_err(|e| ApiError::Crypto(format!("open volume: {e}")))?;

        tracing::info!(volume_id = %volume_id, "volume opened");
        Ok(())
    }

    /// List all volumes.
    pub async fn list(&self) -> Result<Vec<VolumeSummary>> {
        let configs = manager::list_volumes(&self.base_dir)
            .map_err(|e| ApiError::Storage(format!("list volumes: {e}")))?;

        let mounted = self.mounted.read().await;
        let mut summaries = Vec::new();

        for config in configs {
            let vid = config.volume_id.to_string();
            let is_mounted = mounted.contains_key(&vid);
            let mount_point = mounted
                .get(&vid)
                .map(|m| m.mount_point.display().to_string());

            summaries.push(VolumeSummary {
                volume_id: vid,
                display_name: config.display_name,
                created_at: 0,
                mounted: is_mounted,
                mount_point,
                block_count: 0,
                total_size: 0,
            });
        }

        Ok(summaries)
    }

    /// Delete a volume.
    pub async fn delete(&self, volume_id: &str) -> Result<()> {
        let mounted = self.mounted.read().await;
        if mounted.contains_key(volume_id) {
            return Err(ApiError::VolumeIsMounted(volume_id.into()));
        }
        drop(mounted);

        manager::delete_volume(volume_id, &self.base_dir)
            .map_err(|e| ApiError::Storage(format!("delete volume: {e}")))?;

        tracing::info!(volume_id = %volume_id, "volume deleted");
        Ok(())
    }

    /// Get the base directory.
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Check if a volume is mounted.
    pub async fn is_mounted(&self, volume_id: &str) -> bool {
        self.mounted.read().await.contains_key(volume_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn service(dir: &TempDir) -> VolumeService {
        VolumeService::new(dir.path().to_path_buf())
    }

    #[tokio::test]
    async fn create_volume() {
        let dir = TempDir::new().unwrap();
        let svc = service(&dir);
        let resp = svc.create(Some("test-vol"), "password123").await.unwrap();
        assert_eq!(resp.display_name, "test-vol");
        assert!(!resp.volume_id.is_empty());
        assert!(!resp.sentence_ref.is_empty());
    }

    #[tokio::test]
    async fn create_short_password_rejected() {
        let dir = TempDir::new().unwrap();
        let svc = service(&dir);
        let result = svc.create(None, "short").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn create_and_list() {
        let dir = TempDir::new().unwrap();
        let svc = service(&dir);

        let r1 = svc.create(Some("vol-a"), "password123").await.unwrap();
        let r2 = svc.create(Some("vol-b"), "password456").await.unwrap();

        let vols = svc.list().await.unwrap();
        assert_eq!(vols.len(), 2);
        let ids: Vec<&str> = vols.iter().map(|v| v.volume_id.as_str()).collect();
        assert!(ids.contains(&r1.volume_id.as_str()));
        assert!(ids.contains(&r2.volume_id.as_str()));
    }

    #[tokio::test]
    async fn create_and_open() {
        let dir = TempDir::new().unwrap();
        let svc = service(&dir);
        let resp = svc.create(Some("open-test"), "password123").await.unwrap();
        svc.open(&resp.volume_id, "password123").await.unwrap();
    }

    #[tokio::test]
    async fn delete_volume() {
        let dir = TempDir::new().unwrap();
        let svc = service(&dir);
        let resp = svc.create(Some("del-test"), "password123").await.unwrap();

        svc.delete(&resp.volume_id).await.unwrap();

        let vols = svc.list().await.unwrap();
        assert!(vols.is_empty());
    }

    #[tokio::test]
    async fn open_nonexistent() {
        let dir = TempDir::new().unwrap();
        let svc = service(&dir);
        let result = svc.open("nonexistent", "password").await;
        assert!(result.is_err());
    }
}
