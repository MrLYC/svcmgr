//! MockMiseAdapter - Test adapter wrapping MiseMock for unit/integration tests
//!
//! This adapter implements all Port traits using MiseMock's in-memory state,
//! avoiding the need for actual mise installation during tests.

use crate::mocks::mise::{MiseMock, TaskDef};
use crate::ports::*;
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

pub struct MockMiseAdapter {
    /// Shared mock state (allows interior mutability across async boundaries)
    mock: Arc<Mutex<MiseMock>>,
    version: MiseVersion,
}

impl MockMiseAdapter {
    /// Create a new MockMiseAdapter wrapping the given MiseMock
    pub fn new(mock: MiseMock, version: MiseVersion) -> Self {
        Self {
            mock: Arc::new(Mutex::new(mock)),
            version,
        }
    }

    /// Access to underlying MiseMock for test setup
    pub fn mock(&self) -> Arc<Mutex<MiseMock>> {
        Arc::clone(&self.mock)
    }
}

#[async_trait]
impl DependencyPort for MockMiseAdapter {
    async fn install(&self, tool: &str, version: &str) -> Result<()> {
        let mut mock = self.mock.lock().unwrap();
        mock.tools.insert(tool.to_string(), version.to_string());
        Ok(())
    }

    async fn list_installed(&self) -> Result<Vec<ToolInfo>> {
        let mock = self.mock.lock().unwrap();
        Ok(mock
            .tools
            .iter()
            .map(|(name, version)| ToolInfo {
                name: name.clone(),
                version: version.clone(),
                source: "mock".to_string(),
            })
            .collect())
    }

    async fn use_tool(&self, tool: &str, version: &str) -> Result<()> {
        let mut mock = self.mock.lock().unwrap();
        mock.tools.insert(tool.to_string(), version.to_string());
        Ok(())
    }

    async fn remove(&self, tool: &str, _version: &str) -> Result<()> {
        let mut mock = self.mock.lock().unwrap();
        mock.tools.remove(tool);
        Ok(())
    }

    fn mise_version(&self) -> &MiseVersion {
        &self.version
    }
}

#[async_trait]
impl TaskPort for MockMiseAdapter {
    async fn get_task_command(&self, name: &str) -> Result<TaskCommand> {
        let mock = self.mock.lock().unwrap();
        let task = mock
            .tasks
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Task '{}' not found", name))?;

        Ok(TaskCommand {
            command: task.run.clone(),
            env: task.env.clone(),
            workdir: None,
        })
    }

    async fn run_task(&self, name: &str, _args: &[String]) -> Result<TaskOutput> {
        let mock = self.mock.lock().unwrap();
        let task = mock
            .tasks
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Task '{}' not found", name))?;

        // Mock execution: return success with task command in stdout
        Ok(TaskOutput {
            exit_code: 0,
            stdout: format!("Mock execution: {}", task.run),
            stderr: String::new(),
        })
    }

    async fn list_tasks(&self) -> Result<Vec<TaskInfo>> {
        let mock = self.mock.lock().unwrap();
        Ok(mock
            .tasks
            .iter()
            .map(|(name, task)| TaskInfo {
                name: name.clone(),
                description: task.description.clone(),
                command: task.run.clone(),
                depends: task.depends.clone(),
            })
            .collect())
    }
}

#[async_trait]
impl EnvPort for MockMiseAdapter {
    async fn get_env(&self) -> Result<HashMap<String, String>> {
        let mock = self.mock.lock().unwrap();
        Ok(mock.env.clone())
    }

    async fn get_env_for_dir(&self, _dir: &Path) -> Result<HashMap<String, String>> {
        // Mock implementation: same as get_env (ignore dir)
        let mock = self.mock.lock().unwrap();
        Ok(mock.env.clone())
    }
}

#[async_trait]
impl ConfigPort for MockMiseAdapter {
    async fn list_config_files(&self) -> Result<Vec<std::path::PathBuf>> {
        let mock = self.mock.lock().unwrap();
        Ok(vec![mock.workdir.join(".mise.toml")])
    }

    async fn read_config(&self, _path: &Path) -> Result<toml::Value> {
        let mock = self.mock.lock().unwrap();

        // Generate TOML from mock state
        let mut table = toml::value::Table::new();

        // [tools]
        if !mock.tools.is_empty() {
            let tools: toml::value::Table = mock
                .tools
                .iter()
                .map(|(k, v)| (k.clone(), toml::Value::String(v.clone())))
                .collect();
            table.insert("tools".to_string(), toml::Value::Table(tools));
        }

        // [env]
        if !mock.env.is_empty() {
            let env: toml::value::Table = mock
                .env
                .iter()
                .map(|(k, v)| (k.clone(), toml::Value::String(v.clone())))
                .collect();
            table.insert("env".to_string(), toml::Value::Table(env));
        }

        // [tasks]
        if !mock.tasks.is_empty() {
            let tasks: toml::value::Table = mock
                .tasks
                .iter()
                .map(|(name, task)| {
                    let mut task_table = toml::value::Table::new();
                    task_table.insert("run".to_string(), toml::Value::String(task.run.clone()));

                    if let Some(desc) = &task.description {
                        task_table
                            .insert("description".to_string(), toml::Value::String(desc.clone()));
                    }

                    if !task.depends.is_empty() {
                        let depends: Vec<toml::Value> = task
                            .depends
                            .iter()
                            .map(|d| toml::Value::String(d.clone()))
                            .collect();
                        task_table.insert("depends".to_string(), toml::Value::Array(depends));
                    }

                    (name.clone(), toml::Value::Table(task_table))
                })
                .collect();
            table.insert("tasks".to_string(), toml::Value::Table(tasks));
        }

        Ok(toml::Value::Table(table))
    }

    async fn write_config(&self, _path: &Path, value: &toml::Value) -> Result<()> {
        let mut mock = self.mock.lock().unwrap();

        // Parse TOML and update mock state
        if let Some(tools) = value.get("tools").and_then(|v| v.as_table()) {
            mock.tools.clear();
            for (k, v) in tools {
                if let Some(version) = v.as_str() {
                    mock.tools.insert(k.clone(), version.to_string());
                }
            }
        }

        if let Some(env) = value.get("env").and_then(|v| v.as_table()) {
            mock.env.clear();
            for (k, v) in env {
                if let Some(val) = v.as_str() {
                    mock.env.insert(k.clone(), val.to_string());
                }
            }
        }

        if let Some(tasks) = value.get("tasks").and_then(|v| v.as_table()) {
            mock.tasks.clear();
            for (name, task_value) in tasks {
                if let Some(task_table) = task_value.as_table()
                    && let Some(run) = task_table.get("run").and_then(|v| v.as_str())
                {
                    let task = TaskDef {
                        run: run.to_string(),
                        description: task_table
                            .get("description")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        depends: task_table
                            .get("depends")
                            .and_then(|v| v.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str())
                                    .map(String::from)
                                    .collect()
                            })
                            .unwrap_or_default(),
                        env: HashMap::new(), // Simplified for mock
                    };
                    mock.tasks.insert(name.clone(), task);
                }
            }
        }

        Ok(())
    }
}

/// Implement MiseAdapter marker trait (combines all 4 port traits)
impl super::MiseAdapter for MockMiseAdapter {}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_mock_dependency_port() {
        let temp = TempDir::new().unwrap();
        let mock = MiseMock::new(temp.path().to_path_buf())
            .with_tool("node", "20")
            .with_tool("rust", "1.75");

        let adapter = MockMiseAdapter::new(mock, MiseVersion::new(2026, 2, 17));

        // Test list_installed
        let tools = adapter.list_installed().await.unwrap();
        assert_eq!(tools.len(), 2);

        // Test install
        adapter.install("python", "3.12").await.unwrap();
        let tools = adapter.list_installed().await.unwrap();
        assert_eq!(tools.len(), 3);
        assert!(tools
            .iter()
            .any(|t| t.name == "python" && t.version == "3.12"));

        // Test remove
        adapter.remove("node", "20").await.unwrap();
        let tools = adapter.list_installed().await.unwrap();
        assert_eq!(tools.len(), 2);
        assert!(!tools.iter().any(|t| t.name == "node"));
    }

    #[tokio::test]
    async fn test_mock_task_port() {
        let temp = TempDir::new().unwrap();
        let mut mock = MiseMock::new(temp.path().to_path_buf());

        let task = TaskDef {
            run: "npm run build".to_string(),
            description: Some("Build the project".to_string()),
            depends: vec!["install".to_string()],
            env: HashMap::new(),
        };
        mock = mock.with_task("build", task);

        let adapter = MockMiseAdapter::new(mock, MiseVersion::new(2026, 2, 17));

        // Test get_task_command
        let cmd = adapter.get_task_command("build").await.unwrap();
        assert_eq!(cmd.command, "npm run build");

        // Test list_tasks
        let tasks = adapter.list_tasks().await.unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].name, "build");
        assert_eq!(tasks[0].description, Some("Build the project".to_string()));

        // Test run_task
        let output = adapter.run_task("build", &[]).await.unwrap();
        assert_eq!(output.exit_code, 0);
        assert!(output.stdout.contains("npm run build"));
    }

    #[tokio::test]
    async fn test_mock_env_port() {
        let temp = TempDir::new().unwrap();
        let mock = MiseMock::new(temp.path().to_path_buf())
            .with_env("NODE_ENV", "test")
            .with_env("LOG_LEVEL", "debug");

        let adapter = MockMiseAdapter::new(mock, MiseVersion::new(2026, 2, 17));

        let env = adapter.get_env().await.unwrap();
        assert_eq!(env.len(), 2);
        assert_eq!(env.get("NODE_ENV"), Some(&"test".to_string()));
        assert_eq!(env.get("LOG_LEVEL"), Some(&"debug".to_string()));
    }

    #[tokio::test]
    async fn test_mock_config_port() {
        let temp = TempDir::new().unwrap();
        let mock = MiseMock::new(temp.path().to_path_buf())
            .with_tool("node", "20")
            .with_env("APP_ENV", "test");

        let adapter = MockMiseAdapter::new(mock, MiseVersion::new(2026, 2, 17));

        // Test read_config
        let config_path = temp.path().join(".mise.toml");
        let config = adapter.read_config(&config_path).await.unwrap();

        assert!(config.get("tools").is_some());
        assert!(config.get("env").is_some());

        let tools = config.get("tools").unwrap().as_table().unwrap();
        assert_eq!(tools.get("node").unwrap().as_str(), Some("20"));

        let env = config.get("env").unwrap().as_table().unwrap();
        assert_eq!(env.get("APP_ENV").unwrap().as_str(), Some("test"));

        // Test write_config
        let mut new_config = toml::value::Table::new();
        let mut new_tools = toml::value::Table::new();
        new_tools.insert(
            "python".to_string(),
            toml::Value::String("3.12".to_string()),
        );
        new_config.insert("tools".to_string(), toml::Value::Table(new_tools));

        adapter
            .write_config(&config_path, &toml::Value::Table(new_config))
            .await
            .unwrap();

        // Verify write by reading back
        let tools = adapter.list_installed().await.unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "python");
    }
}
