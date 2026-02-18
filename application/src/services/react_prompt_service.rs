use crate::services::react_context_retriever::RetrievedContext;
use domain::entities::react::ReactTool;

pub struct ReactPromptService;

impl ReactPromptService {
    pub fn new() -> Self {
        Self
    }

    pub fn tool_selection_prompt(
        &self,
        goal: &str,
        reasoning: &str,
        context: &RetrievedContext,
    ) -> String {
        format!(
            "You are a systems debugging assistant using a ReAct loop with dynamic tool selection.\n\
\n\
## Current Task\n\
{goal}\n\
\n\
## Previous Analysis\n\
{reasoning}\n\
\n\
## Context\n\
{latest_output}\n\
\n\
## Session History\n\
{history}\n\
\n\
## TOOL SELECTION\n\
\n\
Based on your analysis, choose ONE tool from this list:\n\
\n\
### Investigation (need data)\n\
- suggest_command: Propose diagnostic command to run\n\
- suggest_read: Propose file to read\n\
- suggest_grep: Propose search pattern\n\
- suggest_rag: Propose RAG query for code context\n\
- suggest_discovery: Propose system discovery command\n\
\n\
### Analysis (understand data)\n\
- summarize: Summarize output in 3-5 sentences\n\
- extract_errors: Extract error messages from output\n\
- extract_warnings: Extract warnings from output\n\
- extract_metrics: Extract numeric metrics from output\n\
- compare: Compare two outputs or states\n\
\n\
### Planning (strategy)\n\
- plan_next: Propose 2-3 next steps\n\
- narrow_focus: Narrow investigation scope\n\
- branch: Explore alternative approaches\n\
- rethink: Take completely new approach\n\
\n\
### Action (make changes)\n\
- apply_fix: Apply a fix or change\n\
- edit_file: Edit an existing file\n\
- create_file: Create a new file\n\
\n\
### Verification (check)\n\
- check_goal: Verify if original goal achieved\n\
- verify_fix: Verify if fix was applied correctly\n\
\n\
### Memory (context)\n\
- show_facts: Show extracted facts\n\
- show_hypotheses: Show current hypotheses\n\
- show_history: Show session history\n\
\n\
### Resolution (end)\n\
- conclude_success: Problem solved\n\
- conclude_fail: Cannot solve, escalate needed\n\
\n\
### Interaction (user)\n\
- ask_clarification: Need user clarification\n\
- explain: Explain reasoning to user\n\
\n\
---\n\
\n\
Respond in this exact format:\n\
TOOL: <tool_name>\n\
JUSTIFY: <why this tool is the right choice>\n\
CONTEXT: <what data you're using>\n\n",
            goal = goal,
            reasoning = reasoning,
            latest_output = context.latest_output,
            history = context.session_history,
        )
    }

    pub fn parse_tool_selection(&self, response: &str) -> Option<(ReactTool, String, String)> {
        let mut tool_name = None;
        let mut justification = String::new();
        let mut context_needed = String::new();

        for line in response.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("TOOL:") {
                tool_name = Some(rest.trim().to_string());
            } else if let Some(rest) = line.strip_prefix("JUSTIFY:") {
                justification = rest.trim().to_string();
            } else if let Some(rest) = line.strip_prefix("CONTEXT:") {
                context_needed = rest.trim().to_string();
            }
        }

        tool_name.and_then(|name| {
            name.parse::<ReactTool>()
                .ok()
                .map(|tool| (tool, justification, context_needed))
        })
    }

    pub fn reasoning_prompt(
        &self,
        goal: &str,
        context: &RetrievedContext,
        learning_context: &str,
        failed_commands: &str,
    ) -> String {
        format!(
            "You are a systems debugging assistant using a conversational ReAct loop.\n\
\n\
## Current Task\n\
{goal}\n\
\n\
## Session History - MOST RECENT LAST\n\
{history}\n\
\n\
## Latest Output - ALWAYS USE THIS\n\
{latest_output}\n\
\n\
## Extracted Facts\n\
{facts}\n\
\n\
## Current Hypotheses\n\
{hypotheses}\n\
\n\
## Constraints\n\
{constraints}\n\
\n\
## System Context\n\
{knowledge_context}\n\
\n\
{learning_context}\n\
\n\
## Avoid These Commands\n\
{failed_commands}\n\
\n\
## STRICT BEHAVIORAL RULES\n\
\n\
### Latest Output Supremacy Rule\n\
The MOST RECENT OUTPUT block above overrides ALL prior assumptions.\n\
- Do NOT rely on earlier outputs if newer data exists\n\
- Treat each tool execution as NEW evidence\n\
- If you repeat prior analysis without the latest output, you FAIL\n\
\n\
### Evidence-Based Reasoning\n\
- Every ANALYZE must explicitly reference CONCRETE details from the LATEST OUTPUT\n\
- Do NOT produce generic fallback explanations\n\
- Do NOT restate prior reasoning unless directly supported by new evidence\n\
\n\
### Progressive Problem Solving\n\
- Each suggested action must move the investigation FORWARD\n\
- Avoid loops and re-running broad diagnostics without narrowing scope\n\
- Adapt strategy dynamically based on RESULTS\n\
\n\
### Loop Prevention\n\
Before suggesting ANY action, you MUST check:\n\
- Has this exact action already been executed in history?\n\
- If YES, you MUST provide a NEW justification or choose a DIFFERENT action\n\
- Do NOT repeat the same command without explicit justification\n\
\n\
## Instructions\n\
- Use FACTS from the latest output to support reasoning\n\
- Consider user constraints\n\
- AVOID commands that failed before\n\
- Focus on the next NARROW diagnostic step\n\
- Do not include commands or code blocks\n\
- Your analysis must be GROUNDED in the most recent OUTPUT\n\
\n\
Output format:\n\
ANALYZE: <reasoning referencing latest output>",
            goal = goal,
            history = context.session_history,
            latest_output = context.latest_output,
            facts = context.facts,
            hypotheses = context.hypotheses,
            constraints = context.constraints,
            knowledge_context = context.knowledge_context,
            learning_context = learning_context,
            failed_commands = failed_commands,
        )
    }

    pub fn reasoning_prompt_with_depth(
        &self,
        goal: &str,
        context: &RetrievedContext,
        learning_context: &str,
        failed_commands: &str,
        depth: u8,
        previous_reasoning: Option<&str>,
    ) -> String {
        let depth_instruction = match depth {
            1 => "This is Step 1 of reasoning. Focus on INITIAL UNDERSTANDING of the user's query. What is the user asking for? What is the context?",
            2 => "This is Step 2 of reasoning. REFINE your understanding based on initial analysis. What specific information do you need? What constraints apply?",
            _ => "This is Step 3 (final) reasoning. VERIFY your understanding and prepare the final command. Confirm all requirements from the query are met.",
        };

        let previous = if let Some(prev) = previous_reasoning {
            format!("## Previous Reasoning\n{}\n", prev)
        } else {
            String::new()
        };

        format!(
            "You are a systems debugging assistant using a conversational ReAct loop.\n\
\n\
## Current Task\n\
{goal}\n\
\n\
{previous}\
## Session History - MOST RECENT LAST\n\
{history}\n\
\n\
## Latest Output - ALWAYS USE THIS\n\
{latest_output}\n\
\n\
## Extracted Facts\n\
{facts}\n\
\n\
## Current Hypotheses\n\
{hypotheses}\n\
\n\
## Constraints\n\
{constraints}\n\
\n\
## System Context\n\
{knowledge_context}\n\
\n\
{learning_context}\n\
\n\
## Avoid These Commands\n\
{failed_commands}\n\
\n\
## STRICT BEHAVIORAL RULES\n\
\n\
### Reasoning Depth - Step {depth}\n\
{depth_instruction}\n\
\n\
### Latest Output Supremacy Rule\n\
The MOST RECENT OUTPUT block above overrides ALL prior assumptions.\n\
- Do NOT rely on earlier outputs if newer data exists\n\
- Treat each tool execution as NEW evidence\n\
- If you repeat prior analysis without the latest output, you FAIL\n\
\n\
### Evidence-Based Reasoning\n\
- Every ANALYZE must explicitly reference CONCRETE details from the LATEST OUTPUT\n\
- Do NOT produce generic fallback explanations\n\
- Do NOT restate prior reasoning unless directly supported by new evidence\n\
\n\
### Progressive Problem Solving\n\
- Each suggested action must move the investigation FORWARD\n\
- Avoid loops and re-running broad diagnostics without narrowing scope\n\
- Adapt strategy dynamically based on RESULTS\n\
\n\
### Loop Prevention\n\
Before suggesting ANY action, you MUST check:\n\
- Has this exact action already been executed in history?\n\
- If YES, you MUST provide a NEW justification or choose a DIFFERENT action\n\
- Do NOT repeat the same command without explicit justification\n\
\n\
## Instructions\n\
- Use FACTS from the latest output to support reasoning\n\
- Consider user constraints\n\
- AVOID commands that failed before\n\
- Focus on the next NARROW diagnostic step\n\
- Do not include commands or code blocks\n\
- Your analysis must be GROUNDED in the most recent OUTPUT\n\
- At Step 3, provide a clear command if appropriate\n\
\n\
Output format:\n\
ANALYZE: <reasoning referencing latest output>",
            goal = goal,
            history = context.session_history,
            latest_output = context.latest_output,
            facts = context.facts,
            hypotheses = context.hypotheses,
            constraints = context.constraints,
            knowledge_context = context.knowledge_context,
            learning_context = learning_context,
            failed_commands = failed_commands,
            depth = depth,
            depth_instruction = depth_instruction,
        )
    }

    pub fn analysis_prompt(
        &self,
        goal: &str,
        output: &str,
        facts: &str,
        hypotheses: &str,
    ) -> String {
        format!(
            "You are a helpful assistant. Analyze the command output and provide USEFUL INFORMATION to the user.\n\
\n\
## Original User Query\n\
{goal}\n\
\n\
## Command Output\n\
{output}\n\
\n\
## Context (if available)\n\
Facts: {facts}\n\
Hypotheses: {hypotheses}\n\
\n\
## Instructions\n\
Analyze the command output and provide useful, actionable information to the user.\n\
- Explain what the output means in plain language\n\
- Point out any issues, errors, or anomalies you notice\n\
- Suggest what the user should do next if relevant\n\
- Do NOT use rigid sections like \"Summary:\", \"Errors:\", \"Warnings:\" - just write naturally\n\
- Keep it concise but informative\n\
\n\
Provide your analysis:",
            goal = goal,
            output = output,
            facts = if facts.is_empty() { "None" } else { facts },
            hypotheses = if hypotheses.is_empty() { "None" } else { hypotheses },
        )
    }

    pub fn command_prompt(
        &self,
        goal: &str,
        reasoning: &str,
        context: &RetrievedContext,
        failed_commands: &str,
    ) -> String {
        format!(
            "You are a cautious systems assistant. Based on the goal and reasoning, propose 1-3 executable suggestions.\n\
Respond ONLY with a JSON array of strings. No prose.\n\
Goal: {goal}\n\
Reasoning (MUST be grounded in latest output): {reasoning}\n\
\n\
## Latest Output - USE THIS DATA\n\
{latest_output}\n\
\n\
History:\n{history}\n\
Facts:\n{facts}\n\
Hypotheses:\n{hypotheses}\n\
Constraints:\n{constraints}\n\
System Context:\n{knowledge_context}\n\
Avoid commands:\n{failed_commands}\n\
\n\
## STRICT LOOP PREVENTION RULES\n\
Before proposing ANY command:\n\
1. CHECK the History above - has this exact command been executed?\n\
2. If YES: You MUST either:\n\
   - Propose a DIFFERENT command that advances the investigation\n\
   - OR explain WHY you are intentionally repeating it (e.g., verifying fix)\n\
3. NEVER propose the same diagnostic command twice without new justification\n\
4. Each command must narrow the problem scope\n\
\n\
Available tools:\
\n\
- read <path> [lines] [offset]\n\
- grep <pattern> [path]\n\
- fd <pattern> [directory]\n\
- rag <query> [num_results]\n\
- sed <pattern> <replacement> <path>\n\
- perl <regex> <replacement> <path>\n\
- awk <script> <path>\n\
- apply_patch <patch_file>\n\
- write <path> <content>\n\
- remove <path>\n\
- update <path> <old> <new>\n\
- replace_block <path> <old_block> <new_block>\n\
- shell <command>\n\
- pkg <install|remove|search|update|upgrade> [package]\n\
- svc <start|stop|restart|status|enable|disable> <service>\n\
- git <status|diff|add|commit|log> [args]\n\
- build <check|build|fmt|clippy> [package]\n\
- test [pattern]\n\
Constraints:\n\
- Prefer read-only diagnostics first.\n\
- Avoid destructive commands.\n\
- Use standard Linux tools.\n\
- If a built-in tool is better, output it directly as the command string.\n\
- NEVER use placeholder paths such as <path>, /path/to/..., your_file, or /tmp/example.\n\
- Only use real paths likely to exist under current working directory.\n\
- If path is unknown, first suggest discovery commands using shell/fd/find to locate files.\n\
- For codebase exploration/explanation tasks, prefer rag first; rag is AST-aware during indexing.\n\
- For project explanation tasks, start by discovering structure, then read real files (README.md, Cargo.toml, src/*).\n",
            goal = goal,
            reasoning = reasoning,
            latest_output = context.latest_output,
            history = context.session_history,
            facts = context.facts,
            hypotheses = context.hypotheses,
            constraints = context.constraints,
            knowledge_context = context.knowledge_context,
            failed_commands = failed_commands,
        )
    }

    pub fn symbolic_inference_prompt(&self, goal: &str, history: &str) -> String {
        format!(
            "You are a symbolic diagnostics engine for Linux troubleshooting.\n\
Based on the ReAct history, produce a concise symbolic inference in this exact shape:\n\
Rule: <rule_name>\n\
Conditions:\n\
  - <condition 1>\n\
Conclusion: <single sentence>\n\
No markdown fences. No extra sections.\n\
Goal: {goal}\n\
History:\n{history}\n",
            goal = goal,
            history = history,
        )
    }

    pub fn goal_check_prompt(&self, goal: &str, history: &str) -> String {
        format!(
            "Decide if the troubleshooting goal is achieved based on history.\n\
Reply with ONLY YES or NO.\n\
Goal: {goal}\n\
History:\n{history}\n",
            goal = goal,
            history = history,
        )
    }

    pub fn goal_summary_prompt(&self, goal: &str, history: &str) -> String {
        format!(
            "Summarize troubleshooting result in this exact format:\n\
Root cause: <text>\n\
Fix applied: <text>\n\
Use \"Unknown\" when not confirmed. No extra lines.\n\
Goal: {goal}\n\
History:\n{history}\n",
            goal = goal,
            history = history,
        )
    }

    pub fn compact_prompt(&self, history: &str) -> String {
        format!(
            "Summarize the troubleshooting history into 3-5 concise sentences.\n\
No bullet points. No extra headers.\n\
History:\n{history}\n",
            history = history
        )
    }
}
