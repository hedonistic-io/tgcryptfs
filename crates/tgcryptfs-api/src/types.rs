use serde::{Deserialize, Serialize};

/// Summary of a volume for listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeSummary {
    pub volume_id: String,
    pub display_name: String,
    pub created_at: i64,
    pub mounted: bool,
    pub mount_point: Option<String>,
    pub block_count: u64,
    pub total_size: u64,
}

/// Volume creation request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateVolumeRequest {
    pub name: Option<String>,
    pub password: String,
    pub block_size: Option<usize>,
}

/// Volume creation response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateVolumeResponse {
    pub volume_id: String,
    pub display_name: String,
    pub sentence_ref: String,
}

/// Mount request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountRequest {
    pub volume_id: String,
    pub password: String,
    pub mount_point: String,
    pub read_only: bool,
}

/// System status summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatus {
    pub version: String,
    pub telegram_connected: bool,
    pub volumes_mounted: usize,
    pub total_volumes: usize,
    pub cache_entries: usize,
    pub cache_size_bytes: u64,
    pub deadman_armed: bool,
    pub uptime_secs: u64,
}

/// Telegram transport statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportStatus {
    pub connected: bool,
    pub blocks_uploaded: u64,
    pub blocks_downloaded: u64,
    pub bytes_uploaded: u64,
    pub bytes_downloaded: u64,
    pub upload_errors: u64,
    pub download_errors: u64,
}

/// Sharing invite response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteResponse {
    pub invite_code: String,
    pub invite_id: String,
    pub expires_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_summary_serialization() {
        let summary = VolumeSummary {
            volume_id: "vol-1".into(),
            display_name: "Test Volume".into(),
            created_at: 1000000,
            mounted: false,
            mount_point: None,
            block_count: 42,
            total_size: 1024 * 1024,
        };
        let json = serde_json::to_string(&summary).unwrap();
        let deser: VolumeSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.volume_id, "vol-1");
        assert_eq!(deser.block_count, 42);
    }

    #[test]
    fn system_status_serialization() {
        let status = SystemStatus {
            version: "0.1.0".into(),
            telegram_connected: true,
            volumes_mounted: 2,
            total_volumes: 5,
            cache_entries: 100,
            cache_size_bytes: 1024 * 1024 * 50,
            deadman_armed: false,
            uptime_secs: 3600,
        };
        let json = serde_json::to_string(&status).unwrap();
        let deser: SystemStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.volumes_mounted, 2);
        assert!(deser.telegram_connected);
    }

    #[test]
    fn create_volume_request_deser() {
        let json = r#"{"password":"secret","block_size":524288}"#;
        let req: CreateVolumeRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.password, "secret");
        assert_eq!(req.block_size, Some(524288));
        assert!(req.name.is_none());
    }
}
