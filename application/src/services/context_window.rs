use std::env;

#[derive(Debug, Clone, Copy)]
pub struct ContextWindowPolicy {
    pub max_tokens: usize,
    pub reserve_tokens: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct ContextWindowUsage {
    pub estimated_tokens: usize,
    pub max_tokens: usize,
    pub compact_at_tokens: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct ContextWindowStatus {
    pub usage: ContextWindowUsage,
    pub compacted: bool,
}

impl ContextWindowPolicy {
    pub fn from_env() -> Self {
        let max_tokens = read_env_usize("VIBE_CONTEXT_WINDOW_TOKENS")
            .unwrap_or(DEFAULT_CONTEXT_WINDOW_TOKENS);
        let reserve_tokens = read_env_usize("VIBE_CONTEXT_WINDOW_RESERVE")
            .unwrap_or(DEFAULT_CONTEXT_WINDOW_RESERVE);
        Self {
            max_tokens: max_tokens.max(1024),
            reserve_tokens: reserve_tokens.min(max_tokens.saturating_sub(256)),
        }
    }

    pub fn compact_at_tokens(&self) -> usize {
        self.max_tokens.saturating_sub(self.reserve_tokens)
    }

    pub fn estimate_tokens(&self, text: &str) -> usize {
        estimate_tokens(text)
    }

    pub fn usage(&self, text: &str) -> ContextWindowUsage {
        ContextWindowUsage {
            estimated_tokens: self.estimate_tokens(text),
            max_tokens: self.max_tokens,
            compact_at_tokens: self.compact_at_tokens(),
        }
    }

    pub fn should_compact(&self, text: &str) -> bool {
        let usage = self.usage(text);
        usage.should_compact()
    }
}

impl Default for ContextWindowPolicy {
    fn default() -> Self {
        Self {
            max_tokens: DEFAULT_CONTEXT_WINDOW_TOKENS,
            reserve_tokens: DEFAULT_CONTEXT_WINDOW_RESERVE,
        }
    }
}

impl ContextWindowUsage {
    pub fn utilization(&self) -> f32 {
        if self.max_tokens == 0 {
            0.0
        } else {
            self.estimated_tokens as f32 / self.max_tokens as f32
        }
    }

    pub fn should_compact(&self) -> bool {
        self.estimated_tokens >= self.compact_at_tokens
    }
}

fn estimate_tokens(text: &str) -> usize {
    let char_count = text.chars().count();
    let approx_by_chars = (char_count + 3) / 4;
    let approx_by_words = text.split_whitespace().count();
    approx_by_chars.max(approx_by_words)
}

fn read_env_usize(key: &str) -> Option<usize> {
    env::var(key).ok().and_then(|value| value.parse::<usize>().ok())
}

const DEFAULT_CONTEXT_WINDOW_TOKENS: usize = 8192;
const DEFAULT_CONTEXT_WINDOW_RESERVE: usize = 1024;
