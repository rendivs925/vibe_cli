use infrastructure::ollama_client::OllamaClient;
use shared::types::Result;

use crate::services::research_agent_service::{ResearchDepth, ResearchSource};
use crate::services::research_pipeline_prompts::{
    critique_prompt, evidence_prompt, experiment_prompt, invention_prompt, refine_prompt,
    stage_system, hypotheses_prompt,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResearchMode {
    Invention,
    Hypothesis,
    Experiment,
    Critique,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpeculationLevel {
    Low,
    Medium,
    High,
}

pub struct ResearchPipelineService {
    client: OllamaClient,
}

impl ResearchPipelineService {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: OllamaClient::new()?,
        })
    }

    pub async fn run(
        &self,
        query: &str,
        depth: ResearchDepth,
        mode: ResearchMode,
        speculation: SpeculationLevel,
        sources: &[&ResearchSource],
    ) -> Result<String> {
        let source_context = build_source_context(sources, depth);
        let source_list = build_source_list(sources, depth);
        let evidence = self
            .generate_stage(
                stage_system("evidence"),
                evidence_prompt(query, &source_context),
            )
            .await?;
        let hypotheses = self
            .generate_stage(
                stage_system("hypotheses"),
                hypotheses_prompt(query, speculation, &evidence),
            )
            .await?;
        let critique = self
            .generate_stage(
                stage_system("critique"),
                critique_prompt(query, &hypotheses),
            )
            .await?;
        let refined = self
            .generate_stage(
                stage_system("refine"),
                refine_prompt(query, speculation, &hypotheses, &critique),
            )
            .await?;

        let experiments = if matches!(mode, ResearchMode::Experiment | ResearchMode::Invention) {
            Some(
                self.generate_stage(
                    stage_system("experiments"),
                    experiment_prompt(query, &refined),
                )
                .await?,
            )
        } else {
            None
        };

        let invention = if mode == ResearchMode::Invention {
            Some(
                self.generate_stage(
                    stage_system("invention"),
                    invention_prompt(query, speculation, &refined, experiments.as_deref()),
                )
                .await?,
            )
        } else {
            None
        };

        Ok(format_brief(
            query,
            depth,
            mode,
            speculation,
            &source_list,
            &evidence,
            &refined,
            &critique,
            experiments.as_deref(),
            invention.as_deref(),
        ))
    }

    async fn generate_stage(&self, system: String, prompt: String) -> Result<String> {
        let response = self
            .client
            .generate_response_with_system(&prompt, &system)
            .await?;
        Ok(response.trim().to_string())
    }
}

fn build_source_context(sources: &[&ResearchSource], depth: ResearchDepth) -> String {
    let (max_sources, max_chars) = source_limits(depth);
    let mut out = String::new();
    for (i, source) in sources.iter().take(max_sources).enumerate() {
        out.push_str(&format!(
            "[{}] {}\nURL: {}\nContent: {}\n\n",
            i + 1,
            source.title.trim(),
            source.url.trim(),
            trim_for_prompt(&source.content, max_chars)
        ));
    }
    out
}

fn build_source_list(sources: &[&ResearchSource], depth: ResearchDepth) -> String {
    let (max_sources, _) = source_limits(depth);
    let mut out = String::new();
    for (i, source) in sources.iter().take(max_sources).enumerate() {
        out.push_str(&format!(
            "{}. {}\n   URL: {}\n",
            i + 1,
            source.title.trim(),
            source.url.trim()
        ));
    }
    out.trim().to_string()
}

fn source_limits(depth: ResearchDepth) -> (usize, usize) {
    match depth {
        ResearchDepth::Quick => (3, 1800),
        ResearchDepth::Standard => (6, 2400),
        ResearchDepth::Deep => (10, 3200),
        ResearchDepth::Comprehensive => (14, 4200),
    }
}

fn trim_for_prompt(text: &str, max_len: usize) -> String {
    let cleaned = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.len() <= max_len {
        return cleaned;
    }
    let mut out = String::new();
    let mut count = 0;
    for ch in cleaned.chars() {
        if count >= max_len {
            break;
        }
        out.push(ch);
        count += 1;
    }
    out.push_str("...");
    out
}


fn format_brief(
    query: &str,
    depth: ResearchDepth,
    mode: ResearchMode,
    speculation: SpeculationLevel,
    source_list: &str,
    evidence: &str,
    refined: &str,
    critique: &str,
    experiments: Option<&str>,
    invention: Option<&str>,
) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Research Brief: {}\n\n", query));
    out.push_str(&format!("**Depth:** {:?}\n", depth));
    out.push_str(&format!("**Mode:** {:?}\n", mode));
    out.push_str(&format!("**Speculation:** {:?}\n\n", speculation));

    out.push_str("## Sources\n\n");
    out.push_str(source_list.trim());
    out.push_str("\n\n## Evidence Ledger\n\n");
    out.push_str(evidence.trim());
    out.push_str("\n\n## Refined Hypotheses\n\n");
    out.push_str(refined.trim());

    if mode != ResearchMode::Hypothesis {
        out.push_str("\n\n## Critique\n\n");
        out.push_str(critique.trim());
    }

    if let Some(experiments) = experiments {
        out.push_str("\n\n## Experiments\n\n");
        out.push_str(experiments.trim());
    }

    if let Some(invention) = invention {
        out.push_str("\n\n## Novel Directions\n\n");
        out.push_str(invention.trim());
    }

    if mode == ResearchMode::Critique {
        out.push_str("\n\n## Focus\n\n");
        out.push_str("This mode emphasizes critical evaluation over novelty.");
    }

    out
}
