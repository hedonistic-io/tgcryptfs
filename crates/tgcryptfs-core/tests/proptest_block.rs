use proptest::prelude::*;

use tgcryptfs_core::block::cdc::{self, CdcConfig};
use tgcryptfs_core::block::compress::{self, CompressionAlgorithm};

proptest! {
    /// CDC chunks cover the entire input with no gaps or overlaps.
    #[test]
    fn cdc_chunks_cover_input(
        data in prop::collection::vec(any::<u8>(), 1..8192),
    ) {
        let config = CdcConfig {
            min_size: 64,
            max_size: 4096,
            target_size: 1024,
        };
        let chunks = cdc::chunk(&data, &config).unwrap();

        // No gaps: chunks should cover [0, data.len())
        let mut expected_offset = 0;
        for chunk in &chunks {
            prop_assert_eq!(chunk.offset, expected_offset,
                "Gap detected at offset {}", expected_offset);
            expected_offset = chunk.offset + chunk.length;
        }
        prop_assert_eq!(expected_offset, data.len(),
            "Chunks don't cover full input");
    }

    /// CDC is deterministic: same input produces same chunks.
    #[test]
    fn cdc_deterministic(
        data in prop::collection::vec(any::<u8>(), 1..4096),
    ) {
        let config = CdcConfig {
            min_size: 64,
            max_size: 2048,
            target_size: 512,
        };
        let chunks1 = cdc::chunk(&data, &config).unwrap();
        let chunks2 = cdc::chunk(&data, &config).unwrap();
        prop_assert_eq!(chunks1.len(), chunks2.len());
        for (c1, c2) in chunks1.iter().zip(chunks2.iter()) {
            prop_assert_eq!(c1.offset, c2.offset);
            prop_assert_eq!(c1.length, c2.length);
        }
    }

    /// CDC respects max_size: no chunk larger than max_size.
    #[test]
    fn cdc_respects_max_size(
        data in prop::collection::vec(any::<u8>(), 1..16384),
    ) {
        let max_size = 2048;
        let config = CdcConfig {
            min_size: 64,
            max_size,
            target_size: 512,
        };
        let chunks = cdc::chunk(&data, &config).unwrap();
        for chunk in &chunks {
            prop_assert!(chunk.length <= max_size,
                "Chunk length {} exceeds max {}", chunk.length, max_size);
        }
    }

    /// Lz4 compression roundtrip: decompress(compress(data)) == data
    #[test]
    fn lz4_roundtrip(
        data in prop::collection::vec(any::<u8>(), 0..4096),
    ) {
        let compressed = compress::compress(&data, CompressionAlgorithm::Lz4).unwrap();
        let decompressed = compress::decompress(&compressed, CompressionAlgorithm::Lz4).unwrap();
        prop_assert_eq!(&data, &decompressed);
    }

    /// Zstd compression roundtrip.
    #[test]
    fn zstd_roundtrip(
        data in prop::collection::vec(any::<u8>(), 0..4096),
    ) {
        let compressed = compress::compress(&data, CompressionAlgorithm::Zstd { level: 3 }).unwrap();
        let decompressed = compress::decompress(&compressed, CompressionAlgorithm::Zstd { level: 3 }).unwrap();
        prop_assert_eq!(&data, &decompressed);
    }

    /// No-compression roundtrip (identity).
    #[test]
    fn none_compression_identity(
        data in prop::collection::vec(any::<u8>(), 0..2048),
    ) {
        let result = compress::compress(&data, CompressionAlgorithm::None).unwrap();
        prop_assert_eq!(&data, &result);
    }

    /// Chunk reconstruction: concatenating data slices at chunk boundaries equals original.
    #[test]
    fn chunk_reconstruction(
        data in prop::collection::vec(any::<u8>(), 1..8192),
    ) {
        let config = CdcConfig {
            min_size: 64,
            max_size: 4096,
            target_size: 1024,
        };
        let chunks = cdc::chunk(&data, &config).unwrap();

        let mut reconstructed = Vec::new();
        for chunk in &chunks {
            reconstructed.extend_from_slice(&data[chunk.offset..chunk.offset + chunk.length]);
        }
        prop_assert_eq!(&data, &reconstructed);
    }
}
