use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationalGuardrails {
    pub groundedness: bool,
    pub traceability: bool,
    pub recency_priority: bool,
    pub delta_only: bool,
    pub loop_prevention: bool,
    pub latest_output_ref: Option<String>,
}

impl Default for OperationalGuardrails {
    fn default() -> Self {
        Self {
            groundedness: true,
            traceability: true,
            recency_priority: true,
            delta_only: false,
            loop_prevention: true,
            latest_output_ref: None,
        }
    }
}

impl OperationalGuardrails {
    pub fn with_delta_only(mut self, enabled: bool) -> Self {
        self.delta_only = enabled;
        self
    }

    pub fn with_latest_output_ref(mut self, ref_id: &str) -> Self {
        if !ref_id.trim().is_empty() {
            self.latest_output_ref = Some(ref_id.to_string());
        }
        self
    }

    pub fn to_markdown(&self) -> String {
        let mut rules = Vec::new();
        if self.groundedness {
            rules.push(
                "**Groundedness**: Base all answers strictly on the CONTEXT_VAULT. Do NOT hallucinate.".to_string(),
            );
        }
        if self.traceability {
            rules.push("**Traceability**: Cite sources using [REF-XX] notation for every claim.".to_string());
        }
        if self.recency_priority {
            let ref_id = self
                .latest_output_ref
                .as_deref()
                .unwrap_or("REF-02");
            rules.push(format!(
                "**Recency Priority**: {} (latest_output) overrides all prior references.",
                ref_id
            ));
        }
        if self.delta_only {
            rules.push("**Delta-Only**: For code changes, provide only the diff, not full rewrites.".to_string());
        }
        if self.loop_prevention {
            rules.push("**Loop Prevention**: Do NOT repeat commands from history without new justification.".to_string());
        }

        format!(
            "### ## OPERATIONAL_GUARDRAILS\n{}\n\n---\n\n",
            rules
                .iter()
                .enumerate()
                .map(|(i, r)| format!("{}. {}", i + 1, r))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}
