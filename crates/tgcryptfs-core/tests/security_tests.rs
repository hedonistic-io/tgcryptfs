//! Security & negative path tests for the core crypto layer.
//!
//! These tests validate that cryptographic operations fail safely and securely:
//! wrong passwords, tampered ciphertext, key isolation, and domain separation.

use tgcryptfs_core::crypto::aead;
use tgcryptfs_core::crypto::kdf;
use tgcryptfs_core::crypto::keys::{Argon2Params, SymmetricKey};

// ---- Wrong password produces different key hierarchy ----

#[test]
fn wrong_password_different_hierarchy() {
    let salt = [0x42u8; 32];
    let params = Argon2Params::default();

    let root1 = kdf::derive_root_key(b"correct-password", &salt, &params).unwrap();
    let root2 = kdf::derive_root_key(b"wrong-password", &salt, &params).unwrap();

    // Different passwords → different root keys
    assert_ne!(root1.as_bytes(), root2.as_bytes());

    // Derive full hierarchies
    let hier1 = kdf::derive_hierarchy(root1, &[0x01; 32]).unwrap();
    let hier2 = kdf::derive_hierarchy(root2, &[0x02; 32]).unwrap();

    // Every derived key differs
    assert_ne!(hier1.data.as_bytes(), hier2.data.as_bytes());
    assert_ne!(hier1.meta.as_bytes(), hier2.meta.as_bytes());
    assert_ne!(hier1.schema.as_bytes(), hier2.schema.as_bytes());
    assert_ne!(hier1.integrity.as_bytes(), hier2.integrity.as_bytes());
    assert_ne!(hier1.wrapping.as_bytes(), hier2.wrapping.as_bytes());
    assert_ne!(hier1.deadman.as_bytes(), hier2.deadman.as_bytes());
}

// ---- Same password + different salt → different keys ----

#[test]
fn different_salt_different_keys() {
    let params = Argon2Params::default();

    let root1 = kdf::derive_root_key(b"same-password", &[0x01u8; 32], &params).unwrap();
    let root2 = kdf::derive_root_key(b"same-password", &[0x02u8; 32], &params).unwrap();

    assert_ne!(root1.as_bytes(), root2.as_bytes());
}

// ---- Data encrypted with wrong key cannot be decrypted ----

#[test]
fn wrong_key_decryption_fails() {
    let key1 = SymmetricKey::from_bytes([0x11; 32]);
    let key2 = SymmetricKey::from_bytes([0x22; 32]);
    let aad = b"inode:42";
    let plaintext = b"secret inode data";

    let ciphertext = aead::encrypt(&key1, plaintext, aad).unwrap();

    // Wrong key → decryption fails
    let result = aead::decrypt(&key2, &ciphertext, aad);
    assert!(result.is_err());
}

// ---- Tampered ciphertext detected ----

#[test]
fn tampered_ciphertext_detected() {
    let key = SymmetricKey::from_bytes([0x33; 32]);
    let aad = b"policy:1";
    let plaintext = b"policy data";

    let mut ciphertext = aead::encrypt(&key, plaintext, aad).unwrap();

    // Flip a bit in the middle of the ciphertext (past the nonce)
    let mid = ciphertext.len() / 2;
    ciphertext[mid] ^= 0xFF;

    let result = aead::decrypt(&key, &ciphertext, aad);
    assert!(result.is_err());
}

// ---- Wrong AAD causes authentication failure ----

#[test]
fn wrong_aad_authentication_fails() {
    let key = SymmetricKey::from_bytes([0x44; 32]);
    let plaintext = b"user record data";

    let ciphertext = aead::encrypt(&key, plaintext, b"user:alice").unwrap();

    // Decrypt with wrong AAD
    let result = aead::decrypt(&key, &ciphertext, b"user:bob");
    assert!(result.is_err());
}

// ---- AAD domain isolation ----

#[test]
fn aad_domain_isolation() {
    let key = SymmetricKey::from_bytes([0x55; 32]);
    let plaintext = b"shared data";

    // Same key and plaintext, different AAD
    let ct1 = aead::encrypt(&key, plaintext, b"inode:1").unwrap();
    let ct2 = aead::encrypt(&key, plaintext, b"inode:2").unwrap();

    // Ciphertexts are different (different nonces + different AAD)
    assert_ne!(ct1, ct2);

    // Each only decrypts with its own AAD
    assert!(aead::decrypt(&key, &ct1, b"inode:2").is_err());
    assert!(aead::decrypt(&key, &ct2, b"inode:1").is_err());

    // Correct AAD works
    assert_eq!(aead::decrypt(&key, &ct1, b"inode:1").unwrap(), plaintext);
    assert_eq!(aead::decrypt(&key, &ct2, b"inode:2").unwrap(), plaintext);
}

// ---- Truncated ciphertext fails ----

#[test]
fn truncated_ciphertext_fails() {
    let key = SymmetricKey::from_bytes([0x66; 32]);
    let plaintext = b"data to be truncated";
    let aad = b"test";

    let ciphertext = aead::encrypt(&key, plaintext, aad).unwrap();

    // Truncate at various points
    for len in [0, 1, 10, 24, ciphertext.len() - 1] {
        let truncated = &ciphertext[..len];
        let result = aead::decrypt(&key, truncated, aad);
        assert!(
            result.is_err(),
            "decryption should fail for truncated len={len}"
        );
    }
}

// ---- AEAD nonce uniqueness ----

#[test]
fn nonces_are_unique_across_encryptions() {
    let key = SymmetricKey::from_bytes([0x77; 32]);
    let plaintext = b"same data";
    let aad = b"same aad";

    // Encrypt 100 times — nonces (first 24 bytes) should all be unique
    let nonces: Vec<Vec<u8>> = (0..100)
        .map(|_| {
            let ct = aead::encrypt(&key, plaintext, aad).unwrap();
            ct[..24].to_vec()
        })
        .collect();

    for i in 0..nonces.len() {
        for j in (i + 1)..nonces.len() {
            assert_ne!(nonces[i], nonces[j], "nonce collision at i={i}, j={j}");
        }
    }
}

// ---- Key hierarchy domain separation ----

#[test]
fn hierarchy_keys_all_distinct() {
    let salt = [0x88u8; 32];
    let params = Argon2Params::default();
    let root = kdf::derive_root_key(b"password", &salt, &params).unwrap();
    let hier = kdf::derive_hierarchy(root, &[0x01; 32]).unwrap();

    // All 6 derived keys should be different from each other
    let keys = [
        hier.data.as_bytes(),
        hier.meta.as_bytes(),
        hier.schema.as_bytes(),
        hier.integrity.as_bytes(),
        hier.wrapping.as_bytes(),
        hier.deadman.as_bytes(),
    ];

    for i in 0..keys.len() {
        for j in (i + 1)..keys.len() {
            assert_ne!(keys[i], keys[j], "keys at indices {i} and {j} collided");
        }
    }
}

// ---- Argon2 params affect output ----

#[test]
fn argon2_different_params_different_output() {
    let salt = [0x99u8; 32];

    let params1 = Argon2Params {
        memory_kib: 65536,
        iterations: 3,
        parallelism: 4,
        output_len: 32,
    };

    let params2 = Argon2Params {
        memory_kib: 65536,
        iterations: 4, // Different iteration count
        parallelism: 4,
        output_len: 32,
    };

    let root1 = kdf::derive_root_key(b"test-password", &salt, &params1).unwrap();
    let root2 = kdf::derive_root_key(b"test-password", &salt, &params2).unwrap();

    assert_ne!(root1.as_bytes(), root2.as_bytes());
}

// ---- AEAD with large data ----

#[test]
fn aead_roundtrip_large_payload() {
    let key = SymmetricKey::from_bytes([0xAA; 32]);
    let aad = b"block:0";

    // 1MB payload
    let data: Vec<u8> = (0..1_048_576).map(|i| (i % 256) as u8).collect();

    let ciphertext = aead::encrypt(&key, &data, aad).unwrap();
    assert!(ciphertext.len() > data.len()); // overhead: nonce + tag

    let decrypted = aead::decrypt(&key, &ciphertext, aad).unwrap();
    assert_eq!(decrypted, data);
}

// ---- Appended data detected ----

#[test]
fn appended_data_to_ciphertext_detected() {
    let key = SymmetricKey::from_bytes([0xBB; 32]);
    let plaintext = b"original data";
    let aad = b"test";

    let mut ciphertext = aead::encrypt(&key, plaintext, aad).unwrap();

    // Append extra bytes
    ciphertext.extend_from_slice(b"extra garbage");

    // AEAD should detect the modification
    let result = aead::decrypt(&key, &ciphertext, aad);
    assert!(result.is_err());
}
