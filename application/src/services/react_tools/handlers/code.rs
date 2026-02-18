use anyhow::Result;
use async_trait::async_trait;
use domain::entities::react::{ReactTool, ToolCategory, ToolResult};
use std::path::Path;
use std::sync::Arc;

use crate::services::react_context_retriever::RetrievedContext;
use crate::services::react_tools::ReactToolHandler;

pub struct CodeExecuteHandler;

#[async_trait]
impl ReactToolHandler for CodeExecuteHandler {
    fn name(&self) -> &str {
        "code_execute"
    }

    fn description(&self) -> &str {
        "Execute code with confirmation"
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Action
    }

    fn requires_output(&self) -> bool {
        false
    }

    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let command = infer_run_command(context);
        Ok(ToolResult::new(ReactTool::CodeExecute)
            .with_commands(vec![command])
            .with_next_tool(ReactTool::Summarize))
    }

    fn get_prompt(&self, _context: &RetrievedContext) -> String {
        "Suggest a safe command to execute the relevant code (prefer project defaults).".to_string()
    }
}

pub struct CodeTestHandler;

#[async_trait]
impl ReactToolHandler for CodeTestHandler {
    fn name(&self) -> &str {
        "code_test"
    }

    fn description(&self) -> &str {
        "Run tests"
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Verification
    }

    fn requires_output(&self) -> bool {
        false
    }

    async fn execute(&self, _context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        Ok(ToolResult::new(ReactTool::CodeTest)
            .with_commands(vec![infer_test_command()])
            .with_next_tool(ReactTool::Summarize))
    }

    fn get_prompt(&self, _context: &RetrievedContext) -> String {
        "Propose the most relevant test command for this project.".to_string()
    }
}

pub struct CodeLintHandler;

#[async_trait]
impl ReactToolHandler for CodeLintHandler {
    fn name(&self) -> &str {
        "code_lint"
    }

    fn description(&self) -> &str {
        "Run linters"
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Verification
    }

    fn requires_output(&self) -> bool {
        false
    }

    async fn execute(&self, _context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        Ok(ToolResult::new(ReactTool::CodeLint)
            .with_commands(vec![infer_lint_command()])
            .with_next_tool(ReactTool::Summarize))
    }

    fn get_prompt(&self, _context: &RetrievedContext) -> String {
        "Propose the most relevant lint command for this project.".to_string()
    }
}

pub struct CodeDiffHandler;

#[async_trait]
impl ReactToolHandler for CodeDiffHandler {
    fn name(&self) -> &str {
        "code_diff"
    }

    fn description(&self) -> &str {
        "Analyze git diff"
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Analysis
    }

    fn requires_output(&self) -> bool {
        false
    }

    async fn execute(&self, _context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        Ok(ToolResult::new(ReactTool::CodeDiff)
            .with_commands(vec!["git diff".to_string()])
            .with_next_tool(ReactTool::Summarize))
    }

    fn get_prompt(&self, _context: &RetrievedContext) -> String {
        "Propose a git diff command to review recent changes.".to_string()
    }
}

pub struct CodeExplainHandler;

#[async_trait]
impl ReactToolHandler for CodeExplainHandler {
    fn name(&self) -> &str {
        "code_explain"
    }

    fn description(&self) -> &str {
        "Explain code structure"
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Analysis
    }

    fn requires_output(&self) -> bool {
        false
    }

    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let command = infer_explain_command(context);
        Ok(ToolResult::new(ReactTool::CodeExplain)
            .with_commands(vec![command])
            .with_next_tool(ReactTool::Summarize))
    }

    fn get_prompt(&self, _context: &RetrievedContext) -> String {
        "Propose the best command to inspect or summarize relevant code.".to_string()
    }
}

pub fn build_code_handlers() -> Vec<(ReactTool, Arc<dyn ReactToolHandler>)> {
    vec![
        (ReactTool::CodeExecute, Arc::new(CodeExecuteHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::CodeTest, Arc::new(CodeTestHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::CodeLint, Arc::new(CodeLintHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::CodeDiff, Arc::new(CodeDiffHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::CodeExplain, Arc::new(CodeExplainHandler) as Arc<dyn ReactToolHandler>),
    ]
}

fn infer_run_command(context: &RetrievedContext) -> String {
    if Path::new("Cargo.toml").exists() {
        return "shell cargo run".to_string();
    }
    if Path::new("package.json").exists() {
        return "shell npm run start".to_string();
    }
    if Path::new("main.py").exists() {
        return "shell python main.py".to_string();
    }

    let fallback = context.goal.split_whitespace().take(4).collect::<Vec<_>>().join(" ");
    if fallback.is_empty() {
        "shell ls".to_string()
    } else {
        format!("rag \"{}\" 6", fallback)
    }
}

fn infer_test_command() -> String {
    if Path::new("Cargo.toml").exists() {
        "test".to_string()
    } else if Path::new("package.json").exists() {
        "shell npm test".to_string()
    } else if Path::new("pytest.ini").exists() || Path::new("pyproject.toml").exists() {
        "shell pytest".to_string()
    } else {
        "shell ls".to_string()
    }
}

fn infer_lint_command() -> String {
    if Path::new("Cargo.toml").exists() {
        "build clippy".to_string()
    } else if Path::new("package.json").exists() {
        "shell npm run lint".to_string()
    } else if Path::new("pyproject.toml").exists() {
        "shell ruff check .".to_string()
    } else {
        "shell ls".to_string()
    }
}

fn infer_explain_command(context: &RetrievedContext) -> String {
    if context.goal.to_lowercase().contains("config") {
        if Path::new("Cargo.toml").exists() {
            return "read Cargo.toml".to_string();
        }
        if Path::new("README.md").exists() {
            return "read README.md".to_string();
        }
    }

    let query = context.goal.replace('"', "\\\"");
    format!("rag \"{}\" 8", query)
}
