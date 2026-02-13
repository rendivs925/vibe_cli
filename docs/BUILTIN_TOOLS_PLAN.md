# Built-in Tools for Agentic CLI

## Overview

This plan outlines the implementation of built-in tools for the ReAct agent in Vibe CLI. The tools provide structured operations beyond raw shell commands, following Clean Architecture principles as defined in AGENTS.md.

---

## Architecture Overview (Following Clean Architecture)

```
domain/src/
├── entities/
│   └── react.rs                    # ReactStep, ProposedCommand (EXISTING)
├── services/
│   └── command_extraction.rs       # Command extraction (EXISTING)
├── domain_config/                  # Neurosymbolic (EXISTING)
└── tools/                          # NEW - Clean Architecture
    ├── mod.rs                      # Module exports
    ├── tool_trait.rs              # Tool trait (interface)
    ├── tool_result.rs             # ToolOutput value object
    ├── package_manager.rs         # Package manager enum + detection
    └── service_manager.rs         # Service manager enum + detection

application/src/services/
├── react_agent_service.rs          # ReAct logic (EXISTING)
└── tool_executor.rs               # NEW - Use case for tool execution

infrastructure/src/
└── tools/                         # NEW - Tool implementations
    ├── exploration/
    │   ├── read_tool.rs
    │   ├── grep_tool.rs
    │   ├── fd_tool.rs
    │   └── rag_tool.rs
    ├── editing/
    │   ├── sed_tool.rs
    │   ├── perl_tool.rs
    │   ├── awk_tool.rs
    │   └── apply_patch_tool.rs
    ├── file_ops/
    │   ├── write_tool.rs
    │   ├── remove_tool.rs
    │   └── update_tool.rs
    └── system/
        ├── shell_tool.rs
        ├── pkg_tool.rs            # Uses PackageManager
        └── svc_tool.rs            # Uses ServiceManager

presentation/src/cli/handlers/
└── react.rs                       # Update for tool output (EXISTING)
```

---

## Dependency Flow (Inward Only)

```
shared/          →  domain/           →  application/
   ↓                  ↓                     ↓
primitives     ←  entities          ←  services
                 tools (trait)      ←  tool_executor
                      ↓
                 infrastructure/
                      ↓
                 tool implementations
```

---

## Domain Layer (Pure - No External Dependencies)

### Tool Trait

**File:** `domain/src/tools/tool_trait.rs`

```rust
use crate::tools::tool_result::ToolOutput;

pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn usage(&self) -> &str;
    fn examples(&self) -> Vec<&str>;
    fn requires_confirmation(&self) -> bool;
    fn execute(&self, args: &[str]) -> Result<ToolOutput, ToolError>;
}

#[derive(Debug, Clone)]
pub enum ToolError {
    InvalidArguments(String),
    ExecutionFailed(String),
    NotFound(String),
    PermissionDenied(String),
}
```

### Tool Output

**File:** `domain/src/tools/tool_result.rs`

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub format: OutputFormat,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutputFormat {
    Text,
    Json,
    Table,
    Error,
}
```

### Package Manager Detection

**File:** `domain/src/tools/package_manager.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PackageManager {
    // System managers
    Apt,    // Debian/Ubuntu
    Dnf,    // Fedora/RHEL 8+
    Yum,    // RHEL 7/CentOS
    Pacman, // Arch Linux
    Zypper, // SUSE
    Apk,    // Alpine
    Xbps,   // Void
    Pkg,    // FreeBSD
    Brew,   // macOS/Linux
    // AUR helpers
    Yay,    // Arch
    Paru,   // Arch
    Aurutils,
    Trizen,
    Pikaur,
    Unknown,
}

impl PackageManager {
    pub fn detect() -> Self;
    pub fn install_command(&self, package: &str) -> String;
    pub fn remove_command(&self, package: &str) -> String;
    pub fn search_command(&self, query: &str) -> String;
    pub fn update_command(&self) -> String;
    pub fn upgrade_command(&self) -> String;
}
```

### Service Manager Detection

**File:** `domain/src/tools/service_manager.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ServiceManager {
    Systemd,    // Most Linux distros
    SysVInit,   // Legacy
    OpenRC,     // Alpine, Gentoo
    Runit,      // Void
    Supervisor, // Python-based
    Unknown,
}

impl ServiceManager {
    pub fn detect() -> Self;
    pub fn start_command(&self, service: &str) -> String;
    pub fn stop_command(&self, service: &str) -> String;
    pub fn restart_command(&self, service: &str) -> String;
    pub fn status_command(&self, service: &str) -> String;
    pub fn enable_command(&self, service: &str) -> String;
    pub fn disable_command(&self, service: &str) -> String;
}
```

---

## Application Layer (Depends Only on Domain)

### Tool Executor

**File:** `application/src/services/tool_executor.rs`

```rust
use domain::tools::{Tool, ToolOutput, ToolError};
use std::sync::Arc;

pub struct ToolExecutor {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolExecutor {
    pub fn new() -> Self;
    pub fn register(&mut self, tool: Arc<dyn Tool>);
    pub fn execute(&self, tool_name: &str, args: &[str]) -> Result<ToolOutput, ToolError>;
    pub fn list_tools(&self) -> Vec<ToolInfo>;
    pub fn get_tool_help(&self, tool_name: &str) -> Option<String>;
}
```

---

## Infrastructure Layer (Implements Domain Interfaces)

### Tool Implementation Pattern

```rust
// Example: infrastructure/src/tools/exploration/read_tool.rs
use domain::tools::{Tool, ToolError, ToolOutput, OutputFormat};
use std::fs;
use std::path::Path;

pub struct ReadTool;

impl Tool for ReadTool {
    fn name(&self) -> &str { "read" }
    fn description(&self) -> &str { "Read file contents with smart context" }
    fn usage(&self) -> &str { "read <path> [lines] [offset]" }
    fn examples(&self) -> Vec<&str> { 
        vec!["read src/main.rs", "read src/main.rs 50", "read src/main.rs 20 100"] 
    }
    fn requires_confirmation(&self) -> bool { false }
    
    fn execute(&self, args: &[str]) -> Result<ToolOutput, ToolError> {
        // Implementation
    }
}
```

---

## Tool Categories

### Exploration Tools

| Tool | Description | Input |
|------|-------------|-------|
| `read` | Read file with smart context | `read <path> [lines] [offset]` |
| `grep` | Search patterns in files | `grep <pattern> [path]` |
| `fd` | Find files by name/extension | `fd <pattern> [directory]` |
| `rag` | Semantic search with RAG | `rag <query> [num_results]` |

### Editing Tools

| Tool | Description | Input |
|------|-------------|-------|
| `sed` | Stream editor | `sed <pattern> <replacement> <path>` |
| `perl` | Perl regex operations | `perl <regex> <replacement> <path>` |
| `awk` | Text processing | `awk <script> <path>` |
| `apply_patch` | Apply code changes | `apply_patch <diff>` |

### File Operations

| Tool | Description | Input |
|------|-------------|-------|
| `write` | Create/write file | `write <path> <content>` |
| `remove` | Delete files | `remove <path>` |
| `update` | Smart file update | `update <path> <old> <new>` |

### System Tools (Dynamic)

| Tool | Description | Input |
|------|-------------|-------|
| `shell` | Execute shell command | `shell <command>` |
| `pkg` | Dynamic package manager | `pkg <install\|remove\|search\|update\|upgrade> <package>` |
| `svc` | Dynamic service manager | `svc <start\|stop\|restart\|status\|enable\|disable> <service>` |

---

## Package Manager Detection

### Full Detection Logic

```rust
pub fn detect_package_manager() -> PackageManager {
    // System package managers (in order of priority)
    if command_exists("apt") { return PackageManager::Apt; }
    if command_exists("dnf") { return PackageManager::Dnf; }
    if command_exists("yum") { return PackageManager::Yum; }
    if command_exists("pacman") { return PackageManager::Pacman; }
    if command_exists("zypper") { return PackageManager::Zypper; }
    if command_exists("apk") { return PackageManager::Apk; }
    if command_exists("xbps-query") { return PackageManager::Xbps; }
    if command_exists("pkg") { return PackageManager::Pkg; }
    if command_exists("brew") { return PackageManager::Brew; }
    
    // AUR helpers (if no system package manager found)
    if command_exists("yay") { return PackageManager::Yay; }
    if command_exists("paru") { return PackageManager::Paru; }
    if command_exists("aurutils") { return PackageManager::Aurutils; }
    if command_exists("trizen") { return PackageManager::Trizen; }
    if command_exists("pikaur") { return PackageManager::Pikaur; }
    
    PackageManager::Unknown
}
```

### Command Mapping

| User Input | Detected PM | Commands Generated |
|------------|-------------|-------------------|
| `pkg install nginx` | Ubuntu → Apt | `sudo apt install nginx -y` |
| `pkg install nginx` | Fedora → Dnf | `sudo dnf install nginx -y` |
| `pkg install nginx` | Arch → Pacman | `sudo pacman -S --noconfirm nginx` |
| `pkg install nginx` | Arch + Yay | `yay -S --noconfirm nginx` |
| `pkg install nginx` | Arch + Paru | `paru -S --noconfirm nginx` |
| `pkg install nginx` | Alpine → Apk | `apk add nginx` |
| `pkg install nginx` | openSUSE → Zypper | `sudo zypper install -y nginx` |

---

## Service Manager Detection

```rust
pub fn detect_service_manager() -> ServiceManager {
    if Path::exists("/run/systemd/system") { ServiceManager::Systemd }
    else if Path::exists("/etc/init.d") { ServiceManager::SysVInit }
    else if Path::exists("/etc/init.d/openrc") { ServiceManager::OpenRC }
    else if Path::exists("/etc/runit") { ServiceManager::Runit }
    else if command_exists("supervisord") { ServiceManager::Supervisor }
    else { ServiceManager::Unknown }
}
```

---

## Integration with ReAct

### Updated ReAct Flow

```
User Input → AI Reasoning → Tool/Suggestion → Allow? y/n>
                                              ├─ y → Execute → Output → AI Reasoning → repeat
                                              └─ n → User Input → AI Reasoning → repeat
```

### Tool Execution

1. **Update `ReactContext`** - Add available tools to session context
2. **Update `ReactAgentService`** - Route to `ToolExecutor` when tool detected
3. **Update handler** - Display tool output with proper formatting

### LLM Prompt Enhancement

```
Available tools:
- read <path> [lines] - Read file contents
- grep <pattern> [path] - Search in files
- fd <pattern> [dir] - Find files
- rag <query> - Semantic search
- write <path> <content> - Write file
- sed <pattern> <replacement> <path> - Stream editor
- perl <regex> <replacement> <path> - Perl regex
- awk <script> <path> - Text processing
- shell <command> - Execute shell
- pkg <action> <package> - Package manager (auto-detects distro)
- svc <action> <service> - Service manager (auto-detects init system)
```

---

## Example Usage

```
--- REASONING ---
I need to understand the codebase first.

--- TOOL: fd ---
fd "*.rs" ./src

--- OUTPUT ---
src/main.rs
src/lib.rs
src/handlers/mod.rs
...

--- REASONING ---
Found Rust files. Let me read the main entry point.

--- TOOL: read ---
read src/main.rs 50

--- OUTPUT ---
1: use clap::Parser;
2: 
3: #[derive(Parser)]
4: pub struct Cli {
5:     ...
6: }

--- TOOL: pkg ---
pkg install nginx

[Detected: Arch Linux]
[Using: yay]
yay -S --noconfirm nginx

--- OUTPUT ---
Installing nginx...

--- TOOL: svc ---
svc start nginx

[Detected: systemd]
sudo systemctl start nginx

--- OUTPUT ---
Service started
```

---

## Files to Create/Modify

| File | Layer | Action |
|------|-------|--------|
| `domain/src/tools/mod.rs` | Domain | Create |
| `domain/src/tools/tool_trait.rs` | Domain | Create |
| `domain/src/tools/tool_result.rs` | Domain | Create |
| `domain/src/tools/package_manager.rs` | Domain | Create |
| `domain/src/tools/service_manager.rs` | Domain | Create |
| `application/src/services/tool_executor.rs` | Application | Create |
| `infrastructure/src/tools/mod.rs` | Infrastructure | Create |
| `infrastructure/src/tools/exploration/read_tool.rs` | Infrastructure | Create |
| `infrastructure/src/tools/exploration/grep_tool.rs` | Infrastructure | Create |
| `infrastructure/src/tools/exploration/fd_tool.rs` | Infrastructure | Create |
| `infrastructure/src/tools/exploration/rag_tool.rs` | Infrastructure | Create |
| `infrastructure/src/tools/editing/sed_tool.rs` | Infrastructure | Create |
| `infrastructure/src/tools/editing/perl_tool.rs` | Infrastructure | Create |
| `infrastructure/src/tools/editing/awk_tool.rs` | Infrastructure | Create |
| `infrastructure/src/tools/editing/apply_patch_tool.rs` | Infrastructure | Create |
| `infrastructure/src/tools/file_ops/write_tool.rs` | Infrastructure | Create |
| `infrastructure/src/tools/file_ops/remove_tool.rs` | Infrastructure | Create |
| `infrastructure/src/tools/file_ops/update_tool.rs` | Infrastructure | Create |
| `infrastructure/src/tools/system/shell_tool.rs` | Infrastructure | Create |
| `infrastructure/src/tools/system/pkg_tool.rs` | Infrastructure | Create |
| `infrastructure/src/tools/system/svc_tool.rs` | Infrastructure | Create |
| `domain/src/entities/react.rs` | Domain | Modify |
| `application/src/services/react_agent_service.rs` | Application | Modify |
| `presentation/src/cli/handlers/react.rs` | Presentation | Modify |

---

## Code Style (per AGENTS.md)

- Modules: 200-300 lines max
- No comments unless requested
- Idiomatic Rust
- Follow existing patterns in codebase
- Dependency direction: inward only
