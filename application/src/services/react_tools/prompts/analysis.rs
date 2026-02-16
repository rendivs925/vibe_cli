use crate::services::react_context_retriever::RetrievedContext;

pub fn summarize_prompt(context: &RetrievedContext) -> String {
    format!(
        "Summarize the following output in 3-5 concise sentences.

Output to summarize:
{output}

Provide a brief summary capturing the key points.",
        output = context.latest_output
    )
}

pub fn extract_errors_prompt(context: &RetrievedContext) -> String {
    format!(
        "Extract all error messages from the following output.

Output to analyze:
{output}

List all error messages, one per line. If no errors found, say 'No errors found.'",
        output = context.latest_output
    )
}

pub fn extract_warnings_prompt(context: &RetrievedContext) -> String {
    format!(
        "Extract all warning messages from the following output.

Output to analyze:
{output}

List all warning messages, one per line. If no warnings found, say 'No warnings found.'",
        output = context.latest_output
    )
}

pub fn extract_metrics_prompt(context: &RetrievedContext) -> String {
    format!(
        "Extract all numeric metrics from the following output (percentages, sizes, counts, etc.).

Output to analyze:
{output}

List all metrics found in format 'metric_name: value'. If no metrics found, say 'No metrics found.'",
        output = context.latest_output
    )
}

pub fn extract_patterns_prompt(context: &RetrievedContext) -> String {
    format!(
        "Identify patterns in the following output (repeated structures, common prefixes, etc.).

Output to analyze:
{output}

Describe any patterns detected. If no patterns found, say 'No patterns detected.'",
        output = context.latest_output
    )
}

pub fn compare_prompt(context: &RetrievedContext) -> String {
    format!(
        "Compare the current output with the previous state and identify differences.

Current output:
{output}

History:
{history}

Describe what has changed or what's different. Focus on key differences.",
        output = context.latest_output,
        history = context.session_history
    )
}

pub fn correlate_prompt(context: &RetrievedContext) -> String {
    format!(
        "Find relationships and correlations between the current output and known facts.

Current output:
{output}

Known facts:
{facts}

Hypotheses:
{hypotheses}

Identify any connections or correlations. If none found, say 'No correlations detected.'",
        output = context.latest_output,
        facts = context.facts,
        hypotheses = context.hypotheses
    )
}
