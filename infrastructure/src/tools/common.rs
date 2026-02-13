use domain::tools::{ToolError, ToolOutput};
use std::process::Command;

pub fn run_process(program: &str, args: &[&str]) -> Result<ToolOutput, ToolError> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;

    let code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    Ok(ToolOutput {
        success: output.status.success(),
        stdout,
        stderr,
        exit_code: code,
        format: if output.status.success() {
            domain::tools::OutputFormat::Text
        } else {
            domain::tools::OutputFormat::Error
        },
        metadata: Default::default(),
    })
}

pub fn run_bash(command: &str) -> Result<ToolOutput, ToolError> {
    run_process("bash", &["-lc", command])
}

pub fn ensure_args_at_least(args: &[&str], min: usize, usage: &str) -> Result<(), ToolError> {
    if args.len() < min {
        return Err(ToolError::InvalidArguments(format!(
            "expected at least {min} args, usage: {usage}"
        )));
    }
    Ok(())
}
