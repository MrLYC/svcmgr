// Configuration file parser
//
// This module handles:
// 1. Loading svcmgr config from .config/mise/svcmgr/config.toml
// 2. Merging with default config
// 3. Hot reload support via file watcher
//
// Note: mise config (.mise.toml) parsing is handled by MisePort adapter (Phase 1.2)

use crate::config::models::{MiseConfig, SvcmgrConfig};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Configuration loader and parser
pub struct ConfigParser {
    /// Path to svcmgr config directory
    config_dir: PathBuf,
}

impl ConfigParser {
    /// Create new parser with default config directory
    ///
    /// Default path: ~/.config/mise/svcmgr/
    pub fn new() -> Result<Self> {
        let config_dir = Self::default_config_dir()?;
        Ok(Self { config_dir })
    }

    /// Create parser with custom config directory
    pub fn with_config_dir(config_dir: PathBuf) -> Self {
        Self { config_dir }
    }

    /// Get default config directory (~/.config/mise/svcmgr/)
    fn default_config_dir() -> Result<PathBuf> {
        let home = dirs::home_dir().context("Failed to get home directory")?;
        Ok(home.join(".config/mise/svcmgr"))
    }

    /// Get path to svcmgr config file
    fn config_file_path(&self) -> PathBuf {
        self.config_dir.join("config.toml")
    }

    /// Load svcmgr configuration
    ///
    /// Returns default config if file doesn't exist.
    pub fn load(&self) -> Result<SvcmgrConfig> {
        let config_path = self.config_file_path();

        if !config_path.exists() {
            tracing::info!(
                "Config file not found at {:?}, using default config",
                config_path
            );
            return Ok(SvcmgrConfig::default());
        }

        let content = fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file: {:?}", config_path))?;

        let config: SvcmgrConfig = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {:?}", config_path))?;

        tracing::debug!("Loaded config from {:?}", config_path);
        Ok(config)
    }

    /// Load and validate configuration against mise config
    pub fn load_and_validate(&self, mise_config: &MiseConfig) -> Result<SvcmgrConfig> {
        let config = self.load()?;

        // Validate configuration
        config.validate(mise_config).map_err(|errors| {
            let error_list = errors.join("\n  - ");
            anyhow::anyhow!("Configuration validation failed:\n  - {}", error_list)
        })?;

        Ok(config)
    }

    /// Initialize config directory and create default config file
    pub fn init(&self) -> Result<()> {
        // Create config directory
        if !self.config_dir.exists() {
            fs::create_dir_all(&self.config_dir).with_context(|| {
                format!("Failed to create config directory: {:?}", self.config_dir)
            })?;
            tracing::info!("Created config directory: {:?}", self.config_dir);
        }

        let config_path = self.config_file_path();

        // Don't overwrite existing config
        if config_path.exists() {
            tracing::info!("Config file already exists: {:?}", config_path);
            return Ok(());
        }

        // Write default config with examples
        let default_config = Self::default_config_content();
        fs::write(&config_path, default_config)
            .with_context(|| format!("Failed to write config file: {:?}", config_path))?;

        tracing::info!("Created default config: {:?}", config_path);
        Ok(())
    }

    /// Get default config file content with examples
    fn default_config_content() -> &'static str {
        r#"# svcmgr Configuration
#
# This file defines services, credentials, and feature flags.
# Learn more: https://github.com/jdx/svcmgr

[features]
web_ui = true
proxy = true
tunnel = false
scheduler = true
git_versioning = true
resource_limits = true

# Example: Long-running service via mise task
# [services.web]
# task = "dev-server"           # mise task name
# enable = true                  # auto-start on system boot
# restart = "always"             # restart policy: "no", "always", "on-failure"
# restart_delay = "2s"           # initial restart delay (exponential backoff)
# restart_limit = 10             # max restart attempts
# restart_window = "60s"         # time window for counting failures
# stop_timeout = "10s"           # graceful stop timeout
# ports.http = 3000              # port mapping for HTTP proxy

# Example: Script mode service (direct command)
# [services.redis]
# run_mode = "script"            # "mise" (default) or "script"
# command = "redis-server --port 6379"
# restart = "on-failure"
# cpu_max_percent = 50           # CPU limit (0-100)
# memory_max = "512m"            # Memory limit (e.g., "512m", "1g")
# pids_max = 100                 # Max number of processes

# Example: Scheduled task (cron)
# [services.backup]
# task = "backup-db"
# cron = "0 2 * * *"             # Run at 2:00 AM daily

# Example: HTTP Basic Auth credential
# [credentials.admin]
# type = "basic"
# username_secret = "admin_user" # fnox secret reference
# password_secret = "admin_pass" # fnox secret reference
# realm = "Admin Area"

# Example: Git-versioned configuration directory
# [configurations.nginx]
# path = "/etc/nginx"
"#
    }

    /// Get config directory path
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }
}

impl Default for ConfigParser {
    fn default() -> Self {
        Self::new().expect("Failed to create default ConfigParser")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::tempdir;

    #[test]
    fn test_load_nonexistent_returns_default() {
        let dir = tempdir().unwrap();
        let parser = ConfigParser::with_config_dir(dir.path().to_path_buf());

        let config = parser.load().unwrap();
        assert_eq!(config, SvcmgrConfig::default());
    }

    #[test]
    fn test_load_valid_config() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");

        let toml_content = r#"
            [features]
            web_ui = false
            proxy = true

            [services.web]
            task = "dev-server"
            enable = true
            restart = "always"
        "#;

        fs::write(&config_path, toml_content).unwrap();

        let parser = ConfigParser::with_config_dir(dir.path().to_path_buf());
        let config = parser.load().unwrap();

        assert!(!config.features.web_ui);
        assert!(config.features.proxy);
        assert_eq!(config.services.len(), 1);
        assert!(config.services.contains_key("web"));
    }

    #[test]
    fn test_load_invalid_toml() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");

        fs::write(&config_path, "invalid { toml ]").unwrap();

        let parser = ConfigParser::with_config_dir(dir.path().to_path_buf());
        let result = parser.load();

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to parse config"));
    }

    #[test]
    fn test_validate_missing_task() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");

        let toml_content = r#"
            [services.web]
            task = "non-existent-task"
        "#;

        fs::write(&config_path, toml_content).unwrap();

        let parser = ConfigParser::with_config_dir(dir.path().to_path_buf());
        let mise_config = MiseConfig::default();

        let result = parser.load_and_validate(&mise_config);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("non-existent mise task"));
    }

    #[test]
    fn test_validate_with_valid_mise_task() {
        use crate::config::models::MiseTask;

        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");

        let toml_content = r#"
            [services.web]
            task = "dev-server"
        "#;

        fs::write(&config_path, toml_content).unwrap();

        let parser = ConfigParser::with_config_dir(dir.path().to_path_buf());

        let mut mise_config = MiseConfig::default();
        mise_config.tasks.insert(
            "dev-server".to_string(),
            MiseTask {
                description: None,
                run: "npm run dev".to_string(),
                depends: vec![],
                env: HashMap::new(),
                sources: vec![],
                outputs: vec![],
            },
        );

        let result = parser.load_and_validate(&mise_config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_init_creates_directory_and_config() {
        let dir = tempdir().unwrap();
        let parser = ConfigParser::with_config_dir(dir.path().to_path_buf());

        parser.init().unwrap();

        assert!(dir.path().exists());
        assert!(dir.path().join("config.toml").exists());

        let content = fs::read_to_string(dir.path().join("config.toml")).unwrap();
        assert!(content.contains("svcmgr Configuration"));
        assert!(content.contains("[features]"));
    }

    #[test]
    fn test_init_does_not_overwrite() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");

        fs::create_dir_all(dir.path()).unwrap();
        fs::write(&config_path, "original content").unwrap();

        let parser = ConfigParser::with_config_dir(dir.path().to_path_buf());
        parser.init().unwrap();

        let content = fs::read_to_string(&config_path).unwrap();
        assert_eq!(content, "original content");
    }
}
