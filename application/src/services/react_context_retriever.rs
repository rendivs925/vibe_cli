use domain::entities::{ReactSession, ReactStepType};

pub struct RetrievedContext {
    pub session_history: String,
    pub compacted_summary: Option<String>,
    pub facts: String,
    pub hypotheses: String,
    pub constraints: String,
}

pub struct ContextRetriever;

impl ContextRetriever {
    pub fn new() -> Self {
        Self
    }

    pub fn retrieve(&self, session: &ReactSession) -> RetrievedContext {
        let session_history = format_history(session);
        let compacted_summary = session.compacted_summary.clone();
        let facts = format_facts(session);
        let hypotheses = format_hypotheses(session);
        let constraints = format_constraints(session);

        RetrievedContext {
            session_history,
            compacted_summary,
            facts,
            hypotheses,
            constraints,
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
        .map(|h| format!("{} (confidence {:.0}%)", h.description, h.confidence * 100.0))
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
