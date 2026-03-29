use anyhow::Result;
use async_trait::async_trait;
use domain::entities::react::{ReactTool, ToolCategory, ToolResult};
use domain::entities::{Fact, Hypothesis};
use std::sync::Arc;

use crate::services::react_context_retriever::RetrievedContext;
use crate::services::react_tools::ReactToolHandler;
use crate::services::react_tools::prompts::analysis as prompts;

/// Handler for summarize tool
pub struct SummarizeHandler;

#[async_trait]
impl ReactToolHandler for SummarizeHandler {
    fn name(&self) -> &str {
        "summarize"
    }
    
    fn description(&self) -> &str {
        "Summarize output in 3-5 sentences"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Analysis
    }
    
    fn requires_output(&self) -> bool {
        true
    }
    
    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let summary = generate_summary(context);
        
        Ok(ToolResult::new(ReactTool::Summarize)
            .with_output(summary)
            .with_next_tool(ReactTool::ExtractErrors))
    }
    
    fn get_prompt(&self, context: &RetrievedContext) -> String {
        prompts::summarize_prompt(context)
    }
}

/// Handler for extract_errors tool
pub struct ExtractErrorsHandler;

#[async_trait]
impl ReactToolHandler for ExtractErrorsHandler {
    fn name(&self) -> &str {
        "extract_errors"
    }
    
    fn description(&self) -> &str {
        "Extract error messages from output"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Analysis
    }
    
    fn requires_output(&self) -> bool {
        true
    }
    
    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let errors = extract_errors(&context.latest_output);
        
        // Create facts from extracted errors
        let facts: Vec<Fact> = errors.iter().enumerate().map(|(i, error)| {
            Fact {
                key: format!("error_{}", i + 1),
                value: error.clone(),
                source_command: "extract_errors".to_string(),
                source_step: context.steps,
                verified: true,
                embedding_id: None,
            }
        }).collect();
        
        let output = if errors.is_empty() {
            "No errors found in output.".to_string()
        } else {
            format!("Found {} error(s):\n{}", 
                errors.len(),
                errors.join("\n"))
        };
        
        Ok(ToolResult::new(ReactTool::ExtractErrors)
            .with_output(output)
            .with_facts(facts)
            .with_next_tool(ReactTool::ExtractWarnings))
    }
    
    fn get_prompt(&self, context: &RetrievedContext) -> String {
        prompts::extract_errors_prompt(context)
    }
}

/// Handler for extract_warnings tool
pub struct ExtractWarningsHandler;

#[async_trait]
impl ReactToolHandler for ExtractWarningsHandler {
    fn name(&self) -> &str {
        "extract_warnings"
    }
    
    fn description(&self) -> &str {
        "Extract warnings from output"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Analysis
    }
    
    fn requires_output(&self) -> bool {
        true
    }
    
    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let warnings = extract_warnings(&context.latest_output);
        
        // Create facts from extracted warnings
        let facts: Vec<Fact> = warnings.iter().enumerate().map(|(i, warning)| {
            Fact {
                key: format!("warning_{}", i + 1),
                value: warning.clone(),
                source_command: "extract_warnings".to_string(),
                source_step: context.steps,
                verified: true,
                embedding_id: None,
            }
        }).collect();
        
        let output = if warnings.is_empty() {
            "No warnings found in output.".to_string()
        } else {
            format!("Found {} warning(s):\n{}", 
                warnings.len(),
                warnings.join("\n"))
        };
        
        Ok(ToolResult::new(ReactTool::ExtractWarnings)
            .with_output(output)
            .with_facts(facts)
            .with_next_tool(ReactTool::ExtractMetrics))
    }
    
    fn get_prompt(&self, context: &RetrievedContext) -> String {
        prompts::extract_warnings_prompt(context)
    }
}

/// Handler for extract_metrics tool
pub struct ExtractMetricsHandler;

#[async_trait]
impl ReactToolHandler for ExtractMetricsHandler {
    fn name(&self) -> &str {
        "extract_metrics"
    }
    
    fn description(&self) -> &str {
        "Extract numeric metrics from output"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Analysis
    }
    
    fn requires_output(&self) -> bool {
        true
    }
    
    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let metrics = extract_metrics(&context.latest_output);
        
        // Create facts from extracted metrics
        let facts: Vec<Fact> = metrics.iter().map(|(key, value)| {
            Fact {
                key: key.clone(),
                value: value.clone(),
                source_command: "extract_metrics".to_string(),
                source_step: context.steps,
                verified: true,
                embedding_id: None,
            }
        }).collect();
        
        let output = if metrics.is_empty() {
            "No numeric metrics found in output.".to_string()
        } else {
            format!("Extracted metrics:\n{}", 
                metrics.iter()
                    .map(|(k, v)| format!("  {}: {}", k, v))
                    .collect::<Vec<_>>()
                    .join("\n"))
        };
        
        Ok(ToolResult::new(ReactTool::ExtractMetrics)
            .with_output(output)
            .with_facts(facts)
            .with_next_tool(ReactTool::PlanNext))
    }
    
    fn get_prompt(&self, context: &RetrievedContext) -> String {
        prompts::extract_metrics_prompt(context)
    }
}

/// Handler for extract_patterns tool
pub struct ExtractPatternsHandler;

#[async_trait]
impl ReactToolHandler for ExtractPatternsHandler {
    fn name(&self) -> &str {
        "extract_patterns"
    }
    
    fn description(&self) -> &str {
        "Find patterns in data"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Analysis
    }
    
    fn requires_output(&self) -> bool {
        true
    }
    
    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let patterns = extract_patterns(&context.latest_output);
        
        // Create facts from detected patterns
        let facts: Vec<Fact> = patterns.iter().enumerate().map(|(i, pattern)| {
            Fact {
                key: format!("pattern_{}", i + 1),
                value: pattern.clone(),
                source_command: "extract_patterns".to_string(),
                source_step: context.steps,
                verified: true,
                embedding_id: None,
            }
        }).collect();
        
        let output = if patterns.is_empty() {
            "No significant patterns detected.".to_string()
        } else {
            format!("Detected patterns:\n{}", 
                patterns.join("\n"))
        };
        
        Ok(ToolResult::new(ReactTool::ExtractPatterns)
            .with_output(output)
            .with_facts(facts)
            .with_next_tool(ReactTool::Correlate))
    }
    
    fn get_prompt(&self, context: &RetrievedContext) -> String {
        prompts::extract_patterns_prompt(context)
    }
}

/// Handler for compare tool
pub struct CompareHandler;

#[async_trait]
impl ReactToolHandler for CompareHandler {
    fn name(&self) -> &str {
        "compare"
    }
    
    fn description(&self) -> &str {
        "Compare two outputs or states"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Analysis
    }
    
    fn requires_output(&self) -> bool {
        true
    }
    
    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        // Compare current output with previous if available
        let comparison = generate_comparison(context);
        
        Ok(ToolResult::new(ReactTool::Compare)
            .with_output(comparison)
            .with_next_tool(ReactTool::CheckGoal))
    }
    
    fn get_prompt(&self, context: &RetrievedContext) -> String {
        prompts::compare_prompt(context)
    }
}

/// Handler for correlate tool
pub struct CorrelateHandler;

#[async_trait]
impl ReactToolHandler for CorrelateHandler {
    fn name(&self) -> &str {
        "correlate"
    }
    
    fn description(&self) -> &str {
        "Find relationships in data"
    }
    
    fn category(&self) -> ToolCategory {
        ToolCategory::Analysis
    }
    
    fn requires_output(&self) -> bool {
        true
    }
    
    async fn execute(&self, context: &RetrievedContext, _params: Option<&str>) -> Result<ToolResult> {
        let correlations = find_correlations(context);
        
        // Create hypotheses from detected correlations
        let hypotheses: Vec<Hypothesis> = correlations.iter().enumerate().map(|(_i, corr)| {
            Hypothesis {
                description: corr.clone(),
                confidence: 0.7,
                supporting_facts: vec![],
                created_at: chrono::Utc::now(),
            }
        }).collect();
        
        let output = if correlations.is_empty() {
            "No obvious correlations detected in current data.".to_string()
        } else {
            format!("Detected correlations:\n{}", correlations.join("\n"))
        };
        
        Ok(ToolResult::new(ReactTool::Correlate)
            .with_output(output)
            .with_hypotheses(hypotheses)
            .with_next_tool(ReactTool::CheckGoal))
    }
    
    fn get_prompt(&self, context: &RetrievedContext) -> String {
        prompts::correlate_prompt(context)
    }
}

// Helper functions

fn generate_summary(context: &RetrievedContext) -> String {
    let output = &context.latest_output;
    
    // Simple heuristics for summarization
    let lines: Vec<&str> = output.lines().collect();
    
    if lines.len() <= 5 {
        return output.clone();
    }
    
    // Take first few meaningful lines
    let mut summary_parts = Vec::new();
    
    // Include first line if not empty
    if let Some(first) = lines.first() {
        if !first.trim().is_empty() {
            summary_parts.push(first.to_string());
        }
    }
    
    // Look for key information (lines with colons, numbers, or keywords)
    for line in lines.iter().take(20).skip(1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        
        // Lines with key information
        if trimmed.contains(':') || 
           trimmed.contains("Error") ||
           trimmed.contains("error") ||
           trimmed.contains("FAIL") ||
           trimmed.contains("PASS") ||
           trimmed.contains("status") {
            summary_parts.push(trimmed.to_string());
        }
        
        if summary_parts.len() >= 5 {
            break;
        }
    }
    
    if summary_parts.is_empty() {
        // Fallback: just take first few lines
        summary_parts = lines.iter().take(3).map(|s| s.to_string()).collect();
    }
    
    summary_parts.join("\n")
}

fn extract_errors(output: &str) -> Vec<String> {
    let mut errors = Vec::new();
    
    for line in output.lines() {
        let lower = line.to_lowercase();
        if lower.contains("error") || 
           lower.contains("fatal") ||
           lower.contains("failed") ||
           lower.contains("exception") ||
           lower.contains("panic") ||
           lower.contains("crash") ||
           lower.contains("denied") ||
           lower.contains("permission denied") ||
           lower.contains("not found") && lower.contains("error") ||
           lower.contains("could not") ||
           lower.contains("unable to") ||
           line.starts_with("E") && line.chars().nth(1).map(|c| c.is_ascii_digit()).unwrap_or(false) {
            errors.push(line.trim().to_string());
        }
    }
    
    errors
}

fn extract_warnings(output: &str) -> Vec<String> {
    let mut warnings = Vec::new();
    
    for line in output.lines() {
        let lower = line.to_lowercase();
        if lower.contains("warning") ||
           lower.contains("deprecated") ||
           lower.contains("unused") ||
           lower.contains("obsolete") ||
           lower.contains("caution") ||
           lower.contains("attention") ||
           line.starts_with("W") && line.chars().nth(1).map(|c| c.is_ascii_digit()).unwrap_or(false) {
            warnings.push(line.trim().to_string());
        }
    }
    
    warnings
}

fn extract_metrics(output: &str) -> Vec<(String, String)> {
    let mut metrics = Vec::new();
    
    // Look for patterns like "key: value" or "key=value" with numeric values
    for line in output.lines() {
        // Pattern: Key: 123 or Key: 12.5MB, etc.
        if let Some(pos) = line.find(':') {
            let key = line[..pos].trim();
            let value_part = &line[pos + 1..];
            
            // Check if value contains numbers
            if value_part.chars().any(|c| c.is_ascii_digit()) {
                metrics.push((key.to_string(), value_part.trim().to_string()));
            }
        }
        
        // Pattern: Key=123
        if let Some(pos) = line.find('=') {
            let key = line[..pos].trim();
            let value_part = &line[pos + 1..];
            
            if value_part.chars().any(|c| c.is_ascii_digit()) {
                // Avoid duplicates
                if !metrics.iter().any(|(k, _)| k == key) {
                    metrics.push((key.to_string(), value_part.trim().to_string()));
                }
            }
        }
    }
    
    metrics
}

fn extract_patterns(output: &str) -> Vec<String> {
    let mut patterns = Vec::new();
    let lines: Vec<&str> = output.lines().collect();
    
    // Look for repeated patterns
    let mut line_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    
    for line in &lines {
        let normalized = normalize_line(line);
        if !normalized.is_empty() && normalized.len() > 10 {
            *line_counts.entry(normalized).or_insert(0) += 1;
        }
    }
    
    // Report patterns that appear multiple times
    for (pattern, count) in line_counts {
        if count > 1 {
            patterns.push(format!("Repeated pattern ({}x): {}", count, &pattern[..pattern.len().min(80)]));
        }
    }
    
    // Look for sequential patterns
    if lines.len() >= 3 {
        patterns.push(format!("Output contains {} lines", lines.len()));
    }
    
    patterns
}

fn normalize_line(line: &str) -> String {
    // Remove variable parts (numbers, timestamps, etc.)
    line.chars()
        .map(|c| if c.is_ascii_digit() { 'X' } else { c })
        .collect::<String>()
        .trim()
        .to_string()
}

fn generate_comparison(context: &RetrievedContext) -> String {
    let current = &context.latest_output;
    
    // Simple comparison logic - in real implementation, would compare with stored state
    let lines_current = current.lines().count();
    
    format!(
        "Current output analysis:\n  - Total lines: {}\n  - Contains errors: {}\n  - Contains warnings: {}",
        lines_current,
        !extract_errors(current).is_empty(),
        !extract_warnings(current).is_empty()
    )
}

fn find_correlations(context: &RetrievedContext) -> Vec<String> {
    let mut correlations = Vec::new();
    let out_lower = context.latest_output.to_lowercase();
    
    // Correlate facts with current output
    // Facts is a string, so we check if any lines from facts appear in output
    for fact_line in context.facts.lines() {
        let fact_lower = fact_line.to_lowercase();
        if !fact_lower.is_empty() && out_lower.contains(&fact_lower) {
            correlations.push(format!(
                "Fact '{}' appears in current output",
                fact_line.trim()
            ));
        }
    }
    
    // Check hypotheses against output
    for hypothesis_line in context.hypotheses.lines() {
        let hyp_lower = hypothesis_line.to_lowercase();
        
        // Simple keyword matching
        let keywords: Vec<&str> = hyp_lower.split_whitespace().filter(|w: &&str| w.len() > 4).collect();
        let matches = keywords.iter().filter(|k| out_lower.contains(*k)).count();
        
        if matches > 0 {
            correlations.push(format!(
                "Hypothesis '{}' supported by {} keyword matches in output",
                hypothesis_line.trim(),
                matches
            ));
        }
    }
    
    correlations
}

/// Build the default analysis tool handlers
pub fn build_analysis_handlers() -> Vec<(ReactTool, Arc<dyn ReactToolHandler>)> {
    vec![
        (ReactTool::Summarize, Arc::new(SummarizeHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::ExtractErrors, Arc::new(ExtractErrorsHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::ExtractWarnings, Arc::new(ExtractWarningsHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::ExtractMetrics, Arc::new(ExtractMetricsHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::ExtractPatterns, Arc::new(ExtractPatternsHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::Compare, Arc::new(CompareHandler) as Arc<dyn ReactToolHandler>),
        (ReactTool::Correlate, Arc::new(CorrelateHandler) as Arc<dyn ReactToolHandler>),
    ]
}
