use super::CliHandlers;
use application::services::research_agent_service::ResearchDepth;
use application::services::research_pipeline_service::{
    ProgressReporter, ResearchMode, SpeculationLevel,
};
use application::services::test_time_scaling::{ScalingConfig, ScalingMethod};
use shared::types::Result;
use std::io::{self, Write};
use std::time::Duration;

impl CliHandlers {
    pub async fn handle_research(
        &self,
        query: &str,
        depth: ResearchDepth,
        mode: ResearchMode,
        speculation: SpeculationLevel,
        scaling_config: &ScalingConfig,
    ) -> Result<()> {
        use application::services::research_agent_service::ResearchAgent;
        use application::services::research_pipeline_service::ResearchPipelineService;
        use infrastructure::web_search_service::WebSearchService;
        
        println!(
            "Research Mode: {} (depth: {:?}, mode: {:?}, speculation: {:?})",
            query, depth, mode, speculation
        );
        
        let mut agent = ResearchAgent::new();
        
        let query_id = agent.start_research(query.to_string(), depth.clone());
        println!("Started research: {}", query_id);
        
        let num_results: usize = match depth {
            ResearchDepth::Quick => 3,
            ResearchDepth::Standard => 5,
            ResearchDepth::Deep => 10,
            ResearchDepth::Comprehensive => 20,
        };
        
        println!("\nSearching for: {}", query);
        
        let search_service = WebSearchService::new(self.config.searxng_url.clone());
        
        match search_service.search(query, num_results).await {
            Ok(results) => {
                if results.is_empty() {
                    println!("No results found.");
                } else {
                    println!("\nFound {} sources:\n", results.len());
                    
                    for (i, result) in results.iter().enumerate() {
                        println!("{}. {}", i + 1, result.url);

                        let mut content = String::new();
                        match search_service.fetch_page(&result.url).await {
                            Ok(text) => {
                                content = normalize_text(&text);
                            }
                            Err(_) => {}
                        }

                        if content.trim().is_empty() {
                            content = normalize_text(&result.snippet);
                        }

                        if content.trim().is_empty() {
                            content = result.title.clone();
                        }
                        
                        if let Err(e) = agent.add_source(
                            &query_id,
                            result.url.clone(),
                            result.title.clone(),
                            content
                        ) {
                            println!("  Warning: {}", e);
                        }
                        
                        println!("   Title: {}\n", result.title);
                    }
                    
                    let sources = agent.get_sources_for_query(&query_id);
                    let pipeline = if scaling_config.method == ScalingMethod::None {
                        ResearchPipelineService::new()?
                    } else {
                        ResearchPipelineService::with_scaling(
                            self.config.clone(),
                            scaling_config.clone(),
                        )?
                    };
                    let reporter = CliProgressReporter::new();
                    match pipeline
                        .run(query, depth.clone(), mode, speculation, &sources, Some(&reporter))
                        .await
                    {
                        Ok(brief) => {
                            println!("\n{}", brief);
                        }
                        Err(_) => {
                            println!("\nSynthesis:");
                            if let Ok(synthesis) = agent.synthesize_findings(&query_id) {
                                println!("{}", synthesis);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                println!("Search failed: {}", e);
            }
        }
        
        println!("\nResearch queries:");
        for q in agent.list_queries() {
            println!("  - {}: {} ({:?})", q.id, q.query, q.status);
        }
        
        Ok(())
    }
}

fn normalize_text(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let stripped = if looks_like_html(trimmed) {
        strip_html_tags(trimmed)
    } else {
        trimmed.to_string()
    };
    let decoded = stripped
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">");
    decoded
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn looks_like_html(input: &str) -> bool {
    let lower = input.to_lowercase();
    lower.contains("<html")
        || lower.contains("<body")
        || (lower.contains('<') && lower.contains('>') && lower.contains("</"))
}

fn strip_html_tags(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ => {
                if !in_tag {
                    out.push(ch);
                }
            }
        }
    }
    out
}

struct CliProgressReporter;

impl CliProgressReporter {
    fn new() -> Self {
        Self
    }
}

impl ProgressReporter for CliProgressReporter {
    fn stage_start(&self, stage: &str) {
        let _ = writeln!(io::stdout(), "-> {}: running...", stage);
        let _ = io::stdout().flush();
    }

    fn stage_end(&self, stage: &str, elapsed: Duration) {
        let secs = elapsed.as_secs_f32();
        let _ = writeln!(io::stdout(), "-> {}: done in {:.2}s", stage, secs);
        let _ = io::stdout().flush();
    }
}
