//! Cron Task Management Feature
//!
//! High-level cron task management built on the unified built-in supervisor.
//!
//! Dependencies:
//! - atoms::supervisor: SchedulerAtom via SupervisorManager
//! - atoms::template: TemplateAtom

use crate::atoms::template::{TemplateAtom, TemplateContext, TemplateEngine};
use crate::atoms::{CronTask, SchedulerAtom, SupervisorManager};
use crate::error::{Error, Result};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::path::PathBuf;

// ========================================
// Data Structures
// ========================================

/// Crontab task configuration for creation/update
#[derive(Debug, Clone)]
pub struct TaskConfig {
    /// Task ID (optional for creation, auto-generated)
    pub id: Option<String>,
    /// Task description
    pub description: String,
    /// Cron expression (e.g., "0 9 * * *" or "@daily")
    pub expression: String,
    /// Command to execute
    pub command: String,
    /// Task-level environment variables
    pub env: HashMap<String, String>,
    /// Whether task is enabled
    pub enabled: bool,
    /// Optional: Template name to use
    pub template: Option<String>,
    /// Template variables (if using template)
    pub variables: TemplateContext,
}

/// Crontab task information (for display)
#[derive(Debug, Clone)]
pub struct TaskInfo {
    /// Task ID
    pub id: String,
    /// Task description
    pub description: String,
    /// Cron expression
    pub expression: String,
    /// Command to execute
    pub command: String,
    /// Task-level environment variables
    pub env: HashMap<String, String>,
    /// Whether task is enabled
    pub enabled: bool,
    /// Next execution time (if calculable)
    pub next_run: Option<DateTime<Utc>>,
}

impl From<CronTask> for TaskInfo {
    fn from(task: CronTask) -> Self {
        Self {
            id: task.id.unwrap_or_default(),
            description: task.description,
            expression: task.expression,
            command: task.command,
            env: task.env,
            enabled: task.enabled,
            next_run: None,
        }
    }
}

// ========================================
// CrontabTaskManager
// ========================================

/// High-level cron task management
pub struct CrontabTaskManager {
    scheduler: SupervisorManager,
    template: TemplateEngine,
    #[allow(dead_code)]
    config_dir: PathBuf,
}

impl CrontabTaskManager {
    /// Create new manager with custom config directory
    pub fn new(config_dir: PathBuf) -> Result<Self> {
        let template_dir = config_dir.join("managed").join("templates");
        let supervisor_dir = config_dir.join("managed").join("supervisor");

        let template = TemplateEngine::new(template_dir)?;
        let scheduler = SupervisorManager::new(supervisor_dir, true);

        Ok(Self {
            scheduler,
            template,
            config_dir,
        })
    }

    /// Create manager with default config directory (~/.config/svcmgr)
    pub fn default_config() -> Result<Self> {
        let home = std::env::var("HOME").map_err(|_| Error::InvalidConfig {
            reason: "HOME environment variable not set".to_string(),
        })?;
        let config_dir = PathBuf::from(home).join(".config").join("svcmgr");
        Self::new(config_dir)
    }

    // ========================================
    // Task CRUD Operations
    // ========================================

    /// Create a new crontab task
    pub fn create_task(&self, config: &TaskConfig) -> Result<String> {
        self.scheduler.validate_expression(&config.expression)?;

        let command = if let Some(template_name) = &config.template {
            self.render_command(template_name, &config.variables)?
        } else {
            config.command.clone()
        };

        let task = CronTask {
            id: config.id.clone(),
            description: config.description.clone(),
            expression: config.expression.clone(),
            command,
            env: config.env.clone(),
            enabled: config.enabled,
        };

        let task_id = self.scheduler.add(&task)?;

        Ok(task_id)
    }

    /// List all managed crontab tasks
    pub fn list_tasks(&self) -> Result<Vec<TaskInfo>> {
        let tasks = self.scheduler.list()?;

        let mut task_infos = Vec::new();
        for task in tasks {
            let mut info = TaskInfo::from(task.clone());

            if let Ok(next_runs) = self.scheduler.next_runs(&info.id, 1) {
                info.next_run = next_runs.first().copied();
            }

            task_infos.push(info);
        }

        Ok(task_infos)
    }

    /// Get a specific task by ID
    pub fn get_task(&self, task_id: &str) -> Result<TaskInfo> {
        let task = self.scheduler.get(task_id)?;
        let mut info = TaskInfo::from(task);

        if let Ok(next_runs) = self.scheduler.next_runs(task_id, 1) {
            info.next_run = next_runs.first().copied();
        }

        Ok(info)
    }

    /// Update an existing task
    pub fn update_task(&self, task_id: &str, config: &TaskConfig) -> Result<()> {
        self.scheduler.validate_expression(&config.expression)?;

        let command = if let Some(template_name) = &config.template {
            self.render_command(template_name, &config.variables)?
        } else {
            config.command.clone()
        };

        let task = CronTask {
            id: Some(task_id.to_string()),
            description: config.description.clone(),
            expression: config.expression.clone(),
            command,
            env: config.env.clone(),
            enabled: config.enabled,
        };

        self.scheduler.update(task_id, &task)?;

        Ok(())
    }

    /// Delete a task
    pub fn delete_task(&self, task_id: &str) -> Result<()> {
        self.scheduler.remove(task_id)
    }

    // ========================================
    // Task Scheduling Operations
    // ========================================

    /// Get next N execution times for a task
    pub fn get_next_runs(&self, task_id: &str, count: usize) -> Result<Vec<DateTime<Utc>>> {
        self.scheduler.next_runs(task_id, count)
    }

    /// Validate a cron expression
    pub fn validate_expression(&self, expression: &str) -> Result<bool> {
        self.scheduler.validate_expression(expression)
    }

    // ========================================
    // Environment Variable Management
    // ========================================

    /// Set a global crontab environment variable
    pub fn set_env(&self, key: &str, value: &str) -> Result<()> {
        self.scheduler.set_env(key, value)
    }

    /// Get all global crontab environment variables
    pub fn get_env(&self) -> Result<HashMap<String, String>> {
        self.scheduler.get_env()
    }

    // ========================================
    // Template Management
    // ========================================

    /// List available crontab templates
    #[allow(dead_code)]
    pub fn list_templates(&self) -> Result<Vec<String>> {
        let templates = self.template.list_templates(Some("crontab"))?;
        Ok(templates.into_iter().map(|t| t.name).collect())
    }

    /// Get template content
    #[allow(dead_code)]
    pub fn get_template(&self, name: &str) -> Result<String> {
        Ok(self.template.get_template(name)?)
    }

    /// Validate template with context
    #[allow(dead_code)]
    pub fn validate_template(&self, name: &str) -> Result<bool> {
        let result = self.template.validate(name)?;
        Ok(result.valid)
    }

    // ========================================
    // Helper Methods
    // ========================================

    /// Render command from template
    fn render_command(&self, template_name: &str, context: &TemplateContext) -> Result<String> {
        let rendered = self.template.render(template_name, context)?;
        Ok(rendered.trim().to_string())
    }
}

impl Default for CrontabTaskManager {
    fn default() -> Self {
        Self::default_config().expect("Failed to create default CrontabTaskManager")
    }
}

// ========================================
// Unit Tests
// ========================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_manager() {
        let temp_dir = std::env::temp_dir().join("svcmgr_test_crontab");
        std::fs::create_dir_all(&temp_dir).unwrap();

        let manager = CrontabTaskManager::new(temp_dir.clone());
        assert!(manager.is_ok());

        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_validate_cron_expression() {
        let manager = CrontabTaskManager::default();

        assert!(manager.validate_expression("0 9 * * *").is_ok());
        assert!(manager.validate_expression("*/5 * * * *").is_ok());
        assert!(manager.validate_expression("@daily").is_ok());
        assert!(manager.validate_expression("@hourly").is_ok());

        assert!(manager.validate_expression("invalid").is_err());
        assert!(manager.validate_expression("60 * * * *").is_err());
    }

    #[test]
    fn test_task_info_from_cron_task() {
        let mut env = HashMap::new();
        env.insert("PATH".to_string(), "/usr/bin".to_string());

        let task = CronTask {
            id: Some("test123".to_string()),
            description: "Test task".to_string(),
            expression: "0 9 * * *".to_string(),
            command: "echo hello".to_string(),
            env: env.clone(),
            enabled: true,
        };

        let info = TaskInfo::from(task);
        assert_eq!(info.id, "test123");
        assert_eq!(info.description, "Test task");
        assert_eq!(info.expression, "0 9 * * *");
        assert_eq!(info.command, "echo hello");
        assert_eq!(info.env, env);
        assert!(info.enabled);
        assert_eq!(info.next_run, None);
    }
}
