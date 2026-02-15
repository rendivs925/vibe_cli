use domain::services::command_extraction::extract_candidate_commands;

pub fn parse_command_list(response: &str) -> Vec<String> {
    if let Ok(list) = serde_json::from_str::<Vec<String>>(response.trim()) {
        return list;
    }

    if let Some(json) = extract_json_array(response) {
        if let Ok(list) = serde_json::from_str::<Vec<String>>(json) {
            return list;
        }
    }

    extract_candidate_commands(response, "")
}

fn extract_json_array(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    let mut depth = 0_i32;
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
