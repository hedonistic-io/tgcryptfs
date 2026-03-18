use serde::{Deserialize, Serialize};

/// Access level for a shared volume user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessLevel {
    /// Can only read files.
    ReadOnly,
    /// Can read and write files.
    ReadWrite,
    /// Full access including user management.
    Admin,
}

impl AccessLevel {
    pub fn can_read(&self) -> bool {
        true
    }

    pub fn can_write(&self) -> bool {
        matches!(self, AccessLevel::ReadWrite | AccessLevel::Admin)
    }

    pub fn can_manage_users(&self) -> bool {
        matches!(self, AccessLevel::Admin)
    }
}

/// A user's share record for a volume.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareRecord {
    /// User identifier.
    pub user_id: String,
    /// Telegram user ID.
    pub telegram_user_id: i64,
    /// Display name.
    pub display_name: String,
    /// Access level.
    pub access_level: AccessLevel,
    /// Wrapped data key (encrypted with user's ML-KEM public key).
    pub wrapped_key: Vec<u8>,
    /// When the share was granted.
    pub granted_at: i64,
    /// Whether this share is currently active.
    pub active: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_levels() {
        assert!(AccessLevel::ReadOnly.can_read());
        assert!(!AccessLevel::ReadOnly.can_write());
        assert!(!AccessLevel::ReadOnly.can_manage_users());

        assert!(AccessLevel::ReadWrite.can_read());
        assert!(AccessLevel::ReadWrite.can_write());
        assert!(!AccessLevel::ReadWrite.can_manage_users());

        assert!(AccessLevel::Admin.can_read());
        assert!(AccessLevel::Admin.can_write());
        assert!(AccessLevel::Admin.can_manage_users());
    }

    #[test]
    fn serialization() {
        let record = ShareRecord {
            user_id: "user-1".into(),
            telegram_user_id: 12345,
            display_name: "Alice".into(),
            access_level: AccessLevel::ReadWrite,
            wrapped_key: vec![0x42; 32],
            granted_at: 1000000,
            active: true,
        };

        let json = serde_json::to_string(&record).unwrap();
        let deserialized: ShareRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.user_id, "user-1");
        assert_eq!(deserialized.access_level, AccessLevel::ReadWrite);
    }
}
