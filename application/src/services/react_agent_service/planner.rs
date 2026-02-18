use super::classifier::TaskClass;

#[derive(Debug, Clone)]
pub struct PlanStep {
    pub order: u32,
    pub description: String,
    pub suggested_tool: Option<String>,
}

pub struct DynamicPlanner;

impl DynamicPlanner {
    pub fn new() -> Self {
        Self
    }

    pub fn plan(&self, task_class: TaskClass, _query: &str) -> Vec<PlanStep> {
        match task_class {
            TaskClass::Coding => vec![
                PlanStep::new(1, "Discover relevant files using RAG or grep", Some("rag")),
                PlanStep::new(2, "Inspect key files and identify changes", Some("read")),
                PlanStep::new(3, "Run tests or checks after changes", Some("code_test")),
            ],
            TaskClass::Research => vec![
                PlanStep::new(1, "Search the web for authoritative sources", Some("web_search")),
                PlanStep::new(2, "Fetch and summarize key sources", Some("web_fetch")),
                PlanStep::new(3, "Synthesize findings into answer", None),
            ],
            TaskClass::FileOps => vec![
                PlanStep::new(1, "Locate target files", Some("fd")),
                PlanStep::new(2, "Review contents", Some("read")),
                PlanStep::new(3, "Apply edits carefully", Some("edit_file")),
            ],
            TaskClass::SystemAdmin => vec![
                PlanStep::new(1, "Gather system status", Some("suggest_discovery")),
                PlanStep::new(2, "Isolate the failing component", Some("narrow_focus")),
                PlanStep::new(3, "Apply safe remediation", Some("apply_fix")),
            ],
            TaskClass::General => vec![
                PlanStep::new(1, "Clarify goal and constraints", Some("ask_clarification")),
                PlanStep::new(2, "Propose next actions", Some("plan_next")),
                PlanStep::new(3, "Execute selected action", Some("run_command")),
            ],
        }
    }
}

impl PlanStep {
    pub fn new(order: u32, description: &str, tool: Option<&str>) -> Self {
        Self {
            order,
            description: description.to_string(),
            suggested_tool: tool.map(|t| t.to_string()),
        }
    }
}
