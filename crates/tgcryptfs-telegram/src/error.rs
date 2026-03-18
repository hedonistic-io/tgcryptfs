use thiserror::Error;

#[derive(Debug, Error)]
pub enum TelegramError {
    #[error("not connected")]
    NotConnected,

    #[error("authentication required")]
    AuthRequired,

    #[error("upload failed: {0}")]
    Upload(String),

    #[error("download failed: {0}")]
    Download(String),

    #[error("message not found: {0}")]
    MessageNotFound(i64),

    #[error("delete failed: {0}")]
    Delete(String),

    #[error("block too large: {size} bytes (max {max})")]
    BlockTooLarge { size: usize, max: usize },

    #[error("session error: {0}")]
    Session(String),

    #[error("rate limited, retry after {seconds}s")]
    RateLimited { seconds: u32 },

    #[error("telegram API error: {0}")]
    Api(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl TelegramError {
    /// Returns a user-facing suggestion for how to resolve this error.
    pub fn suggestion(&self) -> &'static str {
        match self {
            TelegramError::NotConnected => "Run `tgcryptfs auth login` to connect to Telegram",
            TelegramError::AuthRequired => {
                "Run `tgcryptfs auth login` to authenticate with your Telegram account"
            }
            TelegramError::Upload(_) => "Check your internet connection and try again",
            TelegramError::Download(_) => {
                "Check your internet connection; the message may have been deleted from Telegram"
            }
            TelegramError::MessageNotFound(_) => {
                "The block may have been deleted from the Telegram group"
            }
            TelegramError::Delete(_) => "You may lack admin permissions in the storage group",
            TelegramError::BlockTooLarge { .. } => "Reduce the volume's block size configuration",
            TelegramError::Session(_) => {
                "Try `tgcryptfs auth logout` then `tgcryptfs auth login` to reset the session"
            }
            TelegramError::RateLimited { seconds } => {
                if *seconds > 60 {
                    "Telegram is rate-limiting requests; wait and try again later"
                } else {
                    "Telegram is rate-limiting requests; retrying automatically"
                }
            }
            TelegramError::Api(_) => {
                "Check Telegram service status; this may be a temporary outage"
            }
            TelegramError::Io(_) => "Check file permissions and available disk space",
        }
    }
}

pub type Result<T> = std::result::Result<T, TelegramError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_variants_have_suggestions() {
        let errors: Vec<TelegramError> = vec![
            TelegramError::NotConnected,
            TelegramError::AuthRequired,
            TelegramError::Upload("test".into()),
            TelegramError::Download("test".into()),
            TelegramError::MessageNotFound(42),
            TelegramError::Delete("test".into()),
            TelegramError::BlockTooLarge { size: 100, max: 50 },
            TelegramError::Session("test".into()),
            TelegramError::RateLimited { seconds: 30 },
            TelegramError::Api("test".into()),
        ];

        for err in &errors {
            assert!(!err.suggestion().is_empty(), "Empty suggestion for: {err}");
        }
    }

    #[test]
    fn not_connected_suggests_login() {
        let err = TelegramError::NotConnected;
        assert!(err.suggestion().contains("login"));
    }

    #[test]
    fn rate_limit_long_wait_suggests_later() {
        let err = TelegramError::RateLimited { seconds: 120 };
        assert!(err.suggestion().contains("later"));
    }

    #[test]
    fn rate_limit_short_wait_suggests_auto_retry() {
        let err = TelegramError::RateLimited { seconds: 10 };
        assert!(err.suggestion().contains("automatically"));
    }
}
