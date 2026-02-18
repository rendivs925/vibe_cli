use domain::tools::{ToolError, ToolOutput};
use std::env;
use std::path::Path;
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
    run_shell(command)
}

pub fn run_shell(command: &str) -> Result<ToolOutput, ToolError> {
    let shell = resolve_shell_program();
    run_process(&shell, &["-lc", command])
}

pub fn ensure_args_at_least(args: &[&str], min: usize, usage: &str) -> Result<(), ToolError> {
    if args.len() < min {
        return Err(ToolError::InvalidArguments(format!(
            "expected at least {min} args, usage: {usage}"
        )));
    }
    Ok(())
}

fn resolve_shell_program() -> String {
    if let Ok(shell) = env::var("VIBE_CLI_SHELL") {
        if is_executable(&shell) {
            return shell;
        }
    }

    if let Ok(shell) = env::var("SHELL") {
        if is_executable(&shell) {
            return shell;
        }
    }

    if has_in_path("zsh") {
        return "zsh".to_string();
    }

    "bash".to_string()
}

fn is_executable(path: &str) -> bool {
    if path.trim().is_empty() {
        return false;
    }
    if path.contains('/') {
        return Path::new(path).exists();
    }
    has_in_path(path)
}

fn has_in_path(bin: &str) -> bool {
    let Ok(path) = env::var("PATH") else {
        return false;
    };
    for entry in path.split(':') {
        let candidate = Path::new(entry).join(bin);
        if candidate.exists() {
            return true;
        }
    }
    false
}
