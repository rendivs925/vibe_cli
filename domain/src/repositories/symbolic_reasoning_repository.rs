use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Repository interface for symbolic reasoning storage and retrieval
#[async_trait]
pub trait SymbolicReasoningRepository: Send + Sync {
    /// Store a symbolic reasoning trace
    async fn save_trace(
        &self,
        trace: &SymbolicReasoningTrace,
    ) -> Result<String, SymbolicStorageError>;

    /// Retrieve a symbolic reasoning trace by ID
    async fn find_trace_by_id(
        &self,
        id: &str,
    ) -> Result<Option<SymbolicReasoningTrace>, SymbolicStorageError>;

    /// Store a symbolic expression
    async fn save_expression(
        &self,
        expression: &SymbolicExpressionData,
    ) -> Result<String, SymbolicStorageError>;

    /// Retrieve a symbolic expression by ID
    async fn find_expression_by_id(
        &self,
        id: &str,
    ) -> Result<Option<SymbolicExpressionData>, SymbolicStorageError>;

    /// Store a constraint set
    async fn save_constraints(
        &self,
        constraints: &ConstraintSet,
    ) -> Result<String, SymbolicStorageError>;

    /// Retrieve constraints by ID
    async fn find_constraints_by_id(
        &self,
        id: &str,
    ) -> Result<Option<ConstraintSet>, SymbolicStorageError>;

    /// Query traces by domain and time range
    async fn query_traces(
        &self,
        query: &SymbolicQuery,
    ) -> Result<Vec<SymbolicReasoningTrace>, SymbolicStorageError>;

    /// Delete a trace by ID
    async fn delete_trace(&self, id: &str) -> Result<(), SymbolicStorageError>;

    /// Get storage statistics
    async fn get_stats(&self) -> Result<SymbolicStorageStats, SymbolicStorageError>;

    /// List all trace IDs with pagination
    async fn list_trace_ids(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<String>, SymbolicStorageError>;
}

/// Flexible symbolic reasoning trace format
/// Designed to be versioned and extensible
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolicReasoningTrace {
    pub id: String,
    pub version: TraceVersion,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub domain: SymbolicDomain,
    pub metadata: HashMap<String, String>,
    pub steps: Vec<SymbolicStep>,
    pub conclusions: Vec<SymbolicConclusion>,
    pub annotations: Vec<TraceAnnotation>,
}

/// Version information for trace format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub format: StorageFormat,
}

impl TraceVersion {
    pub fn new(major: u32, minor: u32, patch: u32, format: StorageFormat) -> Self {
        Self {
            major,
            minor,
            patch,
            format,
        }
    }

    pub fn current() -> Self {
        Self::new(1, 0, 0, StorageFormat::Json)
    }
}

/// Supported storage formats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageFormat {
    Json,
    MessagePack,
    Cbor,
    Custom(String),
}

/// Symbolic domain categorization
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SymbolicDomain {
    LinuxSystem,
    Network,
    FileSystem,
    Process,
    Container,
    Security,
    Custom(String),
}

/// Individual step in symbolic reasoning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolicStep {
    pub step_id: String,
    pub step_type: StepType,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub duration_ms: u64,
    pub expression_ref: Option<String>,
    pub constraint_refs: Vec<String>,
    pub inputs: Vec<SymbolicValue>,
    pub outputs: Vec<SymbolicValue>,
    pub metadata: HashMap<String, String>,
}

/// Type of symbolic step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepType {
    VariableDeclaration,
    ExpressionEvaluation,
    ConstraintCheck,
    Inference,
    PatternMatch,
    Unification,
    Substitution,
    Simplification,
    Custom(String),
}

/// Symbolic value representation (flexible format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolicValue {
    pub value_type: ValueType,
    pub data: serde_json::Value,
    pub annotations: Vec<ValueAnnotation>,
}

/// Value type classification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValueType {
    Concrete,
    Symbolic,
    Expression,
    Tainted,
    Boolean,
    String,
    Integer,
    Float,
    Reference,
    Unknown,
}

/// Value annotations for metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueAnnotation {
    pub key: String,
    pub value: serde_json::Value,
}

/// Symbolic expression data (stored separately for reuse)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolicExpressionData {
    pub id: String,
    pub version: TraceVersion,
    pub expression_type: ExpressionType,
    pub content: serde_json::Value,
    pub hash: String,
    pub references: Vec<String>,
}

/// Expression type classification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExpressionType {
    Atomic,
    Binary,
    Unary,
    Nary,
    Variable,
    Quantified,
    Lambda,
    Custom(String),
}

/// Constraint set for storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintSet {
    pub id: String,
    pub version: TraceVersion,
    pub constraints: Vec<ConstraintData>,
    pub variables: Vec<VariableDeclaration>,
    pub metadata: HashMap<String, String>,
}

/// Individual constraint data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintData {
    pub constraint_id: String,
    pub constraint_type: String,
    pub expression_refs: Vec<String>,
    pub parameters: serde_json::Value,
    pub priority: i32,
}

/// Variable declaration in constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableDeclaration {
    pub name: String,
    pub var_type: String,
    pub domain: Option<serde_json::Value>,
    pub initial_value: Option<SymbolicValue>,
}

/// Reasoning conclusion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolicConclusion {
    pub conclusion_id: String,
    pub conclusion_type: ConclusionType,
    pub confidence: f64,
    pub supporting_steps: Vec<String>,
    pub result: SymbolicValue,
    pub explanation: String,
}

/// Conclusion type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConclusionType {
    Satisfiable,
    Unsatisfiable,
    Valid,
    Invalid,
    Unknown,
    Partial,
}

/// Trace annotation for metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceAnnotation {
    pub key: String,
    pub value: serde_json::Value,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Query parameters for symbolic traces
#[derive(Debug, Clone, Default)]
pub struct SymbolicQuery {
    pub domain: Option<SymbolicDomain>,
    pub from_timestamp: Option<chrono::DateTime<chrono::Utc>>,
    pub to_timestamp: Option<chrono::DateTime<chrono::Utc>>,
    pub metadata_filter: HashMap<String, String>,
    pub step_types: Vec<StepType>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

impl SymbolicQuery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_domain(mut self, domain: SymbolicDomain) -> Self {
        self.domain = Some(domain);
        self
    }

    pub fn with_time_range(
        mut self,
        from: chrono::DateTime<chrono::Utc>,
        to: chrono::DateTime<chrono::Utc>,
    ) -> Self {
        self.from_timestamp = Some(from);
        self.to_timestamp = Some(to);
        self
    }

    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata_filter
            .insert(key.to_string(), value.to_string());
        self
    }

    pub fn with_step_type(mut self, step_type: StepType) -> Self {
        self.step_types.push(step_type);
        self
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn with_offset(mut self, offset: usize) -> Self {
        self.offset = Some(offset);
        self
    }
}

/// Storage statistics
#[derive(Debug, Clone)]
pub struct SymbolicStorageStats {
    pub total_traces: usize,
    pub total_expressions: usize,
    pub total_constraints: usize,
    pub storage_size_bytes: u64,
    pub oldest_trace: Option<chrono::DateTime<chrono::Utc>>,
    pub newest_trace: Option<chrono::DateTime<chrono::Utc>>,
    pub average_trace_size: u64,
}

impl SymbolicStorageStats {
    pub fn new(
        total_traces: usize,
        total_expressions: usize,
        total_constraints: usize,
        storage_size_bytes: u64,
        oldest_trace: Option<chrono::DateTime<chrono::Utc>>,
        newest_trace: Option<chrono::DateTime<chrono::Utc>>,
        average_trace_size: u64,
    ) -> Self {
        Self {
            total_traces,
            total_expressions,
            total_constraints,
            storage_size_bytes,
            oldest_trace,
            newest_trace,
            average_trace_size,
        }
    }
}

/// Storage error types
#[derive(Debug, Clone)]
pub enum SymbolicStorageError {
    ConnectionError(String),
    NotFound(String),
    SerializationError(String),
    DeserializationError(String),
    ValidationError(String),
    StorageError(String),
    VersionMismatch(String),
    FormatError(String),
}

impl std::fmt::Display for SymbolicStorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SymbolicStorageError::ConnectionError(msg) => {
                write!(f, "Connection error: {}", msg)
            }
            SymbolicStorageError::NotFound(msg) => write!(f, "Not found: {}", msg),
            SymbolicStorageError::SerializationError(msg) => {
                write!(f, "Serialization error: {}", msg)
            }
            SymbolicStorageError::DeserializationError(msg) => {
                write!(f, "Deserialization error: {}", msg)
            }
            SymbolicStorageError::ValidationError(msg) => {
                write!(f, "Validation error: {}", msg)
            }
            SymbolicStorageError::StorageError(msg) => write!(f, "Storage error: {}", msg),
            SymbolicStorageError::VersionMismatch(msg) => {
                write!(f, "Version mismatch: {}", msg)
            }
            SymbolicStorageError::FormatError(msg) => write!(f, "Format error: {}", msg),
        }
    }
}

impl std::error::Error for SymbolicStorageError {}
