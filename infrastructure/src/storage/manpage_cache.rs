//! SQLite cache for parsed man pages
//!
//! Persistent storage for man page parsing results to avoid
//! re-parsing the same man pages repeatedly.

use crate::manpage_crawler::{Flag, FlagCategory, ManpageEntry};
use rusqlite::{params, Connection, Result as SqliteResult};
use std::path::Path;

/// SQLite-backed storage for man page entries
pub struct ManpageCache {
    conn: Connection,
}

impl ManpageCache {
    /// Initialize cache at given path
    pub fn new<P: AsRef<Path>>(db_path: P) -> SqliteResult<Self> {
        let conn = Connection::open(db_path)?;
        let cache = Self { conn };
        cache.init_tables()?;
        Ok(cache)
    }

    /// Initialize database tables
    fn init_tables(&self) -> SqliteResult<()> {
        // Main man page entries table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS manpage_entries (
                command TEXT PRIMARY KEY,
                version TEXT,
                section TEXT,
                parsed_at TEXT NOT NULL
            )",
            [],
        )?;

        // Flags table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS manpage_flags (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                command TEXT NOT NULL,
                flag_name TEXT NOT NULL,
                flag_type TEXT NOT NULL, -- 'short' or 'long'
                takes_value INTEGER NOT NULL,
                value_name TEXT,
                description TEXT NOT NULL,
                category TEXT NOT NULL
            )",
            [],
        )?;

        // Indexes for performance
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_flags_command ON manpage_flags(command)",
            [],
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_flags_name ON manpage_flags(flag_name)",
            [],
        )?;

        Ok(())
    }

    /// Store a man page entry
    pub fn store(&self, entry: &ManpageEntry) -> SqliteResult<()> {
        // Insert or update main entry
        self.conn.execute(
            "INSERT OR REPLACE INTO manpage_entries (command, version, section, parsed_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![entry.command, entry.version, entry.section, entry.parsed_at,],
        )?;

        // Delete old flags for this command
        self.conn.execute(
            "DELETE FROM manpage_flags WHERE command = ?1",
            [&entry.command],
        )?;

        // Insert short flags
        for flag in &entry.short_flags {
            self.insert_flag(&entry.command, flag, "short")?;
        }

        // Insert long flags
        for flag in &entry.long_flags {
            self.insert_flag(&entry.command, flag, "long")?;
        }

        Ok(())
    }

    /// Insert a single flag
    fn insert_flag(&self, command: &str, flag: &Flag, flag_type: &str) -> SqliteResult<()> {
        self.conn.execute(
            "INSERT INTO manpage_flags 
             (command, flag_name, flag_type, takes_value, value_name, description, category)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                command,
                flag.name,
                flag_type,
                flag.takes_value as i32,
                flag.value_name,
                flag.description,
                format!("{:?}", flag.category),
            ],
        )?;
        Ok(())
    }

    /// Retrieve a man page entry
    pub fn get(&self, command: &str) -> SqliteResult<Option<ManpageEntry>> {
        // Get main entry
        let mut stmt = self.conn.prepare(
            "SELECT version, section, parsed_at FROM manpage_entries WHERE command = ?1",
        )?;

        let entry_result = stmt.query_row([command], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
            ))
        });

        let (version, section, parsed_at) = match entry_result {
            Ok(data) => data,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(e),
        };

        // Get flags
        let short_flags = self.get_flags(command, "short")?;
        let long_flags = self.get_flags(command, "long")?;

        Ok(Some(ManpageEntry {
            command: command.to_string(),
            short_flags,
            long_flags,
            version,
            section,
            parsed_at,
        }))
    }

    /// Get flags for a command
    fn get_flags(&self, command: &str, flag_type: &str) -> SqliteResult<Vec<Flag>> {
        let mut stmt = self.conn.prepare(
            "SELECT flag_name, takes_value, value_name, description, category 
             FROM manpage_flags 
             WHERE command = ?1 AND flag_type = ?2",
        )?;

        let flags = stmt.query_map([command, flag_type], |row| {
            let category_str: String = row.get(4)?;
            let category = parse_category(&category_str);

            Ok(Flag {
                name: row.get(0)?,
                takes_value: row.get::<_, i32>(1)? != 0,
                value_name: row.get(2)?,
                description: row.get(3)?,
                category,
            })
        })?;

        flags.collect()
    }

    /// Check if a flag exists for a command
    pub fn has_flag(&self, command: &str, flag_name: &str) -> SqliteResult<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM manpage_flags WHERE command = ?1 AND flag_name = ?2",
            params![command, flag_name],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Get all valid flags for a command
    pub fn get_valid_flags(&self, command: &str) -> SqliteResult<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT flag_name FROM manpage_flags WHERE command = ?1")?;

        let flags = stmt.query_map([command], |row| row.get::<_, String>(0))?;

        flags.collect()
    }

    /// Check if entry is stale (older than given days)
    pub fn is_stale(&self, command: &str, max_age_days: i64) -> SqliteResult<bool> {
        let result = self.conn.query_row(
            "SELECT parsed_at FROM manpage_entries WHERE command = ?1",
            [command],
            |row| row.get::<_, String>(0),
        );

        match result {
            Ok(parsed_at) => {
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&parsed_at) {
                    let age = chrono::Local::now().signed_duration_since(dt);
                    Ok(age.num_days() > max_age_days)
                } else {
                    Ok(true) // Parse error = stale
                }
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(true), // Not found = stale
            Err(e) => Err(e),
        }
    }

    /// List all cached commands
    pub fn list_commands(&self) -> SqliteResult<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT command FROM manpage_entries")?;
        let commands = stmt.query_map([], |row| row.get::<_, String>(0))?;
        commands.collect()
    }

    /// Delete a cached entry
    pub fn delete(&self, command: &str) -> SqliteResult<()> {
        self.conn
            .execute("DELETE FROM manpage_entries WHERE command = ?1", [command])?;
        self.conn
            .execute("DELETE FROM manpage_flags WHERE command = ?1", [command])?;
        Ok(())
    }

    /// Clear all cached entries
    pub fn clear(&self) -> SqliteResult<()> {
        self.conn.execute("DELETE FROM manpage_flags", [])?;
        self.conn.execute("DELETE FROM manpage_entries", [])?;
        Ok(())
    }

    /// Get cache statistics
    pub fn stats(&self) -> SqliteResult<(usize, usize)> {
        let commands: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM manpage_entries", [], |row| row.get(0))?;

        let flags: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM manpage_flags", [], |row| row.get(0))?;

        Ok((commands as usize, flags as usize))
    }
}

/// Parse category from string representation
fn parse_category(s: &str) -> FlagCategory {
    match s {
        "Verbose" => FlagCategory::Verbose,
        "Output" => FlagCategory::Output,
        "Input" => FlagCategory::Input,
        "Filter" => FlagCategory::Filter,
        "Format" => FlagCategory::Format,
        "Recursive" => FlagCategory::Recursive,
        "Force" => FlagCategory::Force,
        "Help" => FlagCategory::Help,
        "Version" => FlagCategory::Version,
        _ => FlagCategory::General,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_db_path(prefix: &str) -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let dir = PathBuf::from(home).join(".config/vibe_cli/test_dbs");
        let dir = if std::fs::create_dir_all(&dir).is_ok()
            && std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .open(dir.join(".write_test"))
                .is_ok()
        {
            let _ = std::fs::remove_file(dir.join(".write_test"));
            dir
        } else {
            let fallback = PathBuf::from("/tmp/vibe_cli_test_dbs");
            let _ = std::fs::create_dir_all(&fallback);
            fallback
        };
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        dir.join(format!("{}_{}.db", prefix, nanos))
    }

    #[test]
    fn test_init_tables() {
        let db_path = test_db_path("mp_init");
        let _ = std::fs::remove_file(&db_path);
        let cache = ManpageCache::new(db_path.clone()).unwrap();
        let (commands, flags) = cache.stats().unwrap();
        assert_eq!(commands, 0);
        assert_eq!(flags, 0);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn test_store_and_retrieve() {
        let db_path = test_db_path("mp_store");
        let _ = std::fs::remove_file(&db_path);
        let cache = ManpageCache::new(db_path.clone()).unwrap();

        let entry = ManpageEntry {
            command: "testcmd".to_string(),
            short_flags: vec![Flag {
                name: "-t".to_string(),
                takes_value: false,
                value_name: None,
                description: "Test flag".to_string(),
                category: FlagCategory::General,
            }],
            long_flags: vec![],
            version: Some("1.0".to_string()),
            section: Some("1".to_string()),
            parsed_at: chrono::Local::now().to_rfc3339(),
        };

        cache.store(&entry).unwrap();

        let retrieved = cache.get("testcmd").unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.command, "testcmd");
        assert_eq!(retrieved.short_flags.len(), 1);

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn test_has_flag() {
        let db_path = test_db_path("mp_flag");
        let _ = std::fs::remove_file(&db_path);
        let cache = ManpageCache::new(db_path.clone()).unwrap();

        let entry = ManpageEntry {
            command: "testcmd".to_string(),
            short_flags: vec![Flag {
                name: "-v".to_string(),
                takes_value: false,
                value_name: None,
                description: "Verbose".to_string(),
                category: FlagCategory::Verbose,
            }],
            long_flags: vec![],
            version: None,
            section: None,
            parsed_at: chrono::Local::now().to_rfc3339(),
        };

        cache.store(&entry).unwrap();

        assert!(cache.has_flag("testcmd", "-v").unwrap());
        assert!(!cache.has_flag("testcmd", "-x").unwrap());

        let _ = std::fs::remove_file(&db_path);
    }
}
