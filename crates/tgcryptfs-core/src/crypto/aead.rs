use chacha20poly1305::{
    aead::{Aead, KeyInit, OsRng},
    XChaCha20Poly1305, XNonce,
};
use rand::RngCore;

use crate::crypto::keys::SymmetricKey;
use crate::error::{CoreError, Result};

/// Nonce size for XChaCha20-Poly1305 (24 bytes / 192 bits).
pub const NONCE_SIZE: usize = 24;

/// Authentication tag size (16 bytes / 128 bits).
pub const TAG_SIZE: usize = 16;

/// Encrypt plaintext with XChaCha20-Poly1305.
///
/// Returns `nonce || ciphertext || tag` (24 + plaintext.len() + 16 bytes).
/// Nonce is generated from a CSPRNG.
pub fn encrypt(key: &SymmetricKey, plaintext: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new(key.as_bytes().into());

    let mut nonce_bytes = [0u8; NONCE_SIZE];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = XNonce::from_slice(&nonce_bytes);

    let payload = chacha20poly1305::aead::Payload {
        msg: plaintext,
        aad,
    };

    let ciphertext = cipher
        .encrypt(nonce, payload)
        .map_err(|e| CoreError::Encryption(e.to_string()))?;

    let mut output = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
    output.extend_from_slice(&nonce_bytes);
    output.extend_from_slice(&ciphertext);
    Ok(output)
}

/// Decrypt ciphertext produced by [`encrypt`].
///
/// Input format: `nonce (24 bytes) || ciphertext || tag (16 bytes)`.
pub fn decrypt(key: &SymmetricKey, ciphertext: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
    if ciphertext.len() < NONCE_SIZE + TAG_SIZE {
        return Err(CoreError::Decryption("ciphertext too short".to_string()));
    }

    let (nonce_bytes, ct) = ciphertext.split_at(NONCE_SIZE);
    let nonce = XNonce::from_slice(nonce_bytes);

    let cipher = XChaCha20Poly1305::new(key.as_bytes().into());

    let payload = chacha20poly1305::aead::Payload { msg: ct, aad };

    cipher
        .decrypt(nonce, payload)
        .map_err(|e| CoreError::Decryption(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> SymmetricKey {
        SymmetricKey::from_bytes([0x42; 32])
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = test_key();
        let plaintext = b"hello world";
        let aad = b"context";

        let ct = encrypt(&key, plaintext, aad).unwrap();
        let pt = decrypt(&key, &ct, aad).unwrap();
        assert_eq!(pt, plaintext);
    }

    #[test]
    fn encrypt_decrypt_empty() {
        let key = test_key();
        let ct = encrypt(&key, b"", b"").unwrap();
        let pt = decrypt(&key, &ct, b"").unwrap();
        assert_eq!(pt, b"");
    }

    #[test]
    fn decrypt_wrong_key_fails() {
        let key1 = test_key();
        let key2 = SymmetricKey::from_bytes([0x43; 32]);
        let ct = encrypt(&key1, b"secret", b"").unwrap();
        assert!(decrypt(&key2, &ct, b"").is_err());
    }

    #[test]
    fn decrypt_wrong_aad_fails() {
        let key = test_key();
        let ct = encrypt(&key, b"secret", b"correct-aad").unwrap();
        assert!(decrypt(&key, &ct, b"wrong-aad").is_err());
    }

    #[test]
    fn decrypt_truncated_fails() {
        let key = test_key();
        assert!(decrypt(&key, &[0u8; 10], b"").is_err());
    }

    #[test]
    fn decrypt_tampered_ciphertext_fails() {
        let key = test_key();
        let mut ct = encrypt(&key, b"secret", b"").unwrap();
        // Flip a byte in the ciphertext portion (after nonce)
        ct[NONCE_SIZE] ^= 0xFF;
        assert!(decrypt(&key, &ct, b"").is_err());
    }

    #[test]
    fn ciphertext_size_correct() {
        let key = test_key();
        let plaintext = b"hello";
        let ct = encrypt(&key, plaintext, b"").unwrap();
        assert_eq!(ct.len(), NONCE_SIZE + plaintext.len() + TAG_SIZE);
    }

    #[test]
    fn each_encryption_produces_unique_nonce() {
        let key = test_key();
        let ct1 = encrypt(&key, b"same", b"").unwrap();
        let ct2 = encrypt(&key, b"same", b"").unwrap();
        // Nonces should differ (first 24 bytes)
        assert_ne!(&ct1[..NONCE_SIZE], &ct2[..NONCE_SIZE]);
    }

    #[test]
    fn large_payload() {
        let key = test_key();
        let plaintext = vec![0xAB; 1024 * 1024]; // 1 MB
        let ct = encrypt(&key, &plaintext, b"big").unwrap();
        let pt = decrypt(&key, &ct, b"big").unwrap();
        assert_eq!(pt, plaintext);
    }
}
