//! Formal Query Language (FQL) for neurosymbolic reasoning
//!
//! FQL provides a structured intermediate representation that decouples
//! intent understanding from command syntax generation.
//!
//! Example:
//! - NL: "clean old logs in /var/log safely"
//! - FQL: ACTION(delete) & TARGET(path:/var/log) & PATTERN(older_than:7d) & CONSTRAINT(safe_delete)

pub mod parser;
pub mod types;

// Re-export main types
pub use parser::FqlParser;
pub use types::*;
