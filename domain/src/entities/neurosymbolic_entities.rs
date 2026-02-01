use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Core symbolic value representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SymbolicValue {
    Concrete(u64),
    Symbolic {
        name: String,
        bits: u32,
    },
    Expression {
        op: BinaryOp,
        operands: Vec<SymbolicValue>,
    },
    Tainted {
        source: TaintSource,
        path: Vec<String>,
    },
    Boolean(bool),
    String(String),
}

/// Symbolic binary operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    And,
    Or,
    Xor,
    Not,
    Equals,
    NotEquals,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    ShiftLeft,
    ShiftRight,
    Load {
        address: Box<SymbolicValue>,
    },
    Store {
        address: Box<SymbolicValue>,
        value: Box<SymbolicValue>,
    },
}

/// Taint tracking sources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaintSource {
    UserInput { id: String },
    Network { source: String },
    File { path: String },
    Environment { variable: String },
}

/// Symbolic variable with constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolicVariable {
    pub name: String,
    pub domain: ValueDomain,
    pub constraints: Vec<Constraint>,
}

/// Value domains for symbolic reasoning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValueDomain {
    Integer { min: i64, max: i64 },
    String { min_len: usize, max_len: usize },
    Boolean,
    IPAddress,
    FilePath,
    Permission,
}

/// Logical constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Constraint {
    Equals {
        left: Box<SymbolicValue>,
        right: Box<SymbolicValue>,
    },
    GreaterThan {
        left: Box<SymbolicValue>,
        right: Box<SymbolicValue>,
    },
    LessThan {
        left: Box<SymbolicValue>,
        right: Box<SymbolicValue>,
    },
    And {
        operands: Vec<Constraint>,
    },
    Or {
        operands: Vec<Constraint>,
    },
    Not {
        operand: Box<Constraint>,
    },
    InSet {
        value: Box<SymbolicValue>,
        set: Vec<SymbolicValue>,
    },
    Range {
        value: Box<SymbolicValue>,
        min: Box<SymbolicValue>,
        max: Box<SymbolicValue>,
    },
    Regex {
        value: Box<SymbolicValue>,
        pattern: String,
    },
    FileExists {
        path: String,
        required: bool,
    },
    SystemState {
        property: String,
        expected_value: SymbolicValue,
    },
    ResourceAvailable {
        resource: ResourceType,
        amount: u64,
    },
}

/// Process state for Linux systems
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProcessState {
    Running {
        pid: u32,
        cpu: f32,
        memory: u64,
        command: String,
        parent_pid: u32,
    },
    Sleeping {
        pid: u32,
        wake_conditions: Vec<WakeCondition>,
    },
    Stopped {
        pid: u32,
        exit_code: i32,
        duration: Option<std::time::Duration>,
    },
    Zombie {
        ppid: u32,
    },
}

/// Wake conditions for sleeping processes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WakeCondition {
    Signal(String),
    Timeout { seconds: u64 },
    IOReady { fd: u32 },
    Timer { interval: std::time::Duration },
}

/// Resource constraints for Linux systems
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceConstraints {
    pub total_memory: u64,
    pub available_memory: u64,
    pub total_cpu_cores: u32,
    pub available_cpu_percent: f32,
    pub disk_space: HashMap<String, u64>,
    pub network_bandwidth: u64,
}

/// Linux-specific constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LinuxConstraint {
    ProcessExists {
        pid: u32,
    },
    PortAvailable {
        port: u16,
    },
    FileExists {
        path: String,
    },
    DirectoryExists {
        path: String,
    },
    UserExists {
        name: String,
    },
    GroupExists {
        name: String,
    },
    Permission {
        user: String,
        file: String,
        required_perm: String,
    },
    ResourceAvailable {
        resource: ResourceType,
        amount: u64,
    },
    SystemState {
        property: String,
        expected_value: SymbolicValue,
    },
    ServiceState {
        property: String,
        expected_value: String,
    },
}

/// System resources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourceType {
    Memory,
    CPU,
    DiskSpace { path: String },
    NetworkPort { port: u16 },
    Bandwidth,
}

/// Command effects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemEffect {
    ProcessChange {
        pid: u32,
        new_state: ProcessState,
    },
    FileModification {
        path: String,
        operation: FileOperation,
    },
    NetworkConnection {
        source: String,
        destination: String,
        protocol: String,
    },
    ResourceUsage {
        resource: ResourceType,
        amount: u64,
    },
}

/// Permission set for file access
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionSet {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
    pub owner: String,
    pub group: String,
}

/// File operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileOperation {
    Create {
        path: String,
    },
    Read {
        path: String,
    },
    Write {
        from: String,
        to: String,
    },
    Delete {
        path: String,
    },
    Modify {
        path: String,
    },
    ChangePermissions {
        path: String,
        from: String,
        to: String,
    },
}

/// Enhanced symbolic command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolicCommand {
    pub id: String,
    pub description: String,
    pub command_line: String,
    pub preconditions: Vec<LinuxConstraint>,
    pub effects: Vec<SystemEffect>,
    pub resource_requirements: ResourceVector,
    pub safety_rules: Vec<SafetyPolicy>,
    pub symbolic_representation: SymbolicExpression,
    pub confidence: f32,
}

/// Resource requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceVector {
    pub memory_mb: u64,
    pub cpu_percent: f32,
    pub disk_mb: u64,
    pub network_bandwidth_kbps: u64,
}

/// Safety policies for Linux commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyPolicy {
    pub id: String,
    pub rule_type: SafetyRuleType,
    pub expression: SymbolicExpression,
    pub severity: SafetySeverity,
    pub exceptions: Vec<ExceptionClause>,
}

/// Safety rule types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SafetyRuleType {
    NoPrivilegeEscalation,
    NoFileSystemWrites,
    NoNetworkAccess,
    NoSystemModification,
    RequiresConfirmation,
    RequiresSpecificUser,
    RestrictedCommand,
}

/// Safety severity levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SafetySeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Exception clauses for safety rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExceptionClause {
    pub condition: SymbolicExpression,
    pub action: ExceptionAction,
}

/// Exception actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExceptionAction {
    Allow,
    RequireConfirmation,
    LogWarning,
    Block,
}

/// Symbolic expression representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SymbolicExpression {
    AtomicValue(SymbolicValue),
    Variable(String),
    Operation {
        op: String,
        operands: Vec<SymbolicExpression>,
    },
    Quantifier {
        quantifier: QuantifierType,
        variable: String,
        expression: Box<SymbolicExpression>,
    },
}

/// Quantifier types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QuantifierType {
    ForAll,
    Exists,
}

/// Linux system state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinuxSystemState {
    pub processes: Vec<ProcessState>,
    pub open_files: HashMap<String, FileState>,
    pub network_connections: Vec<NetworkConnection>,
    pub resource_usage: ResourceUsage,
    pub user_sessions: Vec<UserSession>,
    pub service_states: HashMap<String, ServiceState>,
}

/// File state tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileState {
    pub path: String,
    pub permissions: String,
    pub size: u64,
    pub modified: std::time::SystemTime,
    pub locked_by: Option<u32>,
}

/// Network connection tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConnection {
    pub local_port: Option<u16>,
    pub remote_address: String,
    pub remote_port: u16,
    pub protocol: String,
    pub state: ConnectionState,
    pub pid: u32,
}

/// Connection states
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionState {
    Listening,
    Established,
    TimeWait,
    CloseWait,
    FinWait,
    Unknown,
}

/// Resource validation results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceValidationResult {
    pub valid: bool,
    pub memory_check: ResourceCheck,
    pub cpu_check: ResourceCheck,
    pub disk_check: ResourceCheck,
    pub network_check: ResourceCheck,
}

/// Resource check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceCheck {
    pub required: u64,
    pub available: u64,
    pub ok: bool,
}

/// Resource usage tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub memory_used: u64,
    pub memory_available: u64,
    pub cpu_usage_percent: f32,
    pub disk_usage: HashMap<String, u64>,
    pub network_traffic: NetworkTraffic,
}

/// Network traffic statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkTraffic {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub active_connections: u16,
}

/// User session tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSession {
    pub username: String,
    pub uid: u32,
    pub login_time: std::time::SystemTime,
    pub tty: Option<String>,
    pub remote_host: Option<String>,
    pub processes: Vec<u32>,
}

/// Service state tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceState {
    pub name: String,
    pub status: ServiceStatus,
    pub pid: Option<u32>,
    pub cpu_usage: f32,
    pub memory_usage: u64,
    pub uptime: Option<std::time::Duration>,
}

/// Service status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServiceStatus {
    Running,
    Stopped,
    Failed,
    Restarting,
    Unknown,
}

/// Partial solution during constraint solving
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartialSolution {
    pub variable_assignments: HashMap<String, SymbolicValue>,
    pub satisfied_constraints: Vec<Constraint>,
    pub unsatisfied_constraints: Vec<Constraint>,
    pub quality_score: f32,
}
