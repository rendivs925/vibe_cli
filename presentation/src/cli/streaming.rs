use anyhow::Context;
use futures_util::StreamExt;
use infrastructure::config::Config;
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use tokio::io::{AsyncBufReadExt, BufReader as AsyncBufReader};
use tokio_util::io::StreamReader;

#[derive(Serialize)]
pub struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [super::Message],
    stream: bool,
}

#[derive(Deserialize, Debug)]
pub struct ChatResponse {
    message: super::Message,
    #[serde(default)]
    done: bool,
}

pub fn normalize_ollama_url(base: &str) -> String {
    let b = base.trim_end_matches('/');
    if b.ends_with("/api/chat") || b.ends_with("/api/generate") {
        b.to_string()
    } else {
        format!("{}/api/chat", b)
    }
}

pub fn clear_last_lines(lines: usize) {
    if lines == 0 {
        return;
    }
    for _ in 0..lines {
        print!("\x1b[1A");
        print!("\x1b[2K");
    }
    io::stdout().flush().ok();
}

pub fn save_cursor() {
    print!("\x1b7");
    io::stdout().flush().ok();
}

pub fn restore_cursor_and_clear_to_end() {
    print!("\x1b8\x1b[J");
    io::stdout().flush().ok();
}

pub async fn stream_assistant_content(
    client: &reqwest::Client,
    config: &Config,
    messages: &[super::Message],
) -> anyhow::Result<(String, bool)> {
    let req = ChatRequest {
        model: &config.ollama_model,
        messages,
        stream: true,
    };

    let url = normalize_ollama_url(&config.ollama_base_url);

    let resp = client
        .post(url)
        .json(&req)
        .send()
        .await
        .context("Failed contacting Ollama")?
        .error_for_status()
        .context("Ollama returned non-2xx status")?;

    let byte_stream = resp
        .bytes_stream()
        .map(|r| r.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)));
    let reader = StreamReader::new(byte_stream);
    let mut lines = AsyncBufReader::new(reader).lines();

    let mut full = String::new();
    let mut printed_anything = false;

    const PREFIXES: [&str; 4] = ["COMMAND:", "Command:", "CMD:", "BATTERY:"];

    let keep_tail = PREFIXES
        .iter()
        .map(|p| p.len())
        .max()
        .unwrap_or(8)
        .saturating_sub(1);

    let mut suppress_print = false;
    let mut buf = String::new();

    let mut print_now = |s: &str| {
        if s.is_empty() {
            return;
        }
        printed_anything = true;
        print!("{s}");
        io::stdout().flush().ok();
    };

    let find_any_prefix = |s: &str| -> Option<usize> {
        PREFIXES
            .iter()
            .filter_map(|p| s.find(p).map(|idx| idx))
            .min()
    };

    while let Some(line) = lines.next_line().await? {
        let line = line.trim();

        if line.is_empty() {
            continue;
        }

        if let Ok(v) = serde_json::from_str::<ChatResponse>(&line) {
            if v.message.role == "assistant" && !v.message.content.is_empty() {
                let chunk = &v.message.content;
                full.push_str(chunk);

                if !suppress_print {
                    buf.push_str(chunk);

                    if let Some(pos) = find_any_prefix(&buf) {
                        let before = &buf[..pos];
                        print_now(before);

                        suppress_print = true;
                        buf.clear();
                    } else if buf.len() > keep_tail {
                        let cut = super::utils::floor_char_boundary(&buf, keep_tail);
                        let to_print = &buf[..cut];
                        print_now(to_print);

                        let tail = buf[cut..].to_string();
                        buf.clear();
                        buf.push_str(&tail);
                    }
                }
            }

            if v.done {
                break;
            }
        }
    }

    if !suppress_print && !buf.is_empty() {
        print_now(&buf);
    }

    Ok((full, printed_anything))
}
