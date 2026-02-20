use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::entities::{Fact, Hypothesis, QueryIntent, SessionMemory};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactSession {
    pub id: String,
    pub query: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub status: ReactStatus,
    pub steps: Vec<ReactStep>,
    pub context: HashMap<String, String>,
    pub memory: SessionMemory,
    pub intent: Option<QueryIntent>,
    pub compacted_summary: Option<String>,
    pub compacted_history_at: Option<DateTime<Utc>>,
    pub neurosymbolic_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReactStatus {
    Running,
    Completed,
    Failed,
    Aborted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactStep {
    pub id: String,
    pub session_id: String,
    pub step_type: ReactStepType,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub status: ReactStepStatus,
    pub commands: Vec<ProposedCommand>,
    pub observations: Vec<String>,
    pub reasoning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReactStepType {
    Thought,
    Action,
    Observation,
    Verify,
    Complete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReactStepStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposedCommand {
    pub id: String,
    pub command: String,
    pub description: String,
    pub reasoning: String,
    pub safety: CommandSafety,
    pub approved: Option<bool>,
    pub executed: bool,
    pub exit_code: Option<i32>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactContext {
    pub current_step: usize,
    pub iteration_count: u32,
    pub max_iterations: u32,
    pub available_tools: Vec<String>,
    pub user_preferences: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReactTool {
    // Category A: Investigation Tools (Gathering Data)
    SuggestCommand,
    SuggestRead,
    SuggestGrep,
    SuggestRag,
    SuggestDiscovery,
    WebSearch,
    WebFetch,
    ReadPdf,
    ReadDocx,
    ReadXlsx,
    SemanticSearch,
    GrepContext,

    // Category B: Analysis Tools (Understanding Data)
    Summarize,
    ExtractErrors,
    ExtractWarnings,
    ExtractMetrics,
    ExtractPatterns,
    Compare,
    Correlate,
    WebSummarize,
    WebExtract,
    ExtractTables,
    DocQa,
    FindPatterns,
    CodeDiff,
    CodeExplain,

    // Category C: Planning Tools (Strategy)
    PlanNext,
    NarrowFocus,
    Branch,
    Rethink,
    Prioritize,

    // Category D: Action Tools (Making Changes)
    ApplyFix,
    EditFile,
    CreateFile,
    RunCommand,
    Retry,
    CodeExecute,

    // Category E: Verification Tools (Checking)
    CheckGoal,
    VerifyFix,
    VerifySyntax,
    TestHypothesis,
    CodeTest,
    CodeLint,

    // Category F: Memory Tools (Context)
    ShowFacts,
    ShowHypotheses,
    ShowHistory,
    ShowContext,
    ShowPlan,
    CompactSession,
    Remember,
    Recall,
    Consolidate,
    SearchMemory,
    LearnPatterns,

    // Category G: Resolution Tools (Ending)
    ConcludeSuccess,
    ConcludeFail,
    Escalate,
    Defer,

    // Category H: Interaction Tools (User)
    AskClarification,
    AskConfirmation,
    Explain,
    SuggestAlternatives,
}

impl ReactTool {
    pub fn name(&self) -> &'static str {
        match self {
            ReactTool::SuggestCommand => "suggest_command",
            ReactTool::SuggestRead => "suggest_read",
            ReactTool::SuggestGrep => "suggest_grep",
            ReactTool::SuggestRag => "suggest_rag",
            ReactTool::SuggestDiscovery => "suggest_discovery",
            ReactTool::WebSearch => "web_search",
            ReactTool::WebFetch => "web_fetch",
            ReactTool::ReadPdf => "read_pdf",
            ReactTool::ReadDocx => "read_docx",
            ReactTool::ReadXlsx => "read_xlsx",
            ReactTool::SemanticSearch => "semantic_search",
            ReactTool::GrepContext => "grep_context",
            ReactTool::Summarize => "summarize",
            ReactTool::ExtractErrors => "extract_errors",
            ReactTool::ExtractWarnings => "extract_warnings",
            ReactTool::ExtractMetrics => "extract_metrics",
            ReactTool::ExtractPatterns => "extract_patterns",
            ReactTool::Compare => "compare",
            ReactTool::Correlate => "correlate",
            ReactTool::WebSummarize => "web_summarize",
            ReactTool::WebExtract => "web_extract",
            ReactTool::ExtractTables => "extract_tables",
            ReactTool::DocQa => "doc_qa",
            ReactTool::FindPatterns => "find_patterns",
            ReactTool::CodeDiff => "code_diff",
            ReactTool::CodeExplain => "code_explain",
            ReactTool::PlanNext => "plan_next",
            ReactTool::NarrowFocus => "narrow_focus",
            ReactTool::Branch => "branch",
            ReactTool::Rethink => "rethink",
            ReactTool::Prioritize => "prioritize",
            ReactTool::ApplyFix => "apply_fix",
            ReactTool::EditFile => "edit_file",
            ReactTool::CreateFile => "create_file",
            ReactTool::RunCommand => "run_command",
            ReactTool::Retry => "retry",
            ReactTool::CodeExecute => "code_execute",
            ReactTool::CheckGoal => "check_goal",
            ReactTool::VerifyFix => "verify_fix",
            ReactTool::VerifySyntax => "verify_syntax",
            ReactTool::TestHypothesis => "test_hypothesis",
            ReactTool::CodeTest => "code_test",
            ReactTool::CodeLint => "code_lint",
            ReactTool::ShowFacts => "show_facts",
            ReactTool::ShowHypotheses => "show_hypotheses",
            ReactTool::ShowHistory => "show_history",
            ReactTool::ShowContext => "show_context",
            ReactTool::ShowPlan => "show_plan",
            ReactTool::CompactSession => "compact_session",
            ReactTool::Remember => "remember",
            ReactTool::Recall => "recall",
            ReactTool::Consolidate => "consolidate",
            ReactTool::SearchMemory => "search_memory",
            ReactTool::LearnPatterns => "learn_patterns",
            ReactTool::ConcludeSuccess => "conclude_success",
            ReactTool::ConcludeFail => "conclude_fail",
            ReactTool::Escalate => "escalate",
            ReactTool::Defer => "defer",
            ReactTool::AskClarification => "ask_clarification",
            ReactTool::AskConfirmation => "ask_confirmation",
            ReactTool::Explain => "explain",
            ReactTool::SuggestAlternatives => "suggest_alternatives",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ReactTool::SuggestCommand => "Propose diagnostic command to run",
            ReactTool::SuggestRead => "Propose file to read",
            ReactTool::SuggestGrep => "Propose search pattern",
            ReactTool::SuggestRag => "Propose RAG query for code context",
            ReactTool::SuggestDiscovery => "Propose system discovery command",
            ReactTool::WebSearch => "Search the web via SearXNG",
            ReactTool::WebFetch => "Fetch content from a URL",
            ReactTool::ReadPdf => "Extract text from a PDF document",
            ReactTool::ReadDocx => "Extract text from a DOCX document",
            ReactTool::ReadXlsx => "Read data from XLSX or CSV",
            ReactTool::SemanticSearch => "Semantic search across past sessions",
            ReactTool::GrepContext => "Grep with surrounding context",
            ReactTool::Summarize => "Summarize output in 3-5 sentences",
            ReactTool::ExtractErrors => "Extract error messages from output",
            ReactTool::ExtractWarnings => "Extract warnings from output",
            ReactTool::ExtractMetrics => "Extract numeric metrics from output",
            ReactTool::ExtractPatterns => "Find patterns in data",
            ReactTool::Compare => "Compare two outputs or states",
            ReactTool::Correlate => "Find relationships in data",
            ReactTool::WebSummarize => "Summarize a web page",
            ReactTool::WebExtract => "Extract structured data from a web page",
            ReactTool::ExtractTables => "Extract tables from documents",
            ReactTool::DocQa => "Answer questions over document content",
            ReactTool::FindPatterns => "Find learned patterns from memory",
            ReactTool::CodeDiff => "Analyze git diff",
            ReactTool::CodeExplain => "Explain code structure",
            ReactTool::PlanNext => "Propose 2-3 next steps",
            ReactTool::NarrowFocus => "Narrow investigation scope",
            ReactTool::Branch => "Explore alternative approaches",
            ReactTool::Rethink => "Take completely new approach",
            ReactTool::Prioritize => "Rank options",
            ReactTool::ApplyFix => "Apply a fix or change",
            ReactTool::EditFile => "Edit an existing file",
            ReactTool::CreateFile => "Create a new file",
            ReactTool::RunCommand => "Run a command directly",
            ReactTool::Retry => "Retry failed operation",
            ReactTool::CodeExecute => "Execute code with confirmation",
            ReactTool::CheckGoal => "Verify if original goal achieved",
            ReactTool::VerifyFix => "Verify if fix was applied correctly",
            ReactTool::VerifySyntax => "Check syntax before applying",
            ReactTool::TestHypothesis => "Test a hypothesis",
            ReactTool::CodeTest => "Run tests",
            ReactTool::CodeLint => "Run linters",
            ReactTool::ShowFacts => "Show extracted facts",
            ReactTool::ShowHypotheses => "Show current hypotheses",
            ReactTool::ShowHistory => "Show session history",
            ReactTool::ShowContext => "Show all context",
            ReactTool::ShowPlan => "Show current plan",
            ReactTool::CompactSession => "Compact session history",
            ReactTool::Remember => "Store a fact in lifelong memory",
            ReactTool::Recall => "Retrieve from memory",
            ReactTool::Consolidate => "Summarize to long-term memory",
            ReactTool::SearchMemory => "Search lifelong memory",
            ReactTool::LearnPatterns => "Extract reusable patterns",
            ReactTool::ConcludeSuccess => "Problem solved - end session",
            ReactTool::ConcludeFail => "Cannot solve - end session",
            ReactTool::Escalate => "Need human assistance",
            ReactTool::Defer => "Defer task for later",
            ReactTool::AskClarification => "Need user clarification",
            ReactTool::AskConfirmation => "Need user confirmation",
            ReactTool::Explain => "Explain reasoning to user",
            ReactTool::SuggestAlternatives => "Offer options to user",
        }
    }

    pub fn category(&self) -> ToolCategory {
        match self {
            ReactTool::SuggestCommand
            | ReactTool::SuggestRead
            | ReactTool::SuggestGrep
            | ReactTool::SuggestRag
            | ReactTool::SuggestDiscovery
            | ReactTool::WebSearch
            | ReactTool::WebFetch
            | ReactTool::ReadPdf
            | ReactTool::ReadDocx
            | ReactTool::ReadXlsx
            | ReactTool::SemanticSearch
            | ReactTool::GrepContext => ToolCategory::Investigation,
            ReactTool::Summarize
            | ReactTool::ExtractErrors
            | ReactTool::ExtractWarnings
            | ReactTool::ExtractMetrics
            | ReactTool::ExtractPatterns
            | ReactTool::Compare
            | ReactTool::Correlate
            | ReactTool::WebSummarize
            | ReactTool::WebExtract
            | ReactTool::ExtractTables
            | ReactTool::DocQa
            | ReactTool::FindPatterns
            | ReactTool::CodeDiff
            | ReactTool::CodeExplain => ToolCategory::Analysis,
            ReactTool::PlanNext
            | ReactTool::NarrowFocus
            | ReactTool::Branch
            | ReactTool::Rethink
            | ReactTool::Prioritize => ToolCategory::Planning,
            ReactTool::ApplyFix
            | ReactTool::EditFile
            | ReactTool::CreateFile
            | ReactTool::RunCommand
            | ReactTool::Retry
            | ReactTool::CodeExecute => ToolCategory::Action,
            ReactTool::CheckGoal
            | ReactTool::VerifyFix
            | ReactTool::VerifySyntax
            | ReactTool::TestHypothesis
            | ReactTool::CodeTest
            | ReactTool::CodeLint => ToolCategory::Verification,
            ReactTool::ShowFacts
            | ReactTool::ShowHypotheses
            | ReactTool::ShowHistory
            | ReactTool::ShowContext
            | ReactTool::ShowPlan
            | ReactTool::CompactSession
            | ReactTool::Remember
            | ReactTool::Recall
            | ReactTool::Consolidate
            | ReactTool::SearchMemory
            | ReactTool::LearnPatterns => ToolCategory::Memory,
            ReactTool::ConcludeSuccess
            | ReactTool::ConcludeFail
            | ReactTool::Escalate
            | ReactTool::Defer => ToolCategory::Resolution,
            ReactTool::AskClarification
            | ReactTool::AskConfirmation
            | ReactTool::Explain
            | ReactTool::SuggestAlternatives => ToolCategory::Interaction,
        }
    }

    pub fn available_tools() -> &'static [ReactTool] {
        use ReactTool::*;
        &[
            SuggestCommand,
            SuggestRead,
            SuggestGrep,
            SuggestRag,
            SuggestDiscovery,
            WebSearch,
            WebFetch,
            ReadPdf,
            ReadDocx,
            ReadXlsx,
            SemanticSearch,
            GrepContext,
            Summarize,
            ExtractErrors,
            ExtractWarnings,
            ExtractMetrics,
            ExtractPatterns,
            Compare,
            Correlate,
            WebSummarize,
            WebExtract,
            ExtractTables,
            DocQa,
            FindPatterns,
            CodeDiff,
            CodeExplain,
            PlanNext,
            NarrowFocus,
            Branch,
            Rethink,
            Prioritize,
            ApplyFix,
            EditFile,
            CreateFile,
            RunCommand,
            Retry,
            CodeExecute,
            CheckGoal,
            VerifyFix,
            VerifySyntax,
            TestHypothesis,
            CodeTest,
            CodeLint,
            ShowFacts,
            ShowHypotheses,
            ShowHistory,
            ShowContext,
            ShowPlan,
            CompactSession,
            Remember,
            Recall,
            Consolidate,
            SearchMemory,
            LearnPatterns,
            ConcludeSuccess,
            ConcludeFail,
            Escalate,
            Defer,
            AskClarification,
            AskConfirmation,
            Explain,
            SuggestAlternatives,
        ]
    }
}

impl std::str::FromStr for ReactTool {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "suggest_command" => Ok(ReactTool::SuggestCommand),
            "suggest_read" => Ok(ReactTool::SuggestRead),
            "suggest_grep" => Ok(ReactTool::SuggestGrep),
            "suggest_rag" => Ok(ReactTool::SuggestRag),
            "suggest_discovery" => Ok(ReactTool::SuggestDiscovery),
            "web_search" => Ok(ReactTool::WebSearch),
            "web_fetch" => Ok(ReactTool::WebFetch),
            "read_pdf" => Ok(ReactTool::ReadPdf),
            "read_docx" => Ok(ReactTool::ReadDocx),
            "read_xlsx" => Ok(ReactTool::ReadXlsx),
            "semantic_search" => Ok(ReactTool::SemanticSearch),
            "grep_context" => Ok(ReactTool::GrepContext),
            "summarize" => Ok(ReactTool::Summarize),
            "extract_errors" => Ok(ReactTool::ExtractErrors),
            "extract_warnings" => Ok(ReactTool::ExtractWarnings),
            "extract_metrics" => Ok(ReactTool::ExtractMetrics),
            "extract_patterns" => Ok(ReactTool::ExtractPatterns),
            "compare" => Ok(ReactTool::Compare),
            "correlate" => Ok(ReactTool::Correlate),
            "web_summarize" => Ok(ReactTool::WebSummarize),
            "web_extract" => Ok(ReactTool::WebExtract),
            "extract_tables" => Ok(ReactTool::ExtractTables),
            "doc_qa" => Ok(ReactTool::DocQa),
            "find_patterns" => Ok(ReactTool::FindPatterns),
            "code_diff" => Ok(ReactTool::CodeDiff),
            "code_explain" => Ok(ReactTool::CodeExplain),
            "plan_next" => Ok(ReactTool::PlanNext),
            "narrow_focus" => Ok(ReactTool::NarrowFocus),
            "branch" => Ok(ReactTool::Branch),
            "rethink" => Ok(ReactTool::Rethink),
            "prioritize" => Ok(ReactTool::Prioritize),
            "apply_fix" => Ok(ReactTool::ApplyFix),
            "edit_file" => Ok(ReactTool::EditFile),
            "create_file" => Ok(ReactTool::CreateFile),
            "run_command" => Ok(ReactTool::RunCommand),
            "retry" => Ok(ReactTool::Retry),
            "code_execute" => Ok(ReactTool::CodeExecute),
            "check_goal" => Ok(ReactTool::CheckGoal),
            "verify_fix" => Ok(ReactTool::VerifyFix),
            "verify_syntax" => Ok(ReactTool::VerifySyntax),
            "test_hypothesis" => Ok(ReactTool::TestHypothesis),
            "code_test" => Ok(ReactTool::CodeTest),
            "code_lint" => Ok(ReactTool::CodeLint),
            "show_facts" => Ok(ReactTool::ShowFacts),
            "show_hypotheses" => Ok(ReactTool::ShowHypotheses),
            "show_history" => Ok(ReactTool::ShowHistory),
            "show_context" => Ok(ReactTool::ShowContext),
            "show_plan" => Ok(ReactTool::ShowPlan),
            "compact_session" => Ok(ReactTool::CompactSession),
            "remember" => Ok(ReactTool::Remember),
            "recall" => Ok(ReactTool::Recall),
            "consolidate" => Ok(ReactTool::Consolidate),
            "search_memory" => Ok(ReactTool::SearchMemory),
            "learn_patterns" => Ok(ReactTool::LearnPatterns),
            "conclude_success" => Ok(ReactTool::ConcludeSuccess),
            "conclude_fail" => Ok(ReactTool::ConcludeFail),
            "escalate" => Ok(ReactTool::Escalate),
            "defer" => Ok(ReactTool::Defer),
            "ask_clarification" => Ok(ReactTool::AskClarification),
            "ask_confirmation" => Ok(ReactTool::AskConfirmation),
            "explain" => Ok(ReactTool::Explain),
            "suggest_alternatives" => Ok(ReactTool::SuggestAlternatives),
            _ => Err(format!("Unknown tool: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ToolCategory {
    Investigation,
    Analysis,
    Planning,
    Action,
    Verification,
    Memory,
    Resolution,
    Interaction,
}

impl ToolCategory {
    pub fn name(&self) -> &'static str {
        match self {
            ToolCategory::Investigation => "Investigation",
            ToolCategory::Analysis => "Analysis",
            ToolCategory::Planning => "Planning",
            ToolCategory::Action => "Action",
            ToolCategory::Verification => "Verification",
            ToolCategory::Memory => "Memory",
            ToolCategory::Resolution => "Resolution",
            ToolCategory::Interaction => "Interaction",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDecision {
    pub tool: ReactTool,
    pub justification: String,
    pub context_needed: String,
    pub confidence: f32,
}

impl ToolDecision {
    pub fn new(tool: ReactTool, justification: String) -> Self {
        Self {
            tool,
            justification,
            context_needed: String::new(),
            confidence: 1.0,
        }
    }

    pub fn with_context(mut self, context: String) -> Self {
        self.context_needed = context;
        self
    }

    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool: ReactTool,
    pub output: String,
    pub commands: Vec<String>,
    pub facts_extracted: Vec<Fact>,
    pub hypotheses_updated: Vec<Hypothesis>,
    pub next_tool_suggestion: Option<ReactTool>,
    pub should_continue: bool,
    pub should_ask_user: bool,
    pub user_question: Option<String>,
}

impl ToolResult {
    pub fn new(tool: ReactTool) -> Self {
        Self {
            tool,
            output: String::new(),
            commands: Vec::new(),
            facts_extracted: Vec::new(),
            hypotheses_updated: Vec::new(),
            next_tool_suggestion: None,
            should_continue: true,
            should_ask_user: false,
            user_question: None,
        }
    }

    pub fn with_output(mut self, output: String) -> Self {
        self.output = output;
        self
    }

    pub fn with_commands(mut self, commands: Vec<String>) -> Self {
        self.commands = commands;
        self
    }

    pub fn with_facts(mut self, facts: Vec<Fact>) -> Self {
        self.facts_extracted = facts;
        self
    }

    pub fn with_hypotheses(mut self, hypotheses: Vec<Hypothesis>) -> Self {
        self.hypotheses_updated = hypotheses;
        self
    }

    pub fn with_next_tool(mut self, tool: ReactTool) -> Self {
        self.next_tool_suggestion = Some(tool);
        self
    }

    pub fn conclude(self) -> Self {
        Self {
            should_continue: false,
            ..self
        }
    }

    pub fn ask_user(mut self, question: String) -> Self {
        self.should_ask_user = true;
        self.user_question = Some(question);
        self
    }
}

impl ReactSession {
    pub fn new(query: String, neurosymbolic_enabled: bool) -> Self {
        let now = Utc::now();
        let session_id = uuid::Uuid::new_v4().to_string();
        Self {
            id: session_id.clone(),
            query: query.clone(),
            created_at: now,
            updated_at: now,
            status: ReactStatus::Running,
            steps: Vec::new(),
            context: HashMap::new(),
            memory: SessionMemory::new(session_id, query.clone()),
            intent: None,
            compacted_summary: None,
            compacted_history_at: None,
            neurosymbolic_enabled,
        }
    }

    pub fn add_step(&mut self, step: ReactStep) {
        self.updated_at = Utc::now();
        self.steps.push(step);
    }

    pub fn current_step(&self) -> Option<&ReactStep> {
        self.steps.last()
    }

    pub fn complete(&mut self) {
        self.status = ReactStatus::Completed;
        self.updated_at = Utc::now();
    }

    pub fn abort(&mut self) {
        self.status = ReactStatus::Aborted;
        self.updated_at = Utc::now();
    }

    pub fn fail(&mut self) {
        self.status = ReactStatus::Failed;
        self.updated_at = Utc::now();
    }

    pub fn set_intent(&mut self, intent: QueryIntent) {
        self.intent = Some(intent);
    }

    pub fn set_compacted_summary(&mut self, summary: String) {
        self.compacted_summary = Some(summary);
        self.compacted_history_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }
}

impl ReactStep {
    pub fn new(session_id: String, step_type: ReactStepType, content: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id,
            step_type,
            content,
            created_at: Utc::now(),
            status: ReactStepStatus::Pending,
            commands: Vec::new(),
            observations: Vec::new(),
            reasoning: None,
        }
    }

    pub fn with_reasoning(mut self, reasoning: String) -> Self {
        self.reasoning = Some(reasoning);
        self
    }

    pub fn add_command(&mut self, command: ProposedCommand) {
        self.commands.push(command);
    }

    pub fn add_observation(&mut self, observation: String) {
        self.observations.push(observation);
    }

    pub fn start(&mut self) {
        self.status = ReactStepStatus::InProgress;
    }

    pub fn complete(&mut self) {
        self.status = ReactStepStatus::Completed;
    }

    pub fn fail(&mut self) {
        self.status = ReactStepStatus::Failed;
    }

    pub fn skip(&mut self) {
        self.status = ReactStepStatus::Skipped;
    }
}

impl ProposedCommand {
    pub fn new(command: String, description: String, reasoning: String) -> Self {
        let safety = classify_command(&command);
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            command,
            description,
            reasoning,
            safety,
            approved: None,
            executed: false,
            exit_code: None,
            stdout: None,
            stderr: None,
        }
    }

    pub fn approve(&mut self) {
        self.approved = Some(true);
    }

    pub fn reject(&mut self) {
        self.approved = Some(false);
    }

    pub fn execute(&mut self, exit_code: i32, stdout: String, stderr: String) {
        self.executed = true;
        self.exit_code = Some(exit_code);
        self.stdout = Some(stdout);
        self.stderr = Some(stderr);
    }
}

impl ReactContext {
    pub fn new(max_iterations: u32) -> Self {
        Self {
            current_step: 0,
            iteration_count: 0,
            max_iterations,
            available_tools: vec![
                "read".to_string(),
                "grep".to_string(),
                "fd".to_string(),
                "rag".to_string(),
                "web_search".to_string(),
                "web_fetch".to_string(),
                "web_summarize".to_string(),
                "web_extract".to_string(),
                "read_pdf".to_string(),
                "read_docx".to_string(),
                "read_xlsx".to_string(),
                "extract_tables".to_string(),
                "doc_qa".to_string(),
                "semantic_search".to_string(),
                "grep_context".to_string(),
                "search_memory".to_string(),
                "find_patterns".to_string(),
                "remember".to_string(),
                "recall".to_string(),
                "consolidate".to_string(),
                "learn_patterns".to_string(),
                "code_execute".to_string(),
                "code_test".to_string(),
                "code_lint".to_string(),
                "code_diff".to_string(),
                "code_explain".to_string(),
                "sed".to_string(),
                "perl".to_string(),
                "awk".to_string(),
                "apply_patch".to_string(),
                "write".to_string(),
                "remove".to_string(),
                "update".to_string(),
                "replace_block".to_string(),
                "shell".to_string(),
                "pkg".to_string(),
                "svc".to_string(),
                "git".to_string(),
                "build".to_string(),
                "test".to_string(),
            ],
            user_preferences: HashMap::new(),
        }
    }

    pub fn next_step(&mut self) {
        self.current_step += 1;
    }

    pub fn increment_iteration(&mut self) {
        self.iteration_count += 1;
    }

    pub fn should_continue(&self) -> bool {
        self.iteration_count < self.max_iterations
    }

    pub fn add_tool(&mut self, tool: String) {
        self.available_tools.push(tool);
    }

    pub fn set_preference(&mut self, key: String, value: String) {
        self.user_preferences.insert(key, value);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandSafety {
    ReadOnly,
    Write,
    Destructive,
}

pub fn classify_command(command: &str) -> CommandSafety {
    let mut cmd = command.trim().to_ascii_lowercase();
    if let Some(rest) = cmd.strip_prefix("shell ") {
        cmd = rest.trim().to_string();
    }
    if cmd.is_empty() {
        return CommandSafety::ReadOnly;
    }

    let destructive = [
        "rm ",
        "rmdir",
        "dd ",
        "mkfs",
        "fdisk",
        "parted",
        "wipefs",
        "sudo systemctl start",
        "sudo systemctl stop",
        "sudo systemctl restart",
        "systemctl start",
        "systemctl stop",
        "systemctl restart",
        "svc start",
        "svc stop",
        "svc restart",
        "kill ",
        "pkill ",
        "killall",
        "reboot",
        "shutdown",
        "halt",
        "poweroff",
        "git push",
        "git force",
        "remove ",
        "delete ",
    ];
    for pattern in destructive {
        if cmd.contains(pattern) {
            return CommandSafety::Destructive;
        }
    }

    let write = [
        "sed -i",
        "perl -i",
        "awk -i",
        "tee ",
        ">",
        ">>",
        "mv ",
        "cp ",
        "mkdir",
        "touch",
        "chmod",
        "chown",
        "truncate",
        "write ",
        "update ",
        "replace_block",
        "apply_patch",
        "pkg install",
        "pkg remove",
        "pkg upgrade",
        "pkg update",
        "git add",
        "git commit",
        "git checkout",
        "git merge",
        "git pull",
        "svc enable",
        "svc disable",
        "systemctl enable",
        "systemctl disable",
    ];
    for pattern in write {
        if cmd.contains(pattern) {
            return CommandSafety::Write;
        }
    }

    CommandSafety::ReadOnly
}
