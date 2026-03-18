/// Telegram API configuration.
#[derive(Debug, Clone)]
pub struct TelegramConfig {
    /// Telegram API ID (from my.telegram.org).
    pub api_id: i32,
    /// Telegram API hash.
    pub api_hash: String,
    /// Path to session file for persistent auth.
    pub session_path: String,
    /// Maximum upload size in bytes (Telegram limit: 2GB for premium, 50MB standard).
    pub max_upload_size: usize,
    /// Maximum concurrent uploads.
    pub max_concurrent_uploads: usize,
    /// Maximum concurrent downloads.
    pub max_concurrent_downloads: usize,
    /// Retry count for failed operations.
    pub max_retries: u32,
    /// Base delay between retries in milliseconds.
    pub retry_base_delay_ms: u64,
}

impl Default for TelegramConfig {
    fn default() -> Self {
        Self {
            api_id: 0,
            api_hash: String::new(),
            session_path: "tgcryptfs.session".into(),
            max_upload_size: 50 * 1024 * 1024, // 50MB conservative default
            max_concurrent_uploads: 3,
            max_concurrent_downloads: 5,
            max_retries: 3,
            retry_base_delay_ms: 1000,
        }
    }
}

/// Result of uploading a block to Telegram.
#[derive(Debug, Clone)]
pub struct UploadResult {
    /// Telegram message ID containing this block.
    pub message_id: i64,
    /// Size of the uploaded data.
    pub size: usize,
}

/// Result of downloading a block from Telegram.
#[derive(Debug, Clone)]
pub struct DownloadResult {
    /// The raw block data.
    pub data: Vec<u8>,
    /// Telegram message ID it was downloaded from.
    pub message_id: i64,
}

/// Authentication state.
#[derive(Debug, Clone, PartialEq)]
pub enum AuthState {
    /// Not authenticated at all.
    NotAuthenticated,
    /// Phone number submitted, waiting for code.
    AwaitingCode,
    /// Code submitted, waiting for 2FA password.
    AwaitingPassword,
    /// Fully authenticated.
    Authenticated,
}

/// Statistics for the Telegram transport.
#[derive(Debug, Clone, Default)]
pub struct TransportStats {
    pub blocks_uploaded: u64,
    pub blocks_downloaded: u64,
    pub bytes_uploaded: u64,
    pub bytes_downloaded: u64,
    pub upload_errors: u64,
    pub download_errors: u64,
    pub messages_deleted: u64,
}
