use super::CliHandlers;
use application::services::research_agent_service::ResearchDepth;
use shared::types::Result;

impl CliHandlers {
    pub async fn handle_task(&self, query: &str) -> Result<()> {
        use application::services::task_service::{TaskService, TaskStatus};
        
        println!("Task Mode: {}", query);
        
        let mut service = TaskService::new();
        
        let task_id = service.create_task(query.to_string(), format!("Task: {}", query));
        
        println!("Created task: {}", task_id);
        
        let steps = service.decompose_task(query);
        println!("Decomposed into {} steps:", steps.len());
        for (i, step) in steps.iter().enumerate() {
            println!("  {}. {}", i + 1, step);
        }
        
        let tasks = service.list_tasks(None);
        println!("\nAll tasks:");
        for t in tasks {
            println!("  - {}: {} ({:?})", t.id, t.title, t.status);
        }
        
        Ok(())
    }
    
    pub async fn handle_research(&self, query: &str, depth: ResearchDepth) -> Result<()> {
        use application::services::research_agent_service::ResearchAgent;
        
        println!("Research Mode: {} (depth: {:?})", query, depth);
        
        let mut agent = ResearchAgent::new();
        
        let query_id = agent.start_research(query.to_string(), depth);
        println!("Started research: {}", query_id);
        
        println!("\nNote: Web search integration requires configuration.");
        println!("To enable web research, set SEARCH_API_KEY environment variable.");
        
        println!("\nResearch queries:");
        for q in agent.list_queries() {
            println!("  - {}: {} ({:?})", q.id, q.query, q.status);
        }
        
        Ok(())
    }
    
    pub async fn handle_work(&self, query: &str) -> Result<()> {
        use infrastructure::tools::documents::creator::{DocumentCreator, SpreadsheetAnalyzer};
        use std::path::Path;
        
        println!("Work Mode: {}", query);
        
        let query_lower = query.to_lowercase();
        
        if query_lower.contains("analyze") || query_lower.contains("spreadsheet") || query_lower.contains("excel") || query_lower.contains("csv") {
            println!("\nSearching for spreadsheets in current directory...");
            
            let current_dir = std::env::current_dir()?;
            let mut found_files = Vec::new();
            
            if let Ok(entries) = std::fs::read_dir(&current_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        if ext == "csv" || ext == "xlsx" || ext == "xls" {
                            found_files.push(path);
                        }
                    }
                }
            }
            
            if found_files.is_empty() {
                println!("No spreadsheets found in current directory.");
            } else {
                println!("Found {} spreadsheet(s):", found_files.len());
                for path in &found_files {
                    println!("  - {}", path.display());
                    
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        if ext == "csv" {
                            if let Ok(analyzer) = SpreadsheetAnalyzer::from_csv(path.to_str().unwrap_or("")) {
                                println!("\nSummary:\n{}", analyzer.get_summary());
                            }
                        } else if ext == "xlsx" || ext == "xls" {
                            if let Ok(analyzer) = SpreadsheetAnalyzer::from_xlsx(path.to_str().unwrap_or("")) {
                                println!("\nSummary:\n{}", analyzer.get_summary());
                            }
                        }
                    }
                }
            }
        } else if query_lower.contains("create") || query_lower.contains("document") {
            println!("\nDocument creation:");
            println!("  - Markdown: create .md files");
            println!("  - CSV: create .csv files");
            println!("  - HTML: create .html files");
            println!("\nUsage: vibe_cli --work 'create report.md with content'");
        } else {
            println!("\nWork mode supports:");
            println!("  - Analyzing spreadsheets (CSV, XLSX)");
            println!("  - Creating documents (Markdown, CSV, HTML)");
            println!("  - Processing PDFs");
            println!("\nExamples:");
            println!("  vibe_cli --work 'analyze sales.csv'");
            println!("  vibe_cli --work 'create meeting notes.md'");
        }
        
        Ok(())
    }
}
