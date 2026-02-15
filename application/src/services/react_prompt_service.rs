use crate::services::react_context_retriever::RetrievedContext;

pub struct ReactPromptService;

impl ReactPromptService {
    pub fn new() -> Self {
        Self
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
## Session History (recent)\n\
{history}\n\
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
{learning_context}\n\
\n\
## Avoid These Commands\n\
{failed_commands}\n\
\n\
## Instructions\n\
- Use facts to support reasoning.\n\
- Consider user constraints.\n\
- Avoid commands that failed before.\n\
- Focus on the next diagnostic step.\n\
- Do not include commands or code blocks.\n\
\n\
Output format:\n\
ANALYZE: <reasoning>\n",
            goal = goal,
            history = context.session_history,
            facts = context.facts,
            hypotheses = context.hypotheses,
            constraints = context.constraints,
            learning_context = learning_context,
            failed_commands = failed_commands,
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
Reasoning: {reasoning}\n\
History:\n{history}\n\
Facts:\n{facts}\n\
Hypotheses:\n{hypotheses}\n\
Constraints:\n{constraints}\n\
Avoid commands:\n{failed_commands}\n\
Available tools:\n\
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
            history = context.session_history,
            facts = context.facts,
            hypotheses = context.hypotheses,
            constraints = context.constraints,
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
