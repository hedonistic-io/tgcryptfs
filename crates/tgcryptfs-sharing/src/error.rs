use thiserror::Error;

#[derive(Debug, Error)]
pub enum SharingError {
    #[error("key exchange failed: {0}")]
    KeyExchange(String),

    #[error("user not found: {0}")]
    UserNotFound(String),

    #[error("access denied: {0}")]
    AccessDenied(String),

    #[error("invalid invite: {0}")]
    InvalidInvite(String),

    #[error("crypto error: {0}")]
    Crypto(String),
}

impl SharingError {
    /// Returns a user-facing suggestion for how to resolve this error.
    pub fn suggestion(&self) -> &'static str {
        match self {
            SharingError::KeyExchange(_) => {
                "The recipient's public key may be invalid or from an incompatible version"
            }
            SharingError::UserNotFound(_) => {
                "Verify the user ID; they may need to create an account first"
            }
            SharingError::AccessDenied(_) => {
                "You do not have permission for this operation; contact the volume owner"
            }
            SharingError::InvalidInvite(_) => {
                "The invite code may have expired or already been used"
            }
            SharingError::Crypto(_) => {
                "Cryptographic operation failed; ensure both parties use compatible versions"
            }
        }
    }
}

pub type Result<T> = std::result::Result<T, SharingError>;
