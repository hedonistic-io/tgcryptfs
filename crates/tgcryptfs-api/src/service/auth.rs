use std::path::PathBuf;

use tgcryptfs_telegram::types::TelegramConfig;

use crate::error::Result;

/// Authentication service for Telegram.
pub struct AuthService {
    config: TelegramConfig,
}

impl AuthService {
    /// Create with the given Telegram config.
    pub fn new(config: TelegramConfig) -> Self {
        Self { config }
    }

    /// Create with API credentials and default session path.
    pub fn with_credentials(api_id: i32, api_hash: String, session_path: PathBuf) -> Self {
        Self {
            config: TelegramConfig {
                api_id,
                api_hash,
                session_path: session_path.to_string_lossy().into_owned(),
                ..Default::default()
            },
        }
    }

    /// Check if a session file exists.
    pub fn has_session(&self) -> bool {
        std::path::Path::new(&self.config.session_path).exists()
    }

    /// Get the session file path.
    pub fn session_path(&self) -> &str {
        &self.config.session_path
    }

    /// Remove the session file (logout).
    pub fn remove_session(&self) -> Result<()> {
        let path = std::path::Path::new(&self.config.session_path);
        if path.exists() {
            std::fs::remove_file(path)?;
            tracing::info!("session file removed");
        }
        Ok(())
    }

    /// Get the telegram config.
    pub fn config(&self) -> &TelegramConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn no_session_initially() {
        let dir = TempDir::new().unwrap();
        let session_path = dir.path().join("test.session");
        let svc = AuthService::with_credentials(12345, "abc123".into(), session_path);
        assert!(!svc.has_session());
    }

    #[test]
    fn session_detection() {
        let dir = TempDir::new().unwrap();
        let session_path = dir.path().join("test.session");
        std::fs::write(&session_path, b"fake-session").unwrap();

        let svc = AuthService::with_credentials(12345, "abc123".into(), session_path);
        assert!(svc.has_session());
    }

    #[test]
    fn remove_session() {
        let dir = TempDir::new().unwrap();
        let session_path = dir.path().join("test.session");
        std::fs::write(&session_path, b"fake-session").unwrap();

        let svc = AuthService::with_credentials(12345, "abc123".into(), session_path);
        assert!(svc.has_session());
        svc.remove_session().unwrap();
        assert!(!svc.has_session());
    }

    #[test]
    fn remove_nonexistent_session_ok() {
        let dir = TempDir::new().unwrap();
        let session_path = dir.path().join("nonexistent.session");
        let svc = AuthService::with_credentials(12345, "abc123".into(), session_path);
        svc.remove_session().unwrap();
    }
}
