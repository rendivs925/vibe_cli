use super::CliHandlers;
use colored::Colorize;
use shared::types::Result;

impl CliHandlers {
    pub async fn handle_neurosymbolic_init(&self) -> Result<()> {
        let config_dir = self.config_dir();

        println!(
            "{}",
            "Initializing complete Linux symbolic reasoning domain..."
                .green()
                .bold()
        );

        if config_dir.exists() {
            println!(
                "{}",
                "Domain config directory already exists. Updating...".yellow()
            );
        } else {
            std::fs::create_dir_all(&config_dir)?;
        }

        let linux_dir = config_dir.join("linux/entities");
        let shared_dir = config_dir.join("../shared_entities");

        std::fs::create_dir_all(&linux_dir)?;
        std::fs::create_dir_all(&shared_dir)?;

        println!("{}", "Creating Linux symbolic reasoning domain...".green());

        let domain_json = include_str!("../domain_templates/linux/domain.json");
        std::fs::write(config_dir.join("linux/domain.json"), domain_json)?;
        println!("  {}", "OK domain.json");

        let ops_json = include_str!("../domain_templates/linux/operations.json");

        let ops_path = config_dir.join("linux/operations.json");
        let base_ops: Vec<serde_json::Value> = serde_json::from_str(ops_json)?;
        let mut merged_ops: Vec<serde_json::Value> = Vec::new();

        if ops_path.exists() {
            if let Ok(existing_data) = std::fs::read_to_string(&ops_path) {
                if let Ok(existing_ops) =
                    serde_json::from_str::<Vec<serde_json::Value>>(&existing_data)
                {
                    merged_ops.extend(existing_ops);
                }
            }
        }

        for base in base_ops {
            let base_id = base.get("op_id").and_then(|v| v.as_str()).unwrap_or("");
            if base_id.is_empty() {
                continue;
            }
            let exists = merged_ops.iter().any(|op| {
                op.get("op_id")
                    .and_then(|v| v.as_str())
                    .map(|id| id == base_id)
                    .unwrap_or(false)
            });
            if !exists {
                merged_ops.push(base);
            }
        }

        let output = serde_json::to_string_pretty(&merged_ops)?;
        std::fs::write(&ops_path, output)?;
        println!("  {}", "OK operations.json (merged)");

        let entity_files = [
            ("process.json", include_str!("../domain_templates/linux/entities/process.json")),
            ("file.json", include_str!("../domain_templates/linux/entities/file.json")),
            ("service.json", include_str!("../domain_templates/linux/entities/service.json")),
            ("network_connection.json", include_str!("../domain_templates/linux/entities/network_connection.json")),
            ("user.json", include_str!("../domain_templates/linux/entities/user.json")),
            ("filesystem.json", include_str!("../domain_templates/linux/entities/filesystem.json")),
            ("memory.json", include_str!("../domain_templates/linux/entities/memory.json")),
            ("cpu.json", include_str!("../domain_templates/linux/entities/cpu.json")),
            ("network_interface.json", include_str!("../domain_templates/linux/entities/network_interface.json")),
            ("docker_container.json", include_str!("../domain_templates/linux/entities/docker_container.json")),
            ("systemd_unit.json", include_str!("../domain_templates/linux/entities/systemd_unit.json")),
        ];
        for (name, content) in entity_files {
            std::fs::write(linux_dir.join(name), content)?;
        }

        println!("  {}", "OK entities/ (11 entities: Process, File, Service, NetworkConnection, User, Filesystem, MemoryInfo, Cpu, NetworkInterface, DockerContainer, SystemdUnit)");

        let relationships_json = include_str!("../domain_templates/linux/relationships.json");
        std::fs::write(config_dir.join("linux/relationships.json"), relationships_json)?;
        println!("  {}", "OK relationships.json (8 relationships)");

        let inference_rules_json = include_str!("../domain_templates/linux/inference_rules.json");
        std::fs::write(
            config_dir.join("linux/inference_rules.json"),
            inference_rules_json,
        )?;
        println!("  {}", "OK inference_rules.json (30 inference rules)");

        let troubleshooting_json = include_str!("../domain_templates/linux/troubleshooting.json");
        std::fs::write(
            config_dir.join("linux/troubleshooting.json"),
            troubleshooting_json,
        )?;
        println!(
            "  {}",
            "OK troubleshooting.json (15 troubleshooting patterns)"
        );

        let reasoning_templates_json =
            include_str!("../domain_templates/linux/reasoning_templates.json");

        let templates_path = config_dir.join("linux/reasoning_templates.json");
        let base_templates: Vec<serde_json::Value> =
            serde_json::from_str(reasoning_templates_json)?;
        let mut merged_templates: Vec<serde_json::Value> = Vec::new();

        if templates_path.exists() {
            if let Ok(existing_data) = std::fs::read_to_string(&templates_path) {
                if let Ok(existing_templates) =
                    serde_json::from_str::<Vec<serde_json::Value>>(&existing_data)
                {
                    merged_templates.extend(existing_templates);
                }
            }
        }

        for base in base_templates {
            let base_id = base.get("template_id").and_then(|v| v.as_str()).unwrap_or("");
            if base_id.is_empty() {
                continue;
            }
            let exists = merged_templates.iter().any(|tpl| {
                tpl.get("template_id")
                    .and_then(|v| v.as_str())
                    .map(|id| id == base_id)
                    .unwrap_or(false)
            });
            if !exists {
                merged_templates.push(base);
            }
        }

        let templates_output = serde_json::to_string_pretty(&merged_templates)?;
        std::fs::write(&templates_path, templates_output)?;
        println!("  {}", "OK reasoning_templates.json (6 templates)");

        let shared_port = include_str!("../domain_templates/shared/port.json");
        std::fs::write(shared_dir.join("port.json"), shared_port)?;

        println!(
            "\n{}",
            "OK Linux symbolic reasoning domain initialized!"
                .green()
                .bold()
        );
        println!("\n{}", "Summary:".green());
        println!("  - 32 operations (process, memory, disk, network, services, files, containers, hardware, security, etc.)");
        println!("  - 11 entities (Process, File, Service, NetworkConnection, User, Filesystem, MemoryInfo, Cpu, NetworkInterface, DockerContainer, SystemdUnit)");
        println!("  - 8 relationships (hierarchical, ownership, containment, etc.)");
        println!("  - 30 inference rules for symbolic reasoning");
        println!("  - 15 troubleshooting patterns for common issues");
        println!("  - 21 reasoning templates for step-by-step diagnostics");

        println!("\n{}", "Usage:".green());
        println!("  vibe_cli \"list processes\"");
        println!("  vibe_cli \"check disk usage\"");
        println!("  vibe_cli \"nginx is not running\"");
        println!("  vibe_cli \"memory is full\"");

        Ok(())
    }
}
