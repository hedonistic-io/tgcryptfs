use thiserror::Error;

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("block not in cache: {0}")]
    Miss(String),

    #[error("cache IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("encryption error: {0}")]
    Encryption(String),

    #[error("decryption error: {0}")]
    Decryption(String),

    #[error("cache full, eviction failed")]
    EvictionFailed,
}

impl CacheError {
    /// Returns a user-facing suggestion for how to resolve this error.
    pub fn suggestion(&self) -> &'static str {
        match self {
            CacheError::Miss(_) => {
                "Block will be fetched from Telegram; this is normal on first access"
            }
            CacheError::Io(_) => "Check available disk space and cache directory permissions",
            CacheError::Encryption(_) => {
                "Cache encryption key may be invalid; try remounting the volume"
            }
            CacheError::Decryption(_) => {
                "Cached block is corrupted; it will be re-fetched from Telegram"
            }
            CacheError::EvictionFailed => {
                "Increase cache size or free disk space in the cache directory"
            }
        }
    }
}

pub type Result<T> = std::result::Result<T, CacheError>;
