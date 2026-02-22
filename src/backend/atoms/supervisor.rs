#![allow(dead_code)]

/// Built-in process supervisor atom (replaces systemd)
///
/// This module provides a built-in process management capability
/// that works in Docker containers without requiring systemd:
/// - Service definition management (create/update/delete)
/// - Process lifecycle control (start/stop/restart/reload)
/// - Status query (active state, PID, memory, logs)
/// - Transient process support for temporary tasks
/// - In-memory log capture with ring buffer
use crate::error::{Error, Result};
use chrono::{DateTime, Utc};
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::RwLock;

// ========================================
// Data Structures
// ========================================

/// Service definition stored on disk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDef {
    pub name: String,
    pub description: String,
    pub command: String,
    pub args: Vec<String>,
    pub working_directory: Option<PathBuf>,
    pub env: HashMap<String, String>,
    pub restart_policy: RestartPolicy,
    pub enabled: bool,
}

/// Restart policy for supervised processes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RestartPolicy {
    /// Never restart on exit
    No,
    /// Always restart on exit
    Always,
    /// Restart only on non-zero exit code
    OnFailure,
}

/// Unit file content (compatible with old interface)
#[derive(Debug, Clone)]
pub struct UnitFile {
    pub name: String,
    pub path: PathBuf,
    pub content: String,
}

/// Unit info for listing
#[derive(Debug, Clone)]
pub struct UnitInfo {
    pub name: String,
    pub description: String,
    pub load_state: LoadState,
    pub active_state: ActiveState,
    pub sub_state: String,
    pub enabled: bool,
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
// Internal Runtime State
// ========================================

/// Runtime state for a managed process
struct ProcessState {
    child: Option<Child>,
    pid: Option<u32>,
    active_state: ActiveState,
    sub_state: String,
    started_at: Option<DateTime<Utc>>,
    logs: LogBuffer,
}

/// Ring buffer for captured log lines
struct LogBuffer {
    entries: Vec<LogEntry>,
    capacity: usize,
}

impl LogBuffer {
    fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
            capacity,
        }
    }

    fn push(&mut self, entry: LogEntry) {
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push(entry);
    }

    fn recent(&self, n: usize) -> Vec<LogEntry> {
        let start = self.entries.len().saturating_sub(n);
        self.entries[start..].to_vec()
    }

    fn query(&self, opts: &LogOptions) -> Vec<LogEntry> {
        let mut result: Vec<LogEntry> = self
            .entries
            .iter()
            .filter(|e| {
                if let Some(since) = opts.since
                    && e.timestamp < since
                {
                    return false;
                }
                if let Some(until) = opts.until
                    && e.timestamp > until
                {
                    return false;
                }
                if let Some(priority) = opts.priority
                    && (e.priority as u8) > (priority as u8)
                {
                    return false;
                }
                true
            })
            .cloned()
            .collect();

        if let Some(lines) = opts.lines {
            let start = result.len().saturating_sub(lines);
            result = result[start..].to_vec();
        }

        result
    }
}

// ========================================
// SupervisorAtom Trait
// ========================================

/// Built-in process supervisor trait (replaces SystemdAtom)
pub trait SupervisorAtom {
    /// Create a new service definition
    fn create_unit(
        &self,
        name: &str,
        content: &str,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Update an existing service definition
    fn update_unit(
        &self,
        name: &str,
        content: &str,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Delete a service definition (stops first)
    fn delete_unit(&self, name: &str) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Get service definition content
    fn get_unit(&self, name: &str) -> impl std::future::Future<Output = Result<UnitFile>> + Send;

    /// List all managed services
    fn list_units(&self) -> impl std::future::Future<Output = Result<Vec<UnitInfo>>> + Send;

    /// Start a service
    fn start(&self, name: &str) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Stop a service
    fn stop(&self, name: &str) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Restart a service
    fn restart(&self, name: &str) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Reload service configuration (re-read definition, restart if running)
    fn reload(&self, name: &str) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Enable service (auto-start)
    fn enable(&self, name: &str) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Disable service
    fn disable(&self, name: &str) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Get service status
    fn status(&self, name: &str) -> impl std::future::Future<Output = Result<UnitStatus>> + Send;

    /// Get process tree for a service
    fn process_tree(
        &self,
        name: &str,
    ) -> impl std::future::Future<Output = Result<ProcessTree>> + Send;

    /// Query logs with options
    fn logs(
        &self,
        name: &str,
        opts: &LogOptions,
    ) -> impl std::future::Future<Output = Result<Vec<LogEntry>>> + Send;

    /// Stream logs in real-time
    fn logs_stream(&self, name: &str) -> Result<Pin<Box<dyn Stream<Item = LogEntry> + Send>>>;

    /// Run a transient process (temporary task)
    fn run_transient(
        &self,
        opts: &TransientOptions,
    ) -> impl std::future::Future<Output = Result<TransientUnit>> + Send;

    /// List active transient processes
    fn list_transient(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<TransientUnit>>> + Send;

    /// Stop a transient process
    fn stop_transient(&self, name: &str) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Reload all service definitions from disk
    fn daemon_reload(&self) -> impl std::future::Future<Output = Result<()>> + Send;
}

// ========================================
// SupervisorManager Implementation
// ========================================

/// Built-in process supervisor manager
pub struct SupervisorManager {
    /// Directory for service definition files
    service_dir: PathBuf,
    /// Whether service files are tracked by git
    #[allow(dead_code)]
    git_managed: bool,
    /// Runtime process states (shared for async access)
    processes: Arc<RwLock<HashMap<String, ProcessState>>>,
    /// Log buffer capacity per service
    log_capacity: usize,
}

impl SupervisorManager {
    /// Create a new supervisor manager
    pub fn new(service_dir: PathBuf, git_managed: bool) -> Self {
        Self {
            service_dir,
            git_managed,
            processes: Arc::new(RwLock::new(HashMap::new())),
            log_capacity: 1000,
        }
    }

    /// Create with default configuration (~/.config/svcmgr/services)
    pub fn default_config() -> Result<Self> {
        let home = std::env::var("HOME")
            .map_err(|_| Error::Config("HOME environment variable not set".to_string()))?;
        let service_dir = PathBuf::from(home).join(".config/svcmgr/services");
        Ok(Self::new(service_dir, true))
    }

    /// Get full path for a service definition file
    fn service_path(&self, name: &str) -> PathBuf {
        self.service_dir.join(format!("{}.toml", name))
    }

    /// Ensure service directory exists
    async fn ensure_service_dir(&self) -> Result<()> {
        tokio::fs::create_dir_all(&self.service_dir).await?;
        Ok(())
    }

    /// Read a service definition from disk
    async fn read_service_def(&self, name: &str) -> Result<ServiceDef> {
        let path = self.service_path(name);
        let content = tokio::fs::read_to_string(&path).await?;
        let def: ServiceDef = toml::from_str(&content)
            .map_err(|e| Error::Config(format!("Invalid service definition: {}", e)))?;
        Ok(def)
    }

    /// Write a service definition to disk
    async fn write_service_def(&self, def: &ServiceDef) -> Result<()> {
        let path = self.service_path(&def.name);
        let content = toml::to_string_pretty(def)
            .map_err(|e| Error::Config(format!("Failed to serialize service definition: {}", e)))?;
        tokio::fs::write(&path, content).await?;
        Ok(())
    }

    /// Spawn a child process for a service definition
    async fn spawn_process(&self, def: &ServiceDef) -> Result<Child> {
        let mut cmd = Command::new(&def.command);
        cmd.args(&def.args);

        if let Some(ref wd) = def.working_directory {
            cmd.current_dir(wd);
        }

        for (k, v) in &def.env {
            cmd.env(k, v);
        }

        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        cmd.kill_on_drop(true);

        let child = cmd.spawn().map_err(|e| Error::CommandFailed {
            command: format!("{} {}", def.command, def.args.join(" ")),
            exit_code: None,
            stderr: e.to_string(),
        })?;

        Ok(child)
    }

    /// Start background log capture for a process
    fn start_log_capture(
        processes: Arc<RwLock<HashMap<String, ProcessState>>>,
        name: String,
        child: &mut Child,
    ) {
        // Capture stdout
        if let Some(stdout) = child.stdout.take() {
            let procs = processes.clone();
            let svc_name = name.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let entry = LogEntry {
                        timestamp: Utc::now(),
                        priority: LogPriority::Info,
                        message: line,
                        unit: svc_name.clone(),
                    };
                    let mut procs = procs.write().await;
                    if let Some(state) = procs.get_mut(&svc_name) {
                        state.logs.push(entry);
                    }
                }
            });
        }

        // Capture stderr
        if let Some(stderr) = child.stderr.take() {
            let procs = processes.clone();
            let svc_name = name;
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let entry = LogEntry {
                        timestamp: Utc::now(),
                        priority: LogPriority::Error,
                        message: line,
                        unit: svc_name.clone(),
                    };
                    let mut procs = procs.write().await;
                    if let Some(state) = procs.get_mut(&svc_name) {
                        state.logs.push(entry);
                    }
                }
            });
        }
    }

    /// List all service definition files on disk
    async fn list_service_files(&self) -> Result<Vec<String>> {
        let mut names = Vec::new();
        if !self.service_dir.exists() {
            return Ok(names);
        }
        let mut entries = tokio::fs::read_dir(&self.service_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "toml")
                && let Some(stem) = path.file_stem()
            {
                names.push(stem.to_string_lossy().to_string());
            }
        }
        Ok(names)
    }

    /// Check if a managed process is still alive, update state accordingly
    async fn refresh_process_state(&self, name: &str) {
        let mut procs = self.processes.write().await;
        if let Some(state) = procs.get_mut(name)
            && let Some(ref mut child) = state.child
        {
            match child.try_wait() {
                Ok(Some(exit_status)) => {
                    // Process has exited
                    state.active_state = if exit_status.success() {
                        ActiveState::Inactive
                    } else {
                        ActiveState::Failed
                    };
                    state.sub_state = "exited".to_string();
                    state.pid = None;
                    state.child = None;
                }
                Ok(None) => {
                    // Still running
                    state.active_state = ActiveState::Active;
                    state.sub_state = "running".to_string();
                }
                Err(_) => {
                    state.active_state = ActiveState::Failed;
                    state.sub_state = "error".to_string();
                    state.pid = None;
                    state.child = None;
                }
            }
        }
    }
}

impl SupervisorAtom for SupervisorManager {
    async fn create_unit(&self, name: &str, content: &str) -> Result<()> {
        self.ensure_service_dir().await?;
        let path = self.service_path(name);
        tokio::fs::write(&path, content).await?;
        Ok(())
    }

    async fn update_unit(&self, name: &str, content: &str) -> Result<()> {
        let path = self.service_path(name);
        if !path.exists() {
            return Err(Error::NotSupported(format!("Service {} not found", name)));
        }
        tokio::fs::write(&path, content).await?;
        Ok(())
    }

    async fn delete_unit(&self, name: &str) -> Result<()> {
        // Stop first
        let _ = self.stop(name).await;

        let path = self.service_path(name);
        if path.exists() {
            tokio::fs::remove_file(&path).await?;
        }

        // Clean up runtime state
        let mut procs = self.processes.write().await;
        procs.remove(name);

        Ok(())
    }

    async fn get_unit(&self, name: &str) -> Result<UnitFile> {
        let path = self.service_path(name);
        let content = tokio::fs::read_to_string(&path).await?;
        Ok(UnitFile {
            name: name.to_string(),
            path,
            content,
        })
    }

    async fn list_units(&self) -> Result<Vec<UnitInfo>> {
        let names = self.list_service_files().await?;
        let mut units = Vec::new();

        for name in names {
            let def = match self.read_service_def(&name).await {
                Ok(d) => d,
                Err(_) => continue,
            };

            self.refresh_process_state(&name).await;

            let procs = self.processes.read().await;
            let (active_state, sub_state) = procs
                .get(&name)
                .map(|s| (s.active_state, s.sub_state.clone()))
                .unwrap_or((ActiveState::Inactive, "dead".to_string()));

            units.push(UnitInfo {
                name: name.clone(),
                description: def.description.clone(),
                load_state: LoadState::Loaded,
                active_state,
                sub_state,
                enabled: def.enabled,
            });
        }

        Ok(units)
    }

    async fn start(&self, name: &str) -> Result<()> {
        let def = self.read_service_def(name).await?;

        // Check if already running
        self.refresh_process_state(name).await;
        {
            let procs = self.processes.read().await;
            if let Some(state) = procs.get(name)
                && state.active_state == ActiveState::Active
            {
                return Ok(()); // Already running
            }
        }

        let mut child = self.spawn_process(&def).await?;
        let pid = child.id();

        // Set up log capture
        Self::start_log_capture(self.processes.clone(), name.to_string(), &mut child);

        let state = ProcessState {
            child: Some(child),
            pid,
            active_state: ActiveState::Active,
            sub_state: "running".to_string(),
            started_at: Some(Utc::now()),
            logs: LogBuffer::new(self.log_capacity),
        };

        let mut procs = self.processes.write().await;
        procs.insert(name.to_string(), state);

        Ok(())
    }

    async fn stop(&self, name: &str) -> Result<()> {
        let mut procs = self.processes.write().await;
        if let Some(state) = procs.get_mut(name) {
            if let Some(ref mut child) = state.child {
                let _ = child.kill().await;
                let _ = child.wait().await;
            }
            state.child = None;
            state.pid = None;
            state.active_state = ActiveState::Inactive;
            state.sub_state = "dead".to_string();
        }
        Ok(())
    }

    async fn restart(&self, name: &str) -> Result<()> {
        self.stop(name).await?;
        self.start(name).await?;
        Ok(())
    }

    async fn reload(&self, name: &str) -> Result<()> {
        // Re-read definition; if running, restart with new config
        self.refresh_process_state(name).await;
        let is_running = {
            let procs = self.processes.read().await;
            procs
                .get(name)
                .is_some_and(|s| s.active_state == ActiveState::Active)
        };

        if is_running {
            self.restart(name).await?;
        }
        Ok(())
    }

    async fn enable(&self, name: &str) -> Result<()> {
        let mut def = self.read_service_def(name).await?;
        def.enabled = true;
        self.write_service_def(&def).await?;
        Ok(())
    }

    async fn disable(&self, name: &str) -> Result<()> {
        let mut def = self.read_service_def(name).await?;
        def.enabled = false;
        self.write_service_def(&def).await?;
        Ok(())
    }

    async fn status(&self, name: &str) -> Result<UnitStatus> {
        // Ensure definition exists
        let _def = self.read_service_def(name).await?;

        self.refresh_process_state(name).await;

        let procs = self.processes.read().await;
        if let Some(state) = procs.get(name) {
            Ok(UnitStatus {
                name: name.to_string(),
                active_state: state.active_state,
                sub_state: state.sub_state.clone(),
                pid: state.pid,
                memory: None, // TODO: read from /proc/{pid}/status
                cpu_time: None,
                started_at: state.started_at,
                recent_logs: state
                    .logs
                    .recent(10)
                    .iter()
                    .map(|e| e.message.clone())
                    .collect(),
            })
        } else {
            Ok(UnitStatus {
                name: name.to_string(),
                active_state: ActiveState::Inactive,
                sub_state: "dead".to_string(),
                pid: None,
                memory: None,
                cpu_time: None,
                started_at: None,
                recent_logs: Vec::new(),
            })
        }
    }

    async fn process_tree(&self, name: &str) -> Result<ProcessTree> {
        self.refresh_process_state(name).await;

        let procs = self.processes.read().await;
        let state = procs
            .get(name)
            .ok_or_else(|| Error::Other(format!("Service {} not found in runtime", name)))?;

        let root_pid = state
            .pid
            .ok_or_else(|| Error::Other(format!("Service {} has no running PID", name)))?;

        Ok(ProcessTree {
            root_pid,
            processes: vec![ProcessInfo {
                pid: root_pid,
                ppid: std::process::id(),
                name: name.to_string(),
                cmdline: String::new(),
            }],
        })
    }

    async fn logs(&self, name: &str, opts: &LogOptions) -> Result<Vec<LogEntry>> {
        let procs = self.processes.read().await;
        if let Some(state) = procs.get(name) {
            Ok(state.logs.query(opts))
        } else {
            Ok(Vec::new())
        }
    }

    fn logs_stream(&self, _name: &str) -> Result<Pin<Box<dyn Stream<Item = LogEntry> + Send>>> {
        // TODO: Implement real-time log streaming via tokio broadcast
        Err(Error::NotSupported(
            "Log streaming not yet implemented".to_string(),
        ))
    }

    async fn run_transient(&self, opts: &TransientOptions) -> Result<TransientUnit> {
        if opts.command.is_empty() {
            return Err(Error::InvalidArgument(
                "Transient command cannot be empty".to_string(),
            ));
        }

        let program = &opts.command[0];
        let args = &opts.command[1..];

        let mut cmd = Command::new(program);
        cmd.args(args);

        for (k, v) in &opts.env {
            cmd.env(k, v);
        }

        if let Some(ref wd) = opts.working_directory {
            cmd.current_dir(wd);
        }

        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        cmd.kill_on_drop(opts.collect);

        let mut child = cmd.spawn().map_err(|e| Error::CommandFailed {
            command: opts.command.join(" "),
            exit_code: None,
            stderr: e.to_string(),
        })?;

        let pid = child.id();

        // Set up log capture for transient
        Self::start_log_capture(self.processes.clone(), opts.name.clone(), &mut child);

        let state = ProcessState {
            child: Some(child),
            pid,
            active_state: ActiveState::Active,
            sub_state: "running".to_string(),
            started_at: Some(Utc::now()),
            logs: LogBuffer::new(self.log_capacity),
        };

        let mut procs = self.processes.write().await;
        procs.insert(opts.name.clone(), state);

        Ok(TransientUnit {
            name: opts.name.clone(),
            pid,
            started_at: Utc::now(),
        })
    }

    async fn list_transient(&self) -> Result<Vec<TransientUnit>> {
        let service_names = self.list_service_files().await?;
        let procs = self.processes.read().await;

        // Transient units are those in runtime state but not on disk
        Ok(procs
            .iter()
            .filter(|(name, _)| !service_names.contains(name))
            .map(|(name, state)| TransientUnit {
                name: name.clone(),
                pid: state.pid,
                started_at: state.started_at.unwrap_or_else(Utc::now),
            })
            .collect())
    }

    async fn stop_transient(&self, name: &str) -> Result<()> {
        self.stop(name).await
    }

    async fn daemon_reload(&self) -> Result<()> {
        // Re-scan service directory; no external daemon to reload
        let _ = self.list_service_files().await?;
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
    async fn test_supervisor_manager_creation() {
        let tmpdir = std::env::temp_dir().join("svcmgr-test-supervisor");
        let manager = SupervisorManager::new(tmpdir.clone(), false);
        assert_eq!(manager.service_dir, tmpdir);
        assert!(!manager.git_managed);
    }

    #[tokio::test]
    async fn test_service_path_generation() {
        let tmpdir = std::env::temp_dir().join("svcmgr-test-supervisor");
        let manager = SupervisorManager::new(tmpdir.clone(), false);
        let path = manager.service_path("test-service");
        assert_eq!(path, tmpdir.join("test-service.toml"));
    }

    #[test]
    fn test_log_buffer_push_and_recent() {
        let mut buf = LogBuffer::new(3);
        for i in 0..5 {
            buf.push(LogEntry {
                timestamp: Utc::now(),
                priority: LogPriority::Info,
                message: format!("msg{}", i),
                unit: "test".to_string(),
            });
        }
        let recent = buf.recent(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].message, "msg3");
        assert_eq!(recent[1].message, "msg4");
    }

    #[test]
    fn test_log_buffer_capacity() {
        let mut buf = LogBuffer::new(2);
        buf.push(LogEntry {
            timestamp: Utc::now(),
            priority: LogPriority::Info,
            message: "a".to_string(),
            unit: "t".to_string(),
        });
        buf.push(LogEntry {
            timestamp: Utc::now(),
            priority: LogPriority::Info,
            message: "b".to_string(),
            unit: "t".to_string(),
        });
        buf.push(LogEntry {
            timestamp: Utc::now(),
            priority: LogPriority::Info,
            message: "c".to_string(),
            unit: "t".to_string(),
        });
        assert_eq!(buf.entries.len(), 2);
        assert_eq!(buf.entries[0].message, "b");
        assert_eq!(buf.entries[1].message, "c");
    }

    #[test]
    fn test_log_buffer_query_with_lines() {
        let mut buf = LogBuffer::new(10);
        for i in 0..5 {
            buf.push(LogEntry {
                timestamp: Utc::now(),
                priority: LogPriority::Info,
                message: format!("msg{}", i),
                unit: "test".to_string(),
            });
        }
        let opts = LogOptions {
            lines: Some(2),
            ..Default::default()
        };
        let result = buf.query(&opts);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].message, "msg3");
        assert_eq!(result[1].message, "msg4");
    }

    #[test]
    fn test_service_def_serialization() {
        let def = ServiceDef {
            name: "test-svc".to_string(),
            description: "Test service".to_string(),
            command: "/usr/bin/sleep".to_string(),
            args: vec!["infinity".to_string()],
            working_directory: None,
            env: HashMap::new(),
            restart_policy: RestartPolicy::OnFailure,
            enabled: true,
        };

        let toml_str = toml::to_string_pretty(&def).unwrap();
        assert!(toml_str.contains("name = \"test-svc\""));
        assert!(toml_str.contains("restart_policy = \"OnFailure\""));

        let parsed: ServiceDef = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.name, "test-svc");
        assert_eq!(parsed.restart_policy, RestartPolicy::OnFailure);
    }

    #[tokio::test]
    async fn test_create_and_get_unit() {
        let tmpdir = tempfile::tempdir().unwrap();
        let manager = SupervisorManager::new(tmpdir.path().to_path_buf(), false);

        let content = r#"
name = "hello"
description = "Hello service"
command = "echo"
args = ["hello"]
restart_policy = "No"
enabled = true
"#;

        manager.create_unit("hello", content).await.unwrap();
        let unit = manager.get_unit("hello").await.unwrap();
        assert_eq!(unit.name, "hello");
        assert!(unit.content.contains("Hello service"));
    }

    #[tokio::test]
    async fn test_delete_unit() {
        let tmpdir = tempfile::tempdir().unwrap();
        let manager = SupervisorManager::new(tmpdir.path().to_path_buf(), false);

        let content = r#"
name = "to-delete"
description = "Will be deleted"
command = "echo"
args = []
restart_policy = "No"
enabled = false
"#;

        manager.create_unit("to-delete", content).await.unwrap();
        assert!(manager.service_path("to-delete").exists());

        manager.delete_unit("to-delete").await.unwrap();
        assert!(!manager.service_path("to-delete").exists());
    }

    #[tokio::test]
    async fn test_list_units_empty() {
        let tmpdir = tempfile::tempdir().unwrap();
        let manager = SupervisorManager::new(tmpdir.path().to_path_buf(), false);

        let units = manager.list_units().await.unwrap();
        assert!(units.is_empty());
    }

    #[tokio::test]
    async fn test_enable_disable() {
        let tmpdir = tempfile::tempdir().unwrap();
        let manager = SupervisorManager::new(tmpdir.path().to_path_buf(), false);

        let def = ServiceDef {
            name: "svc".to_string(),
            description: "Test".to_string(),
            command: "echo".to_string(),
            args: vec![],
            working_directory: None,
            env: HashMap::new(),
            restart_policy: RestartPolicy::No,
            enabled: false,
        };
        manager.write_service_def(&def).await.unwrap();

        manager.enable("svc").await.unwrap();
        let updated = manager.read_service_def("svc").await.unwrap();
        assert!(updated.enabled);

        manager.disable("svc").await.unwrap();
        let updated = manager.read_service_def("svc").await.unwrap();
        assert!(!updated.enabled);
    }

    #[tokio::test]
    async fn test_status_not_running() {
        let tmpdir = tempfile::tempdir().unwrap();
        let manager = SupervisorManager::new(tmpdir.path().to_path_buf(), false);

        let def = ServiceDef {
            name: "idle".to_string(),
            description: "Idle service".to_string(),
            command: "echo".to_string(),
            args: vec![],
            working_directory: None,
            env: HashMap::new(),
            restart_policy: RestartPolicy::No,
            enabled: false,
        };
        manager.write_service_def(&def).await.unwrap();

        let status = manager.status("idle").await.unwrap();
        assert_eq!(status.active_state, ActiveState::Inactive);
        assert!(status.pid.is_none());
    }

    #[test]
    fn test_active_state_equality() {
        assert_eq!(ActiveState::Active, ActiveState::Active);
        assert_ne!(ActiveState::Active, ActiveState::Inactive);
        assert_eq!(ActiveState::Failed, ActiveState::Failed);
    }

    #[test]
    fn test_load_state_equality() {
        assert_eq!(LoadState::Loaded, LoadState::Loaded);
        assert_ne!(LoadState::Loaded, LoadState::NotFound);
    }

    #[test]
    fn test_restart_policy_serde() {
        assert_eq!(
            serde_json::to_string(&RestartPolicy::Always).unwrap(),
            "\"Always\""
        );
        assert_eq!(
            serde_json::to_string(&RestartPolicy::OnFailure).unwrap(),
            "\"OnFailure\""
        );
        assert_eq!(serde_json::to_string(&RestartPolicy::No).unwrap(), "\"No\"");
    }
}
