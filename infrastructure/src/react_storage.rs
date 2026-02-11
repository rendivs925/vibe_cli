use async_trait::async_trait;
use domain::entities::react::{ProposedCommand, ReactSession, ReactStatus, ReactStep};
use domain::repositories::react_repository::{ReactCommandRepository, ReactRepository};
use std::collections::HashMap;
use std::error::Error;
use std::sync::Mutex;

pub struct InMemoryReactStorage {
    sessions: Mutex<HashMap<String, ReactSession>>,
    steps_by_session: Mutex<HashMap<String, Vec<ReactStep>>>,
    steps_by_id: Mutex<HashMap<String, ReactStep>>,
    commands_by_step: Mutex<HashMap<String, Vec<ProposedCommand>>>,
    commands_by_id: Mutex<HashMap<String, ProposedCommand>>,
}

impl InMemoryReactStorage {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            steps_by_session: Mutex::new(HashMap::new()),
            steps_by_id: Mutex::new(HashMap::new()),
            commands_by_step: Mutex::new(HashMap::new()),
            commands_by_id: Mutex::new(HashMap::new()),
        }
    }

    fn status_matches(status: &str, candidate: &ReactStatus) -> bool {
        let normalized = status.trim().to_ascii_lowercase();
        match candidate {
            ReactStatus::Running => normalized == "running",
            ReactStatus::Completed => normalized == "completed",
            ReactStatus::Failed => normalized == "failed",
            ReactStatus::Aborted => normalized == "aborted",
        }
    }

    fn update_step_in_session(steps: &mut Vec<ReactStep>, step: &ReactStep) {
        if let Some(existing) = steps.iter_mut().find(|s| s.id == step.id) {
            *existing = step.clone();
        } else {
            steps.push(step.clone());
        }
    }

    fn update_command_in_step(commands: &mut Vec<ProposedCommand>, command: &ProposedCommand) {
        if let Some(existing) = commands.iter_mut().find(|c| c.id == command.id) {
            *existing = command.clone();
        } else {
            commands.push(command.clone());
        }
    }
}

#[async_trait]
impl ReactRepository for InMemoryReactStorage {
    async fn save_session(&self, session: &ReactSession) -> Result<(), Box<dyn Error>> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| "failed to lock sessions")?;
        sessions.insert(session.id.clone(), session.clone());
        Ok(())
    }

    async fn get_session(&self, session_id: &str) -> Result<Option<ReactSession>, Box<dyn Error>> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| "failed to lock sessions")?;
        Ok(sessions.get(session_id).cloned())
    }

    async fn update_session(&self, session: &ReactSession) -> Result<(), Box<dyn Error>> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| "failed to lock sessions")?;
        sessions.insert(session.id.clone(), session.clone());
        Ok(())
    }

    async fn delete_session(&self, session_id: &str) -> Result<(), Box<dyn Error>> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| "failed to lock sessions")?;
        sessions.remove(session_id);
        Ok(())
    }

    async fn save_step(&self, step: &ReactStep) -> Result<(), Box<dyn Error>> {
        let mut steps_by_session = self
            .steps_by_session
            .lock()
            .map_err(|_| "failed to lock steps_by_session")?;
        let mut steps_by_id = self
            .steps_by_id
            .lock()
            .map_err(|_| "failed to lock steps_by_id")?;

        let steps = steps_by_session
            .entry(step.session_id.clone())
            .or_insert_with(Vec::new);
        Self::update_step_in_session(steps, step);
        steps_by_id.insert(step.id.clone(), step.clone());
        Ok(())
    }

    async fn get_steps(&self, session_id: &str) -> Result<Vec<ReactStep>, Box<dyn Error>> {
        let steps_by_session = self
            .steps_by_session
            .lock()
            .map_err(|_| "failed to lock steps_by_session")?;
        Ok(steps_by_session
            .get(session_id)
            .cloned()
            .unwrap_or_default())
    }

    async fn update_step(&self, step: &ReactStep) -> Result<(), Box<dyn Error>> {
        self.save_step(step).await
    }

    async fn get_recent_sessions(&self, limit: usize) -> Result<Vec<ReactSession>, Box<dyn Error>> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| "failed to lock sessions")?;
        let mut items: Vec<_> = sessions.values().cloned().collect();
        items.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        items.truncate(limit);
        Ok(items)
    }

    async fn get_sessions_by_status(
        &self,
        status: &str,
    ) -> Result<Vec<ReactSession>, Box<dyn Error>> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| "failed to lock sessions")?;
        Ok(sessions
            .values()
            .filter(|session| Self::status_matches(status, &session.status))
            .cloned()
            .collect())
    }
}

#[async_trait]
impl ReactCommandRepository for InMemoryReactStorage {
    async fn save_command(&self, command: &ProposedCommand) -> Result<(), Box<dyn Error>> {
        let mut commands_by_id = self
            .commands_by_id
            .lock()
            .map_err(|_| "failed to lock commands_by_id")?;
        commands_by_id.insert(command.id.clone(), command.clone());
        Ok(())
    }

    async fn update_command(&self, command: &ProposedCommand) -> Result<(), Box<dyn Error>> {
        self.save_command(command).await
    }

    async fn get_commands_by_step(
        &self,
        step_id: &str,
    ) -> Result<Vec<ProposedCommand>, Box<dyn Error>> {
        let commands_by_step = self
            .commands_by_step
            .lock()
            .map_err(|_| "failed to lock commands_by_step")?;
        Ok(commands_by_step.get(step_id).cloned().unwrap_or_default())
    }

    async fn get_pending_commands(
        &self,
        step_id: &str,
    ) -> Result<Vec<ProposedCommand>, Box<dyn Error>> {
        let commands_by_step = self
            .commands_by_step
            .lock()
            .map_err(|_| "failed to lock commands_by_step")?;
        Ok(commands_by_step
            .get(step_id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|cmd| cmd.approved.is_none() && !cmd.executed)
            .collect())
    }
}

impl InMemoryReactStorage {
    pub fn attach_command_to_step(&self, step_id: &str, command: &ProposedCommand) {
        if let Ok(mut commands_by_step) = self.commands_by_step.lock() {
            let commands = commands_by_step
                .entry(step_id.to_string())
                .or_insert_with(Vec::new);
            Self::update_command_in_step(commands, command);
        }
        if let Ok(mut commands_by_id) = self.commands_by_id.lock() {
            commands_by_id.insert(command.id.clone(), command.clone());
        }
    }
}
