use crate::services::context_engineer::ContextEngineer;
use crate::services::operational_guardrails::OperationalGuardrails;
use crate::services::react_context_retriever::RetrievedContext;
use domain::entities::react::{ReactSession, ReactTool};

pub struct ReactPromptService;

impl ReactPromptService {
    pub fn new() -> Self {
        Self
    }

    pub fn tool_selection_prompt(
        &self,
        session: &ReactSession,
        reasoning: &str,
        context: &RetrievedContext,
        max_iterations: u32,
    ) -> String {
        let base = self.build_context_engineering_prompt(
            session,
            context,
            "",
            "",
            max_iterations,
            None,
        );
        format!(
            "{base}\n\
### ## TOOL_SELECTION\n\
Previous analysis:\n\
{reasoning}\n\
\n\
Based on your analysis, choose ONE tool from this list:\n\
\n\
### Investigation (need data)\n\
- suggest_command: Propose diagnostic command to run\n\
- suggest_read: Propose file to read\n\
- suggest_grep: Propose search pattern\n\
- suggest_rag: Propose RAG query for code context\n\
- suggest_discovery: Propose system discovery command\n\
- web_search: Search the web via SearXNG\n\
- web_fetch: Fetch content from a URL\n\
- read_pdf: Extract text from a PDF\n\
- read_docx: Extract text from a DOCX\n\
- read_xlsx: Read data from an XLSX/CSV\n\
- semantic_search: Semantic search across past sessions\n\
- grep_context: Grep with surrounding context\n\
\n\
### Analysis (understand data)\n\
- summarize: Summarize output in 3-5 sentences\n\
- extract_errors: Extract error messages from output\n\
- extract_warnings: Extract warnings from output\n\
- extract_metrics: Extract numeric metrics from output\n\
- extract_patterns: Find patterns in output\n\
- compare: Compare two outputs or states\n\
- correlate: Find relationships in data\n\
- web_summarize: Summarize a web page\n\
- web_extract: Extract structured data from a web page\n\
- extract_tables: Extract tables from documents\n\
- doc_qa: Q&A over document content\n\
- find_patterns: Find learned patterns from memory\n\
- code_diff: Analyze git diff\n\
- code_explain: Explain code structure\n\
\n\
### Planning (strategy)\n\
- plan_next: Propose 2-3 next steps\n\
- narrow_focus: Narrow investigation scope\n\
- branch: Explore alternative approaches\n\
- rethink: Take completely new approach\n\
- prioritize: Rank options\n\
\n\
### Action (make changes)\n\
- apply_fix: Apply a fix or change\n\
- edit_file: Edit an existing file\n\
- create_file: Create a new file\n\
- run_command: Run a command directly\n\
- retry: Retry failed operation\n\
- code_execute: Execute code with confirmation\n\
\n\
### Verification (check)\n\
- check_goal: Verify if original goal achieved\n\
- verify_fix: Verify if fix was applied correctly\n\
- verify_syntax: Check syntax before applying\n\
- test_hypothesis: Test a hypothesis\n\
- code_test: Run tests\n\
- code_lint: Run linters\n\
\n\
### Memory (context)\n\
- show_facts: Show extracted facts\n\
- show_hypotheses: Show current hypotheses\n\
- show_history: Show session history\n\
- show_context: Show all context\n\
- show_plan: Show current plan\n\
- compact_session: Compact session history\n\
- remember: Store a fact in lifelong memory\n\
- recall: Retrieve from memory\n\
- consolidate: Summarize to long-term memory\n\
- search_memory: Search lifelong memory\n\
- learn_patterns: Extract reusable patterns\n\
\n\
### Resolution (end)\n\
- conclude_success: Problem solved\n\
- conclude_fail: Cannot solve - end session\n\
- escalate: Need human assistance\n\
- defer: Defer task for later\n\
\n\
### Interaction (user)\n\
- ask_clarification: Need user clarification\n\
- ask_confirmation: Need user confirmation\n\
- explain: Explain reasoning to user\n\
- suggest_alternatives: Offer options to user\n\
\n\
Preferred format (keep concise):\n\
TOOL: <tool_name>\n\
JUSTIFY: <why this tool is the right choice>\n\
CONTEXT: <what data you're using>\n\n",
            base = base,
            reasoning = reasoning
        )
    }

    pub fn parse_tool_selection(&self, response: &str) -> Option<(ReactTool, String, String)> {
        let mut tool_name = None;
        let mut justification = String::new();
        let mut context_needed = String::new();

        for line in response.lines() {
            let line = line.trim();
            let lower = line.to_ascii_lowercase();
            if lower.starts_with("tool") {
                if let Some(rest) = line.splitn(2, |c| c == ':' || c == '-' || c == '=').nth(1)
                {
                    tool_name = Some(rest.trim().to_string());
                }
            } else if lower.starts_with("justify") {
                if let Some(rest) = line.splitn(2, |c| c == ':' || c == '-' || c == '=').nth(1)
                {
                    justification = rest.trim().to_string();
                }
            } else if lower.starts_with("context") {
                if let Some(rest) = line.splitn(2, |c| c == ':' || c == '-' || c == '=').nth(1)
                {
                    context_needed = rest.trim().to_string();
                }
            }
        }

        let parsed = tool_name
            .and_then(|name| name.parse::<ReactTool>().ok())
            .or_else(|| infer_tool_from_response(response));

        parsed.map(|tool| (tool, justification, context_needed))
    }

    pub fn reasoning_prompt(
        &self,
        session: &ReactSession,
        context: &RetrievedContext,
        learning_context: &str,
        failed_commands: &str,
        max_iterations: u32,
    ) -> String {
        let base = self.build_context_engineering_prompt(
            session,
            context,
            learning_context,
            failed_commands,
            max_iterations,
            None,
        );
        format!(
            "{base}\nANALYZE: <reasoning with citations using [REF-XX]>\n",
            base = base
        )
    }

    pub fn reasoning_prompt_with_depth(
        &self,
        session: &ReactSession,
        context: &RetrievedContext,
        learning_context: &str,
        failed_commands: &str,
        depth: u8,
        previous_reasoning: Option<&str>,
        max_iterations: u32,
    ) -> String {
        let depth_instruction = match depth {
            1 => "This is Step 1 of reasoning. Focus on INITIAL UNDERSTANDING of the user's query. What is the user asking for? What is the context?",
            2 => "This is Step 2 of reasoning. REFINE your understanding based on initial analysis. What specific information do you need? What constraints apply?",
            _ => "This is Step 3 (final) reasoning. VERIFY your understanding and prepare the final command. Confirm all requirements from the query are met.",
        };

        let mut base = self.build_context_engineering_prompt(
            session,
            context,
            learning_context,
            failed_commands,
            max_iterations,
            Some(depth_instruction),
        );

        if let Some(prev) = previous_reasoning {
            if !prev.trim().is_empty() {
                base.push_str("\n### ## PREVIOUS_REASONING\n");
                base.push_str(prev);
                base.push_str("\n");
            }
        }

        format!("{base}\nANALYZE: <reasoning with citations using [REF-XX]>\n", base = base)
    }

    fn build_context_engineering_prompt(
        &self,
        session: &ReactSession,
        context: &RetrievedContext,
        learning_context: &str,
        failed_commands: &str,
        max_iterations: u32,
        depth_instruction: Option<&str>,
    ) -> String {
        let task_type = task_type_label(session);
        let mut engineer = ContextEngineer::new(&session.query)
            .with_session_id(&session.id)
            .with_iteration((context.steps as u32).max(1), max_iterations)
            .with_task_type(&task_type);

        let guardrails = OperationalGuardrails::default().with_delta_only(should_delta_only(&session.query));
        engineer = engineer.with_guardrails(guardrails);

        engineer.add_session_history(&context.session_history);
        engineer.add_latest_output(
            &context.latest_output,
            context.latest_output_source.as_deref(),
        );
        engineer.add_facts(&context.facts_list);
        engineer.add_hypotheses(&context.hypotheses_list);

        let avoid_commands = normalize_failed_commands(failed_commands);
        engineer.add_constraints(&context.constraints_list, avoid_commands.as_deref());

        if !learning_context.trim().is_empty() {
            engineer.add_learning_context(learning_context);
        }
        if !context.knowledge_context.trim().is_empty() {
            engineer.add_knowledge_base(&context.knowledge_context);
        }
        if let Some(similar) = &context.similar_sessions_context {
            engineer.add_knowledge_base(similar);
        }
        if let Some(patterns) = &context.command_patterns_context {
            engineer.add_knowledge_base(patterns);
        }

        let mut prompt = engineer.render(
            &session.query,
            (context.steps as u32).saturating_add(1),
            max_iterations.max(1),
            &task_type,
        );

        if let Some(depth) = depth_instruction {
            prompt.push_str("### ## REASONING_DEPTH\n");
            prompt.push_str(depth);
            prompt.push_str("\n\n");
        }

        prompt
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
Preferred response is a JSON array of command strings, but brief prose is OK. We will extract commands automatically.\n\
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

    pub fn command_extraction_prompt(&self, goal: &str, raw: &str) -> String {
        format!(
            "Extract 1-3 executable shell commands from the text below.\n\
Return ONLY a JSON array of strings. No prose.\n\
- If the text says things like \"execute the top command\", return \"top\".\n\
- If commands are inside backticks or quotes, extract those.\n\
- Do not include placeholders like <path> or /path/to/...\n\
Goal: {goal}\n\
Text:\n{raw}\n",
            goal = goal,
            raw = raw
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

fn infer_tool_from_response(response: &str) -> Option<ReactTool> {
    let lower = response.to_ascii_lowercase();
    for tool in ReactTool::available_tools() {
        let name = tool.name();
        if lower.contains(name) || lower.contains(&name.replace('_', " ")) {
            return Some(*tool);
        }
    }
    None
}

fn task_type_label(session: &ReactSession) -> String {
    session
        .intent
        .as_ref()
        .map(|intent| format!("{:?}", intent.task_type))
        .unwrap_or_else(|| "Analyze".to_string())
}

fn should_delta_only(query: &str) -> bool {
    let lower = query.to_lowercase();
    ["edit", "update", "refactor", "implement", "fix", "patch"]
        .iter()
        .any(|kw| lower.contains(kw))
}

fn normalize_failed_commands(failed: &str) -> Option<String> {
    let trimmed = failed.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
        None
    } else {
        Some(format!("avoid_commands: {}", trimmed))
    }
}
