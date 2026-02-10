#[cfg(test)]
mod domain_config_tests {
    use crate::domain_config::types::{
        Generator, OutputItem, OutputProperty, OutputSchema, RequiredInput,
    };
    use crate::{CommandGenerator, DomainRegistry, OutputParser};
    use std::collections::HashMap;
    use std::env;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_home() -> PathBuf {
        PathBuf::from(env::var("HOME").unwrap_or_else(|_| "/tmp".to_string()))
    }

    fn test_root_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        test_home()
            .join(".config/vibe_cli/test_domains")
            .join(format!("run_{}", nanos))
    }

    fn test_user_dir(base: &PathBuf, subpath: &str) -> PathBuf {
        base.join(subpath)
    }

    fn setup_test_domain(user: &PathBuf, name: &str) {
        let domain_dir = user.join(name);
        std::fs::create_dir_all(&domain_dir).unwrap();
        std::fs::create_dir_all(&domain_dir.join("entities")).unwrap();

        let domain_json = format!(
            r#"{{"domain": "{}", "version": "1.0.0", "description": "Test domain", "depends_on": [], "priority": 10, "enabled": true}}"#,
            name
        );
        std::fs::write(domain_dir.join("domain.json"), domain_json).unwrap();

        let ops_json = r#"[
            {
                "op_id": "list_processes",
                "name": "List processes",
                "description": "Get list",
                "input_schema": {},
                "generators": [
                    {
                        "name": "ps_standard",
                        "tool": "ps",
                        "template": "ps aux",
                        "when": []
                    }
                ],
                "examples": []
            }
        ]"#;
        std::fs::write(domain_dir.join("operations.json"), ops_json).unwrap();

        std::fs::write(domain_dir.join("relationships.json"), "[]").unwrap();
        std::fs::write(domain_dir.join("inference_rules.json"), "[]").unwrap();
        std::fs::write(domain_dir.join("troubleshooting.json"), "[]").unwrap();
    }

    #[test]
    fn test_domain_loading() {
        let base = test_root_dir();
        let user = test_user_dir(&base, "domains");
        let shared = test_user_dir(&base, "shared_entities");
        let prebuilt = test_user_dir(&base, "domains");

        std::fs::create_dir_all(&user).unwrap();
        std::fs::create_dir_all(&shared).unwrap();
        std::fs::create_dir_all(&user.join("linux/entities")).unwrap();

        let domain_json = r#"{
            "domain": "linux",
            "version": "1.0.0",
            "description": "Test Linux domain",
            "depends_on": [],
            "priority": 10,
            "enabled": true
        }"#;
        std::fs::write(user.join("linux/domain.json"), domain_json).unwrap();

        let ops_json = r#"[{
            "op_id": "list_processes",
            "name": "List processes",
            "description": "Get list of running processes",
            "input_schema": {
                "filter": { "type": "string", "optional": true }
            },
            "generators": [{
                "name": "ps_standard",
                "tool": "ps",
                "template": "ps -eo pid,ppid,cmd",
                "when": []
            }],
            "examples": [{"description": "List all", "inputs": {}}]
        }]"#;
        std::fs::write(user.join("linux/operations.json"), ops_json).unwrap();

        let entity_json = r#"{
            "name": "Process",
            "description": "A running process",
            "core_properties": [
                {"name": "pid", "type": "integer", "meaning": "Process ID"},
                {"name": "cmdline", "type": "string", "meaning": "Command line"}
            ]
        }"#;
        std::fs::write(user.join("linux/entities/process.json"), entity_json).unwrap();

        let rel_json = r#"[
            {"name": "fork_creates_child", "type": "causal", "from": "Process", "to": "Process", "meaning": "Parent creates child"}
        ]"#;
        std::fs::write(user.join("linux/relationships.json"), rel_json).unwrap();

        let rules_json = r#"[
            {"rule_id": "zombie_detect", "if": [{"entity": "Process", "prop": "state", "equals": "Z"}], "then": [{"conclude": "zombie", "confidence": 0.99}]}
        ]"#;
        std::fs::write(user.join("linux/inference_rules.json"), rules_json).unwrap();

        let trouble_json = r#"[
            {"pattern_id": "high_cpu", "symptoms": [{"metric": "cpu", "observation": "high cpu"}], "likely_causes": [{"cause": "loop"}], "checks": [{"step": "check top", "command": "top"}], "actions": [{"action": "fix", "methods": ["kill"]}]}
        ]"#;
        std::fs::write(user.join("linux/troubleshooting.json"), trouble_json).unwrap();

        let registry = DomainRegistry::new(prebuilt, user.clone(), shared.clone());
        assert!(
            registry.is_ok(),
            "DomainRegistry should load successfully, error: {:?}",
            registry
        );

        let registry = registry.unwrap();
        let domains = registry.list_domains();
        assert!(
            domains.contains(&"linux".to_string()),
            "Should load linux domain: {:?}",
            domains
        );
    }

    #[test]
    fn test_command_generation() {
        let generator = CommandGenerator::new();

        assert!(generator.is_tool_available("ps"), "ps should be available");
        assert!(generator.is_tool_available("ls"), "ls should be available");
        assert!(
            generator.is_tool_available("cat"),
            "cat should be available"
        );
        assert!(
            generator.is_tool_available("grep"),
            "grep should be available"
        );
    }

    #[test]
    fn test_output_parsing() {
        let parser = OutputParser;

        let output = "1234 nginx 5.0\n5678 python 2.5";
        let schema = OutputSchema {
            type_: "array".to_string(),
            items: Some(OutputItem {
                type_: "object".to_string(),
                properties: vec![
                    (
                        "pid".to_string(),
                        OutputProperty {
                            type_: "integer".to_string(),
                            column: Some(0),
                            key: None,
                        },
                    ),
                    (
                        "cmdline".to_string(),
                        OutputProperty {
                            type_: "string".to_string(),
                            column: Some(1),
                            key: None,
                        },
                    ),
                    (
                        "cpu".to_string(),
                        OutputProperty {
                            type_: "number".to_string(),
                            column: Some(2),
                            key: None,
                        },
                    ),
                ]
                .into_iter()
                .collect(),
            }),
            properties: HashMap::new(),
            format: None,
            delimiter: Some(" ".to_string()),
        };

        let results = parser.parse(output, &schema);
        assert_eq!(results.len(), 2, "Should parse 2 lines, got: {:?}", results);

        if let Some(first) = results.get(0) {
            assert_eq!(
                first.get("pid").unwrap(),
                &serde_json::Value::Number(1234.into())
            );
            assert_eq!(
                first.get("cmdline").unwrap(),
                &serde_json::Value::String("nginx".to_string())
            );
        }
    }

    #[test]
    fn test_query_intent() {
        let base = test_root_dir();
        let user = test_user_dir(&base, "domains");
        let shared = test_user_dir(&base, "shared_entities");
        let prebuilt = test_user_dir(&base, "domains");

        std::fs::create_dir_all(&user).unwrap();
        std::fs::create_dir_all(&shared).unwrap();
        setup_test_domain(&user, "linux");

        let registry = DomainRegistry::new(prebuilt, user.clone(), shared.clone()).unwrap();

        let domains = registry.query_intent("list processes");
        assert!(
            !domains.is_empty(),
            "Should find domain for 'list processes'"
        );
    }

    #[test]
    fn test_generator_scoring() {
        let generator = CommandGenerator::new();

        let gen = Generator {
            name: "test".to_string(),
            tool: "ps".to_string(),
            template: "ps -eo pid,cmd".to_string(),
            when: vec![RequiredInput {
                name: "filter".to_string(),
                equals: None,
            }],
            optional: vec![],
            timeout_seconds: None,
            preference_score: 0.0,
        };

        let mut inputs: HashMap<String, serde_json::Value> = HashMap::new();
        inputs.insert(
            "filter".to_string(),
            serde_json::Value::String("nginx".to_string()),
        );
        let score = generator.score_generator(&gen, &inputs);
        assert!(score > 0.0, "Score should be > 0 with required input");

        let empty_inputs: HashMap<String, serde_json::Value> = HashMap::new();
        let score = generator.score_generator(&gen, &empty_inputs);
        assert_eq!(score, 0.0, "Score should be 0 without required input");
    }

    #[test]
    fn test_operation_lookup() {
        let base = test_root_dir();
        let user = test_user_dir(&base, "domains");
        let shared = test_user_dir(&base, "shared_entities");
        let prebuilt = test_user_dir(&base, "domains");

        std::fs::create_dir_all(&user).unwrap();
        std::fs::create_dir_all(&shared).unwrap();
        setup_test_domain(&user, "linux");

        let registry = DomainRegistry::new(prebuilt, user.clone(), shared.clone()).unwrap();

        let result = registry.get_operation("list_processes");
        assert!(result.is_some(), "Should find list_processes operation");

        let (domain, op) = result.unwrap();
        assert_eq!(domain.id, "linux");
        assert_eq!(op.id, "list_processes");
    }

    #[test]
    fn test_entity_lookup() {
        let base = test_root_dir();
        let user = test_user_dir(&base, "domains");
        let shared = test_user_dir(&base, "shared_entities");
        let prebuilt = test_user_dir(&base, "domains");

        std::fs::create_dir_all(&user).unwrap();
        std::fs::create_dir_all(&shared).unwrap();
        setup_test_domain(&user, "linux");

        let entity_json = r#"{
            "name": "Process",
            "description": "A running process",
            "core_properties": [
                {"name": "pid", "type": "integer", "meaning": "Process ID"}
            ]
        }"#;
        std::fs::write(user.join("linux/entities/process.json"), entity_json).unwrap();

        let registry = DomainRegistry::new(prebuilt, user.clone(), shared.clone()).unwrap();

        let entity = registry.get_entity("Process");
        assert!(entity.is_some(), "Should find Process entity");
        assert_eq!(entity.unwrap().name, "Process");
    }
}
