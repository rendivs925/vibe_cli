use crate::entities::neurosymbolic_entities::{
    Constraint as LegacyConstraint, PartialSolution, SymbolicExpression as LegacyExpression,
    SymbolicValue as LegacyValue,
};
use crate::repositories::symbolic_reasoning_repository::*;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// Converts between legacy neurosymbolic entities and the new flexible storage format
pub struct SymbolicFormatConverter;

impl SymbolicFormatConverter {
    /// Convert legacy SymbolicValue to new flexible format
    pub fn convert_value(value: &LegacyValue) -> SymbolicValue {
        let (value_type, data) = match value {
            LegacyValue::Concrete(v) => (ValueType::Concrete, json!(v)),
            LegacyValue::Symbolic { name, bits } => (
                ValueType::Symbolic,
                json!({
                    "name": name,
                    "bits": bits
                }),
            ),
            LegacyValue::Expression { op, operands } => {
                let ops: Vec<Value> = operands
                    .iter()
                    .map(|o| json!(Self::convert_value(o)))
                    .collect();
                (
                    ValueType::Expression,
                    json!({
                        "op": format!("{:?}", op),
                        "operands": ops
                    }),
                )
            }
            LegacyValue::Tainted { source, path } => (
                ValueType::Tainted,
                json!({
                    "source": format!("{:?}", source),
                    "path": path
                }),
            ),
            LegacyValue::Boolean(b) => (ValueType::Boolean, json!(b)),
            LegacyValue::String(s) => (ValueType::String, json!(s)),
        };

        SymbolicValue {
            value_type,
            data,
            annotations: vec![],
        }
    }

    /// Convert legacy Constraint to new flexible format
    pub fn convert_constraint(constraint: &LegacyConstraint) -> ConstraintData {
        let (constraint_type, expression_refs, parameters) = match constraint {
            LegacyConstraint::Equals { left, right } => (
                "Equals".to_string(),
                vec![],
                json!({
                    "left": Self::convert_value(left),
                    "right": Self::convert_value(right)
                }),
            ),
            LegacyConstraint::GreaterThan { left, right } => (
                "GreaterThan".to_string(),
                vec![],
                json!({
                    "left": Self::convert_value(left),
                    "right": Self::convert_value(right)
                }),
            ),
            LegacyConstraint::LessThan { left, right } => (
                "LessThan".to_string(),
                vec![],
                json!({
                    "left": Self::convert_value(left),
                    "right": Self::convert_value(right)
                }),
            ),
            LegacyConstraint::And { operands } => (
                "And".to_string(),
                vec![],
                json!({ "operand_count": operands.len() }),
            ),
            LegacyConstraint::Or { operands } => (
                "Or".to_string(),
                vec![],
                json!({ "operand_count": operands.len() }),
            ),
            LegacyConstraint::Not { .. } => ("Not".to_string(), vec![], json!({})),
            LegacyConstraint::InSet { value, set } => (
                "InSet".to_string(),
                vec![],
                json!({
                    "value": Self::convert_value(value),
                    "set_size": set.len()
                }),
            ),
            LegacyConstraint::Range { value, min, max } => (
                "Range".to_string(),
                vec![],
                json!({
                    "value": Self::convert_value(value),
                    "min": Self::convert_value(min),
                    "max": Self::convert_value(max)
                }),
            ),
            LegacyConstraint::Regex { value, pattern } => (
                "Regex".to_string(),
                vec![],
                json!({
                    "value": Self::convert_value(value),
                    "pattern": pattern
                }),
            ),
            LegacyConstraint::FileExists { path, required } => (
                "FileExists".to_string(),
                vec![],
                json!({
                    "path": path,
                    "required": required
                }),
            ),
            LegacyConstraint::SystemState {
                property,
                expected_value,
            } => (
                "SystemState".to_string(),
                vec![],
                json!({
                    "property": property,
                    "expected_value": Self::convert_value(expected_value)
                }),
            ),
            LegacyConstraint::ResourceAvailable { resource, amount } => (
                "ResourceAvailable".to_string(),
                vec![],
                json!({
                    "resource": format!("{:?}", resource),
                    "amount": amount
                }),
            ),
        };

        ConstraintData {
            constraint_id: format!(
                "legacy_{}",
                &uuid::Uuid::new_v4().to_string()[..8]
            ),
            constraint_type,
            expression_refs,
            parameters,
            priority: 0,
        }
    }

    /// Convert legacy SymbolicExpression to new format
    pub fn convert_expression(expr: &LegacyExpression) -> SymbolicExpressionData {
        let (expression_type, content) = match expr {
            LegacyExpression::AtomicValue(val) => (
                ExpressionType::Atomic,
                json!({ "value": Self::convert_value(val) }),
            ),
            LegacyExpression::Variable(name) => (ExpressionType::Variable, json!({ "name": name })),
            LegacyExpression::Operation { op, operands } => {
                let ops: Vec<Value> = operands
                    .iter()
                    .map(|o| json!(Self::convert_expression(o).content))
                    .collect();
                (
                    ExpressionType::Nary,
                    json!({
                        "operator": op,
                        "operands": ops
                    }),
                )
            }
            LegacyExpression::Quantifier {
                quantifier,
                variable,
                expression,
            } => (
                ExpressionType::Quantified,
                json!({
                    "quantifier": format!("{:?}", quantifier),
                    "variable": variable,
                    "expression": Self::convert_expression(expression).content
                }),
            ),
        };

        let content_str = content.to_string();
        let hash_bytes = Sha256::digest(content_str.as_bytes());
        let hash = format!("{:x}", hash_bytes)[..16].to_string();

        SymbolicExpressionData {
            id: format!("expr_{}", &uuid::Uuid::new_v4().to_string()[..8]),
            version: TraceVersion::current(),
            expression_type,
            content,
            hash,
            references: vec![],
        }
    }

    /// Convert domain type to new format
    pub fn convert_domain(domain_type: &str) -> SymbolicDomain {
        match domain_type.to_lowercase().as_str() {
            "linux" | "system" => SymbolicDomain::LinuxSystem,
            "network" => SymbolicDomain::Network,
            "filesystem" | "file" => SymbolicDomain::FileSystem,
            "process" => SymbolicDomain::Process,
            "container" | "docker" | "kubernetes" => SymbolicDomain::Container,
            "security" => SymbolicDomain::Security,
            _ => SymbolicDomain::Custom(domain_type.to_string()),
        }
    }
}

/// Extension trait for converting PartialSolution to new format
pub trait PartialSolutionExt {
    fn to_trace(
        &self,
        domain: SymbolicDomain,
        metadata: std::collections::HashMap<String, String>,
    ) -> SymbolicReasoningTrace;
}

impl PartialSolutionExt for PartialSolution {
    fn to_trace(
        &self,
        domain: SymbolicDomain,
        metadata: std::collections::HashMap<String, String>,
    ) -> SymbolicReasoningTrace {
        let mut steps = Vec::new();

        // Convert variable assignments to steps
        for (var_name, value) in &self.variable_assignments {
            steps.push(SymbolicStep {
                step_id: format!("step_{}", &uuid::Uuid::new_v4().to_string()[..8]),
                step_type: StepType::VariableDeclaration,
                timestamp: chrono::Utc::now(),
                duration_ms: 0,
                expression_ref: None,
                constraint_refs: vec![],
                inputs: vec![],
                outputs: vec![SymbolicFormatConverter::convert_value(value)],
                metadata: {
                    let mut m = std::collections::HashMap::new();
                    m.insert("variable_name".to_string(), var_name.clone());
                    m
                },
            });
        }

        // Convert satisfied constraints to steps
        for constraint in &self.satisfied_constraints {
            steps.push(SymbolicStep {
                step_id: format!("step_{}", &uuid::Uuid::new_v4().to_string()[..8]),
                step_type: StepType::ConstraintCheck,
                timestamp: chrono::Utc::now(),
                duration_ms: 0,
                expression_ref: None,
                constraint_refs: vec![],
                inputs: vec![],
                outputs: vec![],
                metadata: {
                    let mut m = std::collections::HashMap::new();
                    m.insert("constraint_type".to_string(), format!("{:?}", constraint));
                    m.insert("satisfied".to_string(), "true".to_string());
                    m
                },
            });
        }

        // Create conclusion
        let conclusions = vec![SymbolicConclusion {
            conclusion_id: format!(
                "concl_{}",
                &uuid::Uuid::new_v4().to_string()[..8]
            ),
            conclusion_type: ConclusionType::Satisfiable,
            confidence: self.quality_score as f64,
            supporting_steps: steps.iter().map(|s| s.step_id.clone()).collect(),
            result: SymbolicValue {
                value_type: ValueType::Boolean,
                data: json!(true),
                annotations: vec![],
            },
            explanation: format!("Solution with quality score {}", self.quality_score),
        }];

        SymbolicReasoningTrace {
            id: format!(
                "trace_{}",
                &uuid::Uuid::new_v4().to_string()[..8]
            ),
            version: TraceVersion::current(),
            timestamp: chrono::Utc::now(),
            domain,
            metadata,
            steps,
            conclusions,
            annotations: vec![],
        }
    }
}

/// Factory for creating constraint sets from legacy constraints
pub struct ConstraintSetFactory;

impl ConstraintSetFactory {
    pub fn from_legacy_constraints(constraints: &[LegacyConstraint]) -> ConstraintSet {
        let converted_constraints: Vec<_> = constraints
            .iter()
            .map(SymbolicFormatConverter::convert_constraint)
            .collect();

        ConstraintSet {
            id: format!("cs_{}", &uuid::Uuid::new_v4().to_string()[..8]),
            version: TraceVersion::current(),
            constraints: converted_constraints,
            variables: vec![],
            metadata: std::collections::HashMap::new(),
        }
    }
}
