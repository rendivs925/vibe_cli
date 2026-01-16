//! Build display and formatting helpers

use colored::Colorize;

/// Create logical chunks from file lines for display
pub fn create_file_chunks(lines: &[&str]) -> Vec<(usize, usize, &'static str)> {
    let total_lines = lines.len();
    let mut chunks = Vec::new();

    if total_lines == 0 {
        return chunks;
    }

    if total_lines <= 20 {
        chunks.push((1, total_lines, "Complete file"));
    } else {
        // Dynamic chunking - divide into 3 equal parts
        let chunk_size = (total_lines as f32 / 3.0).ceil() as usize;

        // Ensure chunks don't overlap and stay within bounds
        let chunk1_end = chunk_size.min(total_lines);
        let chunk2_start = (chunk1_end + 1).min(total_lines);
        let chunk2_end = (chunk_size * 2).min(total_lines);
        let chunk3_start = (chunk2_end + 1).min(total_lines);

        // Determine labels based on file type
        let is_html = lines
            .iter()
            .any(|l| l.contains("<html") || l.contains("DOCTYPE"));

        if chunk3_start <= total_lines {
            // Three chunks
            if is_html {
                chunks = vec![
                    (1, chunk1_end, "HTML skeleton and setup"),
                    (chunk2_start, chunk2_end, "Main content structure"),
                    (chunk3_start, total_lines, "Footer and closing tags"),
                ];
            } else {
                chunks = vec![
                    (1, chunk1_end, "Beginning of file"),
                    (chunk2_start, chunk2_end, "Middle section"),
                    (chunk3_start, total_lines, "End of file"),
                ];
            }
        } else if chunk2_start <= total_lines {
            // Two chunks (file too small for 3)
            chunks = vec![
                (
                    1,
                    chunk1_end,
                    if is_html {
                        "HTML skeleton and setup"
                    } else {
                        "Beginning of file"
                    },
                ),
                (
                    chunk2_start,
                    total_lines,
                    if is_html {
                        "Main content and footer"
                    } else {
                        "Rest of file"
                    },
                ),
            ];
        } else {
            // Single chunk
            chunks.push((1, total_lines, "Complete file"));
        }
    }

    chunks
}

/// Display code with basic syntax highlighting
pub fn display_code_with_syntax(lines: &[&str], start_line: usize, ext: &str) {
    for (i, line) in lines.iter().enumerate() {
        let line_num = start_line + i + 1;
        let line_num_display = format!("{:2}", line_num).bright_black();

        let trimmed = line.trim_start();
        let highlighted = match ext {
            "py" => {
                if trimmed.starts_with('#') {
                    trimmed.bright_black().to_string()
                } else if trimmed.starts_with("def ") || trimmed.starts_with("class ") {
                    trimmed.bright_blue().to_string()
                } else if trimmed.starts_with("import ") || trimmed.starts_with("from ") {
                    trimmed.bright_magenta().to_string()
                } else {
                    line.to_string()
                }
            }
            "js" | "ts" => {
                if trimmed.starts_with("//") {
                    trimmed.bright_black().to_string()
                } else if trimmed.starts_with("function ")
                    || trimmed.starts_with("const ")
                    || trimmed.starts_with("let ")
                    || trimmed.starts_with("class ")
                {
                    trimmed.bright_blue().to_string()
                } else {
                    line.to_string()
                }
            }
            "html" => {
                if line.trim().is_empty() {
                    String::new()
                } else if line.contains("<!DOCTYPE") {
                    line.bright_blue().to_string()
                } else if line.contains("<html")
                    || line.contains("<head")
                    || line.contains("<body")
                    || line.contains("</html>")
                    || line.contains("</head>")
                    || line.contains("</body>")
                {
                    line.bright_blue().to_string()
                } else if line.contains("<div")
                    || line.contains("<main")
                    || line.contains("<h1")
                    || line.contains("<p")
                    || line.contains("<button")
                    || line.contains("<footer")
                    || line.contains("</div>")
                    || line.contains("</main>")
                    || line.contains("</h1>")
                    || line.contains("</p>")
                    || line.contains("</button>")
                    || line.contains("</footer>")
                {
                    line.bright_green().to_string()
                } else if line.contains("class=")
                    || line.contains("href=")
                    || line.contains("src=")
                {
                    line.bright_yellow().to_string()
                } else {
                    line.to_string()
                }
            }
            _ => line.to_string(),
        };

        println!("  {} │ {}", line_num_display, highlighted);
    }
}

/// Strip code fences from code blocks
pub fn strip_code_fences(code: &str) -> String {
    let lines: Vec<&str> = code.lines().collect();
    let mut result = Vec::new();
    let mut in_fence = false;

    for line in lines {
        if line.trim().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if !in_fence || !line.trim().starts_with("```") {
            result.push(line);
        }
    }

    result.join("\n")
}

/// Display chain of thought in tree format
pub fn display_chain_of_thought(reasoning: &str) {
    println!("\nChain of Thought:");

    // Parse reasoning into key points
    let lines: Vec<&str> = reasoning
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(4) // Limit to 4 key points
        .collect();

    for (i, line) in lines.iter().enumerate() {
        let marker = if i == lines.len() - 1 { "└─" } else { "├─" };
        println!("  {} {}", marker, line.trim().bright_white());
    }
}

/// Display progress with status indicator
pub fn display_progress(current: usize, total: usize, description: &str, session: Option<&String>) {
    let status = match current {
        1..=2 => "[Check]",
        3 => "[Arrow]",
        4 => "[Flash]",
        5 => "[Check]",
        _ => "[Circle]",
    };

    let session_prefix = if let Some(s) = session {
        format!("[{}] ", s)
    } else {
        "[main] ".to_string()
    };

    println!(
        "{}{} [{}/{}] {}",
        session_prefix, status, current, total, description
    );
}
