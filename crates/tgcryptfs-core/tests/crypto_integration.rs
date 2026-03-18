use tgcryptfs_core::block::srb1;
use tgcryptfs_core::crypto::keys::{Argon2Params, SymmetricKey};
/// Integration tests for the cryptographic pipeline.
use tgcryptfs_core::crypto::{aead, blake3, kdf, mlkem};

/// Test: Full key derivation → encryption → decryption pipeline.
#[test]
fn full_key_derivation_and_encryption_pipeline() {
    let password = b"correct horse battery staple";
    let salt = [0xAA; 32];
    let params = Argon2Params::default();

    let root_key = kdf::derive_root_key(password, &salt, &params).unwrap();
    let hierarchy = kdf::derive_hierarchy(root_key, &salt).unwrap();

    let plaintext = b"top secret file contents";
    let aad = b"inode:42:block:0";
    let ciphertext = aead::encrypt(&hierarchy.data, plaintext, aad).unwrap();
    let decrypted = aead::decrypt(&hierarchy.data, &ciphertext, aad).unwrap();
    assert_eq!(decrypted, plaintext);

    // Wrong key fails
    assert!(aead::decrypt(&hierarchy.meta, &ciphertext, aad).is_err());
    // Wrong AAD fails
    assert!(aead::decrypt(&hierarchy.data, &ciphertext, b"wrong").is_err());
}

/// Test: SRB1 block encoding end-to-end.
#[test]
fn srb1_block_encode_decode_pipeline() {
    let key = SymmetricKey::from_bytes([0x42; 32]);
    let plaintext = vec![0xDE; 4096];
    let aad = b"inode:1:block:0";
    let content_hash = blake3::hash(&plaintext);

    // Full encode with all arguments
    let block = srb1::encode(&key, 0, &plaintext, false, &content_hash, aad).unwrap();
    assert!(block.len() > plaintext.len());

    let decoded = srb1::decode(&key, &block, aad).unwrap();
    assert_eq!(decoded.plaintext, plaintext);
    assert_eq!(decoded.epoch, 0);
    assert!(!decoded.compressed);
}

/// Test: Content-addressable dedup via BLAKE3 hashing.
#[test]
fn blake3_content_addressing() {
    let data1 = b"hello world";
    let data2 = b"hello world";
    let data3 = b"different data";

    let h1 = blake3::hash(data1);
    let h2 = blake3::hash(data2);
    let h3 = blake3::hash(data3);

    assert_eq!(h1, h2);
    assert_ne!(h1, h3);
    assert!(blake3::verify_content_hash(data1, &h1));
    assert!(!blake3::verify_content_hash(data3, &h1));
}

/// Test: ML-KEM key exchange for sharing.
#[test]
fn mlkem_key_exchange_flow() {
    let (alice_dk, alice_ek) = mlkem::generate_keypair().unwrap();
    let (bob_ss, ciphertext) = mlkem::encapsulate(&alice_ek).unwrap();
    let alice_ss = mlkem::decapsulate(&alice_dk, &ciphertext).unwrap();
    assert_eq!(bob_ss.as_bytes(), alice_ss.as_bytes());

    // Use shared secret to encrypt/decrypt
    let msg = b"shared volume key material";
    let ct = aead::encrypt(&bob_ss, msg, b"share:vol-1").unwrap();
    let pt = aead::decrypt(&alice_ss, &ct, b"share:vol-1").unwrap();
    assert_eq!(pt, msg);
}

/// Test: Epoch key forward secrecy.
#[test]
fn epoch_key_forward_secrecy() {
    let password = b"test-password";
    let salt = [0xBB; 32];
    let params = Argon2Params::default();

    let root_key = kdf::derive_root_key(password, &salt, &params).unwrap();
    let hierarchy = kdf::derive_hierarchy(root_key, &salt).unwrap();

    let epoch0 = kdf::derive_epoch_key(&hierarchy.data, 0).unwrap();
    let epoch1 = kdf::derive_epoch_key(&hierarchy.data, 1).unwrap();
    let epoch2 = kdf::derive_epoch_key(&hierarchy.data, 2).unwrap();

    assert_ne!(epoch0.key.as_bytes(), epoch1.key.as_bytes());
    assert_ne!(epoch1.key.as_bytes(), epoch2.key.as_bytes());

    // Epoch 0 data can't be decrypted with epoch 1
    let data = b"epoch 0 data";
    let ct = aead::encrypt(&epoch0.key, data, b"epoch:0").unwrap();
    assert!(aead::decrypt(&epoch1.key, &ct, b"epoch:0").is_err());
    let pt = aead::decrypt(&epoch0.key, &ct, b"epoch:0").unwrap();
    assert_eq!(pt, data);
}

/// Test: Deterministic key derivation.
#[test]
fn deterministic_key_derivation() {
    let password = b"reproducible";
    let salt = [0xCC; 32];
    let params = Argon2Params::default();

    let h1 =
        kdf::derive_hierarchy(kdf::derive_root_key(password, &salt, &params).unwrap(), &salt).unwrap();
    let h2 =
        kdf::derive_hierarchy(kdf::derive_root_key(password, &salt, &params).unwrap(), &salt).unwrap();

    assert_eq!(h1.data.as_bytes(), h2.data.as_bytes());
    assert_eq!(h1.meta.as_bytes(), h2.meta.as_bytes());
    assert_eq!(h1.schema.as_bytes(), h2.schema.as_bytes());
}

/// Test: Keyed MAC for integrity verification.
#[test]
fn integrity_mac_verification() {
    let password = b"integrity-test";
    let salt = [0xDD; 32];
    let params = Argon2Params::default();

    let hierarchy =
        kdf::derive_hierarchy(kdf::derive_root_key(password, &salt, &params).unwrap(), &salt).unwrap();

    let data = b"critical metadata";
    let mac = blake3::keyed_hash(&hierarchy.integrity, data);

    assert!(blake3::verify_mac(&hierarchy.integrity, data, &mac));
    assert!(!blake3::verify_mac(&hierarchy.integrity, b"tampered", &mac));
    assert!(!blake3::verify_mac(&hierarchy.data, data, &mac));
}
