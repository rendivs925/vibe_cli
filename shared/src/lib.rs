pub mod batch_processing;
pub mod confirmation;
pub mod content_sanitizer;
pub mod error;
pub mod memory_pool;
pub mod performance;
pub mod performance_monitor;
pub mod secrets_detector;
pub mod telemetry;
pub mod types;
pub mod utils;
pub mod ultra_fast_memory;
pub mod zero_copy;

/// Re-export ultra-fast memory utilities for easy access
pub use ultra_fast_memory::{UltraFastArena, StringInterner, with_thread_local_arena};
