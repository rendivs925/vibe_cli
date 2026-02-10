use application::services::integrated_neurosymbolic_service::{
    IntegratedNeurosymbolicService, NeurosymbolicConfig,
};
use domain::domain_config::DomainRegistry;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

struct HomeGuard {
    original: Option<String>,
}

impl Drop for HomeGuard {
    fn drop(&mut self) {
        if let Some(value) = &self.original {
            env::set_var("HOME", value);
        } else {
            env::remove_var("HOME");
        }
    }
}

fn set_temp_home() -> (PathBuf, HomeGuard) {
    let original = env::var("HOME").ok();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_home = env::temp_dir().join(format!("vibe_cli_test_{}", nanos));
    fs::create_dir_all(&temp_home).unwrap();
    env::set_var("HOME", &temp_home);
    (temp_home, HomeGuard { original })
}

fn write_test_domain(home: &PathBuf) {
    let base = home.join(".config/vibe_cli");
    let domains_dir = base.join("domains");
    let shared_dir = base.join("shared_entities");
    let domain_dir = domains_dir.join("linux");
    let entities_dir = domain_dir.join("entities");

    fs::create_dir_all(&entities_dir).unwrap();
    fs::create_dir_all(&shared_dir).unwrap();

    let domain_json = r#"{
        "domain": "linux",
        "version": "2.0.0",
        "description": "Test Linux domain",
        "depends_on": [],
        "priority": 10,
        "enabled": true,
        "tags": ["linux", "process", "filesystem", "network", "services", "users"]
    }"#;
    fs::write(domain_dir.join("domain.json"), domain_json).unwrap();

    let operations_json = r#"[
        {
            "op_id": "list_processes",
            "name": "list processes",
            "description": "list running processes",
            "intent": "list processes",
            "input_schema": {},
            "generators": [
                {"name": "ps_standard", "tool": "ps", "template": "ps aux", "when": []}
            ],
            "examples": [{"description": "list processes", "inputs": {}}]
        },
        {
            "op_id": "check_disk_usage",
            "name": "check disk usage",
            "description": "check disk usage",
            "intent": "disk full",
            "input_schema": {},
            "generators": [
                {"name": "df_h", "tool": "df", "template": "df -h", "when": []}
            ],
            "examples": [{"description": "disk is full", "inputs": {}}]
        },
        {
            "op_id": "check_memory_usage",
            "name": "check memory usage",
            "description": "check memory usage",
            "intent": "memory full",
            "input_schema": {},
            "generators": [
                {"name": "free_h", "tool": "free", "template": "free -h", "when": []}
            ],
            "examples": [{"description": "memory is full", "inputs": {}}]
        },
        {
            "op_id": "service_status",
            "name": "check service status",
            "description": "check service status",
            "intent": "service status",
            "input_schema": {
                "service": {"type": "string", "meaning": "service name", "optional": false},
                "action": {"type": "string", "meaning": "service action", "optional": false}
            },
            "generators": [
                {
                    "name": "systemctl_action",
                    "tool": "systemctl",
                    "template": "systemctl {{action}} {{service}}",
                    "when": [{"name": "service"}, {"name": "action"}]
                }
            ],
            "examples": [{"description": "check nginx status", "inputs": {"service": "nginx", "action": "status"}}]
        },
        {
            "op_id": "tail_logs",
            "name": "tail logs",
            "description": "show log lines",
            "intent": "show last lines",
            "input_schema": {
                "lines": {"type": "integer", "meaning": "lines to show", "optional": false},
                "log": {"type": "string", "meaning": "log name", "optional": false}
            },
            "generators": [
                {
                    "name": "tail_syslog",
                    "tool": "tail",
                    "template": "tail -n {{lines}} /var/log/{{log}}",
                    "when": [{"name": "lines"}, {"name": "log"}]
                }
            ],
            "examples": [{"description": "show last 50 lines of syslog", "inputs": {"lines": 50, "log": "syslog"}}]
        },
        {
            "op_id": "show_gpu_name",
            "name": "show gpu name",
            "description": "show gpu name",
            "intent": "gpu name",
            "input_schema": {},
            "generators": [
                {"name": "lspci_vga", "tool": "lspci", "template": "lspci | grep -i vga", "when": []}
            ],
            "examples": [{"description": "show my gpu name", "inputs": {}}]
        }
    ]"#;
    fs::write(domain_dir.join("operations.json"), operations_json).unwrap();

    fs::write(domain_dir.join("relationships.json"), "[]").unwrap();

    let inference_rules_json = r#"[
        {
            "rule_id": "zombie_detect",
            "name": "Zombie Detection",
            "if": [{"entity": "Process", "prop": "state", "equals": "Z"}],
            "then": [{"conclude": "zombie_process", "confidence": 0.99}]
        }
    ]"#;
    fs::write(domain_dir.join("inference_rules.json"), inference_rules_json).unwrap();

    let troubleshooting_json = r#"[
        {
            "pattern_id": "high_cpu",
            "name": "High CPU Usage",
            "symptoms": [{"metric": "cpu", "observation": "high cpu"}],
            "likely_causes": [{"cause": "runaway_process", "probability": 0.6}],
            "checks": [{"step": "Find CPU hog", "command": "top -bn1 | head -20"}],
            "actions": [{"action": "kill_process", "methods": ["kill", "pkill"]}]
        }
    ]"#;
    fs::write(domain_dir.join("troubleshooting.json"), troubleshooting_json).unwrap();

    let templates_json = r#"[
        {
            "template_id": "disk_full",
            "goal": "disk full",
            "inputs": [{"name": "path", "type": "string", "optional": true, "example": "/"}],
            "steps": [{"step": 1, "check": "df -h", "logic": "identify filesystem at capacity", "next": ["cleanup"]}],
            "outputs": [{"name": "cleanup", "type": "string", "example": "remove unused files"}]
        }
    ]"#;
    fs::write(domain_dir.join("reasoning_templates.json"), templates_json).unwrap();

    let process_entity = r#"{
        "name": "Process",
        "description": "A running process on the system",
        "core_properties": [
            {"name": "pid", "type": "integer", "meaning": "Process ID"},
            {"name": "cpu", "type": "number", "meaning": "CPU usage percentage"},
            {"name": "state", "type": "string", "meaning": "Process state (R/S/D/Z)"}
        ],
        "derived_properties": [
            {"name": "is_zombie", "expression": "state == 'Z'"}
        ]
    }"#;
    fs::write(entities_dir.join("process.json"), process_entity).unwrap();

    let filesystem_entity = r#"{
        "name": "Filesystem",
        "description": "A filesystem on the system",
        "core_properties": [
            {"name": "mount", "type": "string", "meaning": "Mount point"},
            {"name": "usage", "type": "number", "meaning": "Disk usage percent"}
        ]
    }"#;
    fs::write(entities_dir.join("filesystem.json"), filesystem_entity).unwrap();

    let memory_entity = r#"{
        "name": "Memory",
        "description": "System memory",
        "core_properties": [
            {"name": "total", "type": "number", "meaning": "Total memory"},
            {"name": "used", "type": "number", "meaning": "Used memory"}
        ]
    }"#;
    fs::write(entities_dir.join("memory.json"), memory_entity).unwrap();
}

#[test]
fn test_neurosymbolic_feature_requests() {
    let (home, _guard) = set_temp_home();
    write_test_domain(&home);

    let mut service = IntegratedNeurosymbolicService::with_config(NeurosymbolicConfig {
        enable_safety: true,
        enable_manpage_validation: false,
        enable_learning: false,
        block_on_safety: true,
        block_on_invalid_syntax: true,
    })
    .unwrap();
    service.reload_domain_registry().unwrap();

    let cases = vec![
        ("list processes", "ps aux", false),
        ("check disk usage", "df -h", false),
        ("disk is full", "df -h", true),
        ("memory is full", "free -h", false),
        ("check nginx status", "systemctl status nginx", false),
        ("show last 50 lines of syslog", "tail -n 50 /var/log/syslog", false),
        ("show my gpu name", "lspci | grep -i vga", false),
    ];

    for (query, expected_command, expect_template) in cases {
        let result = service.process(query).unwrap();
        assert!(
            result.command.contains(expected_command),
            "Query '{}' expected command containing '{}', got '{}'",
            query,
            expected_command,
            result.command
        );
        assert!(
            result.can_execute,
            "Query '{}' should be executable, got block reason: {:?}",
            query,
            result.block_reason
        );
        if expect_template {
            assert!(
                result.reasoning_template.is_some(),
                "Query '{}' should resolve a reasoning template",
                query
            );
        }
    }

    let home_dir = PathBuf::from(env::var("HOME").unwrap());
    let base = home_dir.join(".config/vibe_cli");
    let domains_dir = base.join("domains");
    let shared_dir = base.join("shared_entities");
    let registry = DomainRegistry::new(domains_dir.clone(), domains_dir, shared_dir).unwrap();

    let mut context = HashMap::new();
    context.insert(
        "Process".to_string(),
        serde_json::Value::String("Z".to_string()),
    );
    let inferences = registry.apply_inference_rules(&context);
    assert!(
        inferences.iter().any(|v| {
            v.get("conclude")
                .and_then(|c| c.as_str())
                .map(|c| c == "zombie_process")
                .unwrap_or(false)
        }),
        "Expected zombie_process inference"
    );

    let patterns = registry.find_troubleshooting_patterns(&vec!["high cpu usage".to_string()]);
    assert!(
        patterns.iter().any(|(_, p)| p.id == "high_cpu"),
        "Expected high_cpu troubleshooting pattern"
    );

    let template = registry.resolve_reasoning_template("disk is full");
    assert!(
        template.is_some(),
        "Expected reasoning template match for 'disk is full'"
    );
}
