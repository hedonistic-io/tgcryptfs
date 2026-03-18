use std::collections::HashMap;

use crate::crypto::blake3 as b3;

/// Deduplication index: maps content hashes to block IDs.
/// This is an in-memory structure used during writes to avoid
/// uploading blocks with identical content.
#[derive(Debug, Default)]
pub struct DedupIndex {
    /// content_hash → (rid, reference_count)
    entries: HashMap<[u8; 32], DedupEntry>,
}

#[derive(Debug, Clone)]
pub struct DedupEntry {
    /// Block random ID that holds this content
    pub rid: [u8; 32],
    /// Number of file manifests referencing this block
    pub ref_count: u32,
}

/// Result of a dedup check.
#[derive(Debug)]
pub enum DedupResult {
    /// Content already exists; reuse this block ID
    Duplicate { rid: [u8; 32] },
    /// Content is new; upload required
    Unique { content_hash: [u8; 32] },
}

impl DedupIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if content already exists in the index.
    pub fn check(&self, data: &[u8]) -> DedupResult {
        let hash = b3::hash(data);
        match self.entries.get(&hash) {
            Some(entry) => DedupResult::Duplicate { rid: entry.rid },
            None => DedupResult::Unique { content_hash: hash },
        }
    }

    /// Register a new block in the dedup index.
    pub fn insert(&mut self, content_hash: [u8; 32], rid: [u8; 32]) {
        self.entries
            .entry(content_hash)
            .and_modify(|e| e.ref_count += 1)
            .or_insert(DedupEntry { rid, ref_count: 1 });
    }

    /// Decrement reference count; returns true if block can be garbage collected.
    pub fn release(&mut self, content_hash: &[u8; 32]) -> bool {
        if let Some(entry) = self.entries.get_mut(content_hash) {
            entry.ref_count = entry.ref_count.saturating_sub(1);
            if entry.ref_count == 0 {
                self.entries.remove(content_hash);
                return true;
            }
        }
        false
    }

    /// Number of unique blocks tracked.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_content_is_unique() {
        let idx = DedupIndex::new();
        match idx.check(b"hello") {
            DedupResult::Unique { .. } => {}
            DedupResult::Duplicate { .. } => panic!("should be unique"),
        }
    }

    #[test]
    fn inserted_content_is_duplicate() {
        let mut idx = DedupIndex::new();
        let hash = b3::hash(b"hello");
        let rid = [0x42; 32];
        idx.insert(hash, rid);

        match idx.check(b"hello") {
            DedupResult::Duplicate { rid: found } => assert_eq!(found, rid),
            DedupResult::Unique { .. } => panic!("should be duplicate"),
        }
    }

    #[test]
    fn release_removes_at_zero_refs() {
        let mut idx = DedupIndex::new();
        let hash = b3::hash(b"data");
        idx.insert(hash, [0x01; 32]);
        assert_eq!(idx.len(), 1);

        let gc = idx.release(&hash);
        assert!(gc);
        assert_eq!(idx.len(), 0);
    }

    #[test]
    fn release_decrements_refs() {
        let mut idx = DedupIndex::new();
        let hash = b3::hash(b"data");
        idx.insert(hash, [0x01; 32]);
        idx.insert(hash, [0x01; 32]); // ref_count = 2

        let gc = idx.release(&hash);
        assert!(!gc);
        assert_eq!(idx.len(), 1);

        let gc = idx.release(&hash);
        assert!(gc);
        assert_eq!(idx.len(), 0);
    }
}
