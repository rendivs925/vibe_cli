use crate::services::react_context_retriever::RetrievedContext;

pub fn ask_clarification_prompt(context: &RetrievedContext) -> String {
    format!(
        "The user's request is unclear or ambiguous. Formulate a clarification question.

Goal: {goal}
History: {history}

Ask a specific question to get the information needed to proceed.",
        goal = context.goal,
        history = context.session_history
    )
}

pub fn ask_confirmation_prompt(context: &RetrievedContext) -> String {
    format!(
        "Summarize the proposed action and ask for user confirmation.

Goal: {goal}
Current plan: Based on findings so far

Request confirmation before proceeding.",
        goal = context.goal
    )
}

pub fn explain_prompt(context: &RetrievedContext) -> String {
    format!(
        "Explain the reasoning and approach to the user.

Goal: {goal}
Facts: {facts}
History: {history}

Provide a clear explanation of what we're doing and why.",
        goal = context.goal,
        facts = context.facts,
        history = context.session_history
    )
}

pub fn suggest_alternatives_prompt(context: &RetrievedContext) -> String {
    format!(
        "There are multiple ways to proceed. Present options to the user.

Goal: {goal}
Current state: {output}

Offer 2-3 alternative approaches with trade-offs for each.",
        goal = context.goal,
        output = context.latest_output
    )
}
