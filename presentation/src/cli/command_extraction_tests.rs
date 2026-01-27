#[cfg(test)]
mod tests {
    use crate::cli::command_extraction::*;

    #[test]
    fn test_extract_command_simple() {
        let input = "lspci | grep -i nvidia";
        let result = extract_command(input, "");
        assert_eq!(result, Some("lspci | grep -i nvidia".to_string()));
    }

    #[test]
    fn test_extract_command_with_backticks() {
        let input = "Run `lspci | grep -i nvidia` to check GPU";
        let result = extract_command(input, "");
        assert_eq!(result, Some("lspci | grep -i nvidia".to_string()));
    }

    #[test]
    fn test_extract_command_from_code_fence() {
        let input = r#"```bash
lspci | grep -i nvidia
```"#;
        let result = extract_command(input, "");
        assert_eq!(result, Some("lspci | grep -i nvidia".to_string()));
    }

    #[test]
    fn test_extract_command_with_prefix() {
        let input = "COMMAND: lspci | grep -i nvidia";
        let result = extract_command(input, "");
        assert_eq!(result, Some("lspci | grep -i nvidia".to_string()));
    }

    #[test]
    fn test_forbidden_commands() {
        let forbidden_commands = [
            "sudo rm -rf /",
            "dd if=/dev/zero of=/dev/sda",
            "mkfs.ext4 /dev/sda1",
            ":(){ :|:& };:",
            "shutdown -h now",
        ];

        for cmd in forbidden_commands {
            let result = extract_command(cmd, "");
            assert_eq!(result, None, "Should block forbidden command: {}", cmd);
        }
    }

    #[test]
    fn test_non_command_rejection() {
        let non_commands = [
            "To check your GPU, run the following:",
            "1. Open terminal",
            "* This is a bullet point",
            "This is just text",
            "```bash\n```",
        ];

        for text in non_commands {
            let result = extract_command(text, "");
            assert_eq!(result, None, "Should reject non-command: {}", text);
        }
    }

    #[test]
    fn test_multi_line_command_extraction() {
        let input = r#"Run the following commands:
```bash
lspci | grep -i nvidia
nvidia-smi
```"#;
        let results = extract_commands(input, "");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].command, "nvidia-smi");
        assert_eq!(results[1].command, "lspci | grep -i nvidia");
    }

    #[test]
    fn test_clean_command_output() {
        let input = r#"```bash
lspci | grep -i nvidia
```"#;
        let result = clean_command_output(input);
        assert_eq!(result, "lspci | grep -i nvidia");
    }

    #[test]
    fn test_clean_command_output_with_quotes() {
        let input = "`lspci | grep -i nvidia`";
        let result = clean_command_output(input);
        assert_eq!(result, "lspci | grep -i nvidia");
    }

    #[test]
    fn test_extract_json_valid() {
        let input = r#"{"command": "lspci | grep -i nvidia"}"#;
        let result = extract_last_json(input);
        assert_eq!(result, Some(r#"{"command": "lspci | grep -i nvidia"}"#));
    }

    #[test]
    fn test_extract_json_from_text() {
        let input = r#"Some text then {"command": "lspci"} more text"#;
        let result = extract_last_json(input);
        assert_eq!(result, Some(r#"{"command": "lspci"}"#));
    }

    #[test]
    fn test_extract_json_array() {
        let input = r#"["lspci", "nvidia-smi"]"#;
        let result = extract_json_array(input);
        assert_eq!(result, Some(r#"["lspci", "nvidia-smi"]"#));
    }

    #[test]
    fn test_parse_agent_plan_json() {
        let input = r#"["lspci | grep -i nvidia", "nvidia-smi"]"#;
        let result = parse_agent_plan(input);
        assert_eq!(result, vec!["lspci | grep -i nvidia", "nvidia-smi"]);
    }

    #[test]
    fn test_parse_agent_plan_markdown() {
        let input = r#"1. lspci | grep -i nvidia
2. nvidia-smi
3. Check driver version"#;
        let result = parse_agent_plan(input);
        assert_eq!(result, vec!["lspci | grep -i nvidia", "nvidia-smi"]);
    }

    #[test]
    fn test_parse_agent_plan_bullets() {
        let input = r#"- lspci | grep -i nvidia
- nvidia-smi
- Check version"#;
        let result = parse_agent_plan(input);
        assert_eq!(result, vec!["lspci | grep -i nvidia", "nvidia-smi"]);
    }

    #[test]
    fn test_command_normalization() {
        let input = "  `lspci  | grep  -i  nvidia`  ";
        let result = extract_command(input, "");
        assert_eq!(result, Some("lspci | grep -i nvidia".to_string()));
    }

    #[test]
    fn test_complex_command_chain() {
        let input = "lspci | grep -i nvidia && nvidia-smi";
        let result = extract_command(input, "");
        assert_eq!(
            result,
            Some("lspci | grep -i nvidia && nvidia-smi".to_string())
        );
    }

    #[test]
    fn test_malformed_json_recovery() {
        let input = r#"Text then {"invalid": json,} more text ["valid", "json"]"#;
        let result = parse_agent_plan(input);
        assert_eq!(result, vec!["valid", "json"]);
    }

    #[test]
    fn test_nested_string_handling() {
        let input = r#"{"cmd": "echo \"Hello, World!\""}"#;
        let result = extract_last_json(input);
        assert_eq!(result, Some(r#"{"cmd": "echo \"Hello, World!\""}"#));
    }

    #[test]
    fn test_duplicate_command_removal() {
        let input = r#"```bash
lspci | grep -i nvidia
lspci | grep -i nvidia
nvidia-smi
```"#;
        let results = extract_commands(input, "");
        assert_eq!(results.len(), 2); // Should deduplicate
    }

    #[test]
    fn test_edge_case_inputs() {
        let edge_cases = [
            "",
            "   ",
            "```",
            "```\n```",
            "[]",
            "{}",
            "null",
            "Just some text without commands",
        ];

        for case in edge_cases {
            let result = extract_command(case, "");
            assert_eq!(result, None, "Should handle edge case: {:?}", case);
        }
    }

    #[test]
    fn test_unicode_handling() {
        let input = "lspci | grep -i nvidia # Checking 🎮 GPU";
        let result = extract_command(input, "");
        assert_eq!(result, Some("lspci | grep -i nvidia".to_string()));
    }
}
