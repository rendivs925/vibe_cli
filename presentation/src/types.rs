use serde::{Deserialize, Serialize};

/// Classification of user query intent
#[derive(Debug, Clone, PartialEq)]
pub enum CommandIntent {
    InfoQuery,      // "what's my GPU", "how much RAM"
    Installation,   // "install python", "setup nginx"
    Configuration,  // "configure nginx", "enable firewall"
    ServiceControl, // "start apache", "restart mysql"
    SystemQuery,    // "show disk usage", "list processes"
    AgentTask,      // Multi-step tasks using --agent
    Unknown,        // Unclassified queries
}

/// Risk assessment for commands in agent execution
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub enum AgentCommandRisk {
    InfoOnly,           // Read-only queries (ls, pwd, cat)
    SafeOperations,     // Safe operations (mkdir, echo, cp)
    NetworkAccess,      // Network-dependent (npm install, git clone)
    SystemChanges,      // System modifications (chmod, chown, systemctl)
    Destructive,        // Destructive operations (rm -rf, dd, format)
    Unknown,            // Cannot assess risk
}

/// Individual step in an agent execution plan
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AgentStep {
    pub id: String,
    pub command: String,
    pub description: String,
    pub risk_level: AgentCommandRisk,
    pub estimated_duration: Option<String>,
    pub dependencies: Vec<String>,
    pub rollback_command: Option<String>,
}

/// Complete agent execution plan
#[derive(Debug, Clone)]
#[derive(serde::Deserialize)]
pub struct AgentPlan {
    pub steps: Vec<AgentStep>,
    pub total_estimated_time: Option<String>,
    pub total_disk_impact: Option<String>,
    pub network_required: bool,
    pub safety_concerns: Vec<String>,
}

/// Risk assessment for commands
#[derive(Debug, Clone, PartialEq)]
pub enum CommandRisk {
    InfoOnly,      // Read-only queries, no system changes
    SafeSetup,     // Package installs, basic service setup
    SystemChanges, // Configuration changes, user creation
    HighRisk,      // Destructive operations, system-wide changes
    Unknown,       // Cannot assess risk
}

/// Installation option for user selection
#[derive(Debug, Clone)]
pub struct InstallationOption {
    pub name: String,
    pub description: String,
    pub pros: Vec<String>,
    pub cons: Vec<String>,
    pub risk_level: CommandRisk,
    pub commands: Vec<String>,
    pub estimated_time: Option<String>,
    pub disk_space: Option<String>,
}

/// Statistics about context gathering for display
#[derive(Default, Clone)]
pub struct ContextStats {
    pub files_scanned: usize,
    pub files_analyzed: usize,
    pub keywords_count: usize,
    pub os_info: String,
    pub cwd: String,
    pub total_files: usize,
    pub relevant_files: usize,
}

#[derive(Serialize, Deserialize, Default)]
pub struct CacheFile {
    pub entries: Vec<CacheEntry>,
}

#[derive(Serialize, Deserialize)]
pub struct CacheEntry {
    pub prompt: String,
    pub command: String,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize, Default)]
pub struct ExplainCacheFile {
    pub entries: Vec<ExplainCacheEntry>,
}

#[derive(Serialize, Deserialize)]
pub struct ExplainCacheEntry {
    pub prompt: String,
    pub response: String,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize, Default)]
pub struct RagCacheFile {
    pub entries: Vec<RagCacheEntry>,
}

#[derive(Serialize, Deserialize)]
pub struct RagCacheEntry {
    pub question: String,
    pub response: String,
    pub timestamp: u64,
}