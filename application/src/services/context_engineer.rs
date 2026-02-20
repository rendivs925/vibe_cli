use crate::services::context_vault::ContextVault;
use crate::services::context_window::ContextWindowUsage;
use crate::services::operational_guardrails::OperationalGuardrails;
use crate::services::task_orchestration::TaskOrchestration;
use domain::entities::context_document::ContextDocumentType;
use domain::entities::session_summary::SessionSummary;
use domain::entities::react_memory::{Constraint, Fact, Hypothesis};

pub struct ContextEngineer {
    session_summary: SessionSummary,
    context_vault: ContextVault,
    guardrails: OperationalGuardrails,
}

impl ContextEngineer {
    pub fn new(task: &str) -> Self {
        Self {
            session_summary: SessionSummary::new(task),
            context_vault: ContextVault::new(),
            guardrails: OperationalGuardrails::default(),
        }
    }

    pub fn with_session_id(mut self, session_id: &str) -> Self {
        self.session_summary = self.session_summary.with_session_id(session_id);
        self
    }

    pub fn with_task_type(mut self, task_type: &str) -> Self {
        self.session_summary = self.session_summary.with_task_type(task_type);
        self
    }

    pub fn with_iteration(mut self, iteration: u32, max_iterations: u32) -> Self {
        self.session_summary = self
            .session_summary
            .with_iteration(iteration, max_iterations);
        self
    }

    pub fn with_guardrails(mut self, guardrails: OperationalGuardrails) -> Self {
        self.guardrails = guardrails;
        self
    }

    pub fn add_session_history(&mut self, content: &str) -> String {
        self.context_vault.add(
            ContextDocumentType::SessionHistory,
            "session_history",
            normalize_empty(content, "(none)"),
        )
    }

    pub fn add_latest_output(&mut self, content: &str, source_command: Option<&str>) -> String {
        let id = if let Some(source) = source_command {
            self.context_vault.add_with_source(
                ContextDocumentType::LatestOutput,
                "latest_output",
                normalize_empty(content, "(no output yet)"),
                source,
            )
        } else {
            self.context_vault.add(
                ContextDocumentType::LatestOutput,
                "latest_output",
                normalize_empty(content, "(no output yet)"),
            )
        };
        self.guardrails = self.guardrails.clone().with_latest_output_ref(&id);
        id
    }

    pub fn add_facts(&mut self, facts: &[Fact]) -> String {
        let content = if facts.is_empty() {
            "(none)".to_string()
        } else {
            facts
                .iter()
                .map(|f| format!("- {}: {} [source: {}]", f.key, f.value, f.source_command))
                .collect::<Vec<_>>()
                .join("\n")
        };
        self.context_vault
            .add(ContextDocumentType::ExtractedFacts, "extracted_facts", content)
    }

    pub fn add_hypotheses(&mut self, hypotheses: &[Hypothesis]) -> String {
        let content = if hypotheses.is_empty() {
            "(none)".to_string()
        } else {
            hypotheses
                .iter()
                .map(|h| format!("- \"{}\" (confidence: {:.2})", h.description, h.confidence))
                .collect::<Vec<_>>()
                .join("\n")
        };
        self.context_vault
            .add(ContextDocumentType::Hypotheses, "hypotheses", content)
    }

    pub fn add_constraints(&mut self, constraints: &[Constraint], extra: Option<&str>) -> String {
        let mut parts = Vec::new();
        if !constraints.is_empty() {
            let list = constraints
                .iter()
                .map(|c| format!("- {}={}", c.key, c.value))
                .collect::<Vec<_>>()
                .join("\n");
            parts.push(list);
        }
        if let Some(extra) = extra {
            if !extra.trim().is_empty() {
                parts.push(format!("- {}", extra.trim()));
            }
        }
        let content = if parts.is_empty() {
            "(none)".to_string()
        } else {
            parts.join("\n")
        };
        self.context_vault
            .add(ContextDocumentType::Constraints, "constraints", content)
    }

    pub fn add_learning_context(&mut self, content: &str) -> String {
        self.context_vault.add(
            ContextDocumentType::LearningContext,
            "learning_context",
            normalize_empty(content, "(none)"),
        )
    }

    pub fn add_code_context(&mut self, content: &str, source: Option<&str>) -> String {
        let cleaned = normalize_empty(content, "(none)");
        if let Some(source) = source {
            self.context_vault.add_with_source(
                ContextDocumentType::CodeContext,
                "code_context",
                cleaned,
                source,
            )
        } else {
            self.context_vault
                .add(ContextDocumentType::CodeContext, "code_context", cleaned)
        }
    }

    pub fn add_knowledge_base(&mut self, content: &str) -> String {
        self.context_vault.add(
            ContextDocumentType::KnowledgeBase,
            "knowledge_base",
            normalize_empty(content, "(none)"),
        )
    }

    pub fn add_context_window(&mut self, usage: ContextWindowUsage) -> String {
        let utilization = usage.utilization() * 100.0;
        let status = if usage.should_compact() {
            "limit_reached"
        } else if utilization >= 85.0 {
            "near_limit"
        } else {
            "ok"
        };
        let content = format!(
            "- Estimated Tokens: {}\n- Window Limit: {}\n- Compact At: {}\n- Utilization: {:.1}%\n- Status: {}",
            usage.estimated_tokens, usage.max_tokens, usage.compact_at_tokens, utilization, status
        );
        self.context_vault
            .add(ContextDocumentType::Metadata, "context_window", content)
    }

    pub fn render(&self, task: &str, task_type: &str, depth_instruction: Option<&str>) -> String {
        let orchestration = TaskOrchestration::new(
            task,
            self.session_summary.iteration,
            self.session_summary.max_iterations,
        )
        .with_type(task_type);
        let mut output = String::new();
        output.push_str("# [[ GLOBAL_INTERFACE ]]\n\n");
        output.push_str(&self.session_summary.to_markdown());
        output.push_str("### ## CONTEXT_VAULT\n");
        output.push_str(&self.context_vault.render());
        output.push_str("\n---\n\n");
        output.push_str(&self.guardrails.to_markdown());
        if let Some(depth) = depth_instruction {
            if !depth.trim().is_empty() {
                output.push_str("### ## REASONING_DEPTH\n");
                output.push_str(depth.trim());
                output.push_str("\n\n");
            }
        }
        output.push_str(&orchestration.to_markdown());
        output
    }
}

fn normalize_empty(content: &str, fallback: &str) -> String {
    if content.trim().is_empty() {
        fallback.to_string()
    } else {
        content.to_string()
    }
}
