use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::Zeroize;

use crate::crypto::keys::{Argon2Params, EpochKey, KeyHierarchy, SymmetricKey};
use crate::error::{CoreError, Result};

/// Derive the root key from a password and salt using Argon2id.
pub fn derive_root_key(
    password: &[u8],
    salt: &[u8; 32],
    params: &Argon2Params,
) -> Result<SymmetricKey> {
    use argon2::{Algorithm, Argon2, Version};

    let argon2 = Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        argon2::Params::new(
            params.memory_kib,
            params.iterations,
            params.parallelism,
            Some(params.output_len),
        )
        .map_err(|e| CoreError::KeyDerivation(format!("argon2 params: {e}")))?,
    );

    let mut output = vec![0u8; params.output_len];
    argon2
        .hash_password_into(password, salt, &mut output)
        .map_err(|e| CoreError::KeyDerivation(format!("argon2: {e}")))?;

    let mut key_bytes = [0u8; 32];
    key_bytes.copy_from_slice(&output[..32]);
    output.zeroize();
    let key = SymmetricKey::from_bytes(key_bytes);
    key_bytes.zeroize();
    Ok(key)
}

/// Derive a child key from a parent key using HKDF-SHA256.
///
/// An optional salt provides additional domain separation between volumes
/// that might share a password. When `None`, HKDF uses a zero-filled salt
/// per RFC 5869, which is safe when IKM is already uniform.
pub fn hkdf_derive(parent: &SymmetricKey, info: &[u8]) -> Result<SymmetricKey> {
    hkdf_derive_with_salt(parent, info, None)
}

/// Derive a child key with an explicit salt (e.g., the volume salt).
pub fn hkdf_derive_with_salt(
    parent: &SymmetricKey,
    info: &[u8],
    salt: Option<&[u8]>,
) -> Result<SymmetricKey> {
    let hk = Hkdf::<Sha256>::new(salt, parent.as_bytes());
    let mut okm = [0u8; 32];
    hk.expand(info, &mut okm)
        .map_err(|e| CoreError::KeyDerivation(format!("hkdf: {e}")))?;
    let key = SymmetricKey::from_bytes(okm);
    okm.zeroize();
    Ok(key)
}

/// Derive the complete key hierarchy from a root key and volume salt.
///
/// The salt is passed to HKDF's extract step, providing additional
/// defense-in-depth against related-key scenarios across volumes.
pub fn derive_hierarchy(root: SymmetricKey, salt: &[u8; 32]) -> Result<KeyHierarchy> {
    use crate::crypto::keys::labels;

    let data = hkdf_derive_with_salt(&root, labels::DATA, Some(salt))?;
    let meta = hkdf_derive_with_salt(&root, labels::META, Some(salt))?;
    let schema = hkdf_derive_with_salt(&root, labels::SCHEMA, Some(salt))?;
    let integrity = hkdf_derive_with_salt(&root, labels::INTEGRITY, Some(salt))?;
    let wrapping = hkdf_derive_with_salt(&root, labels::WRAPPING, Some(salt))?;
    let deadman = hkdf_derive_with_salt(&root, labels::DEADMAN, Some(salt))?;

    Ok(KeyHierarchy {
        root,
        data,
        meta,
        schema,
        integrity,
        wrapping,
        deadman,
    })
}

/// Derive an epoch-scoped key from a parent key.
pub fn derive_epoch_key(parent: &SymmetricKey, epoch: u32) -> Result<EpochKey> {
    let info = format!("epoch:{epoch}");
    let key = hkdf_derive(parent, info.as_bytes())?;
    Ok(EpochKey { epoch, key })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_root_key_deterministic() {
        let salt = [0x01; 32];
        let params = Argon2Params {
            memory_kib: 1024, // Low for tests
            iterations: 1,
            parallelism: 1,
            output_len: 32,
        };
        let k1 = derive_root_key(b"password", &salt, &params).unwrap();
        let k2 = derive_root_key(b"password", &salt, &params).unwrap();
        assert_eq!(k1.as_bytes(), k2.as_bytes());
    }

    #[test]
    fn different_passwords_produce_different_keys() {
        let salt = [0x01; 32];
        let params = Argon2Params {
            memory_kib: 1024,
            iterations: 1,
            parallelism: 1,
            output_len: 32,
        };
        let k1 = derive_root_key(b"password1", &salt, &params).unwrap();
        let k2 = derive_root_key(b"password2", &salt, &params).unwrap();
        assert_ne!(k1.as_bytes(), k2.as_bytes());
    }

    #[test]
    fn different_salts_produce_different_keys() {
        let params = Argon2Params {
            memory_kib: 1024,
            iterations: 1,
            parallelism: 1,
            output_len: 32,
        };
        let k1 = derive_root_key(b"password", &[0x01; 32], &params).unwrap();
        let k2 = derive_root_key(b"password", &[0x02; 32], &params).unwrap();
        assert_ne!(k1.as_bytes(), k2.as_bytes());
    }

    #[test]
    fn hkdf_derive_deterministic() {
        let parent = SymmetricKey::from_bytes([0x42; 32]);
        let k1 = hkdf_derive(&parent, b"test-info").unwrap();
        let k2 = hkdf_derive(&parent, b"test-info").unwrap();
        assert_eq!(k1.as_bytes(), k2.as_bytes());
    }

    #[test]
    fn hkdf_different_info_produces_different_keys() {
        let parent = SymmetricKey::from_bytes([0x42; 32]);
        let k1 = hkdf_derive(&parent, b"info-a").unwrap();
        let k2 = hkdf_derive(&parent, b"info-b").unwrap();
        assert_ne!(k1.as_bytes(), k2.as_bytes());
    }

    #[test]
    fn derive_hierarchy_all_keys_distinct() {
        let root = SymmetricKey::from_bytes([0x42; 32]);
        let h = derive_hierarchy(root, &[0x01; 32]).unwrap();

        let keys: Vec<&[u8; 32]> = vec![
            h.root.as_bytes(),
            h.data.as_bytes(),
            h.meta.as_bytes(),
            h.schema.as_bytes(),
            h.integrity.as_bytes(),
            h.wrapping.as_bytes(),
            h.deadman.as_bytes(),
        ];

        // Verify all keys are distinct
        for i in 0..keys.len() {
            for j in (i + 1)..keys.len() {
                assert_ne!(keys[i], keys[j], "keys {i} and {j} should differ");
            }
        }
    }

    #[test]
    fn derive_epoch_keys_distinct() {
        let parent = SymmetricKey::from_bytes([0x42; 32]);
        let e0 = derive_epoch_key(&parent, 0).unwrap();
        let e1 = derive_epoch_key(&parent, 1).unwrap();
        assert_ne!(e0.key.as_bytes(), e1.key.as_bytes());
        assert_eq!(e0.epoch, 0);
        assert_eq!(e1.epoch, 1);
    }

    #[test]
    fn epoch_key_derivation_deterministic() {
        let parent = SymmetricKey::from_bytes([0x42; 32]);
        let e0a = derive_epoch_key(&parent, 0).unwrap();
        let e0b = derive_epoch_key(&parent, 0).unwrap();
        assert_eq!(e0a.key.as_bytes(), e0b.key.as_bytes());
    }
}
