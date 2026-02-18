use crate::entities::react::{ProposedCommand, ReactSession, ReactStep};
use async_trait::async_trait;
use std::error::Error;
use std::result::Result;

#[async_trait]
pub trait ReactRepository: Send + Sync {
    async fn save_session(&self, session: &ReactSession) -> Result<(), Box<dyn Error + Send + Sync>>;
    async fn get_session(
        &self,
        session_id: &str,
    ) -> Result<Option<ReactSession>, Box<dyn Error + Send + Sync>>;
    async fn update_session(&self, session: &ReactSession)
        -> Result<(), Box<dyn Error + Send + Sync>>;
    async fn delete_session(&self, session_id: &str) -> Result<(), Box<dyn Error + Send + Sync>>;

    async fn save_step(&self, step: &ReactStep) -> Result<(), Box<dyn Error + Send + Sync>>;
    async fn get_steps(
        &self,
        session_id: &str,
    ) -> Result<Vec<ReactStep>, Box<dyn Error + Send + Sync>>;
    async fn update_step(&self, step: &ReactStep) -> Result<(), Box<dyn Error + Send + Sync>>;

    async fn get_recent_sessions(
        &self,
        limit: usize,
    ) -> Result<Vec<ReactSession>, Box<dyn Error + Send + Sync>>;
    async fn get_sessions_by_status(
        &self,
        status: &str,
    ) -> Result<Vec<ReactSession>, Box<dyn Error + Send + Sync>>;
}

#[async_trait]
pub trait ReactCommandRepository: Send + Sync {
    async fn save_command(&self, command: &ProposedCommand)
        -> Result<(), Box<dyn Error + Send + Sync>>;
    async fn update_command(
        &self,
        command: &ProposedCommand,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;
    async fn get_commands_by_step(
        &self,
        step_id: &str,
    ) -> Result<Vec<ProposedCommand>, Box<dyn Error + Send + Sync>>;
    async fn get_pending_commands(
        &self,
        step_id: &str,
    ) -> Result<Vec<ProposedCommand>, Box<dyn Error + Send + Sync>>;
}
