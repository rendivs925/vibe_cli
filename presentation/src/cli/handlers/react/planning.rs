use application::services::react_agent_service::planner::PlanStep;

pub fn format_plan(steps: &[PlanStep]) -> String {
    if steps.is_empty() {
        return "(no plan available)".to_string();
    }
    let mut lines = Vec::new();
    for step in steps {
        if let Some(tool) = &step.suggested_tool {
            lines.push(format!("{}. {} (tool: {})", step.order, step.description, tool));
        } else {
            lines.push(format!("{}. {}", step.order, step.description));
        }
    }
    lines.join("\n")
}
