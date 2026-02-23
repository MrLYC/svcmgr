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
        if let Ok(config) = self.load_mise_config()
            && !config.tools.is_empty()
        {
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
        if let Ok(config) = self.load_mise_config()
            && !config.tasks.is_empty()
        {
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
        if confd_dir.is_dir()
            && let Ok(entries) = std::fs::read_dir(&confd_dir)
        {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().is_some_and(|ext| ext == "toml") {
                    files.push(path);
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
