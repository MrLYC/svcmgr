#![allow(dead_code)]

/// Built-in cron scheduler atom (replaces crontab)
///
/// This module provides a built-in cron scheduling capability
/// that works in Docker containers without requiring system crontab:
/// - Task CRUD operations (add/update/remove/get/list)
/// - Cron expression validation and next-run prediction
/// - Task definitions stored as TOML files on disk
/// - Environment variable management per task
/// - Only manages tasks with [svcmgr:*] identifiers
use crate::error::{Error, Result};
use chrono::{DateTime, Utc};
use cron::Schedule;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

// ========================================
// Data Structures
// ========================================

/// Scheduled task definition stored on disk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronTask {
    /// Task ID (auto-generated or specified)
    pub id: Option<String>,
    /// Task description
    pub description: String,
    /// Cron expression (standard 5-field or predefined like @hourly)
    pub expression: String,
    /// Command to execute
    pub command: String,
    /// Environment variables
    pub env: HashMap<String, String>,
    /// Whether the task is enabled
    pub enabled: bool,
}

/// Collection of all scheduled tasks stored on disk
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct TaskStore {
    /// Global environment variables
    #[serde(default)]
    env: HashMap<String, String>,
    /// Scheduled tasks
    #[serde(default)]
    tasks: Vec<CronTask>,
}

// ========================================
// SchedulerAtom Trait
// ========================================

/// Built-in cron scheduler trait (replaces CrontabAtom)
pub trait SchedulerAtom {
    /// Add a new scheduled task
    ///
    /// # Arguments
    /// - `task`: Task configuration
    ///
    /// # Returns
    /// - Generated task ID
    fn add(&self, task: &CronTask) -> impl std::future::Future<Output = Result<String>> + Send;

    /// Update an existing scheduled task
    ///
    /// # Arguments
    /// - `task_id`: Task ID
    /// - `task`: New task configuration
    fn update(
        &self,
        task_id: &str,
        task: &CronTask,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Remove a scheduled task
    ///
    /// # Arguments
    /// - `task_id`: Task ID
    fn remove(&self, task_id: &str) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Get a specific task
    ///
    /// # Arguments
    /// - `task_id`: Task ID
    fn get(&self, task_id: &str) -> impl std::future::Future<Output = Result<CronTask>> + Send;

    /// List all managed tasks
    fn list(&self) -> impl std::future::Future<Output = Result<Vec<CronTask>>> + Send;

    /// Predict the next N execution times for a task
    ///
    /// # Arguments
    /// - `task_id`: Task ID
    /// - `count`: Number of predictions
    fn next_runs(
        &self,
        task_id: &str,
        count: usize,
    ) -> impl std::future::Future<Output = Result<Vec<DateTime<Utc>>>> + Send;

    /// Validate a cron expression
    ///
    /// # Arguments
    /// - `expr`: Cron expression
    fn validate_expression(&self, expr: &str) -> Result<bool>;

    /// Set a global environment variable
    ///
    /// # Arguments
    /// - `key`: Variable name
    /// - `value`: Variable value
    fn set_env(
        &self,
        key: &str,
        value: &str,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Get global environment variables
    fn get_env(&self) -> impl std::future::Future<Output = Result<HashMap<String, String>>> + Send;

    /// Reload task definitions from disk
    fn reload(&self) -> impl std::future::Future<Output = Result<()>> + Send;
}

// ========================================
// SchedulerManager Implementation
// ========================================

/// Built-in scheduler manager using TOML task store
pub struct SchedulerManager {
    /// Path to the task store file
    store_path: PathBuf,
    /// Task ID prefix for identification
    prefix: String,
}

impl SchedulerManager {
    /// Create a new scheduler manager
    pub fn new(store_path: PathBuf) -> Self {
        Self {
            store_path,
            prefix: "svcmgr".to_string(),
        }
    }

    /// Create with default configuration (~/.config/svcmgr/scheduler.toml)
    pub fn default_config() -> Result<Self> {
        let home = std::env::var("HOME")
            .map_err(|_| Error::Config("HOME environment variable not set".to_string()))?;
        let store_path = PathBuf::from(home).join(".config/svcmgr/scheduler.toml");
        Ok(Self::new(store_path))
    }

    /// Generate a unique task ID
    fn generate_task_id(&self) -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        format!("{}", timestamp)
    }

    /// Read the task store from disk
    async fn read_store(&self) -> Result<TaskStore> {
        if !self.store_path.exists() {
            return Ok(TaskStore::default());
        }
        let content = tokio::fs::read_to_string(&self.store_path).await?;
        let store: TaskStore = toml::from_str(&content)
            .map_err(|e| Error::Config(format!("Invalid task store: {}", e)))?;
        Ok(store)
    }

    /// Write the task store to disk
    async fn write_store(&self, store: &TaskStore) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.store_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let content = toml::to_string_pretty(store)
            .map_err(|e| Error::Config(format!("Failed to serialize task store: {}", e)))?;
        tokio::fs::write(&self.store_path, content).await?;
        Ok(())
    }

    /// Normalize cron expression (convert predefined names to standard form)
    fn normalize_expression(&self, expr: &str) -> String {
        match expr.trim() {
            "@yearly" | "@annually" => "0 0 1 1 *".to_string(),
            "@monthly" => "0 0 1 * *".to_string(),
            "@weekly" => "0 0 * * 1".to_string(),
            "@daily" | "@midnight" => "0 0 * * *".to_string(),
            "@hourly" => "0 * * * *".to_string(),
            other => other.to_string(),
        }
    }

    /// Convert standard 5-field cron expression to 6-field format (with seconds)
    fn to_schedule_format(&self, expr: &str) -> String {
        let normalized = self.normalize_expression(expr);
        format!("0 {}", normalized)
    }
}

impl Default for SchedulerManager {
    fn default() -> Self {
        Self::new(PathBuf::from("/tmp/svcmgr-scheduler.toml"))
    }
}

impl SchedulerAtom for SchedulerManager {
    async fn add(&self, task: &CronTask) -> Result<String> {
        // Validate cron expression
        self.validate_expression(&task.expression)?;

        // Read existing store
        let mut store = self.read_store().await?;

        // Generate task ID
        let task_id = task.id.clone().unwrap_or_else(|| self.generate_task_id());

        // Check for duplicate ID
        if store.tasks.iter().any(|t| t.id.as_ref() == Some(&task_id)) {
            return Err(Error::InvalidArgument(format!(
                "Task ID {} already exists",
                task_id
            )));
        }

        // Add the task
        let mut new_task = task.clone();
        new_task.id = Some(task_id.clone());
        store.tasks.push(new_task);

        // Write back
        self.write_store(&store).await?;

        Ok(task_id)
    }

    async fn update(&self, task_id: &str, task: &CronTask) -> Result<()> {
        // Validate cron expression
        self.validate_expression(&task.expression)?;

        let mut store = self.read_store().await?;

        let task_index = store
            .tasks
            .iter()
            .position(|t| t.id.as_ref() == Some(&task_id.to_string()))
            .ok_or_else(|| Error::NotSupported(format!("Task {} not found", task_id)))?;

        let mut updated_task = task.clone();
        updated_task.id = Some(task_id.to_string());
        store.tasks[task_index] = updated_task;

        self.write_store(&store).await?;

        Ok(())
    }

    async fn remove(&self, task_id: &str) -> Result<()> {
        let mut store = self.read_store().await?;

        let original_len = store.tasks.len();
        store
            .tasks
            .retain(|t| t.id.as_ref() != Some(&task_id.to_string()));

        if store.tasks.len() == original_len {
            return Err(Error::NotSupported(format!("Task {} not found", task_id)));
        }

        self.write_store(&store).await?;

        Ok(())
    }

    async fn get(&self, task_id: &str) -> Result<CronTask> {
        let store = self.read_store().await?;

        store
            .tasks
            .into_iter()
            .find(|t| t.id.as_ref() == Some(&task_id.to_string()))
            .ok_or_else(|| Error::NotSupported(format!("Task {} not found", task_id)))
    }

    async fn list(&self) -> Result<Vec<CronTask>> {
        let store = self.read_store().await?;
        Ok(store.tasks)
    }

    async fn next_runs(&self, task_id: &str, count: usize) -> Result<Vec<DateTime<Utc>>> {
        let task = self.get(task_id).await?;

        let schedule_expr = self.to_schedule_format(&task.expression);

        let schedule = Schedule::from_str(&schedule_expr)
            .map_err(|e| Error::InvalidArgument(format!("Invalid cron expression: {}", e)))?;

        let now = Utc::now();
        let upcoming: Vec<DateTime<Utc>> = schedule.after(&now).take(count).collect();

        Ok(upcoming)
    }

    fn validate_expression(&self, expr: &str) -> Result<bool> {
        let schedule_expr = self.to_schedule_format(expr);

        Schedule::from_str(&schedule_expr)
            .map(|_| true)
            .map_err(|e| Error::InvalidArgument(format!("Invalid cron expression: {}", e)))
    }

    async fn set_env(&self, key: &str, value: &str) -> Result<()> {
        let mut store = self.read_store().await?;
        store.env.insert(key.to_string(), value.to_string());
        self.write_store(&store).await?;
        Ok(())
    }

    async fn get_env(&self) -> Result<HashMap<String, String>> {
        let store = self.read_store().await?;
        Ok(store.env)
    }

    async fn reload(&self) -> Result<()> {
        // Re-read from disk; no external daemon to reload
        let _ = self.read_store().await?;
        Ok(())
    }
}

// ========================================
// Unit Tests
// ========================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_task() -> CronTask {
        CronTask {
            id: None,
            description: "Test task".to_string(),
            expression: "0 2 * * *".to_string(),
            command: "echo 'test'".to_string(),
            env: HashMap::new(),
            enabled: true,
        }
    }

    #[test]
    fn test_normalize_expression() {
        let manager = SchedulerManager::default();

        assert_eq!(manager.normalize_expression("@hourly"), "0 * * * *");
        assert_eq!(manager.normalize_expression("@daily"), "0 0 * * *");
        assert_eq!(manager.normalize_expression("@weekly"), "0 0 * * 1");
        assert_eq!(manager.normalize_expression("@monthly"), "0 0 1 * *");
        assert_eq!(manager.normalize_expression("@yearly"), "0 0 1 1 *");
        assert_eq!(manager.normalize_expression("0 2 * * *"), "0 2 * * *");
    }

    #[test]
    fn test_to_schedule_format() {
        let manager = SchedulerManager::default();

        assert_eq!(manager.to_schedule_format("0 2 * * *"), "0 0 2 * * *");
        assert_eq!(manager.to_schedule_format("@hourly"), "0 0 * * * *");
        assert_eq!(manager.to_schedule_format("@daily"), "0 0 0 * * *");
    }

    #[test]
    fn test_validate_expression() {
        let manager = SchedulerManager::default();

        // Valid expressions
        assert!(manager.validate_expression("0 2 * * *").is_ok());
        assert!(manager.validate_expression("*/5 * * * *").is_ok());
        assert!(manager.validate_expression("@hourly").is_ok());
        assert!(manager.validate_expression("@daily").is_ok());

        // Invalid expressions
        assert!(manager.validate_expression("invalid").is_err());
        assert!(manager.validate_expression("0 25 * * *").is_err());
    }

    #[test]
    fn test_validate_predefined_expressions() {
        let manager = SchedulerManager::default();

        assert!(manager.validate_expression("@hourly").is_ok());
        assert!(manager.validate_expression("@daily").is_ok());
        assert!(manager.validate_expression("@weekly").is_ok());
        assert!(manager.validate_expression("@monthly").is_ok());
        assert!(manager.validate_expression("@yearly").is_ok());
        assert!(manager.validate_expression("@annually").is_ok());
    }

    #[test]
    fn test_validate_invalid_expressions() {
        let manager = SchedulerManager::default();

        assert!(manager.validate_expression("").is_err());
        assert!(manager.validate_expression("not a cron").is_err());
        assert!(manager.validate_expression("60 * * * *").is_err());
    }

    #[test]
    fn test_normalize_expression_edge_cases() {
        let manager = SchedulerManager::default();

        assert_eq!(manager.normalize_expression("@reboot"), "@reboot");
        assert_eq!(manager.normalize_expression(""), "");
    }

    #[test]
    fn test_to_schedule_format_monthly() {
        let manager = SchedulerManager::default();

        assert_eq!(manager.to_schedule_format("@monthly"), "0 0 0 1 * *");
    }

    #[test]
    fn test_cron_task_creation() {
        let task = create_test_task();

        assert_eq!(task.description, "Test task");
        assert_eq!(task.expression, "0 2 * * *");
        assert_eq!(task.command, "echo 'test'");
        assert!(task.enabled);
        assert!(task.id.is_none());
    }

    #[test]
    fn test_task_store_default() {
        let store = TaskStore::default();
        assert!(store.tasks.is_empty());
        assert!(store.env.is_empty());
    }

    #[test]
    fn test_task_store_serialization() {
        let mut store = TaskStore::default();
        store
            .env
            .insert("SHELL".to_string(), "/bin/bash".to_string());
        store.tasks.push(CronTask {
            id: Some("123".to_string()),
            description: "Test".to_string(),
            expression: "0 2 * * *".to_string(),
            command: "echo hello".to_string(),
            env: HashMap::new(),
            enabled: true,
        });

        let toml_str = toml::to_string_pretty(&store).unwrap();
        assert!(toml_str.contains("SHELL"));
        assert!(toml_str.contains("echo hello"));

        let parsed: TaskStore = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.tasks.len(), 1);
        assert_eq!(parsed.env.get("SHELL"), Some(&"/bin/bash".to_string()));
    }

    #[test]
    fn test_generate_task_id() {
        let manager = SchedulerManager::default();

        let id1 = manager.generate_task_id();
        std::thread::sleep(std::time::Duration::from_secs(1));
        let id2 = manager.generate_task_id();

        assert_ne!(id1, id2);
        assert!(id1.parse::<u64>().is_ok());
    }

    #[tokio::test]
    async fn test_add_and_get_task() {
        let tmpdir = tempfile::tempdir().unwrap();
        let store_path = tmpdir.path().join("scheduler.toml");
        let manager = SchedulerManager::new(store_path);

        let task = CronTask {
            id: Some("test-1".to_string()),
            description: "Test add".to_string(),
            expression: "0 2 * * *".to_string(),
            command: "echo test".to_string(),
            env: HashMap::new(),
            enabled: true,
        };

        let id = manager.add(&task).await.unwrap();
        assert_eq!(id, "test-1");

        let retrieved = manager.get("test-1").await.unwrap();
        assert_eq!(retrieved.description, "Test add");
        assert_eq!(retrieved.command, "echo test");
    }

    #[tokio::test]
    async fn test_add_duplicate_id() {
        let tmpdir = tempfile::tempdir().unwrap();
        let store_path = tmpdir.path().join("scheduler.toml");
        let manager = SchedulerManager::new(store_path);

        let task = CronTask {
            id: Some("dup".to_string()),
            description: "First".to_string(),
            expression: "0 2 * * *".to_string(),
            command: "echo first".to_string(),
            env: HashMap::new(),
            enabled: true,
        };

        manager.add(&task).await.unwrap();
        assert!(manager.add(&task).await.is_err());
    }

    #[tokio::test]
    async fn test_update_task() {
        let tmpdir = tempfile::tempdir().unwrap();
        let store_path = tmpdir.path().join("scheduler.toml");
        let manager = SchedulerManager::new(store_path);

        let task = CronTask {
            id: Some("upd".to_string()),
            description: "Original".to_string(),
            expression: "0 2 * * *".to_string(),
            command: "echo original".to_string(),
            env: HashMap::new(),
            enabled: true,
        };
        manager.add(&task).await.unwrap();

        let updated = CronTask {
            id: Some("upd".to_string()),
            description: "Updated".to_string(),
            expression: "0 3 * * *".to_string(),
            command: "echo updated".to_string(),
            env: HashMap::new(),
            enabled: true,
        };
        manager.update("upd", &updated).await.unwrap();

        let retrieved = manager.get("upd").await.unwrap();
        assert_eq!(retrieved.description, "Updated");
        assert_eq!(retrieved.command, "echo updated");
    }

    #[tokio::test]
    async fn test_remove_task() {
        let tmpdir = tempfile::tempdir().unwrap();
        let store_path = tmpdir.path().join("scheduler.toml");
        let manager = SchedulerManager::new(store_path);

        let task = CronTask {
            id: Some("del".to_string()),
            description: "To delete".to_string(),
            expression: "0 2 * * *".to_string(),
            command: "echo delete".to_string(),
            env: HashMap::new(),
            enabled: true,
        };
        manager.add(&task).await.unwrap();

        manager.remove("del").await.unwrap();
        assert!(manager.get("del").await.is_err());
    }

    #[tokio::test]
    async fn test_remove_nonexistent() {
        let tmpdir = tempfile::tempdir().unwrap();
        let store_path = tmpdir.path().join("scheduler.toml");
        let manager = SchedulerManager::new(store_path);

        assert!(manager.remove("nonexistent").await.is_err());
    }

    #[tokio::test]
    async fn test_list_tasks() {
        let tmpdir = tempfile::tempdir().unwrap();
        let store_path = tmpdir.path().join("scheduler.toml");
        let manager = SchedulerManager::new(store_path);

        let tasks = manager.list().await.unwrap();
        assert!(tasks.is_empty());

        let task = CronTask {
            id: Some("t1".to_string()),
            description: "Task 1".to_string(),
            expression: "0 2 * * *".to_string(),
            command: "echo 1".to_string(),
            env: HashMap::new(),
            enabled: true,
        };
        manager.add(&task).await.unwrap();

        let tasks = manager.list().await.unwrap();
        assert_eq!(tasks.len(), 1);
    }

    #[tokio::test]
    async fn test_set_and_get_env() {
        let tmpdir = tempfile::tempdir().unwrap();
        let store_path = tmpdir.path().join("scheduler.toml");
        let manager = SchedulerManager::new(store_path);

        manager.set_env("SHELL", "/bin/bash").await.unwrap();
        manager.set_env("PATH", "/usr/bin").await.unwrap();

        let env = manager.get_env().await.unwrap();
        assert_eq!(env.get("SHELL"), Some(&"/bin/bash".to_string()));
        assert_eq!(env.get("PATH"), Some(&"/usr/bin".to_string()));
    }

    #[tokio::test]
    async fn test_reload() {
        let tmpdir = tempfile::tempdir().unwrap();
        let store_path = tmpdir.path().join("scheduler.toml");
        let manager = SchedulerManager::new(store_path);

        // reload on non-existent file should succeed (returns empty store)
        assert!(manager.reload().await.is_ok());
    }

    #[tokio::test]
    async fn test_next_runs() {
        let tmpdir = tempfile::tempdir().unwrap();
        let store_path = tmpdir.path().join("scheduler.toml");
        let manager = SchedulerManager::new(store_path);

        let task = CronTask {
            id: Some("nr".to_string()),
            description: "Next runs test".to_string(),
            expression: "*/5 * * * *".to_string(),
            command: "echo test".to_string(),
            env: HashMap::new(),
            enabled: true,
        };
        manager.add(&task).await.unwrap();

        let runs = manager.next_runs("nr", 3).await.unwrap();
        assert_eq!(runs.len(), 3);
        // Each subsequent run should be after the previous
        assert!(runs[0] < runs[1]);
        assert!(runs[1] < runs[2]);
    }
}
