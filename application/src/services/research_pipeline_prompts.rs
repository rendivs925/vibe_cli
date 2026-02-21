use crate::services::research_pipeline_service::SpeculationLevel;

pub fn stage_system(stage: &str) -> String {
    format!(
        "You are a research co-pilot specializing in physics, AI, energy, and advanced propulsion. \
Work in a disciplined, structured way. This stage is: {}. \
Use clear headings and bullet points, cite sources as [#] where possible, and avoid fluff.",
        stage
    )
}

pub fn evidence_prompt(query: &str, context: &str) -> String {
    format!(
        "Goal: Build an evidence ledger for the topic.\n\
Topic: {query}\n\n\
Sources:\n{context}\n\
Instructions:\n\
- Extract factual claims, methods, and constraints.\n\
- Group by theme.\n\
- For each claim, include [#] source reference and confidence (0-1).\n\
- Note contradictions and open gaps.\n\
Output format:\n\
Evidence Ledger:\n\
- Theme: ...\n  - Claim: ... [#] (confidence)\n  - Method/Support: ...\n  - Constraints: ...\n\
Gaps:\n- ...\n",
        query = query,
        context = context
    )
}

pub fn hypotheses_prompt(query: &str, speculation: SpeculationLevel, evidence: &str) -> String {
    format!(
        "Goal: Generate speculative hypotheses grounded in evidence.\n\
Topic: {query}\n\
Speculation level: {spec}\n\n\
Evidence:\n{evidence}\n\n\
Instructions:\n\
- Propose 6-12 hypotheses.\n\
- Each hypothesis includes: description, required assumptions, why it might work, and a quick test.\n\
- Use [#] citations.\n\
Output format:\n\
Hypotheses:\n\
1. ...\n   - Assumptions: ...\n   - Why plausible: ... [#]\n   - Fast test: ...\n",
        query = query,
        spec = speculation_label(speculation),
        evidence = evidence
    )
}

pub fn critique_prompt(query: &str, hypotheses: &str) -> String {
    format!(
        "Goal: Critique hypotheses for feasibility and contradictions.\n\
Topic: {query}\n\n\
Hypotheses:\n{hypotheses}\n\n\
Instructions:\n\
- Identify physics constraints, missing evidence, and logical gaps.\n\
- Score each hypothesis (feasibility 0-1, novelty 0-1, risk 0-1).\n\
- Suggest how to strengthen or falsify each.\n\
Output format:\n\
Critique:\n\
1. ...\n   - Feasibility: ...\n   - Novelty: ...\n   - Risk: ...\n   - Weak points: ...\n   - Strengthen by: ...\n",
        query = query,
        hypotheses = hypotheses
    )
}

pub fn refine_prompt(
    query: &str,
    speculation: SpeculationLevel,
    hypotheses: &str,
    critique: &str,
) -> String {
    format!(
        "Goal: Refine hypotheses using critique.\n\
Topic: {query}\n\
Speculation level: {spec}\n\n\
Hypotheses:\n{hypotheses}\n\n\
Critique:\n{critique}\n\n\
Instructions:\n\
- Rewrite the hypotheses to resolve the biggest weaknesses.\n\
- Keep 5-10 refined hypotheses.\n\
- Each should include: description, assumptions, why it might work, and a falsification test.\n\
Output format:\n\
Refined Hypotheses:\n\
1. ...\n   - Assumptions: ...\n   - Why plausible: ...\n   - Falsification: ...\n",
        query = query,
        spec = speculation_label(speculation),
        hypotheses = hypotheses,
        critique = critique
    )
}

pub fn experiment_prompt(query: &str, refined: &str) -> String {
    format!(
        "Goal: Design experiments or simulations to test refined hypotheses.\n\
Topic: {query}\n\n\
Refined Hypotheses:\n{refined}\n\n\
Instructions:\n\
- Propose experiments ordered by fastest-to-run and lowest-cost first.\n\
- Include required tools, expected signal, and what result would falsify it.\n\
Output format:\n\
Experiments:\n\
1. ...\n   - Tools: ...\n   - Expected signal: ...\n   - Falsification: ...\n",
        query = query,
        refined = refined
    )
}

pub fn invention_prompt(
    query: &str,
    speculation: SpeculationLevel,
    refined: &str,
    experiments: Option<&str>,
) -> String {
    let experiments = experiments.unwrap_or("");
    format!(
        "Goal: Propose novel research directions that could be groundbreaking.\n\
Topic: {query}\n\
Speculation level: {spec}\n\n\
Refined Hypotheses:\n{refined}\n\n\
Experiments:\n{experiments}\n\n\
Instructions:\n\
- Propose 3-6 novel directions or architectures.\n\
- Explain why each might be breakthrough.\n\
- Include the shortest path to validate.\n\
Output format:\n\
Novel Directions:\n\
1. ...\n   - Breakthrough rationale: ...\n   - Fast validation: ...\n",
        query = query,
        spec = speculation_label(speculation),
        refined = refined,
        experiments = experiments
    )
}

fn speculation_label(level: SpeculationLevel) -> &'static str {
    match level {
        SpeculationLevel::Low => "Low (conservative, evidence-heavy)",
        SpeculationLevel::Medium => "Medium (balanced, cautious leaps)",
        SpeculationLevel::High => "High (bold, creative leaps allowed)",
    }
}
