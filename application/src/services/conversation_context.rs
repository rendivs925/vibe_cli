use domain::entities::react::{ConversationEntry, ConversationEntryType, ReactSession};

const MAX_CONVERSATION_ENTRIES: usize = 20;

pub struct ConversationContextManager {
    max_entries: usize,
}

impl ConversationContextManager {
    pub fn new() -> Self {
        Self {
            max_entries: MAX_CONVERSATION_ENTRIES,
        }
    }

    pub fn with_max_entries(mut self, max: usize) -> Self {
        self.max_entries = max;
        self
    }

    pub fn format_conversation_for_prompt(&self, session: &ReactSession) -> String {
        if session.conversation_history.is_empty() {
            return "(no conversation history)".to_string();
        }

        let mut lines = Vec::new();
        lines.push("## Conversation History".to_string());
        lines.push(String::new());

        for entry in &session.conversation_history {
            let label = match entry.entry_type {
                ConversationEntryType::UserQuery => "USER",
                ConversationEntryType::ToolExecution => "TOOL",
                ConversationEntryType::ToolOutput => "OUTPUT",
                ConversationEntryType::AiSummary => "AI",
            };

            let tool_info = entry
                .tool_name
                .as_ref()
                .map(|t| format!(" [{}]", t))
                .unwrap_or_default();

            let content_preview = if entry.content.len() > 200 {
                format!("{}...", &entry.content[..200])
            } else {
                entry.content.clone()
            };

            lines.push(format!("{label}{tool_info}: {content_preview}"));
        }

        lines.join("\n")
    }

    pub fn format_compact_conversation(&self, session: &ReactSession) -> String {
        if session.conversation_history.is_empty() {
            return "(no conversation history)".to_string();
        }

        summarize_entries(&session.conversation_history)
    }

    pub fn trim_old_entries(&self, session: &mut ReactSession) {
        if session.conversation_history.len() > self.max_entries {
            let to_remove = session.conversation_history.len() - self.max_entries;
            session.conversation_history = session
                .conversation_history
                .into_iter()
                .skip(to_remove)
                .collect();
        }
    }

    pub fn get_recent_entries(
        &self,
        session: &ReactSession,
        count: usize,
    ) -> Vec<&ConversationEntry> {
        session
            .conversation_history
            .iter()
            .rev()
            .take(count)
            .collect()
    }

    pub fn compact_if_needed(&self, session: &mut ReactSession) -> bool {
        if session.conversation_history.len() <= self.max_entries {
            return false;
        }

        let to_compact = session.conversation_history.len() - self.max_entries;
        let (older, recent) = session.conversation_history.split_at(to_compact);
        let summary = summarize_entries(older);

        let mut updated = Vec::new();
        if !summary.trim().is_empty() {
            updated.push(ConversationEntry::ai_summary(summary));
        }
        updated.extend(recent.iter().cloned());
        session.conversation_history = updated;
        true
    }
}

impl Default for ConversationContextManager {
    fn default() -> Self {
        Self::new()
    }
}

fn summarize_entries(entries: &[ConversationEntry]) -> String {
    if entries.is_empty() {
        return "(no conversation)".to_string();
    }

    let mut sentences = Vec::new();
    let mut user_queries = Vec::new();
    let mut tool_names = Vec::new();
    let mut outputs = Vec::new();
    let mut summaries = Vec::new();

    for entry in entries {
        match entry.entry_type {
            ConversationEntryType::UserQuery => user_queries.push(entry.content.clone()),
            ConversationEntryType::ToolExecution => {
                if let Some(tool) = entry.tool_name.clone() {
                    tool_names.push(tool);
                }
            }
            ConversationEntryType::ToolOutput => outputs.push(entry.content.clone()),
            ConversationEntryType::AiSummary => summaries.push(entry.content.clone()),
        }
    }

    if !user_queries.is_empty() {
        let picks = user_queries.into_iter().rev().take(3).collect::<Vec<_>>();
        sentences.push(format!(
            "User asked about: {}.",
            picks.into_iter().rev().collect::<Vec<_>>().join("; ")
        ));
    }

    if !tool_names.is_empty() {
        tool_names.sort();
        tool_names.dedup();
        let tools = tool_names.into_iter().take(4).collect::<Vec<_>>();
        sentences.push(format!("Tools used: {}.", tools.join(", ")));
    }

    if !outputs.is_empty() {
        let preview = outputs
            .into_iter()
            .rev()
            .take(2)
            .map(|text| {
                let line = text.lines().next().unwrap_or("").trim();
                truncate_chars(line, 120)
            })
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>();
        if !preview.is_empty() {
            sentences.push(format!("Key outputs: {}.", preview.join(" | ")));
        }
    }

    if !summaries.is_empty() {
        let recent = summaries.into_iter().rev().next().unwrap_or_default();
        if !recent.trim().is_empty() {
            sentences.push(format!("Previous summary noted: {}.", trim_sentence(&recent, 160)));
        }
    }

    if sentences.len() < 3 {
        sentences.push(format!("Conversation included {} entries.", entries.len()));
    }
    while sentences.len() < 3 {
        sentences.push("Older context was compacted to preserve the most relevant details."
            .to_string());
    }

    if sentences.len() > 5 {
        sentences.truncate(5);
    }

    sentences
        .into_iter()
        .map(|sentence| trim_sentence(&sentence, 200))
        .collect::<Vec<_>>()
        .join(" ")
}

fn trim_sentence(text: &str, max_len: usize) -> String {
    let trimmed = text.trim();
    truncate_chars(trimmed, max_len)
}

fn truncate_chars(text: &str, max_len: usize) -> String {
    let trimmed = text.trim();
    let mut chars = trimmed.chars();
    let mut collected = String::new();
    for _ in 0..max_len {
        match chars.next() {
            Some(ch) => collected.push(ch),
            None => return trimmed.to_string(),
        }
    }
    collected.push_str("...");
    collected
}
