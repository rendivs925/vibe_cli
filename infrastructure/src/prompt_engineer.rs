use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

/// Structured prompt engineering for zero-cost AI workflows
/// Generates context-aware prompts for ChatGPT queries
pub struct PromptEngineer {
    project_root: Option<String>,
    context_cache: HashMap<String, String>,
}

#[derive(Debug)]
pub struct StructuredPrompt {
    pub system_prompt: String,
    pub user_prompt: String,
    pub context_attachments: Vec<String>,
}

impl PromptEngineer {
    /// Create a new prompt engineer
    pub fn new() -> Self {
        Self {
            project_root: None,
            context_cache: HashMap::new(),
        }
    }

    /// Set the project root for context gathering
    pub fn with_project_root(mut self, root: String) -> Self {
        self.project_root = Some(root);
        self
    }

    /// Generate a structured prompt for a given goal
    pub async fn generate_prompt(&mut self, goal: &str) -> Result<StructuredPrompt> {
        // Determine the type of request
        let request_type = self.classify_request(goal);

        // Gather relevant context
        let context = self.gather_context(&request_type, goal).await?;

        // Generate system prompt based on request type
        let system_prompt = self.generate_system_prompt(&request_type);

        // Generate user prompt with context
        let user_prompt = self.generate_user_prompt(goal, &context, &request_type);

        Ok(StructuredPrompt {
            system_prompt,
            user_prompt,
            context_attachments: context.file_contents,
        })
    }

    /// Gather relevant context for the request
    async fn gather_context(&mut self, request_type: &RequestType, goal: &str) -> Result<PromptContext> {
        let mut context = PromptContext::default();

        // Add project context
        if let Some(root) = &self.project_root {
            // Gather project structure
            context.project_info = self.get_project_info(root)?;

            // Gather relevant files based on request type
            context.file_contents = self.gather_relevant_files(root, request_type, goal).await?;
        }

        // Add request-specific context
        context.request_type = request_type.clone();
        context.original_goal = goal.to_string();

        Ok(context)
    }

    /// Classify the type of request to tailor the prompt
    fn classify_request(&self, goal: &str) -> RequestType {
        let goal_lower = goal.to_lowercase();

        if goal_lower.contains("analyze") || goal_lower.contains("review") || goal_lower.contains("audit") {
            RequestType::Analysis
        } else if goal_lower.contains("fix") || goal_lower.contains("bug") || goal_lower.contains("error") {
            RequestType::BugFix
        } else if goal_lower.contains("implement") || goal_lower.contains("add") || goal_lower.contains("create") {
            RequestType::Implementation
        } else if goal_lower.contains("optimize") || goal_lower.contains("performance") || goal_lower.contains("speed") {
            RequestType::Optimization
        } else if goal_lower.contains("test") || goal_lower.contains("testing") {
            RequestType::Testing
        } else if goal_lower.contains("document") || goal_lower.contains("docs") || goal_lower.contains("readme") {
            RequestType::Documentation
        } else {
            RequestType::General
        }
    }



        // Add request-specific context
        context.request_type = request_type.clone();
        context.original_goal = goal.to_string();

        Ok(context)
    }

    /// Get basic project information
    fn get_project_info(&self, root: &str) -> Result<ProjectInfo> {
        let mut info = ProjectInfo::default();

        // Check for common project files
        let project_files = vec![
            "Cargo.toml", "package.json", "requirements.txt", "pyproject.toml",
            "Makefile", "Dockerfile", "README.md", ".gitignore"
        ];

        for file in project_files {
            let path = Path::new(root).join(file);
            if path.exists() {
                info.project_files.push(file.to_string());
            }
        }

        // Try to determine project type
        if Path::new(root).join("Cargo.toml").exists() {
            info.project_type = "Rust".to_string();
        } else if Path::new(root).join("package.json").exists() {
            info.project_type = "JavaScript/TypeScript".to_string();
        } else if Path::new(root).join("requirements.txt").exists() || Path::new(root).join("pyproject.toml").exists() {
            info.project_type = "Python".to_string();
        }

        Ok(info)
    }

    /// Gather relevant files based on request type and goal
    async fn gather_relevant_files(&mut self, root: &str, request_type: &RequestType, goal: &str) -> Result<Vec<String>> {
        let mut relevant_files = Vec::new();

        // Always include key project files
        let key_files = vec!["README.md", "Cargo.toml", "package.json"];
        for file in key_files {
            if let Ok(content) = self.read_file_safe(root, file).await {
                relevant_files.push(format!("=== {} ===\n{}", file, content));
            }
        }

        // Include files based on request type
        match request_type {
            RequestType::BugFix | RequestType::Analysis => {
                // Look for error logs, test files, main source files
                let patterns = vec!["*.rs", "*.js", "*.ts", "*.py", "*test*", "*error*", "*log*"];
                for pattern in patterns {
                    if let Ok(files) = self.find_files_by_pattern(root, pattern).await {
                        for file in files.into_iter().take(3) { // Limit to 3 files per pattern
                            if let Ok(content) = self.read_file_safe(root, &file).await {
                                relevant_files.push(format!("=== {} ===\n{}", file, content));
                            }
                        }
                    }
                }
            }
            RequestType::Implementation => {
                // Look for main source files and recent changes
                let patterns = vec!["src/*.rs", "lib/*.js", "*.py"];
                for pattern in patterns {
                    if let Ok(files) = self.find_files_by_pattern(root, pattern).await {
                        for file in files.into_iter().take(2) {
                            if let Ok(content) = self.read_file_safe(root, &file).await {
                                relevant_files.push(format!("=== {} ===\n{}", file, content));
                            }
                        }
                    }
                }
            }
            _ => {
                // For other types, include a few main files
                let patterns = vec!["*.rs", "*.js", "*.py"];
                for pattern in patterns {
                    if let Ok(files) = self.find_files_by_pattern(root, pattern).await {
                        for file in files.into_iter().take(1) {
                            if let Ok(content) = self.read_file_safe(root, &file).await {
                                relevant_files.push(format!("=== {} ===\n{}", file, content));
                            }
                        }
                    }
                }
            }
        }

        Ok(relevant_files)
    }

    /// Safely read a file with error handling
    async fn read_file_safe(&self, root: &str, file_path: &str) -> Result<String> {
        let full_path = Path::new(root).join(file_path);

        // Check if file exists and is not too large
        let metadata = tokio::fs::metadata(&full_path).await?;
        if metadata.len() > 100_000 { // 100KB limit
            return Ok(format!("[File too large: {} bytes]", metadata.len()));
        }

        let content = tokio::fs::read_to_string(&full_path).await?;
        Ok(content)
    }

    /// Find files matching a pattern (simplified implementation)
    async fn find_files_by_pattern(&self, root: &str, pattern: &str) -> Result<Vec<String>> {
        let mut files = Vec::new();

        // Simple implementation - in a real system, use walkdir or similar
        let entries = std::fs::read_dir(root)?;
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_file() {
                    if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                        // Simple pattern matching
                        if Self::matches_pattern(file_name, pattern) {
                            if let Some(relative_path) = path.strip_prefix(root).ok().and_then(|p| p.to_str()) {
                                files.push(relative_path.to_string());
                            }
                        }
                    }
                }
            }
        }

        Ok(files)
    }

    /// Simple pattern matching (supports * wildcards)
    fn matches_pattern(file_name: &str, pattern: &str) -> bool {
        if pattern.contains('*') {
            let regex_pattern = pattern.replace('.', r"\.").replace('*', ".*");
            regex::Regex::new(&format!("^{}$", regex_pattern)).unwrap().is_match(file_name)
        } else {
            file_name == pattern
        }
    }

    /// Generate system prompt based on request type
    fn generate_system_prompt(&self, request_type: &RequestType) -> String {
        let base_prompt = "You are an expert software engineer with deep knowledge of best practices, design patterns, and modern development tools. You provide clear, actionable advice and high-quality code solutions.";

        match request_type {
            RequestType::Analysis => format!("{} Focus on code analysis, identifying potential issues, security vulnerabilities, performance bottlenecks, and maintainability concerns. Provide specific recommendations with code examples.", base_prompt),
            RequestType::BugFix => format!("{} You excel at debugging and fixing software issues. Analyze error messages, stack traces, and code to identify root causes and provide precise fixes.", base_prompt),
            RequestType::Implementation => format!("{} You are skilled at implementing new features following clean code principles, proper error handling, and comprehensive testing. Provide complete, production-ready code.", base_prompt),
            RequestType::Optimization => format!("{} You specialize in performance optimization, algorithmic improvements, and efficient resource usage. Focus on measurable performance gains and scalability.", base_prompt),
            RequestType::Testing => format!("{} You are an expert in software testing strategies, unit tests, integration tests, and test-driven development. Provide comprehensive test coverage and testing best practices.", base_prompt),
            RequestType::Documentation => format!("{} You excel at technical writing, API documentation, code comments, and user guides. Create clear, comprehensive documentation that helps developers understand and use the code.", base_prompt),
            RequestType::General => base_prompt.to_string(),
        }
    }

    /// Generate user prompt with context
    fn generate_user_prompt(&self, goal: &str, context: &PromptContext, request_type: &RequestType) -> String {
        let mut prompt = format!("Goal: {}\n\n", goal);

        // Add project context
        if !context.project_info.project_type.is_empty() {
            prompt.push_str(&format!("Project Type: {}\n", context.project_info.project_type));
        }

        if !context.project_info.project_files.is_empty() {
            prompt.push_str(&format!("Key Files: {}\n", context.project_info.project_files.join(", ")));
        }

        // Add context files if available
        if !context.file_contents.is_empty() {
            prompt.push_str("\nRelevant Code Context:\n");
            for (i, content) in context.file_contents.iter().enumerate() {
                if content.len() > 2000 { // Truncate very long files
                    prompt.push_str(&format!("File {}: [Content truncated - {} chars]\n", i + 1, content.len()));
                } else {
                    prompt.push_str(&format!("File {}: {}\n", i + 1, content));
                }
            }
        }

        // Add request-specific instructions
        match request_type {
            RequestType::Analysis => {
                prompt.push_str("\nPlease analyze the provided code and context. Focus on:
- Code quality and maintainability
- Potential bugs or security issues
- Performance considerations
- Best practices compliance
- Specific recommendations for improvement");
            }
            RequestType::BugFix => {
                prompt.push_str("\nPlease help fix the reported issue. Provide:
- Root cause analysis
- Specific code changes needed
- Testing steps to verify the fix
- Prevention measures for similar issues");
            }
            RequestType::Implementation => {
                prompt.push_str("\nPlease implement the requested feature. Provide:
- Complete, production-ready code
- Proper error handling
- Unit tests
- Documentation/comments
- Integration considerations");
            }
            RequestType::Optimization => {
                prompt.push_str("\nPlease optimize the code for better performance. Focus on:
- Algorithmic improvements
- Memory usage optimization
- I/O efficiency
- Concurrent processing where applicable
- Measurable performance metrics");
            }
            RequestType::Testing => {
                prompt.push_str("\nPlease create comprehensive tests. Include:
- Unit tests for individual functions
- Integration tests for components
- Edge cases and error conditions
- Test data and fixtures
- Test coverage analysis");
            }
            RequestType::Documentation => {
                prompt.push_str("\nPlease create comprehensive documentation. Include:
- Function/class purpose and usage
- Parameter descriptions
- Return value documentation
- Examples and code snippets
- Important notes and warnings");
            }
            RequestType::General => {
                prompt.push_str("\nPlease provide a helpful, accurate response based on the context provided.");
            }
        }

        prompt
    }
}

#[derive(Debug, Clone)]
enum RequestType {
    Analysis,
    BugFix,
    Implementation,
    Optimization,
    Testing,
    Documentation,
    #[default]
    General,
}

impl Default for RequestType {
    fn default() -> Self {
        RequestType::General
    }
}

#[derive(Debug, Default)]
struct PromptContext {
    project_info: ProjectInfo,
    file_contents: Vec<String>,
    request_type: RequestType,
    original_goal: String,
}

#[derive(Debug, Default)]
struct ProjectInfo {
    project_type: String,
    project_files: Vec<String>,
}