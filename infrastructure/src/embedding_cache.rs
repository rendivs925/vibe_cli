use crate::ollama_client::OllamaClient;
use md5;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use shared::types::Result;
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct EmbeddingCacheConfig {
    pub similarity_threshold: f32,
    pub ttl_seconds: i64,
    pub max_entries: i64,
    pub enable_deduplication: bool,
}

impl Default for EmbeddingCacheConfig {
    fn default() -> Self {
        Self {
            similarity_threshold: 0.7,
            ttl_seconds: 604800,
            max_entries: 10000,
            enable_deduplication: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub id: String,
    pub query: String,
    pub response: String,
    pub embedding: Vec<f32>,
    pub timestamp: i64,
    pub hit_count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub total_entries: i64,
    pub cache_hits: i64,
    pub cache_misses: i64,
    pub hit_rate: f64,
}

pub struct EmbeddingCache {
    conn: Arc<Mutex<Connection>>,
    client: OllamaClient,
    config: EmbeddingCacheConfig,
    stats: Arc<Mutex<CacheStats>>,
}

impl EmbeddingCache {
    pub async fn new(db_path: impl AsRef<Path>, client: OllamaClient) -> Result<Self> {
        Self::with_config(db_path, client, EmbeddingCacheConfig::default()).await
    }

    pub async fn with_config(
        db_path: impl AsRef<Path>,
        client: OllamaClient,
        config: EmbeddingCacheConfig,
    ) -> Result<Self> {
        let db_path = db_path.as_ref().to_path_buf();
        
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&db_path)?;
        
        conn.execute_batch(
            "
            PRAGMA journal_mode=WAL;
            PRAGMA synchronous=NORMAL;
            PRAGMA cache_size=-64000;
            PRAGMA temp_store=MEMORY;
            
            CREATE TABLE IF NOT EXISTS embedding_cache (
                id TEXT PRIMARY KEY,
                query TEXT NOT NULL,
                response TEXT NOT NULL,
                embedding BLOB NOT NULL,
                timestamp INTEGER NOT NULL,
                hit_count INTEGER DEFAULT 1
            );
            
            CREATE INDEX IF NOT EXISTS idx_cache_timestamp ON embedding_cache(timestamp);
            CREATE INDEX IF NOT EXISTS idx_cache_query ON embedding_cache(query);
            "
        )?;

        let stats = CacheStats {
            total_entries: 0,
            cache_hits: 0,
            cache_misses: 0,
            hit_rate: 0.0,
        };

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            client,
            config,
            stats: Arc::new(Mutex::new(stats)),
        })
    }

    pub async fn get(&self, query: &str) -> Result<Option<String>> {
        let query_embedding = self.client.generate_embedding(query).await?;
        
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        
        let cutoff = now - self.config.ttl_seconds;

        let rows = {
            let conn = self.conn.lock().await;
            let mut stmt = conn.prepare(
                "SELECT id, query, response, embedding, timestamp, hit_count 
                 FROM embedding_cache 
                 WHERE timestamp > ?"
            )?;
            
            let mut results = Vec::new();
            let mut rows = stmt.query(params![cutoff])?;
            while let Some(row) = rows.next()? {
                let id: String = row.get(0)?;
                let query: String = row.get(1)?;
                let response: String = row.get(2)?;
                let embedding_bytes: Vec<u8> = row.get(3)?;
                let timestamp: i64 = row.get(4)?;
                let hit_count: i32 = row.get(5)?;
                let embedding: Vec<f32> = bincode::deserialize(&embedding_bytes).unwrap_or_default();
                results.push((id, query, response, embedding, timestamp, hit_count));
            }
            results
        };

        let mut best_match: Option<(String, String, f32)> = None;

        for (id, _cached_query, _response, embedding, _timestamp, _hit_count) in rows {
            let similarity = cosine_similarity(&query_embedding, &embedding);
            if similarity >= self.config.similarity_threshold {
                if let Some((_, _, best_sim)) = &best_match {
                    if similarity > *best_sim {
                        best_match = Some((id, _response, similarity));
                    }
                } else {
                    best_match = Some((id, _response, similarity));
                }
            }
        }

        if let Some((id, _response, _similarity)) = best_match {
            self.increment_hit_count(&id).await?;
            
            let mut stats = self.stats.lock().await;
            stats.cache_hits += 1;
            stats.hit_rate = stats.cache_hits as f64 / (stats.cache_hits + stats.cache_misses) as f64;

            let response = {
                let conn = self.conn.lock().await;
                let mut stmt = conn.prepare("SELECT response FROM embedding_cache WHERE id = ?")?;
                stmt.query_row(params![id], |row| row.get(0))?
            };

            Ok(Some(response))
        } else {
            let mut stats = self.stats.lock().await;
            stats.cache_misses += 1;
            stats.hit_rate = stats.cache_hits as f64 / (stats.cache_hits + stats.cache_misses).max(1) as f64;

            Ok(None)
        }
    }

    pub async fn set(&self, query: &str, response: &str) -> Result<()> {
        let query_embedding = self.client.generate_embedding(query).await?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        
        let id = format!("{:x}", md5::compute(query.as_bytes()));
        let embedding_bytes = bincode::serialize(&query_embedding)?;
        
        {
            let conn = self.conn.lock().await;
            conn.execute(
                "INSERT OR REPLACE INTO embedding_cache (id, query, response, embedding, timestamp, hit_count) 
                 VALUES (?, ?, ?, ?, ?, 1)",
                params![id, query, response, embedding_bytes, now],
            )?;
        }

        self.cleanup_if_needed().await?;

        let mut stats = self.stats.lock().await;
        stats.total_entries += 1;

        Ok(())
    }

    async fn increment_hit_count(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "UPDATE embedding_cache SET hit_count = hit_count + 1 WHERE id = ?",
            params![id],
        )?;
        Ok(())
    }

    async fn cleanup_if_needed(&self) -> Result<()> {
        let count: i64 = {
            let conn = self.conn.lock().await;
            conn.query_row(
                "SELECT COUNT(*) FROM embedding_cache",
                [],
                |row| row.get(0),
            )?
        };

        if count > self.config.max_entries {
            let to_delete = count - self.config.max_entries;
            let conn = self.conn.lock().await;
            conn.execute(
                "DELETE FROM embedding_cache WHERE id IN (
                    SELECT id FROM embedding_cache ORDER BY hit_count ASC, timestamp ASC LIMIT ?)",
                params![to_delete],
            )?;
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let cutoff = now - self.config.ttl_seconds;

        let conn = self.conn.lock().await;
        conn.execute(
            "DELETE FROM embedding_cache WHERE timestamp < ?",
            params![cutoff],
        )?;

        Ok(())
    }

    pub async fn get_stats(&self) -> CacheStats {
        let stats = self.stats.lock().await;
        
        let total: i64 = {
            let conn = self.conn.lock().await;
            conn.query_row(
                "SELECT COUNT(*) FROM embedding_cache",
                [],
                |row| row.get(0),
            ).unwrap_or(0)
        };

        CacheStats {
            total_entries: total,
            cache_hits: stats.cache_hits,
            cache_misses: stats.cache_misses,
            hit_rate: stats.hit_rate,
        }
    }

    pub async fn clear(&self) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute("DELETE FROM embedding_cache", [])?;

        let mut stats = self.stats.lock().await;
        stats.total_entries = 0;
        stats.cache_hits = 0;
        stats.cache_misses = 0;
        stats.hit_rate = 0.0;

        Ok(())
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if mag_a == 0.0 || mag_b == 0.0 {
        return 0.0;
    }

    dot_product / (mag_a * mag_b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 0.0).abs() < 0.001);
    }
}
