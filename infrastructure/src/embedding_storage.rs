use domain::models::Embedding;
use rusqlite::{params, Connection, Result as SqlResult};
use shared::types::Result;
use std::fs;
use std::path::Path;

pub struct EmbeddingStorage {
    conn: Connection,
}

impl EmbeddingStorage {
    pub fn new(db_path: impl AsRef<Path>) -> Result<Self> {
        if let Some(parent) = db_path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        Self::setup_db(&conn)?;
        Ok(Self { conn })
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

    pub fn insert_embeddings(&self, embeddings: &[Embedding]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO embeddings (id, vector, text, path) VALUES (?, ?, ?, ?)",
            )?;
            for embedding in embeddings {
                let vector_bytes = serde_json::to_vec(&embedding.vector)?;
                stmt.execute(params![
                    embedding.id,
                    vector_bytes,
                    embedding.text,
                    embedding.path
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn get_all_embeddings(&self) -> Result<Vec<Embedding>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, vector, text, path FROM embeddings")?;
        let mut rows = stmt.query([])?;
        let mut embeddings = Vec::new();
        while let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            let vector_bytes: Vec<u8> = row.get(1)?;
            let text: String = row.get(2)?;
            let path: String = row.get(3)?;
            let vector: Vec<f32> = serde_json::from_slice(&vector_bytes)?;
            embeddings.push(Embedding {
                id,
                vector,
                text,
                path,
            });
        }
        Ok(embeddings)
    }

    pub fn get_file_hash(&self, path: &str) -> Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT hash FROM file_meta WHERE path = ?1")?;
        let mut rows = stmt.query([path])?;
        if let Some(row) = rows.next()? {
            let hash: String = row.get(0)?;
            return Ok(Some(hash));
        }
        Ok(None)
    }

    pub fn upsert_file_hash(&self, path: &str, hash: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO file_meta (path, hash) VALUES (?1, ?2)",
            params![path, hash],
        )?;
        Ok(())
    }

    pub fn delete_embeddings_for_path(&self, path: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM embeddings WHERE path = ?1", params![path])?;
        Ok(())
    }
}
