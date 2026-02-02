use clap::Parser;
use presentation::cli::main::{Cli, CliApp};
use shared::types::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut app = CliApp::new();
    app.run(cli).await
}
