use super::neurosymbolic_utils::{is_disallowed_by_learning, normalize_command, normalize_set};
use super::CliHandlers;
use crate::cli::cache::CommandCandidate;
use application::services::neurosymbolic_service::{
    DomainCommandValidation, SymbolicCommandSuggestion,
};
use colored::Colorize;
use shared::confirmation::{ask_confirmation, ask_selection};
use shared::types::{Message, Result};
use std::collections::HashSet;

impl CliHandlers {
    pub(crate) fn build_learning_context_message(&self, query: &str) -> Option<Message> {
        let service = self.neurosymbolic_service.as_ref()?;
        let context = service.learning_context(query).ok().flatten();
        let failed = service
            .failed_commands_for_query(query, 6)
            .ok()
            .unwrap_or_default();

        let has_context = context
            .as_deref()
            .map(|ctx| !ctx.trim().is_empty())
            .unwrap_or(false);

        if !has_context && failed.is_empty() {
            return None;
        }

        let mut content = String::new();
        if let Some(ctx) = context {
            if !ctx.trim().is_empty() {
                content.push_str(ctx.trim());
                content.push('\n');
            }
        }

        if !failed.is_empty() {
            content.push_str("Previously failed commands:\n");
            for cmd in failed {
                content.push_str(&format!("- {}\n", cmd));
            }
        }

        Some(Message {
            role: "system".to_string(),
            content: content.trim().to_string(),
        })
    }

    pub(crate) fn filter_candidates_by_domain(
        &self,
        query: &str,
        candidates: Vec<CommandCandidate>,
        allowed_commands: Option<&HashSet<String>>,
    ) -> (
        Vec<CommandCandidate>,
        bool,
        Option<DomainCommandValidation>,
        Option<SymbolicCommandSuggestion>,
    ) {
        let Some(service) = self.neurosymbolic_service.as_ref() else {
            return (candidates, false, None, None);
        };

        let mut suggestion = service.suggest_commands_from_domains(query);
        if suggestion
            .as_ref()
            .map(|s| s.confidence < 0.8)
            .unwrap_or(false)
        {
            suggestion = None;
        }
        let allowed_normalized = allowed_commands.map(normalize_set);

        if let Some(allowed) = &allowed_normalized {
            if let Some(ref mut suggestion_ref) = suggestion {
                suggestion_ref
                    .commands
                    .retain(|cmd| allowed.contains(&normalize_command(cmd)));
                if suggestion_ref.commands.is_empty() {
                    suggestion = None;
                }
            }
        }

        let has_symbolic = suggestion.is_some();
        let failed_commands = service
            .failed_commands_for_query(query, 10)
            .unwrap_or_default();

        let mut validation: Option<DomainCommandValidation> = None;
        let mut valid_candidates = Vec::new();

        for candidate in candidates {
            let command = candidate.command.trim();
            if command.is_empty() {
                continue;
            }

            if is_disallowed_by_learning(command, &failed_commands) {
                continue;
            }

            if let Some(allowed) = &allowed_normalized {
                if !allowed.contains(&normalize_command(command)) {
                    continue;
                }
            }

            if let Some(ref suggestion) = suggestion {
                let result = service.validate_command_against_suggestion(command, suggestion);
                if result.is_valid {
                    valid_candidates.push(candidate);
                } else if validation.is_none() {
                    validation = Some(result);
                }
            } else {
                valid_candidates.push(candidate);
            }
        }

        if validation.is_none() && has_symbolic {
            validation = Some(DomainCommandValidation {
                is_valid: false,
                reason: Some("no candidate commands matched symbolic operation".to_string()),
                suggestion: suggestion.clone(),
            });
        }

        (valid_candidates, has_symbolic, validation, suggestion)
    }

    pub(crate) fn select_symbolic_command(
        &self,
        suggestion: &SymbolicCommandSuggestion,
        query: &str,
    ) -> Result<Option<String>> {
        if suggestion.commands.is_empty() {
            return Ok(None);
        }

        println!(
            "{}",
            format!(
                "Symbolic match: {} ({:.0}% confidence)",
                suggestion.op_name,
                suggestion.confidence * 100.0
            )
            .green()
            .bold()
        );

        if suggestion.commands.len() == 1 {
            let command = suggestion.commands[0].clone();
            let prompt = format!("Run this command for \"{}\"?", query);
            if ask_confirmation(&prompt, true)? {
                return Ok(Some(command));
            }
            return Ok(None);
        }

        println!("Symbolic command options for \"{}\":", query);
        for (idx, cmd) in suggestion.commands.iter().enumerate() {
            println!("  [{}] {}", idx + 1, cmd);
        }

        let options = suggestion.commands.clone();
        match ask_selection(&options, false) {
            Ok(Some(index)) => Ok(Some(options[index].clone())),
            Ok(None) => Ok(None),
            Err(_) => Ok(None),
        }
    }

    pub(crate) fn build_domain_critique_prompt(
        &self,
        query: &str,
        validation: &DomainCommandValidation,
    ) -> String {
        let mut prompt = format!(
            "The previous command does not match the symbolic domain for: \"{}\".",
            query
        );

        if let Some(reason) = validation.reason.as_deref() {
            prompt.push_str(&format!(" Reason: {}.", reason));
        }

        if let Some(suggestion) = validation.suggestion.as_ref() {
            let commands = suggestion.commands.join("\n- ");
            prompt.push_str(&format!(
                "\nUse the '{}' operation and respond with ONE of these commands:\n- {}",
                suggestion.op_name, commands
            ));
        } else {
            prompt.push_str("\nPlease provide a safe, valid command for this query.");
        }

        prompt
    }
}
