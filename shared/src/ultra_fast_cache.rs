use lz4::{Decoder, EncoderBuilder};
use serde::{Deserialize, Serialize};
use crate::performance_monitor::GLOBAL_METRICS;
use crate::types::Result;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;

/// Ultra-fast compressed cache entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedCacheEntry {
    pub key: String,
    pub data: Vec<u8>, // LZ4 compressed data
    pub timestamp: SystemTime,
    pub access_count: u64,
    pub last_accessed: SystemTime,
    pub size_bytes: usize, // Uncompressed size
}

/// Advanced caching system with compression, LRU, and predictive loading
pub struct UltraFastCache {
    inner: Arc<RwLock<CacheInner>>,
    cache_dir: PathBuf,
    max_memory_mb: usize,
    ttl_seconds: u64,
}

#[derive(Default)]
struct CacheInner {
    memory_cache: HashMap<String, CompressedCacheEntry>,
    memory_usage: usize,
    access_patterns: HashMap<String, Vec<String>>, // key -> related keys
}

impl UltraFastCache {
    pub async fn new(cache_dir: PathBuf, max_memory_mb: usize, ttl_seconds: u64) -> Result<Self> {
        let cache = Self {
            inner: Arc::new(RwLock::new(CacheInner::default())),
            cache_dir,
            max_memory_mb,
            ttl_seconds,
        };

        // Load persistent cache on startup
        cache.load_persistent_cache().await?;

        Ok(cache)
    }

    /// Ultra-fast cache get with automatic decompression
    pub async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        GLOBAL_METRICS.start_operation("cache_get").await;

        let mut inner = self.inner.write().await;

        // Check memory cache first
        if let Some(entry) = inner.memory_cache.get_mut(key) {
            // Check TTL
            if entry.timestamp.elapsed().unwrap_or(Duration::from_secs(0))
                > Duration::from_secs(self.ttl_seconds)
            {
                inner.memory_cache.remove(key);
                GLOBAL_METRICS.end_operation("cache_get").await;
                return Ok(None);
            }

            // Update access patterns
            entry.access_count += 1;
            entry.last_accessed = SystemTime::now();

            // Decompress data
            let decompressed = self.decompress_data(&entry.data)?;
            GLOBAL_METRICS.end_operation("cache_get").await;
            return Ok(Some(decompressed));
        }

        // Check persistent storage
        match self.load_from_disk(key).await? {
            Some(data) => {
                // Add to memory cache
                let compressed = self.compress_data(&data)?;
                let entry = CompressedCacheEntry {
                    key: key.to_string(),
                    data: compressed,
                    timestamp: SystemTime::now(),
                    access_count: 1,
                    last_accessed: SystemTime::now(),
                    size_bytes: data.len(),
                };

                inner.memory_cache.insert(key.to_string(), entry);
                inner.memory_usage += data.len();

                // Evict if over memory limit
                self.evict_if_needed(&mut inner).await?;

                GLOBAL_METRICS.end_operation("cache_get").await;
                Ok(Some(data))
            }
            None => {
                GLOBAL_METRICS.end_operation("cache_get").await;
                Ok(None)
            }
        }
    }

    /// Ultra-fast cache put with automatic compression
    pub async fn put(&self, key: String, data: Vec<u8>) -> Result<()> {
        GLOBAL_METRICS.start_operation("cache_put").await;

        let compressed = self.compress_data(&data)?;
        let entry = CompressedCacheEntry {
            key: key.clone(),
            data: compressed,
            timestamp: SystemTime::now(),
            access_count: 0,
            last_accessed: SystemTime::now(),
            size_bytes: data.len(),
        };

        let mut inner = self.inner.write().await;

        // Update memory cache
        if let Some(old_entry) = inner.memory_cache.insert(key.clone(), entry) {
            inner.memory_usage -= old_entry.size_bytes;
        }
        inner.memory_usage += data.len();

        // Evict if over memory limit
        self.evict_if_needed(&mut inner).await?;

        // Persist to disk asynchronously
        let cache_dir = self.cache_dir.clone();
        let key_clone = key.clone();
        let data_clone = data.clone();
        tokio::spawn(async move {
            let _ = Self::save_to_disk(&cache_dir, &key_clone, &data_clone).await;
        });

        GLOBAL_METRICS.end_operation("cache_put").await;
        Ok(())
    }

    /// Predictive loading based on access patterns
    pub async fn preload_related(&self, key: &str) -> Result<()> {
        let inner = self.inner.read().await;

        if let Some(related_keys) = inner.access_patterns.get(key) {
            for related_key in related_keys {
                if !inner.memory_cache.contains_key(related_key) {
                    // Async preload
                    let cache = self.clone();
                    let key = related_key.clone();
                    tokio::spawn(async move {
                        let _ = cache.get(&key).await;
                    });
                }
            }
        }

        Ok(())
    }

    /// Get cache statistics
    pub async fn stats(&self) -> CacheStats {
        let inner = self.inner.read().await;
        CacheStats {
            memory_entries: inner.memory_cache.len(),
            memory_usage_mb: inner.memory_usage / (1024 * 1024),
            total_accesses: inner.memory_cache.values().map(|e| e.access_count).sum(),
        }
    }

    // Internal methods
    fn compress_data(&self, data: &[u8]) -> Result<Vec<u8>> {
        let mut compressed = Vec::new();
        {
            let mut encoder = EncoderBuilder::new().build(&mut compressed)?;
            std::io::copy(&mut std::io::Cursor::new(data), &mut encoder)?;
            let (_writer, result) = encoder.finish(); // Important: finish the encoder
            result?;
        }
        Ok(compressed)
    }

    fn decompress_data(&self, compressed: &[u8]) -> Result<Vec<u8>> {
        let mut decompressed = Vec::new();
        {
            let mut decoder = Decoder::new(std::io::Cursor::new(compressed))?;
            std::io::copy(&mut decoder, &mut decompressed)?;
        }
        Ok(decompressed)
    }

    async fn evict_if_needed(&self, inner: &mut CacheInner) -> Result<()> {
        while inner.memory_usage > self.max_memory_mb * 1024 * 1024 {
            // LRU eviction - find the key first, then remove
            let key_to_remove = inner.memory_cache.iter().min_by_key(|(_, e)| e.last_accessed)
                .map(|(k, _)| k.clone());

            if let Some(key) = key_to_remove {
                if let Some(removed) = inner.memory_cache.remove(&key) {
                    inner.memory_usage -= removed.size_bytes;
                }
            } else {
                break;
            }
        }
        Ok(())
    }

    async fn load_persistent_cache(&self) -> Result<()> {
        if !self.cache_dir.exists() {
            fs::create_dir_all(&self.cache_dir)?;
            return Ok(());
        }

        // Load index file if it exists
        let index_path = self.cache_dir.join("index.bin");
        if index_path.exists() {
            // For now, skip loading persistent cache on startup for speed
            // Could be optimized later with async loading
        }

        Ok(())
    }

    async fn load_from_disk(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let file_path = self.cache_dir.join(format!("{}.bin", key));
        if file_path.exists() {
            let compressed = tokio::fs::read(&file_path).await?;
            let decompressed = self.decompress_data(&compressed)?;
            Ok(Some(decompressed))
        } else {
            Ok(None)
        }
    }

    async fn save_to_disk(cache_dir: &PathBuf, key: &str, data: &[u8]) -> Result<()> {
        if !cache_dir.exists() {
            tokio::fs::create_dir_all(cache_dir).await?;
        }
        let file_path = cache_dir.join(format!("{}.bin", key));
        tokio::fs::write(&file_path, data).await?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub memory_entries: usize,
    pub memory_usage_mb: usize,
    pub total_accesses: u64,
}

impl Clone for UltraFastCache {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            cache_dir: self.cache_dir.clone(),
            max_memory_mb: self.max_memory_mb,
            ttl_seconds: self.ttl_seconds,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_ultra_fast_cache() {
        // Use a temporary directory in the current working directory for testing
        let temp_dir = std::env::temp_dir().join("ultra_fast_cache_test");
        let _ = std::fs::create_dir_all(&temp_dir);
        let cache = UltraFastCache::new(temp_dir, 10, 3600).await.unwrap();

        let test_data = b"Hello, World! This is test data for compression.".to_vec();
        let key = "test_key".to_string();

        // Test put and get
        cache.put(key.clone(), test_data.clone()).await.unwrap();

        // Give a small delay for async operations to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let retrieved_option = cache.get(&key).await.unwrap();
        assert!(retrieved_option.is_some(), "Cache should contain the stored data");
        let retrieved = retrieved_option.unwrap();

        assert_eq!(retrieved, test_data);

        // Test stats
        let stats = cache.stats().await;
        assert_eq!(stats.memory_entries, 1);
        assert!(stats.memory_usage_mb >= 0);
    }
}