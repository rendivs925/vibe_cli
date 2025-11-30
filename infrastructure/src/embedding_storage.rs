use rusqlite::{Connection, params, Result as SqlResult};
use std::path::Path;
use domain::models::Embedding;
use shared::types::Result;

pub struct EmbeddingStorage {
    conn: Connection,
}

impl EmbeddingStorage {
    pub fn new(db_path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        Self::setup_db(&conn)?;
        Ok(Self { conn })
    }

    fn setup_db(conn: &Connection) -> SqlResult<()> {
        conn.execute_batch("
            PRAGMA journal_mode=WAL;
            PRAGMA synchronous=NORMAL;
            PRAGMA cache_size=-64000;
            PRAGMA temp_store=MEMORY;
            CREATE TABLE IF NOT EXISTS embeddings (
                id TEXT PRIMARY KEY,
                vector BLOB NOT NULL,
                text TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_embeddings_vector ON embeddings(vector);
        ")?;
        Ok(())
    }

    pub fn insert_embeddings(&self, embeddings: &[Embedding]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare("INSERT OR REPLACE INTO embeddings (id, vector, text) VALUES (?, ?, ?)")?;
            for embedding in embeddings {
                let vector_bytes = serde_json::to_vec(&embedding.vector)?;
                stmt.execute(params![embedding.id, vector_bytes, embedding.text])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn get_all_embeddings(&self) -> Result<Vec<Embedding>> {
        let mut stmt = self.conn.prepare("SELECT id, vector, text FROM embeddings")?;
        let mut rows = stmt.query([])?;
        let mut embeddings = Vec::new();
        while let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            let vector_bytes: Vec<u8> = row.get(1)?;
            let text: String = row.get(2)?;
            let vector: Vec<f32> = serde_json::from_slice(&vector_bytes)?;
            embeddings.push(Embedding { id, vector, text });
        }
        Ok(embeddings)
    }
}