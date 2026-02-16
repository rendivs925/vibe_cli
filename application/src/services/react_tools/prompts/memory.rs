use crate::services::react_context_retriever::RetrievedContext;

pub fn compact_session_prompt(context: &RetrievedContext) -> String {
    format!(
        "Summarize the session history into a compact form.

Goal: {goal}
History: {history}
Facts: {facts}

Create a concise summary that captures the key findings and current state.",
        goal = context.goal,
        history = context.session_history,
        facts = context.facts
    )
}
