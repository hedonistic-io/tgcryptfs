use crate::error::{CoreError, Result};

/// Configuration for content-defined chunking.
#[derive(Debug, Clone)]
pub struct CdcConfig {
    /// Minimum chunk size in bytes (default: 4 KB)
    pub min_size: usize,
    /// Maximum chunk size in bytes (default: 50 MB)
    pub max_size: usize,
    /// Target average chunk size in bytes (default: 1 MB)
    pub target_size: usize,
}

impl Default for CdcConfig {
    fn default() -> Self {
        Self {
            min_size: 4 * 1024,
            max_size: 50 * 1024 * 1024,
            target_size: 1024 * 1024,
        }
    }
}

/// A chunk produced by content-defined chunking.
#[derive(Debug, Clone)]
pub struct Chunk {
    /// Offset within the input data
    pub offset: usize,
    /// Length of this chunk
    pub length: usize,
}

const POLYNOMIAL: u64 = 0x3DA3358B4DC173;
const WINDOW_SIZE: usize = 48;

/// Perform content-defined chunking using Rabin fingerprinting.
///
/// Returns chunk boundaries. The data is NOT copied; callers use
/// the offset/length to slice the input.
pub fn chunk(data: &[u8], config: &CdcConfig) -> Result<Vec<Chunk>> {
    if config.min_size == 0 || config.max_size == 0 || config.target_size == 0 {
        return Err(CoreError::Chunking(
            "invalid config: sizes must be > 0".into(),
        ));
    }
    if config.min_size > config.max_size {
        return Err(CoreError::Chunking("min_size > max_size".into()));
    }
    if config.target_size < config.min_size || config.target_size > config.max_size {
        return Err(CoreError::Chunking(
            "target_size out of [min, max] range".into(),
        ));
    }

    if data.is_empty() {
        return Ok(vec![]);
    }

    // If data is smaller than min_size, return as a single chunk
    if data.len() <= config.min_size {
        return Ok(vec![Chunk {
            offset: 0,
            length: data.len(),
        }]);
    }

    // Mask derived from target size: floor(log2(target_size)) bits set
    let mask = mask_from_target(config.target_size);

    let mut chunks = Vec::new();
    let mut chunk_start = 0;
    let mut fingerprint: u64 = 0;

    let mut i = chunk_start + config.min_size;
    while i < data.len() {
        // Rabin rolling hash update
        fingerprint = fingerprint
            .wrapping_mul(256)
            .wrapping_add(u64::from(data[i]));
        if i >= WINDOW_SIZE {
            fingerprint = fingerprint.wrapping_sub(
                u64::from(data[i - WINDOW_SIZE]).wrapping_mul(pow_mod(256, WINDOW_SIZE as u64)),
            );
        }
        fingerprint ^= POLYNOMIAL;

        let chunk_len = i - chunk_start + 1;

        // Cut point: fingerprint matches mask (target boundary) OR max_size reached
        if (fingerprint & mask == 0) || chunk_len >= config.max_size {
            chunks.push(Chunk {
                offset: chunk_start,
                length: chunk_len,
            });
            chunk_start = i + 1;
            fingerprint = 0;
            i = chunk_start + config.min_size;
            continue;
        }

        i += 1;
    }

    // Remaining data becomes the last chunk
    if chunk_start < data.len() {
        chunks.push(Chunk {
            offset: chunk_start,
            length: data.len() - chunk_start,
        });
    }

    Ok(chunks)
}

fn mask_from_target(target: usize) -> u64 {
    let bits = (target as f64).log2().floor() as u32;
    (1u64 << bits) - 1
}

fn pow_mod(base: u64, exp: u64) -> u64 {
    let mut result = 1u64;
    for _ in 0..exp {
        result = result.wrapping_mul(base);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_data_produces_no_chunks() {
        let chunks = chunk(b"", &CdcConfig::default()).unwrap();
        assert!(chunks.is_empty());
    }

    #[test]
    fn small_data_single_chunk() {
        let data = vec![0u8; 100];
        let config = CdcConfig {
            min_size: 1024,
            max_size: 4096,
            target_size: 2048,
        };
        let chunks = chunk(&data, &config).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].offset, 0);
        assert_eq!(chunks[0].length, 100);
    }

    #[test]
    fn respects_max_size() {
        let data = vec![0xAB; 10_000];
        let config = CdcConfig {
            min_size: 100,
            max_size: 1000,
            target_size: 500,
        };
        let chunks = chunk(&data, &config).unwrap();
        for c in &chunks {
            assert!(c.length <= config.max_size + 1);
        }
    }

    #[test]
    fn respects_min_size() {
        let data = vec![0xAB; 10_000];
        let config = CdcConfig {
            min_size: 100,
            max_size: 1000,
            target_size: 500,
        };
        let chunks = chunk(&data, &config).unwrap();
        // All chunks except potentially the last should be >= min_size
        for c in &chunks[..chunks.len().saturating_sub(1)] {
            assert!(c.length >= config.min_size, "chunk too small: {}", c.length);
        }
    }

    #[test]
    fn chunks_cover_all_data() {
        let data = vec![0xAB; 50_000];
        let config = CdcConfig {
            min_size: 100,
            max_size: 5000,
            target_size: 2000,
        };
        let chunks = chunk(&data, &config).unwrap();

        // Verify no gaps, no overlaps, full coverage
        let mut offset = 0;
        for c in &chunks {
            assert_eq!(c.offset, offset);
            offset += c.length;
        }
        assert_eq!(offset, data.len());
    }

    #[test]
    fn invalid_config_rejected() {
        assert!(chunk(
            b"x",
            &CdcConfig {
                min_size: 0,
                max_size: 100,
                target_size: 50
            }
        )
        .is_err());
        assert!(chunk(
            b"x",
            &CdcConfig {
                min_size: 200,
                max_size: 100,
                target_size: 150
            }
        )
        .is_err());
    }

    #[test]
    fn deterministic_chunking() {
        let data: Vec<u8> = (0..10_000).map(|i| (i % 256) as u8).collect();
        let config = CdcConfig {
            min_size: 100,
            max_size: 2000,
            target_size: 500,
        };
        let c1 = chunk(&data, &config).unwrap();
        let c2 = chunk(&data, &config).unwrap();
        assert_eq!(c1.len(), c2.len());
        for (a, b) in c1.iter().zip(c2.iter()) {
            assert_eq!(a.offset, b.offset);
            assert_eq!(a.length, b.length);
        }
    }
}
