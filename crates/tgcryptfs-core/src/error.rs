use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("encryption failed: {0}")]
    Encryption(String),

    #[error("decryption failed: {0}")]
    Decryption(String),

    #[error("key derivation failed: {0}")]
    KeyDerivation(String),

    #[error("invalid key material: {0}")]
    InvalidKey(String),

    #[error("integrity check failed: hash mismatch")]
    IntegrityError { expected: String, actual: String },

    #[error("block format error: {0}")]
    BlockFormat(String),

    #[error("compression error: {0}")]
    Compression(String),

    #[error("decompression error: {0}")]
    Decompression(String),

    #[error("CDC chunking error: {0}")]
    Chunking(String),

    #[error("policy error: {0}")]
    Policy(String),

    #[error("sentence encoding error: {0}")]
    SentenceEncoding(String),

    #[error("invalid inode: {0}")]
    InvalidInode(String),

    #[error("manifest error: {0}")]
    Manifest(String),

    #[error("snapshot error: {0}")]
    Snapshot(String),

    #[error("volume error: {0}")]
    Volume(String),

    #[error("serialization error: {0}")]
    Serialization(String),
}

impl CoreError {
    /// Returns a user-facing suggestion for how to resolve this error.
    pub fn suggestion(&self) -> &'static str {
        match self {
            CoreError::Encryption(_) => {
                "Check that the key material is valid and the plaintext is not corrupted"
            }
            CoreError::Decryption(_) => {
                "Verify your password is correct, or the volume may be corrupted"
            }
            CoreError::KeyDerivation(_) => {
                "Ensure sufficient system memory for Argon2 KDF parameters"
            }
            CoreError::InvalidKey(_) => {
                "Key material may be truncated or from an incompatible version"
            }
            CoreError::IntegrityError { .. } => {
                "Data may have been tampered with or corrupted in transit"
            }
            CoreError::BlockFormat(_) => {
                "Block data is malformed; the volume may need repair"
            }
            CoreError::Compression(_) => {
                "Input data may be too large or corrupted"
            }
            CoreError::Decompression(_) => {
                "Stored block is corrupted or was compressed with an unsupported algorithm"
            }
            CoreError::Chunking(_) => {
                "Input data may be empty or exceed maximum chunk size"
            }
            CoreError::Policy(_) => {
                "Review volume policy configuration with `tgcryptfs volume info`"
            }
            CoreError::SentenceEncoding(_) => {
                "Sentence reference may be from an incompatible version"
            }
            CoreError::InvalidInode(_) => {
                "The file or directory metadata is corrupted; consider restoring from a snapshot"
            }
            CoreError::Manifest(_) => {
                "File manifest is inconsistent; try unmounting and remounting the volume"
            }
            CoreError::Snapshot(_) => {
                "Snapshot metadata is corrupted; list available snapshots with `tgcryptfs snapshot list`"
            }
            CoreError::Volume(_) => {
                "Check that the volume directory exists and is accessible"
            }
            CoreError::Serialization(_) => {
                "Data format may be incompatible with this version"
            }
        }
    }
}

pub type Result<T> = std::result::Result<T, CoreError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_variants_have_suggestions() {
        let errors: Vec<CoreError> = vec![
            CoreError::Encryption("test".into()),
            CoreError::Decryption("test".into()),
            CoreError::KeyDerivation("test".into()),
            CoreError::InvalidKey("test".into()),
            CoreError::IntegrityError {
                expected: "a".into(),
                actual: "b".into(),
            },
            CoreError::BlockFormat("test".into()),
            CoreError::Compression("test".into()),
            CoreError::Decompression("test".into()),
            CoreError::Chunking("test".into()),
            CoreError::Policy("test".into()),
            CoreError::SentenceEncoding("test".into()),
            CoreError::InvalidInode("test".into()),
            CoreError::Manifest("test".into()),
            CoreError::Snapshot("test".into()),
            CoreError::Volume("test".into()),
            CoreError::Serialization("test".into()),
        ];

        for err in &errors {
            let suggestion = err.suggestion();
            assert!(!suggestion.is_empty(), "Empty suggestion for: {err}");
        }
    }

    #[test]
    fn decryption_error_suggests_password_check() {
        let err = CoreError::Decryption("aead failure".into());
        assert!(
            err.suggestion().contains("password"),
            "Decryption suggestion should mention password"
        );
    }

    #[test]
    fn integrity_error_does_not_leak_hashes() {
        let err = CoreError::IntegrityError {
            expected: "abc123deadbeef".into(),
            actual: "def456cafebabe".into(),
        };
        let display = format!("{err}");
        // Hash values should NOT appear in the display string
        assert!(!display.contains("abc123deadbeef"));
        assert!(!display.contains("def456cafebabe"));
        assert!(display.contains("hash mismatch"));
    }
}
