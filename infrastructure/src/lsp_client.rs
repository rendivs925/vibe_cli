use anyhow::{Context, Result};
use flume::Sender;
use std::path::PathBuf;
use tokio::process::Command;

/// LSP client for rust-analyzer integration
pub struct LspClient;

impl LspClient {
    /// Start a new LSP client for rust-analyzer
    pub async fn start_rust_analyzer(
        project_root: PathBuf,
        event_tx: Sender<super::background_supervisor::BackgroundEvent>,
    ) -> Result<Self> {
        println!("  └─ 🔍 Starting rust-analyzer LSP client...");

        // For now, just start the process and let it run
        // Basic LSP process management - full protocol integration planned
        let mut child = Command::new("rust-analyzer")
            .current_dir(&project_root)
            .spawn()
            .context("Failed to start rust-analyzer. Make sure it's installed.")?;

        // Wait for the process to start
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Check if the process is still running
        if let Ok(Some(_)) = child.try_wait() {
            println!("  └─ ⚠️ rust-analyzer process exited early");
            return Err(anyhow::anyhow!("rust-analyzer exited early"));
        }

        // For demonstration, send a sample diagnostic immediately
        let sample_event = super::background_supervisor::BackgroundEvent::LspDiagnostic {
            file: project_root.join("src").join("main.rs"),
            severity: super::background_supervisor::DiagnosticSeverity::Information,
            message: "LSP client connected - rust-analyzer is active".to_string(),
        };

        let _ = event_tx.send(sample_event);

        println!("  └─ ✅ rust-analyzer LSP client started (basic monitoring)");
        Ok(Self)
    }
}
