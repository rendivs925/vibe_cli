//! Knowledge Graph - System state representation for neurosymbolic reasoning
//!
//! SQLite-based graph database tracking system entities:
//! - OS (distribution, version, kernel)
//! - Tools (installed commands, versions, paths)
//! - Permissions (sudo access, file permissions)
//! - Dependencies (package relationships)
//!
//! Enables contextual command generation based on actual system state.

use rusqlite::{params, Connection, OptionalExtension, Result as SqliteResult};
use std::collections::HashMap;
use std::path::Path;

/// Types of entities in the knowledge graph
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EntityType {
    OperatingSystem,
    Kernel,
    Distribution,
    Tool,
    Package,
    File,
    Directory,
    User,
    Group,
    Service,
    Process,
    NetworkInterface,
    NetworkConnection,
    Container,
    Filesystem,
    Mount,
    Hardware,
    Cpu,
    Memory,
    Disk,
    Permission,
    EnvironmentVariable,
    Configuration,
}

impl EntityType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EntityType::OperatingSystem => "os",
            EntityType::Kernel => "kernel",
            EntityType::Distribution => "distribution",
            EntityType::Tool => "tool",
            EntityType::Package => "package",
            EntityType::File => "file",
            EntityType::Directory => "directory",
            EntityType::User => "user",
            EntityType::Group => "group",
            EntityType::Service => "service",
            EntityType::Process => "process",
            EntityType::NetworkInterface => "network_interface",
            EntityType::NetworkConnection => "network_connection",
            EntityType::Container => "container",
            EntityType::Filesystem => "filesystem",
            EntityType::Mount => "mount",
            EntityType::Hardware => "hardware",
            EntityType::Cpu => "cpu",
            EntityType::Memory => "memory",
            EntityType::Disk => "disk",
            EntityType::Permission => "permission",
            EntityType::EnvironmentVariable => "env_var",
            EntityType::Configuration => "configuration",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "os" => Some(EntityType::OperatingSystem),
            "kernel" => Some(EntityType::Kernel),
            "distribution" => Some(EntityType::Distribution),
            "tool" => Some(EntityType::Tool),
            "package" => Some(EntityType::Package),
            "file" => Some(EntityType::File),
            "directory" => Some(EntityType::Directory),
            "user" => Some(EntityType::User),
            "group" => Some(EntityType::Group),
            "service" => Some(EntityType::Service),
            "process" => Some(EntityType::Process),
            "network_interface" => Some(EntityType::NetworkInterface),
            "network_connection" => Some(EntityType::NetworkConnection),
            "container" => Some(EntityType::Container),
            "filesystem" => Some(EntityType::Filesystem),
            "mount" => Some(EntityType::Mount),
            "hardware" => Some(EntityType::Hardware),
            "cpu" => Some(EntityType::Cpu),
            "memory" => Some(EntityType::Memory),
            "disk" => Some(EntityType::Disk),
            "permission" => Some(EntityType::Permission),
            "env_var" => Some(EntityType::EnvironmentVariable),
            "configuration" => Some(EntityType::Configuration),
            _ => None,
        }
    }
}

/// An entity in the knowledge graph
#[derive(Debug, Clone)]
pub struct Entity {
    pub id: i64,
    pub entity_type: EntityType,
    pub name: String,
    pub attributes: HashMap<String, String>,
    pub discovered_at: String,
    pub last_updated: String,
}

/// A relationship between entities
#[derive(Debug, Clone)]
pub struct Relationship {
    pub id: i64,
    pub from_entity: i64,
    pub to_entity: i64,
    pub rel_type: String,
    pub attributes: HashMap<String, String>,
}

/// Knowledge graph storage and query interface
pub struct KnowledgeGraph {
    conn: Connection,
}

impl KnowledgeGraph {
    /// Initialize knowledge graph at given path
    pub fn new<P: AsRef<Path>>(db_path: P) -> SqliteResult<Self> {
        let conn = Connection::open(db_path)?;
        let graph = Self { conn };
        graph.init_tables()?;
        Ok(graph)
    }

    /// Initialize database tables
    fn init_tables(&self) -> SqliteResult<()> {
        // Entities table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS entities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                entity_type TEXT NOT NULL,
                name TEXT NOT NULL,
                discovered_at TEXT NOT NULL,
                last_updated TEXT NOT NULL
            )",
            [],
        )?;

        // Entity attributes table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS entity_attributes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                entity_id INTEGER NOT NULL,
                key TEXT NOT NULL,
                value TEXT NOT NULL,
                FOREIGN KEY (entity_id) REFERENCES entities(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Relationships table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS relationships (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                from_entity INTEGER NOT NULL,
                to_entity INTEGER NOT NULL,
                rel_type TEXT NOT NULL,
                FOREIGN KEY (from_entity) REFERENCES entities(id) ON DELETE CASCADE,
                FOREIGN KEY (to_entity) REFERENCES entities(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Relationship attributes table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS relationship_attributes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                relationship_id INTEGER NOT NULL,
                key TEXT NOT NULL,
                value TEXT NOT NULL,
                FOREIGN KEY (relationship_id) REFERENCES relationships(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Indexes for performance
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_entities_type ON entities(entity_type)",
            [],
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_entities_name ON entities(name)",
            [],
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_rel_from ON relationships(from_entity)",
            [],
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_rel_to ON relationships(to_entity)",
            [],
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_rel_type ON relationships(rel_type)",
            [],
        )?;

        Ok(())
    }

    /// Add an entity to the graph
    pub fn add_entity(
        &self,
        entity_type: EntityType,
        name: &str,
        attributes: HashMap<String, String>,
    ) -> SqliteResult<i64> {
        let now = chrono::Utc::now().to_rfc3339();

        self.conn.execute(
            "INSERT INTO entities (entity_type, name, discovered_at, last_updated)
             VALUES (?1, ?2, ?3, ?4)",
            params![entity_type.as_str(), name, &now, &now],
        )?;

        let entity_id = self.conn.last_insert_rowid();

        // Add attributes
        for (key, value) in attributes {
            self.conn.execute(
                "INSERT INTO entity_attributes (entity_id, key, value)
                 VALUES (?1, ?2, ?3)",
                params![entity_id, key, value],
            )?;
        }

        Ok(entity_id)
    }

    /// Add or update an entity (replaces attributes if present)
    pub fn upsert_entity(
        &self,
        entity_type: EntityType,
        name: &str,
        attributes: HashMap<String, String>,
    ) -> SqliteResult<i64> {
        let mut stmt = self.conn.prepare(
            "SELECT id FROM entities WHERE entity_type = ?1 AND name = ?2 LIMIT 1",
        )?;

        let existing_id: Option<i64> = stmt
            .query_row(params![entity_type.as_str(), name], |row| row.get(0))
            .optional()?;

        if let Some(entity_id) = existing_id {
            let now = chrono::Utc::now().to_rfc3339();
            self.conn.execute(
                "UPDATE entities SET last_updated = ?1 WHERE id = ?2",
                params![&now, entity_id],
            )?;
            self.conn.execute(
                "DELETE FROM entity_attributes WHERE entity_id = ?1",
                params![entity_id],
            )?;
            for (key, value) in attributes {
                self.conn.execute(
                    "INSERT INTO entity_attributes (entity_id, key, value)
                     VALUES (?1, ?2, ?3)",
                    params![entity_id, key, value],
                )?;
            }
            Ok(entity_id)
        } else {
            self.add_entity(entity_type, name, attributes)
        }
    }

    /// Get entity by ID
    pub fn get_entity(&self, id: i64) -> SqliteResult<Option<Entity>> {
        let mut stmt = self.conn.prepare(
            "SELECT entity_type, name, discovered_at, last_updated
             FROM entities WHERE id = ?1",
        )?;

        let entity_result = stmt.query_row([id], |row| {
            let type_str: String = row.get(0)?;
            let entity_type = EntityType::from_str(&type_str).unwrap_or(EntityType::Configuration);

            Ok((
                entity_type,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        });

        match entity_result {
            Ok((entity_type, name, discovered_at, last_updated)) => {
                let attributes = self.get_entity_attributes(id)?;
                Ok(Some(Entity {
                    id,
                    entity_type,
                    name,
                    attributes,
                    discovered_at,
                    last_updated,
                }))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get entity attributes
    fn get_entity_attributes(&self, entity_id: i64) -> SqliteResult<HashMap<String, String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT key, value FROM entity_attributes WHERE entity_id = ?1")?;

        let attrs: Result<Vec<(String, String)>, _> = stmt
            .query_map([entity_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect();

        Ok(attrs?.into_iter().collect())
    }

    /// Find entity by name and type
    pub fn find_entity(&self, entity_type: EntityType, name: &str) -> SqliteResult<Option<Entity>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM entities WHERE entity_type = ?1 AND name = ?2")?;

        let id_result: Result<i64, _> =
            stmt.query_row(params![entity_type.as_str(), name], |row| row.get(0));

        match id_result {
            Ok(id) => self.get_entity(id),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Lookup entities by name across all types
    pub fn lookup_entity(&self, name: &str) -> SqliteResult<Vec<Entity>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, entity_type, discovered_at, last_updated
             FROM entities WHERE name = ?1",
        )?;

        let results: SqliteResult<Vec<Entity>> = stmt
            .query_map([name], |row| {
                let id: i64 = row.get(0)?;
                let entity_type_str: String = row.get(1)?;
                let discovered_at: String = row.get(2)?;
                let last_updated: String = row.get(3)?;

                let entity_type =
                    EntityType::from_str(&entity_type_str).unwrap_or(EntityType::File);
                let attributes = self.get_entity_attributes(id)?;

                Ok(Entity {
                    id,
                    entity_type,
                    name: name.to_string(),
                    attributes,
                    discovered_at,
                    last_updated,
                })
            })?
            .collect();

        results
    }

    /// Get all entities of a specific type
    pub fn get_entities_by_type(&self, entity_type: EntityType) -> SqliteResult<Vec<Entity>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, discovered_at, last_updated
             FROM entities WHERE entity_type = ?1",
        )?;

        let ids: Vec<(i64, String, String, String)> = stmt
            .query_map([entity_type.as_str()], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut entities = vec![];
        for (id, name, discovered_at, last_updated) in ids {
            let attributes = self.get_entity_attributes(id)?;
            entities.push(Entity {
                id,
                entity_type,
                name,
                attributes,
                discovered_at,
                last_updated,
            });
        }

        Ok(entities)
    }

    /// Add a relationship between entities
    pub fn add_relationship(
        &self,
        from_entity: i64,
        to_entity: i64,
        rel_type: &str,
        attributes: HashMap<String, String>,
    ) -> SqliteResult<i64> {
        self.conn.execute(
            "INSERT INTO relationships (from_entity, to_entity, rel_type)
             VALUES (?1, ?2, ?3)",
            params![from_entity, to_entity, rel_type],
        )?;

        let rel_id = self.conn.last_insert_rowid();

        // Add relationship attributes
        for (key, value) in attributes {
            self.conn.execute(
                "INSERT INTO relationship_attributes (relationship_id, key, value)
                 VALUES (?1, ?2, ?3)",
                params![rel_id, key, value],
            )?;
        }

        Ok(rel_id)
    }

    /// Add a relationship if it doesn't already exist
    pub fn add_relationship_unique(
        &self,
        from_entity: i64,
        to_entity: i64,
        rel_type: &str,
        attributes: HashMap<String, String>,
    ) -> SqliteResult<i64> {
        let mut stmt = self.conn.prepare(
            "SELECT id FROM relationships WHERE from_entity = ?1 AND to_entity = ?2 AND rel_type = ?3 LIMIT 1",
        )?;

        let existing_id: Option<i64> = stmt
            .query_row(params![from_entity, to_entity, rel_type], |row| row.get(0))
            .optional()?;

        if let Some(id) = existing_id {
            return Ok(id);
        }

        self.add_relationship(from_entity, to_entity, rel_type, attributes)
    }

    /// Get relationships from an entity
    pub fn get_relationships_from(
        &self,
        entity_id: i64,
        rel_type: Option<&str>,
    ) -> SqliteResult<Vec<(Relationship, Entity)>> {
        let query = if let Some(_rt) = rel_type {
            "SELECT r.id, r.to_entity, r.rel_type
             FROM relationships r
             WHERE r.from_entity = ?1 AND r.rel_type = ?2"
                .to_string()
        } else {
            "SELECT r.id, r.to_entity, r.rel_type
             FROM relationships r
             WHERE r.from_entity = ?1"
                .to_string()
        };

        let mut stmt = self.conn.prepare(&query)?;

        let rels: Vec<(i64, i64, String)> = if let Some(rt) = rel_type {
            stmt.query_map(params![entity_id, rt], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<Result<Vec<_>, _>>()?
        } else {
            stmt.query_map([entity_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<Result<Vec<_>, _>>()?
        };

        let mut results = vec![];
        for (rel_id, to_id, rel_type) in rels {
            if let Some(to_entity) = self.get_entity(to_id)? {
                let rel_attrs = self.get_relationship_attributes(rel_id)?;
                results.push((
                    Relationship {
                        id: rel_id,
                        from_entity: entity_id,
                        to_entity: to_id,
                        rel_type,
                        attributes: rel_attrs,
                    },
                    to_entity,
                ));
            }
        }

        Ok(results)
    }

    /// Get relationship attributes
    fn get_relationship_attributes(&self, rel_id: i64) -> SqliteResult<HashMap<String, String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT key, value FROM relationship_attributes WHERE relationship_id = ?1")?;

        let attrs: Result<Vec<(String, String)>, _> = stmt
            .query_map([rel_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect();

        Ok(attrs?.into_iter().collect())
    }

    /// Query: Get tools that depend on a package
    pub fn get_tools_for_package(&self, package_name: &str) -> SqliteResult<Vec<Entity>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.id FROM entities t
             JOIN relationships r ON t.id = r.from_entity
             JOIN entities p ON r.to_entity = p.id
             WHERE t.entity_type = 'tool' 
               AND p.entity_type = 'package'
               AND p.name = ?1
               AND r.rel_type = 'depends_on'",
        )?;

        let tool_ids: Vec<i64> = stmt
            .query_map([package_name], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        let mut tools = vec![];
        for id in tool_ids {
            if let Some(tool) = self.get_entity(id)? {
                tools.push(tool);
            }
        }

        Ok(tools)
    }

    /// Query: Get all services that require root
    pub fn get_privileged_services(&self) -> SqliteResult<Vec<Entity>> {
        let mut stmt = self.conn.prepare(
            "SELECT s.id FROM entities s
             JOIN relationships r ON s.id = r.from_entity
             JOIN entities p ON r.to_entity = p.id
             WHERE s.entity_type = 'service'
               AND p.entity_type = 'permission'
               AND p.name = 'root'",
        )?;

        let service_ids: Vec<i64> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        let mut services = vec![];
        for id in service_ids {
            if let Some(service) = self.get_entity(id)? {
                services.push(service);
            }
        }

        Ok(services)
    }

    /// Delete entity and all related data
    pub fn delete_entity(&self, id: i64) -> SqliteResult<()> {
        // Cascading delete will handle relationships and attributes
        self.conn
            .execute("DELETE FROM entities WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Clear all data
    pub fn clear_all(&self) -> SqliteResult<()> {
        self.conn
            .execute("DELETE FROM relationship_attributes", [])?;
        self.conn.execute("DELETE FROM entity_attributes", [])?;
        self.conn.execute("DELETE FROM relationships", [])?;
        self.conn.execute("DELETE FROM entities", [])?;
        Ok(())
    }

    /// Get statistics
    pub fn stats(&self) -> SqliteResult<(usize, usize)> {
        let entities: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM entities", [], |row| row.get(0))?;

        let relationships: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM relationships", [], |row| row.get(0))?;

        Ok((entities as usize, relationships as usize))
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
        let db_path = test_db_path("kg_init");
        let _ = std::fs::remove_file(&db_path);
        let graph = KnowledgeGraph::new(db_path.clone()).unwrap();
        let (entities, rels) = graph.stats().unwrap();
        assert_eq!(entities, 0);
        assert_eq!(rels, 0);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn test_add_and_get_entity() {
        let db_path = test_db_path("kg_entity");
        let _ = std::fs::remove_file(&db_path);
        let graph = KnowledgeGraph::new(db_path.clone()).unwrap();

        let mut attrs = HashMap::new();
        attrs.insert("version".to_string(), "1.0".to_string());
        attrs.insert("path".to_string(), "/usr/bin/python3".to_string());

        let id = graph
            .add_entity(EntityType::Tool, "python3", attrs)
            .unwrap();
        let entity = graph.get_entity(id).unwrap().unwrap();

        assert_eq!(entity.name, "python3");
        assert_eq!(entity.entity_type, EntityType::Tool);
        assert_eq!(entity.attributes.get("version").unwrap(), "1.0");

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn test_relationships() {
        let db_path = test_db_path("kg_rels");
        let _ = std::fs::remove_file(&db_path);
        let graph = KnowledgeGraph::new(db_path.clone()).unwrap();

        // Create entities
        let tool_id = graph
            .add_entity(EntityType::Tool, "nginx", HashMap::new())
            .unwrap();
        let pkg_id = graph
            .add_entity(EntityType::Package, "nginx-core", HashMap::new())
            .unwrap();

        // Create relationship
        graph
            .add_relationship(tool_id, pkg_id, "depends_on", HashMap::new())
            .unwrap();

        // Query relationships
        let rels = graph.get_relationships_from(tool_id, None).unwrap();
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0].0.rel_type, "depends_on");
        assert_eq!(rels[0].1.name, "nginx-core");

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn test_find_entity() {
        let db_path = test_db_path("kg_find");
        let _ = std::fs::remove_file(&db_path);
        let graph = KnowledgeGraph::new(db_path.clone()).unwrap();

        graph
            .add_entity(EntityType::User, "root", HashMap::new())
            .unwrap();

        let found = graph.find_entity(EntityType::User, "root").unwrap();
        assert!(found.is_some());

        let not_found = graph.find_entity(EntityType::User, "nobody").unwrap();
        assert!(not_found.is_none());

        let _ = std::fs::remove_file(&db_path);
    }
}
