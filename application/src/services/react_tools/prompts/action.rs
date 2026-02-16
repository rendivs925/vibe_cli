use crate::services::react_context_retriever::RetrievedContext;

pub fn apply_fix_prompt(context: &RetrievedContext) -> String {
    format!(
        "Based on the findings, plan a fix for the issue.

Goal: {goal}
Current output: {output}
Facts: {facts}

Outline a fix plan with clear steps.",
        goal = context.goal,
        output = context.latest_output,
        facts = context.facts
    )
}

pub fn edit_file_prompt(context: &RetrievedContext) -> String {
    format!(
        "Identify which file needs to be edited to address the current issue.

Goal: {goal}
Current output: {output}

Suggest the specific file path and what changes might be needed.",
        goal = context.goal,
        output = context.latest_output
    )
}

pub fn create_file_prompt(context: &RetrievedContext) -> String {
    format!(
        "Identify what new file might need to be created.

Goal: {goal}
Current output: {output}

Suggest a file path and describe what content it should contain.",
        goal = context.goal,
        output = context.latest_output
    )
}

pub fn run_command_prompt(context: &RetrievedContext) -> String {
    format!(
        "Specify a command to run directly.

Goal: {goal}
Current output: {output}

Provide the exact command to execute.",
        goal = context.goal,
        output = context.latest_output
    )
}

pub fn retry_prompt(context: &RetrievedContext) -> String {
    format!(
        "The previous operation failed. Prepare to retry.

Goal: {goal}
History: {history}

Identify what failed and suggest how to retry it.",
        goal = context.goal,
        history = context.session_history
    )
}
