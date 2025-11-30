use serde::{Deserialize, Serialize};
use shared::types::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandPlan {
    pub id: String,
    pub description: String,
    pub steps: Vec<String>,
    pub safety_checks: Vec<SafetyCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyCheck {
    pub check_type: String,
    pub passed: bool,
}

pub trait CommandPlanner {
    fn plan_command(&self, input: &str) -> impl std::future::Future<Output = Result<CommandPlan>> + Send;
}