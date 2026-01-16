//! Background event handling and monitoring

use colored::Colorize;
use flume::Receiver;
use infrastructure::background_supervisor::{
    BackgroundEvent, DiagnosticSeverity, FileChangeType, GitStatus, LogLevel, TestStatus,
};

/// Handle background events from the supervisor
pub async fn handle_events(event_receiver: Receiver<BackgroundEvent>) {
    while let Ok(event) = event_receiver.recv_async().await {
        match event {
            BackgroundEvent::FileChanged { path, change_type } => {
                let (change_icon, change_str) = match change_type {
                    FileChangeType::Created => ("New", "created"),
                    FileChangeType::Modified => ("Edit", "modified"),
                    FileChangeType::Deleted => ("Del", "deleted"),
                    FileChangeType::Renamed => ("Ren", "renamed"),
                };
                println!("{} {} {}", change_icon, change_str, path.display());
            }
            BackgroundEvent::TestResult {
                session,
                status,
                output,
            } => {
                let (status_icon, _status_str) = match status {
                    TestStatus::Started => ("Run", "started"),
                    TestStatus::Passed => ("Pass", "passed"),
                    TestStatus::Failed { .. } => ("Fail", "failed"),
                    TestStatus::Completed => ("Done", "completed"),
                };
                println!(
                    "{} Test {}: {}",
                    status_icon,
                    session,
                    output.lines().next().unwrap_or("")
                );
            }
            BackgroundEvent::LogEntry {
                source,
                level,
                message,
            } => {
                let (level_icon, level_str) = match level {
                    LogLevel::Debug => ("Debug", "debug"),
                    LogLevel::Info => ("Info", "info"),
                    LogLevel::Warn => ("Warn", "warn"),
                    LogLevel::Error => ("Error", "error"),
                };
                println!("{} [{}] {}: {}", level_icon, source, level_str, message);
            }
            BackgroundEvent::LspDiagnostic {
                file,
                severity,
                message,
            } => {
                let severity_icon = match severity {
                    DiagnosticSeverity::Error => "Error",
                    DiagnosticSeverity::Warning => "Warn",
                    DiagnosticSeverity::Information => "Info",
                    DiagnosticSeverity::Hint => "Hint",
                };
                println!("{} {}: {}", severity_icon, file.display(), message);
            }
            BackgroundEvent::GitStatus { status } => match status {
                GitStatus::Clean => println!("{} Repository is clean", "Clean".green()),
                GitStatus::Dirty { modified_files } => {
                    println!("{} {} modified files", "Dirty".yellow(), modified_files.len());
                }
                GitStatus::Untracked { files } => {
                    println!("{} {} untracked files", "Untracked".yellow(), files.len());
                }
            },
        }
    }
}

/// Display background status header
pub fn display_background_status_header() {
    println!("\n{}Background Intelligence:", "Brain ".bright_blue());
}
