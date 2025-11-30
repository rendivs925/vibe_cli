use clap::Parser;
use presentation::cli::{Cli, CliApp};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let mut app = CliApp::new();
    app.run(cli).await?;
    Ok(())
}
