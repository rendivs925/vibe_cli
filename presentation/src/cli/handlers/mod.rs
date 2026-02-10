mod agent;
mod cache;
mod chat;
mod domain_init;
mod domain_learning;
mod domain_manage;
mod domain_output;
mod explain;
mod neurosymbolic_filter;
mod neurosymbolic_flow;
mod neurosymbolic_utils;
mod rag;

#[cfg(test)]
mod tests;

use super::cache::CacheManager;
use super::command_extraction::query_keywords;
use super::utils::{detect_system_info, project_cache_suffix};
use application::services::neurosymbolic_service::IntegratedNeurosymbolicService;
use application::services::rag_service::RagService;
use infrastructure::config::Config;
use shared::types::Message;
use shared::types::Result;
use std::path::PathBuf;
use std::process::{Command, ExitStatus};

pub struct CliHandlers {
    cache_manager: CacheManager,
    system_info: String,
    config: Config,
    rag_service: Option<RagService>,
    integrated_service: Option<IntegratedNeurosymbolicService>,
}

impl CliHandlers {
    pub fn new(config: Config) -> Self {
        let cache_dir = Self::default_cache_dir();
        let system_info_path = Self::default_system_info_path();
        let system_info = Self::load_or_collect_system_info(&system_info_path);

        let integrated_service = IntegratedNeurosymbolicService::new().ok();

        Self {
            cache_manager: CacheManager::new(cache_dir.clone(), false),
            system_info,
            config,
            rag_service: None,
            integrated_service,
        }
    }

    pub fn has_neurosymbolic_domains(&self) -> bool {
        self.integrated_service
            .as_ref()
            .map(|service| service.has_enabled_domains())
            .unwrap_or(false)
    }

    fn default_cache_dir() -> PathBuf {
        let mut path = Self::home_dir();
        path.push(".local");
        path.push("share");
        path.push("vibe_cli");
        let suffix = project_cache_suffix();
        path.push(suffix);
        path
    }

    fn default_system_info_path() -> PathBuf {
        let mut path = Self::home_dir();
        path.push(".config");
        path.push("vibe_cli");
        path.push("system_info.txt");
        path
    }

    fn home_dir() -> PathBuf {
        PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
    }

    fn load_or_collect_system_info(path: &PathBuf) -> String {
        if let Ok(existing) = std::fs::read_to_string(path) {
            if !existing.trim().is_empty() {
                return existing.trim().to_string();
            }
        }

        let detected = detect_system_info();

        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, &detected);

        detected
    }

    fn ensure_integrated_service(&mut self) {
        if self.integrated_service.is_none() {
            self.integrated_service = IntegratedNeurosymbolicService::new().ok();
        }
    }

    fn direct_answer(&self, query: &str) -> Option<String> {
        self.integrated_service
            .as_ref()
            .and_then(|service| service.direct_answer(query))
    }

    async fn allowed_commands_from_rag(
        &mut self,
        query: &str,
    ) -> Result<Option<std::collections::HashSet<String>>> {
        if self.rag_service.is_none() {
            let client = infrastructure::ollama_client::OllamaClient::new()?;
            self.rag_service = Some(
                RagService::new(".", &self.config.db_path, client, self.config.clone()).await?,
            );
            let keywords = query_keywords(query);
            self.rag_service
                .as_ref()
                .unwrap()
                .build_index_for_keywords(&keywords)
                .await?;
        }

        let Some(rag) = self.rag_service.as_ref() else {
            return Ok(None);
        };

        let chunks = rag.relevant_chunks(query, 30).await?;
        let mut allowed = std::collections::HashSet::new();
        for chunk in chunks {
            for cmd in super::command_extraction::extract_commands(&chunk, query) {
                allowed.insert(cmd.command);
            }
        }

        if allowed.is_empty() {
            Ok(None)
        } else {
            Ok(Some(allowed))
        }
    }

    fn user_message(query: &str, critique_feedback: Option<&str>) -> Message {
        Message {
            role: "user".to_string(),
            content: critique_feedback.unwrap_or(query).to_string(),
        }
    }

    fn run_shell_command(&self, cmd: &str) -> Result<CommandOutput> {
        let output = Command::new("bash").arg("-c").arg(cmd).output()?;
        Ok(CommandOutput::from(output))
    }

    fn keywords_from_text(text: &str) -> Vec<String> {
        text.split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
            .filter(|w| w.len() > 2)
            .map(|w| w.to_lowercase())
            .collect()
    }

    fn config_dir(&self) -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".config/vibe_cli/domains")
    }
}

struct CommandOutput {
    stdout: String,
    stderr: String,
    full_output: String,
    status: ExitStatus,
}

impl From<std::process::Output> for CommandOutput {
    fn from(output: std::process::Output) -> Self {
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let full_output = if stderr.is_empty() {
            stdout.clone()
        } else {
            format!("{}\nErrors:\n{}", stdout, stderr)
        };
        Self {
            stdout,
            stderr,
            full_output,
            status: output.status,
        }
    }
}
