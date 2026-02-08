//! Graph Builder - auto-discovers system state for knowledge graph
//!
//! Scans the system to populate the knowledge graph with:
//! - OS information
//! - Installed tools and packages
//! - Users and permissions
//! - Services and processes

use infrastructure::storage::knowledge_graph::{EntityType, KnowledgeGraph};
use shared::types::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

/// Builds and maintains the system knowledge graph
pub struct GraphBuilder {
    graph: KnowledgeGraph,
}

impl GraphBuilder {
    /// Create a new graph builder with default storage location
    pub fn new() -> Result<Self> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let db_path = PathBuf::from(home).join(".config/vibe_cli/knowledge_graph.db");

        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let graph = KnowledgeGraph::new(&db_path)?;
        Ok(Self { graph })
    }

    /// Create with custom database path
    pub fn with_path(db_path: PathBuf) -> Result<Self> {
        let graph = KnowledgeGraph::new(&db_path)?;
        Ok(Self { graph })
    }

    /// Perform full system discovery
    pub fn discover_system(&self) -> Result<DiscoveryReport> {
        let mut report = DiscoveryReport::new();

        // Discover OS information
        self.discover_os()?;
        report.add_section("OS Information", "discovered");

        // Discover tools
        let tools = self.discover_tools()?;
        report.add_section("Tools", &format!("{} found", tools));

        // Discover users
        let users = self.discover_users()?;
        report.add_section("Users", &format!("{} found", users));

        // Discover services
        let services = self.discover_services()?;
        report.add_section("Services", &format!("{} found", services));

        // Discover environment variables
        self.discover_env_vars()?;
        report.add_section("Environment", "discovered");

        Ok(report)
    }

    /// Discover OS information
    fn discover_os(&self) -> Result<()> {
        // Try to get OS info from various sources
        let os_type = self
            .run_command("uname", &["-s"])
            .unwrap_or_else(|| "Unknown".to_string());

        let kernel_version = self
            .run_command("uname", &["-r"])
            .unwrap_or_else(|| "Unknown".to_string());

        let hostname = self
            .run_command("hostname", &[])
            .unwrap_or_else(|| "localhost".to_string());

        // Try to get distribution info
        let distribution = self.get_distribution();

        // Add OS entity
        let mut attrs = HashMap::new();
        attrs.insert("type".to_string(), os_type.clone());
        attrs.insert("kernel".to_string(), kernel_version.clone());
        attrs.insert("hostname".to_string(), hostname);

        self.graph
            .add_entity(EntityType::OperatingSystem, &os_type, attrs)?;

        // Add kernel entity
        let mut kernel_attrs = HashMap::new();
        kernel_attrs.insert("version".to_string(), kernel_version);
        self.graph
            .add_entity(EntityType::Kernel, "kernel", kernel_attrs)?;

        // Add distribution if found
        if let Some((distro, version)) = distribution {
            let mut distro_attrs = HashMap::new();
            distro_attrs.insert("version".to_string(), version);
            self.graph
                .add_entity(EntityType::Distribution, &distro, distro_attrs)?;
        }

        Ok(())
    }

    /// Get Linux distribution info
    fn get_distribution(&self) -> Option<(String, String)> {
        // Try /etc/os-release first
        if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
            let mut name = None;
            let mut version = None;

            for line in content.lines() {
                if line.starts_with("NAME=") {
                    name = Some(line[5..].trim_matches('"').to_string());
                } else if line.starts_with("VERSION_ID=") {
                    version = Some(line[11..].trim_matches('"').to_string());
                }
            }

            if let (Some(n), Some(v)) = (name, version) {
                return Some((n, v));
            }
        }

        None
    }

    /// Discover installed tools
    fn discover_tools(&self) -> Result<usize> {
        let mut count = 0;

        // Common tools to check
        let common_tools = vec![
            "ls",
            "cat",
            "grep",
            "awk",
            "sed",
            "find",
            "tar",
            "gzip",
            "git",
            "curl",
            "wget",
            "ssh",
            "docker",
            "python3",
            "node",
            "npm",
            "cargo",
            "rustc",
            "go",
            "java",
            "javac",
            "make",
            "cmake",
            "gcc",
            "g++",
            "clang",
            "vim",
            "nvim",
            "nano",
            "htop",
            "top",
            "ps",
            "kill",
            "systemctl",
            "service",
            "apt",
            "yum",
            "dnf",
            "pacman",
            "brew",
        ];

        for tool in common_tools {
            if self.command_exists(tool) {
                let path = self
                    .get_command_path(tool)
                    .unwrap_or_else(|| format!("/usr/bin/{}", tool));

                let version = self.get_tool_version(tool);

                let mut attrs = HashMap::new();
                attrs.insert("path".to_string(), path);
                if let Some(v) = version {
                    attrs.insert("version".to_string(), v);
                }

                self.graph.add_entity(EntityType::Tool, tool, attrs)?;
                count += 1;
            }
        }

        Ok(count)
    }

    /// Check if a command exists
    fn command_exists(&self, cmd: &str) -> bool {
        Command::new("command")
            .args(&["-v", cmd])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Get command path
    fn get_command_path(&self, cmd: &str) -> Option<String> {
        Command::new("which").arg(cmd).output().ok().and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })
    }

    /// Get tool version
    fn get_tool_version(&self, tool: &str) -> Option<String> {
        // Try common version flags
        let version_flags = vec!["--version", "-v", "-V", "version"];

        for flag in version_flags {
            if let Ok(output) = Command::new(tool).arg(flag).output() {
                if output.status.success() {
                    if let Ok(stdout) = String::from_utf8(output.stdout) {
                        // Extract first line or first version-like string
                        let first_line = stdout.lines().next().unwrap_or("");
                        if !first_line.is_empty() {
                            return Some(first_line.trim().to_string());
                        }
                    }
                }
            }
        }

        None
    }

    /// Discover users
    fn discover_users(&self) -> Result<usize> {
        let mut count = 0;

        // Try to read /etc/passwd
        if let Ok(content) = std::fs::read_to_string("/etc/passwd") {
            for line in content.lines() {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 3 {
                    let username = parts[0];
                    let uid = parts[2];
                    let home = parts.get(5).unwrap_or(&"");
                    let shell = parts.get(6).unwrap_or(&"");

                    let mut attrs = HashMap::new();
                    attrs.insert("uid".to_string(), uid.to_string());
                    attrs.insert("home".to_string(), home.to_string());
                    attrs.insert("shell".to_string(), shell.to_string());

                    // Check if root
                    if uid == "0" {
                        attrs.insert("is_root".to_string(), "true".to_string());
                    }

                    self.graph.add_entity(EntityType::User, username, attrs)?;
                    count += 1;
                }
            }
        }

        // Also add current user
        if let Ok(current_user) = std::env::var("USER") {
            if self
                .graph
                .find_entity(EntityType::User, &current_user)?
                .is_none()
            {
                let mut attrs = HashMap::new();
                attrs.insert("current".to_string(), "true".to_string());
                self.graph
                    .add_entity(EntityType::User, &current_user, attrs)?;
                count += 1;
            }
        }

        Ok(count)
    }

    /// Discover system services
    fn discover_services(&self) -> Result<usize> {
        let mut count = 0;

        // Try systemctl
        if self.command_exists("systemctl") {
            if let Ok(output) = Command::new("systemctl")
                .args(&[
                    "list-units",
                    "--type=service",
                    "--state=running",
                    "--no-pager",
                    "--no-legend",
                ])
                .output()
            {
                if output.status.success() {
                    if let Ok(stdout) = String::from_utf8(output.stdout) {
                        for line in stdout.lines().take(50) {
                            // Parse service name
                            let parts: Vec<&str> = line.split_whitespace().collect();
                            if let Some(service_name) = parts.first() {
                                let name = service_name.replace(".service", "");

                                let mut attrs = HashMap::new();
                                attrs.insert("status".to_string(), "running".to_string());

                                self.graph.add_entity(EntityType::Service, &name, attrs)?;
                                count += 1;
                            }
                        }
                    }
                }
            }
        }

        Ok(count)
    }

    /// Discover environment variables
    fn discover_env_vars(&self) -> Result<()> {
        let important_vars = vec![
            "HOME",
            "USER",
            "SHELL",
            "PATH",
            "EDITOR",
            "LANG",
            "XDG_SESSION_TYPE",
            "XDG_CURRENT_DESKTOP",
            "DISPLAY",
        ];

        for var in important_vars {
            if let Ok(value) = std::env::var(var) {
                let mut attrs = HashMap::new();
                attrs.insert("value".to_string(), value);

                self.graph
                    .add_entity(EntityType::EnvironmentVariable, var, attrs)?;
            }
        }

        Ok(())
    }

    /// Get knowledge graph reference
    pub fn graph(&self) -> &KnowledgeGraph {
        &self.graph
    }

    /// Run a command and return output
    fn run_command(&self, cmd: &str, args: &[&str]) -> Option<String> {
        Command::new(cmd).args(args).output().ok().and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })
    }

    /// Get discovery statistics
    pub fn get_stats(&self) -> Result<(usize, usize)> {
        self.graph.stats()
    }

    /// Clear and rebuild the graph
    pub fn rebuild(&self) -> Result<DiscoveryReport> {
        self.graph.clear_all()?;
        self.discover_system()
    }
}

/// Report from discovery process
#[derive(Debug, Clone)]
pub struct DiscoveryReport {
    sections: Vec<(String, String)>,
}

impl DiscoveryReport {
    fn new() -> Self {
        Self { sections: vec![] }
    }

    fn add_section(&mut self, name: &str, status: &str) {
        self.sections.push((name.to_string(), status.to_string()));
    }

    /// Format report for display
    pub fn format_display(&self) -> String {
        let mut output = String::from("System Discovery Report:\n");
        output.push_str("========================\n\n");

        for (name, status) in &self.sections {
            output.push_str(&format!("✓ {}: {}\n", name, status));
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_db_path() -> PathBuf {
        PathBuf::from("/tmp/test_graph_builder.db")
    }

    #[test]
    fn test_graph_builder_creation() {
        let _ = std::fs::remove_file(test_db_path());
        let builder = GraphBuilder::with_path(test_db_path()).unwrap();
        let (entities, _) = builder.get_stats().unwrap();
        assert_eq!(entities, 0);
        let _ = std::fs::remove_file(test_db_path());
    }

    #[test]
    fn test_discover_os() {
        let _ = std::fs::remove_file(test_db_path());
        let builder = GraphBuilder::with_path(test_db_path()).unwrap();

        builder.discover_os().unwrap();

        let (entities, _) = builder.get_stats().unwrap();
        assert!(entities > 0);

        let _ = std::fs::remove_file(test_db_path());
    }

    #[test]
    fn test_discover_env_vars() {
        let _ = std::fs::remove_file(test_db_path());
        let builder = GraphBuilder::with_path(test_db_path()).unwrap();

        builder.discover_env_vars().unwrap();

        let (entities, _) = builder.get_stats().unwrap();
        assert!(entities > 0);

        let _ = std::fs::remove_file(test_db_path());
    }

    #[test]
    fn test_report_formatting() {
        let mut report = DiscoveryReport::new();
        report.add_section("Test Section", "completed");

        let formatted = report.format_display();
        assert!(formatted.contains("Test Section"));
        assert!(formatted.contains("completed"));
    }
}
