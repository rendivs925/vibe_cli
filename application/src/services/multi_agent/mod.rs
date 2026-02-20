pub mod agent;
pub mod consensus;
pub mod debate_manager;
pub mod generator_agent;
pub mod critic_agent;
pub mod tester_agent;

pub use agent::Agent;
pub use debate_manager::DebateManager;
pub use generator_agent::GeneratorAgent;
pub use critic_agent::CriticAgent;
pub use tester_agent::TesterAgent;
pub use consensus::Consensus;
