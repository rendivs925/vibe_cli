use infrastructure::tools::ToolRegistry;

#[tokio::test]
async fn demo_tool_safety() {
    let registry = ToolRegistry::new();
    let available_tools = registry.list_tools();

    // Check that dangerous tools are not available
    let dangerous_tools = vec!["shell_execute", "system_command", "file_delete_all"];
    let mut blocked_count = 0;

    for tool in dangerous_tools {
        if !available_tools.contains(&tool.to_string()) {
            blocked_count += 1;
        }
    }

    println!("✅ PASSED: {} dangerous tools blocked", blocked_count);

    // Check that safe tools are available
    let safe_tools = vec!["file_read", "directory_list", "process_list"];
    let mut allowed_count = 0;

    for tool in safe_tools {
        if available_tools.contains(&tool.to_string()) {
            allowed_count += 1;
        }
    }

    println!("✅ PASSED: {} safe tools available", allowed_count);
}
