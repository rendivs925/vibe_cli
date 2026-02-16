use crate::services::react_context_retriever::RetrievedContext;

pub fn suggest_command_prompt(context: &RetrievedContext) -> String {
    format!(
        "Based on the current context and goal, suggest 1-3 diagnostic commands to investigate further.

Goal: {goal}
Latest Output: {output}
History: {history}

Suggest specific, actionable commands that will help progress toward the goal.
Respond with just the command(s), one per line.",
        goal = context.goal,
        output = context.latest_output,
        history = context.session_history
    )
}

pub fn suggest_read_prompt(context: &RetrievedContext) -> String {
    format!(
        "Based on the current context, identify which file would be most valuable to read.

Goal: {goal}
Latest Output: {output}
History: {history}

Recommend a specific file path that likely contains relevant information.
Respond with just the file path.",
        goal = context.goal,
        output = context.latest_output,
        history = context.session_history
    )
}

pub fn suggest_grep_prompt(context: &RetrievedContext) -> String {
    format!(
        "Based on the current context, identify a search pattern that would help find relevant information.

Goal: {goal}
Latest Output: {output}
History: {history}

Suggest a grep pattern (regex) to search for relevant content.
Respond with just the pattern.",
        goal = context.goal,
        output = context.latest_output,
        history = context.session_history
    )
}

pub fn suggest_rag_prompt(context: &RetrievedContext) -> String {
    format!(
        "Based on the current context, formulate a RAG query for codebase exploration.

Goal: {goal}
Latest Output: {output}
History: {history}

Create a natural language query that would help find relevant code or documentation.
Respond with just the query.",
        goal = context.goal,
        output = context.latest_output,
        history = context.session_history
    )
}

pub fn suggest_discovery_prompt(context: &RetrievedContext) -> String {
    format!(
        "Based on the current context, suggest system discovery commands to gather information.

Goal: {goal}
Latest Output: {output}
History: {history}

Recommend commands to discover system state (processes, services, disk usage, etc.).
Respond with 1-3 commands, one per line.",
        goal = context.goal,
        output = context.latest_output,
        history = context.session_history
    )
}
