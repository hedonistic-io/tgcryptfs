use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};

/// Compression algorithm selection.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum CompressionAlgorithm {
    None,
    #[default]
    Lz4,
    Zstd {
        level: i32,
    },
}

/// Compress data using the specified algorithm.
pub fn compress(data: &[u8], algorithm: CompressionAlgorithm) -> Result<Vec<u8>> {
    match algorithm {
        CompressionAlgorithm::None => Ok(data.to_vec()),
        CompressionAlgorithm::Lz4 => Ok(lz4_flex::compress_prepend_size(data)),
        CompressionAlgorithm::Zstd { level } => {
            zstd::bulk::compress(data, level).map_err(|e| CoreError::Compression(e.to_string()))
        }
    }
}

/// Decompress data using the specified algorithm.
pub fn decompress(data: &[u8], algorithm: CompressionAlgorithm) -> Result<Vec<u8>> {
    match algorithm {
        CompressionAlgorithm::None => Ok(data.to_vec()),
        CompressionAlgorithm::Lz4 => lz4_flex::decompress_size_prepended(data)
            .map_err(|e| CoreError::Decompression(e.to_string())),
        CompressionAlgorithm::Zstd { .. } => {
            zstd::bulk::decompress(data, 64 * 1024 * 1024) // 64 MB max
                .map_err(|e| CoreError::Decompression(e.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_roundtrip() {
        let data = b"hello world";
        let c = compress(data, CompressionAlgorithm::None).unwrap();
        let d = decompress(&c, CompressionAlgorithm::None).unwrap();
        assert_eq!(d, data);
    }

    #[test]
    fn lz4_roundtrip() {
        let data = b"hello world hello world hello world";
        let c = compress(data, CompressionAlgorithm::Lz4).unwrap();
        let d = decompress(&c, CompressionAlgorithm::Lz4).unwrap();
        assert_eq!(d, data);
    }

    #[test]
    fn zstd_roundtrip() {
        let data = b"hello world hello world hello world";
        let algo = CompressionAlgorithm::Zstd { level: 3 };
        let c = compress(data, algo).unwrap();
        let d = decompress(&c, algo).unwrap();
        assert_eq!(d, data);
    }

    #[test]
    fn lz4_compresses_repetitive_data() {
        let data = vec![0xAA; 10_000];
        let c = compress(&data, CompressionAlgorithm::Lz4).unwrap();
        assert!(c.len() < data.len());
    }

    #[test]
    fn zstd_compresses_repetitive_data() {
        let data = vec![0xAA; 10_000];
        let algo = CompressionAlgorithm::Zstd { level: 3 };
        let c = compress(&data, algo).unwrap();
        assert!(c.len() < data.len());
    }

    #[test]
    fn empty_data_roundtrip() {
        for algo in [
            CompressionAlgorithm::None,
            CompressionAlgorithm::Lz4,
            CompressionAlgorithm::Zstd { level: 1 },
        ] {
            let c = compress(b"", algo).unwrap();
            let d = decompress(&c, algo).unwrap();
            assert_eq!(d, b"");
        }
    }
}
