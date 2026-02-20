use super::CliHandlers;
use colored::Colorize;
use shared::theme;
use shared::confirmation::ask_confirmation;
use shared::types::Result;

impl CliHandlers {
    pub(crate) async fn learn_command(&mut self, query: &str, command: &str) -> Result<()> {
        if self.is_known_operation(query, command) {
            return Ok(());
        }

        println!(
            "\n{}",
            theme::accent("=== Learning New Command ===").bold()
        );

        let operation_name = Self::generate_operation_name(query);
        let operation_id = operation_name.to_lowercase().replace(" ", "_");

        let client = infrastructure::ollama_client::OllamaClient::new()?;
        let desc_prompt = format!(
            "Generate a short (max 80 chars) description for this command: {}\n\
             Query was: {}\n\
             Just return the description, no formatting.",
            command, query
        );
        let description = client.generate_response(&desc_prompt).await?;

        let tool = command.split_whitespace().next().unwrap_or("bash");
        let template = command;

        println!("Operation Name: {}", operation_name);
        println!("Operation ID: {}", operation_id);
        println!("Description: {}", description.trim());
        println!("Tool: {}", tool);
        println!("Template: {}", template);

        if ask_confirmation("Save this operation to the Linux domain?", false)? {
            let domains_dir = self.config_dir();
            let linux_dir = domains_dir.join("linux");

            if !linux_dir.exists() {
                std::fs::create_dir_all(&linux_dir)?;
            }

            let ops_file = linux_dir.join("operations.json");

            let mut operations: Vec<serde_json::Value> = if ops_file.exists() {
                let data = std::fs::read_to_string(&ops_file)?;
                serde_json::from_str(&data)?
            } else {
                Vec::new()
            };

            let new_op = serde_json::json!({
                "op_id": operation_id,
                "name": operation_name,
                "description": description.trim(),
                "input_schema": {},
                "generators": [
                    {
                        "name": format!("{}_generator", operation_id),
                        "tool": tool,
                        "template": template,
                        "when": []
                    }
                ],
                "examples": [
                    {
                        "description": query,
                        "inputs": {}
                    }
                ]
            });

            operations.push(new_op);

            let output = serde_json::to_string_pretty(&operations)?;
            std::fs::write(&ops_file, output)?;

            println!(
                "\n{}",
                theme::success(&format!("Saved new operation to: {}", ops_file.display()))
            );
            if let Some(service) = self.neurosymbolic_service.as_mut() {
                let _ = service.reload_domain_registry();
            }
        }

        Ok(())
    }

    pub(crate) fn is_known_operation(&self, query: &str, command: &str) -> bool {
        let ops_file = self.config_dir().join("linux").join("operations.json");
        if !ops_file.exists() {
            return false;
        }

        let data = match std::fs::read_to_string(&ops_file) {
            Ok(data) => data,
            Err(_) => return false,
        };

        let operations: Vec<serde_json::Value> = match serde_json::from_str(&data) {
            Ok(ops) => ops,
            Err(_) => return false,
        };

        for op in operations {
            let examples = op
                .get("examples")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            for ex in examples {
                if let Some(desc) = ex.get("description").and_then(|v| v.as_str()) {
                    if desc.eq_ignore_ascii_case(query) {
                        return true;
                    }
                }
            }

            let generators = op
                .get("generators")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            for gen in generators {
                if let Some(template) = gen.get("template").and_then(|v| v.as_str()) {
                    if template.trim() == command.trim() {
                        return true;
                    }
                }
            }
        }

        false
    }

    pub(crate) fn generate_operation_name(query: &str) -> String {
        let words: Vec<&str> = query.split_whitespace().collect();

        let action_words: Vec<&str> = words
            .iter()
            .filter(|w| {
                let w = w.to_lowercase();
                ["check", "show", "list", "get", "find", "view", "display"].contains(&w.as_str())
            })
            .copied()
            .collect();

        if !action_words.is_empty() {
            let rest: Vec<&str> = words
                .iter()
                .filter(|w| !action_words.contains(w))
                .copied()
                .collect();

            let capitalized: Vec<String> = rest
                .iter()
                .map(|w| {
                    let mut chars = w.chars();
                    match chars.next() {
                        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                        None => String::new(),
                    }
                })
                .collect();

            format!(
                "{} {}",
                action_words[0].to_lowercase() + " " + &capitalized.join(" "),
                if query.to_lowercase().contains("log") || query.to_lowercase().contains("journal")
                {
                    "logs"
                } else if query.to_lowercase().contains("line") {
                    "output"
                } else {
                    "info"
                }
            )
            .trim()
            .to_string()
        } else {
            format!(
                "Check {} info",
                words
                    .first()
                    .map(|w| {
                        let mut chars = w.chars();
                        match chars.next() {
                            Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                            None => "System".to_string(),
                        }
                    })
                    .unwrap_or_else(|| "System".to_string())
            )
        }
    }
}
