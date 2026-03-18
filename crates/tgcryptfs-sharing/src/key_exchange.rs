use tgcryptfs_core::crypto::aead;
use tgcryptfs_core::crypto::keys::SymmetricKey;
use tgcryptfs_core::crypto::mlkem::{
    self, MlKemCiphertext, MlKemDecapsulationKey, MlKemEncapsulationKey,
};

use crate::error::{Result, SharingError};

/// A user's ML-KEM keypair for receiving wrapped keys.
#[derive(Debug)]
pub struct UserKeyPair {
    pub decapsulation_key: MlKemDecapsulationKey,
    pub encapsulation_key: MlKemEncapsulationKey,
}

impl UserKeyPair {
    /// Generate a new ML-KEM-768 keypair.
    pub fn generate() -> Result<Self> {
        let (dk, ek) = mlkem::generate_keypair()
            .map_err(|e| SharingError::KeyExchange(format!("keygen: {e}")))?;
        Ok(Self {
            decapsulation_key: dk,
            encapsulation_key: ek,
        })
    }
}

/// Wrap a data key for a recipient using their ML-KEM public key.
///
/// This performs:
/// 1. ML-KEM encapsulation to derive a shared secret
/// 2. AEAD encryption of the data key using the shared secret
///
/// Returns (ciphertext, wrapped_key) - both needed by the recipient to unwrap.
pub fn wrap_key_for_user(
    data_key: &SymmetricKey,
    recipient_ek: &MlKemEncapsulationKey,
    aad: &[u8],
) -> Result<WrappedKey> {
    // Encapsulate to get shared secret + ciphertext
    let (shared_secret, kem_ciphertext) = mlkem::encapsulate(recipient_ek)
        .map_err(|e| SharingError::KeyExchange(format!("encapsulate: {e}")))?;

    // Encrypt the data key with the shared secret
    let encrypted_key = aead::encrypt(&shared_secret, data_key.as_bytes(), aad)
        .map_err(|e| SharingError::Crypto(format!("wrap: {e}")))?;

    Ok(WrappedKey {
        kem_ciphertext: kem_ciphertext.0,
        encrypted_key,
    })
}

/// Unwrap a data key using the recipient's ML-KEM secret key.
///
/// This performs:
/// 1. ML-KEM decapsulation to recover the shared secret
/// 2. AEAD decryption of the wrapped key
pub fn unwrap_key(
    wrapped: &WrappedKey,
    recipient_dk: &MlKemDecapsulationKey,
    aad: &[u8],
) -> Result<SymmetricKey> {
    let kem_ct = MlKemCiphertext(wrapped.kem_ciphertext.clone());

    // Decapsulate to recover shared secret
    let shared_secret = mlkem::decapsulate(recipient_dk, &kem_ct)
        .map_err(|e| SharingError::KeyExchange(format!("decapsulate: {e}")))?;

    // Decrypt the data key
    let key_bytes = aead::decrypt(&shared_secret, &wrapped.encrypted_key, aad)
        .map_err(|e| SharingError::Crypto(format!("unwrap: {e}")))?;

    if key_bytes.len() != 32 {
        return Err(SharingError::Crypto(format!(
            "unwrapped key has wrong length: {} (expected 32)",
            key_bytes.len()
        )));
    }

    let mut arr = [0u8; 32];
    arr.copy_from_slice(&key_bytes);
    Ok(SymmetricKey::from_bytes(arr))
}

/// A wrapped key ready for storage or transmission.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WrappedKey {
    /// ML-KEM ciphertext (encapsulated shared secret).
    pub kem_ciphertext: Vec<u8>,
    /// AEAD-encrypted data key.
    pub encrypted_key: Vec<u8>,
}

impl WrappedKey {
    /// Serialize to bytes for storage.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        postcard::to_allocvec(self)
            .map_err(|e| SharingError::Crypto(format!("serialize wrapped key: {e}")))
    }

    /// Deserialize from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        postcard::from_bytes(data)
            .map_err(|e| SharingError::Crypto(format!("deserialize wrapped key: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_data_key() -> SymmetricKey {
        SymmetricKey::from_bytes([0x42; 32])
    }

    #[test]
    fn wrap_unwrap_roundtrip() {
        let kp = UserKeyPair::generate().unwrap();
        let data_key = test_data_key();
        let aad = b"volume:test-vol";

        let wrapped = wrap_key_for_user(&data_key, &kp.encapsulation_key, aad).unwrap();
        let unwrapped = unwrap_key(&wrapped, &kp.decapsulation_key, aad).unwrap();

        assert_eq!(data_key.as_bytes(), unwrapped.as_bytes());
    }

    #[test]
    fn wrong_decapsulation_key_fails() {
        let kp1 = UserKeyPair::generate().unwrap();
        let kp2 = UserKeyPair::generate().unwrap();
        let data_key = test_data_key();
        let aad = b"volume:test-vol";

        let wrapped = wrap_key_for_user(&data_key, &kp1.encapsulation_key, aad).unwrap();
        // Using kp2's decapsulation key should fail (AEAD will fail because shared secrets differ)
        assert!(unwrap_key(&wrapped, &kp2.decapsulation_key, aad).is_err());
    }

    #[test]
    fn wrong_aad_fails() {
        let kp = UserKeyPair::generate().unwrap();
        let data_key = test_data_key();

        let wrapped = wrap_key_for_user(&data_key, &kp.encapsulation_key, b"correct").unwrap();
        assert!(unwrap_key(&wrapped, &kp.decapsulation_key, b"wrong").is_err());
    }

    #[test]
    fn wrapped_key_serialization() {
        let kp = UserKeyPair::generate().unwrap();
        let data_key = test_data_key();
        let aad = b"vol:1";

        let wrapped = wrap_key_for_user(&data_key, &kp.encapsulation_key, aad).unwrap();
        let bytes = wrapped.to_bytes().unwrap();
        let restored = WrappedKey::from_bytes(&bytes).unwrap();

        let unwrapped = unwrap_key(&restored, &kp.decapsulation_key, aad).unwrap();
        assert_eq!(data_key.as_bytes(), unwrapped.as_bytes());
    }

    #[test]
    fn multiple_wraps_produce_different_ciphertexts() {
        let kp = UserKeyPair::generate().unwrap();
        let data_key = test_data_key();
        let aad = b"vol:1";

        let w1 = wrap_key_for_user(&data_key, &kp.encapsulation_key, aad).unwrap();
        let w2 = wrap_key_for_user(&data_key, &kp.encapsulation_key, aad).unwrap();

        // KEM ciphertexts should differ (randomized encapsulation)
        assert_ne!(w1.kem_ciphertext, w2.kem_ciphertext);

        // Both should unwrap to the same key
        let k1 = unwrap_key(&w1, &kp.decapsulation_key, aad).unwrap();
        let k2 = unwrap_key(&w2, &kp.decapsulation_key, aad).unwrap();
        assert_eq!(k1.as_bytes(), k2.as_bytes());
    }
}
