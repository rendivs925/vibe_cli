use anyhow::Result;
use regex::Regex;

/// Error analysis and fix generation engine
#[derive(Clone)]
pub struct ErrorAnalyzer;

#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub error_type: ErrorType,
    pub message: String,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub context: String,
    pub severity: ErrorSeverity,
}

#[derive(Debug, Clone)]
pub enum ErrorType {
    CompilationError,
    RuntimeError,
    TestFailure,
    LspDiagnostic,
    LogError,
}

#[derive(Debug, Clone)]
pub enum ErrorSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone)]
pub struct FixSuggestion {
    pub description: String,
    pub confidence: f32, // 0.0 to 1.0
    pub changes: Vec<CodeChange>,
    pub explanation: String,
}

#[derive(Debug, Clone)]
pub struct CodeChange {
    pub file_path: String,
    pub line_start: u32,
    pub line_end: u32,
    pub old_code: String,
    pub new_code: String,
}

impl ErrorAnalyzer {
    /// Analyze an error and generate fix suggestions
    pub async fn analyze_and_fix(
        &self,
        error: ErrorContext,
        project_root: &std::path::Path,
    ) -> Result<Vec<FixSuggestion>> {
        match error.error_type {
            ErrorType::CompilationError => {
                self.analyze_compilation_error(error, project_root).await
            }
            ErrorType::TestFailure => self.analyze_test_failure(error, project_root).await,
            ErrorType::LspDiagnostic => self.analyze_lsp_diagnostic(error, project_root).await,
            ErrorType::LogError => self.analyze_log_error(error, project_root).await,
            ErrorType::RuntimeError => self.analyze_runtime_error(error, project_root).await,
        }
    }

    /// Analyze compilation errors and suggest fixes
    async fn analyze_compilation_error(
        &self,
        error: ErrorContext,
        _project_root: &std::path::Path,
    ) -> Result<Vec<FixSuggestion>> {
        let mut suggestions = Vec::new();

        // Parse common Rust compilation errors
        if error.message.contains("cannot find") && error.message.contains("in this scope") {
            suggestions.push(self.fix_undefined_variable(error.clone()));
        }

        if error.message.contains("expected") && error.message.contains("found") {
            suggestions.push(self.fix_type_mismatch(error.clone()));
        }

        if error.message.contains("borrowed") && error.message.contains("moved") {
            suggestions.push(self.fix_borrow_checker(error.clone()));
        }

        if error.message.contains("unused") && error.message.contains("variable") {
            suggestions.push(self.fix_unused_variable(error.clone()));
        }

        Ok(suggestions)
    }

    /// Analyze test failures
    async fn analyze_test_failure(
        &self,
        error: ErrorContext,
        _project_root: &std::path::Path,
    ) -> Result<Vec<FixSuggestion>> {
        let mut suggestions = Vec::new();

        if error.message.contains("assertion failed") {
            suggestions.push(self.fix_assertion_failure(error.clone()));
        }

        if error.message.contains("panic") {
            suggestions.push(self.fix_panic(error.clone()));
        }

        Ok(suggestions)
    }

    /// Analyze LSP diagnostics
    async fn analyze_lsp_diagnostic(
        &self,
        error: ErrorContext,
        _project_root: &std::path::Path,
    ) -> Result<Vec<FixSuggestion>> {
        let mut suggestions = Vec::new();

        // LSP-specific fixes
        if error.message.contains("unused import") {
            suggestions.push(self.fix_unused_import(error.clone()));
        }

        if error.message.contains("missing") && error.message.contains("documentation") {
            suggestions.push(self.fix_missing_docs(error.clone()));
        }

        Ok(suggestions)
    }

    /// Analyze log errors
    async fn analyze_log_error(
        &self,
        error: ErrorContext,
        _project_root: &std::path::Path,
    ) -> Result<Vec<FixSuggestion>> {
        let mut suggestions = Vec::new();

        if error.message.contains("connection") && error.message.contains("failed") {
            suggestions.push(self.fix_connection_error(error.clone()));
        }

        if error.message.contains("timeout") {
            suggestions.push(self.fix_timeout_error(error.clone()));
        }

        Ok(suggestions)
    }

    /// Analyze runtime errors
    async fn analyze_runtime_error(
        &self,
        error: ErrorContext,
        _project_root: &std::path::Path,
    ) -> Result<Vec<FixSuggestion>> {
        let mut suggestions = Vec::new();

        if error.message.contains("index out of bounds") {
            suggestions.push(self.fix_index_out_of_bounds(error.clone()));
        }

        if error.message.contains("null pointer") {
            suggestions.push(self.fix_null_pointer(error.clone()));
        }

        Ok(suggestions)
    }

    // Specific fix implementations

    fn fix_undefined_variable(&self, error: ErrorContext) -> FixSuggestion {
        let var_name = Self::extract_variable_name(&error.message);
        let suggestion = format!("Add import or define variable `{}`", var_name);

        FixSuggestion {
            description: "Import missing variable or define it".to_string(),
            confidence: 0.8,
            changes: vec![], // Would need more context to generate actual code changes
            explanation: format!("The variable `{}` is not defined in the current scope. Consider adding an import statement or defining the variable.", var_name),
        }
    }

    fn fix_type_mismatch(&self, error: ErrorContext) -> FixSuggestion {
        FixSuggestion {
            description: "Fix type mismatch by converting types".to_string(),
            confidence: 0.7,
            changes: vec![],
            explanation: "The compiler found a type mismatch. Consider using type conversion methods like `as`, `into()`, or `try_into()`.".to_string(),
        }
    }

    fn fix_borrow_checker(&self, error: ErrorContext) -> FixSuggestion {
        FixSuggestion {
            description: "Fix borrow checker error by cloning or restructuring ownership".to_string(),
            confidence: 0.6,
            changes: vec![],
            explanation: "Rust's borrow checker prevents multiple mutable references. Consider cloning the value or restructuring the code to avoid ownership conflicts.".to_string(),
        }
    }

    fn fix_unused_variable(&self, error: ErrorContext) -> FixSuggestion {
        let var_name = Self::extract_variable_name(&error.message);

        FixSuggestion {
            description: format!("Remove or use unused variable `{}`", var_name),
            confidence: 0.9,
            changes: vec![],
            explanation: format!("Variable `{}` is declared but never used. Either remove it or prefix with underscore if intentional.", var_name),
        }
    }

    fn fix_assertion_failure(&self, error: ErrorContext) -> FixSuggestion {
        FixSuggestion {
            description: "Fix failing assertion by correcting the logic".to_string(),
            confidence: 0.5,
            changes: vec![],
            explanation:
                "The test assertion failed. Review the test logic and expected vs actual values."
                    .to_string(),
        }
    }

    fn fix_panic(&self, error: ErrorContext) -> FixSuggestion {
        FixSuggestion {
            description: "Handle panic condition gracefully".to_string(),
            confidence: 0.7,
            changes: vec![],
            explanation: "The code panicked instead of handling the error gracefully. Consider using Result types or proper error handling.".to_string(),
        }
    }

    fn fix_unused_import(&self, error: ErrorContext) -> FixSuggestion {
        FixSuggestion {
            description: "Remove unused import".to_string(),
            confidence: 0.95,
            changes: vec![],
            explanation: "An import statement is not being used. Remove it to clean up the code."
                .to_string(),
        }
    }

    fn fix_missing_docs(&self, error: ErrorContext) -> FixSuggestion {
        FixSuggestion {
            description: "Add documentation comment".to_string(),
            confidence: 0.9,
            changes: vec![],
            explanation: "Public items should be documented. Add a `///` comment above the item."
                .to_string(),
        }
    }

    fn fix_connection_error(&self, error: ErrorContext) -> FixSuggestion {
        FixSuggestion {
            description: "Add connection error handling and retry logic".to_string(),
            confidence: 0.6,
            changes: vec![],
            explanation:
                "Connection failures should be handled with retries and proper error messages."
                    .to_string(),
        }
    }

    fn fix_timeout_error(&self, error: ErrorContext) -> FixSuggestion {
        FixSuggestion {
            description: "Increase timeout or add async handling".to_string(),
            confidence: 0.5,
            changes: vec![],
            explanation: "Timeout errors indicate operations taking too long. Consider increasing timeouts or using async operations.".to_string(),
        }
    }

    fn fix_index_out_of_bounds(&self, error: ErrorContext) -> FixSuggestion {
        FixSuggestion {
            description: "Add bounds checking before array access".to_string(),
            confidence: 0.8,
            changes: vec![],
            explanation: "Array access without bounds checking can cause panics. Use bounds checking or safer access methods.".to_string(),
        }
    }

    fn fix_null_pointer(&self, error: ErrorContext) -> FixSuggestion {
        FixSuggestion {
            description: "Handle null/None values properly".to_string(),
            confidence: 0.8,
            changes: vec![],
            explanation: "Dereferencing null pointers causes runtime errors. Use Option types and proper null checking.".to_string(),
        }
    }

    /// Extract variable name from error message using regex
    fn extract_variable_name(error_msg: &str) -> String {
        // Try to extract variable names from common error patterns
        let patterns = [
            r"cannot find value `([^`]+)`",
            r"cannot find `([^`]+)`",
            r"unused variable: `([^`]+)`",
            r"variable `([^`]+)` is",
        ];

        for pattern in &patterns {
            if let Ok(regex) = Regex::new(pattern) {
                if let Some(captures) = regex.captures(error_msg) {
                    if let Some(var_name) = captures.get(1) {
                        return var_name.as_str().to_string();
                    }
                }
            }
        }

        "unknown".to_string()
    }
}

/// Convert background events to error contexts for analysis
impl From<&super::background_supervisor::BackgroundEvent> for ErrorContext {
    fn from(event: &super::background_supervisor::BackgroundEvent) -> Self {
        match event {
            super::background_supervisor::BackgroundEvent::LspDiagnostic {
                file,
                severity,
                message,
            } => ErrorContext {
                error_type: ErrorType::LspDiagnostic,
                message: message.clone(),
                file: Some(file.to_string_lossy().to_string()),
                line: None,
                column: None,
                context: "LSP diagnostic".to_string(),
                severity: match severity {
                    super::background_supervisor::DiagnosticSeverity::Error => ErrorSeverity::High,
                    super::background_supervisor::DiagnosticSeverity::Warning => {
                        ErrorSeverity::Medium
                    }
                    super::background_supervisor::DiagnosticSeverity::Information => {
                        ErrorSeverity::Low
                    }
                    super::background_supervisor::DiagnosticSeverity::Hint => ErrorSeverity::Low,
                },
            },
            super::background_supervisor::BackgroundEvent::TestResult {
                session,
                status,
                output,
            } => {
                if let super::background_supervisor::TestStatus::Failed { error } = status {
                    ErrorContext {
                        error_type: ErrorType::TestFailure,
                        message: error.clone(),
                        file: None,
                        line: None,
                        column: None,
                        context: format!("Test session: {}", session),
                        severity: ErrorSeverity::High,
                    }
                } else {
                    // Not an error
                    ErrorContext {
                        error_type: ErrorType::TestFailure,
                        message: output.clone(),
                        file: None,
                        line: None,
                        column: None,
                        context: format!("Test session: {}", session),
                        severity: ErrorSeverity::Low,
                    }
                }
            }
            super::background_supervisor::BackgroundEvent::LogEntry {
                source,
                level,
                message,
            } => {
                let severity = match level {
                    super::background_supervisor::LogLevel::Error => ErrorSeverity::High,
                    super::background_supervisor::LogLevel::Warn => ErrorSeverity::Medium,
                    super::background_supervisor::LogLevel::Info => ErrorSeverity::Low,
                    super::background_supervisor::LogLevel::Debug => ErrorSeverity::Low,
                };

                ErrorContext {
                    error_type: ErrorType::LogError,
                    message: message.clone(),
                    file: None,
                    line: None,
                    column: None,
                    context: format!("Log source: {}", source),
                    severity,
                }
            }
            _ => ErrorContext {
                error_type: ErrorType::RuntimeError,
                message: "Unknown error".to_string(),
                file: None,
                line: None,
                column: None,
                context: "Unknown context".to_string(),
                severity: ErrorSeverity::Low,
            },
        }
    }
}
