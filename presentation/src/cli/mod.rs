//! CLI module - refactored into focused submodules
//!
//! This module is split into:
//! - agent: Agent task analysis and execution planning
//! - cache: Cache data structures
//! - utils: Utility functions

pub mod agent;
pub mod cache;
pub mod utils;

// Re-export main types for backward compatibility
pub use cache::{CommandCacheEntry, CommandCacheFile, ExplainCacheEntry, ExplainCacheFile, RagCacheEntry, RagCacheFile};
pub use agent::{analyze_agent_task, display_agent_plan, enhance_agent_plan, format_risk_level};
pub use utils::validate_command_syntax;
