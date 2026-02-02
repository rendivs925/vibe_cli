// Domain configuration module for neurosymbolic reasoning
// Provides config-driven, extensible symbolic reasoning

pub mod command_generator;
pub mod loader;
pub mod output_parser;
pub mod registry;
#[cfg(test)]
pub mod tests;
pub mod types;

pub use command_generator::CommandGenerator;
pub use output_parser::OutputParser;
pub use registry::DomainRegistry;
pub use types::*;
