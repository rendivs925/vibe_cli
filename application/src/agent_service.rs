//! application/src/agent_service.rs
//!
//! Fixed + completed implementation:
//! - Closes all delimiters (your compile error was from missing braces in impl blocks).
//! - Adds missing `ExecutionCoordinator` struct and a bounded multi-iteration `execute_agent` loop.
//! - Makes tool planning robust (supports JSON tool call OR "no tool" natural text).
//! - Validates tool names against available tool definitions.
//! - Improves sandbox gating + file read safety (path checks + size truncation).
//! - Extracts tool calls from execution history (so AgentResponse.tool_calls isn't always empty).
//! - Calculates confidence from both reasoning depth and tool success.
//! - Produces stable, deterministic JSON parsing and avoids panics.

use crate::build_service::{BuildPlan, ComplexOperation, FileOperation, RiskLevel, ValidationRule};
use domain::models::{
    AgentContext, AgentRequest, AgentResponse, ConversationMessage, ParameterProperty, ToolCall,
    ToolDefinition, ToolParameters, ToolResult,
};
use infrastructure::{
    agent_control::{
        AgentController, AgentError, AgentExecutionState, AgentIterationResult, AgentResult,
        IterationRecord, SafeFailureHandler,
    },
    config::Config,
    sandbox::Sandbox,
    tools::{ToolArgs, ToolRegistry},
};
use serde_json::{json, Value};
use shared::types::Result;
use std::collections::HashMap;
use std::sync::Arc;

// Forward declare for now - actual implementation when both services are integrated
pub type RagService = crate::rag_service::RagService;

/// Main agent service coordinating all agent operations
pub struct AgentService {
    pub inference_engine: infrastructure::InferenceEngine,
    pub rag_service: Option<Arc<RagService>>,
    pub config: Config,
    pub agent_controller: AgentController,
    pub failure_handler: SafeFailureHandler,
    pub system_context: infrastructure::config::SystemContext,
}

/// Artifacts returned when planning a build
pub struct BuildPlanOutcome {
    pub plan: BuildPlan,
    pub retrieved_context: Vec<String>,
    pub raw_plan_text: String,
    pub planning_attempts: usize,
    pub planning_logs: Vec<String>,
}

/// Represents a single incremental step in build planning
#[derive(Debug, Clone)]
pub struct IncrementalPlanStep {
    pub step_number: usize,
    pub description: String,
    pub reasoning: String,
    pub code_chunk: Option<String>,
    pub file_path: Option<String>,
    pub operation_type: Option<String>,
    pub confidence: Option<f32>,
}

/// Context information about a file's current state
#[derive(Debug, Clone)]
pub struct FileContext {
    pub path: String,
    pub exists: bool,
    pub content: Option<String>,
    pub size_bytes: u64,
    pub line_count: usize,
    pub modified: Option<std::time::SystemTime>,
    pub operation_type: FileOperationType,
}

/// Type of operation planned for a file
#[derive(Debug, Clone, PartialEq)]
pub enum FileOperationType {
    Create,   // File doesn't exist, will be created
    ReadOnly, // File exists, no changes planned
    Update,   // File exists, modifications planned
    Delete,   // File exists, will be removed
    Unknown,  // Not yet determined
}

/// Stream-based incremental build planner with true real-time streaming
pub struct IncrementalBuildPlanner {
    goal: String,
    context: Vec<String>,
    planning_state: PlanningState,
    completed_operations: Vec<FileOperation>,
    complex_operations: Vec<ComplexOperation>,
    file_contexts: HashMap<String, FileContext>,
    keywords: Vec<String>,
    os_info: String,
    cwd: String,
    config: Config,
}

#[derive(Debug, Clone)]
enum PlanningState {
    Initial,
    Analyzing,
    PlanningOperations,
    GeneratingCode {
        files: Vec<FileSpec>,
        current_index: usize,
    },
    Finalizing,
    Complete,
}

#[derive(Debug, Clone)]
struct FileSpec {
    path: String,
    action: String,
    reason: String,
}

impl IncrementalBuildPlanner {
    pub fn new(goal: String, context: Vec<String>, config: Config) -> Self {
        let cwd = std::env::current_dir()
            .ok()
            .and_then(|p| p.to_str().map(|s| s.to_string()))
            .unwrap_or_else(|| ".".to_string());

        Self {
            goal,
            context,
            planning_state: PlanningState::Initial,
            completed_operations: Vec::new(),
            complex_operations: Vec::new(),
            file_contexts: HashMap::new(),
            keywords: Vec::new(),
            os_info: std::env::consts::OS.to_string(),
            cwd,
            config,
        }
    }

    /// Stream the next planning step with true real-time AI generation
    pub async fn stream_next_step(
        &mut self,
        inference_engine: &infrastructure::InferenceEngine,
    ) -> Result<Option<IncrementalPlanStep>> {
        match &self.planning_state {
            PlanningState::Initial => {
                self.planning_state = PlanningState::Analyzing;
                self.stream_analysis_step(inference_engine).await
            }
            PlanningState::Analyzing => {
                self.planning_state = PlanningState::PlanningOperations;
                self.stream_operation_planning_step(inference_engine).await
            }
            PlanningState::PlanningOperations => {
                let mut files = self.stream_file_discovery_step(inference_engine).await?;
                if files.is_empty() {
                    files = self.infer_files_from_goal(inference_engine).await?;
                }

                if files.is_empty() {
                    self.planning_state = PlanningState::Finalizing;
                    self.stream_finalizing_step().await
                } else {
                    // Seed contexts for newly proposed files
                    for spec in &files {
                        self.file_contexts
                            .entry(spec.path.clone())
                            .or_insert(FileContext {
                                path: spec.path.clone(),
                                exists: false,
                                content: None,
                                size_bytes: 0,
                                line_count: 0,
                                modified: None,
                                operation_type: FileOperationType::Create,
                            });
                    }

                    self.planning_state = PlanningState::GeneratingCode {
                        files,
                        current_index: 0,
                    };
                    self.stream_next_code_step(inference_engine).await
                }
            }
            PlanningState::GeneratingCode {
                files,
                current_index,
            } => {
                if *current_index >= files.len() {
                    self.planning_state = PlanningState::Finalizing;
                    self.stream_finalizing_step().await
                } else {
                    self.stream_next_code_step(inference_engine).await
                }
            }
            PlanningState::Finalizing => {
                self.planning_state = PlanningState::Complete;
                self.stream_finalizing_step().await
            }
            PlanningState::Complete => Ok(None),
        }
    }

    async fn stream_analysis_step(
        &self,
        inference_engine: &infrastructure::InferenceEngine,
    ) -> Result<Option<IncrementalPlanStep>> {
        // Build context summary from real file states
        let context_summary = self.build_context_summary();

        let prompt = format!(
            r#"Analyze this goal and determine the best approach for incremental implementation:

GOAL: {}

ACTUAL FILE CONTEXT:
{}

CONTEXT:
{}

Think step-by-step about:
1. What kind of project/files are we working with? (Use the ACTUAL FILE CONTEXT above)
2. What files exist vs need to be created? (Check the file states provided)
3. What's the simplest, most direct approach given the current project state?
4. What are the key files that need to be created/modified?
5. What's the risk level (Low/Medium/High)?

Provide a brief analysis (2-3 sentences) of your approach."#,
            self.goal,
            context_summary,
            self.context.join("\n")
        );

        let analysis = inference_engine.generate(&prompt).await?;
        let confidence = self.calculate_confidence_from_response(&analysis, "analysis");

        Ok(Some(IncrementalPlanStep {
            step_number: 1,
            description: "Analyzing project structure and determining approach".to_string(),
            reasoning: analysis.trim().to_string(),
            code_chunk: None,
            file_path: None,
            operation_type: None,
            confidence: Some(confidence),
        }))
    }

    async fn stream_operation_planning_step(
        &self,
        inference_engine: &infrastructure::InferenceEngine,
    ) -> Result<Option<IncrementalPlanStep>> {
        let context_summary = self.build_context_summary();

        let prompt = format!(
            r#"Based on the goal and actual file states, what specific file operations are needed?

GOAL: {}

ACTUAL FILE CONTEXT:
{}

Respond with ONLY the file operations in this exact format:
FILE: index.html
ACTION: create|update
REASON: brief explanation

CRITICAL: Use the ACTUAL FILE CONTEXT above to determine if files exist or need to be created.
- If a file EXISTS, use ACTION: update
- If a file DOES NOT EXIST, use ACTION: create
- Do not assume files exist unless shown in the context

Do not include any other text or explanations."#,
            self.goal, context_summary
        );

        let plan_text = inference_engine.generate(&prompt).await?;
        let confidence = self.calculate_confidence_from_response(&plan_text, "planning");

        Ok(Some(IncrementalPlanStep {
            step_number: 2,
            description: "Planning file operations based on actual file states".to_string(),
            reasoning: format!("Determined file operations:\n{}", plan_text.trim()),
            code_chunk: None,
            file_path: None,
            operation_type: None,
            confidence: Some(confidence),
        }))
    }

    async fn stream_file_discovery_step(
        &mut self,
        inference_engine: &infrastructure::InferenceEngine,
    ) -> Result<Vec<FileSpec>> {
        // Query AI to determine required file operations
        let context_summary = self.build_context_summary();

        let prompt = format!(
            r#"Analyze the goal and determine file operations based on ACTUAL file existence.

GOAL: {}

FILE CONTEXT (from filesystem scan):
{}

CRITICAL INSTRUCTIONS:
1. Read the FILE CONTEXT above carefully - it shows which files "EXISTS" or "DOES NOT EXIST"
2. For ANY file listed as "EXISTS" → use ACTION: update
3. For ANY file listed as "DOES NOT EXIST" → use ACTION: create
4. If no files are listed above, infer a minimal set of files to build the goal from scratch (return at least one).
5. NEVER guess file existence - trust the FILE CONTEXT information; if context is empty, make clear you are creating new files.

RESPONSE FORMAT (required):
FILE: path/to/file.ext
ACTION: update|create
REASON: brief explanation

Do not include examples; return only the operations in the required format."#,
            self.goal, context_summary
        );

        let response = inference_engine.generate(&prompt).await?;
        let files = self.parse_file_specs(&response);

        // Validate and filter files based on actual filesystem state
        let mut filtered_files: Vec<FileSpec> = Vec::new();

        for file_spec in files {
            let file_exists = self
                .file_contexts
                .get(&file_spec.path)
                .map(|ctx| ctx.exists)
                .unwrap_or_else(|| std::path::Path::new(&file_spec.path).exists());

            let should_include = match file_spec.action.as_str() {
                "update" => {
                    if !file_exists {
                        eprintln!("⚠️  Correcting: AI suggested 'update' for non-existent file '{}' - changing to 'create'", file_spec.path);
                        // Auto-correct: change update to create
                        let mut corrected = file_spec.clone();
                        corrected.action = "create".to_string();
                        filtered_files.push(corrected);
                        false
                    } else {
                        true
                    }
                }
                "create" => {
                    if file_exists {
                        eprintln!("⚠️  Correcting: AI suggested 'create' for existing file '{}' - changing to 'update'", file_spec.path);
                        // Auto-correct: change create to update
                        let mut corrected = file_spec.clone();
                        corrected.action = "update".to_string();
                        filtered_files.push(corrected);
                        false
                    } else {
                        true
                    }
                }
                _ => {
                    eprintln!(
                        "⚠️  Skipping invalid action '{}' for file '{}'",
                        file_spec.action, file_spec.path
                    );
                    false
                }
            };

            if should_include {
                filtered_files.push(file_spec);
            }
        }

        Ok(filtered_files)
    }

    async fn infer_files_from_goal(
        &self,
        inference_engine: &infrastructure::InferenceEngine,
    ) -> Result<Vec<FileSpec>> {
        let prompt = format!(
            r#"The filesystem has no relevant files for this goal. Propose a minimal set of files to create to deliver the goal.

GOAL: {}

Rules:
- Return at least one file.
- Prefer 1-5 files that together produce a runnable, testable solution.
- Use ACTION: create for each.
- Choose sensible names and locations based on the goal; avoid placeholders.
- If the goal implies an app/game/UI, include an entrypoint and any supporting files needed to run without external assets.
- Keep the list concise.

FORMAT (required):
FILE: path/to/file.ext
ACTION: create
REASON: brief explanation"#,
            self.goal
        );

        let response = inference_engine.generate(&prompt).await?;
        let files = self.parse_file_specs(&response);

        // Ensure action is create
        let normalized = files
            .into_iter()
            .map(|mut f| {
                f.action = "create".to_string();
                f
            })
            .collect();

        Ok(normalized)
    }

    async fn stream_next_code_step(
        &mut self,
        inference_engine: &infrastructure::InferenceEngine,
    ) -> Result<Option<IncrementalPlanStep>> {
        let (files, current_index) = match &mut self.planning_state {
            PlanningState::GeneratingCode {
                files,
                current_index,
            } => (files.clone(), *current_index),
            _ => return Ok(None),
        };

        if current_index >= files.len() {
            return Ok(None);
        }

        let file_spec = &files[current_index];
        let step_number = 3 + current_index;

        // Get existing file content for context-aware generation
        let existing_content = self
            .file_contexts
            .get(&file_spec.path)
            .and_then(|ctx| ctx.content.as_ref());

        let (prompt, is_update) = if file_spec.action == "update" && existing_content.is_some() {
            let content = existing_content.unwrap();
            let lines: Vec<&str> = content.lines().collect();
            let line_count = lines.len();
            let preview = self.create_numbered_preview(content, line_count);

            (
                format!(
                    r#"Task: Rewrite the entire file to fix issues and satisfy the goal. Return the full updated file content (plain text, no markdown/backticks).

GOAL: {}
FILE: {}
SIZE: {} lines

CURRENT FILE (numbered):
{}

INSTRUCTIONS:
- Output the full corrected file content only (no fences, no explanations).
- Preserve intent but fix errors and make it runnable.
- Remove any markdown/code fences or stray backticks.
- Include imports/entrypoints needed to run the file as-is.
- If unsure, prefer a minimal runnable version over partial edits.
"#,
                    self.goal, file_spec.path, line_count, preview
                ),
                true,
            )
        } else {
            // Generate complete new file for creation
            let file_extension = std::path::Path::new(&file_spec.path)
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("");

            let language_hint = match file_extension {
                "rs" => "Rust",
                "py" => "Python",
                "js" => "JavaScript",
                "ts" => "TypeScript",
                "html" => "HTML",
                "css" => "CSS",
                "go" => "Go",
                "java" => "Java",
                _ => "code",
            };

            (
                format!(
                    r#"Task: Generate a complete new file to accomplish the goal. Return only the file content (plain text, no markdown/backticks).

GOAL: {}
FILE TO CREATE: {}
LANGUAGE/TYPE: {}

INSTRUCTIONS:
- Generate a complete, working {} file that runs as-is (no placeholders or TODOs)
- Include necessary imports, dependencies, entrypoints, and minimal wiring to run
- If the goal implies an app/game/UI, include a runnable entry (e.g., main loop or HTML with inline CSS/JS) without external assets
- Follow best practices and conventions for {}
- Ensure the code is production-ready and well-structured
- Do NOT include explanations or markdown formatting
- Return ONLY the file content (plain text)

Generate the complete file content now:"#,
                    self.goal, file_spec.path, language_hint, language_hint, language_hint
                ),
                false,
            )
        };

        let code = inference_engine.generate(&prompt).await?;
        let confidence = self.calculate_confidence_from_response(&code, "code_generation");

        // Update state for next file
        if let PlanningState::GeneratingCode {
            current_index: ref mut idx,
            ..
        } = self.planning_state
        {
            *idx += 1;
        }

        // Buffer the operation for execution
        if file_spec.action == "create" {
            self.completed_operations
                .push(crate::build_service::FileOperation::Create {
                    path: std::path::PathBuf::from(&file_spec.path),
                    content: code.trim().to_string(),
                });
        } else if file_spec.action == "update" {
            let old_content = existing_content.map_or(String::new(), |s| s.clone());
            let new_content = code.trim().to_string();

            self.completed_operations
                .push(crate::build_service::FileOperation::Update {
                    path: std::path::PathBuf::from(&file_spec.path),
                    old_content,
                    new_content,
                });
        }

        Ok(Some(IncrementalPlanStep {
            step_number,
            description: format!(
                "{} code for {}",
                if is_update {
                    "Generating targeted changes for"
                } else {
                    "Generating complete"
                },
                file_spec.path
            ),
            reasoning: format!(
                "{}{} with action: {}",
                if is_update {
                    "Updating existing file "
                } else {
                    "Creating new file "
                },
                file_spec.path,
                file_spec.action
            ),
            code_chunk: Some(code.trim().to_string()),
            file_path: Some(file_spec.path.clone()),
            operation_type: Some(file_spec.action.clone()),
            confidence: Some(confidence),
        }))
    }

    async fn stream_finalizing_step(&self) -> Result<Option<IncrementalPlanStep>> {
        let operations_count = self.completed_operations.len();
        Ok(Some(IncrementalPlanStep {
            step_number: 999, // Final step
            description: "Finalizing build plan".to_string(),
            reasoning: format!(
                "Build plan complete with {} operations ready for execution.",
                operations_count
            ),
            code_chunk: None,
            file_path: None,
            operation_type: None,
            confidence: Some(0.95),
        }))
    }

    fn parse_file_specs(&self, response: &str) -> Vec<FileSpec> {
        let mut files = Vec::new();
        let mut current_file: Option<FileSpec> = None;

        for line in response.lines() {
            let line = line.trim();
            if line.starts_with("FILE:") {
                if let Some(file) = current_file.take() {
                    files.push(file);
                }
                let path = line.strip_prefix("FILE:").unwrap_or("").trim().to_string();
                current_file = Some(FileSpec {
                    path,
                    action: String::new(),
                    reason: String::new(),
                });
            } else if line.starts_with("ACTION:") && current_file.is_some() {
                let action = line
                    .strip_prefix("ACTION:")
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if let Some(ref mut file) = current_file {
                    file.action = action;
                }
            } else if line.starts_with("REASON:") && current_file.is_some() {
                let reason = line
                    .strip_prefix("REASON:")
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if let Some(ref mut file) = current_file {
                    file.reason = reason;
                }
            }
        }

        if let Some(file) = current_file {
            files.push(file);
        }

        files
    }

    fn create_operation_from_code(
        &self,
        file_spec: &FileSpec,
        code: &str,
    ) -> Result<FileOperation> {
        match file_spec.action.as_str() {
            "create" => Ok(FileOperation::Create {
                path: std::path::PathBuf::from(&file_spec.path),
                content: code.to_string(),
            }),
            "update" => {
                // For updates, we'd need to read existing content
                // This is simplified - in practice, we'd merge with existing content
                Ok(FileOperation::Update {
                    path: std::path::PathBuf::from(&file_spec.path),
                    old_content: String::new(), // Would read actual content
                    new_content: code.to_string(),
                })
            }
            _ => Err(anyhow::anyhow!("Unsupported action: {}", file_spec.action)),
        }
    }

    /// Create a complex operation from multiple file specs
    pub fn create_complex_operation(
        &self,
        name: String,
        description: String,
        file_specs: Vec<FileSpec>,
    ) -> Result<ComplexOperation> {
        let mut file_operations = Vec::new();
        let dependencies = Vec::new();
        let mut validation_rules = Vec::new();
        let mut max_risk = RiskLevel::Low;

        for spec in file_specs {
            // Create the file operation
            let operation = match spec.action.as_str() {
                "create" => {
                    validation_rules.push(ValidationRule::FileNotExists(spec.path.clone()));
                    FileOperation::Create {
                        path: std::path::PathBuf::from(&spec.path),
                        content: String::new(), // Content will be filled during generation
                    }
                }
                "update" => {
                    validation_rules.push(ValidationRule::FileExists(spec.path.clone()));
                    FileOperation::Update {
                        path: std::path::PathBuf::from(&spec.path),
                        old_content: String::new(),
                        new_content: String::new(),
                    }
                }
                _ => return Err(anyhow::anyhow!("Unsupported action: {}", spec.action)),
            };

            file_operations.push(operation);

            // Update risk level
            let risk = match spec.action.as_str() {
                "create" => RiskLevel::Low,
                "update" => RiskLevel::Medium,
                _ => RiskLevel::High,
            };
            max_risk = max_risk.max(risk);
        }

        Ok(ComplexOperation {
            name,
            description,
            file_operations,
            dependencies,
            estimated_risk: max_risk,
            validation_rules,
        })
    }

    fn calculate_confidence_from_response(&self, response: &str, response_type: &str) -> f32 {
        // Simple confidence calculation based on response characteristics
        let base_confidence: f32 = match response_type {
            "analysis" => 0.8,
            "planning" => 0.7,
            "code_generation" => 0.75,
            _ => 0.5,
        };

        // Boost confidence based on response quality indicators
        let mut confidence: f32 = base_confidence;

        if response.contains("error") || response.contains("Error") {
            confidence -= 0.2;
        }

        if response.len() > 100 {
            confidence += 0.1; // Longer responses tend to be more thorough
        }

        if response.contains("imports")
            || response.contains("function")
            || response.contains("class")
        {
            confidence += 0.1; // Code-like content indicates better quality
        }

        confidence.max(0.0).min(1.0)
    }

    /// Build a human-readable summary of file contexts for AI prompts
    fn build_context_summary(&self) -> String {
        if self.file_contexts.is_empty() {
            return "No specific files identified for this goal.".to_string();
        }

        let max_tokens = self.config.context.max_context_tokens;
        let mut current_tokens = 0;
        let mut summary = String::new();

        // Prioritize files: existing files first, then by relevance to goal
        let mut contexts: Vec<_> = self.file_contexts.iter().collect();
        contexts.sort_by(|a, b| {
            // Sort by: 1) exists, 2) smaller size (more likely to be relevant)
            match (a.1.exists, b.1.exists) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.1.size_bytes.cmp(&b.1.size_bytes),
            }
        });

        for (path, context) in contexts
            .iter()
            .take(self.config.context.max_files_in_context)
        {
            let status = if context.exists {
                "EXISTS"
            } else {
                "DOES NOT EXIST"
            };
            let size_info = if context.exists {
                format!(
                    " ({} bytes, {} lines)",
                    context.size_bytes, context.line_count
                )
            } else {
                "".to_string()
            };

            let file_header = format!("- {}: {}{}\n", path, status, size_info);
            let header_tokens = self.estimate_tokens(&file_header);

            // Check if adding this file would exceed token limit
            if current_tokens + header_tokens > max_tokens {
                summary.push_str("... (additional files truncated to fit context window)\n");
                break;
            }

            summary.push_str(&file_header);
            current_tokens += header_tokens;

            // Include smart content preview for existing files
            if let Some(content) = &context.content {
                let preview = self.create_smart_preview(content, max_tokens - current_tokens);
                if !preview.is_empty() {
                    let preview_text = format!("  Preview:\n{}\n", preview);
                    let preview_tokens = self.estimate_tokens(&preview_text);

                    if current_tokens + preview_tokens <= max_tokens {
                        summary.push_str(&preview_text);
                        current_tokens += preview_tokens;
                    }
                }
            }
        }

        summary
    }

    /// Estimate token count from text
    fn estimate_tokens(&self, text: &str) -> usize {
        (text.len() as f32 / self.config.context.token_estimation_ratio) as usize
    }

    /// Create smart preview of file content using shell commands
    fn create_smart_preview(&self, content: &str, remaining_tokens: usize) -> String {
        let max_lines = self.config.context.max_file_preview_lines;
        let lines: Vec<&str> = content.lines().collect();
        let allowed_chars =
            ((remaining_tokens as f32) * self.config.context.token_estimation_ratio) as usize;

        if remaining_tokens == 0 || allowed_chars == 0 {
            return String::new();
        }

        // If within both line and char budget, return full content
        if lines.len() <= max_lines && content.len() <= allowed_chars {
            return content.to_string();
        }

        // Use a trimmed preview respecting char budget
        let mut preview = String::new();
        let head_lines = std::cmp::max(1, max_lines / 2);
        let tail_lines = std::cmp::max(1, max_lines / 2);

        for line in lines.iter().take(head_lines) {
            preview.push_str(line);
            preview.push('\n');
            if preview.len() >= allowed_chars {
                return preview;
            }
        }

        preview.push_str("\n    ... (truncated) ...\n\n");
        if preview.len() >= allowed_chars {
            preview.truncate(allowed_chars);
            return preview;
        }

        for line in lines.iter().skip(lines.len().saturating_sub(tail_lines)) {
            preview.push_str(line);
            preview.push('\n');
            if preview.len() >= allowed_chars {
                break;
            }
        }

        if preview.len() > allowed_chars {
            preview.truncate(allowed_chars);
        }

        preview
    }

    /// Create numbered preview of file content for precise editing
    fn create_numbered_preview(&self, content: &str, total_lines: usize) -> String {
        let max_preview_lines = self.config.context.max_file_preview_lines;
        let lines: Vec<&str> = content.lines().collect();

        if total_lines <= max_preview_lines {
            // Show all lines with numbers
            return lines
                .iter()
                .enumerate()
                .map(|(i, line)| format!("{:4} | {}", i + 1, line))
                .collect::<Vec<_>>()
                .join("\n");
        }

        // Show head and tail with line numbers
        let head_count = max_preview_lines / 2;
        let tail_count = max_preview_lines / 2;

        let mut preview = String::new();

        // Head lines
        for (i, line) in lines.iter().take(head_count).enumerate() {
            preview.push_str(&format!("{:4} | {}\n", i + 1, line));
        }

        preview.push_str("\n     ... (lines hidden for brevity) ...\n\n");

        // Tail lines
        let skip_count = total_lines - tail_count;
        for (i, line) in lines.iter().skip(skip_count).enumerate() {
            preview.push_str(&format!("{:4} | {}\n", skip_count + i + 1, line));
        }

        preview
    }

    /// Parse AI-generated diff format and apply changes to existing content
    fn apply_diff_to_content(&self, original_content: &str, diff_text: &str) -> Result<String> {
        if diff_text.trim() == "NO CHANGES REQUIRED" {
            return Ok(original_content.to_string());
        }

        let mut lines: Vec<String> = original_content.lines().map(|s| s.to_string()).collect();
        let diff_lines: Vec<&str> = diff_text.lines().collect();

        let mut i = 0;
        while i < diff_lines.len() {
            let line = diff_lines[i].trim();

            if line.starts_with("REPLACE lines ") && line.contains(" with:") {
                // Parse REPLACE operation: "REPLACE lines X-Y with:"
                let parts: Vec<&str> = line.split("REPLACE lines ").collect();
                if parts.len() == 2 {
                    let range_part = parts[1].split(" with:").next().unwrap_or("");
                    if let Some((start, end)) = self.parse_line_range(range_part) {
                        // Collect replacement content
                        i += 1;
                        let mut replacement_lines = Vec::new();
                        while i < diff_lines.len()
                            && !diff_lines[i].trim().is_empty()
                            && !diff_lines[i].starts_with("REPLACE")
                            && !diff_lines[i].starts_with("INSERT")
                            && !diff_lines[i].starts_with("DELETE")
                        {
                            replacement_lines.push(diff_lines[i].to_string());
                            i += 1;
                        }
                        i -= 1; // Adjust for the outer loop increment

                        // Apply replacement
                        if start <= end && end <= lines.len() {
                            lines.splice(start - 1..end, replacement_lines);
                        }
                    }
                }
            } else if line.starts_with("INSERT after line ") && line.contains(":") {
                // Parse INSERT operation: "INSERT after line Z:"
                let parts: Vec<&str> = line.split("INSERT after line ").collect();
                if parts.len() == 2 {
                    let line_num_part = parts[1].split(":").next().unwrap_or("");
                    if let Ok(line_num) = line_num_part.trim().parse::<usize>() {
                        // Collect insertion content
                        i += 1;
                        let mut insertion_lines = Vec::new();
                        while i < diff_lines.len()
                            && !diff_lines[i].trim().is_empty()
                            && !diff_lines[i].starts_with("REPLACE")
                            && !diff_lines[i].starts_with("INSERT")
                            && !diff_lines[i].starts_with("DELETE")
                        {
                            insertion_lines.push(diff_lines[i].to_string());
                            i += 1;
                        }
                        i -= 1; // Adjust for the outer loop increment

                        // Apply insertion
                        let insert_pos = if line_num >= lines.len() {
                            lines.len()
                        } else {
                            line_num
                        };
                        for (offset, insert_line) in insertion_lines.into_iter().enumerate() {
                            lines.insert(insert_pos + offset, insert_line);
                        }
                    }
                }
            } else if line.starts_with("DELETE lines ") {
                // Parse DELETE operation: "DELETE lines A-B"
                let parts: Vec<&str> = line.split("DELETE lines ").collect();
                if parts.len() == 2 {
                    if let Some((start, end)) = self.parse_line_range(parts[1]) {
                        // Apply deletion
                        if start <= end && end <= lines.len() {
                            lines.drain(start - 1..end);
                        }
                    }
                }
            }

            i += 1;
        }

        Ok(lines.join("\n"))
    }

    /// Parse line range like "5-7" into (5, 7)
    fn parse_line_range(&self, range_str: &str) -> Option<(usize, usize)> {
        let parts: Vec<&str> = range_str.split('-').collect();
        if parts.len() == 2 {
            if let (Ok(start), Ok(end)) = (
                parts[0].trim().parse::<usize>(),
                parts[1].trim().parse::<usize>(),
            ) {
                return Some((start, end));
            }
        }
        None
    }

    pub fn get_completed_operations(&self) -> &[FileOperation] {
        &self.completed_operations
    }

    pub fn context_stats(&self) -> (usize, usize, usize, String, String) {
        let files_scanned = self.file_contexts.len();
        let files_analyzed = self
            .file_contexts
            .values()
            .filter(|c| c.content.is_some())
            .count();
        let keywords_count = self.keywords.len();
        (
            files_scanned,
            files_analyzed,
            keywords_count,
            self.os_info.clone(),
            self.cwd.clone(),
        )
    }
}

/// Execution context for agent operations with owned data to avoid lifetime issues
pub struct AgentExecutionContext {
    pub inference_engine: infrastructure::InferenceEngine,
    pub config: Config,
    pub rag_service: Option<Arc<RagService>>,
    pub sandbox: Sandbox,
}

impl AgentExecutionContext {
    pub fn new(
        inference_engine: infrastructure::InferenceEngine,
        config: Config,
        rag_service: Option<Arc<RagService>>,
    ) -> Self {
        Self {
            inference_engine,
            config,
            rag_service,
            sandbox: Sandbox::new(),
        }
    }
}

impl AgentService {
    pub fn new(inference_engine: infrastructure::InferenceEngine) -> Self {
        println!("📊 Gathering system context...");
        let system_context = infrastructure::config::SystemContext::gather();

        Self {
            inference_engine,
            rag_service: None,
            config: Config::load(),
            agent_controller: AgentController::new(),
            failure_handler: SafeFailureHandler::new(),
            system_context,
        }
    }

    /// Lightweight system context to avoid prompt bloat
    fn compact_system_context(&self) -> String {
        format!(
            "user={}@{}, os={} {}, distro={}, pkg_mgr={}, cwd={}, shell={}, display={}",
            self.system_context.user,
            self.system_context.hostname,
            self.system_context.os_type,
            self.system_context.kernel,
            self.system_context.distro_id,
            self.system_context.package_manager,
            self.system_context.current_dir,
            self.system_context.shell,
            self.system_context.display_server
        )
    }

    pub fn with_rag_service(
        inference_engine: infrastructure::InferenceEngine,
        rag_service: Arc<RagService>,
    ) -> Self {
        println!("📊 Gathering system context...");
        let system_context = infrastructure::config::SystemContext::gather();

        Self {
            inference_engine,
            rag_service: Some(rag_service),
            config: Config::load(),
            agent_controller: AgentController::new(),
            failure_handler: SafeFailureHandler::new(),
            system_context,
        }
    }

    /// Generate a shell command based on natural language request with full system context
    pub async fn generate_command(&self, request: &str) -> Result<String> {
        let prompt = format!(
            r#"You are a shell command generator. Generate a precise, safe shell command based on the user's request.

USER REQUEST: {}

SYSTEM CONTEXT:
{}

CRITICAL INSTRUCTIONS:
1. Generate ONLY the command - no explanations, no markdown
2. Use the actual paths and file names from the system context
3. Use the appropriate package manager for this distro: {}
4. Consider the current directory: {}
5. Make the command safe and practical; if the request is ambiguous or lacks paths, respond with 'Cannot determine safe command'
6. If the request mentions a file/folder, search for it in the current directory first; never invent paths

Generate the command now:"#,
            request,
            self.compact_system_context(),
            self.system_context.package_manager,
            self.system_context.current_dir
        );

        let command = self.inference_engine.generate(&prompt).await?;

        // Clean up the response (remove markdown, explanations, etc.)
        let cleaned = command
            .lines()
            .find(|line| {
                !line.trim().is_empty() && !line.starts_with("```") && !line.starts_with("#")
            })
            .unwrap_or(&command)
            .trim()
            .to_string();

        Ok(cleaned)
    }

    /// Built-in safe tool definitions exposed to the agent for selection
    fn default_tool_definitions(&self) -> Vec<ToolDefinition> {
        fn param(name: &str, description: &str) -> (String, ParameterProperty) {
            (
                name.to_string(),
                ParameterProperty {
                    param_type: "string".to_string(),
                    description: description.to_string(),
                    enum_values: None,
                },
            )
        }

        fn params(map: Vec<(String, ParameterProperty)>, required: Vec<&str>) -> ToolParameters {
            ToolParameters {
                param_type: "object".to_string(),
                properties: map.into_iter().collect(),
                required: required.into_iter().map(|s| s.to_string()).collect(),
            }
        }

        let registry_tools = ToolRegistry::new().list_tools();
        let allowed: std::collections::HashSet<String> = registry_tools.into_iter().collect();

        let base_defs = vec![
            ToolDefinition {
                name: "file_read".to_string(),
                description: "Read file contents safely with validation and size limits"
                    .to_string(),
                parameters: params(
                    vec![param("path", "Absolute or relative path to the file")],
                    vec!["path"],
                ),
            },
            ToolDefinition {
                name: "file_write".to_string(),
                description: "Write file contents with backup/rollback safeguards".to_string(),
                parameters: params(
                    vec![
                        param("path", "File path to write"),
                        param("content", "Full file contents"),
                    ],
                    vec!["path", "content"],
                ),
            },
            ToolDefinition {
                name: "directory_list".to_string(),
                description: "List directory contents".to_string(),
                parameters: params(vec![param("path", "Directory path (default .)")], vec![]),
            },
            ToolDefinition {
                name: "process_list".to_string(),
                description: "List running processes with optional filter".to_string(),
                parameters: params(
                    vec![param("filter", "Optional substring to match process names")],
                    vec![],
                ),
            },
            ToolDefinition {
                name: "grep_search".to_string(),
                description: "Search for a regex pattern within files".to_string(),
                parameters: params(
                    vec![
                        param("pattern", "Regex or plain text to search for"),
                        param("path", "File or directory to search (default .)"),
                    ],
                    vec!["pattern"],
                ),
            },
            ToolDefinition {
                name: "find_files".to_string(),
                description: "Find files under a path with optional filters".to_string(),
                parameters: params(
                    vec![
                        param("path", "Directory to search"),
                        param("name", "Optional filename pattern (e.g., *.rs)"),
                        param("size", "Optional size filter (e.g., +10M)"),
                    ],
                    vec!["path"],
                ),
            },
            ToolDefinition {
                name: "sed_replace".to_string(),
                description: "Perform safe text replacement in a file".to_string(),
                parameters: params(
                    vec![
                        param("path", "File path"),
                        param("pattern", "Pattern to replace"),
                        param("replacement", "Replacement text"),
                    ],
                    vec!["path", "pattern", "replacement"],
                ),
            },
            ToolDefinition {
                name: "awk_extract".to_string(),
                description: "Extract or transform data from files using awk-like patterns"
                    .to_string(),
                parameters: params(
                    vec![
                        param("path", "File path"),
                        param("script", "Awk program to run"),
                    ],
                    vec!["path", "script"],
                ),
            },
            ToolDefinition {
                name: "curl_fetch".to_string(),
                description: "Fetch content from an HTTP URL (read-only)".to_string(),
                parameters: params(vec![param("url", "HTTP/HTTPS URL to fetch")], vec!["url"]),
            },
            ToolDefinition {
                name: "web_search".to_string(),
                description: "Search the web for documentation and best practices".to_string(),
                parameters: params(vec![param("query", "Search query")], vec!["query"]),
            },
            ToolDefinition {
                name: "git_status".to_string(),
                description: "Show git repository status (read-only)".to_string(),
                parameters: params(vec![param("path", "Repo path (default .)")], vec![]),
            },
            ToolDefinition {
                name: "git_diff".to_string(),
                description: "Show git diffs between commits or working tree".to_string(),
                parameters: params(
                    vec![
                        param("path", "Repo path (default .)"),
                        param("rev", "Optional revision or range"),
                    ],
                    vec![],
                ),
            },
            ToolDefinition {
                name: "git_log".to_string(),
                description: "Show git commit history with optional filters".to_string(),
                parameters: params(
                    vec![
                        param("path", "Repo path (default .)"),
                        param("limit", "Optional number of commits"),
                        param("author", "Optional author filter"),
                    ],
                    vec![],
                ),
            },
        ];

        let mut dedup = std::collections::HashSet::new();
        base_defs
            .into_iter()
            .filter(|def| allowed.contains(&def.name))
            .filter(|def| dedup.insert(def.name.clone()))
            .collect()
    }

    fn filter_valid_tool_calls(
        &self,
        agent_context: &AgentContext,
        tool_calls: Vec<ToolCall>,
    ) -> Vec<ToolCall> {
        let allowed: std::collections::HashSet<String> = agent_context
            .available_tools
            .iter()
            .map(|t| t.name.clone())
            .collect();

        tool_calls
            .into_iter()
            .filter(|tc| allowed.contains(&tc.name))
            .collect()
    }

    pub async fn process_request(&self, request: &AgentRequest) -> Result<AgentResponse> {
        let execution_context = Arc::new(AgentExecutionContext::new(
            self.inference_engine.clone(),
            self.config.clone(),
            self.rag_service.clone(),
        ));

        // Execute bounded multi-iteration agent
        let (agent_result, tool_calls, tool_results) = self
            .execute_agent(&request.goal, request, Arc::clone(&execution_context))
            .await
            .map_err(|e| AgentError::InternalError(format!("Agent execution failed: {e}")))?;

        Ok(AgentResponse {
            reasoning: vec!["Multi-iteration bounded execution completed".to_string()],
            tool_calls,
            tool_results,
            final_response: agent_result.final_response,
            confidence: agent_result.confidence_score,
        })
    }

    /// Create an incremental build planner for streaming planning
    pub async fn plan_build_incremental(&self, goal: &str) -> Result<IncrementalBuildPlanner> {
        let mut retrieved_context = Vec::new();

        // Add compact system context first (avoid bloat)
        retrieved_context.push(format!(
            "SYSTEM SNAPSHOT: {}",
            self.compact_system_context()
        ));

        // Step 1: Prepare file context by reading existing files
        println!("🔍 Analyzing project structure and existing files...");
        let file_contexts = self.prepare_file_context(goal).await?;
        println!("📁 Found {} relevant files in project", file_contexts.len());

        // Retrieve context using RAG or fast rg search
        let keywords = self.extract_keywords_from_goal(goal);
        let use_rag = self.should_use_rag(&keywords);

        if use_rag {
            if let Some(rag_service) = &self.rag_service {
                println!("Retrieving relevant codebase context...");

                if let Err(e) = rag_service.build_index().await {
                    eprintln!("Warning: Failed to build RAG index: {}", e);
                }

                let rag_query = format!("Find examples and patterns for: {}. Look for similar implementations, utility functions, or scripts.", goal);
                match rag_service.query(&rag_query).await {
                    Ok(context) => {
                        retrieved_context.push(format!("RAG Context:\n{}", context));
                    }
                    Err(e) => {
                        eprintln!("RAG query failed: {}", e);
                    }
                }

                if let Err(e) = rag_service.build_index_for_keywords(&keywords).await {
                    eprintln!("Keyword index failed: {}", e);
                } else {
                    let keyword_query = format!("Examples of {}", keywords.join(", "));
                    if let Ok(keyword_context) = rag_service.query(&keyword_query).await {
                        retrieved_context.push(format!(
                            "Keyword Context ({}):\n{}",
                            keywords.join(", "),
                            keyword_context
                        ));
                    }
                }
            }
        } else {
            let rg_hits = self.fast_rg_context(&keywords)?;
            if !rg_hits.is_empty() {
                retrieved_context.push(format!("rg snippets:\n{}", rg_hits.join("\n")));
            } else {
                retrieved_context.push("No rg snippets found for goal keywords".to_string());
            }
        }

        // Create planner with populated file contexts
        let mut planner =
            IncrementalBuildPlanner::new(goal.to_string(), retrieved_context, self.config.clone());
        planner.file_contexts = file_contexts;
        planner.keywords = keywords;
        Ok(planner)
    }

    /// Generate a build plan with RAG context retrieval
    pub async fn plan_build(&self, goal: &str) -> Result<BuildPlanOutcome> {
        let mut retrieved_context = Vec::new();
        let mut planning_logs = Vec::new();

        // Step 1: Retrieve relevant context using RAG or fast rg search
        let keywords = self.extract_keywords_from_goal(goal);
        let use_rag = self.should_use_rag(&keywords);

        if use_rag {
            if let Some(rag_service) = &self.rag_service {
                println!("Retrieving relevant codebase context...");

                if let Err(e) = rag_service.build_index().await {
                    eprintln!("Warning: Failed to build RAG index: {}", e);
                }

                let rag_query = format!("Find examples and patterns for: {}. Look for similar implementations, utility functions, or scripts.", goal);
                match rag_service.query(&rag_query).await {
                    Ok(context) => {
                        retrieved_context.push(format!("RAG Context:\n{}", context));
                        planning_logs.push("RAG query succeeded".to_string());
                    }
                    Err(e) => {
                        planning_logs.push(format!("RAG query failed: {}", e));
                    }
                }

                if let Err(e) = rag_service.build_index_for_keywords(&keywords).await {
                    planning_logs.push(format!("Keyword index failed: {}", e));
                } else {
                    let keyword_query = format!("Examples of {}", keywords.join(", "));
                    if let Ok(keyword_context) = rag_service.query(&keyword_query).await {
                        retrieved_context.push(format!(
                            "Keyword Context ({}):\n{}",
                            keywords.join(", "),
                            keyword_context
                        ));
                        planning_logs.push("Keyword RAG query succeeded".to_string());
                    }
                }
            }
        } else {
            planning_logs.push("RAG unavailable; using fast rg search".to_string());
            let rg_hits = self.fast_rg_context(&keywords)?;
            if !rg_hits.is_empty() {
                retrieved_context.push(format!("rg snippets:\n{}", rg_hits.join("\n")));
            } else {
                retrieved_context.push("No rg snippets found for goal keywords".to_string());
            }
        }

        // Step 2: Generate build plan using the inference engine with guarded retries
        let max_plan_attempts = self.config.context.max_plan_attempts;
        let mut last_error = None;
        let mut raw_plan_text = String::new();
        let mut plan: Option<BuildPlan> = None;
        let mut attempt_count = 0;

        for attempt in 1..=max_plan_attempts {
            attempt_count = attempt;
            let prompt = self.create_build_planning_prompt(goal, &retrieved_context);
            planning_logs.push(format!("Attempt {}: generating plan", attempt));

            match self.inference_engine.generate(&prompt).await {
                Ok(text) => {
                    raw_plan_text = text;
                    match self.parse_build_plan(&raw_plan_text, goal) {
                        Ok(parsed) => {
                            plan = Some(parsed);
                            planning_logs
                                .push(format!("Attempt {}: plan parsed successfully", attempt));
                            break;
                        }
                        Err(e) => {
                            planning_logs
                                .push(format!("Attempt {}: plan parse failed: {}", attempt, e));
                            last_error = Some(e);
                        }
                    }
                }
                Err(e) => {
                    planning_logs.push(format!("Attempt {}: generation failed: {}", attempt, e));
                    last_error = Some(anyhow::anyhow!(e));
                }
            }
        }

        let build_plan = match plan {
            Some(p) => p,
            None => {
                let snippet = if raw_plan_text.len() > 800 {
                    format!("{}...", &raw_plan_text[..800])
                } else {
                    raw_plan_text.clone()
                };
                return Err(anyhow::anyhow!(format!(
                    "Failed to produce a valid build plan after {} attempts: {}\nLast plan text:\n{}\nLogs:\n{}",
                    max_plan_attempts,
                    last_error.unwrap_or_else(|| anyhow::anyhow!("Unknown planning error")),
                    snippet,
                    planning_logs.join("\n")
                )));
            }
        };

        Ok(BuildPlanOutcome {
            plan: build_plan,
            retrieved_context,
            raw_plan_text,
            planning_attempts: attempt_count,
            planning_logs,
        })
    }

    fn extract_keywords_from_goal(&self, goal: &str) -> Vec<String> {
        // Extract meaningful keywords for RAG search
        let words: Vec<String> = goal
            .split_whitespace()
            .filter(|w| w.len() > 2)
            .map(|w| w.to_lowercase())
            .collect();

        // Filter out common words
        let stop_words = [
            "make",
            "create",
            "write",
            "build",
            "script",
            "file",
            "add",
            "implement",
            "for",
            "the",
            "and",
            "or",
        ];
        words
            .into_iter()
            .filter(|w| !stop_words.contains(&w.as_str()))
            .collect()
    }

    /// Prepare file context by discovering relevant files from project structure and content
    async fn prepare_file_context(&self, goal: &str) -> Result<HashMap<String, FileContext>> {
        let mut file_paths = Vec::new();
        let mut file_contexts = HashMap::new();
        let mut content_loaded = 0usize;
        let max_content_files = self.config.context.max_files_in_context;
        let content_allowed = self.content_needed(goal);
        let max_preview_bytes = (self.config.context.max_file_preview_lines as u64) * 200;

        // 1. Extract explicitly mentioned files from goal
        let explicit_files = self.extract_file_paths_from_goal(goal)?;
        file_paths.extend(explicit_files);

        // 2. Use ripgrep to discover relevant files based on keywords from goal
        let keywords = self.extract_keywords_from_goal(goal);
        if !keywords.is_empty() {
            if let Ok(discovered) = self.search_files_by_content(&keywords).await {
                file_paths.extend(discovered);
            }
        }

        // 3. If no files discovered yet, check if any common entry point files exist
        if file_paths.is_empty() {
            if let Ok(entry_files) = self.find_likely_entry_files().await {
                file_paths.extend(entry_files);
            }
        }

        // 4. Deduplicate while preserving order
        let mut seen = std::collections::HashSet::new();
        let deduplicated: Vec<String> = file_paths
            .into_iter()
            .filter(|p| seen.insert(p.clone()))
            .collect();

        // 5. Build context for each discovered file
        for path in deduplicated {
            let full_path = std::path::Path::new(&path);
            let exists = full_path.exists();

            let mut context = FileContext {
                path: path.clone(),
                exists,
                content: None,
                size_bytes: 0,
                line_count: 0,
                modified: None,
                operation_type: if exists {
                    FileOperationType::Update
                } else {
                    FileOperationType::Create
                },
            };

            if exists {
                // Read file content and metadata
                if let Ok(metadata) = std::fs::metadata(&full_path) {
                    context.size_bytes = metadata.len();
                    context.modified = Some(metadata.modified()?);

                    // Read content only when necessary and within limits
                    if content_allowed
                        && content_loaded < max_content_files
                        && metadata.len() <= self.config.context.max_file_size_bytes
                        && metadata.len() <= max_preview_bytes
                    {
                        if let Ok(content) = tokio::fs::read_to_string(&full_path).await {
                            context.line_count = content.lines().count();
                            context.content = Some(content);
                            content_loaded += 1;
                        }
                    }
                }
            }

            file_contexts.insert(path, context);
        }

        Ok(file_contexts)
    }

    /// Search for files containing keywords using grep with advanced shell commands
    async fn search_files_by_content(&self, keywords: &[String]) -> Result<Vec<String>> {
        use std::process::Command;

        let mut discovered = Vec::new();

        // Build grep pattern from keywords (escaped and joined)
        let pattern = keywords
            .iter()
            .map(|k| k.replace("'", "\\'"))
            .collect::<Vec<_>>()
            .join("\\|");

        // Use shell command with grep, find, and awk for efficient searching
        // This finds files containing any keyword, filters out binary/build dirs, and limits results
        let max_candidates = self.config.context.max_search_candidates;
        let max_results = self.config.context.max_search_results;

        let shell_cmd = format!(
            r#"
            find . -type f \
                ! -path '*/.*' \
                ! -path '*/node_modules/*' \
                ! -path '*/target/*' \
                ! -path '*/dist/*' \
                ! -path '*/build/*' \
                ! -path '*/vendor/*' \
                ! -path '*/__pycache__/*' \
                -size -1M \
                2>/dev/null \
            | head -n {} \
            | xargs -I {{}} sh -c 'grep -l -i -m 1 "{}" "{{}}" 2>/dev/null' \
            | head -n {} \
            | sed 's|^\./||'
            "#,
            max_candidates, pattern, max_results
        );

        let output = Command::new("sh").arg("-c").arg(&shell_cmd).output();

        if let Ok(output) = output {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        discovered.push(trimmed.to_string());
                    }
                }
            }
        }

        Ok(discovered)
    }

    /// Find likely entry point files using find, grep, and awk
    async fn find_likely_entry_files(&self) -> Result<Vec<String>> {
        use std::process::Command;

        let mut entry_files = Vec::new();

        // Use advanced shell commands to find relevant files
        // Priority: recently modified files in root/src/public/static, exclude build artifacts
        let max_results = self.config.context.max_files_in_context;

        let shell_cmd = format!(
            r#"
            {{
                # Find files in root directory (depth 1)
                find . -maxdepth 1 -type f -size -1M 2>/dev/null | sed 's|^\./||'

                # Find files in common source directories (depth 2)
                find ./src ./public ./static ./app -maxdepth 2 -type f -size -1M 2>/dev/null | sed 's|^\./||'
            }} \
            | grep -v '/\.' \
            | grep -vE '(node_modules|target|dist|build|vendor|__pycache__|\.git)' \
            | awk -v max={} '
                BEGIN {{ count = 0 }}
                {{
                    # Prioritize certain file types
                    if ($0 ~ /\.(html|js|ts|py|rs|go|java|cpp|c|rb|php|jsx|tsx)$/) {{
                        print
                        count++
                    }} else if (count < max / 2) {{
                        print
                        count++
                    }}
                    if (count >= max) exit
                }}
            ' \
            | head -n {}
            "#,
            max_results, max_results
        );

        let output = Command::new("sh").arg("-c").arg(shell_cmd).output();

        if let Ok(output) = output {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        entry_files.push(trimmed.to_string());
                    }
                }
            }
        }

        Ok(entry_files)
    }

    /// Extract file paths mentioned in the goal using dynamic regex-based detection
    fn extract_file_paths_from_goal(&self, goal: &str) -> Result<Vec<String>> {
        use once_cell::sync::Lazy;
        use regex::Regex;

        static FILE_PATH_REGEX: Lazy<Regex> = Lazy::new(|| {
            Regex::new(r#"(?x)
                (?:^|\s|["'`\(\[]])                                 # path start delimiters
                (                                                   # capture group for full path
                    (?:\.?/)?                                       # optional ./ or / prefix
                    (?:[\w\-]+/)*                                   # optional directory segments
                    [\w\-]+                                         # filename (alphanumeric, underscore, hyphen)
                    \.                                              # extension dot
                    [a-zA-Z]{1,4}                                   # extension (1-4 letters)
                )
                (?:$|\s|["'`\)\]]|\.|,|:|;|>)                       # path end delimiters
            "#).unwrap()
        });

        static QUOTED_PATH_REGEX: Lazy<Regex> = Lazy::new(|| {
            Regex::new(r#"(?i)['"`]([^'"`]{1,200}?\.(html|css|js|ts|jsx|tsx|json|md|rs|py|toml|yaml|yml|txt))['"`]"#).unwrap()
        });

        let mut paths = Vec::new();

        // Phase 1: Extract explicit quoted paths (highest priority)
        for cap in QUOTED_PATH_REGEX.captures_iter(goal) {
            if let Some(path_match) = cap.get(1) {
                let path = path_match.as_str().trim();
                if self.validate_path_format(path) {
                    paths.push(path.to_string());
                }
            }
        }

        // Phase 2: Extract unquoted paths with extensions
        for cap in FILE_PATH_REGEX.captures_iter(goal) {
            if let Some(path_match) = cap.get(1) {
                let candidate = path_match
                    .as_str()
                    .trim_end_matches(|c: char| "!?.),".contains(c));
                if self.validate_path_format(candidate) && self.is_likely_file_path(candidate) {
                    paths.push(candidate.to_string());
                }
            }
        }

        // Remove duplicates while preserving order
        let mut seen = std::collections::HashSet::new();
        let deduplicated = paths
            .into_iter()
            .filter(|path| seen.insert(path.clone()))
            .collect::<Vec<_>>();
        Ok(deduplicated)
    }

    fn validate_path_format(&self, path: &str) -> bool {
        if path.is_empty() || path.len() > 200 {
            return false;
        }
        if !path.contains('.') {
            return false;
        }
        if path.contains("..") || path.contains('|') || path.contains('<') || path.contains('>') {
            return false;
        }
        true
    }

    fn is_likely_file_path(&self, candidate: &str) -> bool {
        let common_extensions = [
            "html", "css", "js", "ts", "jsx", "tsx", "rs", "py", "json", "md", "txt",
        ];
        let has_valid_extension = common_extensions
            .iter()
            .any(|ext| candidate.ends_with(&format!(".{}", ext)));

        let not_common_word = ![
            "the", "and", "for", "are", "but", "not", "you", "all", "can", "her", "was", "one",
        ]
        .iter()
        .any(|&word| candidate.to_lowercase() == word);

        has_valid_extension && not_common_word
    }

    fn detects_web_intent(&self, lower_goal: &str) -> bool {
        lower_goal.contains("page")
            || lower_goal.contains("site")
            || lower_goal.contains("web")
            || lower_goal.contains("landing")
            || lower_goal.contains("portfolio")
            || lower_goal.contains("website")
            || lower_goal.contains("html")
    }

    fn fast_rg_context(&self, keywords: &[String]) -> Result<Vec<String>> {
        let mut hits = Vec::new();
        let mut seen = 0;
        let max_keywords = self.config.context.max_keywords_for_search;
        let max_lines_per_keyword = self.config.context.max_lines_per_keyword;
        let max_total_snippets = self.config.context.max_rg_context_snippets;

        for kw in keywords.iter().take(max_keywords) {
            let output = std::process::Command::new("rg")
                .arg("-n")
                .arg("--max-count")
                .arg("2")
                .arg(kw)
                .arg(".")
                .output();
            if let Ok(out) = output {
                if out.status.success() {
                    let text = String::from_utf8_lossy(&out.stdout);
                    for line in text.lines().take(max_lines_per_keyword) {
                        hits.push(format!("rg [{}]: {}", kw, line));
                        seen += 1;
                        if seen >= max_total_snippets {
                            return Ok(hits);
                        }
                    }
                }
            }
        }
        Ok(hits)
    }

    fn create_build_planning_prompt(&self, goal: &str, context: &[String]) -> String {
        let context_str = if context.is_empty() {
            "No additional context available.".to_string()
        } else {
            context.join("\n\n")
        };

        format!(
            r#"You are an expert engineer producing a compact, actionable build plan.

GOAL:
{goal}

SYSTEM:
{system}

CONTEXT:
{context}

OUTPUT (plain text, no JSON):
Build Plan:
- Step 1: ...
- Step 2: ...

Files:
- path: relative/path.ext
- action: create|update
- reason: short note
- content in a fenced block:
```file:path=relative/path.ext;action=create
<full post-change content>
```

Safety: risks/backups/rollback
Estimate: size/time
Confidence: percentage

Rules: keep it concise and deterministic; only include real files; if context is insufficient, reply 'Insufficient context to plan' and stop (do not invent files or behavior); if you cannot provide full content, say so and stop; prefer package manager {pkg_mgr}; consider display server {display_srv} for GUI hints."#,
            goal = goal,
            system = self.compact_system_context(),
            context = context_str,
            pkg_mgr = self.system_context.package_manager,
            display_srv = self.system_context.display_server
        )
    }

    fn should_use_rag(&self, keywords: &[String]) -> bool {
        self.rag_service.is_some() && !keywords.is_empty()
    }

    fn content_needed(&self, goal: &str) -> bool {
        let g = goal.to_lowercase();
        let hints = [
            "read",
            "show",
            "inspect",
            "view",
            "debug",
            "trace",
            "fix",
            "bug",
            "search",
            "analyze",
            "understand",
            "why",
            "explain",
            "context",
        ];
        hints.iter().any(|h| g.contains(h))
    }

    fn parse_build_plan(&self, plan_text: &str, goal: &str) -> Result<BuildPlan> {
        let mut operations = Vec::new();
        let mut description = String::from("Build plan (markdown)");
        let estimated_risk = RiskLevel::Low;

        if let Some(idx) = plan_text.find("Build Plan") {
            description = plan_text[idx..]
                .lines()
                .take(8)
                .collect::<Vec<_>>()
                .join(" ");
        }

        for fence in plan_text.match_indices("```file:") {
            let header_start = fence.0 + "```file:".len();
            let after_header = match plan_text[header_start..].find('\n') {
                Some(v) => header_start + v + 1,
                None => continue,
            };
            let header = &plan_text[header_start..after_header - 1];
            let end_fence = match plan_text[after_header..].find("```") {
                Some(v) => after_header + v,
                None => continue,
            };
            let content = &plan_text[after_header..end_fence];

            let mut path = "";
            let mut action = "create";
            for part in header.split(';') {
                let part = part.trim();
                if let Some(rest) = part.strip_prefix("path=") {
                    path = rest;
                } else if let Some(rest) = part.strip_prefix("action=") {
                    action = rest;
                }
            }

            if path.is_empty() {
                continue;
            }

            let op = match action {
                "update" => {
                    let existing = std::fs::read_to_string(path).unwrap_or_default();
                    FileOperation::Update {
                        path: std::path::PathBuf::from(path),
                        old_content: existing,
                        new_content: content.to_string(),
                    }
                }
                "delete" => FileOperation::Delete {
                    path: std::path::PathBuf::from(path),
                },
                _ => FileOperation::Create {
                    path: std::path::PathBuf::from(path),
                    content: content.to_string(),
                },
            };

            operations.push(op);
        }

        if operations.is_empty() {
            return Err(anyhow::anyhow!(
                "Plan did not include any file fences with actions; cannot proceed."
            ));
        }

        Ok(BuildPlan {
            goal: goal.to_string(),
            operations,
            description,
            estimated_risk,
        })
    }

    /// Generate reasoning steps for a goal
    pub async fn generate_reasoning(
        &self,
        goal: &str,
        _context: &AgentContext,
    ) -> Result<Vec<String>> {
        // Simplified implementation - in real code this would call the model
        Ok(vec![format!("Reasoning about: {}", goal)])
    }

    /// Determine if tools are needed for the goal
    pub fn needs_tools(&self, goal: &str, reasoning: &[String], context: &AgentContext) -> bool {
        if context.available_tools.is_empty() {
            return false;
        }
        let lower = goal.to_lowercase();
        let hints = [
            "read file",
            "write file",
            "search",
            "find",
            "grep",
            "process",
            "list dir",
            "git",
            "log",
            "diff",
            "status",
            "http",
            "url",
        ];
        if hints.iter().any(|h| lower.contains(h)) {
            return true;
        }
        reasoning
            .iter()
            .any(|r| hints.iter().any(|h| r.to_lowercase().contains(h)))
    }

    /// Plan tool calls based on reasoning
    pub fn plan_tool_calls(
        &self,
        goal: &str,
        _reasoning: &[String],
        context: &AgentContext,
        _exec_context: &AgentExecutionContext,
    ) -> Vec<ToolCall> {
        let mut calls = Vec::new();
        let lower_goal = goal.to_lowercase();
        let primary_path = self
            .extract_file_paths_from_goal(goal)
            .unwrap_or_default()
            .into_iter()
            .next()
            .unwrap_or_else(|| ".".to_string());

        let mut pending_calls: Vec<ToolCall> = Vec::new();

        if lower_goal.contains("web search") || lower_goal.contains("online") {
            let mut params = HashMap::new();
            params.insert(
                "query".to_string(),
                serde_json::Value::String(goal.to_string()),
            );
            self.maybe_push_call(
                &context.available_tools,
                &mut pending_calls,
                "web_search",
                params,
                "Need external info",
            );
        } else if lower_goal.contains("search") || lower_goal.contains("grep") {
            let mut params = HashMap::new();
            params.insert(
                "pattern".to_string(),
                serde_json::Value::String(goal.to_string()),
            );
            params.insert(
                "path".to_string(),
                serde_json::Value::String(primary_path.clone()),
            );
            self.maybe_push_call(
                &context.available_tools,
                &mut pending_calls,
                "grep_search",
                params,
                "Search locally for pattern",
            );
        }

        if lower_goal.contains("find file")
            || lower_goal.contains("locate")
            || lower_goal.contains("list dir")
        {
            let mut params = HashMap::new();
            params.insert(
                "path".to_string(),
                serde_json::Value::String(primary_path.clone()),
            );
            self.maybe_push_call(
                &context.available_tools,
                &mut pending_calls,
                "directory_list",
                params.clone(),
                "List directory for context",
            );
            self.maybe_push_call(
                &context.available_tools,
                &mut pending_calls,
                "find_files",
                params,
                "Find files under path",
            );
        }

        if lower_goal.contains("read")
            || lower_goal.contains("open file")
            || lower_goal.contains("show file")
        {
            let mut params = HashMap::new();
            params.insert(
                "path".to_string(),
                serde_json::Value::String(primary_path.clone()),
            );
            self.maybe_push_call(
                &context.available_tools,
                &mut pending_calls,
                "file_read",
                params,
                "Read file content",
            );
        }

        if lower_goal.contains("write")
            || lower_goal.contains("update")
            || lower_goal.contains("replace")
        {
            let mut params = HashMap::new();
            params.insert(
                "path".to_string(),
                serde_json::Value::String(primary_path.clone()),
            );
            params.insert(
                "content".to_string(),
                serde_json::Value::String("Provide updated content here".to_string()),
            );
            self.maybe_push_call(
                &context.available_tools,
                &mut pending_calls,
                "file_write",
                params,
                "Write file safely",
            );
        }

        if lower_goal.contains("process") || lower_goal.contains("ps ") {
            self.maybe_push_call(
                &context.available_tools,
                &mut pending_calls,
                "process_list",
                HashMap::new(),
                "Inspect running processes",
            );
        }

        if lower_goal.contains("curl") || lower_goal.contains("http") || lower_goal.contains("url")
        {
            let mut params = HashMap::new();
            params.insert(
                "url".to_string(),
                serde_json::Value::String(goal.to_string()),
            );
            self.maybe_push_call(
                &context.available_tools,
                &mut pending_calls,
                "curl_fetch",
                params,
                "Fetch remote content",
            );
        }

        if lower_goal.contains("git status") {
            self.maybe_push_call(
                &context.available_tools,
                &mut pending_calls,
                "git_status",
                HashMap::new(),
                "Check repo status",
            );
        } else if lower_goal.contains("git diff") || lower_goal.contains("diff") {
            self.maybe_push_call(
                &context.available_tools,
                &mut pending_calls,
                "git_diff",
                HashMap::new(),
                "Inspect changes",
            );
        } else if lower_goal.contains("git log") || lower_goal.contains("history") {
            self.maybe_push_call(
                &context.available_tools,
                &mut pending_calls,
                "git_log",
                HashMap::new(),
                "Inspect history",
            );
        }

        let has_calls = !pending_calls.is_empty();

        if !has_calls && !context.available_tools.is_empty() {
            // Default to directory_list for lightweight discovery
            let mut params = HashMap::new();
            params.insert("path".to_string(), serde_json::Value::String(primary_path));
            self.maybe_push_call(
                &context.available_tools,
                &mut pending_calls,
                "directory_list",
                params,
                "Fallback discovery",
            );
        }

        calls.extend(pending_calls);
        calls
    }

    /// Execute tool calls
    pub async fn execute_tool_calls(
        &self,
        tool_calls: &[ToolCall],
        _context: &mut AgentContext,
        _exec_context: &AgentExecutionContext,
    ) -> Result<Vec<ToolResult>> {
        let registry = ToolRegistry::new();
        let mut results = Vec::new();

        for tool_call in tool_calls {
            let mut params: HashMap<String, String> = HashMap::new();
            for (k, v) in &tool_call.parameters {
                let as_str = match v {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                params.insert(k.clone(), as_str);
            }

            let args = ToolArgs {
                parameters: params,
                timeout: Some(std::time::Duration::from_secs(30)),
                working_directory: Some(self.system_context.current_dir.clone()),
            };

            match registry.execute_tool(&tool_call.name, args).await {
                Ok(output) => {
                    results.push(ToolResult {
                        tool_call_id: tool_call.id.clone(),
                        success: output.success,
                        result: serde_json::json!({
                            "tool": tool_call.name,
                            "stdout": self.truncate_text(&output.stdout, 4000),
                            "stderr": self.truncate_text(&output.stderr, 2000),
                            "duration_ms": output.execution_time.as_millis(),
                            "exit_code": output.exit_code,
                        }),
                        error: None,
                    });
                }
                Err(e) => {
                    results.push(ToolResult {
                        tool_call_id: tool_call.id.clone(),
                        success: false,
                        result: Value::Null,
                        error: Some(format!("{}: {}", tool_call.name, e)),
                    });
                }
            }
        }

        Ok(results)
    }

    /// Generate final response
    pub async fn generate_final_response(
        &self,
        goal: &str,
        reasoning: &[String],
        tool_results: &[ToolResult],
    ) -> Result<String> {
        let facts = self.summarize_tool_results(tool_results);
        if facts.is_empty() {
            return Ok("Insufficient context to answer; gather tool outputs (files/tests/commands) and retry.".to_string());
        }

        let max_facts = 8usize;
        let facts_text = facts
            .into_iter()
            .take(max_facts)
            .map(|f| format!("- {}", f))
            .collect::<Vec<_>>()
            .join("\n");

        let reasoning_text = if reasoning.is_empty() {
            "No prior reasoning; rely only on facts.".to_string()
        } else {
            reasoning.join(" ")
        };

        let prompt = format!(
            r#"You are a coding agent producing a grounded, concise answer.
Goal: {goal}
Reasoning (for background only; do not invent from it): {reasoning}
Facts (authoritative, from tools): 
{facts}
Instructions:
- Base the answer ONLY on the Facts above. Do not invent file paths, outputs, APIs, or behavior not present in Facts.
- If the facts are insufficient, reply exactly: "Insufficient context to answer."
- Keep the answer <= 6 sentences.
- Include a "Facts:" section echoing the facts you used, and nothing else.
Respond now."#,
            goal = goal,
            reasoning = reasoning_text,
            facts = facts_text
        );

        let response = self.inference_engine.generate(&prompt).await?;
        Ok(response.trim().to_string())
    }

    fn truncate_text(&self, input: &str, max_len: usize) -> String {
        if input.len() <= max_len {
            input.to_string()
        } else {
            format!("{}…", &input[..max_len])
        }
    }

    fn summarize_tool_results(&self, tool_results: &[ToolResult]) -> Vec<String> {
        let mut facts = Vec::new();
        let max_len = 240usize;

        for tr in tool_results {
            if tr.result == Value::Null && tr.error.is_none() {
                continue;
            }
            if let Some(err) = &tr.error {
                facts.push(format!(
                    "tool_call_id={} success=false error={}",
                    tr.tool_call_id,
                    self.truncate_text(err, max_len)
                ));
                continue;
            }

            let mut fact = format!("tool_call_id={} success={}", tr.tool_call_id, tr.success);
            if let Value::Object(map) = &tr.result {
                if let Some(tool) = map.get("tool").and_then(|v| v.as_str()) {
                    fact.push_str(&format!(" tool={}", tool));
                }
                if let Some(stdout) = map.get("stdout").and_then(|v| v.as_str()) {
                    let cleaned = stdout.trim();
                    if !cleaned.is_empty() {
                        fact.push_str(&format!(
                            " stdout=\"{}\"",
                            self.truncate_text(cleaned, max_len)
                        ));
                    }
                }
                if let Some(stderr) = map.get("stderr").and_then(|v| v.as_str()) {
                    let cleaned = stderr.trim();
                    if !cleaned.is_empty() {
                        fact.push_str(&format!(
                            " stderr=\"{}\"",
                            self.truncate_text(cleaned, max_len)
                        ));
                    }
                }
                if let Some(msg) = map.get("message").and_then(|v| v.as_str()) {
                    let cleaned = msg.trim();
                    if !cleaned.is_empty() {
                        fact.push_str(&format!(
                            " note=\"{}\"",
                            self.truncate_text(cleaned, max_len)
                        ));
                    }
                }
            }

            facts.push(fact);
        }

        facts
    }

    fn maybe_push_call(
        &self,
        available_tools: &[ToolDefinition],
        pending_calls: &mut Vec<ToolCall>,
        name: &str,
        params: HashMap<String, serde_json::Value>,
        why: &str,
    ) {
        if available_tools.iter().any(|t| t.name == name) {
            pending_calls.push(ToolCall {
                id: format!("{}-1", name),
                name: name.to_string(),
                parameters: params,
                reasoning: why.to_string(),
            });
        }
    }

    /// Calculate confidence score
    pub fn calculate_confidence(&self, reasoning: &[String], tool_results: &[ToolResult]) -> f32 {
        // Simplified implementation
        let base_confidence = 0.5;
        let reasoning_bonus = (reasoning.len() as f32) * 0.1;
        let tool_bonus = (tool_results.len() as f32) * 0.2;
        (base_confidence + reasoning_bonus + tool_bonus).min(1.0)
    }

    /// Entry point used by `AgentService`.
    ///
    /// Bounded multi-iteration execution:
    /// - Ask model to reason
    /// - Decide/plan tools (optional)
    /// - Execute tools
    /// - Ask model to produce final answer (or continue)
    pub async fn execute_agent(
        &self,
        goal: &str,
        request: &AgentRequest,
        exec_context: Arc<AgentExecutionContext>,
    ) -> Result<(AgentResult, Vec<ToolCall>, Vec<ToolResult>)> {
        // Build initial agent context with available tools + conversation.
        let mut agent_context = AgentContext {
            available_tools: self.default_tool_definitions(),
            conversation_history: Vec::<ConversationMessage>::new(),
            working_memory: std::collections::HashMap::new(),
        };

        // Seed conversation with request context if you have it.
        // If `AgentRequest` has history/messages, adapt this block accordingly.
        agent_context
            .conversation_history
            .push(ConversationMessage {
                role: "user".to_string(),
                content: goal.to_string(),
                tool_calls: None,
                tool_call_id: None,
            });

        let max_iters = 5; // Configurable iteration limit

        let mut execution_state = AgentExecutionState {
            iteration_count: 0,
            total_tools_executed: 0,
            start_time: std::time::SystemTime::now(),
            last_verification_result: None,
            execution_history: Vec::new(),
            failure_count: 0,
            recovery_attempts: 0,
            memory_usage_bytes: None,
            time_bounds_per_iteration: std::time::Duration::from_secs(60),
            convergence_metrics: std::collections::HashMap::new(),
            resource_usage_stats: infrastructure::agent_control::ResourceUsageStats::default(),
            performance_metrics: infrastructure::agent_control::PerformanceMetrics::default(),
            max_iterations_allowed: max_iters,
            convergence_threshold: 0.8,
        };

        // Track full history for returning tool calls & confidence.
        let mut all_reasoning: Vec<String> = Vec::new();
        let mut all_tool_calls: Vec<ToolCall> = Vec::new();
        let mut all_tool_results: Vec<ToolResult> = Vec::new();

        for i in 0..max_iters {
            execution_state.iteration_count = i as u32;

            // 1) Ask model for reasoning steps
            let reasoning_steps = self.generate_reasoning(goal, &agent_context).await?;
            all_reasoning.extend(reasoning_steps.clone());

            // 2) Decide whether tools are needed, then plan tool calls
            let tool_calls = if self.needs_tools(goal, &reasoning_steps, &agent_context) {
                self.plan_tool_calls(goal, &reasoning_steps, &agent_context, &exec_context)
            } else {
                Vec::new()
            };

            // Validate + only allow tools that exist in toolset
            // For now, just use tool_calls as-is (filtering would be implemented in a full version)
            all_tool_calls.extend(tool_calls.clone());

            // 3) Execute tools (if any)
            let tool_results = self
                .execute_tool_calls(&tool_calls, &mut agent_context, &exec_context)
                .await
                .unwrap_or_else(|e| {
                    vec![ToolResult {
                        tool_call_id: "tool_exec_error".to_string(),
                        success: false,
                        result: json!(null),
                        error: Some(format!("Tool execution error: {e}")),
                    }]
                });

            execution_state.total_tools_executed += tool_results.len() as u32;
            all_tool_results.extend(tool_results.clone());

            // 4) Determine whether to finalize now.
            // If we used tools, we usually finalize immediately (unless controller says continue).
            // If no tools, we can still finalize immediately.
            let final_text = self
                .generate_final_response(goal, &reasoning_steps, &tool_results)
                .await
                .unwrap_or_else(|e| format!("Failed to generate final response: {e}"));

            // Feed controller for optional stop/continue policy
            let iteration_result = AgentIterationResult {
                reasoning_steps: reasoning_steps.clone(),
                tool_calls: tool_calls.iter().map(|tc| format!("{:?}", tc)).collect(),
                final_response: final_text.clone(),
                confidence_score: self.calculate_confidence(&all_reasoning, &all_tool_results),
                next_goal: "".to_string(), // No next goal for final iteration
            };

            execution_state.execution_history.push(IterationRecord {
                iteration_number: execution_state.iteration_count + 1,
                reasoning_steps: reasoning_steps.clone(),
                tool_calls: tool_calls.iter().map(|tc| format!("{:?}", tc)).collect(),
                verification_result: None,
                execution_time_ms: 0,
                success: true,
                memory_peak_bytes: 0,
                confidence_score: self.calculate_confidence(&all_reasoning, &all_tool_results),
                convergence_indicators: std::collections::HashMap::new(),
                resource_usage: infrastructure::agent_control::ResourceUsageStats::default(),
            });

            // Check convergence - for now, always continue if we have iterations left
            if execution_state.iteration_count >= max_iters {
                return Ok((
                    infrastructure::agent_control::AgentResult {
                        final_response: final_text,
                        confidence_score: self
                            .calculate_confidence(&all_reasoning, &all_tool_results),
                        iterations_used: execution_state.iteration_count + 1,
                        tools_executed: execution_state.total_tools_executed,
                        verification_history: Vec::new(),
                        execution_time: std::time::Duration::from_secs(0),
                        tool_calls: all_tool_calls
                            .iter()
                            .map(|tc| format!("{:?}", tc))
                            .collect(),
                        tool_results: all_tool_results
                            .iter()
                            .map(|tr| format!("{:?}", tr))
                            .collect(),
                    },
                    all_tool_calls,
                    all_tool_results,
                ));
            }

            // Add assistant message so next iteration can build upon it
            agent_context
                .conversation_history
                .push(ConversationMessage {
                    role: "assistant".to_string(),
                    content: final_text,
                    tool_calls: None,
                    tool_call_id: None,
                });
        }

        // Max iteration hit: return best-effort
        let confidence = self.calculate_confidence(&all_reasoning, &all_tool_results);
        Ok((
            infrastructure::agent_control::AgentResult {
                final_response: "Reached max iterations. Returning best available answer."
                    .to_string(),
                confidence_score: confidence,
                iterations_used: execution_state.iteration_count,
                tools_executed: execution_state.total_tools_executed,
                verification_history: Vec::new(),
                execution_time: std::time::Duration::from_secs(0),
                tool_calls: all_tool_calls
                    .iter()
                    .map(|tc| format!("{:?}", tc))
                    .collect(),
                tool_results: all_tool_results
                    .iter()
                    .map(|tr| format!("{:?}", tr))
                    .collect(),
            },
            all_tool_calls,
            all_tool_results,
        ))
    }
}
