use super::CliHandlers;
use crate::cli::cache::CommandCandidate;
use infrastructure::config::Config;
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn setup_test_domain_home() -> (String, PathBuf) {
    let original = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_home = PathBuf::from(original.clone())
        .join(".config/vibe_cli/test_homes")
        .join(format!("handlers_{}", nanos));
    std::fs::create_dir_all(&temp_home).unwrap();
    std::env::set_var("HOME", &temp_home);

    let domain_dir = temp_home.join(".config/vibe_cli/domains/linux");
    std::fs::create_dir_all(domain_dir.join("entities")).unwrap();
    std::fs::create_dir_all(temp_home.join(".config/vibe_cli/shared_entities")).unwrap();

    let domain_json = r#"{
        "domain": "linux",
        "version": "1.0.0",
        "description": "Test Linux domain",
        "depends_on": [],
        "priority": 10,
        "enabled": true
    }"#;
    std::fs::write(domain_dir.join("domain.json"), domain_json).unwrap();

    let operations_json = r#"[
        {
            "op_id": "list_files",
            "name": "list files",
            "description": "list files",
            "intent": "list files",
            "input_schema": {},
            "generators": [
                {"name": "ls_all", "tool": "ls", "template": "ls -la", "when": []},
                {"name": "ls_one", "tool": "ls", "template": "ls", "when": []}
            ],
            "examples": [{"description": "list files", "inputs": {}}]
        }
    ]"#;
    std::fs::write(domain_dir.join("operations.json"), operations_json).unwrap();
    std::fs::write(domain_dir.join("relationships.json"), "[]").unwrap();
    std::fs::write(domain_dir.join("inference_rules.json"), "[]").unwrap();
    std::fs::write(domain_dir.join("troubleshooting.json"), "[]").unwrap();

    (original, temp_home)
}

#[test]
fn test_document_constrained_filtering() {
    let (original_home, temp_home) = setup_test_domain_home();
    let handlers = CliHandlers::new(Config::load());

    let candidates = vec![
        CommandCandidate::new("ls -la".to_string()),
        CommandCandidate::new("ls".to_string()),
    ];

    let mut allowed = HashSet::new();
    allowed.insert("ls -la".to_string());

    let (valid, _, _, suggestion) =
        handlers.filter_candidates_by_domain("list files", candidates, Some(&allowed));

    assert_eq!(valid.len(), 1);
    assert_eq!(valid[0].command, "ls -la");
    if let Some(suggestion) = suggestion {
        assert!(suggestion.commands.iter().all(|c| c == "ls -la"));
    }

    std::env::set_var("HOME", &original_home);
    let _ = std::fs::remove_dir_all(temp_home);
}
