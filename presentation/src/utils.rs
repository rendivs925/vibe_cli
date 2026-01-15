use std::collections::HashSet;
use std::hash::{Hash, Hasher, DefaultHasher};

/// Find the project root by looking for common project files
pub fn find_project_root() -> Option<String> {
    let mut current = std::env::current_dir().ok()?;
    loop {
        // Check for various project indicators
        let project_files = [
            "Cargo.toml",       // Rust
            "package.json",     // Node.js
            "requirements.txt", // Python
            "Pipfile",          // Python
            "pyproject.toml",   // Python
            "setup.py",         // Python
            "Makefile",         // C/C++
            "CMakeLists.txt",   // C/C++
            "configure.ac",     // C/C++
            "go.mod",           // Go
            "Gemfile",          // Ruby
            "composer.json",    // PHP
            ".git",             // Git repo as fallback
        ];

        for file in &project_files {
            if current.join(file).exists() {
                return Some(current.display().to_string());
            }
        }

        if !current.pop() {
            break;
        }
    }
    None
}

/// Generate a cache suffix based on the project root
pub fn project_cache_suffix() -> String {
    if let Some(root) = find_project_root() {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        root.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    } else {
        "global".to_string()
    }
}

/// Detect system information
pub fn detect_system_info() -> String {
    let mut info = Vec::new();

    // Detect OS
    if let Ok(os) = std::fs::read_to_string("/etc/os-release") {
        for line in os.lines() {
            if line.starts_with("ID=") {
                info.push(format!(
                    "Distro: {}",
                    line.trim_start_matches("ID=").trim_matches('"')
                ));
            } else if line.starts_with("VERSION_ID=") {
                info.push(format!(
                    "Version: {}",
                    line.trim_start_matches("VERSION_ID=").trim_matches('"')
                ));
            }
        }
    } else if let Ok(os) = std::process::Command::new("uname").arg("-s").output() {
        info.push(format!(
            "OS: {}",
            String::from_utf8_lossy(&os.stdout).trim()
        ));
    }

    // Detect init system
    if std::path::Path::new("/run/systemd/system").exists() {
        info.push("Init system: systemd".to_string());
    } else if std::path::Path::new("/etc/init.d").exists() {
        info.push("Init system: init.d".to_string());
    }

    // Detect package manager
    if std::process::Command::new("which")
        .arg("apt")
        .output()
        .is_ok()
    {
        info.push("Package manager: apt".to_string());
    } else if std::process::Command::new("which")
        .arg("yum")
        .output()
        .is_ok()
    {
        info.push("Package manager: yum".to_string());
    } else if std::process::Command::new("which")
        .arg("dnf")
        .output()
        .is_ok()
    {
        info.push("Package manager: dnf".to_string());
    } else if std::process::Command::new("which")
        .arg("pacman")
        .output()
        .is_ok()
    {
        info.push("Package manager: pacman".to_string());
    }

    // Kernel version
    if let Ok(kernel) = std::process::Command::new("uname").arg("-r").output() {
        info.push(format!(
            "Kernel: {}",
            String::from_utf8_lossy(&kernel.stdout).trim()
        ));
    }

    info.join(", ")
}

/// Remove markdown code fences/backticks and surrounding quotes
pub fn clean_command_output(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.starts_with("```") && trimmed.ends_with("```") {
        let lines: Vec<&str> = trimmed.lines().collect();
        if lines.len() >= 3 && lines.last().unwrap().trim() == "```" {
            return lines[1..lines.len() - 1].join("\n").trim().to_string();
        }
    }
    trimmed
        .trim_matches('`')
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
}

/// Extract last JSON object/array from text
pub fn extract_last_json(raw: &str) -> Option<&str> {
    let trimmed = raw.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}')
        || trimmed.starts_with('[') && trimmed.ends_with(']')
    {
        return Some(trimmed);
    }
    let bytes = trimmed.as_bytes();
    let mut depth = 0;
    let mut start = None;
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'{' || b == b'[' {
            if depth == 0 {
                start = Some(i);
            }
            depth += 1;
        } else if b == b'}' || b == b']' {
            depth -= 1;
            if depth == 0 {
                if let Some(s) = start {
                    return Some(&trimmed[s..=i]);
                }
            }
        }
    }
    None
}

/// Extract JSON array from possibly noisy text
pub fn extract_json_array(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    let mut depth = 0;
    let mut start = None;
    let mut in_string = false;
    let mut escape_next = false;

    for (i, &b) in bytes.iter().enumerate() {
        if escape_next {
            escape_next = false;
            continue;
        }

        match b {
            b'"' => in_string = !in_string,
            b'\\' => {
                if in_string {
                    escape_next = true;
                }
            }
            b'[' => {
                if !in_string && depth == 0 {
                    start = Some(i);
                }
                if !in_string {
                    depth += 1;
                }
            }
            b']' => {
                if !in_string {
                    depth -= 1;
                    if depth == 0 {
                        if let Some(s) = start {
                            return Some(&text[s..=i]);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// Parse agent response into a list of commands
pub fn parse_agent_plan(raw: &str) -> Vec<String> {
    // Try plain parse
    if let Ok(cmds) = serde_json::from_str::<Vec<String>>(raw) {
        return cmds;
    }
    // Clean and try again
    let cleaned = clean_command_output(raw);
    if let Ok(cmds) = serde_json::from_str::<Vec<String>>(&cleaned) {
        return cmds;
    }
    // Try to pull array from noisy text
    if let Some(arr) = extract_json_array(raw) {
        if let Ok(cmds) = serde_json::from_str::<Vec<String>>(arr) {
            return cmds;
        }
    }
    if let Some(json) = extract_last_json(raw) {
        if let Ok(cmds) = serde_json::from_str::<Vec<String>>(json) {
            return cmds;
        }
    }
    // Fallback: split non-empty lines, stripping common list markers and code fences
    raw.lines()
        .map(|l| l.trim())
        .filter(|l| {
            !l.is_empty() && !l.starts_with("```") && !l.ends_with("```") && *l != "[" && *l != "]"
        })
        .map(|l| {
            let mut line = l
                .trim_start_matches(|c| c == '-' || c == '*' || c == '•')
                .trim();
            if let Some(pos) = line.find(|c: char| c == ')' || c == '.' || c == ':') {
                // Only strip early numbering markers
                if pos < 4 {
                    line = line[pos + 1..].trim();
                }
            }
            line.trim_matches(',').trim().trim_matches('"').to_string()
        })
        .filter(|l| !l.is_empty())
        .collect()
}

/// Extract command from AI response
pub fn extract_command_from_response(response: &str) -> String {
    let response = response.trim();
    let cleaned = if response.starts_with("```bash") && response.ends_with("```") {
        let start = response.find('\n').unwrap_or(0) + 1;
        let end = response.len() - 3;
        response[start..end].trim().to_string()
    } else if response.starts_with("```") && response.ends_with("```") {
        let start = response.find('\n').unwrap_or(0) + 1;
        let end = response.len() - 3;
        response[start..end].trim().to_string()
    } else {
        response.to_string()
    };

    // Smart quote handling: only remove surrounding quotes if they wrap the entire command
    // and there are no quotes within the command (indicating they're part of command syntax)
    let trimmed = cleaned.trim_matches('`').trim();
    if trimmed.starts_with('"') && trimmed.ends_with('"') {
        let inner = &trimmed[1..trimmed.len() - 1];
        // If there are no quotes inside, it's safe to remove the outer quotes
        if !inner.contains('"') && !inner.contains('\'') {
            return inner.trim().to_string();
        }
        // Otherwise, keep the quotes as they're part of the command syntax
    } else if trimmed.starts_with('\'') && trimmed.ends_with('\'') {
        let inner = &trimmed[1..trimmed.len() - 1];
        // Same logic for single quotes
        if !inner.contains('\'') && !inner.contains('"') {
            return inner.trim().to_string();
        }
    }

    trimmed.to_string()
}

/// Extract keywords from text for search
pub fn keywords_from_text(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|w| w.len() > 2)
        .map(|w| w.to_lowercase())
        .collect()
}

/// Strip surrounding code fences/backticks to avoid emitting markdown into files
pub fn strip_code_fences(code: &str) -> String {
    let trimmed = code.trim();
    if trimmed.starts_with("```") && trimmed.ends_with("```") {
        let mut lines: Vec<&str> = trimmed.lines().collect();
        if !lines.is_empty()
            && lines
                .first()
                .map(|l| l.trim().starts_with("```"))
                .unwrap_or(false)
        {
            lines.remove(0);
        }
        if !lines.is_empty() && lines.last().map(|l| l.trim() == "```").unwrap_or(false) {
            lines.pop();
        }
        return lines.join("\n").trim().to_string();
    }
    trimmed.trim_matches('`').trim().to_string()
}