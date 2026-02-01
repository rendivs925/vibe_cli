use vibe_cli::application::use_cases::command_use_case::CommandUseCase;
use vibe_cli::domain::entities::command::Command;
use vibe_cli::infrastructure::storage::StorageService;
use vibe_cli::infrastructure::cache::Cache;
use vibe_cli::shared::confirmation::ask_confirmation;

#[tokio::test]
async fn test_neurosymbolic_integration() {
    // Create a simple mock storage and cache
    let storage = vibe_cli::infrastructure::storage::StorageService::new();
    let cache = Box::new() as Box<dyn vibe_cli::infrastructure::cache::Cache>;
    
    let neuroservice = vibe_cli::application::services::neurosymbolic_service::NeurosymbolicService::new(
        vibe_cli::application::services::neurosymbolic_service::NeurosymbolicConfig::default()
    ).await.unwrap();
    
    let use_case = CommandUseCase::new(
        vibe_cli::domain::services::command_planner::CommandPlanner::new(),
        storage,
        cache,
        neuroservice,
    );
    
    // Test neurosymbolic command generation
    let test_query = "Set up secure web server";
    let result = use_case.generate_command(test_query).await;
    
    assert!(result.is_ok(), "Neurosymbolic command generation should succeed");
    
    if let Ok(plan) = result {
        println!("✅ Neurosymbolic Integration Test Passed");
        println!("🧠 Intent Analysis: {}", plan.command().description());
        
        // Test confirmation mechanism
        if ask_confirmation("Execute this neurosymbolic command?", true).unwrap() {
            println!("🚀 Command execution confirmed");
        } else {
            println!("❌ Command execution cancelled");
        }
    } else {
        println!("❌ Neurosymbolic Integration Test Failed: {:?}", result.err());
    }
}