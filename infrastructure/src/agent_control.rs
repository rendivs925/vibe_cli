use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::time::timeout;

/// Bounded agent execution with automated verification
pub struct AgentController {
    max_iterations: u32,
    max_tools_per_iteration: u32,
    max_execution_time: Duration,
    verification_enabled: bool,
    failure_recovery_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentExecutionLimits {
    pub max_iterations: u32,
    pub max_tools_per_iteration: u32,
    pub max_execution_time_seconds: u64,
    pub verification_timeout_seconds: u64,
    pub allow_iteration_on_failure: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentExecutionState {
    pub iteration_count: u32,
    pub total_tools_executed: u32,
    pub start_time: std::time::SystemTime,
    pub last_verification_result: Option<VerificationResult>,
    pub execution_history: Vec<IterationRecord>,
    pub failure_count: u32,
    pub recovery_attempts: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IterationRecord {
    pub iteration_number: u32,
    pub reasoning_steps: Vec<String>,
    pub tool_calls: Vec<String>, // Simplified for now
    pub verification_result: Option<VerificationResult>,
    pub execution_time_ms: u64,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VerificationResult {
    Passed { confidence_score: f32, checks_performed: Vec<String> },
    Failed { reason: String, failed_checks: Vec<String> },
    Inconclusive { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentExecutionStatus {
    Running,
    Completed { final_result: AgentResult },
    Failed { error: String, can_retry: bool },
    Terminated { reason: String },
    NeedsApproval { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    pub final_response: String,
    pub confidence_score: f32,
    pub iterations_used: u32,
    pub tools_executed: u32,
    pub verification_history: Vec<VerificationResult>,
    pub execution_time: Duration,
}

impl Default for AgentExecutionLimits {
    fn default() -> Self {
        Self {
            max_iterations: 5,
            max_tools_per_iteration: 3,
            max_execution_time_seconds: 120, // 2 minutes
            verification_timeout_seconds: 30,
            allow_iteration_on_failure: true,
        }
    }
}

impl AgentController {
    pub fn new() -> Self {
        Self {
            max_iterations: 5,
            max_tools_per_iteration: 3,
            max_execution_time: Duration::from_secs(120),
            verification_enabled: true,
            failure_recovery_enabled: true,
        }
    }

    pub fn with_limits(limits: AgentExecutionLimits) -> Self {
        Self {
            max_iterations: limits.max_iterations,
            max_tools_per_iteration: limits.max_tools_per_iteration,
            max_execution_time: Duration::from_secs(limits.max_execution_time_seconds),
            verification_enabled: true,
            failure_recovery_enabled: limits.allow_iteration_on_failure,
        }
    }

    /// Execute agent with bounded loops and automated verification
    pub async fn execute_bounded_agent<F, Fut>(
        &self,
        initial_goal: &str,
        agent_executor: F,
    ) -> Result<AgentResult, AgentError>
    where
        F: Fn(&str, &AgentExecutionState) -> Fut,
        Fut: std::future::Future<Output = Result<AgentIterationResult, AgentError>>,
    {
        let start_time = Instant::now();
        let mut state = AgentExecutionState {
            iteration_count: 0,
            total_tools_executed: 0,
            start_time: std::time::SystemTime::now(),
            last_verification_result: None,
            execution_history: Vec::new(),
            failure_count: 0,
            recovery_attempts: 0,
        };

        let mut current_goal = initial_goal.to_string();
        let mut last_result: Option<AgentIterationResult> = None;

        // Main execution loop with bounds
        while state.iteration_count < self.max_iterations {
            state.iteration_count += 1;

            // Check total execution time
            if start_time.elapsed() > self.max_execution_time {
                return Err(AgentError::Timeout(format!(
                    "Agent execution exceeded time limit: {} seconds",
                    self.max_execution_time.as_secs()
                )));
            }

            // Execute one iteration
            let iteration_start = Instant::now();
            let iteration_result = match timeout(
                Duration::from_secs(60), // 1 minute per iteration
                agent_executor(&current_goal, &state)
            ).await {
                Ok(result) => result?,
                Err(_) => {
                    state.failure_count += 1;
                    return Err(AgentError::IterationTimeout(state.iteration_count));
                }
            };

            let execution_time = iteration_start.elapsed();

            // Validate iteration result
            if iteration_result.tool_calls.len() > self.max_tools_per_iteration as usize {
                return Err(AgentError::TooManyTools(
                    iteration_result.tool_calls.len(),
                    self.max_tools_per_iteration as usize,
                ));
            }

            state.total_tools_executed += iteration_result.tool_calls.len() as u32;

            // Perform automated verification if enabled
            let verification_result = if self.verification_enabled {
                match self.verify_iteration_result(&iteration_result, &state).await {
                    Ok(result) => Some(result),
                    Err(e) => {
                        eprintln!("Verification failed: {}", e);
                        None
                    }
                }
            } else {
                None
            };

            // Record iteration
            let reasoning_steps_clone = iteration_result.reasoning_steps.clone();
            let tool_calls_clone = iteration_result.tool_calls.clone();

            let record = IterationRecord {
                iteration_number: state.iteration_count,
                reasoning_steps: reasoning_steps_clone,
                tool_calls: tool_calls_clone.iter().map(|tc| format!("{:?}", tc)).collect(),
                verification_result: verification_result.clone(),
                execution_time_ms: execution_time.as_millis() as u64,
                success: verification_result.as_ref().map_or(false, |v| matches!(v, VerificationResult::Passed { .. })),
            };

            state.execution_history.push(record);
            state.last_verification_result = verification_result;

            // Check if we should continue iterating
            match self.should_continue_iterating(&iteration_result, &state) {
                IterationDecision::Continue(new_goal) => {
                    current_goal = new_goal;
                    last_result = Some(iteration_result);
                }
                IterationDecision::Complete => {
                    // Generate final result
                    let final_response = self.generate_final_response(&iteration_result, &state);
                    let confidence_score = self.calculate_final_confidence(&state);

                    return Ok(AgentResult {
                        final_response,
                        confidence_score,
                        iterations_used: state.iteration_count,
                        tools_executed: state.total_tools_executed,
                        verification_history: state.execution_history.iter()
                            .filter_map(|r| r.verification_result.clone())
                            .collect(),
                        execution_time: start_time.elapsed(),
                    });
                }
                IterationDecision::Fail(reason) => {
                    state.failure_count += 1;

                    if self.failure_recovery_enabled && state.failure_count < 3 {
                        // Attempt recovery
                        state.recovery_attempts += 1;
                        current_goal = format!("{} (Recovery attempt {})", initial_goal, state.recovery_attempts);
                        continue;
                    } else {
                        return Err(AgentError::ExecutionFailed(reason));
                    }
                }
            }
        }

        // Max iterations reached
        Err(AgentError::MaxIterationsExceeded(self.max_iterations))
    }

    /// Verify the results of an agent iteration
    async fn verify_iteration_result(
        &self,
        result: &AgentIterationResult,
        state: &AgentExecutionState,
    ) -> Result<VerificationResult, AgentError> {
        let mut checks_performed = Vec::new();
        let mut failed_checks = Vec::new();

        // Check 1: Reasoning quality
        checks_performed.push("reasoning_quality".to_string());
        if result.reasoning_steps.is_empty() {
            failed_checks.push("No reasoning steps provided".to_string());
        } else if result.reasoning_steps.len() < 2 {
            failed_checks.push("Insufficient reasoning steps".to_string());
        }

        // Check 2: Tool call validity
        checks_performed.push("tool_call_validity".to_string());
        for tool_call in &result.tool_calls {
            if tool_call.is_empty() {
                failed_checks.push("Empty tool call found".to_string());
            }
        }

        // Check 3: Progress check (avoid loops)
        checks_performed.push("progress_check".to_string());
        if state.iteration_count > 1 {
            // Check if we're making progress (simplified)
            let recent_iterations = &state.execution_history[state.execution_history.len().saturating_sub(3)..];
            let similar_results = recent_iterations.iter()
                .filter(|r| r.reasoning_steps == result.reasoning_steps)
                .count();

            if similar_results >= 2 {
                failed_checks.push("Agent appears to be looping without progress".to_string());
            }
        }

        // Check 4: Resource usage
        checks_performed.push("resource_usage".to_string());
        if state.total_tools_executed > 20 {
            failed_checks.push("Too many tools executed overall".to_string());
        }

        // Determine result
        if failed_checks.is_empty() {
            let confidence_score = self.calculate_iteration_confidence(result, state);
            Ok(VerificationResult::Passed {
                confidence_score,
                checks_performed,
            })
        } else {
            Ok(VerificationResult::Failed {
                reason: format!("{} checks failed", failed_checks.len()),
                failed_checks,
            })
        }
    }

    /// Decide whether to continue iterating
    fn should_continue_iterating(
        &self,
        result: &AgentIterationResult,
        state: &AgentExecutionState,
    ) -> IterationDecision {
        // Check verification result
        if let Some(VerificationResult::Failed { .. }) = &state.last_verification_result {
            if !self.failure_recovery_enabled {
                return IterationDecision::Fail("Verification failed".to_string());
            }
        }

        // Check if we have a conclusive answer
        if result.confidence_score > 0.8 && state.iteration_count >= 2 {
            return IterationDecision::Complete;
        }

        // Check if we've reached iteration limits
        if state.iteration_count >= self.max_iterations {
            return IterationDecision::Complete;
        }

        // Check if tools suggest we need more information
        let needs_more_info = result.tool_calls.iter().any(|tc|
            tc.to_lowercase().contains("search") ||
            tc.to_lowercase().contains("lookup") ||
            tc.to_lowercase().contains("check")
        );

        if needs_more_info && state.iteration_count < self.max_iterations {
            IterationDecision::Continue(format!("Gather more information: {}", result.next_goal))
        } else {
            IterationDecision::Complete
        }
    }

    fn calculate_iteration_confidence(&self, result: &AgentIterationResult, _state: &AgentExecutionState) -> f32 {
        let mut confidence = 0.5; // Base confidence

        // Reasoning quality
        confidence += (result.reasoning_steps.len() as f32 * 0.1).min(0.2);

        // Tool usage (balanced is good)
        if result.tool_calls.len() > 0 && result.tool_calls.len() <= 3 {
            confidence += 0.2;
        }

        // Explicit confidence score
        confidence += (result.confidence_score * 0.3).min(0.3);

        confidence.min(1.0)
    }

    fn calculate_final_confidence(&self, state: &AgentExecutionState) -> f32 {
        let verification_scores: Vec<f32> = state.execution_history.iter()
            .filter_map(|r| match &r.verification_result {
                Some(VerificationResult::Passed { confidence_score, .. }) => Some(*confidence_score),
                _ => None,
            })
            .collect();

        if verification_scores.is_empty() {
            0.3 // Low confidence if no verifications passed
        } else {
            verification_scores.iter().sum::<f32>() / verification_scores.len() as f32
        }
    }

    fn generate_final_response(&self, result: &AgentIterationResult, state: &AgentExecutionState) -> String {
        let mut response = result.final_response.clone();

        // Add metadata about execution
        response.push_str(&format!("\n\n--- Execution Summary ---\n"));
        response.push_str(&format!("Iterations: {}\n", state.iteration_count));
        response.push_str(&format!("Tools executed: {}\n", state.total_tools_executed));
        response.push_str(&format!("Execution time: {:.2}s\n", state.start_time.elapsed().unwrap_or_default().as_secs_f64()));

        if state.failure_count > 0 {
            response.push_str(&format!("Recovery attempts: {}\n", state.recovery_attempts));
        }

        response
    }
}

#[derive(Debug)]
pub enum IterationDecision {
    Continue(String), // Continue with new goal
    Complete,         // We have a final answer
    Fail(String),     // Failed with reason
}

#[derive(Debug, Clone)]
pub struct AgentIterationResult {
    pub reasoning_steps: Vec<String>,
    pub tool_calls: Vec<String>,
    pub final_response: String,
    pub confidence_score: f32,
    pub next_goal: String,
}

#[derive(Debug, Clone)]
pub enum AgentError {
    Timeout(String),
    IterationTimeout(u32),
    TooManyTools(usize, usize),
    ExecutionFailed(String),
    MaxIterationsExceeded(u32),
    VerificationError(String),
    InternalError(String),
}

impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentError::Timeout(msg) => write!(f, "Agent timeout: {}", msg),
            AgentError::IterationTimeout(iter) => write!(f, "Iteration {} timed out", iter),
            AgentError::TooManyTools(actual, limit) => write!(f, "Too many tools in iteration: {} > {}", actual, limit),
            AgentError::ExecutionFailed(msg) => write!(f, "Agent execution failed: {}", msg),
            AgentError::MaxIterationsExceeded(max) => write!(f, "Maximum iterations exceeded: {}", max),
            AgentError::VerificationError(msg) => write!(f, "Verification error: {}", msg),
            AgentError::InternalError(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for AgentError {}

/// Safe failure behavior implementation
pub struct SafeFailureHandler {
    max_retries: u32,
    backoff_multiplier: f32,
    enable_fallbacks: bool,
}

impl SafeFailureHandler {
    pub fn new() -> Self {
        Self {
            max_retries: 3,
            backoff_multiplier: 1.5,
            enable_fallbacks: true,
        }
    }

    pub async fn execute_with_failure_handling<F, Fut, T>(
        &self,
        operation: F,
    ) -> Result<T, AgentError>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T, AgentError>>,
    {
        let mut attempt = 0;
        let mut last_error = None;

        while attempt < self.max_retries {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(error) => {
                    last_error = Some(error);
                    attempt += 1;

                    if attempt < self.max_retries {
                        // Exponential backoff
                        let delay_ms = (1000.0 * self.backoff_multiplier.powi(attempt as i32 - 1)) as u64;
                        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| AgentError::InternalError("Unknown error".to_string())))
    }

    pub fn generate_safe_fallback_response(&self, error: &AgentError, original_goal: &str) -> String {
        match error {
            AgentError::Timeout(_) => {
                format!("I apologize, but I couldn't complete the task within the time limit. The request was: '{}'. Please try breaking it down into smaller, more specific tasks.", original_goal)
            }
            AgentError::MaxIterationsExceeded(_) => {
                format!("I explored multiple approaches but couldn't reach a definitive answer for: '{}'. The task might be too complex or require additional context.", original_goal)
            }
            AgentError::ExecutionFailed(reason) => {
                format!("I encountered an issue while working on: '{}'. The problem was: {}. Please check your request or try a different approach.", original_goal, reason)
            }
            _ => {
                format!("I wasn't able to complete the request: '{}'. This might be due to system constraints or the complexity of the task.", original_goal)
            }
        }
    }
}