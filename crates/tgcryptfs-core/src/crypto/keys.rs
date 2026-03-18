use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// 256-bit symmetric key with automatic zeroization on drop.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SymmetricKey([u8; 32]);

impl SymmetricKey {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Debug for SymmetricKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SymmetricKey").field(&"[REDACTED]").finish()
    }
}

/// The complete key hierarchy derived from a root key.
/// All keys are derived via HKDF and zeroized on drop.
#[derive(Debug, Zeroize, ZeroizeOnDrop)]
pub struct KeyHierarchy {
    /// Root key (derived from password via Argon2id)
    pub root: SymmetricKey,
    /// Data encryption key (file content blocks)
    pub data: SymmetricKey,
    /// Metadata encryption key (inode data, snapshots, policies, user records)
    pub meta: SymmetricKey,
    /// Schema obfuscation key (opaque table/column names)
    pub schema: SymmetricKey,
    /// Integrity hash key (BLAKE3 keyed mode)
    pub integrity: SymmetricKey,
    /// Key wrapping key (for sharing keys with other users)
    pub wrapping: SymmetricKey,
    /// Deadman audit log encryption key
    pub deadman: SymmetricKey,
}

/// Epoch-scoped data key for forward secrecy.
#[derive(Debug, Clone, Zeroize, ZeroizeOnDrop)]
pub struct EpochKey {
    pub epoch: u32,
    pub key: SymmetricKey,
}

/// HKDF domain separation labels per SentenceRefs v1 specification.
pub mod labels {
    pub const DATA: &[u8] = b"SentenceRefs.v1:data";
    pub const META: &[u8] = b"SentenceRefs.v1:meta";
    pub const SCHEMA: &[u8] = b"SentenceRefs.v1:schema";
    pub const INTEGRITY: &[u8] = b"SentenceRefs.v1:ih";
    pub const WRAPPING: &[u8] = b"SentenceRefs.v1:wrapping";
    pub const DEADMAN: &[u8] = b"SentenceRefs.v1:deadman";
}

/// Argon2id parameters for password-based key derivation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Argon2Params {
    /// Memory cost in KiB (default: 65536 = 64 MB)
    pub memory_kib: u32,
    /// Number of iterations (default: 3)
    pub iterations: u32,
    /// Degree of parallelism (default: 4)
    pub parallelism: u32,
    /// Output length in bytes (default: 32)
    pub output_len: usize,
}

impl Default for Argon2Params {
    fn default() -> Self {
        Self {
            memory_kib: 65536,
            iterations: 3,
            parallelism: 4,
            output_len: 32,
        }
    }
}
