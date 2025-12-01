use domain::models::Embedding;
use rusqlite::{params, Connection, Result as SqlResult};
use shared::types::Result;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task;

pub struct EmbeddingStorage {
    conn: Arc<Mutex<Connection>>,
}

impl EmbeddingStorage {
    pub async fn new(db_path: impl AsRef<Path>) -> Result<Self> {
        let db_path = db_path.as_ref().to_path_buf();
        let conn = task::spawn_blocking(move || -> Result<Connection> {
            if let Some(parent) = db_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let conn = Connection::open(&db_path)?;
            Self::setup_db(&conn)?;
            Ok(conn)
        }).await??;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    fn setup_db(conn: &Connection) -> SqlResult<()> {
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

    pub async fn insert_embeddings(&self, embeddings: Vec<Embedding>) -> Result<()> {
        let conn = Arc::clone(&self.conn);
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
        eprintln!("Embeddings stored successfully");
        Ok(())
    }

    pub async fn get_all_embeddings(&self) -> Result<Vec<Embedding>> {
        let conn = Arc::clone(&self.conn);
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

    pub async fn get_file_hash(&self, path: String) -> Result<Option<String>> {
        let conn = Arc::clone(&self.conn);
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

    pub async fn upsert_file_hash(&self, path: String, hash: String) -> Result<()> {
        let conn = Arc::clone(&self.conn);
        task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            conn.execute(
                "INSERT OR REPLACE INTO file_meta (path, hash) VALUES (?1, ?2)",
                params![path, hash],
            )?;
            Ok(())
        }).await?
    }

    pub async fn delete_embeddings_for_path(&self, path: String) -> Result<()> {
        let conn = Arc::clone(&self.conn);
        task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            conn.execute("DELETE FROM embeddings WHERE path = ?1", params![path])?;
            Ok(())
        }).await?
    }
}
