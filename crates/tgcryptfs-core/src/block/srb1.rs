use crate::crypto::{aead, keys::SymmetricKey};
use crate::error::{CoreError, Result};

/// SRB1 block format magic bytes.
const SRB1_MAGIC: &[u8; 4] = b"SRB1";

/// SRB1 block format version.
const SRB1_VERSION: u8 = 1;

/// Header size: magic(4) + version(1) + epoch(4) + flags(1) + content_hash(32) + plaintext_len(4) = 46 bytes
const HEADER_SIZE: usize = 46;

/// Flags
const FLAG_COMPRESSED: u8 = 0x01;

/// Encode data into SRB1 block format.
///
/// Format: `SRB1 | version(1) | epoch(4) | flags(1) | content_hash(32) | plaintext_len(4) | encrypted_payload`
///
/// The entire payload (header fields after magic + encrypted data) is authenticated via AEAD AAD.
pub fn encode(
    key: &SymmetricKey,
    epoch: u32,
    plaintext: &[u8],
    compressed: bool,
    content_hash: &[u8; 32],
    aad: &[u8],
) -> Result<Vec<u8>> {
    let flags = if compressed { FLAG_COMPRESSED } else { 0 };
    let plaintext_len = plaintext.len() as u32;

    // Build header
    let mut header = Vec::with_capacity(HEADER_SIZE);
    header.extend_from_slice(SRB1_MAGIC);
    header.push(SRB1_VERSION);
    header.extend_from_slice(&epoch.to_le_bytes());
    header.push(flags);
    header.extend_from_slice(content_hash);
    header.extend_from_slice(&plaintext_len.to_le_bytes());

    // Build full AAD: caller-provided AAD + header metadata
    let mut full_aad = Vec::with_capacity(aad.len() + HEADER_SIZE - 4);
    full_aad.extend_from_slice(aad);
    full_aad.extend_from_slice(&header[4..]); // Exclude magic from AAD binding

    // Encrypt the payload
    let encrypted = aead::encrypt(key, plaintext, &full_aad)?;

    // Final block: header + encrypted payload
    let mut block = Vec::with_capacity(header.len() + encrypted.len());
    block.extend_from_slice(&header);
    block.extend_from_slice(&encrypted);
    Ok(block)
}

/// Decoded SRB1 block.
pub struct DecodedBlock {
    pub epoch: u32,
    pub compressed: bool,
    pub content_hash: [u8; 32],
    pub plaintext_len: u32,
    pub plaintext: Vec<u8>,
}

/// Decode an SRB1 block.
pub fn decode(key: &SymmetricKey, block: &[u8], aad: &[u8]) -> Result<DecodedBlock> {
    if block.len() < HEADER_SIZE + aead::NONCE_SIZE + aead::TAG_SIZE {
        return Err(CoreError::BlockFormat("block too short".into()));
    }

    // Parse header
    if &block[..4] != SRB1_MAGIC {
        return Err(CoreError::BlockFormat("invalid magic".into()));
    }

    let version = block[4];
    if version != SRB1_VERSION {
        return Err(CoreError::BlockFormat(format!(
            "unsupported version: {version}"
        )));
    }

    let epoch = u32::from_le_bytes(block[5..9].try_into().unwrap());
    let flags = block[9];
    let compressed = flags & FLAG_COMPRESSED != 0;

    let mut content_hash = [0u8; 32];
    content_hash.copy_from_slice(&block[10..42]);

    let plaintext_len = u32::from_le_bytes(block[42..46].try_into().unwrap());

    // Reconstruct AAD
    let mut full_aad = Vec::with_capacity(aad.len() + HEADER_SIZE - 4);
    full_aad.extend_from_slice(aad);
    full_aad.extend_from_slice(&block[4..HEADER_SIZE]);

    // Decrypt payload
    let plaintext = aead::decrypt(key, &block[HEADER_SIZE..], &full_aad)?;

    if plaintext.len() != plaintext_len as usize {
        return Err(CoreError::BlockFormat(format!(
            "plaintext length mismatch: header says {plaintext_len}, got {}",
            plaintext.len()
        )));
    }

    Ok(DecodedBlock {
        epoch,
        compressed,
        content_hash,
        plaintext_len,
        plaintext,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::blake3 as b3;

    fn test_key() -> SymmetricKey {
        SymmetricKey::from_bytes([0x42; 32])
    }

    #[test]
    fn encode_decode_roundtrip() {
        let key = test_key();
        let data = b"hello block world";
        let hash = b3::hash(data);
        let aad = b"inode:1:offset:0";

        let block = encode(&key, 0, data, false, &hash, aad).unwrap();
        let decoded = decode(&key, &block, aad).unwrap();

        assert_eq!(decoded.plaintext, data);
        assert_eq!(decoded.epoch, 0);
        assert!(!decoded.compressed);
        assert_eq!(decoded.content_hash, hash);
        assert_eq!(decoded.plaintext_len, data.len() as u32);
    }

    #[test]
    fn compressed_flag_preserved() {
        let key = test_key();
        let data = b"compressed data";
        let hash = b3::hash(data);

        let block = encode(&key, 5, data, true, &hash, b"").unwrap();
        let decoded = decode(&key, &block, b"").unwrap();
        assert!(decoded.compressed);
        assert_eq!(decoded.epoch, 5);
    }

    #[test]
    fn wrong_key_fails() {
        let key1 = test_key();
        let key2 = SymmetricKey::from_bytes([0x43; 32]);
        let data = b"secret";
        let hash = b3::hash(data);

        let block = encode(&key1, 0, data, false, &hash, b"").unwrap();
        assert!(decode(&key2, &block, b"").is_err());
    }

    #[test]
    fn wrong_aad_fails() {
        let key = test_key();
        let data = b"data";
        let hash = b3::hash(data);

        let block = encode(&key, 0, data, false, &hash, b"correct").unwrap();
        assert!(decode(&key, &block, b"wrong").is_err());
    }

    #[test]
    fn tampered_header_fails() {
        let key = test_key();
        let data = b"data";
        let hash = b3::hash(data);

        let mut block = encode(&key, 0, data, false, &hash, b"").unwrap();
        // Tamper with the epoch in the header
        block[5] ^= 0xFF;
        assert!(decode(&key, &block, b"").is_err());
    }

    #[test]
    fn truncated_block_fails() {
        let key = test_key();
        assert!(decode(&key, &[0u8; 20], b"").is_err());
    }

    #[test]
    fn invalid_magic_fails() {
        let key = test_key();
        let mut block = vec![0u8; 200];
        block[..4].copy_from_slice(b"XXXX");
        assert!(decode(&key, &block, b"").is_err());
    }

    #[test]
    fn block_size_overhead() {
        let key = test_key();
        let data = b"hello";
        let hash = b3::hash(data);
        let block = encode(&key, 0, data, false, &hash, b"").unwrap();
        // header(46) + nonce(24) + plaintext(5) + tag(16) = 91
        assert_eq!(
            block.len(),
            HEADER_SIZE + aead::NONCE_SIZE + data.len() + aead::TAG_SIZE
        );
    }
}
