// Clean Architecture Domain Layer
// 
// This module contains the core business logic and entities of Vibe CLI application.
// It has no dependencies on external frameworks or infrastructure concerns.

pub mod entities;
pub mod value_objects;
pub mod services;
pub mod repositories;

// Re-export main types for easier imports
pub use entities::*;
pub use value_objects::*;
pub use services::*;
pub use repositories::*;

// Additional re-exports for specific types
pub use repositories::embedding_repository::EmbeddingStats;
pub use services::command_planner::CommandPlanResult;
