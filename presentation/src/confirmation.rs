use crate::analysis::assess_command_risk;
use crate::types::{CommandIntent, CommandRisk, InstallationOption};
use colored::Colorize;

/// Present confirmation dialog for data collection commands
pub fn prompt_data_collection_confirmation(
    command: &str,
    query: &str,
    risk: CommandRisk,
) -> anyhow::Result<bool> {
    // println!("DATA COLLECTION REQUIRED");
    println!("{}", format!("Command: {}", command).green());

    // Determine purpose based on query content
    // let purpose = if query.to_lowercase().contains("gpu")
    //     || query.to_lowercase().contains("graphics")
    // {
    //     "Gather GPU information for analysis"
    // } else if query.to_lowercase().contains("cpu") || query.to_lowercase().contains("processor") {
    //     "Gather CPU information for analysis"
    // } else if query.to_lowercase().contains("ram") || query.to_lowercase().contains("memory") {
    //     "Gather memory information for analysis"
    // } else if query.to_lowercase().contains("disk") || query.to_lowercase().contains("storage") {
    //     "Gather disk usage information for analysis"
    // } else if query.to_lowercase().contains("network") {
    //     "Gather network information for analysis"
    // } else {
    //     "Gather system information for analysis"
    // };

    // println!("Purpose: {}", purpose);

    // Show safety level
    let safety_desc = match risk {
        CommandRisk::InfoOnly => "Read-only, no system modifications",
        CommandRisk::SafeSetup => "Safe system query with minimal impact",
        CommandRisk::SystemChanges => "May modify system configuration",
        CommandRisk::HighRisk => "High-risk operation requiring careful review",
        CommandRisk::Unknown => "Risk level cannot be determined",
    };

    // println!("Safety: {}", safety_desc);

    shared::confirmation::ask_confirmation(
        "Allow command execution?",
        risk == CommandRisk::InfoOnly,
    )
}

/// Present confirmation dialog for installation commands
pub fn prompt_installation_confirmation(
    command: &str,
    intent: CommandIntent,
    packages: Vec<String>,
    services: Vec<String>,
    disk_space: Option<String>,
) -> anyhow::Result<bool> {
    // println!("INSTALLATION COMMAND DETECTED");
    println!();
    println!("{}", format!("Command: {}", command).green());

    if !packages.is_empty() {
        println!();
        println!("Packages to install:");
        for package in &packages {
            println!("  - {}", package);
        }
    }

    if !services.is_empty() {
        println!();
        println!("System changes:");
        for service in &services {
            println!("  - New service: {}", service);
        }
    }

    if let Some(space) = &disk_space {
        println!("  - Disk space: {}", space);
    }

    // Check if sudo is needed
    let needs_sudo =
        command.contains("sudo") || assess_command_risk(command) != CommandRisk::InfoOnly;
    if needs_sudo {
        println!("  - Requires: sudo privileges");
    }

    println!();

    // Default to 'No' for installations unless it's very safe
    shared::confirmation::ask_confirmation("Execute installation?", false)
}

/// Analyze installation command to extract details
pub fn analyze_installation_command(command: &str) -> (Vec<String>, Vec<String>, Option<String>) {
    let mut packages = Vec::new();
    let mut services = Vec::new();
    let mut disk_space = None;

    // Extract package names from common install commands
    let cmd_lower = command.to_lowercase();

    if cmd_lower.contains("apt install") || cmd_lower.contains("apt-get install") {
        // Extract package names after "install"
        if let Some(install_pos) = cmd_lower.find("install") {
            let package_part = &command[install_pos + 7..].trim();
            packages = package_part
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();
        }
    } else if cmd_lower.contains("pip install") {
        if let Some(install_pos) = cmd_lower.find("install") {
            let package_part = &command[install_pos + 7..].trim();
            packages = package_part
                .split_whitespace()
                .take(3) // Limit to first few packages
                .map(|s| format!("{} (Python package)", s))
                .collect();
        }
    }

    // Estimate disk space based on packages
    if !packages.is_empty() {
        if packages.len() == 1 {
            disk_space = Some("~50MB".to_string());
        } else if packages.len() <= 3 {
            disk_space = Some("~100MB".to_string());
        } else {
            disk_space = Some("~250MB".to_string());
        }
    }

    // Identify services that might be started
    for package in &packages {
        let pkg_lower = package.to_lowercase();
        if pkg_lower.contains("nginx") || pkg_lower == "nginx" {
            services.push("nginx".to_string());
        } else if pkg_lower.contains("apache") || pkg_lower.contains("httpd") {
            services.push("apache2".to_string());
        } else if pkg_lower.contains("mysql") || pkg_lower.contains("mariadb") {
            services.push("mysql".to_string());
        } else if pkg_lower.contains("postgresql") {
            services.push("postgresql".to_string());
        }
    }

    (packages, services, disk_space)
}
