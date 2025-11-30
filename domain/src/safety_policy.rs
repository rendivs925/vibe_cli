use shared::types::Result;

#[derive(Debug, Clone)]
pub struct SafetyPolicy {
    pub rules: Vec<String>,
}

impl SafetyPolicy {
    pub fn new() -> Self {
        Self {
            rules: vec![
                "No file system writes".to_string(),
                "No network access".to_string(),
                "No system commands".to_string(),
            ],
        }
    }

    pub fn validate(&self, plan: &super::command_plan::CommandPlan) -> Result<()> {
        for check in &plan.safety_checks {
            if !check.passed {
                return Err(anyhow::anyhow!("Safety check failed: {}", check.check_type));
            }
        }
        Ok(())
    }
}