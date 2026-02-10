use domain::domain_config::DomainRegistry;
use std::path::PathBuf;

#[test]
fn test_full_neurosymbolic_domain_coverage() {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/neurosymbolic");
    let domains_dir = base.join("domains");
    let shared_dir = base.join("shared_entities");

    let registry = DomainRegistry::new(domains_dir.clone(), domains_dir, shared_dir).unwrap();

    // Domains
    let domains = registry.list_domains();
    assert!(domains.contains(&"linux".to_string()));

    // Expected entities
    let entities = [
        "Process",
        "File",
        "Service",
        "NetworkConnection",
        "User",
        "Filesystem",
        "MemoryInfo",
        "Cpu",
        "NetworkInterface",
        "DockerContainer",
        "SystemdUnit",
    ];
    for name in entities {
        assert!(
            registry.get_entity(name).is_some(),
            "Missing entity: {}",
            name
        );
    }

    // Expected relationships
    let relationships = [
        "process_writes_file",
        "user_owns_file",
        "filesystem_contains_file",
        "service_manages_process",
        "interface_has_connection",
        "container_runs_process",
        "systemd_controls_service",
        "cpu_runs_process",
    ];
    for name in relationships {
        assert!(
            registry.get_relationship(name).is_some(),
            "Missing relationship: {}",
            name
        );
    }

    // Expected inference rules (30)
    for i in 1..=30 {
        let id = format!("rule_{:02}", i);
        assert!(
            registry.get_inference_rule(&id).is_some(),
            "Missing inference rule: {}",
            id
        );
    }

    // Expected troubleshooting patterns (15)
    for i in 1..=15 {
        let id = format!("pattern_{:02}", i);
        assert!(
            registry.get_troubleshooting_pattern(&id).is_some(),
            "Missing troubleshooting pattern: {}",
            id
        );
    }

    // Expected reasoning templates (21)
    for i in 1..=21 {
        let id = format!("template_{:02}", i);
        assert!(
            registry.get_reasoning_template(&id).is_some(),
            "Missing reasoning template: {}",
            id
        );
    }

    // Expected operations (32)
    let op_ids = [
        "list_processes",
        "find_process",
        "kill_process",
        "process_tree",
        "check_memory",
        "top_memory",
        "memory_stats",
        "disk_usage",
        "disk_inodes",
        "list_large_files",
        "list_listening_ports",
        "check_connection",
        "ping_host",
        "dns_lookup",
        "service_status",
        "service_restart",
        "list_services",
        "enable_service",
        "list_files",
        "find_files",
        "file_permissions",
        "file_ownership",
        "tail_file",
        "list_containers",
        "container_logs",
        "container_stats",
        "cpu_info",
        "gpu_info",
        "hardware_summary",
        "user_list",
        "login_history",
        "firewall_status",
    ];

    let ops = registry.list_operations();
    assert_eq!(ops.len(), op_ids.len(), "Unexpected operations count");

    for op_id in op_ids {
        let op = registry.get_operation(op_id);
        assert!(op.is_some(), "Missing operation: {}", op_id);
        let (_domain, op) = op.unwrap();

        // Ensure command generation yields at least one candidate
        let inputs = op
            .examples
            .first()
            .map(|ex| ex.inputs.clone())
            .unwrap_or_default();
        let commands = registry.command_generator().generate(op, &inputs);
        assert!(
            !commands.is_empty(),
            "No commands generated for operation: {}",
            op_id
        );

        // Ensure intent resolution can find the operation
        let query = op_id.replace('_', " ");
        let resolved = registry.resolve_operation(&query);
        assert!(
            resolved.is_some(),
            "Failed to resolve operation for query: {}",
            query
        );
        let resolved = resolved.unwrap();
        assert_eq!(
            resolved.op_id, op_id,
            "Resolved wrong operation for query: {}",
            query
        );
    }
}
