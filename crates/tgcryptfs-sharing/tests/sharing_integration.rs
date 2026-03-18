/// Integration tests for multi-user sharing flows.
/// Tests the complete invite → key exchange → access lifecycle.
use tgcryptfs_core::crypto::keys::SymmetricKey;
use tgcryptfs_sharing::access::{AccessLevel, ShareRecord};
use tgcryptfs_sharing::invite::Invite;
use tgcryptfs_sharing::key_exchange::{self, UserKeyPair, WrappedKey};

/// Test: Full sharing flow - owner creates invite, recipient accepts.
#[test]
fn full_sharing_flow() {
    // === Owner's side ===
    let data_key = SymmetricKey::from_bytes([0x42; 32]);
    let volume_id = "vol-001";

    // Owner creates an invite
    let mut invite = Invite::new(
        volume_id.into(),
        "owner-user-id".into(),
        AccessLevel::ReadWrite,
        0, // no expiry
        1, // single use
    );
    assert!(invite.is_valid());

    // === Recipient's side ===
    // Recipient generates ML-KEM keypair
    let recipient_kp = UserKeyPair::generate().unwrap();

    // === Owner wraps the data key for the recipient ===
    let aad = format!("share:{}:{}", volume_id, invite.invite_id);
    let wrapped =
        key_exchange::wrap_key_for_user(&data_key, &recipient_kp.encapsulation_key, aad.as_bytes())
            .unwrap();

    // Owner marks the invite as used
    invite.try_use().unwrap();
    assert!(!invite.is_valid()); // single use, now exhausted

    // === Recipient unwraps the data key ===
    let unwrapped =
        key_exchange::unwrap_key(&wrapped, &recipient_kp.decapsulation_key, aad.as_bytes())
            .unwrap();

    // Recipient now has the same data key as the owner
    assert_eq!(data_key.as_bytes(), unwrapped.as_bytes());

    // === Create share record ===
    let record = ShareRecord {
        user_id: "recipient-user-id".into(),
        telegram_user_id: 67890,
        display_name: "Bob".into(),
        access_level: AccessLevel::ReadWrite,
        wrapped_key: wrapped.to_bytes().unwrap(),
        granted_at: 1000000,
        active: true,
    };

    assert!(record.access_level.can_read());
    assert!(record.access_level.can_write());
    assert!(!record.access_level.can_manage_users());
}

/// Test: Admin can share, ReadOnly cannot manage users.
#[test]
fn access_level_hierarchy() {
    let admin = AccessLevel::Admin;
    let rw = AccessLevel::ReadWrite;
    let ro = AccessLevel::ReadOnly;

    // Admin can do everything
    assert!(admin.can_read());
    assert!(admin.can_write());
    assert!(admin.can_manage_users());

    // ReadWrite can read and write
    assert!(rw.can_read());
    assert!(rw.can_write());
    assert!(!rw.can_manage_users());

    // ReadOnly can only read
    assert!(ro.can_read());
    assert!(!ro.can_write());
    assert!(!ro.can_manage_users());
}

/// Test: Wrapped key can be serialized, stored, and restored.
#[test]
fn wrapped_key_persistence() {
    let data_key = SymmetricKey::from_bytes([0xAB; 32]);
    let kp = UserKeyPair::generate().unwrap();

    let wrapped =
        key_exchange::wrap_key_for_user(&data_key, &kp.encapsulation_key, b"persist-test").unwrap();

    // Serialize
    let bytes = wrapped.to_bytes().unwrap();

    // Restore
    let restored = WrappedKey::from_bytes(&bytes).unwrap();

    // Unwrap
    let key = key_exchange::unwrap_key(&restored, &kp.decapsulation_key, b"persist-test").unwrap();
    assert_eq!(data_key.as_bytes(), key.as_bytes());
}

/// Test: Sharing with multiple recipients.
#[test]
fn multi_recipient_sharing() {
    let data_key = SymmetricKey::from_bytes([0x42; 32]);
    let aad = b"share:vol-multi";

    // Create 3 recipients
    let recipients: Vec<UserKeyPair> = (0..3).map(|_| UserKeyPair::generate().unwrap()).collect();

    // Owner wraps key for each recipient
    let wrapped_keys: Vec<WrappedKey> = recipients
        .iter()
        .map(|r| key_exchange::wrap_key_for_user(&data_key, &r.encapsulation_key, aad).unwrap())
        .collect();

    // Each wrapped key is unique (different KEM encapsulations)
    assert_ne!(
        wrapped_keys[0].kem_ciphertext,
        wrapped_keys[1].kem_ciphertext
    );
    assert_ne!(
        wrapped_keys[1].kem_ciphertext,
        wrapped_keys[2].kem_ciphertext
    );

    // Each recipient can unwrap to the same data key
    for (i, (kp, wk)) in recipients.iter().zip(wrapped_keys.iter()).enumerate() {
        let unwrapped = key_exchange::unwrap_key(wk, &kp.decapsulation_key, aad).unwrap();
        assert_eq!(
            data_key.as_bytes(),
            unwrapped.as_bytes(),
            "recipient {} failed to unwrap key",
            i
        );
    }
}

/// Test: Invite expiry and max-uses work correctly.
#[test]
fn invite_constraints() {
    // Expired invite
    let expired = Invite::new("vol".into(), "user".into(), AccessLevel::ReadOnly, 1, 0);
    assert!(!expired.is_valid());

    // Multi-use invite
    let mut multi = Invite::new("vol".into(), "user".into(), AccessLevel::ReadOnly, 0, 3);
    assert!(multi.try_use().is_ok());
    assert!(multi.try_use().is_ok());
    assert!(multi.try_use().is_ok());
    assert!(multi.try_use().is_err()); // 4th use fails

    // Revoked invite
    let mut revoked = Invite::new("vol".into(), "user".into(), AccessLevel::ReadWrite, 0, 0);
    assert!(revoked.is_valid());
    revoked.revoke();
    assert!(!revoked.is_valid());
    assert!(revoked.try_use().is_err());
}
