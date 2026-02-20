use std::path::PathBuf;

use infrastructure::storage::{UserProfile, ProjectContext, get_config_dir};
use infrastructure::tools::coding::project_scanner;
use crate::services::task_service::{TaskService, Task, TaskStatus};
use crate::services::research_agent_service::{ResearchAgent, ResearchDepth};
use shared::types::Result;
use anyhow::anyhow;

#[derive(Debug, Clone, PartialEq)]
pub enum AssistantMode {
    Chat,
    Agent,
    Task,
    Research,
    Work,
    Coding,
}

pub struct AssistantContext {
    pub user_profile: UserProfile,
    pub project_context: Option<ProjectContext>,
    pub current_mode: AssistantMode,
    pub session_history: Vec<String>,
    pub config_dir: PathBuf,
}

impl AssistantContext {
    pub fn new() -> Self {
        let config_dir = get_config_dir();
        
        Self {
            user_profile: UserProfile::load(&config_dir),
            project_context: None,
            current_mode: AssistantMode::Chat,
            session_history: Vec::new(),
            config_dir,
        }
    }
    
    pub fn load_project_context(&mut self, project_path: &str) {
        self.project_context = Some(ProjectContext::load(&self.config_dir, project_path));
    }
    
    pub fn save(&self) -> Result<()> {
        self.user_profile.save(&self.config_dir).map_err(|e| anyhow::anyhow!(e))?;
        
        if let Some(ref ctx) = self.project_context {
            ctx.save(&self.config_dir).map_err(|e| anyhow::anyhow!(e))?;
        }
        
        Ok(())
    }
}

pub struct AssistantService {
    context: AssistantContext,
    task_service: Option<TaskService>,
    research_agent: Option<ResearchAgent>,
}

impl AssistantService {
    pub fn new() -> Self {
        Self {
            context: AssistantContext::new(),
            task_service: None,
            research_agent: None,
        }
    }
    
    pub fn with_task_service(mut self, service: TaskService) -> Self {
        self.task_service = Some(service);
        self
    }
    
    pub fn with_research_agent(mut self, agent: ResearchAgent) -> Self {
        self.research_agent = Some(agent);
        self
    }
    
    pub fn context(&self) -> &AssistantContext {
        &self.context
    }
    
    pub fn context_mut(&mut self) -> &mut AssistantContext {
        &mut self.context
    }
    
    pub fn set_mode(&mut self, mode: AssistantMode) {
        self.context.current_mode = mode;
    }
    
    pub fn update_user_profile(&mut self, name: Option<String>, email: Option<String>, preferred_language: Option<String>) {
        if let Some(n) = name {
            self.context.user_profile.name = Some(n);
        }
        if let Some(e) = email {
            self.context.user_profile.email = Some(e);
        }
        if let Some(l) = preferred_language {
            self.context.user_profile.preferred_language = Some(l);
        }
        
        let _ = self.context.save();
    }
    
    pub fn add_skill(&mut self, skill: String) {
        self.context.user_profile.set_skill(skill);
        let _ = self.context.save();
    }
    
    pub fn detect_project(&mut self, path: &str) -> project_scanner::ProjectInfo {
        let info = project_scanner::scan_project(std::path::Path::new(path));
        
        if let Some(ref mut ctx) = self.context.project_context {
            ctx.language = Some(info.language.clone());
            ctx.framework = info.framework.clone();
            ctx.test_framework = info.test_framework.clone();
            let _ = ctx.save(&self.context.config_dir);
        }
        
        info
    }
    
    pub async fn chat(&mut self, message: &str) -> Result<String> {
        self.context.session_history.push(message.to_string());
        
        Ok(format!("Echo: {}", message))
    }
    
    pub fn create_task(&mut self, title: String, description: String) -> Result<String> {
        if let Some(ref mut service) = self.task_service {
            let id = service.create_task(title, description);
            Ok(id)
        } else {
            Err(anyhow!("Task service not configured"))
        }
    }
    
    pub fn start_research(&mut self, query: String, depth: ResearchDepth) -> Result<String> {
        if let Some(ref mut agent) = self.research_agent {
            let id = agent.start_research(query, depth);
            Ok(id)
        } else {
            Err(anyhow!("Research agent not configured"))
        }
    }
    
    pub fn add_research_note(&mut self, content: String, tags: Vec<String>) -> String {
        if let Some(ref mut agent) = self.research_agent {
            agent.add_note(content, None, tags)
        } else {
            String::new()
        }
    }
    
    pub fn list_tasks(&self, status: Option<TaskStatus>) -> Vec<String> {
        if let Some(ref service) = self.task_service {
            service.list_tasks(status)
                .iter()
                .map(|t| format!("{}: {} ({:?})", t.id, t.title, t.status))
                .collect()
        } else {
            Vec::new()
        }
    }
    
    pub fn list_research(&self) -> Vec<String> {
        if let Some(ref agent) = self.research_agent {
            agent.list_queries()
                .iter()
                .map(|q| format!("{}: {} ({:?})", q.id, q.query, q.status))
                .collect()
        } else {
            Vec::new()
        }
    }
    
    pub fn get_task(&self, task_id: &str) -> Option<String> {
        self.task_service.as_ref()
            .and_then(|s| s.get_task(task_id))
            .map(|t| serde_json::to_string_pretty(t).ok())
            .flatten()
    }
}
