use domain::models::Embedding;
use rusqlite::{params, Connection, Result as SqlResult};
use shared::types::Result;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task;

/// Basic storage implementation with Qdrant placeholder
/// TODO: Implement full Qdrant integration with proper API version
pub struct QdrantStorage {
    sqlite_fallback: Arc<Mutex<Connection>>,
    collection_name: String,
    vector_dim: usize,
    qdrant_available: bool,
}

impl QdrantStorage {
    /// Create new storage (currently uses SQLite with Qdrant placeholder)
    pub async fn new(
        _qdrant_url: Option<String>,
        sqlite_path: impl AsRef<Path>,
        collection_name: String,
        vector_dim: usize,
    ) -> Result<Self> {
        // Initialize SQLite fallback
        let sqlite_path = sqlite_path.as_ref().to_path_buf();
        let conn = task::spawn_blocking(move || -> Result<Connection> {
            if let Some(parent) = sqlite_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let conn = Connection::open(&sqlite_path)?;
            Self::setup_sqlite_db(&conn)?;
            Ok(conn)
        }).await??;

        let sqlite_fallback = Arc::new(Mutex::new(conn));

        // TODO: Implement full Qdrant integration when API stabilizes
        let qdrant_available = false;

        Ok(Self {
            sqlite_fallback,
            collection_name,
            vector_dim,
            qdrant_available,
        })
    }

    /// Setup SQLite database schema
    fn setup_sqlite_db(conn: &Connection) -> SqlResult<()> {
        conn.execute_batch(
            "
            PRAGMA journal_mode=WAL;
            PRAGMA synchronous=NORMAL;
            PRAGMA cache_size=-64000;
            PRAGMA temp_store=MEMORY;
            CREATE TABLE IF NOT EXISTS embeddings (
                id TEXT PRIMARY KEY,
                vector BLOB NOT NULL,
                text TEXT NOT NULL,
                path TEXT NOT NULL DEFAULT ''
            );
            CREATE INDEX IF NOT EXISTS idx_embeddings_vector ON embeddings(vector);
            CREATE TABLE IF NOT EXISTS file_meta (
                path TEXT PRIMARY KEY,
                hash TEXT NOT NULL
            );
        ",
        )?;

        // Backfill missing path column for existing DBs.
        let mut stmt = conn.prepare("PRAGMA table_info(embeddings)")?;
        let mut rows = stmt.query([])?;
        let mut has_path = false;
        while let Some(row) = rows.next()? {
            let col_name: String = row.get(1)?;
            if col_name == "path" {
                has_path = true;
                break;
            }
        }
        if !has_path {
            conn.execute(
                "ALTER TABLE embeddings ADD COLUMN path TEXT NOT NULL DEFAULT ''",
                [],
            )?;
        }
        // Ensure the path index exists once the column is known to be present.
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_embeddings_path ON embeddings(path)",
            [],
        )?;
        Ok(())
    }

    /// Insert embeddings using SQLite (Qdrant placeholder)
    pub async fn insert_embeddings(&self, embeddings: Vec<Embedding>) -> Result<()> {
        let conn = Arc::clone(&self.sqlite_fallback);
        let embeddings = embeddings.to_vec();

        task::spawn_blocking(move || -> Result<()> {
            let conn = conn.blocking_lock();
            let tx = conn.unchecked_transaction()?;
            {
                let mut stmt = tx.prepare(
                    "INSERT OR REPLACE INTO embeddings (id, vector, text, path) VALUES (?, ?, ?, ?)",
                )?;
                for embedding in &embeddings {
                    let vector_bytes = bincode::serialize(&embedding.vector)?;
                    stmt.execute(params![
                        &embedding.id,
                        vector_bytes,
                        &embedding.text,
                        &embedding.path
                    ])?;
                }
            }
            tx.commit()?;
            Ok(())
        }).await?;

        eprintln!("Embeddings stored successfully (SQLite fallback)");
        Ok(())
    }

    /// Search embeddings using SQLite (Qdrant placeholder)
    pub async fn search_similar(&self, query_vector: &[f32], limit: usize) -> Result<Vec<Embedding>> {
        let all_embeddings = self.get_all_embeddings().await?;
        let mut scored: Vec<(f32, Embedding)> = all_embeddings
            .into_iter()
            .map(|emb| {
                let score = Self::cosine_similarity(query_vector, &emb.vector);
                (score, emb)
            })
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        Ok(scored
            .into_iter()
            .take(limit)
            .map(|(_, emb)| emb)
            .collect())
    }

    /// Cosine similarity calculation
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot_product / (norm_a * norm_b)
        }
    }

    /// Get all embeddings from SQLite
    pub async fn get_all_embeddings(&self) -> Result<Vec<Embedding>> {
        let conn = Arc::clone(&self.sqlite_fallback);
        task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            let mut stmt = conn
                .prepare("SELECT id, vector, text, path FROM embeddings")?;
            let mut rows = stmt.query([])?;
            let mut embeddings = Vec::new();
            while let Some(row) = rows.next()? {
                let id: String = row.get(0)?;
                let vector_bytes: Vec<u8> = row.get(1)?;
                let text: String = row.get(2)?;
                let path: String = row.get(3)?;
                let vector: Vec<f32> = bincode::deserialize(&vector_bytes)?;
                embeddings.push(Embedding {
                    id,
                    vector,
                    text,
                    path,
                });
            }
            Ok(embeddings)
        }).await?
    }

    /// Get file hash from SQLite
    pub async fn get_file_hash(&self, path: String) -> Result<Option<String>> {
        let conn = Arc::clone(&self.sqlite_fallback);
        task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            let mut stmt = conn
                .prepare("SELECT hash FROM file_meta WHERE path = ?1")?;
            let mut rows = stmt.query([path])?;
            if let Some(row) = rows.next()? {
                let hash: String = row.get(0)?;
                return Ok(Some(hash));
            }
            Ok(None)
        }).await?
    }

    /// Upsert file hash in SQLite
    pub async fn upsert_file_hash(&self, path: String, hash: String) -> Result<()> {
        let conn = Arc::clone(&self.sqlite_fallback);
        task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            conn.execute(
                "INSERT OR REPLACE INTO file_meta (path, hash) VALUES (?1, ?2)",
                params![path, hash],
            )?;
            Ok(())
        }).await?
    }

    /// Delete embeddings for path
    pub async fn delete_embeddings_for_path(&self, path: String) -> Result<()> {
        let conn = Arc::clone(&self.sqlite_fallback);
        task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            conn.execute("DELETE FROM embeddings WHERE path = ?1", params![path])?;
            Ok(())
        }).await?
    }

    /// Get storage statistics
    pub async fn get_stats(&self) -> Result<HashMap<String, String>> {
        let mut stats = HashMap::new();
        stats.insert("qdrant_available".to_string(), self.qdrant_available.to_string());
        stats.insert("collection_name".to_string(), self.collection_name.clone());
        stats.insert("vector_dim".to_string(), self.vector_dim.to_string());

        // SQLite stats
        let conn = Arc::clone(&self.sqlite_fallback);
        let sqlite_count = task::spawn_blocking(move || -> Result<i64> {
            let conn = conn.blocking_lock();
            let mut stmt = conn.prepare("SELECT COUNT(*) FROM embeddings")?;
            let count: i64 = stmt.query_row([], |row| row.get(0))?;
            Ok(count)
        }).await?;

        if let Ok(count) = sqlite_count {
            stats.insert("sqlite_embeddings".to_string(), count.to_string());
        }

        Ok(stats)
    }
}