// Configuration data models for svcmgr
//
// This module defines the data structures for both svcmgr and mise configurations.
// According to OpenSpec 01-config-design.md:
// - svcmgr config: .config/mise/svcmgr/config.toml (independent)
// - mise config: .config/mise/config.toml (parsed via Port-Adapter)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

// ============================================================================
// SvcmgrConfig - Main configuration structure
// ============================================================================

/// Complete svcmgr configuration
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
pub struct SvcmgrConfig {
    /// Feature flags
    #[serde(default)]
    pub features: FeatureFlags,

    /// Service definitions
    #[serde(default)]
    pub services: HashMap<String, ServiceConfig>,

    /// Configuration directories (for Git versioning)
    #[serde(default)]
    pub configurations: HashMap<String, ConfigurationDir>,

    /// Credential definitions (for HTTP proxy auth)
    #[serde(default)]
    pub credentials: HashMap<String, CredentialConfig>,
}

// ============================================================================
// Feature Flags
// ============================================================================

/// Feature toggle flags
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct FeatureFlags {
    #[serde(default = "default_true")]
    pub web_ui: bool,

    #[serde(default = "default_true")]
    pub proxy: bool,

    #[serde(default)]
    pub tunnel: bool,

    #[serde(default = "default_true")]
    pub scheduler: bool,

    #[serde(default = "default_true")]
    pub git_versioning: bool,

    #[serde(default = "default_true")]
    pub resource_limits: bool,
}

fn default_true() -> bool {
    true
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            web_ui: true,
            proxy: true,
            tunnel: false,
            scheduler: true,
            git_versioning: true,
            resource_limits: true,
        }
    }
}

// ============================================================================
// Service Configuration
// ============================================================================

/// Service definition (can be long-running or scheduled task)
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ServiceConfig {
    /// Run mode: "mise" (default) or "script"
    #[serde(default = "default_run_mode")]
    pub run_mode: RunMode,

    /// mise task name (mise mode) or direct command (script mode)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,

    /// Direct command (script mode only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,

    /// Enable auto-start
    #[serde(default = "default_true")]
    pub enable: bool,

    /// Restart policy: "no", "always", "on-failure"
    #[serde(default = "default_restart")]
    pub restart: RestartPolicy,

    /// Initial restart delay (exponential backoff)
    #[serde(default = "default_restart_delay")]
    pub restart_delay: String,

    /// Maximum restart attempts
    #[serde(default = "default_restart_limit")]
    pub restart_limit: u32,

    /// Restart window for counting failures
    #[serde(default = "default_restart_window")]
    pub restart_window: String,

    /// Graceful stop timeout
    #[serde(default = "default_stop_timeout")]
    pub stop_timeout: String,

    /// Working directory
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workdir: Option<PathBuf>,

    /// Execution timeout (0 = no timeout for long-running services)
    #[serde(default)]
    pub timeout: String,

    /// Port mappings (for HTTP proxy)
    #[serde(default)]
    pub ports: HashMap<String, u16>,

    /// Environment variables (script mode only)
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Cron schedule (for scheduled tasks)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cron: Option<String>,

    // Resource limits (cgroups v2)
    /// CPU limit (percentage, 0-100)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_max_percent: Option<u32>,

    /// Memory limit (e.g., "512m", "1g")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_max: Option<String>,

    /// Maximum number of processes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pids_max: Option<u32>,

    /// Health check configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health_check: Option<HealthCheckConfig>,
}

fn default_run_mode() -> RunMode {
    RunMode::Mise
}

fn default_restart() -> RestartPolicy {
    RestartPolicy::No
}

fn default_restart_delay() -> String {
    "2s".to_string()
}

fn default_restart_limit() -> u32 {
    10
}

fn default_restart_window() -> String {
    "60s".to_string()
}

fn default_stop_timeout() -> String {
    "10s".to_string()
}

/// Service run mode
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RunMode {
    /// Execute via `mise run <task>` (inherits mise environment)
    Mise,
    /// Execute command directly (no mise integration)
    Script,
}

/// Service restart policy
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RestartPolicy {
    /// Never restart
    No,
    /// Always restart on exit
    Always,
    /// Restart only on non-zero exit code
    OnFailure,
}

/// Health check configuration
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct HealthCheckConfig {
    /// Enable health check
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// HTTP path to check (e.g., "/health")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_path: Option<String>,

    /// Check interval in seconds
    #[serde(default = "default_health_interval")]
    pub interval_secs: u64,

    /// Number of consecutive failures before marking unhealthy
    #[serde(default = "default_health_threshold")]
    pub failure_threshold: u32,

    /// Request timeout in seconds
    #[serde(default = "default_health_timeout")]
    pub timeout_secs: u64,
}

fn default_health_interval() -> u64 {
    30
}

fn default_health_threshold() -> u32 {
    3
}

fn default_health_timeout() -> u64 {
    5
}

// ============================================================================
// Configuration Directory
// ============================================================================

/// Configuration directory for Git versioning
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ConfigurationDir {
    /// Path to configuration directory
    pub path: PathBuf,
}

// ============================================================================
// Credential Configuration
// ============================================================================

/// Credential definition for HTTP proxy authentication
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum CredentialConfig {
    /// Basic authentication
    Basic {
        /// fnox secret reference for username
        username_secret: String,
        /// fnox secret reference for password
        password_secret: String,
        /// HTTP Basic Auth realm (optional)
        #[serde(skip_serializing_if = "Option::is_none")]
        realm: Option<String>,
    },
    /// Bearer token
    Bearer {
        /// fnox secret reference for token
        token_secret: String,
    },
    /// API key (via header or query param)
    #[serde(rename = "api_key")]
    ApiKey {
        /// fnox secret reference for API key
        key_secret: String,
        /// Header name (e.g., "X-API-Key")
        #[serde(skip_serializing_if = "Option::is_none")]
        header_name: Option<String>,
        /// Query parameter name (e.g., "api_key")
        #[serde(skip_serializing_if = "Option::is_none")]
        query_param: Option<String>,
    },
    /// Custom header
    Custom {
        /// Custom header name
        header_name: String,
        /// fnox secret reference for header value
        value_secret: String,
    },
}

// ============================================================================
// MiseConfig - Parsed from mise configuration files
// ============================================================================

/// Parsed mise configuration (via Port-Adapter)
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
pub struct MiseConfig {
    /// Tool versions (e.g., node = "22", python = "3.12")
    #[serde(default)]
    pub tools: HashMap<String, String>,

    /// Environment variables
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Task definitions
    #[serde(default)]
    pub tasks: HashMap<String, MiseTask>,
}

/// mise task definition
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct MiseTask {
    /// Task description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Command to run
    pub run: String,

    /// Task dependencies
    #[serde(default)]
    pub depends: Vec<String>,

    /// Task-specific environment variables
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Source file patterns (for rebuild detection)
    #[serde(default)]
    pub sources: Vec<String>,

    /// Output file patterns (for rebuild detection)
    #[serde(default)]
    pub outputs: Vec<String>,
}

// ============================================================================
// Validation
// ============================================================================

impl SvcmgrConfig {
    /// Validate configuration against mise config
    pub fn validate(&self, mise_config: &MiseConfig) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        for (name, service) in &self.services {
            // Validate task reference (mise mode only)
            if service.run_mode == RunMode::Mise {
                if let Some(task_name) = &service.task {
                    if !mise_config.tasks.contains_key(task_name) {
                        errors.push(format!(
                            "Service '{}' references non-existent mise task '{}'",
                            name, task_name
                        ));
                    }
                } else {
                    errors.push(format!(
                        "Service '{}' is in mise mode but missing 'task' field",
                        name
                    ));
                }
            }

            // Validate script mode has command
            if service.run_mode == RunMode::Script && service.command.is_none() {
                errors.push(format!(
                    "Service '{}' is in script mode but missing 'command' field",
                    name
                ));
            }

            // Validate cron expression
            if let Some(cron_expr) = &service.cron
                && let Err(e) = cron::Schedule::from_str(cron_expr)
            {
                errors.push(format!(
                    "Service '{}' has invalid cron expression '{}': {}",
                    name, cron_expr, e
                ));
            }

            // Validate CPU percentage
            if let Some(cpu) = service.cpu_max_percent
                && cpu > 100
            {
                errors.push(format!(
                    "Service '{}' has invalid cpu_max_percent {}, must be <= 100",
                    name, cpu
                ));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            run_mode: RunMode::Mise,
            task: None,
            command: None,
            enable: true,
            restart: RestartPolicy::No,
            restart_delay: "2s".to_string(),
            restart_limit: 10,
            restart_window: "60s".to_string(),
            stop_timeout: "10s".to_string(),
            workdir: None,
            timeout: "0".to_string(),
            ports: HashMap::new(),
            env: HashMap::new(),
            cron: None,
            cpu_max_percent: None,
            memory_max: None,
            pids_max: None,
            health_check: None,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_service() {
        let toml = r#"
            [services.web]
            task = "dev-server"
            enable = true
            restart = "always"
        "#;

        let config: SvcmgrConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.services.len(), 1);

        let web = &config.services["web"];
        assert_eq!(web.task, Some("dev-server".to_string()));
        assert!(web.enable);
        assert_eq!(web.restart, RestartPolicy::Always);
    }

    #[test]
    fn test_parse_service_with_resources() {
        let toml = r#"
            [services.api]
            task = "api-start"
            cpu_max_percent = 50
            memory_max = "512m"
            pids_max = 100
        "#;

        let config: SvcmgrConfig = toml::from_str(toml).unwrap();
        let api = &config.services["api"];

        assert_eq!(api.cpu_max_percent, Some(50));
        assert_eq!(api.memory_max, Some("512m".to_string()));
        assert_eq!(api.pids_max, Some(100));
    }

    #[test]
    fn test_parse_script_mode_service() {
        let toml = r#"
            [services.redis]
            run_mode = "script"
            command = "redis-server --port 6379"
            restart = "on-failure"
        "#;

        let config: SvcmgrConfig = toml::from_str(toml).unwrap();
        let redis = &config.services["redis"];

        assert_eq!(redis.run_mode, RunMode::Script);
        assert_eq!(redis.command, Some("redis-server --port 6379".to_string()));
        assert_eq!(redis.restart, RestartPolicy::OnFailure);
    }

    #[test]
    fn test_parse_credentials() {
        let toml = r#"
            [credentials.admin]
            type = "basic"
            username_secret = "admin_user"
            password_secret = "admin_pass"
            realm = "Admin Area"

            [credentials.api]
            type = "bearer"
            token_secret = "api_token"
        "#;

        let config: SvcmgrConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.credentials.len(), 2);

        match &config.credentials["admin"] {
            CredentialConfig::Basic {
                username_secret,
                password_secret,
                realm,
            } => {
                assert_eq!(username_secret, "admin_user");
                assert_eq!(password_secret, "admin_pass");
                assert_eq!(realm.as_deref(), Some("Admin Area"));
            }
            _ => panic!("Expected Basic credential"),
        }
    }

    #[test]
    fn test_validate_missing_task() {
        let svcmgr_config = SvcmgrConfig {
            services: {
                let mut map = HashMap::new();
                map.insert(
                    "web".to_string(),
                    ServiceConfig {
                        run_mode: RunMode::Mise,
                        task: Some("non-existent-task".to_string()),
                        ..Default::default()
                    },
                );
                map
            },
            ..Default::default()
        };

        let mise_config = MiseConfig {
            tasks: HashMap::new(),
            ..Default::default()
        };

        let result = svcmgr_config.validate(&mise_config);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors[0].contains("non-existent mise task"));
    }

    #[test]
    fn test_validate_invalid_cpu() {
        let svcmgr_config = SvcmgrConfig {
            services: {
                let mut map = HashMap::new();
                map.insert(
                    "api".to_string(),
                    ServiceConfig {
                        task: Some("api-start".to_string()),
                        cpu_max_percent: Some(150),
                        ..Default::default()
                    },
                );
                map
            },
            ..Default::default()
        };

        let mise_config = MiseConfig {
            tasks: {
                let mut map = HashMap::new();
                map.insert(
                    "api-start".to_string(),
                    MiseTask {
                        description: None,
                        run: "node server.js".to_string(),
                        depends: vec![],
                        env: HashMap::new(),
                        sources: vec![],
                        outputs: vec![],
                    },
                );
                map
            },
            ..Default::default()
        };

        let result = svcmgr_config.validate(&mise_config);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors[0].contains("cpu_max_percent"));
    }
}
