use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::block::compress::CompressionAlgorithm;
use crate::crypto::keys::Argon2Params;

/// Volume configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeConfig {
    /// Unique volume identifier
    pub volume_id: Uuid,
    /// Human-readable volume name
    pub display_name: String,
    /// Random Telegram group name ("noun verb adjective")
    pub group_name: String,
    /// Telegram group/chat ID
    pub telegram_group_id: i64,
    /// Current encryption epoch
    pub current_epoch: u32,
    /// HKDF salt (32 bytes, generated once)
    pub salt: [u8; 32],
    /// KDF parameters
    pub kdf_params: Argon2Params,
    /// Block size configuration
    pub block_config: BlockConfig,
    /// Default mutability policy ID
    pub default_policy_id: u32,
    /// Cache configuration
    pub cache_config: CacheConfig,
    /// Creation timestamp (seconds since epoch)
    pub created_at: i64,
    /// BLAKE3 hash of a derived verification key, for password checking.
    /// Hex-encoded. Populated on create; verified on open.
    #[serde(default)]
    pub password_verification_hash: Option<String>,
}

/// Block size configuration for CDC and storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockConfig {
    /// Minimum block size (default: 4 KB)
    pub min_block_size: u64,
    /// Maximum block size (default: 50 MB)
    pub max_block_size: u64,
    /// Target average block size for CDC (default: 1 MB)
    pub target_block_size: u64,
    /// Compression algorithm
    pub compression: CompressionAlgorithm,
}

impl Default for BlockConfig {
    fn default() -> Self {
        Self {
            min_block_size: 4 * 1024,
            max_block_size: 50 * 1024 * 1024,
            target_block_size: 1024 * 1024,
            compression: CompressionAlgorithm::Lz4,
        }
    }
}

/// Local cache configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Enable block cache
    pub enabled: bool,
    /// Maximum cache size in bytes (default: 1 GB)
    pub max_size: u64,
    /// Enable metadata cache
    pub metadata_cache: bool,
    /// Prefetch depth (0 = disabled)
    pub prefetch_depth: u32,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_size: 1024 * 1024 * 1024,
            metadata_cache: true,
            prefetch_depth: 4,
        }
    }
}

impl VolumeConfig {
    /// Increment the epoch counter and return the new epoch value.
    pub fn increment_epoch(&mut self) -> u32 {
        self.current_epoch += 1;
        self.current_epoch
    }

    /// Create a new volume configuration with sensible defaults.
    pub fn new(display_name: String, group_name: String) -> Self {
        let mut salt = [0u8; 32];
        use rand::RngCore;
        rand::rngs::OsRng.fill_bytes(&mut salt);

        Self {
            volume_id: Uuid::new_v4(),
            display_name,
            group_name,
            telegram_group_id: 0,
            current_epoch: 0,
            salt,
            kdf_params: Argon2Params::default(),
            block_config: BlockConfig::default(),
            default_policy_id: 0,
            cache_config: CacheConfig::default(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
            password_verification_hash: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_volume_has_unique_id() {
        let v1 = VolumeConfig::new("vol1".into(), "noun verb adj".into());
        let v2 = VolumeConfig::new("vol2".into(), "noun verb adj".into());
        assert_ne!(v1.volume_id, v2.volume_id);
    }

    #[test]
    fn new_volume_has_random_salt() {
        let v1 = VolumeConfig::new("vol1".into(), "a b c".into());
        let v2 = VolumeConfig::new("vol2".into(), "d e f".into());
        assert_ne!(v1.salt, v2.salt);
    }

    #[test]
    fn default_block_config() {
        let bc = BlockConfig::default();
        assert_eq!(bc.min_block_size, 4096);
        assert_eq!(bc.max_block_size, 50 * 1024 * 1024);
        assert_eq!(bc.target_block_size, 1024 * 1024);
    }

    #[test]
    fn increment_epoch() {
        let mut v = VolumeConfig::new("test".into(), "a b c".into());
        assert_eq!(v.current_epoch, 0);
        assert_eq!(v.increment_epoch(), 1);
        assert_eq!(v.current_epoch, 1);
        assert_eq!(v.increment_epoch(), 2);
        assert_eq!(v.current_epoch, 2);
    }
}
