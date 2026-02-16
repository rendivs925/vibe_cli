use crate::services::react_context_retriever::RetrievedContext;

pub fn plan_next_prompt(context: &RetrievedContext) -> String {
    format!(
        "Based on the current state, propose 2-3 concrete next steps to move toward the goal.

Goal: {goal}
Current output: {output}
History: {history}

Suggest actionable next steps with clear rationale.",
        goal = context.goal,
        output = context.latest_output,
        history = context.session_history
    )
}

pub fn narrow_focus_prompt(context: &RetrievedContext) -> String {
    format!(
        "The investigation may be too broad. Identify a specific, narrow focus area.

Goal: {goal}
Current output: {output}
Facts: {facts}

Recommend a narrow, specific area to focus on next.",
        goal = context.goal,
        output = context.latest_output,
        facts = context.facts
    )
}

pub fn branch_prompt(context: &RetrievedContext) -> String {
    format!(
        "There may be multiple approaches. Describe 2-3 alternative paths forward.

Goal: {goal}
Current output: {output}

Present alternative approaches with trade-offs for each.",
        goal = context.goal,
        output = context.latest_output
    )
}

pub fn rethink_prompt(context: &RetrievedContext) -> String {
    format!(
        "Current approach may not be working. Suggest a completely different strategy.

Goal: {goal}
History: {history}
Steps taken: {steps}

Propose a fresh approach or new angle to investigate.",
        goal = context.goal,
        history = context.session_history,
        steps = context.steps
    )
}

pub fn prioritize_prompt(context: &RetrievedContext) -> String {
    format!(
        "Rank the possible next actions by importance and impact.

Goal: {goal}
Current state: {output}

Provide a prioritized list of what to do next.",
        goal = context.goal,
        output = context.latest_output
    )
}
