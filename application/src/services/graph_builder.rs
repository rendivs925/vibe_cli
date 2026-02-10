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

        // Discover hardware
        let hardware = self.discover_hardware()?;
        report.add_section("Hardware", &format!("{} entries", hardware));

        // Discover tools
        let tools = self.discover_tools()?;
        report.add_section("Tools", &format!("{} found", tools));

        // Discover users
        let users = self.discover_users()?;
        report.add_section("Users", &format!("{} found", users));

        // Discover services
        let services = self.discover_services()?;
        report.add_section("Services", &format!("{} found", services));

        // Discover containers
        let containers = self.discover_containers()?;
        report.add_section("Containers", &format!("{} found", containers));

        // Discover filesystems/mounts
        let mounts = self.discover_filesystems()?;
        report.add_section("Filesystems", &format!("{} found", mounts));

        // Discover network interfaces
        let interfaces = self.discover_network_interfaces()?;
        report.add_section("Network", &format!("{} interfaces", interfaces));

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

        let os_id = self
            .graph
            .upsert_entity(EntityType::OperatingSystem, &os_type, attrs)?;

        // Add kernel entity
        let mut kernel_attrs = HashMap::new();
        kernel_attrs.insert("version".to_string(), kernel_version);
        let kernel_id = self
            .graph
            .upsert_entity(EntityType::Kernel, "kernel", kernel_attrs)?;

        // Add distribution if found
        if let Some((distro, version, id_like)) = distribution {
            let mut distro_attrs = HashMap::new();
            distro_attrs.insert("version".to_string(), version);
            if let Some(id_like) = id_like {
                distro_attrs.insert("id_like".to_string(), id_like);
            }
            let distro_id = self
                .graph
                .upsert_entity(EntityType::Distribution, &distro, distro_attrs)?;

            let _ = self.graph.add_relationship_unique(
                os_id,
                distro_id,
                "distribution",
                HashMap::new(),
            );
            let _ = self.graph.add_relationship_unique(
                distro_id,
                kernel_id,
                "runs_on",
                HashMap::new(),
            );
        }

        Ok(())
    }

    /// Get Linux distribution info
    fn get_distribution(&self) -> Option<(String, String, Option<String>)> {
        // Try /etc/os-release first
        if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
            let mut name = None;
            let mut version = None;
            let mut id_like = None;

            for line in content.lines() {
                if line.starts_with("NAME=") {
                    name = Some(line[5..].trim_matches('"').to_string());
                } else if line.starts_with("VERSION_ID=") {
                    version = Some(line[11..].trim_matches('"').to_string());
                } else if line.starts_with("ID_LIKE=") {
                    id_like = Some(line[8..].trim_matches('"').to_string());
                }
            }

            if let (Some(n), Some(v)) = (name, version) {
                return Some((n, v, id_like));
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

                self.graph.upsert_entity(EntityType::Tool, tool, attrs)?;
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

                    self.graph.upsert_entity(EntityType::User, username, attrs)?;
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
                    .upsert_entity(EntityType::User, &current_user, attrs)?;
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

                                self.graph.upsert_entity(EntityType::Service, &name, attrs)?;
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
                    .upsert_entity(EntityType::EnvironmentVariable, var, attrs)?;
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
        Ok(self.graph.stats()?)
    }

    /// Clear and rebuild the graph
    pub fn rebuild(&self) -> Result<DiscoveryReport> {
        self.graph.clear_all()?;
        self.discover_system()
    }

    /// Discover CPU, memory, and disk summary
    fn discover_hardware(&self) -> Result<usize> {
        let mut count = 0;

        if let Ok(content) = std::fs::read_to_string("/proc/cpuinfo") {
            if let Some(model_line) = content.lines().find(|l| l.starts_with("model name")) {
                let model = model_line
                    .splitn(2, ':')
                    .nth(1)
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let cores = content
                    .lines()
                    .filter(|l| l.starts_with("processor"))
                    .count();
                let mut attrs = HashMap::new();
                attrs.insert("model".to_string(), model);
                attrs.insert("cores".to_string(), cores.to_string());
                self.graph.upsert_entity(EntityType::Cpu, "cpu", attrs)?;
                count += 1;
            }
        }

        if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
            let mut attrs = HashMap::new();
            for key in ["MemTotal", "MemFree", "MemAvailable", "SwapTotal", "SwapFree"] {
                if let Some(line) = content.lines().find(|l| l.starts_with(key)) {
                    let val = line.split_whitespace().nth(1).unwrap_or("");
                    attrs.insert(key.to_string(), val.to_string());
                }
            }
            if !attrs.is_empty() {
                self.graph
                    .upsert_entity(EntityType::Memory, "memory", attrs)?;
                count += 1;
            }
        }

        if self.command_exists("lsblk") {
            if let Ok(output) = Command::new("lsblk").arg("-o").arg("NAME,SIZE,TYPE").output() {
            if output.status.success() {
                if let Ok(stdout) = String::from_utf8(output.stdout) {
                    for line in stdout.lines().skip(1) {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 3 && parts[2] == "disk" {
                            let mut attrs = HashMap::new();
                            attrs.insert("size".to_string(), parts[1].to_string());
                            self.graph
                                .upsert_entity(EntityType::Disk, parts[0], attrs)?;
                            count += 1;
                        }
                    }
                }
            }
            }
        }

        Ok(count)
    }

    /// Discover container runtime information
    fn discover_containers(&self) -> Result<usize> {
        let mut count = 0;

        if self.command_exists("docker") {
            if let Ok(output) = Command::new("docker")
                .args(&["ps", "-a", "--format", "{{.ID}} {{.Names}} {{.Status}}"]) 
                .output()
            {
                if output.status.success() {
                    if let Ok(stdout) = String::from_utf8(output.stdout) {
                        for line in stdout.lines() {
                            let parts: Vec<&str> = line.splitn(3, ' ').collect();
                            if parts.len() >= 2 {
                                let name = parts[1];
                                let mut attrs = HashMap::new();
                                attrs.insert("runtime".to_string(), "docker".to_string());
                                if let Some(status) = parts.get(2) {
                                    attrs.insert("status".to_string(), status.to_string());
                                }
                                self.graph.upsert_entity(EntityType::Container, name, attrs)?;
                                count += 1;
                            }
                        }
                    }
                }
            }
        }

        if self.command_exists("podman") {
            if let Ok(output) = Command::new("podman")
                .args(&["ps", "-a", "--format", "{{.ID}} {{.Names}} {{.Status}}"])
                .output()
            {
                if output.status.success() {
                    if let Ok(stdout) = String::from_utf8(output.stdout) {
                        for line in stdout.lines() {
                            let parts: Vec<&str> = line.splitn(3, ' ').collect();
                            if parts.len() >= 2 {
                                let name = parts[1];
                                let mut attrs = HashMap::new();
                                attrs.insert("runtime".to_string(), "podman".to_string());
                                if let Some(status) = parts.get(2) {
                                    attrs.insert("status".to_string(), status.to_string());
                                }
                                self.graph.upsert_entity(EntityType::Container, name, attrs)?;
                                count += 1;
                            }
                        }
                    }
                }
            }
        }

        Ok(count)
    }

    /// Discover filesystems and mounts
    fn discover_filesystems(&self) -> Result<usize> {
        let mut count = 0;

        if let Ok(content) = std::fs::read_to_string("/proc/mounts") {
            for line in content.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    let device = parts[0];
                    let mount = parts[1];
                    let fstype = parts[2];

                    let mut fs_attrs = HashMap::new();
                    fs_attrs.insert("device".to_string(), device.to_string());
                    fs_attrs.insert("fstype".to_string(), fstype.to_string());
                    let fs_id = self
                        .graph
                        .upsert_entity(EntityType::Filesystem, mount, fs_attrs)?;

                    let mut mount_attrs = HashMap::new();
                    mount_attrs.insert("mountpoint".to_string(), mount.to_string());
                    let mount_id = self
                        .graph
                        .upsert_entity(EntityType::Mount, mount, mount_attrs)?;

                    let _ = self.graph.add_relationship_unique(
                        fs_id,
                        mount_id,
                        "mounted_at",
                        HashMap::new(),
                    );
                    count += 1;
                }
            }
        }

        Ok(count)
    }

    /// Discover network interfaces
    fn discover_network_interfaces(&self) -> Result<usize> {
        let mut count = 0;
        if let Ok(content) = std::fs::read_to_string("/proc/net/dev") {
            for line in content.lines().skip(2) {
                if let Some((iface, _)) = line.split_once(':') {
                    let name = iface.trim();
                    if name.is_empty() {
                        continue;
                    }
                    let mut attrs = HashMap::new();
                    attrs.insert("name".to_string(), name.to_string());
                    self.graph
                        .upsert_entity(EntityType::NetworkInterface, name, attrs)?;
                    count += 1;
                }
            }
        }
        Ok(count)
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
            output.push_str(&format!("OK {}: {}\n", name, status));
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_db_path(prefix: &str) -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let dir = PathBuf::from(home).join(".config/vibe_cli/test_dbs");
        let dir = if std::fs::create_dir_all(&dir).is_ok()
            && std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .open(dir.join(".write_test"))
                .is_ok()
        {
            let _ = std::fs::remove_file(dir.join(".write_test"));
            dir
        } else {
            let fallback = PathBuf::from("/tmp/vibe_cli_test_dbs");
            let _ = std::fs::create_dir_all(&fallback);
            let _ = std::fs::remove_file(fallback.join(".write_test"));
            fallback
        };
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        dir.join(format!("{}_{}.db", prefix, nanos))
    }

    #[test]
    fn test_graph_builder_creation() {
        let db_path = test_db_path("graph_builder_create");
        let _ = std::fs::remove_file(&db_path);
        let builder = GraphBuilder::with_path(db_path.clone()).unwrap();
        let (entities, _) = builder.get_stats().unwrap();
        assert_eq!(entities, 0);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn test_discover_os() {
        let db_path = test_db_path("graph_builder_os");
        let _ = std::fs::remove_file(&db_path);
        let builder = GraphBuilder::with_path(db_path.clone()).unwrap();

        builder.discover_os().unwrap();

        let (entities, _) = builder.get_stats().unwrap();
        assert!(entities > 0);

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn test_discover_env_vars() {
        let db_path = test_db_path("graph_builder_env");
        let _ = std::fs::remove_file(&db_path);
        let builder = GraphBuilder::with_path(db_path.clone()).unwrap();

        builder.discover_env_vars().unwrap();

        let (entities, _) = builder.get_stats().unwrap();
        assert!(entities > 0);

        let _ = std::fs::remove_file(&db_path);
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
