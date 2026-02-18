use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskOrchestration {
    pub primary_goal: String,
    pub step: u32,
    pub total_steps: u32,
    pub task_type: String,
    pub final_command: String,
}

impl TaskOrchestration {
    pub fn new(goal: &str, step: u32, total_steps: u32) -> Self {
        Self {
            primary_goal: goal.to_string(),
            step,
            total_steps,
            task_type: "Analyze".to_string(),
            final_command: "Execute the next action.".to_string(),
        }
    }

    pub fn with_type(mut self, task_type: &str) -> Self {
        if !task_type.trim().is_empty() {
            self.task_type = task_type.to_string();
        }
        self
    }

    pub fn with_final_command(mut self, command: &str) -> Self {
        if !command.trim().is_empty() {
            self.final_command = command.to_string();
        }
        self
    }

    pub fn to_markdown(&self) -> String {
        format!(
            "### ## TASK_ORCHESTRATION\n**PRIMARY_GOAL**: {}\n**STEP**: {} / {}\n**TASK_TYPE**: {}\n**FINAL_COMMAND**: \"{}\"\n\n",
            self.primary_goal,
            self.step,
            self.total_steps,
            self.task_type,
            self.final_command
        )
    }
}
