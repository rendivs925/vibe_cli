use domain::models::{
    AgentRequest, AgentResponse, AgentContext, ToolCall, ToolResult,
    ConversationMessage, ToolDefinition, ToolParameters, ParameterProperty
};
use infrastructure::{ollama_client::OllamaClient, config::Config, agent_control::{AgentController, SafeFailureHandler, AgentIterationResult, AgentExecutionState, AgentError, AgentResult}, sandbox::Sandbox};
use shared::types::Result;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

// Forward declare for now - actual implementation when both services are integrated
pub type RagService = crate::rag_service::RagService;

pub struct AgentService {
    client: OllamaClient,
    rag_service: Option<Arc<RagService>>,
    config: Config,
    agent_controller: AgentController,
    failure_handler: SafeFailureHandler,
}

impl AgentService {
    pub fn new(client: OllamaClient) -> Self {
        Self {
            client,
            rag_service: None,
            config: Config::load(),
            agent_controller: AgentController::new(),
            failure_handler: SafeFailureHandler::new(),
        }
    }

    pub fn with_rag_service(client: OllamaClient, rag_service: Arc<RagService>) -> Self {
        Self {
            client,
            rag_service: Some(rag_service),
            config: Config::load(),
            agent_controller: AgentController::new(),
            failure_handler: SafeFailureHandler::new(),
        }
    }

    pub async fn process_request(&self, request: &AgentRequest) -> Result<AgentResponse> {
        // Initialize context for agent execution
        let mut context = self.initialize_context(request).await?;
        let reasoning_steps = self.generate_reasoning(&request.goal, &context).await?;

        // For now, execute a single iteration with bounded execution simulation
        // TODO: Implement full multi-iteration bounded execution once lifetime issues are resolved

        // Execute single iteration directly
        let iteration_result = self.execute_single_iteration(&request.goal, &AgentExecutionState {
            iteration_count: 0,
            total_tools_executed: 0,
            start_time: std::time::SystemTime::now(),
            last_verification_result: None,
            execution_history: vec![],
            failure_count: 0,
            recovery_attempts: 0,
        }, request).await?;

        // Apply basic bounds checking (simulate bounded execution)
        if iteration_result.tool_calls.len() > 3 {
            return Err(anyhow::anyhow!("Iteration would exceed tool limit: {} > 3", iteration_result.tool_calls.len()));
        }

        // Convert tool calls to proper format
        let tool_calls = iteration_result.tool_calls.iter().enumerate().map(|(i, tool_str)| {
            ToolCall {
                id: format!("tool_{}", i),
                name: tool_str.clone(),
                parameters: HashMap::new(),
                reasoning: "Tool call generated during agent execution".to_string(),
            }
        }).collect();

        // Create agent result with bounded execution metadata
        let agent_result = AgentResult {
            final_response: iteration_result.final_response,
            confidence_score: iteration_result.confidence_score,
            execution_time: std::time::Duration::from_secs(1),
            iterations_used: 1, // Single iteration for now
            tools_executed: iteration_result.tool_calls.len() as u32,
            verification_history: vec![], // TODO: Add verification
        };

        // Convert to response format
        Ok(AgentResponse {
            reasoning: iteration_result.reasoning_steps,
            tool_calls,
            final_response: agent_result.final_response,
            confidence: agent_result.confidence_score,
        })
    }

    async fn execute_single_iteration(
        &self,
        goal: &str,
        state: &AgentExecutionState,
        request: &AgentRequest,
    ) -> Result<AgentIterationResult> {
        // Initialize context for this iteration
        let mut context = self.initialize_context(request).await?;

        // Generate reasoning plan (adapt based on iteration state)
        let reasoning_steps = self.generate_reasoning(goal, &context).await?;

        // Determine if tools are needed
        let tool_calls = if self.needs_tools(goal, &reasoning_steps) {
            self.plan_tool_calls(goal, &reasoning_steps, &context).await?
        } else {
            Vec::new()
        };

        // Limit tools per iteration
        if tool_calls.len() > 3 {
            return Err(anyhow::anyhow!("Too many tools in iteration: {} > 3", tool_calls.len()));
        }

        // Execute tools if any
        let tool_results = if !tool_calls.is_empty() {
            self.execute_tool_calls(&tool_calls, &mut context).await?
        } else {
            Vec::new()
        };

        // Generate response for this iteration
        let final_response = self.generate_final_response(
            goal,
            &reasoning_steps,
            &tool_results,
            &context
        ).await?;

        // Calculate confidence
        let confidence_score = self.calculate_confidence(&reasoning_steps, &tool_results);

        // Determine next goal (simplified - could be more sophisticated)
        let next_goal = if confidence_score < 0.7 && state.iteration_count < 3 {
            format!("Refine approach for: {}", goal)
        } else {
            goal.to_string()
        };

        // Convert tool calls to string representation for the result
        let tool_call_strings = tool_calls.iter().map(|tc| format!("{:?}", tc)).collect();

        Ok(AgentIterationResult {
            reasoning_steps,
            tool_calls: tool_call_strings,
            final_response,
            confidence_score,
            next_goal,
        })
    }

    async fn initialize_context(&self, request: &AgentRequest) -> Result<AgentContext> {
        let available_tools = self.get_available_tools();
        
        let conversation_history = if let Some(conversation_id) = &request.conversation_id {
            // In a real implementation, this would load conversation history
            vec![
                ConversationMessage {
                    role: "user".to_string(),
                    content: request.goal.clone(),
                    tool_calls: None,
                    tool_call_id: None,
                }
            ]
        } else {
            vec![
                ConversationMessage {
                    role: "user".to_string(),
                    content: request.goal.clone(),
                    tool_calls: None,
                    tool_call_id: None,
                }
            ]
        };

        Ok(AgentContext {
            conversation_history,
            working_memory: HashMap::new(),
            available_tools,
        })
    }

    async fn generate_reasoning(&self, goal: &str, context: &AgentContext) -> Result<Vec<String>> {
        let system_prompt = self.create_reasoning_prompt();
        
        let prompt = format!(
            "Goal: {}\n\nThink step by step about how to approach this goal. \
             Consider what information you need, what tools might be helpful, \
             and how you should proceed. Break down your thinking into clear steps.",
            goal
        );

        let response = self.client.generate_response_with_system(&prompt, &system_prompt).await?;
        
        // Parse reasoning steps from response
        let reasoning_steps: Vec<String> = response
            .lines()
            .filter(|line| line.trim().starts_with('-') || line.trim().starts_with('*') || 
                             line.trim().starts_with("1.") || line.trim().starts_with("2."))
            .map(|line| {
                let trimmed = line.trim();
                let step = trimmed.trim_start_matches(['-', '*', '1', '2', '.', ' ']);
                step.trim().to_string()
            })
            .filter(|step| !step.is_empty())
            .collect();

        Ok(if reasoning_steps.is_empty() {
            vec!["Analyze the request carefully".to_string(), 
                  "Determine the best approach to answer".to_string(),
                  "Formulate a comprehensive response".to_string()]
        } else {
            reasoning_steps
        })
    }

    fn needs_tools(&self, goal: &str, reasoning_steps: &[String]) -> bool {
        // Simple heuristic: if goal asks about code, files, or requires specific information
        let keywords = ["code", "file", "search", "find", "analyze", "explain", "show", "list"];
        let goal_lower = goal.to_lowercase();
        
        // Check if goal contains keywords
        let has_keywords = keywords.iter().any(|keyword| goal_lower.contains(keyword));
        
        // Check if reasoning steps mention needing information
        let reasoning_mentions_info = reasoning_steps.iter()
            .any(|step| step.to_lowercase().contains("information") || 
                           step.to_lowercase().contains("find") ||
                           step.to_lowercase().contains("analyze"));
        
        has_keywords || reasoning_mentions_info
    }

    async fn plan_tool_calls(&self, goal: &str, reasoning: &[String], context: &AgentContext) -> Result<Vec<ToolCall>> {
        let tool_descriptions: Vec<String> = context.available_tools
            .iter()
            .map(|tool| format!("{}: {}", tool.name, tool.description))
            .collect();
        
        let system_prompt = format!(
            "You are an AI assistant that can use tools to help with software development tasks.\n\n\
             Available tools:\n{}\n\n\
             When you need to use a tool, respond with a JSON object containing the tool call.\n\
             Example: {{\"name\": \"rag_query\", \"parameters\": {{\"question\": \"How does authentication work?\"}}}}\n\n\
             Only use tools when necessary. For general questions, respond directly.",
            tool_descriptions.join("\n")
        );

        let prompt = format!(
            "Goal: {}\n\nReasoning steps:\n{}\n\n\
             Based on your reasoning, do you need to use any tools? If so, provide the tool call as JSON.",
            goal, reasoning.join("\n")
        );

        let response = self.client.generate_response_with_system(&prompt, &system_prompt).await?;
        
        // Try to parse JSON tool call
        if let Ok(tool_call_json) = serde_json::from_str::<Value>(&response) {
            if let Some(tool_name) = tool_call_json.get("name").and_then(|v| v.as_str()) {
                let parameters = tool_call_json.get("parameters").cloned().unwrap_or(Value::Object(Default::default()));
                
                return Ok(vec![ToolCall {
                    id: Uuid::new_v4().to_string(),
                    name: tool_name.to_string(),
                    parameters: self.value_to_hashmap(parameters),
                    reasoning: "Tool selected based on goal analysis".to_string(),
                }]);
            }
        }
        
        Ok(Vec::new())
    }

    async fn execute_tool_calls(&self, tool_calls: &[ToolCall], context: &mut AgentContext) -> Result<Vec<ToolResult>> {
        let mut results = Vec::new();
        
        for tool_call in tool_calls {
            let result = match tool_call.name.as_str() {
                "rag_query" => self.execute_rag_query(tool_call, context).await,
                "file_read" => self.execute_file_read(tool_call, context).await,
                "code_analysis" => self.execute_code_analysis(tool_call, context).await,
                _ => Ok(ToolResult {
                    tool_call_id: tool_call.id.clone(),
                    success: false,
                    result: json!(null),
                    error: Some(format!("Unknown tool: {}", tool_call.name)),
                })
            };
            
            results.push(result?);
        }
        
        Ok(results)
    }

    async fn execute_rag_query(&self, tool_call: &ToolCall, _context: &AgentContext) -> Result<ToolResult> {
        let question = tool_call.parameters
            .get("question")
            .and_then(|v| v.as_str())
            .unwrap_or("No question provided");
            
        // Use real RAG service if available, otherwise fallback
        if let Some(rag_service) = &self.rag_service {
            match rag_service.query(question).await {
                Ok(answer) => Ok(ToolResult {
                    tool_call_id: tool_call.id.clone(),
                    success: true,
                    result: json!({"answer": answer, "source": "rag_service"}),
                    error: None,
                }),
                Err(e) => Ok(ToolResult {
                    tool_call_id: tool_call.id.clone(),
                    success: false,
                    result: json!(null),
                    error: Some(format!("RAG query failed: {}", e)),
                })
            }
        } else {
            // Fallback placeholder
            Ok(ToolResult {
                tool_call_id: tool_call.id.clone(),
                success: true,
                result: json!({"answer": format!("RAG service not available for question: {}", question), "source": "fallback"}),
                error: None,
            })
        }
    }

    async fn execute_file_read(&self, tool_call: &ToolCall, _context: &AgentContext) -> Result<ToolResult> {
        let file_path = tool_call.parameters
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("");
            
        // Validate with sandbox first, then execute actual file read
        let sandbox = Sandbox::new();
        if let Err(e) = sandbox.test_command("cat", &vec![file_path.to_string()]) {
            return Ok(ToolResult {
                tool_call_id: tool_call.id.clone(),
                success: false,
                result: json!(null),
                error: Some(format!("Sandbox blocked file read: {}", e)),
            });
        }
        
        // Real file reading implementation
        match std::fs::read_to_string(file_path) {
            Ok(content) => {
                let content_size = content.len();
                // Limit content size to avoid token limit issues
                let limited_content = if content_size > 5000 {
                    format!("{}...\n\n[Content truncated due to size. File is {} bytes total]", 
                            &content[..5000], content_size)
                } else {
                    content
                };
                
                Ok(ToolResult {
                    tool_call_id: tool_call.id.clone(),
                    success: true,
                    result: json!({"content": limited_content, "size": content_size, "source": "file_read_tool"}),
                    error: None,
                })
            },
            Err(e) => Ok(ToolResult {
                tool_call_id: tool_call.id.clone(),
                success: false,
                result: json!(null),
                error: Some(format!("Failed to read file '{}': {}", file_path, e)),
            })
        }
    }

    async fn execute_code_analysis(&self, tool_call: &ToolCall, _context: &AgentContext) -> Result<ToolResult> {
        let analysis_type = tool_call.parameters
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("general");
            
        // Placeholder implementation
        Ok(ToolResult {
            tool_call_id: tool_call.id.clone(),
            success: true,
            result: json!({"analysis": format!("Code analysis of type: {}", analysis_type)}),
            error: None,
        })
    }

    async fn generate_final_response(&self, goal: &str, reasoning: &[String], tool_results: &[ToolResult], context: &AgentContext) -> Result<String> {
        let tool_results_text: String = tool_results
            .iter()
            .map(|result| {
                if result.success {
                    format!("Tool result: {}", serde_json::to_string_pretty(&result.result).unwrap_or_default())
                } else {
                    format!("Tool error: {}", result.error.as_deref().unwrap_or("Unknown error"))
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        let system_prompt = "You are a helpful AI assistant that provides clear, accurate, and concise responses to user requests. Use the provided reasoning and tool results to give the best possible answer.";

        let prompt = format!(
            "Goal: {}\n\nReasoning steps:\n{}\n\nTool results:\n{}\n\n\
             Based on the reasoning and tool results, provide a comprehensive response to the user's goal.",
            goal, reasoning.join("\n"), tool_results_text
        );

        self.client.generate_response_with_system(&prompt, system_prompt).await
    }

    fn calculate_confidence(&self, reasoning_steps: &[String], tool_results: &[ToolResult]) -> f32 {
        let reasoning_confidence = if reasoning_steps.len() >= 3 { 0.8 } else { 0.6 };
        let tool_success_rate = if tool_results.is_empty() {
            0.7 // Neutral confidence when no tools used
        } else {
            let successful_tools = tool_results.iter().filter(|r| r.success).count();
            successful_tools as f32 / tool_results.len() as f32
        };
        
        (reasoning_confidence + tool_success_rate) / 2.0
    }

    fn create_reasoning_prompt(&self) -> String {
        "You are an intelligent AI assistant that thinks step by step before responding. \
         Break down complex problems into logical steps, consider what information you need, \
         and plan your approach systematically. Your reasoning should be clear and structured."
            .to_string()
    }

    fn get_available_tools(&self) -> Vec<ToolDefinition> {
        vec![
            ToolDefinition {
                name: "rag_query".to_string(),
                description: "Search the codebase for relevant information using RAG".to_string(),
                parameters: ToolParameters {
                    param_type: "object".to_string(),
                    properties: {
                        let mut props = HashMap::new();
                        props.insert("question".to_string(), ParameterProperty {
                            param_type: "string".to_string(),
                            description: "The question to search for in the codebase".to_string(),
                            enum_values: None,
                        });
                        props
                    },
                    required: vec!["question".to_string()],
                },
            },
            ToolDefinition {
                name: "file_read".to_string(),
                description: "Read the contents of a specific file".to_string(),
                parameters: ToolParameters {
                    param_type: "object".to_string(),
                    properties: {
                        let mut props = HashMap::new();
                        props.insert("path".to_string(), ParameterProperty {
                            param_type: "string".to_string(),
                            description: "Path to the file to read".to_string(),
                            enum_values: None,
                        });
                        props
                    },
                    required: vec!["path".to_string()],
                },
            },
            ToolDefinition {
                name: "code_analysis".to_string(),
                description: "Analyze code structure and patterns".to_string(),
                parameters: ToolParameters {
                    param_type: "object".to_string(),
                    properties: {
                        let mut props = HashMap::new();
                        props.insert("type".to_string(), ParameterProperty {
                            param_type: "string".to_string(),
                            description: "Type of analysis to perform".to_string(),
                            enum_values: Some(vec!["structure".to_string(), "dependencies".to_string(), "patterns".to_string()]),
                        });
                        props
                    },
                    required: vec!["type".to_string()],
                },
            },
        ]
    }

    fn value_to_hashmap(&self, value: Value) -> HashMap<String, Value> {
        if let Value::Object(map) = value {
            map.into_iter().collect()
        } else {
            HashMap::new()
        }
    }

    // Legacy method for backward compatibility
    pub async fn run_agent(&self, input: &str) -> Result<String> {
        let request = AgentRequest {
            goal: input.to_string(),
            context: None,
            conversation_id: None,
        };
        
        let response = self.process_request(&request).await?;
        Ok(response.final_response)
    }
}
