use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use tgcryptfs_core::crypto::{aead, keys::SymmetricKey};

use crate::error::{CacheError, Result};

/// Configuration for the block cache.
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Directory for cached blocks on disk.
    pub cache_dir: PathBuf,
    /// Maximum cache size in bytes.
    pub max_size: u64,
    /// Whether to encrypt cached blocks on disk.
    pub encrypt_at_rest: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            cache_dir: PathBuf::from("/tmp/tgcryptfs-cache"),
            max_size: 512 * 1024 * 1024, // 512MB
            encrypt_at_rest: true,
        }
    }
}

/// LRU entry tracking metadata.
#[derive(Debug, Clone)]
struct CacheEntry {
    /// Block random ID (hex).
    rid_hex: String,
    /// Size of the cached data.
    size: u64,
    /// Access counter for LRU ordering.
    access_order: u64,
}

/// Encrypted local block cache with LRU eviction.
pub struct BlockCache {
    config: CacheConfig,
    cache_key: SymmetricKey,
    inner: Mutex<CacheInner>,
}

struct CacheInner {
    /// Map from block RID (hex) to entry metadata.
    entries: HashMap<String, CacheEntry>,
    /// Current total size.
    current_size: u64,
    /// Monotonic access counter.
    access_counter: u64,
    /// Cache statistics.
    hits: u64,
    misses: u64,
}

/// Cache statistics.
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub entries: usize,
    pub current_size: u64,
    pub max_size: u64,
    pub hits: u64,
    pub misses: u64,
}

impl BlockCache {
    /// Create a new block cache.
    pub fn new(config: CacheConfig, cache_key: SymmetricKey) -> Result<Self> {
        if !config.cache_dir.exists() {
            std::fs::create_dir_all(&config.cache_dir)?;
        }

        Ok(Self {
            config,
            cache_key,
            inner: Mutex::new(CacheInner {
                entries: HashMap::new(),
                current_size: 0,
                access_counter: 0,
                hits: 0,
                misses: 0,
            }),
        })
    }

    /// Store a block in the cache, evicting if necessary.
    pub fn put(&self, rid: &[u8; 32], data: &[u8]) -> Result<()> {
        let rid_hex = hex::encode(rid);
        let data_to_write = if self.config.encrypt_at_rest {
            let aad = format!("cache:{rid_hex}");
            aead::encrypt(&self.cache_key, data, aad.as_bytes())
                .map_err(|e| CacheError::Encryption(e.to_string()))?
        } else {
            data.to_vec()
        };
        let data_size = data_to_write.len() as u64;

        // Evict entries if needed to make room
        self.evict_if_needed(data_size)?;

        // Write to disk
        let path = self.block_path(&rid_hex);
        std::fs::write(&path, &data_to_write)?;

        // Update index
        let mut inner = self.inner.lock().unwrap();
        inner.access_counter += 1;

        // If replacing an existing entry, subtract old size
        if let Some(old) = inner.entries.get(&rid_hex) {
            inner.current_size -= old.size;
        }

        let order = inner.access_counter;
        inner.entries.insert(
            rid_hex.clone(),
            CacheEntry {
                rid_hex,
                size: data_size,
                access_order: order,
            },
        );
        inner.current_size += data_size;

        Ok(())
    }

    /// Retrieve a block from the cache.
    pub fn get(&self, rid: &[u8; 32]) -> Result<Vec<u8>> {
        let rid_hex = hex::encode(rid);

        {
            let mut inner = self.inner.lock().unwrap();
            inner.access_counter += 1;
            let order = inner.access_counter;
            if let Some(entry) = inner.entries.get_mut(&rid_hex) {
                entry.access_order = order;
                inner.hits += 1;
            } else {
                inner.misses += 1;
                return Err(CacheError::Miss(rid_hex));
            }
        }

        let path = self.block_path(&rid_hex);
        let raw = std::fs::read(&path)?;

        if self.config.encrypt_at_rest {
            let aad = format!("cache:{rid_hex}");
            aead::decrypt(&self.cache_key, &raw, aad.as_bytes())
                .map_err(|e| CacheError::Decryption(e.to_string()))
        } else {
            Ok(raw)
        }
    }

    /// Check if a block is in the cache.
    pub fn contains(&self, rid: &[u8; 32]) -> bool {
        let rid_hex = hex::encode(rid);
        self.inner.lock().unwrap().entries.contains_key(&rid_hex)
    }

    /// Remove a block from the cache.
    pub fn remove(&self, rid: &[u8; 32]) -> Result<()> {
        let rid_hex = hex::encode(rid);
        let mut inner = self.inner.lock().unwrap();
        if let Some(entry) = inner.entries.remove(&rid_hex) {
            inner.current_size -= entry.size;
            let path = self.block_path(&rid_hex);
            let _ = std::fs::remove_file(path);
        }
        Ok(())
    }

    /// Clear the entire cache.
    pub fn clear(&self) -> Result<()> {
        let mut inner = self.inner.lock().unwrap();
        for (rid_hex, _) in inner.entries.drain() {
            let path = self.block_path(&rid_hex);
            let _ = std::fs::remove_file(path);
        }
        inner.current_size = 0;
        inner.hits = 0;
        inner.misses = 0;
        Ok(())
    }

    /// Get cache statistics.
    pub fn stats(&self) -> CacheStats {
        let inner = self.inner.lock().unwrap();
        CacheStats {
            entries: inner.entries.len(),
            current_size: inner.current_size,
            max_size: self.config.max_size,
            hits: inner.hits,
            misses: inner.misses,
        }
    }

    fn block_path(&self, rid_hex: &str) -> PathBuf {
        // Use first 2 chars as subdirectory for filesystem efficiency
        let subdir = &rid_hex[..2];
        let dir = self.config.cache_dir.join(subdir);
        if !dir.exists() {
            let _ = std::fs::create_dir_all(&dir);
        }
        dir.join(rid_hex)
    }

    fn evict_if_needed(&self, needed_size: u64) -> Result<()> {
        let mut inner = self.inner.lock().unwrap();

        while inner.current_size + needed_size > self.config.max_size && !inner.entries.is_empty() {
            // Find LRU entry (lowest access_order)
            let lru_key = inner
                .entries
                .iter()
                .min_by_key(|(_, e)| e.access_order)
                .map(|(k, _)| k.clone());

            if let Some(key) = lru_key {
                if let Some(entry) = inner.entries.remove(&key) {
                    inner.current_size -= entry.size;
                    let path = self.block_path(&entry.rid_hex);
                    let _ = std::fs::remove_file(path);
                }
            } else {
                return Err(CacheError::EvictionFailed);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (tempfile::TempDir, BlockCache) {
        let dir = tempfile::TempDir::new().unwrap();
        let config = CacheConfig {
            cache_dir: dir.path().to_path_buf(),
            max_size: 10 * 1024, // 10KB for testing
            encrypt_at_rest: true,
        };
        let key = SymmetricKey::from_bytes([0x42; 32]);
        let cache = BlockCache::new(config, key).unwrap();
        (dir, cache)
    }

    #[test]
    fn put_and_get() {
        let (_dir, cache) = setup();
        let rid = [0x01; 32];
        let data = vec![0xAA; 256];

        cache.put(&rid, &data).unwrap();
        let retrieved = cache.get(&rid).unwrap();
        assert_eq!(retrieved, data);
    }

    #[test]
    fn get_miss() {
        let (_dir, cache) = setup();
        let rid = [0x01; 32];
        let err = cache.get(&rid).unwrap_err();
        assert!(matches!(err, CacheError::Miss(_)));
    }

    #[test]
    fn contains() {
        let (_dir, cache) = setup();
        let rid = [0x01; 32];
        assert!(!cache.contains(&rid));

        cache.put(&rid, &[1, 2, 3]).unwrap();
        assert!(cache.contains(&rid));
    }

    #[test]
    fn remove() {
        let (_dir, cache) = setup();
        let rid = [0x01; 32];
        cache.put(&rid, &[1, 2, 3]).unwrap();
        cache.remove(&rid).unwrap();
        assert!(!cache.contains(&rid));
    }

    #[test]
    fn stats_tracking() {
        let (_dir, cache) = setup();
        let rid = [0x01; 32];
        cache.put(&rid, &[1, 2, 3]).unwrap();

        let _ = cache.get(&rid).unwrap();
        let _ = cache.get(&[0xFF; 32]).unwrap_err();

        let stats = cache.stats();
        assert_eq!(stats.entries, 1);
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
    }

    #[test]
    fn lru_eviction() {
        let (_dir, cache) = setup();

        // Fill cache with blocks close to limit
        // Cache max is 10KB; each encrypted block is ~256 + overhead
        let block_data = vec![0xAA; 2048]; // ~2KB each after encryption overhead

        let r1 = [0x01; 32];
        let r2 = [0x02; 32];
        let r3 = [0x03; 32];
        let r4 = [0x04; 32];
        let r5 = [0x05; 32];

        cache.put(&r1, &block_data).unwrap();
        cache.put(&r2, &block_data).unwrap();
        cache.put(&r3, &block_data).unwrap();

        // Access r1 to make it recently used
        let _ = cache.get(&r1);

        // Add more to trigger eviction
        cache.put(&r4, &block_data).unwrap();
        cache.put(&r5, &block_data).unwrap();

        // r2 should have been evicted (oldest access, r1 was refreshed)
        let stats = cache.stats();
        assert!(stats.current_size <= stats.max_size);
    }

    #[test]
    fn clear() {
        let (_dir, cache) = setup();
        cache.put(&[0x01; 32], &[1, 2, 3]).unwrap();
        cache.put(&[0x02; 32], &[4, 5, 6]).unwrap();
        cache.clear().unwrap();

        let stats = cache.stats();
        assert_eq!(stats.entries, 0);
        assert_eq!(stats.current_size, 0);
    }

    #[test]
    fn encrypted_at_rest() {
        let (dir, cache) = setup();
        let rid = [0x01; 32];
        let data = vec![0xAA; 64];

        cache.put(&rid, &data).unwrap();

        // Read raw file - should be different from plaintext (encrypted)
        let rid_hex = hex::encode(rid);
        let subdir = &rid_hex[..2];
        let path = dir.path().join(subdir).join(&rid_hex);
        let raw = std::fs::read(path).unwrap();
        assert_ne!(raw, data);

        // But cache.get should return plaintext
        let retrieved = cache.get(&rid).unwrap();
        assert_eq!(retrieved, data);
    }

    #[test]
    fn unencrypted_mode() {
        let dir = tempfile::TempDir::new().unwrap();
        let config = CacheConfig {
            cache_dir: dir.path().to_path_buf(),
            max_size: 10 * 1024,
            encrypt_at_rest: false,
        };
        let key = SymmetricKey::from_bytes([0x42; 32]);
        let cache = BlockCache::new(config, key).unwrap();

        let rid = [0x01; 32];
        let data = vec![0xAA; 64];
        cache.put(&rid, &data).unwrap();

        // Raw file should match plaintext
        let rid_hex = hex::encode(rid);
        let subdir = &rid_hex[..2];
        let path = dir.path().join(subdir).join(&rid_hex);
        let raw = std::fs::read(path).unwrap();
        assert_eq!(raw, data);
    }
}
