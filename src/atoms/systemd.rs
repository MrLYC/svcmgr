#![allow(dead_code)]

/// Systemd service management atom
/// 
/// This module provides systemd user service management capabilities:
/// - Unit file management (create/update/delete)
/// - Service lifecycle control (start/stop/restart/reload)
/// - Status query (active state, PID, memory, logs)
/// - Transient units for temporary tasks
/// - Journal log query with time filtering
use crate::error::{Error, Result};
use chrono::{DateTime, Utc};
use futures::stream::Stream;
use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::Command;
use std::time::Duration;
use tokio::fs;

// ========================================
// Data Structures
// ========================================

/// Unit file information
#[derive(Debug, Clone)]
pub struct UnitInfo {
    pub name: String,
    pub description: String,
    pub load_state: LoadState,
    pub active_state: ActiveState,
    pub sub_state: String,
    pub enabled: bool,
}

/// Unit file content (read from disk)
#[derive(Debug, Clone)]
pub struct UnitFile {
    pub name: String,
    pub path: PathBuf,
    pub content: String,
}

/// Unit status with resource usage
#[derive(Debug, Clone)]
pub struct UnitStatus {
    pub name: String,
    pub active_state: ActiveState,
    pub sub_state: String,
    pub pid: Option<u32>,
    pub memory: Option<u64>,
    pub cpu_time: Option<Duration>,
    pub started_at: Option<DateTime<Utc>>,
    pub recent_logs: Vec<String>,
}

/// Process tree for a service
#[derive(Debug, Clone)]
pub struct ProcessTree {
    pub root_pid: u32,
    pub processes: Vec<ProcessInfo>,
}

/// Process information
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub ppid: u32,
    pub name: String,
    pub cmdline: String,
}

/// Transient unit configuration
#[derive(Debug, Clone)]
pub struct TransientOptions {
    pub name: String,
    pub command: Vec<String>,
    pub scope: bool,
    pub remain_after_exit: bool,
    pub collect: bool,
    pub env: HashMap<String, String>,
    pub working_directory: Option<PathBuf>,
}

/// Transient unit handle
#[derive(Debug, Clone)]
pub struct TransientUnit {
    pub name: String,
    pub pid: Option<u32>,
    pub started_at: DateTime<Utc>,
}

/// Log query options
#[derive(Debug, Clone, Default)]
pub struct LogOptions {
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
    pub lines: Option<usize>,
    pub priority: Option<LogPriority>,
}

/// Journal log entry
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub priority: LogPriority,
    pub message: String,
    pub unit: String,
}

/// Log priority levels (syslog compatible)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogPriority {
    Emergency = 0,
    Alert = 1,
    Critical = 2,
    Error = 3,
    Warning = 4,
    Notice = 5,
    Info = 6,
    Debug = 7,
}

/// Unit load state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadState {
    Loaded,
    NotFound,
    BadSetting,
    Error,
    Masked,
}

/// Unit active state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveState {
    Active,
    Inactive,
    Activating,
    Deactivating,
    Failed,
    Reloading,
}

// ========================================
// SystemdAtom Trait
// ========================================

/// Systemd user service management trait
pub trait SystemdAtom {
    /// Create a new unit file
    fn create_unit(&self, name: &str, content: &str) -> impl std::future::Future<Output = Result<()>> + Send;
    
    /// Update an existing unit file
    fn update_unit(&self, name: &str, content: &str) -> impl std::future::Future<Output = Result<()>> + Send;
    
    /// Delete a unit file (stops and disables first)
    fn delete_unit(&self, name: &str) -> impl std::future::Future<Output = Result<()>> + Send;
    
    /// Get unit file content
    fn get_unit(&self, name: &str) -> impl std::future::Future<Output = Result<UnitFile>> + Send;
    
    /// List all managed units
    fn list_units(&self) -> impl std::future::Future<Output = Result<Vec<UnitInfo>>> + Send;
    
    /// Start a service
    fn start(&self, name: &str) -> impl std::future::Future<Output = Result<()>> + Send;
    
    /// Stop a service
    fn stop(&self, name: &str) -> impl std::future::Future<Output = Result<()>> + Send;
    
    /// Restart a service
    fn restart(&self, name: &str) -> impl std::future::Future<Output = Result<()>> + Send;
    
    /// Reload service configuration
    fn reload(&self, name: &str) -> impl std::future::Future<Output = Result<()>> + Send;
    
    /// Enable service (auto-start)
    fn enable(&self, name: &str) -> impl std::future::Future<Output = Result<()>> + Send;
    
    /// Disable service
    fn disable(&self, name: &str) -> impl std::future::Future<Output = Result<()>> + Send;
    
    /// Get service status
    fn status(&self, name: &str) -> impl std::future::Future<Output = Result<UnitStatus>> + Send;
    
    /// Get process tree for a service
    fn process_tree(&self, name: &str) -> impl std::future::Future<Output = Result<ProcessTree>> + Send;
    
    /// Query logs with options
    fn logs(&self, name: &str, opts: &LogOptions) -> impl std::future::Future<Output = Result<Vec<LogEntry>>> + Send;
    
    /// Stream logs in real-time
    fn logs_stream(&self, name: &str) -> Result<Pin<Box<dyn Stream<Item = LogEntry> + Send>>>;
    
    /// Run a transient unit (temporary task)
    fn run_transient(&self, opts: &TransientOptions) -> impl std::future::Future<Output = Result<TransientUnit>> + Send;
    
    /// List active transient units
    fn list_transient(&self) -> impl std::future::Future<Output = Result<Vec<TransientUnit>>> + Send;
    
    /// Stop a transient unit
    fn stop_transient(&self, name: &str) -> impl std::future::Future<Output = Result<()>> + Send;
    
    /// Reload systemd daemon configuration
    fn daemon_reload(&self) -> impl std::future::Future<Output = Result<()>> + Send;
}

// ========================================
// SystemdManager Implementation
// ========================================

/// Systemd manager using systemctl --user
pub struct SystemdManager {
    unit_dir: PathBuf,
    #[allow(dead_code)]
    git_managed: bool,
}

impl SystemdManager {
    /// Create a new systemd manager
    pub fn new(unit_dir: PathBuf, git_managed: bool) -> Self {
        Self {
            unit_dir,
            git_managed,
        }
    }

    /// Create with default configuration (~/.config/systemd/user)
    pub fn default_config() -> Result<Self> {
        let home = std::env::var("HOME")
            .map_err(|_| Error::Config("HOME environment variable not set".to_string()))?;
        let unit_dir = PathBuf::from(home).join(".config/systemd/user");
        Ok(Self::new(unit_dir, true))
    }

    /// Run systemctl --user command
    fn run_systemctl(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("systemctl")
            .arg("--user")
            .args(args)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::CommandFailed {
                command: format!("systemctl --user {}", args.join(" ")),
                exit_code: output.status.code(),
                stderr: stderr.to_string(),
            });
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Run journalctl --user command
    fn run_journalctl(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("journalctl")
            .arg("--user")
            .args(args)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::CommandFailed {
                command: format!("journalctl --user {}", args.join(" ")),
                exit_code: output.status.code(),
                stderr: stderr.to_string(),
            });
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Get full path for a unit file
    fn unit_path(&self, name: &str) -> PathBuf {
        self.unit_dir.join(format!("{}.service", name))
    }

    /// Ensure unit directory exists
    async fn ensure_unit_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.unit_dir).await?;
        Ok(())
    }

    /// Parse systemctl list-units output
    fn parse_unit_list(&self, output: &str) -> Vec<UnitInfo> {
        output
            .lines()
            .skip(1) // Skip header
            .filter(|line| !line.trim().is_empty() && !line.contains("loaded units listed"))
            .filter_map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 5 {
                    Some(UnitInfo {
                        name: parts[0].to_string(),
                        load_state: Self::parse_load_state(parts[1]),
                        active_state: Self::parse_active_state(parts[2]),
                        sub_state: parts[3].to_string(),
                        description: parts[4..].join(" "),
                        enabled: false, // TODO: query enable status separately
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /// Parse load state string
    fn parse_load_state(s: &str) -> LoadState {
        match s {
            "loaded" => LoadState::Loaded,
            "not-found" => LoadState::NotFound,
            "bad-setting" => LoadState::BadSetting,
            "error" => LoadState::Error,
            "masked" => LoadState::Masked,
            _ => LoadState::Error,
        }
    }

    /// Parse active state string
    fn parse_active_state(s: &str) -> ActiveState {
        match s {
            "active" => ActiveState::Active,
            "inactive" => ActiveState::Inactive,
            "activating" => ActiveState::Activating,
            "deactivating" => ActiveState::Deactivating,
            "failed" => ActiveState::Failed,
            "reloading" => ActiveState::Reloading,
            _ => ActiveState::Inactive,
        }
    }

    /// Parse systemctl status output
    fn parse_status(&self, name: &str, output: &str) -> Result<UnitStatus> {
        let mut status = UnitStatus {
            name: name.to_string(),
            active_state: ActiveState::Inactive,
            sub_state: String::new(),
            pid: None,
            memory: None,
            cpu_time: None,
            started_at: None,
            recent_logs: Vec::new(),
        };

        for line in output.lines() {
            let line = line.trim();
            
            if line.starts_with("Active:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    status.active_state = Self::parse_active_state(parts[1]);
                    if parts.len() >= 3 {
                        status.sub_state = parts[2].trim_matches('(').trim_matches(')').to_string();
                    }
                }
            } else if line.starts_with("Main PID:") {
                if let Some(pid_str) = line.split_whitespace().nth(2) {
                    status.pid = pid_str.parse().ok();
                }
            } else if line.starts_with("Memory:")
                && let Some(mem_str) = line.split_whitespace().nth(1)
            {
                // Parse memory size (e.g., "4.5M" -> bytes)
                status.memory = Self::parse_memory_size(mem_str);
            }
        }

        Ok(status)
    }

    /// Parse memory size string (e.g., "4.5M" -> bytes)
    fn parse_memory_size(s: &str) -> Option<u64> {
        let s = s.trim();
        let (num_str, unit) = if let Some(stripped) = s.strip_suffix('K') {
            (stripped, 1024)
        } else if let Some(stripped) = s.strip_suffix('M') {
            (stripped, 1024 * 1024)
        } else if let Some(stripped) = s.strip_suffix('G') {
            (stripped, 1024 * 1024 * 1024)
        } else {
            (s, 1)
        };

        num_str.parse::<f64>().ok().map(|n| (n * unit as f64) as u64)
    }

    /// Parse journalctl output into log entries
    fn parse_logs(&self, unit: &str, output: &str) -> Vec<LogEntry> {
        output
            .lines()
            .filter(|line| !line.trim().is_empty())
            .filter_map(|line| {
                // Simple parsing: timestamp + priority + message
                // Real journalctl uses JSON format for structured data
                let parts: Vec<&str> = line.splitn(3, ' ').collect();
                if parts.len() >= 3 {
                    Some(LogEntry {
                        timestamp: Utc::now(), // TODO: parse actual timestamp
                        priority: LogPriority::Info,
                        message: parts[2].to_string(),
                        unit: unit.to_string(),
                    })
                } else {
                    None
                }
            })
            .collect()
    }
}

impl SystemdAtom for SystemdManager {
    async fn create_unit(&self, name: &str, content: &str) -> Result<()> {
        self.ensure_unit_dir().await?;
        let path = self.unit_path(name);
        fs::write(&path, content).await?;
        self.daemon_reload().await?;
        Ok(())
    }

    async fn update_unit(&self, name: &str, content: &str) -> Result<()> {
        let path = self.unit_path(name);
        if !path.exists() {
            return Err(Error::NotSupported(format!("Unit {} not found", name)));
        }
        fs::write(&path, content).await?;
        self.daemon_reload().await?;
        Ok(())
    }

    async fn delete_unit(&self, name: &str) -> Result<()> {
        // Stop and disable first
        let _ = self.stop(name).await;
        let _ = self.disable(name).await;
        
        let path = self.unit_path(name);
        if path.exists() {
            fs::remove_file(&path).await?;
        }
        
        self.daemon_reload().await?;
        Ok(())
    }

    async fn get_unit(&self, name: &str) -> Result<UnitFile> {
        let path = self.unit_path(name);
        let content = fs::read_to_string(&path).await?;
        Ok(UnitFile {
            name: name.to_string(),
            path,
            content,
        })
    }

    async fn list_units(&self) -> Result<Vec<UnitInfo>> {
        let output = self.run_systemctl(&["list-units", "--all", "--no-pager"])?;
        Ok(self.parse_unit_list(&output))
    }

    async fn start(&self, name: &str) -> Result<()> {
        self.run_systemctl(&["start", name])?;
        Ok(())
    }

    async fn stop(&self, name: &str) -> Result<()> {
        self.run_systemctl(&["stop", name])?;
        Ok(())
    }

    async fn restart(&self, name: &str) -> Result<()> {
        self.run_systemctl(&["restart", name])?;
        Ok(())
    }

    async fn reload(&self, name: &str) -> Result<()> {
        self.run_systemctl(&["reload", name])?;
        Ok(())
    }

    async fn enable(&self, name: &str) -> Result<()> {
        self.run_systemctl(&["enable", name])?;
        Ok(())
    }

    async fn disable(&self, name: &str) -> Result<()> {
        self.run_systemctl(&["disable", name])?;
        Ok(())
    }

    async fn status(&self, name: &str) -> Result<UnitStatus> {
        let output = self.run_systemctl(&["status", name])?;
        self.parse_status(name, &output)
    }

    async fn process_tree(&self, name: &str) -> Result<ProcessTree> {
        // Get main PID first
        let status = self.status(name).await?;
        let root_pid = status.pid.ok_or_else(|| {
            Error::Other(format!("Service {} has no main PID", name))
        })?;

        // Use pstree or ps to get process tree
        // For now, return simple tree with root process only
        Ok(ProcessTree {
            root_pid,
            processes: vec![ProcessInfo {
                pid: root_pid,
                ppid: 1,
                name: name.to_string(),
                cmdline: String::new(),
            }],
        })
    }

    async fn logs(&self, name: &str, opts: &LogOptions) -> Result<Vec<LogEntry>> {
        let mut args = vec!["-u", name, "--no-pager"];
        
        let since_str;
        let until_str;
        let lines_str;
        
        if let Some(since) = opts.since {
            since_str = since.to_rfc3339();
            args.push("--since");
            args.push(&since_str);
        }
        
        if let Some(until) = opts.until {
            until_str = until.to_rfc3339();
            args.push("--until");
            args.push(&until_str);
        }
        
        if let Some(lines) = opts.lines {
            lines_str = format!("{}", lines);
            args.push("-n");
            args.push(&lines_str);
        }
        
        let output = self.run_journalctl(&args)?;
        Ok(self.parse_logs(name, &output))
    }

    fn logs_stream(&self, _name: &str) -> Result<Pin<Box<dyn Stream<Item = LogEntry> + Send>>> {
        // TODO: Implement real-time log streaming using journalctl -f
        Err(Error::NotSupported("Log streaming not yet implemented".to_string()))
    }

    async fn run_transient(&self, opts: &TransientOptions) -> Result<TransientUnit> {
        let mut args = vec!["--user", "--unit", &opts.name];
        
        if opts.scope {
            args.push("--scope");
        }
        
        if opts.remain_after_exit {
            args.push("--remain-after-exit");
        }
        
        if opts.collect {
            args.push("--collect");
        }
        
        // Add environment variables
        let env_args: Vec<String> = opts.env.iter()
            .map(|(k, v)| format!("--setenv={}={}", k, v))
            .collect();
        let env_arg_refs: Vec<&str> = env_args.iter().map(|s| s.as_str()).collect();
        args.extend(env_arg_refs);
        
        // Add working directory
        let wd_arg;
        if let Some(ref wd) = opts.working_directory {
            wd_arg = format!("--working-directory={}", wd.display());
            args.push(&wd_arg);
        }
        
        // Add command
        args.push("--");
        let cmd_refs: Vec<&str> = opts.command.iter().map(|s| s.as_str()).collect();
        args.extend(cmd_refs);
        
        let output = Command::new("systemd-run")
            .args(&args)
            .output()?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::CommandFailed {
                command: format!("systemd-run {}", args.join(" ")),
                exit_code: output.status.code(),
                stderr: stderr.to_string(),
            });
        }
        
        Ok(TransientUnit {
            name: opts.name.clone(),
            pid: None,
            started_at: Utc::now(),
        })
    }

    async fn list_transient(&self) -> Result<Vec<TransientUnit>> {
        // List all units and filter transient ones
        let output = self.run_systemctl(&["list-units", "--all", "--no-pager"])?;
        let units = self.parse_unit_list(&output);
        
        // Transient units typically have .scope or .service suffix and are runtime-only
        Ok(units.iter()
            .filter(|u| u.name.contains("run-"))
            .map(|u| TransientUnit {
                name: u.name.clone(),
                pid: None,
                started_at: Utc::now(), // TODO: parse actual start time
            })
            .collect())
    }

    async fn stop_transient(&self, name: &str) -> Result<()> {
        self.stop(name).await
    }

    async fn daemon_reload(&self) -> Result<()> {
        self.run_systemctl(&["daemon-reload"])?;
        Ok(())
    }
}

// ========================================
// Unit Tests
// ========================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_systemd_manager_creation() {
        let tmpdir = std::env::temp_dir().join("svcmgr-test-systemd");
        let manager = SystemdManager::new(tmpdir.clone(), false);
        assert_eq!(manager.unit_dir, tmpdir);
        assert!(!manager.git_managed);
    }

    #[tokio::test]
    async fn test_unit_path_generation() {
        let tmpdir = std::env::temp_dir().join("svcmgr-test-systemd");
        let manager = SystemdManager::new(tmpdir.clone(), false);
        let path = manager.unit_path("test-service");
        assert_eq!(path, tmpdir.join("test-service.service"));
    }

    #[test]
    fn test_parse_load_state() {
        assert_eq!(
            SystemdManager::parse_load_state("loaded"),
            LoadState::Loaded
        );
        assert_eq!(
            SystemdManager::parse_load_state("not-found"),
            LoadState::NotFound
        );
        assert_eq!(
            SystemdManager::parse_load_state("masked"),
            LoadState::Masked
        );
    }

    #[test]
    fn test_parse_active_state() {
        assert_eq!(
            SystemdManager::parse_active_state("active"),
            ActiveState::Active
        );
        assert_eq!(
            SystemdManager::parse_active_state("inactive"),
            ActiveState::Inactive
        );
        assert_eq!(
            SystemdManager::parse_active_state("failed"),
            ActiveState::Failed
        );
    }

    #[test]
    fn test_parse_memory_size() {
        assert_eq!(SystemdManager::parse_memory_size("1024"), Some(1024));
        assert_eq!(SystemdManager::parse_memory_size("4K"), Some(4 * 1024));
        assert_eq!(SystemdManager::parse_memory_size("2.5M"), Some((2.5 * 1024.0 * 1024.0) as u64));
        assert_eq!(SystemdManager::parse_memory_size("1.2G"), Some((1.2 * 1024.0 * 1024.0 * 1024.0) as u64));
    }

    #[test]
    fn test_parse_unit_list() {
        let tmpdir = std::env::temp_dir().join("svcmgr-test-systemd");
        let manager = SystemdManager::new(tmpdir, false);
        
        let output = r#"UNIT                    LOAD   ACTIVE SUB     DESCRIPTION
test.service            loaded active running Test Service
another.service         loaded inactive dead    Another Service

2 loaded units listed."#;
        
        let units = manager.parse_unit_list(output);
        assert_eq!(units.len(), 2);
        assert_eq!(units[0].name, "test.service");
        assert_eq!(units[0].load_state, LoadState::Loaded);
        assert_eq!(units[0].active_state, ActiveState::Active);
        assert_eq!(units[1].name, "another.service");
        assert_eq!(units[1].active_state, ActiveState::Inactive);
    }

    /// 测试 parse_memory_size 无效输入
    #[test]
    fn test_parse_memory_size_invalid() {
        assert_eq!(SystemdManager::parse_memory_size("invalid"), None);
        assert_eq!(SystemdManager::parse_memory_size(""), None);
    }

    /// 测试 parse_load_state 覆盖所有枚举值
    #[test]
    fn test_parse_load_state_all_variants() {
        assert_eq!(SystemdManager::parse_load_state("loaded"), LoadState::Loaded);
        assert_eq!(SystemdManager::parse_load_state("not-found"), LoadState::NotFound);
        assert_eq!(SystemdManager::parse_load_state("error"), LoadState::Error);
        assert_eq!(SystemdManager::parse_load_state("masked"), LoadState::Masked);
        assert_eq!(SystemdManager::parse_load_state("bad-setting"), LoadState::BadSetting);
        assert_eq!(SystemdManager::parse_load_state("unknown_state"), LoadState::Error);
    }

    /// 测试 parse_active_state 覆盖所有枚举值
    #[test]
    fn test_parse_active_state_all_variants() {
        assert_eq!(SystemdManager::parse_active_state("active"), ActiveState::Active);
        assert_eq!(SystemdManager::parse_active_state("inactive"), ActiveState::Inactive);
        assert_eq!(SystemdManager::parse_active_state("failed"), ActiveState::Failed);
        assert_eq!(SystemdManager::parse_active_state("activating"), ActiveState::Activating);
        assert_eq!(SystemdManager::parse_active_state("deactivating"), ActiveState::Deactivating);
        assert_eq!(SystemdManager::parse_active_state("reloading"), ActiveState::Reloading);
        assert_eq!(SystemdManager::parse_active_state("unknown_state"), ActiveState::Inactive);
    }

    /// 测试 parse_unit_list 空输出
    #[test]
    fn test_parse_unit_list_empty() {
        let tmpdir = std::env::temp_dir().join("svcmgr-test-systemd");
        let manager = SystemdManager::new(tmpdir, false);
        
        let units = manager.parse_unit_list("");
        assert_eq!(units.len(), 0);
        
        let units = manager.parse_unit_list("UNIT LOAD ACTIVE SUB DESCRIPTION");
        assert_eq!(units.len(), 0);
    }
}
