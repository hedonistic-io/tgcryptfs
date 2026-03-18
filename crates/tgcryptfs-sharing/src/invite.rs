use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::access::AccessLevel;
use crate::error::{Result, SharingError};

/// An invite token for sharing a volume with another user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invite {
    /// Unique invite ID.
    pub invite_id: String,
    /// Volume ID this invite grants access to.
    pub volume_id: String,
    /// Who created the invite.
    pub created_by: String,
    /// Access level the invite grants.
    pub access_level: AccessLevel,
    /// Unix timestamp when the invite was created.
    pub created_at: i64,
    /// Unix timestamp when the invite expires (0 = never).
    pub expires_at: i64,
    /// Maximum number of uses (0 = unlimited).
    pub max_uses: u32,
    /// Current use count.
    pub use_count: u32,
    /// Whether the invite has been revoked.
    pub revoked: bool,
    /// The wrapped data key, encrypted for the invite recipient.
    /// This is set when the invite is accepted (not at creation time for
    /// multi-use invites), or at creation time for single-use invites
    /// where the recipient's public key is known.
    pub wrapped_key: Option<Vec<u8>>,
}

impl Invite {
    /// Create a new invite.
    pub fn new(
        volume_id: String,
        created_by: String,
        access_level: AccessLevel,
        expires_at: i64,
        max_uses: u32,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        Self {
            invite_id: Uuid::new_v4().to_string(),
            volume_id,
            created_by,
            access_level,
            created_at: now,
            expires_at,
            max_uses,
            use_count: 0,
            revoked: false,
            wrapped_key: None,
        }
    }

    /// Check if this invite is still valid.
    pub fn is_valid(&self) -> bool {
        if self.revoked {
            return false;
        }
        if self.max_uses > 0 && self.use_count >= self.max_uses {
            return false;
        }
        if self.expires_at > 0 {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            if now > self.expires_at {
                return false;
            }
        }
        true
    }

    /// Try to use this invite. Returns error if invalid.
    pub fn try_use(&mut self) -> Result<()> {
        if !self.is_valid() {
            return Err(SharingError::InvalidInvite(format!(
                "invite {} is no longer valid",
                self.invite_id
            )));
        }
        self.use_count += 1;
        Ok(())
    }

    /// Revoke this invite.
    pub fn revoke(&mut self) {
        self.revoked = true;
    }
}

/// Compact invite code that can be shared out-of-band (e.g. via Telegram message).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteCode {
    /// The invite ID.
    pub invite_id: String,
    /// Volume ID (so the recipient knows which volume).
    pub volume_id: String,
    /// HMAC of the invite for integrity verification.
    pub hmac: Vec<u8>,
}

impl InviteCode {
    /// Encode as a base64url string for easy sharing.
    pub fn encode(&self) -> Result<String> {
        let json = serde_json::to_vec(self)
            .map_err(|e| SharingError::InvalidInvite(format!("encode: {e}")))?;
        Ok(base64_encode(&json))
    }

    /// Decode from a base64url string.
    pub fn decode(encoded: &str) -> Result<Self> {
        let bytes = base64_decode(encoded)
            .map_err(|e| SharingError::InvalidInvite(format!("decode base64: {e}")))?;
        serde_json::from_slice(&bytes)
            .map_err(|e| SharingError::InvalidInvite(format!("decode json: {e}")))
    }
}

/// Simple base64url encoding (no padding).
fn base64_encode(data: &[u8]) -> String {
    use base64ct::{Base64UrlUnpadded, Encoding};
    Base64UrlUnpadded::encode_string(data)
}

/// Simple base64url decoding (no padding).
fn base64_decode(s: &str) -> std::result::Result<Vec<u8>, base64ct::Error> {
    use base64ct::{Base64UrlUnpadded, Encoding};
    Base64UrlUnpadded::decode_vec(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_invite() {
        let invite = Invite::new(
            "vol-1".into(),
            "user-1".into(),
            AccessLevel::ReadWrite,
            0,
            0,
        );
        assert!(invite.is_valid());
        assert_eq!(invite.use_count, 0);
        assert!(!invite.revoked);
    }

    #[test]
    fn revoke_invite() {
        let mut invite = Invite::new("vol-1".into(), "user-1".into(), AccessLevel::ReadOnly, 0, 0);
        assert!(invite.is_valid());
        invite.revoke();
        assert!(!invite.is_valid());
    }

    #[test]
    fn max_uses_enforced() {
        let mut invite = Invite::new("vol-1".into(), "user-1".into(), AccessLevel::ReadOnly, 0, 2);
        assert!(invite.try_use().is_ok());
        assert!(invite.try_use().is_ok());
        assert!(invite.try_use().is_err());
    }

    #[test]
    fn expired_invite_invalid() {
        let mut invite = Invite::new(
            "vol-1".into(),
            "user-1".into(),
            AccessLevel::ReadOnly,
            1, // expired at epoch second 1
            0,
        );
        assert!(!invite.is_valid());
        assert!(invite.try_use().is_err());
    }

    #[test]
    fn invite_code_roundtrip() {
        let code = InviteCode {
            invite_id: "inv-123".into(),
            volume_id: "vol-456".into(),
            hmac: vec![0xAB; 32],
        };
        let encoded = code.encode().unwrap();
        let decoded = InviteCode::decode(&encoded).unwrap();
        assert_eq!(decoded.invite_id, "inv-123");
        assert_eq!(decoded.volume_id, "vol-456");
        assert_eq!(decoded.hmac, vec![0xAB; 32]);
    }

    #[test]
    fn invalid_invite_code_fails() {
        assert!(InviteCode::decode("not-valid-base64!!!").is_err());
    }

    #[test]
    fn serialization_roundtrip() {
        let invite = Invite::new("vol-1".into(), "user-1".into(), AccessLevel::Admin, 0, 5);
        let json = serde_json::to_string(&invite).unwrap();
        let deserialized: Invite = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.volume_id, "vol-1");
        assert_eq!(deserialized.access_level, AccessLevel::Admin);
        assert_eq!(deserialized.max_uses, 5);
    }
}
