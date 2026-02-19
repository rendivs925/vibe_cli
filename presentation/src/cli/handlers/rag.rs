use super::CliHandlers;
use application::services::test_time_scaling::ScalingMethod;
use shared::confirmation::ask_confirmation;
use shared::confirmation::ask_feedback;
use shared::types::Result;

impl CliHandlers {
    pub async fn handle_rag(
        &mut self,
        question: &str,
        scaling_config: &application::services::test_time_scaling::ScalingConfig,
    ) -> Result<()> {
        if let Some(cached_response) = self.cache_manager.load_rag_cached(question)? {
            if ask_confirmation("Cached answer found. Use it?", true)? {
                println!("{}", cached_response);
                return Ok(());
            }
        }

        if scaling_config.method != ScalingMethod::None && self.rag_service.is_none() {
            eprintln!("Initializing RAG service with scaling...");
            let client = infrastructure::ollama_client::OllamaClient::new()?;
            self.rag_service = Some(
                application::services::rag_service::RagService::new(
                    ".",
                    &self.config.db_path,
                    client,
                    self.config.clone(),
                )
                .await?,
            );
            let keywords = Self::keywords_from_text(question);
            if let Some(rag) = self.rag_service.as_ref() {
                rag.build_index_for_keywords(&keywords).await?;
            }
        }

        if scaling_config.method != ScalingMethod::None {
            return self
                .handle_rag_with_scaling(question, scaling_config)
                .await;
        }

        self.handle_rag_standard(question).await
    }

    async fn handle_rag_with_scaling(
        &mut self,
        question: &str,
        scaling_config: &application::services::test_time_scaling::ScalingConfig,
    ) -> Result<()> {
        if self.rag_service.is_none() {
            let client = infrastructure::ollama_client::OllamaClient::new()?;
            self.rag_service = Some(
                application::services::rag_service::RagService::new(
                    ".",
                    &self.config.db_path,
                    client,
                    self.config.clone(),
                )
                .await?,
            );
            let keywords = Self::keywords_from_text(question);
            if let Some(rag) = self.rag_service.as_ref() {
                rag.build_index_for_keywords(&keywords).await?;
            }
        }

        let rag = self.rag_service.as_ref().ok_or_else(|| {
            anyhow::anyhow!("RAG service not initialized")
        })?;

        let relevant_chunks = rag.relevant_chunks(question, 20).await?;
        let context = relevant_chunks.join("\n\n");

        let prompt_template = format!(
            "Based on the following code context, answer the question thoroughly.\n\nContext:\n{}\n\nQuestion: {}\n\nProvide a detailed and accurate answer:",
            context, question
        );

        eprintln!(
            "Generating {} answers via {}...",
            scaling_config.num_samples,
            match scaling_config.method {
                ScalingMethod::Knockout => "knockout tournament",
                ScalingMethod::League => "league competition",
                ScalingMethod::None => "none",
            }
        );

        let best_response = self
            .select_best_response_with_scaling(&prompt_template, scaling_config)
            .await;

        let response = if let Some(r) = best_response {
            r
        } else {
            rag.query_with_feedback(question, "").await?
        };

        if ask_confirmation("Satisfied with this response?", true)? {
            self.cache_manager
                .save_rag_cached(question, &response)?;
        }

        println!("{}", response);
        Ok(())
    }

    async fn handle_rag_standard(&mut self, question: &str) -> Result<()> {
        if self.rag_service.is_none() {
            eprintln!("Analyzing query and scanning codebase...");
            let client = infrastructure::ollama_client::OllamaClient::new()?;
            self.rag_service = Some(
                application::services::rag_service::RagService::new(
                    ".",
                    &self.config.db_path,
                    client,
                    self.config.clone(),
                )
                .await?,
            );
            let keywords = Self::keywords_from_text(question);
            if let Some(rag) = self.rag_service.as_ref() {
                rag.build_index_for_keywords(&keywords).await?;
            }
        }

        let mut feedback = String::new();
        loop {
            eprintln!("Thinking...");
            let Some(rag) = self.rag_service.as_ref() else {
                return Err(anyhow::anyhow!("RAG service not initialized"));
            };
            let response = rag.query_with_feedback(question, &feedback).await?;

            println!("{}", response);

            if ask_confirmation("Satisfied with this response?", true)? {
                self.cache_manager
                    .save_rag_cached(question, &response)?;
                break;
            } else {
                feedback.clear();
                feedback = ask_feedback("Provide feedback for improvement: ")?;
                eprintln!("Regenerating with feedback...");
            }
        }

        Ok(())
    }

    pub async fn handle_context(&mut self, path: &str) -> Result<()> {
        eprintln!("Loading context from {}...", path);
        let client = infrastructure::ollama_client::OllamaClient::new()?;
        self.rag_service = Some(
            application::services::rag_service::RagService::new(
                path,
                &self.config.db_path,
                client,
                self.config.clone(),
            )
            .await?,
        );
        if let Some(rag) = self.rag_service.as_ref() {
            rag.build_index().await?;
        }
        eprintln!("Context loaded from {}", path);
        self.handle_chat().await
    }
}
