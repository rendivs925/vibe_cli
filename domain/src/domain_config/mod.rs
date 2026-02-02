// Domain configuration module for neurosymbolic reasoning
// Provides config-driven, extensible symbolic reasoning

pub mod types;
pub mod loader;
pub mod registry;
pub mod command_generator;
pub mod output_parser;
#[cfg(test)]
pub mod tests;

pub use types::*;
pub use registry::DomainRegistry;
pub use command_generator::CommandGenerator;
pub use output_parser::OutputParser;
