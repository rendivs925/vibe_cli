// Clean Architecture Domain Layer
//
// This module contains the core business logic and entities of Vibe CLI application.
// It has no dependencies on external frameworks or infrastructure concerns.

#![allow(ambiguous_glob_reexports)]

pub mod domain_config;
pub mod entities;
pub mod repositories;
pub mod safety;
pub mod services;
pub mod tools;
pub mod value_objects;

// Re-export main types for easier imports
pub use domain_config::*;
pub use entities::*;
pub use repositories::*;
pub use safety::*;
pub use tools::*;
pub use value_objects::*;

// Additional re-exports for specific types
pub use repositories::embedding_repository::EmbeddingStats;
pub use services::command_planner::CommandPlanResult;
