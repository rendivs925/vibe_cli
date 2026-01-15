use crate::types::{AgentPlan, AgentStep, AgentCommandRisk};
use anyhow::{anyhow, Result};
use crate::analysis::assess_agent_command_risk;
use crate::utils::{extract_last_json, clean_command_output};
use shared::confirmation;

/// Analyze agent task and generate execution plan
pub async fn analyze_agent_task(task: &str) -> Result<AgentPlan> {
    println!("ANALYZING TASK: \"{}\"", task);

    // Get current directory context
    let current_dir = std::env::current_dir()
        .ok()
        .and_then(|p| p.to_str().map(|s| s.to_string()))
        .unwrap_or_else(|| ".".to_string());

    let ls_output = std::process::Command::new("sh")
        .arg("-c")
        .arg("ls -la 2>/dev/null | head -n 20")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_else(|| String::new());

    // Use AI to generate detailed execution plan
    let client = infrastructure::ollama_client::OllamaClient::new()?;

    let prompt = format!(
        r#"Analyze this task and create a detailed execution plan with individual steps.

TASK: {}

CURRENT DIRECTORY: {}
DIRECTORY CONTENTS (first 20 entries):
{}

Generate a JSON object with this structure:
{{
  "steps": [
    {{
      "id": "step_1",
      "command": "exact shell command",
      "description": "what this step does",
      "risk_level": "InfoOnly|SafeOperations|NetworkAccess|SystemChanges|Destructive",
      "estimated_duration": "X seconds" or "X minutes",
      "dependencies": ["step_id1", "step_id2"] (empty array if none)
    }}
  ],
  "estimated_total_time": "X minutes",
  "disk_impact": "X MB" (if applicable),
  "network_required": true/false,
  "safety_concerns": ["concern1", "concern2"] (if any)
}}

Rules:
- Commands must be executable shell commands
- Each step should be atomic and independently verifiable
- Include realistic time estimates
- Mark dependencies accurately
- Flag any safety concerns
- Use only commands available in the current directory context
- Prefer safer alternatives when possible

OUTPUT ONLY VALID JSON:"#,
        task, current_dir, ls_output
    );

    let response = client.generate_response(&prompt).await?;

    // Extract JSON from the response (AI might include extra text)
    let plan: AgentPlan = if let Some(json_start) = response.find('{') {
        let json_str = &response[json_start..];
        if let Some(json_end) = json_str.rfind('}') {
            let json_content = &json_str[..=json_end];
            serde_json::from_str(json_content)
                .map_err(|e| anyhow!("Failed to parse agent plan JSON: {}", e))?
        } else {
            return Err(anyhow!("No closing brace found in agent plan response"));
        }
    } else {
        return Err(anyhow!("No JSON found in agent plan response"));
    };

    // Enhance plan with additional analysis
    let enhanced_plan = enhance_agent_plan(plan, task);

    Ok(enhanced_plan)
}

/// Enhance agent plan with additional analysis and safety checks
pub fn enhance_agent_plan(mut plan: AgentPlan, original_task: &str) -> AgentPlan {
    // Re-assess risk levels and add rollback commands
    for step in &mut plan.steps {
        let assessed_risk = assess_agent_command_risk(&step.command);
        step.risk_level = assessed_risk;

        // Add rollback commands for reversible operations
        step.rollback_command = match step.command.split_whitespace().next() {
            Some("mkdir") => {
                // Extract directory name
                let parts: Vec<&str> = step.command.split_whitespace().collect();
                if parts.len() >= 2 {
                    Some(format!("rmdir {}", parts[1]))
                } else {
                    None
                }
            }
            Some("touch") => {
                let parts: Vec<&str> = step.command.split_whitespace().collect();
                if parts.len() >= 2 {
                    Some(format!("rm -f {}", parts[1]))
                } else {
                    None
                }
            }
            _ => None,
        };
    }

    // Analyze for safety concerns
    let mut safety_concerns = Vec::new();
    let network_steps = plan
        .steps
        .iter()
        .filter(|s| s.risk_level == AgentCommandRisk::NetworkAccess)
        .count();

    if network_steps > 0 {
        safety_concerns.push(format!("{} steps require network access", network_steps));
    }

    let destructive_steps = plan
        .steps
        .iter()
        .filter(|s| s.risk_level == AgentCommandRisk::Destructive)
        .count();

    if destructive_steps > 0 {
        safety_concerns.push(format!(
            "{} steps are potentially destructive",
            destructive_steps
        ));
    }

    // Check for disk space impact
    let has_installs = plan
        .steps
        .iter()
        .any(|s| s.command.contains("install") || s.command.contains("download"));

    if has_installs && plan.total_disk_impact.is_none() {
        plan.total_disk_impact = Some("~50MB".to_string());
    }

    // Update network requirement based on analysis
    plan.network_required = plan
        .steps
        .iter()
        .any(|s| s.risk_level == AgentCommandRisk::NetworkAccess);

    plan.safety_concerns = safety_concerns;

    plan
}

/// Display agent execution plan in structured format
pub fn display_agent_plan(plan: &AgentPlan) {
    println!();
    println!(
        "EXECUTION PLAN ({} steps{})",
        plan.steps.len(),
        plan.total_estimated_time
            .as_ref()
            .map(|t| format!(" - Estimated: {}", t))
            .unwrap_or_default()
    );

    for (i, step) in plan.steps.iter().enumerate() {
        let step_num = i + 1;
        println!();
        println!("STEP {}: {}", step_num, step.description.to_uppercase());
        println!("  Command: {}", step.command);
        println!("  Risk Level: {}", format_risk_level(&step.risk_level));

        if let Some(duration) = &step.estimated_duration {
            println!("  Estimated Time: {}", duration);
        }

        if !step.dependencies.is_empty() {
            println!("  Dependencies: {}", step.dependencies.join(", "));
        }
    }

    // Show summary
    println!();
    println!("PLAN SUMMARY:");
    if let Some(disk) = &plan.total_disk_impact {
        println!("  Disk Impact: {}", disk);
    }
    println!(
        "  Network Required: {}",
        if plan.network_required { "Yes" } else { "No" }
    );

    if !plan.safety_concerns.is_empty() {
        println!("  Safety Concerns:");
        for concern in &plan.safety_concerns {
            println!("    - {}", concern);
        }
    }
}

/// Format risk level for display
pub fn format_risk_level(risk: &AgentCommandRisk) -> &'static str {
    match risk {
        AgentCommandRisk::InfoOnly => "Info Only",
        AgentCommandRisk::SafeOperations => "Safe Operations",
        AgentCommandRisk::NetworkAccess => "Network Access",
        AgentCommandRisk::SystemChanges => "System Changes",
        AgentCommandRisk::Destructive => "Destructive",
        AgentCommandRisk::Unknown => "Unknown",
    }
}

/// Execute complete agent plan
pub async fn execute_complete_plan(plan: &AgentPlan) -> Result<()> {
    println!();
    println!("EXECUTING AGENT PLAN...");

    let start_time = std::time::Instant::now();
    let mut completed_steps = 0;
    let total_steps = plan.steps.len();

    for (i, step) in plan.steps.iter().enumerate() {
        let step_num = i + 1;
        println!();
        println!("[{}/{}] {}", step_num, total_steps, step.description);

        // Execute the step
        match execute_agent_step(step).await {
            Ok(_) => {
                completed_steps += 1;
                println!("Step {}/{}: {}", step_num, total_steps, step.description);
            }
            Err(e) => {
                eprintln!("Step {}/{} failed: {}", step_num, total_steps, e);
                if ask_confirmation("Continue with remaining steps?", false)? {
                    continue;
                } else {
                    eprintln!("Execution stopped due to error.");
                    break;
                }
            }
        }
    }

    let duration = start_time.elapsed();
    println!();
    println!("AGENT EXECUTION COMPLETE");
    println!("- Total steps: {}", total_steps);
    println!("- Successful: {}", completed_steps);
    println!("- Failed: {}", total_steps - completed_steps);
    println!("- Duration: {:.1}s", duration.as_secs_f64());

    if completed_steps == total_steps {
        show_agent_completion_steps(plan);
    }

    Ok(())
}

/// Execute agent plan step by step
pub async fn execute_step_by_step(plan: &AgentPlan) -> Result<()> {
    println!();
    println!("STEP-BY-STEP EXECUTION MODE");

    for (i, step) in plan.steps.iter().enumerate() {
        let step_num = i + 1;
        println!();
        println!("STEP {}: {}", step_num, step.description.to_uppercase());
        println!("Command: {}", step.command);
        println!("Risk Level: {}", format_risk_level(&step.risk_level));

        if let Some(duration) = &step.estimated_duration {
            println!("Estimated Time: {}", duration);
        }

        println!();
        let confirm = ask_confirmation("Execute this step?", true)?;

        if !confirm {
            println!("Step {} skipped.", step_num);
            continue;
        }

        match execute_agent_step(step).await {
            Ok(_) => println!("Step {} completed successfully.", step_num),
            Err(e) => {
                eprintln!("Step {} failed: {}", step_num, e);
                if !ask_confirmation("Continue with next step?", false)? {
                    break;
                }
            }
        }
    }

    println!();
    println!("Step-by-step execution complete.");
    Ok(())
}

/// Execute agent plan in dry run mode
pub async fn execute_dry_run(plan: &AgentPlan) -> Result<()> {
    println!();
    println!("DRY RUN MODE - No commands will be executed");
    println!("========================================");

    for (i, step) in plan.steps.iter().enumerate() {
        let step_num = i + 1;
        println!();
        println!("STEP {}: {}", step_num, step.description);
        println!("  Command: {}", step.command);
        println!("  Risk Level: {}", format_risk_level(&step.risk_level));

        if let Some(duration) = &step.estimated_duration {
            println!("  Estimated Time: {}", duration);
        }

        // Simulate validation
        match validate_command_syntax(&step.command) {
            Ok(_) => println!("  Validation: Command syntax OK"),
            Err(e) => println!("  Validation: Syntax error - {}", e),
        }

        // Check if command would be allowed
        let power_config = infrastructure::config::Config::load().power_user;
        let is_allowed = power_config.is_command_allowed(&step.command);
        if is_allowed {
            println!("  Safety: Command allowed");
        } else {
            println!("  Safety: Command blocked by policy");
        }
    }

    println!();
    println!("DRY RUN COMPLETE");
    println!("- Total steps: {}", plan.steps.len());
    println!("- Commands validated and checked for safety");
    println!("- No system changes made");

    Ok(())
}

/// Execute individual agent step
pub async fn execute_agent_step(step: &AgentStep) -> Result<()> {
    let power_config = infrastructure::config::Config::load().power_user;

    // Check safety policy - allow user override if they confirmed
    let is_allowed = power_config.is_command_allowed(&step.command);
    if !is_allowed {
        // Ask for override confirmation like installation commands
        eprintln!("Command '{}' is blocked by safety policy.", step.command);
        if !confirmation::ask_confirmation("Execute anyway?", false)? {
            return Err(anyhow!("Command cancelled due to safety policy."));
        }
        // User explicitly confirmed override
    }

    // Execute the command
    let sandbox = infrastructure::sandbox::Sandbox::new();
    let output = sandbox
        .execute_safe("bash", vec!["-c".to_string(), step.command.clone()])
        .await?;
    if !output.trim().is_empty() {
        println!("{}", output);
    }
    Ok(())
}

/// Show completion steps for agent execution
pub fn show_agent_completion_steps(plan: &AgentPlan) {
    // Analyze the completed plan to suggest next steps
    let has_web_server = plan.steps.iter().any(|s| {
        s.command.contains("nginx")
            || s.command.contains("apache")
            || s.command.contains("httpd")
    });

    let has_node_app = plan
        .steps
        .iter()
        .any(|s| s.command.contains("npm") || s.command.contains("node"));

    let has_python_app = plan
        .steps
        .iter()
        .any(|s| s.command.contains("pip") || s.command.contains("python"));

    println!();
    println!("NEXT STEPS SUGGESTED:");

    if has_web_server {
        println!("1. Start your web server:");
        println!("   sudo systemctl start nginx  # or apache2");
        println!("2. Test your server:");
        println!("   curl http://localhost");
        if has_node_app {
            println!("3. Start your Node.js application:");
            println!("   cd your-app && npm start");
        }
    }

    if has_python_app && !has_web_server {
        println!("1. Run your Python application:");
        println!("   python3 your_app.py");
        println!("2. Or with virtual environment:");
        println!("   source venv/bin/activate && python your_app.py");
    }

    if plan.network_required {
        println!("Note: Some steps required network access for package downloads.");
    }

    println!("Use 'vibe --agent \"verify setup\"' to check your installation.");
}

fn ask_confirmation(prompt: &str, default: bool) -> Result<bool> {
    shared::confirmation::ask_confirmation(prompt, default)
}

fn validate_command_syntax(command: &str) -> std::result::Result<(), String> {
    crate::analysis::validate_command_syntax(command)
}