use domain::repositories::symbolic_reasoning_repository::*;
use infrastructure::symbolic_storage::InMemorySymbolicStorage;
use std::collections::HashMap;

#[tokio::test]
async fn test_symbolic_storage_save_and_retrieve() {
    let storage = InMemorySymbolicStorage::new();

    let trace = create_test_trace(SymbolicDomain::LinuxSystem);
    let id = storage.save_trace(&trace).await.unwrap();

    let retrieved = storage.find_trace_by_id(&id).await.unwrap();
    assert!(retrieved.is_some(), "Should retrieve saved trace");

    let retrieved_trace = retrieved.unwrap();
    assert_eq!(retrieved_trace.domain, SymbolicDomain::LinuxSystem);
    assert_eq!(retrieved_trace.steps.len(), 2);
}

#[tokio::test]
async fn test_symbolic_storage_query_by_domain() {
    let storage = InMemorySymbolicStorage::new();

    let linux_trace = create_test_trace(SymbolicDomain::LinuxSystem);
    let network_trace = create_test_trace(SymbolicDomain::Network);

    storage.save_trace(&linux_trace).await.unwrap();
    storage.save_trace(&network_trace).await.unwrap();

    let query = SymbolicQuery::new().with_domain(SymbolicDomain::LinuxSystem);
    let results = storage.query_traces(&query).await.unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].domain, SymbolicDomain::LinuxSystem);
}

#[tokio::test]
async fn test_symbolic_storage_stats() {
    let storage = InMemorySymbolicStorage::new();

    let trace = create_test_trace(SymbolicDomain::FileSystem);
    storage.save_trace(&trace).await.unwrap();

    let stats = storage.get_stats().await.unwrap();
    assert_eq!(stats.total_traces, 1);
}

#[tokio::test]
async fn test_symbolic_storage_delete() {
    let storage = InMemorySymbolicStorage::new();

    let trace = create_test_trace(SymbolicDomain::Security);
    let id = storage.save_trace(&trace).await.unwrap();

    storage.delete_trace(&id).await.unwrap();

    let retrieved = storage.find_trace_by_id(&id).await.unwrap();
    assert!(retrieved.is_none(), "Deleted trace should not be found");
}

#[tokio::test]
async fn test_constraint_set_storage() {
    let storage = InMemorySymbolicStorage::new();

    let constraint_set = ConstraintSet {
        id: String::new(),
        version: TraceVersion::current(),
        constraints: vec![],
        variables: vec![],
        metadata: HashMap::new(),
    };

    let id = storage.save_constraints(&constraint_set).await.unwrap();
    let retrieved = storage.find_constraints_by_id(&id).await.unwrap();

    assert!(retrieved.is_some());
}

#[tokio::test]
async fn test_symbolic_expression_storage() {
    let storage = InMemorySymbolicStorage::new();

    let expression = SymbolicExpressionData {
        id: String::new(),
        version: TraceVersion::current(),
        expression_type: ExpressionType::Atomic,
        content: serde_json::json!({"value": "test"}),
        hash: "test_hash".to_string(),
        references: vec![],
    };

    let id = storage.save_expression(&expression).await.unwrap();
    let retrieved = storage.find_expression_by_id(&id).await.unwrap();

    assert!(retrieved.is_some());
}

fn create_test_trace(domain: SymbolicDomain) -> SymbolicReasoningTrace {
    let mut metadata = HashMap::new();
    metadata.insert("test".to_string(), "value".to_string());

    SymbolicReasoningTrace {
        id: String::new(),
        version: TraceVersion::current(),
        timestamp: chrono::Utc::now(),
        domain,
        metadata,
        steps: vec![
            SymbolicStep {
                step_id: "step_1".to_string(),
                step_type: StepType::VariableDeclaration,
                timestamp: chrono::Utc::now(),
                duration_ms: 100,
                expression_ref: None,
                constraint_refs: vec![],
                inputs: vec![],
                outputs: vec![SymbolicValue {
                    value_type: ValueType::Integer,
                    data: serde_json::json!(42),
                    annotations: vec![],
                }],
                metadata: HashMap::new(),
            },
            SymbolicStep {
                step_id: "step_2".to_string(),
                step_type: StepType::ConstraintCheck,
                timestamp: chrono::Utc::now(),
                duration_ms: 50,
                expression_ref: None,
                constraint_refs: vec![],
                inputs: vec![],
                outputs: vec![],
                metadata: HashMap::new(),
            },
        ],
        conclusions: vec![SymbolicConclusion {
            conclusion_id: "concl_1".to_string(),
            conclusion_type: ConclusionType::Satisfiable,
            confidence: 0.95,
            supporting_steps: vec!["step_1".to_string(), "step_2".to_string()],
            result: SymbolicValue {
                value_type: ValueType::Boolean,
                data: serde_json::json!(true),
                annotations: vec![],
            },
            explanation: "Test conclusion".to_string(),
        }],
        annotations: vec![],
    }
}
