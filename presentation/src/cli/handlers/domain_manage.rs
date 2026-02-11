use super::CliHandlers;
use colored::Colorize;
use shared::types::Result;
use std::process::Command;

impl CliHandlers {
    pub async fn handle_neurosymbolic_install(&self, package: &str) -> Result<()> {
        let config_dir = self.config_dir();

        println!(
            "{}",
            format!("Installing domain package: {}", package)
                .green()
                .bold()
        );

        if package.starts_with("http://") || package.starts_with("https://") {
            println!("{}", "Downloading from URL...".yellow());
            let client = reqwest::Client::new();
            let response = client.get(package).send().await?;

            if response.status().is_success() {
                let content = response.text().await?;
                let domain_name = package
                    .split('/')
                    .last()
                    .unwrap_or(package)
                    .replace(".json", "");

                let target_dir = config_dir.join(&domain_name);
                std::fs::create_dir_all(&target_dir)?;
                std::fs::write(target_dir.join("domain.json"), content)?;

                println!("{}", format!("Installed domain: {}", domain_name).green());
            } else {
                eprintln!("{}", "Failed to download package".red());
            }
        } else {
            println!(
                "{}",
                format!("Looking for local package: {}", package).yellow()
            );
            let package_dir = std::path::Path::new(package);
            if package_dir.exists() && package_dir.is_dir() {
                let domain_name = package_dir
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                let target_dir = config_dir.join(&domain_name);
                std::fs::create_dir_all(&target_dir)?;

                for entry in std::fs::read_dir(package_dir)? {
                    let entry = entry?;
                    if entry.path().is_file() {
                        let file_name = entry.file_name();
                        std::fs::copy(entry.path(), target_dir.join(&file_name))?;
                    }
                }

                println!("{}", format!("Installed domain: {}", domain_name).green());
            } else {
                eprintln!("{}", format!("Package not found: {}", package).red());
            }
        }

        Ok(())
    }

    pub async fn handle_neurosymbolic_remove(&self, domain: &str) -> Result<()> {
        let config_dir = self.config_dir();
        let domain_dir = config_dir.join(domain);

        println!("{}", format!("Removing domain: {}", domain).green().bold());

        if domain_dir.exists() {
            std::fs::remove_dir_all(&domain_dir)?;
            println!("{}", format!("Removed: {}", domain_dir.display()).green());
        } else {
            println!("{}", format!("Domain not found: {}", domain).yellow());
        }

        Ok(())
    }

    pub async fn handle_neurosymbolic_edit(&self, domain: &str) -> Result<()> {
        let config_dir = self.config_dir();
        let domain_dir = config_dir.join(domain);

        println!("{}", format!("Editing domain: {}", domain).green().bold());

        if !domain_dir.exists() {
            println!(
                "{}",
                format!(
                    "Domain not found: {}. Use --neurosymbolic-add to create it.",
                    domain
                )
                .yellow()
            );
            return Ok(());
        }

        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());

        for entry in std::fs::read_dir(&domain_dir)? {
            let entry = entry?;
            if entry.path().is_file() {
                let file_name = entry.file_name();
                println!("{}", format!("Opening: {}", file_name.display()).yellow());

                let status = Command::new(&editor).arg(entry.path()).status()?;

                if status.success() {
                    println!("{}", format!("Saved: {}", file_name.display()).green());
                }
            }
        }

        Ok(())
    }

    pub async fn handle_neurosymbolic_add(&self, domain: &str) -> Result<()> {
        let config_dir = self.config_dir();
        let domain_dir = config_dir.join(domain);

        println!(
            "{}",
            format!("Adding new domain: {}", domain).green().bold()
        );

        std::fs::create_dir_all(&domain_dir.join("entities"))?;

        let domain_json = format!(
            r#"{{
    \"domain\": \"{}\",
    \"version\": \"1.0.0\",
    \"description\": \"Custom domain: {}\",
    \"depends_on\": [],
    \"priority\": 50,
    \"enabled\": true
}}"#,
            domain, domain
        );

        std::fs::write(domain_dir.join("domain.json"), &domain_json)?;
        println!("{}", format!("Created: {}/domain.json", domain).green());

        let ops_json = r#"[
    {
        \"op_id\": \"custom_operation\",
        \"name\": \"Custom Operation\",
        \"description\": \"Description of your custom operation\",
        \"input_schema\": {},
        \"generators\": [
            {
                \"name\": \"custom_tool\",
                \"tool\": \"your-tool\",
                \"template\": \"your-tool --option value\",
                \"when\": []
            }
        ],
        \"examples\": []
    }
]"#;

        std::fs::write(domain_dir.join("operations.json"), ops_json)?;
        println!("{}", format!("Created: {}/operations.json", domain).green());

        std::fs::write(domain_dir.join("relationships.json"), "[]")?;
        std::fs::write(domain_dir.join("inference_rules.json"), "[]")?;
        std::fs::write(domain_dir.join("troubleshooting.json"), "[]")?;

        println!("\n{}", "Domain template created!".green().bold());
        println!(
            "{}",
            format!("Edit with: vibe_cli --neurosymbolic-edit {}", domain).yellow()
        );

        Ok(())
    }

    pub async fn handle_neurosymbolic_list(&self) -> Result<()> {
        let config_dir = self.config_dir();

        println!("{}", "Installed Domains".green().bold());
        println!("{}", "==============".to_string());

        if !config_dir.exists() {
            println!(
                "{}",
                "No domains installed. Run --neurosymbolic-init first.".yellow()
            );
            return Ok(());
        }

        let mut domains: Vec<String> = Vec::new();

        for entry in std::fs::read_dir(&config_dir)? {
            let entry = entry?;
            if entry.path().is_dir() {
                let domain_name = entry.file_name().to_string_lossy().to_string();
                let domain_json = entry.path().join("domain.json");

                if domain_json.exists() {
                    if let Ok(content) = std::fs::read_to_string(&domain_json) {
                        if let Ok(domain) = serde_json::from_str::<serde_json::Value>(&content) {
                            let desc = domain
                                .get("description")
                                .and_then(|d| d.as_str())
                                .unwrap_or("");
                            let enabled = domain
                                .get("enabled")
                                .and_then(|e| e.as_bool())
                                .unwrap_or(true);

                            let status = if enabled { "enabled" } else { "disabled" };
                            println!("  {} - {} [{}]", domain_name.green().bold(), desc, status);
                            domains.push(domain_name);
                        }
                    }
                }
            }
        }

        if domains.is_empty() {
            println!("{}", "No domains found.".yellow());
        } else {
            println!("\n{}", format!("Total: {} domain(s)", domains.len()).cyan());
        }

        println!("\n{}", "Usage:".green());
        println!("  vibe_cli \"your query\"");
        println!("  vibe_cli --neurosymbolic-edit <domain>  # Edit a domain");
        println!("  vibe_cli --neurosymbolic-remove <domain>  # Remove a domain");

        Ok(())
    }
}
