use crate::services::react_context_retriever::RetrievedContext;

pub fn conclude_success_prompt(context: &RetrievedContext) -> String {
    format!(
        "The problem appears to be solved. Provide a final summary.

Goal: {goal}
Facts: {facts}
History: {history}

Summarize the root cause and resolution.",
        goal = context.goal,
        facts = context.facts,
        history = context.session_history
    )
}

pub fn conclude_fail_prompt(context: &RetrievedContext) -> String {
    format!(
        "Unable to solve the problem. Summarize what was tried and why it failed.

Goal: {goal}
History: {history}
Facts: {facts}

Explain why the issue couldn't be resolved and what would be needed.",
        goal = context.goal,
        history = context.session_history,
        facts = context.facts
    )
}

pub fn escalate_prompt(context: &RetrievedContext) -> String {
    format!(
        "Human assistance is needed. Prepare escalation information.

Goal: {goal}
History: {history}
Facts: {facts}
Steps: {steps}

Summarize what a human expert would need to know to take over.",
        goal = context.goal,
        history = context.session_history,
        facts = context.facts,
        steps = context.steps
    )
}

pub fn defer_prompt(context: &RetrievedContext) -> String {
    format!(
        "Defer this task for later. Document current state.

Goal: {goal}
Current findings: {facts}

Summarize progress and what to do when resuming.",
        goal = context.goal,
        facts = context.facts
    )
}
