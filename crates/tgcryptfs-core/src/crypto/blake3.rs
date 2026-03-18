use crate::crypto::keys::SymmetricKey;

/// Compute an unkeyed BLAKE3 hash of the input (for content addressing / dedup).
pub fn hash(data: &[u8]) -> [u8; 32] {
    *blake3::hash(data).as_bytes()
}

/// Compute a keyed BLAKE3 MAC (for integrity verification).
pub fn keyed_hash(key: &SymmetricKey, data: &[u8]) -> [u8; 32] {
    *blake3::keyed_hash(key.as_bytes(), data).as_bytes()
}

/// Derive an opaque identifier by hashing a domain-separated input with a key.
///
/// Uses length-prefixed encoding to prevent ambiguity:
/// `BLAKE3(Kschema || len(domain) || domain || len(name) || name)`.
/// This ensures `("col", "x:y")` never collides with `("col:x", "y")`.
pub fn derive_opaque_id(key: &SymmetricKey, domain: &str, name: &str) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new_keyed(key.as_bytes());
    hasher.update(&(domain.len() as u64).to_le_bytes());
    hasher.update(domain.as_bytes());
    hasher.update(&(name.len() as u64).to_le_bytes());
    hasher.update(name.as_bytes());
    *hasher.finalize().as_bytes()
}

/// Verify that a content hash matches the given data.
pub fn verify_content_hash(data: &[u8], expected: &[u8; 32]) -> bool {
    let actual = hash(data);
    constant_time_eq(&actual, expected)
}

/// Verify a keyed MAC.
pub fn verify_mac(key: &SymmetricKey, data: &[u8], expected: &[u8; 32]) -> bool {
    let actual = keyed_hash(key, data);
    constant_time_eq(&actual, expected)
}

/// Constant-time comparison to prevent timing attacks.
fn constant_time_eq(a: &[u8; 32], b: &[u8; 32]) -> bool {
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_deterministic() {
        let h1 = hash(b"hello");
        let h2 = hash(b"hello");
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_different_inputs_differ() {
        assert_ne!(hash(b"hello"), hash(b"world"));
    }

    #[test]
    fn hash_empty() {
        let h = hash(b"");
        assert_ne!(h, [0u8; 32]);
    }

    #[test]
    fn keyed_hash_deterministic() {
        let key = SymmetricKey::from_bytes([0x42; 32]);
        let h1 = keyed_hash(&key, b"data");
        let h2 = keyed_hash(&key, b"data");
        assert_eq!(h1, h2);
    }

    #[test]
    fn keyed_hash_different_keys_differ() {
        let k1 = SymmetricKey::from_bytes([0x01; 32]);
        let k2 = SymmetricKey::from_bytes([0x02; 32]);
        assert_ne!(keyed_hash(&k1, b"data"), keyed_hash(&k2, b"data"));
    }

    #[test]
    fn derive_opaque_id_deterministic() {
        let key = SymmetricKey::from_bytes([0x42; 32]);
        let id1 = derive_opaque_id(&key, "table", "inodes");
        let id2 = derive_opaque_id(&key, "table", "inodes");
        assert_eq!(id1, id2);
    }

    #[test]
    fn derive_opaque_id_different_names_differ() {
        let key = SymmetricKey::from_bytes([0x42; 32]);
        let id1 = derive_opaque_id(&key, "table", "inodes");
        let id2 = derive_opaque_id(&key, "table", "blocks");
        assert_ne!(id1, id2);
    }

    #[test]
    fn derive_opaque_id_different_domains_differ() {
        let key = SymmetricKey::from_bytes([0x42; 32]);
        let id1 = derive_opaque_id(&key, "table", "inodes");
        let id2 = derive_opaque_id(&key, "col", "inodes");
        assert_ne!(id1, id2);
    }

    #[test]
    fn verify_content_hash_correct() {
        let data = b"test data";
        let h = hash(data);
        assert!(verify_content_hash(data, &h));
    }

    #[test]
    fn verify_content_hash_wrong() {
        let data = b"test data";
        let wrong = [0u8; 32];
        assert!(!verify_content_hash(data, &wrong));
    }

    #[test]
    fn verify_mac_correct() {
        let key = SymmetricKey::from_bytes([0x42; 32]);
        let mac = keyed_hash(&key, b"message");
        assert!(verify_mac(&key, b"message", &mac));
    }

    #[test]
    fn verify_mac_wrong_data() {
        let key = SymmetricKey::from_bytes([0x42; 32]);
        let mac = keyed_hash(&key, b"message");
        assert!(!verify_mac(&key, b"tampered", &mac));
    }

    #[test]
    fn constant_time_eq_works() {
        let a = [1u8; 32];
        let b = [1u8; 32];
        let c = [2u8; 32];
        assert!(constant_time_eq(&a, &b));
        assert!(!constant_time_eq(&a, &c));
    }
}
