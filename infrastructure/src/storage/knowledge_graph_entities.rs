//! Knowledge Graph Entity Types
//!
//! Types representing system entities stored in the knowledge graph.
//! These types track OS, tools, services, and other system state.

use std::collections::HashMap;

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
