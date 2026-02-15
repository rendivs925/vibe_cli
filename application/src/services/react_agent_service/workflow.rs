use super::ReactAgentService;
use domain::entities::react::{
    ProposedCommand, ReactContext, ReactSession, ReactStatus, ReactStep, ReactStepType,
};
use shared::types::Result;

impl ReactAgentService {
    pub async fn execute_react_loop(&self, session: &mut ReactSession) -> Result<()> {
        let mut context = ReactContext::new(self.max_iterations);

        while context.should_continue() && matches!(session.status, ReactStatus::Running) {
            let reasoning = self.generate_reasoning(session).await?;
            self.ingest_reasoning(session, &reasoning);
            let thought_step = ReactStep::new(
                session.id.clone(),
                ReactStepType::Thought,
                reasoning.clone(),
            )
            .with_reasoning(reasoning.clone());
            self.add_step(session, thought_step).await?;

            let commands = self.propose_commands(&reasoning, session).await?;
            let mut action_step = ReactStep::new(
                session.id.clone(),
                ReactStepType::Action,
                "Proposed commands".to_string(),
            );
            for command in commands {
                self.command_repository
                    .save_command(&command)
                    .await
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                action_step.add_command(command);
            }
            self.add_step(session, action_step).await?;

            context.increment_iteration();
            break;
        }

        Ok(())
    }

    pub async fn process_user_input(
        &self,
        session: &mut ReactSession,
        input: String,
    ) -> Result<ReactStep> {
        self.ingest_user_input(session, &input);
        let mut step = ReactStep::new(session.id.clone(), ReactStepType::Observation, input);
        step.start();
        step.complete();
        self.add_step(session, step.clone()).await?;
        Ok(step)
    }

    pub fn ingest_user_input(&self, session: &mut ReactSession, input: &str) {
        for constraint in self.analysis_service.extract_constraints(input) {
            session.memory.add_constraint(constraint);
        }
    }

    pub fn ingest_reasoning(&self, session: &mut ReactSession, reasoning: &str) {
        for hypothesis in self.analysis_service.extract_hypotheses_from_reasoning(reasoning) {
            session.memory.add_hypothesis(hypothesis);
        }
        for insight in self.analysis_service.extract_insights_from_reasoning(reasoning) {
            session.memory.add_insight(insight);
        }
    }

    pub fn ingest_observation(
        &self,
        session: &mut ReactSession,
        command: &str,
        output: &str,
        step_index: usize,
    ) {
        for fact in self
            .analysis_service
            .extract_facts_from_output(output, command, step_index)
        {
            session.memory.add_fact(fact);
        }
    }

    pub fn reset_memory(&self, session: &mut ReactSession) {
        session.memory.reset_facts_and_hypotheses();
    }

    pub async fn compact_history(&self, session: &ReactSession) -> Result<String> {
        if session.steps.len() <= 6 {
            return Ok("Not enough history to compact.".to_string());
        }

        let history = self.context_retriever.retrieve(session).session_history;
        let prompt = self.prompt_service.compact_prompt(&history);
        let response = self.client.generate_response(&prompt).await?;
        let summary = response.trim().to_string();
        if summary.is_empty() {
            Ok("Summary unavailable.".to_string())
        } else {
            Ok(summary)
        }
    }

    pub async fn generate_symbolic_inference(
        &self,
        session: &ReactSession,
    ) -> Result<Option<String>> {
        if !session.neurosymbolic_enabled {
            return Ok(None);
        }

        let history = self.context_retriever.retrieve(session).session_history;
        let prompt = self
            .prompt_service
            .symbolic_inference_prompt(&session.query, &history);

        let response = self.client.generate_response(&prompt).await?;
        let cleaned = response.trim().to_string();
        if cleaned.is_empty() {
            return Ok(None);
        }
        Ok(Some(cleaned))
    }

    pub async fn is_goal_achieved(&self, session: &ReactSession) -> Result<bool> {
        let history = self.context_retriever.retrieve(session).session_history;
        let prompt = self
            .prompt_service
            .goal_check_prompt(&session.query, &history);

        let response = self.client.generate_response(&prompt).await?;
        Ok(response.trim().eq_ignore_ascii_case("yes"))
    }

    pub async fn generate_goal_summary(&self, session: &ReactSession) -> Result<String> {
        let history = self.context_retriever.retrieve(session).session_history;
        let prompt = self
            .prompt_service
            .goal_summary_prompt(&session.query, &history);

        let response = self.client.generate_response(&prompt).await?;
        let summary = response.trim().to_string();
        if summary.is_empty() {
            return Ok("Root cause: Unknown\nFix applied: Unknown".to_string());
        }
        Ok(summary)
    }

    pub fn record_command_outcome(&self, query: &str, command: &ProposedCommand) {
        if !command.executed {
            return;
        }
        let _ = self.learning_service.record_command_outcome(
            query,
            &command.command,
            command.exit_code,
            command.stdout.as_deref(),
            command.stderr.as_deref(),
        );
    }
}
