use crate::config::Config;
use crate::model::request_agent_plan;
use crate::runner::confirm_and_run_multi_step;
use anyhow::Result;
use colored::*;

pub async fn run_agent_mode(config: &Config, prompt_text: &str) -> Result<()> {
    if prompt_text.trim().is_empty() {
        println!(
            "{}",
            "Agent mode requires a prompt (e.g. qwen-cli --agent \"clean logs and show usage\")"
                .red()
        );
        return Ok(());
    }

    println!("{}", "Requesting plan from model...".green());
    let plan: Vec<String> = request_agent_plan(config, prompt_text).await?;

    if plan.is_empty() {
        println!("{}", "Model returned no commands".red());
        return Ok(());
    }

    println!("\n{}", "Proposed plan:".green().bold());
    for (i, cmd) in plan.iter().enumerate() {
        println!("  {} {}", format!("[{}]", i + 1).blue(), cmd);
    }

    for (i, cmd) in plan.iter().enumerate() {
        println!(
            "\n{} {}",
            "Step".green().bold(),
            format!("{}:", i + 1).green().bold()
        );
        println!("[STEP] {}/{}", i + 1, plan.len());
        confirm_and_run_multi_step(cmd, config)?;
    }

    Ok(())
}
