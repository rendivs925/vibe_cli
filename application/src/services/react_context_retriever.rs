use domain::entities::{ReactSession, ReactStepType};
use infrastructure::storage::KnowledgeGraph;
use std::sync::Arc;

pub struct RetrievedContext {
    pub session_history: String,
    pub compacted_summary: Option<String>,
    pub facts: String,
    pub hypotheses: String,
    pub constraints: String,
    pub knowledge_context: String,
}

pub struct ContextRetriever {
    knowledge_graph: Option<Arc<KnowledgeGraph>>,
}

impl ContextRetriever {
    pub fn new() -> Self {
        Self {
            knowledge_graph: None,
        }
    }

    pub fn with_knowledge_graph(mut self, kg: Arc<KnowledgeGraph>) -> Self {
        self.knowledge_graph = Some(kg);
        self
    }

    pub fn retrieve(&self, session: &ReactSession) -> RetrievedContext {
        let session_history = format_history(session);
        let compacted_summary = session.compacted_summary.clone();
        let facts = format_facts(session);
        let hypotheses = format_hypotheses(session);
        let constraints = format_constraints(session);
        let knowledge_context = self.get_knowledge_context(session);

        RetrievedContext {
            session_history,
            compacted_summary,
            facts,
            hypotheses,
            constraints,
            knowledge_context,
        }
    }

    fn get_knowledge_context(&self, session: &ReactSession) -> String {
        let Some(kg) = &self.knowledge_graph else {
            return "(knowledge graph not available)".to_string();
        };

        let query_lower = session.query.to_lowercase();
        let mut context_parts = Vec::new();

        let target = detect_target_from_query(&query_lower);
        if let Some(target_name) = target {
            if let Ok(Some(entity)) =
                kg.find_entity(infrastructure::storage::EntityType::Tool, &target_name)
            {
                context_parts.push(format!(
                    "Tool: {} - attributes: {:?}",
                    entity.name, entity.attributes
                ));
            }
            if let Ok(Some(entity)) =
                kg.find_entity(infrastructure::storage::EntityType::Service, &target_name)
            {
                context_parts.push(format!(
                    "Service: {} - attributes: {:?}",
                    entity.name, entity.attributes
                ));
            }
        }

        if let Ok(tools) = kg.get_entities_by_type(infrastructure::storage::EntityType::Tool) {
            let relevant: Vec<_> = tools
                .iter()
                .filter(|t| query_lower.contains(&t.name.to_lowercase()))
                .collect();
            if !relevant.is_empty() {
                for tool in relevant.iter().take(3) {
                    context_parts.push(format!("Tool: {} - {:?}", tool.name, tool.attributes));
                }
            }
        }

        if context_parts.is_empty() {
            "(no relevant knowledge graph data)".to_string()
        } else {
            context_parts.join("\n")
        }
    }
}

fn format_history(session: &ReactSession) -> String {
    let mut lines = Vec::new();
    if let Some(summary) = &session.compacted_summary {
        if !summary.trim().is_empty() {
            lines.push(format!("SUMMARY: {}", summary.trim()));
        }
    }

    for step in session.steps.iter().rev().take(6).rev() {
        let label = match step.step_type {
            ReactStepType::Thought => "ANALYZE",
            ReactStepType::Action => "SUGGESTED",
            ReactStepType::Observation => "OUTPUT",
            ReactStepType::Verify => "VERIFY",
            ReactStepType::Complete => "COMPLETE",
        };
        let content = step.content.trim();
        if !content.is_empty() {
            lines.push(format!("{}: {}", label, content));
        }
        if !step.observations.is_empty() {
            lines.push(format!("Observations: {}", step.observations.join(" | ")));
        }
    }
    if lines.is_empty() {
        "(none)".to_string()
    } else {
        lines.join("\n")
    }
}

fn format_facts(session: &ReactSession) -> String {
    if session.memory.facts.is_empty() {
        return "(none)".to_string();
    }
    session
        .memory
        .facts
        .iter()
        .map(|fact| {
            format!(
                "{}={} (step {}, source: {})",
                fact.key, fact.value, fact.source_step, fact.source_command
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_hypotheses(session: &ReactSession) -> String {
    if session.memory.hypotheses.is_empty() {
        return "(none)".to_string();
    }
    session
        .memory
        .hypotheses
        .iter()
        .map(|h| {
            format!(
                "{} (confidence {:.0}%)",
                h.description,
                h.confidence * 100.0
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_constraints(session: &ReactSession) -> String {
    if session.memory.constraints.is_empty() {
        return "(none)".to_string();
    }
    session
        .memory
        .constraints
        .iter()
        .map(|c| format!("{}={}", c.key, c.value))
        .collect::<Vec<_>>()
        .join(", ")
}

fn detect_target_from_query(query: &str) -> Option<String> {
    let services = [
        "nginx",
        "apache",
        "httpd",
        "mysql",
        "postgres",
        "postgresql",
        "redis",
        "docker",
        "ssh",
        "systemd",
        "postgresq",
        "mongodb",
        "elasticsearch",
        "rabbitmq",
    ];
    services
        .iter()
        .find(|svc| query.contains(*svc))
        .map(|svc| svc.to_string())
}
