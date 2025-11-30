use domain::safety_policy::SafetyPolicy;
use shared::types::Result;

pub struct SafetyService {
    policy: SafetyPolicy,
}

impl SafetyService {
    pub fn new() -> Self {
        Self {
            policy: SafetyPolicy::new(),
        }
    }

    pub fn validate(&self, plan: &domain::command_plan::CommandPlan) -> Result<()> {
        self.policy.validate(plan)
    }
}
