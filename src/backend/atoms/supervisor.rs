#![allow(dead_code)]

/// Built-in process supervisor atom
///
/// A lightweight supervisor (similar to Python's supervisord) that provides:
/// - Process group management via setsid (each child is a process group leader)
/// - Graceful shutdown: SIGTERM -> wait -> SIGKILL the entire process group
/// - Auto-restart with configurable policies and backoff
/// - In-memory log capture (stdout/stderr ring buffer)
/// - Cron-based periodic task scheduling (replaces crontab)
/// - Service definition persistence via TOML files
///
/// This single unified module replaces both systemd and crontab atoms
/// for better Docker container compatibility.
use crate::error::{Error, Result};
use chrono::{DateTime, Utc};
use cron::Schedule;
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::RwLock;

// ========================================
// Data Structures - Service (long-running)
// ========================================

/// Service definition stored on disk as TOML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDef {
    pub name: String,
    pub description: String,
    /// Executable path or command
    pub command: String,
    /// Command arguments
    #[serde(default)]
    pub args: Vec<String>,
    /// Working directory (optional)
    pub working_directory: Option<PathBuf>,
    /// Environment variables
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Restart policy
    #[serde(default)]
    pub restart_policy: RestartPolicy,
    /// Seconds to wait before restart
    #[serde(default = "default_restart_sec")]
    pub restart_sec: u64,
    /// Whether the service is enabled (auto-start)
    #[serde(default)]
    pub enabled: bool,
    /// Seconds to wait for graceful SIGTERM before SIGKILL
    #[serde(default = "default_stop_timeout")]
    pub stop_timeout_sec: u64,
}

fn default_restart_sec() -> u64 {
    1
}
fn default_stop_timeout() -> u64 {
    10
}

/// Restart policy for supervised processes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RestartPolicy {
    /// Never restart on exit
    #[default]
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
// Data Structures - Cron (periodic tasks)
// ========================================

/// Cron task definition
#[derive(Debug, Clone)]
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

/// Persistent cron task store (TOML on disk)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct TaskStore {
    #[serde(default)]
    env: HashMap<String, String>,
    #[serde(default)]
    tasks: Vec<TaskEntry>,
}

/// Single task entry in the store
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TaskEntry {
    id: String,
    description: String,
    expression: String,
    command: String,
    #[serde(default)]
    env: HashMap<String, String>,
    #[serde(default = "default_enabled")]
    enabled: bool,
}

fn default_enabled() -> bool {
    true
}

// ========================================
// Internal Runtime State
// ========================================

/// Runtime state for a managed process
struct ProcessState {
    /// Tokio child handle (None if exited)
    child: Option<tokio::process::Child>,
    /// Main PID (also serves as PGID thanks to setsid)
    pid: Option<u32>,
    active_state: ActiveState,
    sub_state: String,
    started_at: Option<DateTime<Utc>>,
    logs: LogBuffer,
    /// Restart policy from definition
    restart_policy: RestartPolicy,
    /// Seconds to wait before restart
    restart_sec: u64,
    /// Seconds to wait for SIGTERM before SIGKILL
    stop_timeout_sec: u64,
    /// Whether the process is being intentionally stopped (skip auto-restart)
    stopping: bool,
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
// SupervisorAtom Trait (service management)
// ========================================

/// Built-in process supervisor trait (replaces SystemdAtom)
///
/// Manages long-running services with process group isolation,
/// graceful shutdown, and auto-restart.
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

    /// Stop a service (SIGTERM -> wait -> SIGKILL the whole process group)
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
// SchedulerAtom Trait (cron task management)
// ========================================

/// Built-in cron scheduler trait (replaces CrontabAtom)
///
/// Manages periodic tasks with cron expressions.  Tasks are persisted
/// in a TOML file.  This trait provides CRUD operations, expression
/// validation, and next-run prediction.  Actual task execution is
/// the responsibility of the caller (e.g. an external scheduler loop).
pub trait SchedulerAtom {
    /// Add a new cron task, returns the generated task ID
    fn add(&self, task: &CronTask) -> Result<String>;

    /// Update an existing cron task
    fn update(&self, task_id: &str, task: &CronTask) -> Result<()>;

    /// Remove a cron task
    fn remove(&self, task_id: &str) -> Result<()>;

    /// Get a specific task
    fn get(&self, task_id: &str) -> Result<CronTask>;

    /// List all managed tasks
    fn list(&self) -> Result<Vec<CronTask>>;

    /// Predict next N execution times for a task
    fn next_runs(&self, task_id: &str, count: usize) -> Result<Vec<DateTime<Utc>>>;

    /// Validate a cron expression
    fn validate_expression(&self, expr: &str) -> Result<bool>;

    /// Set a global environment variable for all tasks
    fn set_env(&self, key: &str, value: &str) -> Result<()>;

    /// Get all global environment variables
    fn get_env(&self) -> Result<HashMap<String, String>>;

    /// Reload task definitions from disk
    fn reload(&self) -> Result<()>;
}

// ========================================
// SupervisorManager Implementation
// ========================================

/// Unified supervisor that manages both long-running services
/// and cron-scheduled tasks.
///
/// ## Process Group Management
///
/// Each spawned child process is placed in its own process group
/// via `setsid()`.  This means the child PID == PGID, so we can:
/// - Send signals to the entire process tree with `kill(-pgid, sig)`
/// - Ensure no orphan processes after stop
///
/// ## Graceful Shutdown
///
/// `stop()` sends SIGTERM to the process group, waits up to
/// `stop_timeout_sec`, then sends SIGKILL if still alive.
///
/// ## Auto-Restart
///
/// A background watchdog task monitors child exits and restarts
/// according to the configured `RestartPolicy`.
///
/// ## Cron Scheduling
///
/// Periodic tasks are stored in `cron-tasks.toml` in the service
/// directory and can be managed via the `SchedulerAtom` trait.
#[derive(Clone)]
pub struct SupervisorManager {
    /// Directory for service definition files and cron task store
    service_dir: PathBuf,
    /// Path to the cron task store TOML file
    task_store_path: PathBuf,
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
        let task_store_path = service_dir.join("cron-tasks.toml");
        Self {
            service_dir,
            task_store_path,
            git_managed,
            processes: Arc::new(RwLock::new(HashMap::new())),
            log_capacity: 1000,
        }
    }

    /// Create with default configuration (~/.config/svcmgr/managed/supervisor)
    pub fn default_config() -> Result<Self> {
        let home = std::env::var("HOME")
            .map_err(|_| Error::Config("HOME environment variable not set".to_string()))?;
        let service_dir = PathBuf::from(home).join(".config/svcmgr/managed/supervisor");
        Ok(Self::new(service_dir, true))
    }

    // ------ service definition helpers ------

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
        self.ensure_service_dir().await?;
        let path = self.service_path(&def.name);
        let content = toml::to_string_pretty(def)
            .map_err(|e| Error::Config(format!("Failed to serialize service definition: {}", e)))?;
        tokio::fs::write(&path, content).await?;
        Ok(())
    }

    // ------ process spawning with process group ------

    /// Spawn a child process in its own process group.
    ///
    /// Uses `setsid()` so that child PID == PGID, enabling whole-tree
    /// signal delivery via `kill(-pgid, sig)`.
    async fn spawn_process(&self, def: &ServiceDef) -> Result<tokio::process::Child> {
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

        // SAFETY: setsid() is async-signal-safe and has no memory-safety
        // implications.  It makes the child the leader of a new process
        // group so we can signal the entire tree later.
        unsafe {
            cmd.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }

        let child = cmd.spawn().map_err(|e| Error::CommandFailed {
            command: format!("{} {}", def.command, def.args.join(" ")),
            exit_code: None,
            stderr: e.to_string(),
        })?;

        Ok(child)
    }

    /// Start background log capture for stdout and stderr of a process.
    fn start_log_capture(
        processes: Arc<RwLock<HashMap<String, ProcessState>>>,
        name: String,
        child: &mut tokio::process::Child,
    ) {
        // Capture stdout -> Info priority
        if let Some(stdout) = child.stdout.take() {
            let procs = processes.clone();
            let svc = name.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let entry = LogEntry {
                        timestamp: Utc::now(),
                        priority: LogPriority::Info,
                        message: line,
                        unit: svc.clone(),
                    };
                    let mut procs = procs.write().await;
                    if let Some(state) = procs.get_mut(&svc) {
                        state.logs.push(entry);
                    }
                }
            });
        }

        // Capture stderr -> Error priority
        if let Some(stderr) = child.stderr.take() {
            let procs = processes.clone();
            let svc = name;
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let entry = LogEntry {
                        timestamp: Utc::now(),
                        priority: LogPriority::Error,
                        message: line,
                        unit: svc.clone(),
                    };
                    let mut procs = procs.write().await;
                    if let Some(state) = procs.get_mut(&svc) {
                        state.logs.push(entry);
                    }
                }
            });
        }
    }

    /// Start a background watchdog that auto-restarts the process
    /// according to its restart policy when it exits.
    fn start_watchdog(
        processes: Arc<RwLock<HashMap<String, ProcessState>>>,
        name: String,
        service_dir: PathBuf,
    ) {
        let procs = processes.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;

                let should_restart = {
                    let mut guard = procs.write().await;
                    let Some(state) = guard.get_mut(&name) else {
                        break;
                    };

                    if state.stopping {
                        break;
                    }

                    if let Some(ref mut child) = state.child {
                        match child.try_wait() {
                            Ok(Some(exit_status)) => {
                                let failed = !exit_status.success();
                                state.child = None;
                                state.pid = None;
                                state.active_state = if failed {
                                    ActiveState::Failed
                                } else {
                                    ActiveState::Inactive
                                };
                                state.sub_state =
                                    format!("exited ({})", exit_status.code().unwrap_or(-1));

                                match state.restart_policy {
                                    RestartPolicy::Always => true,
                                    RestartPolicy::OnFailure => failed,
                                    RestartPolicy::No => false,
                                }
                            }
                            Ok(None) => false,
                            Err(_) => {
                                state.active_state = ActiveState::Failed;
                                state.sub_state = "error".to_string();
                                state.child = None;
                                state.pid = None;
                                false
                            }
                        }
                    } else {
                        break;
                    }
                };

                if should_restart {
                    let restart_sec = {
                        let guard = procs.read().await;
                        guard.get(&name).map(|s| s.restart_sec).unwrap_or(1)
                    };

                    tokio::time::sleep(Duration::from_secs(restart_sec)).await;

                    let def_path = service_dir.join(format!("{}.toml", name));
                    let def: Option<ServiceDef> = tokio::fs::read_to_string(&def_path)
                        .await
                        .ok()
                        .and_then(|content| toml::from_str(&content).ok());

                    if let Some(def) = def {
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
                        unsafe {
                            cmd.pre_exec(|| {
                                libc::setsid();
                                Ok(())
                            });
                        }

                        if let Ok(mut child) = cmd.spawn() {
                            let pid = child.id();
                            Self::start_log_capture(procs.clone(), name.clone(), &mut child);
                            let mut guard = procs.write().await;
                            if let Some(state) = guard.get_mut(&name) {
                                state.child = Some(child);
                                state.pid = pid;
                                state.active_state = ActiveState::Active;
                                state.sub_state = "running".to_string();
                                state.started_at = Some(Utc::now());
                            }
                        }
                    } else {
                        break;
                    }
                }
            }
        });
    }

    // ------ process group signal helpers ------

    /// Send a signal to the process group identified by `pgid`.
    ///
    /// Because we used `setsid()`, the child PID == PGID.
    fn kill_process_group(pgid: u32, signal: i32) -> std::result::Result<(), std::io::Error> {
        let ret = unsafe { libc::kill(-(pgid as i32) as libc::pid_t, signal) };
        if ret == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }

    /// Graceful stop: SIGTERM -> wait up to timeout -> SIGKILL
    ///
    /// Important: after delivering the signal, we also wait until the whole process group
    /// disappears, otherwise callers/tests may still observe the PGID as alive due to
    /// unreaped/zombie descendants.
    async fn graceful_stop(child: &mut tokio::process::Child, pgid: u32, timeout_secs: u64) {
        let _ = Self::kill_process_group(pgid, libc::SIGTERM);

        let wait_result =
            tokio::time::timeout(Duration::from_secs(timeout_secs), child.wait()).await;

        if wait_result.is_err() {
            let _ = Self::kill_process_group(pgid, libc::SIGKILL);
            let _ = child.wait().await;
        }

        // Best-effort: wait for the PGID to fully disappear.
        let deadline = tokio::time::Instant::now() + Duration::from_millis(500);
        loop {
            match Self::kill_process_group(pgid, 0) {
                Ok(_) => {
                    if tokio::time::Instant::now() >= deadline {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Err(e) => {
                    if e.raw_os_error() == Some(libc::ESRCH) {
                        break;
                    }
                    break;
                }
            }
        }
    }

    // ------ service file listing ------

    /// List all service definition files on disk (excluding cron-tasks.toml)
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
                let name = stem.to_string_lossy().to_string();
                if name != "cron-tasks" {
                    names.push(name);
                }
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
                    state.active_state = if exit_status.success() {
                        ActiveState::Inactive
                    } else {
                        ActiveState::Failed
                    };
                    state.sub_state = format!("exited ({})", exit_status.code().unwrap_or(-1));
                    state.pid = None;
                    state.child = None;
                }
                Ok(None) => {
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

    // ------ cron task store helpers ------

    /// Read the task store from disk.
    ///
    /// Note: `SchedulerAtom` methods are synchronous. If called from within a Tokio
    /// runtime, wrap blocking filesystem access with `tokio::task::block_in_place`.
    fn read_task_store(&self) -> Result<TaskStore> {
        if !self.task_store_path.exists() {
            return Ok(TaskStore::default());
        }

        let read = || std::fs::read_to_string(&self.task_store_path);

        let use_block_in_place = tokio::runtime::Handle::try_current().ok().is_some_and(|h| {
            matches!(
                h.runtime_flavor(),
                tokio::runtime::RuntimeFlavor::MultiThread
            )
        });

        let content = if use_block_in_place {
            tokio::task::block_in_place(read)?
        } else {
            // Fallback for current-thread runtimes (where block_in_place would panic).
            read()?
        };

        let store: TaskStore = toml::from_str(&content)
            .map_err(|e| Error::Config(format!("Invalid task store: {}", e)))?;
        Ok(store)
    }

    /// Write the task store to disk.
    ///
    /// Note: `SchedulerAtom` methods are synchronous. If called from within a Tokio
    /// runtime, wrap blocking filesystem access with `tokio::task::block_in_place`.
    fn write_task_store(&self, store: &TaskStore) -> Result<()> {
        let content = toml::to_string_pretty(store)
            .map_err(|e| Error::Config(format!("Failed to serialize task store: {}", e)))?;

        let write = || -> std::io::Result<()> {
            if let Some(parent) = self.task_store_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&self.task_store_path, &content)?;
            Ok(())
        };

        let use_block_in_place = tokio::runtime::Handle::try_current().ok().is_some_and(|h| {
            matches!(
                h.runtime_flavor(),
                tokio::runtime::RuntimeFlavor::MultiThread
            )
        });

        if use_block_in_place {
            tokio::task::block_in_place(write)?;
        } else {
            // Fallback for current-thread runtimes (where block_in_place would panic).
            write()?;
        }

        Ok(())
    }

    /// Generate a timestamp-based task ID (nanosecond precision to avoid collisions)
    fn generate_task_id(&self) -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("{}", ts)
    }

    /// Normalize predefined cron expressions to standard 5-field format
    fn normalize_expression(&self, expr: &str) -> String {
        match expr {
            "@hourly" => "0 * * * *".to_string(),
            "@daily" | "@midnight" => "0 0 * * *".to_string(),
            "@weekly" => "0 0 * * 1".to_string(),
            "@monthly" => "0 0 1 * *".to_string(),
            "@yearly" | "@annually" => "0 0 1 1 *".to_string(),
            other => other.to_string(),
        }
    }

    /// Convert a 5-field cron expression to 6-field (with seconds) for the cron crate
    fn to_schedule_format(&self, expr: &str) -> String {
        let normalized = self.normalize_expression(expr);
        let fields: Vec<&str> = normalized.split_whitespace().collect();
        if fields.len() == 5 {
            format!("0 {}", normalized)
        } else {
            normalized
        }
    }

    fn task_entry_to_cron_task(entry: &TaskEntry) -> CronTask {
        CronTask {
            id: Some(entry.id.clone()),
            description: entry.description.clone(),
            expression: entry.expression.clone(),
            command: entry.command.clone(),
            env: entry.env.clone(),
            enabled: entry.enabled,
        }
    }
}

// ========================================
// SupervisorAtom Implementation
// ========================================

impl SupervisorAtom for SupervisorManager {
    async fn create_unit(&self, name: &str, content: &str) -> Result<()> {
        // Validate that content is a valid TOML ServiceDef
        let def: ServiceDef = toml::from_str(content)
            .map_err(|e| Error::Config(format!("Invalid TOML ServiceDef: {}", e)))?;
        // Enforce that the name inside the definition matches the unit name
        if def.name != name {
            return Err(Error::Config(format!(
                "ServiceDef name '{}' does not match unit name '{}'",
                def.name, name
            )));
        }
        self.ensure_service_dir().await?;
        let path = self.service_path(name);
        tokio::fs::write(&path, content).await?;
        Ok(())
    }

    async fn update_unit(&self, name: &str, content: &str) -> Result<()> {
        // Validate that content is a valid TOML ServiceDef
        let def: ServiceDef = toml::from_str(content)
            .map_err(|e| Error::Config(format!("Invalid TOML ServiceDef: {}", e)))?;
        if def.name != name {
            return Err(Error::Config(format!(
                "ServiceDef name '{}' does not match unit name '{}'",
                def.name, name
            )));
        }
        let path = self.service_path(name);
        if !path.exists() {
            return Err(Error::NotSupported(format!("Service {} not found", name)));
        }
        tokio::fs::write(&path, content).await?;
        Ok(())
    }

    async fn delete_unit(&self, name: &str) -> Result<()> {
        let _ = self.stop(name).await;

        let path = self.service_path(name);
        if path.exists() {
            tokio::fs::remove_file(&path).await?;
        }

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
            match self.read_service_def(&name).await {
                Ok(def) => {
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
                Err(_) => {
                    // Include broken definitions with BadSetting load state
                    units.push(UnitInfo {
                        name: name.clone(),
                        description: String::new(),
                        load_state: LoadState::BadSetting,
                        active_state: ActiveState::Inactive,
                        sub_state: "dead".to_string(),
                        enabled: false,
                    });
                }
            }
        }

        Ok(units)
    }

    async fn start(&self, name: &str) -> Result<()> {
        self.refresh_process_state(name).await;
        {
            let procs = self.processes.read().await;
            if let Some(state) = procs.get(name)
                && state.active_state == ActiveState::Active
            {
                return Ok(());
            }
        }

        let def = self.read_service_def(name).await?;
        let mut child = self.spawn_process(&def).await?;
        let pid = child.id();

        Self::start_log_capture(self.processes.clone(), name.to_string(), &mut child);

        let state = ProcessState {
            child: Some(child),
            pid,
            active_state: ActiveState::Active,
            sub_state: "running".to_string(),
            started_at: Some(Utc::now()),
            logs: LogBuffer::new(self.log_capacity),
            restart_policy: def.restart_policy,
            restart_sec: def.restart_sec,
            stop_timeout_sec: def.stop_timeout_sec,
            stopping: false,
        };

        {
            let mut procs = self.processes.write().await;
            procs.insert(name.to_string(), state);
        }

        if def.restart_policy != RestartPolicy::No {
            Self::start_watchdog(
                self.processes.clone(),
                name.to_string(),
                self.service_dir.clone(),
            );
        }

        Ok(())
    }

    async fn stop(&self, name: &str) -> Result<()> {
        // Extract the child handle + pgid, then release the lock before the
        // async graceful_stop so we don't block log capture and queries.
        let extracted = {
            let mut procs = self.processes.write().await;
            if let Some(state) = procs.get_mut(name) {
                state.stopping = true;
                let child = state.child.take();
                let pgid = state.pid.take();
                let timeout = state.stop_timeout_sec;
                Some((child, pgid, timeout))
            } else {
                None
            }
        };

        if let Some((Some(mut child), Some(pgid), timeout)) = extracted {
            Self::graceful_stop(&mut child, pgid, timeout).await;
        }

        // Re-acquire to update final state
        {
            let mut procs = self.processes.write().await;
            if let Some(state) = procs.get_mut(name) {
                state.child = None;
                state.pid = None;
                state.active_state = ActiveState::Inactive;
                state.sub_state = "stopped".to_string();
            }
        }
        Ok(())
    }

    async fn restart(&self, name: &str) -> Result<()> {
        self.stop(name).await?;
        self.start(name).await
    }

    async fn reload(&self, name: &str) -> Result<()> {
        let is_running = {
            self.refresh_process_state(name).await;
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
        self.write_service_def(&def).await
    }

    async fn disable(&self, name: &str) -> Result<()> {
        let mut def = self.read_service_def(name).await?;
        def.enabled = false;
        self.write_service_def(&def).await
    }

    async fn status(&self, name: &str) -> Result<UnitStatus> {
        self.refresh_process_state(name).await;

        let procs = self.processes.read().await;
        let state = procs.get(name);

        let (active_state, sub_state, pid, started_at, recent_logs) = match state {
            Some(s) => (
                s.active_state,
                s.sub_state.clone(),
                s.pid,
                s.started_at,
                s.logs.recent(20).into_iter().map(|e| e.message).collect(),
            ),
            None => (
                ActiveState::Inactive,
                "dead".to_string(),
                None,
                None,
                Vec::new(),
            ),
        };

        Ok(UnitStatus {
            name: name.to_string(),
            active_state,
            sub_state,
            pid,
            memory: None,
            cpu_time: None,
            started_at,
            recent_logs,
        })
    }

    async fn process_tree(&self, name: &str) -> Result<ProcessTree> {
        let procs = self.processes.read().await;
        let state = procs
            .get(name)
            .ok_or_else(|| Error::NotSupported(format!("Service {} not found", name)))?;

        let root_pid = state
            .pid
            .ok_or_else(|| Error::NotSupported(format!("Service {} is not running", name)))?;

        let mut proc_infos = vec![ProcessInfo {
            pid: root_pid,
            ppid: std::process::id(),
            name: name.to_string(),
            cmdline: name.to_string(),
        }];

        // Enumerate child processes from /proc by matching PGID
        if let Ok(entries) = std::fs::read_dir("/proc") {
            for entry in entries.flatten() {
                let fname = entry.file_name();
                let fname_str = fname.to_string_lossy();
                if let Ok(pid) = fname_str.parse::<u32>() {
                    if pid == root_pid {
                        continue;
                    }
                    let stat_path = format!("/proc/{}/stat", pid);
                    if let Ok(stat) = std::fs::read_to_string(&stat_path) {
                        let fields: Vec<&str> = stat.split_whitespace().collect();
                        if fields.len() > 7 {
                            let pgid: u32 = fields[4].parse().unwrap_or(0);
                            let ppid: u32 = fields[3].parse().unwrap_or(0);
                            if pgid == root_pid {
                                let cmd = fields[1]
                                    .trim_start_matches('(')
                                    .trim_end_matches(')')
                                    .to_string();
                                proc_infos.push(ProcessInfo {
                                    pid,
                                    ppid,
                                    name: cmd.clone(),
                                    cmdline: cmd,
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(ProcessTree {
            root_pid,
            processes: proc_infos,
        })
    }

    async fn logs(&self, name: &str, opts: &LogOptions) -> Result<Vec<LogEntry>> {
        let procs = self.processes.read().await;
        let state = procs.get(name);
        Ok(state.map(|s| s.logs.query(opts)).unwrap_or_default())
    }

    fn logs_stream(&self, _name: &str) -> Result<Pin<Box<dyn Stream<Item = LogEntry> + Send>>> {
        Ok(Box::pin(futures::stream::empty()))
    }

    async fn run_transient(&self, opts: &TransientOptions) -> Result<TransientUnit> {
        let name = format!("transient-{}", opts.name);

        let mut cmd = Command::new(
            opts.command
                .first()
                .ok_or_else(|| Error::InvalidArgument("Empty command".to_string()))?,
        );
        if opts.command.len() > 1 {
            cmd.args(&opts.command[1..]);
        }
        for (k, v) in &opts.env {
            cmd.env(k, v);
        }
        if let Some(ref wd) = opts.working_directory {
            cmd.current_dir(wd);
        }
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        unsafe {
            cmd.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }

        let mut child = cmd.spawn().map_err(|e| Error::CommandFailed {
            command: opts.command.join(" "),
            exit_code: None,
            stderr: e.to_string(),
        })?;

        let pid = child.id();
        let started_at = Utc::now();

        Self::start_log_capture(self.processes.clone(), name.clone(), &mut child);

        let state = ProcessState {
            child: Some(child),
            pid,
            active_state: ActiveState::Active,
            sub_state: "running".to_string(),
            started_at: Some(started_at),
            logs: LogBuffer::new(self.log_capacity),
            restart_policy: RestartPolicy::No,
            restart_sec: 0,
            stop_timeout_sec: 10,
            stopping: false,
        };

        let mut procs = self.processes.write().await;
        procs.insert(name.clone(), state);

        Ok(TransientUnit {
            name,
            pid,
            started_at,
        })
    }

    async fn list_transient(&self) -> Result<Vec<TransientUnit>> {
        let procs = self.processes.read().await;
        let mut units = Vec::new();
        for (name, state) in procs.iter() {
            if name.starts_with("transient-") {
                units.push(TransientUnit {
                    name: name.clone(),
                    pid: state.pid,
                    started_at: state.started_at.unwrap_or_else(Utc::now),
                });
            }
        }
        Ok(units)
    }

    async fn stop_transient(&self, name: &str) -> Result<()> {
        self.stop(name).await
    }

    async fn daemon_reload(&self) -> Result<()> {
        let _ = self.list_service_files().await?;
        Ok(())
    }
}

// ========================================
// SchedulerAtom Implementation
// ========================================

impl SchedulerAtom for SupervisorManager {
    fn add(&self, task: &CronTask) -> Result<String> {
        self.validate_expression(&task.expression)?;

        let mut store = self.read_task_store()?;
        let task_id = task.id.clone().unwrap_or_else(|| self.generate_task_id());

        if store.tasks.iter().any(|t| t.id == task_id) {
            return Err(Error::InvalidArgument(format!(
                "Task ID {} already exists",
                task_id
            )));
        }

        store.tasks.push(TaskEntry {
            id: task_id.clone(),
            description: task.description.clone(),
            expression: task.expression.clone(),
            command: task.command.clone(),
            env: task.env.clone(),
            enabled: task.enabled,
        });

        self.write_task_store(&store)?;
        Ok(task_id)
    }

    fn update(&self, task_id: &str, task: &CronTask) -> Result<()> {
        self.validate_expression(&task.expression)?;

        let mut store = self.read_task_store()?;
        let entry = store
            .tasks
            .iter_mut()
            .find(|t| t.id == task_id)
            .ok_or_else(|| Error::NotSupported(format!("Task {} not found", task_id)))?;

        entry.description = task.description.clone();
        entry.expression = task.expression.clone();
        entry.command = task.command.clone();
        entry.env = task.env.clone();
        entry.enabled = task.enabled;

        self.write_task_store(&store)
    }

    fn remove(&self, task_id: &str) -> Result<()> {
        let mut store = self.read_task_store()?;
        let before = store.tasks.len();
        store.tasks.retain(|t| t.id != task_id);
        if store.tasks.len() == before {
            return Err(Error::NotSupported(format!("Task {} not found", task_id)));
        }
        self.write_task_store(&store)
    }

    fn get(&self, task_id: &str) -> Result<CronTask> {
        let store = self.read_task_store()?;
        store
            .tasks
            .iter()
            .find(|t| t.id == task_id)
            .map(Self::task_entry_to_cron_task)
            .ok_or_else(|| Error::NotSupported(format!("Task {} not found", task_id)))
    }

    fn list(&self) -> Result<Vec<CronTask>> {
        let store = self.read_task_store()?;
        Ok(store
            .tasks
            .iter()
            .map(Self::task_entry_to_cron_task)
            .collect())
    }

    fn next_runs(&self, task_id: &str, count: usize) -> Result<Vec<DateTime<Utc>>> {
        let task = self.get(task_id)?;
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

    fn set_env(&self, key: &str, value: &str) -> Result<()> {
        let mut store = self.read_task_store()?;
        store.env.insert(key.to_string(), value.to_string());
        self.write_task_store(&store)
    }

    fn get_env(&self) -> Result<HashMap<String, String>> {
        let store = self.read_task_store()?;
        Ok(store.env)
    }

    fn reload(&self) -> Result<()> {
        let _ = self.read_task_store()?;
        Ok(())
    }
}

// ========================================
// Tests
// ========================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_manager() -> (SupervisorManager, TempDir) {
        let tmp = TempDir::new().unwrap();
        let mgr = SupervisorManager::new(tmp.path().to_path_buf(), false);
        (mgr, tmp)
    }

    fn create_test_service_def(name: &str) -> ServiceDef {
        ServiceDef {
            name: name.to_string(),
            description: format!("Test service {}", name),
            command: "/bin/echo".to_string(),
            args: vec!["hello".to_string()],
            working_directory: None,
            env: HashMap::new(),
            restart_policy: RestartPolicy::No,
            restart_sec: 1,
            enabled: true,
            stop_timeout_sec: 10,
        }
    }

    // ------ supervisor tests ------

    #[test]
    fn test_supervisor_manager_creation() {
        let (mgr, _tmp) = create_test_manager();
        assert!(mgr.service_dir.exists());
        assert_eq!(mgr.log_capacity, 1000);
    }

    #[test]
    fn test_service_path_generation() {
        let (mgr, _tmp) = create_test_manager();
        let path = mgr.service_path("my-service");
        assert!(path.to_string_lossy().ends_with("my-service.toml"));
    }

    #[test]
    fn test_log_buffer_push_and_recent() {
        let mut buf = LogBuffer::new(100);
        for i in 0..5 {
            buf.push(LogEntry {
                timestamp: Utc::now(),
                priority: LogPriority::Info,
                message: format!("msg {}", i),
                unit: "test".to_string(),
            });
        }
        let recent = buf.recent(3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].message, "msg 2");
    }

    #[test]
    fn test_log_buffer_capacity() {
        let mut buf = LogBuffer::new(3);
        for i in 0..5 {
            buf.push(LogEntry {
                timestamp: Utc::now(),
                priority: LogPriority::Info,
                message: format!("msg {}", i),
                unit: "test".to_string(),
            });
        }
        assert_eq!(buf.entries.len(), 3);
        assert_eq!(buf.entries[0].message, "msg 2");
        assert_eq!(buf.entries[1].message, "msg 3");
        assert_eq!(buf.entries[2].message, "msg 4");
    }

    #[test]
    fn test_log_buffer_query_with_lines() {
        let mut buf = LogBuffer::new(100);
        for i in 0..10 {
            buf.push(LogEntry {
                timestamp: Utc::now(),
                priority: LogPriority::Info,
                message: format!("msg {}", i),
                unit: "test".to_string(),
            });
        }
        let opts = LogOptions {
            lines: Some(3),
            ..Default::default()
        };
        let result = buf.query(&opts);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].message, "msg 7");
    }

    #[test]
    fn test_service_def_serialization() {
        let def = create_test_service_def("test-svc");
        let toml_str = toml::to_string_pretty(&def).unwrap();
        let parsed: ServiceDef = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.name, "test-svc");
        assert_eq!(parsed.command, "/bin/echo");
        assert_eq!(parsed.args, vec!["hello"]);
        assert_eq!(parsed.restart_policy, RestartPolicy::No);
    }

    #[tokio::test]
    async fn test_create_and_get_unit() {
        let (mgr, _tmp) = create_test_manager();
        let def = create_test_service_def("my-svc");
        let content = toml::to_string_pretty(&def).unwrap();

        mgr.create_unit("my-svc", &content).await.unwrap();

        let unit = mgr.get_unit("my-svc").await.unwrap();
        assert_eq!(unit.name, "my-svc");
        assert!(unit.content.contains("my-svc"));
    }

    #[tokio::test]
    async fn test_delete_unit() {
        let (mgr, _tmp) = create_test_manager();
        let def = create_test_service_def("del-svc");
        let content = toml::to_string_pretty(&def).unwrap();

        mgr.create_unit("del-svc", &content).await.unwrap();
        mgr.delete_unit("del-svc").await.unwrap();
        assert!(!mgr.service_path("del-svc").exists());
    }

    #[tokio::test]
    async fn test_list_units_empty() {
        let (mgr, _tmp) = create_test_manager();
        let units = mgr.list_units().await.unwrap();
        assert!(units.is_empty());
    }

    #[tokio::test]
    async fn test_enable_disable() {
        let (mgr, _tmp) = create_test_manager();
        let mut def = create_test_service_def("svc-ed");
        def.enabled = false;
        mgr.write_service_def(&def).await.unwrap();

        mgr.enable("svc-ed").await.unwrap();
        let d = mgr.read_service_def("svc-ed").await.unwrap();
        assert!(d.enabled);

        mgr.disable("svc-ed").await.unwrap();
        let d = mgr.read_service_def("svc-ed").await.unwrap();
        assert!(!d.enabled);
    }

    #[tokio::test]
    async fn test_status_not_running() {
        let (mgr, _tmp) = create_test_manager();
        let def = create_test_service_def("stat-svc");
        mgr.write_service_def(&def).await.unwrap();

        let st = mgr.status("stat-svc").await.unwrap();
        assert_eq!(st.active_state, ActiveState::Inactive);
        assert!(st.pid.is_none());
    }

    #[test]
    fn test_active_state_equality() {
        assert_eq!(ActiveState::Active, ActiveState::Active);
        assert_ne!(ActiveState::Active, ActiveState::Inactive);
    }

    #[test]
    fn test_load_state_equality() {
        assert_eq!(LoadState::Loaded, LoadState::Loaded);
        assert_ne!(LoadState::Loaded, LoadState::NotFound);
    }

    #[test]
    fn test_restart_policy_serde() {
        let json = serde_json::to_string(&RestartPolicy::Always).unwrap();
        let parsed: RestartPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, RestartPolicy::Always);
    }

    #[tokio::test]
    async fn test_start_and_stop_process() {
        let (mgr, _tmp) = create_test_manager();
        let mut def = create_test_service_def("sleeper");
        def.command = "/bin/sleep".to_string();
        def.args = vec!["60".to_string()];
        mgr.write_service_def(&def).await.unwrap();

        mgr.start("sleeper").await.unwrap();

        let st = mgr.status("sleeper").await.unwrap();
        assert_eq!(st.active_state, ActiveState::Active);
        assert!(st.pid.is_some());

        mgr.stop("sleeper").await.unwrap();
        let st = mgr.status("sleeper").await.unwrap();
        assert_eq!(st.active_state, ActiveState::Inactive);
        assert!(st.pid.is_none());
    }

    #[tokio::test]
    async fn test_process_group_stop() {
        let (mgr, _tmp) = create_test_manager();
        let mut def = create_test_service_def("group-test");
        def.command = "/bin/sh".to_string();
        def.args = vec!["-c".to_string(), "sleep 120 & sleep 120 & wait".to_string()];
        mgr.write_service_def(&def).await.unwrap();

        mgr.start("group-test").await.unwrap();

        tokio::time::sleep(Duration::from_millis(500)).await;

        let st = mgr.status("group-test").await.unwrap();
        assert_eq!(st.active_state, ActiveState::Active);
        let pid = st.pid.unwrap();

        mgr.stop("group-test").await.unwrap();

        let ret = unsafe { libc::kill(-(pid as i32) as libc::pid_t, 0) };
        assert_ne!(ret, 0, "Process group should be dead after stop");
    }

    // ------ scheduler / cron tests ------

    fn create_test_cron_task() -> CronTask {
        CronTask {
            id: None,
            description: "Test task".to_string(),
            expression: "0 2 * * *".to_string(),
            command: "echo test".to_string(),
            env: HashMap::new(),
            enabled: true,
        }
    }

    #[test]
    fn test_normalize_expression() {
        let (mgr, _tmp) = create_test_manager();
        assert_eq!(mgr.normalize_expression("@hourly"), "0 * * * *");
        assert_eq!(mgr.normalize_expression("@daily"), "0 0 * * *");
        assert_eq!(mgr.normalize_expression("@weekly"), "0 0 * * 1");
        assert_eq!(mgr.normalize_expression("@monthly"), "0 0 1 * *");
        assert_eq!(mgr.normalize_expression("@yearly"), "0 0 1 1 *");
        assert_eq!(mgr.normalize_expression("0 2 * * *"), "0 2 * * *");
    }

    #[test]
    fn test_to_schedule_format() {
        let (mgr, _tmp) = create_test_manager();
        assert_eq!(mgr.to_schedule_format("0 2 * * *"), "0 0 2 * * *");
        assert_eq!(mgr.to_schedule_format("@hourly"), "0 0 * * * *");
        assert_eq!(mgr.to_schedule_format("@daily"), "0 0 0 * * *");
        assert_eq!(mgr.to_schedule_format("@monthly"), "0 0 0 1 * *");
    }

    #[test]
    fn test_validate_expression() {
        let (mgr, _tmp) = create_test_manager();

        assert!(mgr.validate_expression("0 2 * * *").is_ok());
        assert!(mgr.validate_expression("*/5 * * * *").is_ok());
        assert!(mgr.validate_expression("@hourly").is_ok());
        assert!(mgr.validate_expression("@daily").is_ok());

        assert!(mgr.validate_expression("invalid").is_err());
        assert!(mgr.validate_expression("0 25 * * *").is_err());
    }

    #[test]
    fn test_validate_predefined_expressions() {
        let (mgr, _tmp) = create_test_manager();

        assert!(mgr.validate_expression("@hourly").is_ok());
        assert!(mgr.validate_expression("@daily").is_ok());
        assert!(mgr.validate_expression("@weekly").is_ok());
        assert!(mgr.validate_expression("@monthly").is_ok());
        assert!(mgr.validate_expression("@yearly").is_ok());
        assert!(mgr.validate_expression("@annually").is_ok());
    }

    #[test]
    fn test_validate_invalid_expressions() {
        let (mgr, _tmp) = create_test_manager();

        assert!(mgr.validate_expression("").is_err());
        assert!(mgr.validate_expression("not a cron").is_err());
        assert!(mgr.validate_expression("60 * * * *").is_err());
    }

    #[test]
    fn test_add_and_get_task() {
        let (mgr, _tmp) = create_test_manager();
        let task = create_test_cron_task();

        let task_id = mgr.add(&task).unwrap();
        let retrieved = mgr.get(&task_id).unwrap();

        assert_eq!(retrieved.description, "Test task");
        assert_eq!(retrieved.expression, "0 2 * * *");
        assert_eq!(retrieved.command, "echo test");
        assert!(retrieved.enabled);
    }

    #[test]
    fn test_add_duplicate_task_id() {
        let (mgr, _tmp) = create_test_manager();
        let mut task = create_test_cron_task();
        task.id = Some("dup-id".to_string());

        mgr.add(&task).unwrap();
        assert!(mgr.add(&task).is_err());
    }

    #[test]
    fn test_update_task() {
        let (mgr, _tmp) = create_test_manager();
        let task = create_test_cron_task();
        let task_id = mgr.add(&task).unwrap();

        let mut updated = task.clone();
        updated.command = "echo updated".to_string();
        mgr.update(&task_id, &updated).unwrap();

        let retrieved = mgr.get(&task_id).unwrap();
        assert_eq!(retrieved.command, "echo updated");
    }

    #[test]
    fn test_remove_task() {
        let (mgr, _tmp) = create_test_manager();
        let task = create_test_cron_task();
        let task_id = mgr.add(&task).unwrap();

        mgr.remove(&task_id).unwrap();
        assert!(mgr.get(&task_id).is_err());
    }

    #[test]
    fn test_remove_nonexistent_task() {
        let (mgr, _tmp) = create_test_manager();
        assert!(mgr.remove("nonexistent").is_err());
    }

    #[test]
    fn test_list_tasks() {
        let (mgr, _tmp) = create_test_manager();

        let task1 = CronTask {
            id: Some("t1".to_string()),
            description: "Task 1".to_string(),
            expression: "0 1 * * *".to_string(),
            command: "echo 1".to_string(),
            env: HashMap::new(),
            enabled: true,
        };
        let task2 = CronTask {
            id: Some("t2".to_string()),
            description: "Task 2".to_string(),
            expression: "0 2 * * *".to_string(),
            command: "echo 2".to_string(),
            env: HashMap::new(),
            enabled: true,
        };

        mgr.add(&task1).unwrap();
        mgr.add(&task2).unwrap();

        let tasks = mgr.list().unwrap();
        assert_eq!(tasks.len(), 2);
    }

    #[test]
    fn test_next_runs() {
        let (mgr, _tmp) = create_test_manager();
        let task = create_test_cron_task();
        let task_id = mgr.add(&task).unwrap();

        let runs = mgr.next_runs(&task_id, 5).unwrap();
        assert_eq!(runs.len(), 5);
        for i in 1..runs.len() {
            assert!(runs[i] > runs[i - 1]);
        }
    }

    #[test]
    fn test_set_and_get_env() {
        let (mgr, _tmp) = create_test_manager();

        mgr.set_env("PATH", "/usr/bin").unwrap();
        mgr.set_env("HOME", "/root").unwrap();

        let env = mgr.get_env().unwrap();
        assert_eq!(env.get("PATH"), Some(&"/usr/bin".to_string()));
        assert_eq!(env.get("HOME"), Some(&"/root".to_string()));
    }

    #[test]
    fn test_reload_task_store() {
        let (mgr, _tmp) = create_test_manager();
        let task = create_test_cron_task();
        mgr.add(&task).unwrap();

        assert!(SchedulerAtom::reload(&mgr).is_ok());
    }

    #[test]
    fn test_cron_task_creation() {
        let task = create_test_cron_task();
        assert_eq!(task.description, "Test task");
        assert_eq!(task.expression, "0 2 * * *");
        assert!(task.enabled);
        assert!(task.id.is_none());
    }

    #[test]
    fn test_task_store_serde() {
        let store = TaskStore {
            env: {
                let mut m = HashMap::new();
                m.insert("K".to_string(), "V".to_string());
                m
            },
            tasks: vec![TaskEntry {
                id: "1".to_string(),
                description: "d".to_string(),
                expression: "0 * * * *".to_string(),
                command: "echo".to_string(),
                env: HashMap::new(),
                enabled: true,
            }],
        };
        let s = toml::to_string_pretty(&store).unwrap();
        let parsed: TaskStore = toml::from_str(&s).unwrap();
        assert_eq!(parsed.tasks.len(), 1);
        assert_eq!(parsed.env.get("K"), Some(&"V".to_string()));
    }

    #[tokio::test]
    async fn test_list_service_files_excludes_cron_tasks() {
        let (mgr, _tmp) = create_test_manager();
        let def = create_test_service_def("real-svc");
        mgr.write_service_def(&def).await.unwrap();
        mgr.set_env("X", "Y").unwrap();

        let names = mgr.list_service_files().await.unwrap();
        assert!(names.contains(&"real-svc".to_string()));
        assert!(!names.contains(&"cron-tasks".to_string()));
    }
}
