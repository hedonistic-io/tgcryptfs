use serde::{Deserialize, Serialize};

/// A single block's location and metadata within the filesystem.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockPointer {
    /// Random block ID (32 bytes)
    pub rid: [u8; 32],
    /// Telegram message ID where this block is stored
    pub message_id: i64,
    /// Start offset within the file this block covers
    pub file_offset: u64,
    /// Number of bytes from the file this block contains (plaintext)
    pub length: u64,
    /// Offset within the Telegram message/block data
    pub block_data_offset: u64,
    /// Size of the encrypted block on Telegram (includes SRB1 overhead)
    pub encrypted_size: u64,
    /// Whether this block is compressed
    pub compressed: bool,
    /// BLAKE3 content hash (unkeyed, for dedup)
    pub content_hash: [u8; 32],
    /// Epoch this block was encrypted under
    pub epoch: u32,
}

/// Complete file manifest: ordered list of block pointers covering the file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileManifest {
    /// Inode this manifest belongs to
    pub inode: u64,
    /// Manifest version (incremented on each write)
    pub version: u64,
    /// Total file size (uncompressed)
    pub total_size: u64,
    /// BLAKE3 hash of the complete file content
    pub file_hash: [u8; 32],
    /// Ordered block pointers covering [0, total_size)
    /// Invariant: no gaps, no overlaps, sorted by file_offset
    pub blocks: Vec<BlockPointer>,
}

impl FileManifest {
    /// Create a new empty manifest for an inode.
    pub fn new(inode: u64) -> Self {
        Self {
            inode,
            version: 0,
            total_size: 0,
            file_hash: [0u8; 32],
            blocks: Vec::new(),
        }
    }

    /// Validate manifest invariants: no gaps, no overlaps, sorted by offset.
    pub fn validate(&self) -> Result<(), String> {
        if self.blocks.is_empty() {
            if self.total_size != 0 {
                return Err("empty manifest with non-zero total_size".into());
            }
            return Ok(());
        }

        // First block must start at 0
        if self.blocks[0].file_offset != 0 {
            return Err(format!(
                "first block starts at {} instead of 0",
                self.blocks[0].file_offset
            ));
        }

        for i in 1..self.blocks.len() {
            let prev_end = self.blocks[i - 1].file_offset + self.blocks[i - 1].length;
            let curr_start = self.blocks[i].file_offset;

            if curr_start < prev_end {
                return Err(format!(
                    "overlap at block {i}: prev ends at {prev_end}, this starts at {curr_start}"
                ));
            }
            if curr_start > prev_end {
                return Err(format!(
                    "gap at block {i}: prev ends at {prev_end}, this starts at {curr_start}"
                ));
            }
        }

        // Last block must end at total_size
        let last = self.blocks.last().unwrap();
        let end = last.file_offset + last.length;
        if end != self.total_size {
            return Err(format!(
                "blocks end at {end} but total_size is {}",
                self.total_size
            ));
        }

        Ok(())
    }

    /// Find blocks that overlap with the given byte range [offset, offset+length).
    pub fn blocks_in_range(&self, offset: u64, length: u64) -> Vec<&BlockPointer> {
        let range_end = offset + length;
        self.blocks
            .iter()
            .filter(|bp| {
                let block_end = bp.file_offset + bp.length;
                bp.file_offset < range_end && block_end > offset
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_block(offset: u64, length: u64) -> BlockPointer {
        BlockPointer {
            rid: [0; 32],
            message_id: 0,
            file_offset: offset,
            length,
            block_data_offset: 0,
            encrypted_size: length + 40,
            compressed: false,
            content_hash: [0; 32],
            epoch: 0,
        }
    }

    #[test]
    fn empty_manifest_valid() {
        let m = FileManifest::new(1);
        assert!(m.validate().is_ok());
    }

    #[test]
    fn empty_manifest_with_size_invalid() {
        let mut m = FileManifest::new(1);
        m.total_size = 100;
        assert!(m.validate().is_err());
    }

    #[test]
    fn single_block_manifest_valid() {
        let mut m = FileManifest::new(1);
        m.total_size = 1000;
        m.blocks = vec![make_block(0, 1000)];
        assert!(m.validate().is_ok());
    }

    #[test]
    fn contiguous_blocks_valid() {
        let mut m = FileManifest::new(1);
        m.total_size = 3000;
        m.blocks = vec![
            make_block(0, 1000),
            make_block(1000, 1000),
            make_block(2000, 1000),
        ];
        assert!(m.validate().is_ok());
    }

    #[test]
    fn gap_detected() {
        let mut m = FileManifest::new(1);
        m.total_size = 3000;
        m.blocks = vec![make_block(0, 1000), make_block(1500, 1500)];
        assert!(m.validate().is_err());
    }

    #[test]
    fn overlap_detected() {
        let mut m = FileManifest::new(1);
        m.total_size = 2500;
        m.blocks = vec![make_block(0, 1500), make_block(1000, 1500)];
        assert!(m.validate().is_err());
    }

    #[test]
    fn blocks_in_range_finds_overlapping() {
        let mut m = FileManifest::new(1);
        m.total_size = 3000;
        m.blocks = vec![
            make_block(0, 1000),
            make_block(1000, 1000),
            make_block(2000, 1000),
        ];
        let result = m.blocks_in_range(500, 1000);
        assert_eq!(result.len(), 2); // blocks 0 and 1
    }

    #[test]
    fn blocks_in_range_exact_boundary() {
        let mut m = FileManifest::new(1);
        m.total_size = 2000;
        m.blocks = vec![make_block(0, 1000), make_block(1000, 1000)];
        // Range [1000, 1500) should only match block 1
        let result = m.blocks_in_range(1000, 500);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].file_offset, 1000);
    }
}
