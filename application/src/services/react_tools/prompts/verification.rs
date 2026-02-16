use crate::services::react_context_retriever::RetrievedContext;

pub fn check_goal_prompt(context: &RetrievedContext) -> String {
    format!(
        "Determine if the original goal has been achieved based on current findings.

Goal: {goal}
Current output: {output}
Facts: {facts}

Reply with YES or NO and provide reasoning.",
        goal = context.goal,
        output = context.latest_output,
        facts = context.facts
    )
}

pub fn verify_fix_prompt(context: &RetrievedContext) -> String {
    format!(
        "Verify that the applied fix resolved the issue.

Goal: {goal}
Current output: {output}
History: {history}

Check if the issue is resolved and the system is in expected state.",
        goal = context.goal,
        output = context.latest_output,
        history = context.session_history
    )
}

pub fn verify_syntax_prompt(context: &RetrievedContext) -> String {
    format!(
        "Check the syntax of proposed changes before applying them.

Proposed changes or output: {output}

Identify any syntax errors or issues that should be fixed first.",
        output = context.latest_output
    )
}

pub fn test_hypothesis_prompt(context: &RetrievedContext) -> String {
    format!(
        "Test whether the current hypothesis is supported by the evidence.

Hypotheses: {hypotheses}
Current output: {output}
Facts: {facts}

Evaluate each hypothesis against the current findings.",
        hypotheses = context.hypotheses,
        output = context.latest_output,
        facts = context.facts
    )
}
