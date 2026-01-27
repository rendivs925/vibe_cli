use crate::config::Config;
use crate::session::Message;
use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [Message],
    stream: bool,
}

#[derive(Deserialize, Debug)]
struct ChatResponse {
    message: Message,
}

/// ---------- Prompt building (centralized) ----------

fn env_snapshot() -> (String, String, &'static str) {
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "/home/user".to_string());

    let project_root = find_project_root().unwrap_or_else(|| cwd.clone());

    let platform = if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "unknown"
    };

    (cwd, project_root, platform)
}

fn command_instruction(cwd: &str, project_root: &str, platform: &str) -> String {
    // “No guessing” + “discovery fallback” reduces hallucinations and keeps commands runnable.
    format!(
        r#"You are a command generator. Convert the user's LAST request into EXACTLY ONE POSIX shell command.

Environment:
- platform: {platform}
- cwd: {cwd}
- project_root: {project_root}

Hard constraints:
1) Output ONLY the command text. No markdown, no explanation, no surrounding quotes.
2) Exactly one command line. (Pipes/&& allowed only if truly required.)
3) Non-destructive by default. Never delete/overwrite unless explicitly asked.
4) Do NOT assume files/tools/flags exist. If unsure, output a minimal discovery command instead of guessing.
5) Avoid placeholders like /path/to. Use real paths (absolute or relative to project_root).
6) No network access (curl/wget/git clone/package install) unless explicitly asked.
7) Disk questions: df for filesystem usage, du for directory sizes.

Heuristics:
- To understand a codebase, prefer reading README/Cargo.toml/package.json over ls.
- Prefer rg when available (fallback grep -R).
- Prefer project tools only if their manifest exists (Cargo.toml/package.json/etc).

Now output the single best command:"#,
    )
}

fn agent_plan_system_prompt() -> &'static str {
    r#"You are a shell-plan generator. Turn a user's goal into an ordered list of POSIX shell commands to run one-by-one with confirmation between steps.

Rules:
- Respond with ONLY a JSON array of strings. Each string is ONE command ready to run.
- No markdown, no prose, no comments, no extra keys.
- If you cannot produce a valid JSON array, respond with [].
- Prefer non-destructive, idempotent steps: check state before changing it.
- Never assume paths/tools exist. Add discovery steps instead of guessing.
- Avoid placeholders like /path/to; use cwd/project_root-relative paths when implied.
- Avoid unbounded streaming commands (tail -f) unless bounded (timeout ...).

Example:
["pwd", "ls -la", "rg -n 'fn main' src"]

Output ONLY the JSON array."#
}

fn script_system_prompt() -> &'static str {
    r#"Generate a POSIX-compatible shell script.
Return ONLY the script text. No markdown, no explanation."#
}

fn env_context_message(cwd: &str, project_root: &str, platform: &str) -> Message {
    Message {
        role: "user".into(),
        content: format!(
            "Environment context: platform='{platform}', cwd='{cwd}', project_root='{project_root}'. Use paths that work here and avoid placeholders."
        ),
    }
}

/// ---------- HTTP + parsing (centralized) ----------

fn http_client() -> reqwest::Client {
    reqwest::Client::new()
}

async fn post_chat_raw(
    client: &reqwest::Client,
    config: &Config,
    messages: &[Message],
    stream: bool,
) -> Result<String> {
    let req = ChatRequest {
        model: &config.model,
        messages,
        stream,
    };

    client
        .post(&config.endpoint)
        .json(&req)
        .send()
        .await
        .context("Failed contacting Ollama")?
        .text()
        .await
        .context("Failed reading Ollama response body")
}

/// Try to extract an assistant "content" string from:
/// 1) NDJSON lines
/// 2) Full JSON ChatResponse
/// 3) Noisy text containing JSON object
/// 4) Otherwise raw text
fn extract_assistant_content(raw: &str) -> String {
    // NDJSON: take last assistant message found from bottom.
    for line in raw.lines().rev() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<ChatResponse>(line) {
            if v.message.role == "assistant" {
                return v.message.content;
            }
        }
    }

    // Full JSON
    if let Ok(v) = serde_json::from_str::<ChatResponse>(raw) {
        return v.message.content;
    }

    // Noisy JSON object
    if let Some(json) = extract_last_json(raw) {
        if let Ok(v) = serde_json::from_str::<ChatResponse>(json) {
            return v.message.content;
        }
    }

    raw.to_string()
}

/// Parse a JSON array (Vec<String>) from model output that might contain noise/markdown.
/// Strict: returns Ok(vec) only if valid JSON array is recovered; otherwise Ok([]).
fn parse_json_array_loose(raw: &str) -> Vec<String> {
    // Fast path: raw is already an array
    if let Ok(v) = serde_json::from_str::<Vec<String>>(raw.trim()) {
        return v;
    }

    let content = clean_command_output(raw);

    if let Ok(v) = serde_json::from_str::<Vec<String>>(content.trim()) {
        return v;
    }

    // If content has comments or extra junk, try to clean it.
    let cleaned = clean_json_content(&content);
    if let Ok(v) = serde_json::from_str::<Vec<String>>(cleaned.trim()) {
        return v;
    }

    // Try extracting first JSON array substring
    if let Some(arr) = extract_json_array(&content) {
        if let Ok(v) = serde_json::from_str::<Vec<String>>(arr) {
            return v;
        }
    }
    if let Some(arr) = extract_json_array(raw) {
        if let Ok(v) = serde_json::from_str::<Vec<String>>(arr) {
            return v;
        }
    }

    // Try extracting JSON object and then parse as array if it happens to be array
    if let Some(obj) = extract_last_json(&content) {
        if let Ok(v) = serde_json::from_str::<Vec<String>>(obj) {
            return v;
        }
    }
    if let Some(obj) = extract_last_json(raw) {
        if let Ok(v) = serde_json::from_str::<Vec<String>>(obj) {
            return v;
        }
    }

    Vec::new()
}

/// Generic helper if you later want typed parsing from assistant content.
fn parse_from_assistant<T: DeserializeOwned>(raw: &str) -> Option<T> {
    let content = extract_assistant_content(raw);
    serde_json::from_str::<T>(&content).ok()
}

/// ---------- Public API ----------

/// Request a SINGLE command from Ollama.
pub async fn request_command(config: &Config, messages: &[Message]) -> Result<String> {
    let client = http_client();
    let (cwd, project_root, platform) = env_snapshot();

    // Keep context; add a strict instruction as the last user message.
    let mut msgs = messages.to_vec();
    msgs.push(Message {
        role: "user".into(),
        content: command_instruction(&cwd, &project_root, platform),
    });

    let raw = post_chat_raw(&client, config, &msgs, false).await?;
    let content = extract_assistant_content(&raw);
    Ok(clean_command_output(&content))
}

/// Request multi-step agent plan: returns Vec<String>.
pub async fn request_agent_plan(config: &Config, user_prompt: &str) -> Result<Vec<String>> {
    let client = http_client();
    let (cwd, project_root, platform) = env_snapshot();

    let msgs = vec![
        Message {
            role: "system".into(),
            content: agent_plan_system_prompt().into(),
        },
        env_context_message(&cwd, &project_root, platform),
        Message {
            role: "user".into(),
            content: user_prompt.into(),
        },
    ];

    let raw = post_chat_raw(&client, config, &msgs, false).await?;
    let assistant = extract_assistant_content(&raw);

    // We only accept a JSON array; otherwise return [].
    Ok(parse_json_array_loose(&assistant))
}

/// Request a POSIX shell script (one string output).
pub async fn request_script(config: &Config, user_prompt: &str) -> Result<String> {
    let client = http_client();

    let msgs = vec![
        Message {
            role: "system".into(),
            content: script_system_prompt().into(),
        },
        Message {
            role: "user".into(),
            content: user_prompt.into(),
        },
    ];

    let raw = post_chat_raw(&client, config, &msgs, false).await?;
    let assistant = extract_assistant_content(&raw);
    Ok(assistant.trim().to_string())
}

/// ---------- Utilities (kept, but made tighter) ----------

/// Extract clean JSON object from noisy model output.
/// Note: this extracts the *last* balanced {...} block.
fn extract_last_json(raw: &str) -> Option<&str> {
    let trimmed = raw.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return Some(trimmed);
    }
    let bytes = trimmed.as_bytes();
    let mut depth: i32 = 0;
    let mut start: Option<usize> = None;

    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'{' => {
                if depth == 0 {
                    start = Some(i);
                }
                depth += 1;
            }
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    if let Some(s) = start {
                        return Some(&trimmed[s..=i]);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// Clean model output by removing markdown code fences.
fn clean_command_output(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.starts_with("```") && trimmed.ends_with("```") {
        let lines: Vec<&str> = trimmed.lines().collect();
        if lines.len() >= 3
            && lines[0].trim().starts_with("```")
            && lines.last().unwrap().trim() == "```"
        {
            return lines[1..lines.len() - 1].join("\n").trim().to_string();
        }
    }
    trimmed.to_string()
}

/// Extract a JSON array substring from noisy text.
fn extract_json_array(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    let mut depth: i32 = 0;
    let mut start: Option<usize> = None;
    let mut in_string = false;
    let mut escape_next = false;

    for (i, &b) in bytes.iter().enumerate() {
        if escape_next {
            escape_next = false;
            continue;
        }
        match b {
            b'"' => in_string = !in_string,
            b'\\' if in_string => escape_next = true,
            b'[' if !in_string => {
                if depth == 0 {
                    start = Some(i);
                }
                depth += 1;
            }
            b']' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    if let Some(s) = start {
                        return Some(&text[s..=i]);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// Remove `// ...` comments from a JSON-ish payload (best-effort).
fn clean_json_content(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    let mut in_string = false;
    let mut escape_next = false;
    let mut in_line_comment = false;

    let mut chars = content.chars().peekable();
    while let Some(ch) = chars.next() {
        if escape_next {
            out.push(ch);
            escape_next = false;
            continue;
        }

        match ch {
            '"' if !in_line_comment => {
                in_string = !in_string;
                out.push(ch);
            }
            '\\' if in_string && !in_line_comment => {
                escape_next = true;
                out.push(ch);
            }
            '/' if !in_string && !in_line_comment => {
                if chars.peek() == Some(&'/') {
                    // consume second slash
                    let _ = chars.next();
                    in_line_comment = true;
                } else {
                    out.push(ch);
                }
            }
            '\n' | '\r' => {
                in_line_comment = false;
                out.push(ch);
            }
            _ => {
                if !in_line_comment {
                    out.push(ch);
                }
            }
        }
    }

    out.trim().to_string()
}

fn find_project_root() -> Option<String> {
    let mut current = std::env::current_dir().ok()?;
    let project_markers = [
        "Cargo.toml",
        "package.json",
        "requirements.txt",
        "Pipfile",
        "pyproject.toml",
        "setup.py",
        "Makefile",
        "CMakeLists.txt",
        "configure.ac",
        "go.mod",
        "Gemfile",
        "composer.json",
        ".git",
    ];

    loop {
        if project_markers.iter().any(|m| current.join(m).exists()) {
            return Some(current.display().to_string());
        }
        if !current.pop() {
            break;
        }
    }
    None
}
