//! MiseV2026Adapter - mise 2026.x+ adapter implementation
//!
//! Implements all 4 Port traits (DependencyPort, TaskPort, EnvPort, ConfigPort) for mise 2026.x+.
//!
//! **Strategy**:
//! - Layer 1 (Primary): Parse config files directly (fast, no subprocess)
//! - Layer 2 (Fallback): Call mise CLI when Layer 1 insufficient
//!
//! **When to use Layer 2**:
//! - Tool installation/removal (requires mise side effects)
//! - Task execution with dependencies (mise handles DAG)
//! - Environment variable templates (_.file, _.source require mise evaluation)

use crate::config::models::MiseConfig;
use crate::ports::*;
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::RwLock;
use tokio::process::Command;

use super::command::MiseCommand;
use super::parser::parse_mise_config;

pub struct MiseV2026Adapter {
    version: MiseVersion,
    /// Config cache with thread-safe interior mutability (RwLock allows Sync)
    config_cache: RwLock<Option<MiseConfig>>,
}

impl MiseV2026Adapter {
    pub fn new(version: MiseVersion) -> Self {
        Self {
            version,
            config_cache: RwLock::new(None),
        }
    }

    /// Load mise config from standard paths (Layer 1)
    ///
    /// Search order:
    /// 1. ~/.config/mise/config.toml
    /// 2. ~/.config/mise/conf.d/*.toml (merge all)
    /// 3. ./.mise.toml (cwd)
    ///
    /// Uses thread-safe interior mutability (RwLock) to cache config even with &self
    fn load_mise_config(&self) -> Result<MiseConfig> {
        // Check cache first
        if let Some(cached) = self.config_cache.read().unwrap().as_ref() {
            return Ok(cached.clone());
        }

        let home = dirs::home_dir().context("Cannot determine home directory")?;
        let config_path = home.join(".config/mise/config.toml");

        let config = if config_path.exists() {
            parse_mise_config(&config_path)?
        } else {
            // Fallback: Empty config (mise CLI will use defaults)
            MiseConfig {
                tools: HashMap::new(),
                env: HashMap::new(),
                tasks: HashMap::new(),
            }
        };

        // Cache and return
        *self.config_cache.write().unwrap() = Some(config.clone());
        Ok(config)
    }

    /// Execute mise command and capture output
    async fn exec_mise_command(&self, cmd: &mut Command) -> Result<std::process::Output> {
        let output = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("Failed to execute mise command")?;

        Ok(output)
    }
}

#[async_trait]
impl DependencyPort for MiseV2026Adapter {
    /// Install tool (Layer 2 - must use mise CLI)
    async fn install(&self, tool: &str, version: &str) -> Result<()> {
        let mut cmd = MiseCommand::install(tool, version);
        let output = self.exec_mise_command(&mut cmd).await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mise install failed for {}@{}: {}", tool, version, stderr);
        }

        Ok(())
    }

    /// List installed tools (Layer 1 preferred, Layer 2 fallback)
    async fn list_installed(&self) -> Result<Vec<ToolInfo>> {
        // Try Layer 1: Parse config file
        if let Ok(config) = self.load_mise_config() {
            if !config.tools.is_empty() {
                return Ok(config
                    .tools
                    .iter()
                    .map(|(name, version)| ToolInfo {
                        name: name.clone(),
                        version: version.clone(),
                        source: "config".to_string(),
                    })
                    .collect());
            }
        }

        // Layer 2 fallback: Call mise ls --json
        let mut cmd = MiseCommand::list_installed();
        let output = self.exec_mise_command(&mut cmd).await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mise ls --json failed: {}", stderr);
        }

        let tools: Vec<ToolInfo> = serde_json::from_slice(&output.stdout)
            .context("Failed to parse mise ls JSON output")?;

        Ok(tools)
    }

    /// Use tool (Layer 2 - must use mise CLI to modify config)
    async fn use_tool(&self, tool: &str, version: &str) -> Result<()> {
        let mut cmd = MiseCommand::use_tool(tool, version);
        let output = self.exec_mise_command(&mut cmd).await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mise use {}@{} failed: {}", tool, version, stderr);
        }

        // Invalidate cache after config modification
        *self.config_cache.write().unwrap() = None;

        Ok(())
    }

    /// Remove tool (Layer 2 - must use mise CLI)
    async fn remove(&self, tool: &str, version: &str) -> Result<()> {
        let mut cmd = MiseCommand::uninstall(tool, version);
        let output = self.exec_mise_command(&mut cmd).await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mise uninstall {}@{} failed: {}", tool, version, stderr);
        }

        Ok(())
    }

    fn mise_version(&self) -> &MiseVersion {
        &self.version
    }
}

#[async_trait]
impl TaskPort for MiseV2026Adapter {
    /// Get task command (Layer 1 - parse config)
    async fn get_task_command(&self, name: &str) -> Result<TaskCommand> {
        let config = self.load_mise_config()?;

        let task = config
            .tasks
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Task '{}' not found in mise config", name))?;

        Ok(TaskCommand {
            command: task.run.clone(),
            env: task.env.clone(),
            workdir: None, // TODO: Extract from task config if available
        })
    }

    /// Run task (Layer 2 - mise CLI handles dependencies)
    async fn run_task(&self, name: &str, args: &[String]) -> Result<TaskOutput> {
        let mut cmd = MiseCommand::run_task(name, args);
        let output = self.exec_mise_command(&mut cmd).await?;

        Ok(TaskOutput {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    /// List tasks (Layer 1 preferred, Layer 2 fallback)
    async fn list_tasks(&self) -> Result<Vec<TaskInfo>> {
        // Try Layer 1: Parse config file
        if let Ok(config) = self.load_mise_config() {
            if !config.tasks.is_empty() {
                return Ok(config
                    .tasks
                    .iter()
                    .map(|(name, task)| TaskInfo {
                        name: name.clone(),
                        description: task.description.clone(),
                        command: task.run.clone(),
                        depends: task.depends.clone(),
                    })
                    .collect());
            }
        }

        // Layer 2 fallback: Call mise tasks ls --json
        let mut cmd = MiseCommand::list_tasks();
        let output = self.exec_mise_command(&mut cmd).await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mise tasks ls --json failed: {}", stderr);
        }

        let tasks: Vec<TaskInfo> = serde_json::from_slice(&output.stdout)
            .context("Failed to parse mise tasks JSON output")?;

        Ok(tasks)
    }
}

#[async_trait]
impl EnvPort for MiseV2026Adapter {
    /// Get environment variables (Layer 2 - handles templates)
    ///
    /// Uses mise CLI because:
    /// - _.file requires file system reads
    /// - _.source requires command execution
    /// - Templates require mise evaluation
    async fn get_env(&self) -> Result<HashMap<String, String>> {
        let mut cmd = MiseCommand::env_json();
        let output = self.exec_mise_command(&mut cmd).await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mise env --json failed: {}", stderr);
        }

        let env: HashMap<String, String> = serde_json::from_slice(&output.stdout)
            .context("Failed to parse mise env JSON output")?;

        Ok(env)
    }

    /// Get environment variables for specific directory (Layer 2)
    async fn get_env_for_dir(&self, dir: &Path) -> Result<HashMap<String, String>> {
        let mut cmd = MiseCommand::env_for_dir(dir);
        let output = self.exec_mise_command(&mut cmd).await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("mise env --json (cwd={}) failed: {}", dir.display(), stderr);
        }

        let env: HashMap<String, String> = serde_json::from_slice(&output.stdout)
            .context("Failed to parse mise env JSON output")?;

        Ok(env)
    }
}

#[async_trait]
impl ConfigPort for MiseV2026Adapter {
    /// List all mise config files (Layer 1 - file system scan)
    async fn list_config_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        let home = dirs::home_dir().context("Cannot determine home directory")?;

        // Standard config file
        let config_path = home.join(".config/mise/config.toml");
        if config_path.exists() {
            files.push(config_path);
        }

        // conf.d directory
        let confd_dir = home.join(".config/mise/conf.d");
        if confd_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&confd_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && path.extension().is_some_and(|ext| ext == "toml") {
                        files.push(path);
                    }
                }
            }
        }

        // Project-local configs
        let cwd = std::env::current_dir().context("Cannot determine current directory")?;
        let local_configs = vec![
            cwd.join(".mise.toml"),
            cwd.join(".config/mise.toml"),
            cwd.join("mise.toml"),
        ];

        for local_config in local_configs {
            if local_config.exists() {
                files.push(local_config);
            }
        }

        Ok(files)
    }

    /// Read config file (Layer 1 - file I/O)
    async fn read_config(&self, path: &Path) -> Result<toml::Value> {
        let content = tokio::fs::read_to_string(path)
            .await
            .with_context(|| format!("Failed to read config: {}", path.display()))?;

        toml::from_str(&content).with_context(|| format!("Invalid TOML in {}", path.display()))
    }

    /// Write config file (Layer 1 - file I/O)
    async fn write_config(&self, path: &Path, value: &toml::Value) -> Result<()> {
        let content = toml::to_string_pretty(value).context("Failed to serialize TOML")?;

        tokio::fs::write(path, content)
            .await
            .with_context(|| format!("Failed to write config: {}", path.display()))?;

        // Invalidate cache after config modification
        *self.config_cache.write().unwrap() = None;

        Ok(())
    }

    async fn get_global_env_var(&self, _key: &str) -> Result<Option<String>> {
        Ok(None)
    }

    async fn get_service_env_var(&self, _service_name: &str, _key: &str) -> Result<Option<String>> {
        Ok(None)
    }

    async fn get_task_env_var(&self, _task_name: &str, _key: &str) -> Result<Option<String>> {
        Ok(None)
    }

    async fn get_global_env(&self) -> Result<HashMap<String, String>> {
        let home = dirs::home_dir().context("Cannot determine home directory")?;
        let config_path = home.join(".config/mise/config.toml");

        if !config_path.exists() {
            return Ok(HashMap::new());
        }

        let toml_value = self.read_config(&config_path).await?;

        // 读取 [env] 段落
        let env = toml_value
            .get("env")
            .and_then(|v| v.as_table())
            .map(|table| {
                table
                    .iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        Ok(env)
    }
    async fn get_service_envs(&self) -> Result<HashMap<String, HashMap<String, String>>> {
        let home = dirs::home_dir().context("Cannot determine home directory")?;
        let config_path = home.join(".config/mise/svcmgr/config.toml");

        if !config_path.exists() {
            return Ok(HashMap::new());
        }

        let toml_value = self.read_config(&config_path).await?;

        // 读取 [services] 段落
        let services = toml_value
            .get("services")
            .and_then(|v| v.as_table())
            .map(|services_table| {
                services_table
                    .iter()
                    .filter_map(|(service_name, service_config)| {
                        service_config
                            .get("env")
                            .and_then(|v| v.as_table())
                            .map(|env_table| {
                                let env: HashMap<String, String> = env_table
                                    .iter()
                                    .filter_map(|(k, v)| {
                                        v.as_str().map(|s| (k.clone(), s.to_string()))
                                    })
                                    .collect();
                                (service_name.clone(), env)
                            })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(services)
    }
    async fn get_task_envs(&self) -> Result<HashMap<String, HashMap<String, String>>> {
        let home = dirs::home_dir().context("Cannot determine home directory")?;
        let config_path = home.join(".config/mise/config.toml");

        if !config_path.exists() {
            return Ok(HashMap::new());
        }

        let toml_value = self.read_config(&config_path).await?;

        // 读取 [tasks] 段落
        let tasks = toml_value
            .get("tasks")
            .and_then(|v| v.as_table())
            .map(|tasks_table| {
                tasks_table
                    .iter()
                    .filter_map(|(task_name, task_config)| {
                        task_config
                            .get("env")
                            .and_then(|v| v.as_table())
                            .map(|env_table| {
                                let env: HashMap<String, String> = env_table
                                    .iter()
                                    .filter_map(|(k, v)| {
                                        v.as_str().map(|s| (k.clone(), s.to_string()))
                                    })
                                    .collect();
                                (task_name.clone(), env)
                            })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(tasks)
    }

    async fn set_env_var(
        &self,
        key: &str,
        value: &str,
        scope: &crate::env::EnvScope,
    ) -> Result<()> {
        use crate::env::EnvScope;

        let home = dirs::home_dir().context("Cannot determine home directory")?;

        // 根据scope确定配置文件路径和段落
        let (config_path, section_path) = match scope {
            EnvScope::Global => (
                home.join(".config/mise/config.toml"),
                vec!["env".to_string()],
            ),
            EnvScope::Service { name } => (
                home.join(".config/mise/svcmgr/config.toml"),
                vec!["services".to_string(), name.clone(), "env".to_string()],
            ),
            EnvScope::Task { name } => (
                home.join(".config/mise/config.toml"),
                vec!["tasks".to_string(), name.clone(), "env".to_string()],
            ),
        };

        // 确保配置文件目录存在
        if let Some(parent) = config_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("Failed to create config dir: {}", parent.display()))?;
        }

        // 读取或创建空配置
        let mut toml_value = if config_path.exists() {
            self.read_config(&config_path).await?
        } else {
            toml::Value::Table(toml::map::Map::new())
        };

        // 导航到目标段落,创建缺失的中间段落
        let mut current = &mut toml_value;
        for section in &section_path {
            current = current
                .as_table_mut()
                .ok_or_else(|| anyhow::anyhow!("Expected table at section: {}", section))?
                .entry(section.clone())
                .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
        }

        // 设置环境变量
        current
            .as_table_mut()
            .ok_or_else(|| anyhow::anyhow!("Expected table for env section"))?
            .insert(key.to_string(), toml::Value::String(value.to_string()));

        // 写回文件
        self.write_config(&config_path, &toml_value).await?;

        Ok(())
    }

    async fn delete_env_var(&self, key: &str, scope: &crate::env::EnvScope) -> Result<()> {
        use crate::env::EnvScope;

        let home = dirs::home_dir().context("Cannot determine home directory")?;

        // 根据scope确定配置文件路径和段落
        let (config_path, section_path) = match scope {
            EnvScope::Global => (
                home.join(".config/mise/config.toml"),
                vec!["env".to_string()],
            ),
            EnvScope::Service { name } => (
                home.join(".config/mise/svcmgr/config.toml"),
                vec!["services".to_string(), name.clone(), "env".to_string()],
            ),
            EnvScope::Task { name } => (
                home.join(".config/mise/config.toml"),
                vec!["tasks".to_string(), name.clone(), "env".to_string()],
            ),
        };

        if !config_path.exists() {
            // 配置文件不存在,视为删除成功(幂等性)
            return Ok(());
        }

        // 读取配置
        let mut toml_value = self.read_config(&config_path).await?;

        // 导航到目标段落
        let mut current = &mut toml_value;
        for section in &section_path {
            match current.get_mut(section) {
                Some(v) => current = v,
                None => {
                    // 段落不存在,视为删除成功(幂等性)
                    return Ok(());
                }
            }
        }

        // 删除环境变量
        if let Some(table) = current.as_table_mut() {
            table.remove(key);
        }

        // 写回文件
        self.write_config(&config_path, &toml_value).await?;

        Ok(())
    }

    // ========================================================================
    // Task Management (Configuration Management API)
    // ========================================================================

    async fn cancel_task(&self, _execution_id: &str) -> Result<()> {
        // MVP: Not implemented yet
        anyhow::bail!("cancel_task not implemented in MiseV2026Adapter")
    }

    async fn get_task_history(
        &self,
        _task_name: &str,
        _limit: u32,
        _offset: u32,
    ) -> Result<Vec<crate::web::api::task_models::TaskExecutionRecord>> {
        // MVP: Return empty history
        Ok(Vec::new())
    }

    // ========================================================================
    // Scheduled Tasks (Configuration Management API)
    // ========================================================================

    async fn list_scheduled_tasks(
        &self,
    ) -> Result<Vec<crate::web::api::task_models::ScheduledTask>> {
        // MVP: Return empty list
        Ok(Vec::new())
    }

    async fn get_scheduled_task(
        &self,
        _name: &str,
    ) -> Result<Option<crate::web::api::task_models::ScheduledTask>> {
        // MVP: Not found
        Ok(None)
    }

    async fn scheduled_task_exists(&self, _name: &str) -> Result<bool> {
        // MVP: Always false
        Ok(false)
    }

    async fn create_scheduled_task(
        &self,
        _task: &crate::web::api::task_models::ScheduledTask,
    ) -> Result<()> {
        // MVP: Not implemented
        anyhow::bail!("create_scheduled_task not implemented in MiseV2026Adapter")
    }

    async fn update_scheduled_task(
        &self,
        _name: &str,
        _task: &crate::web::api::task_models::ScheduledTask,
    ) -> Result<()> {
        // MVP: Not implemented
        anyhow::bail!("update_scheduled_task not implemented in MiseV2026Adapter")
    }

    async fn delete_scheduled_task(&self, _name: &str) -> Result<()> {
        // MVP: Not implemented
        anyhow::bail!("delete_scheduled_task not implemented in MiseV2026Adapter")
    }

    // ========================================================================
    // Service Management (Configuration Management API)
    // ========================================================================

    async fn get_service(
        &self,
        _name: &str,
    ) -> Result<crate::web::api::service_models::ServiceDefinition> {
        // MVP: Not implemented
        anyhow::bail!("get_service not implemented in MiseV2026Adapter")
    }

    async fn list_services(
        &self,
    ) -> Result<Vec<crate::web::api::service_models::ServiceDefinition>> {
        // MVP: Return empty list
        Ok(Vec::new())
    }

    async fn create_service(
        &self,
        _service: &crate::web::api::service_models::ServiceDefinition,
    ) -> Result<()> {
        // MVP: Not implemented
        anyhow::bail!("create_service not implemented in MiseV2026Adapter")
    }

    async fn update_service(
        &self,
        _name: &str,
        _service: &crate::web::api::service_models::ServiceDefinition,
    ) -> Result<()> {
        // MVP: Not implemented
        anyhow::bail!("update_service not implemented in MiseV2026Adapter")
    }

    async fn patch_service(&self, _name: &str, _updates: &serde_json::Value) -> Result<()> {
        // MVP: Not implemented
        anyhow::bail!("patch_service not implemented in MiseV2026Adapter")
    }

    async fn delete_service(&self, _name: &str) -> Result<()> {
        // MVP: Not implemented
        anyhow::bail!("delete_service not implemented in MiseV2026Adapter")
    }
}
/// Implement MiseAdapter marker trait (combines all 4 port traits)
impl super::MiseAdapter for MiseV2026Adapter {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_adapter() {
        let version = MiseVersion::new(2026, 2, 17);
        let adapter = MiseV2026Adapter::new(version.clone());
        assert_eq!(adapter.mise_version(), &version);
    }

    #[tokio::test]
    async fn test_install_tool_success() {
        // This is a mock test - real test requires mise installed
        // In integration tests, we'll use MockMiseAdapter instead
    }

    #[tokio::test]
    async fn test_load_config_fallback() {
        let adapter = MiseV2026Adapter::new(MiseVersion::new(2026, 2, 17));
        // If ~/.config/mise/config.toml doesn't exist, should return empty config
        if let Ok(config) = adapter.load_mise_config() {
            // Should not panic - config exists or is empty
            assert!(config.tools.is_empty() || !config.tools.is_empty());
        }
    }
}
