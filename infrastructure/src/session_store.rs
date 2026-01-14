use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sled::{Db, Tree};
use std::path::PathBuf;

/// Session metadata for listing and management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub last_used: DateTime<Utc>,
    pub goal_summary: String,
    pub change_count: u32,
    pub is_active: bool,
}

/// Complete session state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub metadata: SessionMetadata,
    pub conversation_history: Vec<ConversationMessage>,
    pub applied_changes: Vec<AppliedChange>,
    pub undo_stack: Vec<UndoEntry>,
    pub background_state: Option<serde_json::Value>,
}

/// Conversation message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub role: String, // "user" or "assistant"
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

/// Applied change record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppliedChange {
    pub id: String,
    pub description: String,
    pub timestamp: DateTime<Utc>,
    pub files_affected: Vec<String>,
}

/// Undo entry for rollback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoEntry {
    pub change_id: String,
    pub rollback_data: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}

/// Session store using sled for persistent storage
pub struct SessionStore {
    db: Db,
    sessions_tree: Tree,
    metadata_tree: Tree,
    project_hash: String,
}

impl SessionStore {
    /// Create a new session store for a project
    pub fn new(project_path: &str) -> Result<Self> {
        // Generate project hash using BLAKE3
        let project_hash = blake3::hash(project_path.as_bytes()).to_hex().to_string();

        // Create data directory
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let data_dir = PathBuf::from(home).join(".ai-agent").join("data");
        std::fs::create_dir_all(&data_dir).context("Failed to create data directory")?;

        // Open sled database
        let db_path = data_dir.join(format!("{}.sled", project_hash));
        let db = sled::open(&db_path).context("Failed to open sled database")?;

        // Get trees
        let sessions_tree = db
            .open_tree("sessions")
            .context("Failed to open sessions tree")?;
        let metadata_tree = db
            .open_tree("metadata")
            .context("Failed to open metadata tree")?;

        Ok(Self {
            db,
            sessions_tree,
            metadata_tree,
            project_hash,
        })
    }

    /// Create or load a session
    pub fn get_or_create_session(&self, session_name: &str) -> Result<Session> {
        // Try to load existing session
        if let Ok(Some(session)) = self.load_session(session_name) {
            return Ok(session);
        }

        // Create new session
        let now = Utc::now();
        let metadata = SessionMetadata {
            name: session_name.to_string(),
            created_at: now,
            last_used: now,
            goal_summary: "".to_string(),
            change_count: 0,
            is_active: true,
        };

        let session = Session {
            metadata,
            conversation_history: Vec::new(),
            applied_changes: Vec::new(),
            undo_stack: Vec::new(),
            background_state: None,
        };

        // Save the new session
        self.save_session(&session)?;

        Ok(session)
    }

    /// Load a session from storage
    pub fn load_session(&self, session_name: &str) -> Result<Option<Session>> {
        let key = format!("session:{}", session_name);

        match self.sessions_tree.get(key.as_bytes())? {
            Some(data) => {
                let session: Session = serde_json::from_slice(data.as_ref())
                    .context("Failed to deserialize session")?;
                Ok(Some(session))
            }
            None => Ok(None),
        }
    }

    /// Save a session to storage
    pub fn save_session(&self, session: &Session) -> Result<()> {
        let key = format!("session:{}", session.metadata.name);
        let data = serde_json::to_vec(session).context("Failed to serialize session")?;

        self.sessions_tree.insert(key.as_bytes(), data.as_slice())?;
        self.sessions_tree.flush()?;

        // Update session list
        self.update_session_list(session)?;

        Ok(())
    }

    /// List all sessions
    pub fn list_sessions(&self) -> Result<Vec<SessionMetadata>> {
        let list_key = "session:list";

        match self.metadata_tree.get(list_key.as_bytes())? {
            Some(data) => {
                let sessions: Vec<SessionMetadata> = serde_json::from_slice(data.as_ref())
                    .context("Failed to deserialize session list")?;
                Ok(sessions)
            }
            None => Ok(Vec::new()),
        }
    }

    /// Delete a session
    pub fn delete_session(&self, session_name: &str) -> Result<()> {
        let session_key = format!("session:{}", session_name);

        // Remove from sessions tree
        self.sessions_tree.remove(session_key.as_bytes())?;

        // Update session list
        let mut sessions = self.list_sessions()?;
        sessions.retain(|s| s.name != session_name);

        let data =
            serde_json::to_vec(&sessions).context("Failed to serialize updated session list")?;
        self.metadata_tree
            .insert("session:list".as_bytes(), data.as_slice())?;
        self.metadata_tree.flush()?;

        Ok(())
    }

    /// Get the default session (creates "main" if none exists)
    pub fn get_default_session(&self) -> Result<Session> {
        self.get_or_create_session("main")
    }

    /// Update session list with metadata
    fn update_session_list(&self, session: &Session) -> Result<()> {
        let mut sessions = self.list_sessions()?;

        // Remove existing entry for this session
        sessions.retain(|s| s.name != session.metadata.name);

        // Add updated metadata
        sessions.push(session.metadata.clone());

        let data = serde_json::to_vec(&sessions).context("Failed to serialize session list")?;
        self.metadata_tree
            .insert("session:list".as_bytes(), data.as_slice())?;
        self.metadata_tree.flush()?;

        Ok(())
    }

    /// Export session to JSON file for backup
    pub fn export_session(&self, session_name: &str) -> Result<PathBuf> {
        let session = self
            .load_session(session_name)?
            .context("Session not found")?;

        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let export_dir = PathBuf::from(home).join(".ai-agent").join("sessions");
        std::fs::create_dir_all(&export_dir).context("Failed to create export directory")?;

        let filename = format!("{}-{}.json", self.project_hash, session_name);
        let export_path = export_dir.join(filename);

        let json_data = serde_json::to_string_pretty(&session)
            .context("Failed to serialize session to JSON")?;
        std::fs::write(&export_path, json_data).context("Failed to write session export file")?;

        Ok(export_path)
    }

    /// Get project hash
    pub fn project_hash(&self) -> &str {
        &self.project_hash
    }
}

impl Drop for SessionStore {
    fn drop(&mut self) {
        let _ = self.db.flush();
    }
}
