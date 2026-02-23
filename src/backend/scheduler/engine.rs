//! Scheduler engine - core task scheduling and lifecycle management
//!
//! Phase 2.1: Unified scheduler with multiple trigger types

use super::trigger::{EventType, RestartBackoff, RestartPolicy, RestartTracker, Trigger};
use crate::runtime::ProcessHandle;
use anyhow::{anyhow, Context, Result};
use chrono::Local;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, mpsc};
use tokio::time::{interval_at, sleep, Interval};

/// Task execution method
#[derive(Debug, Clone)]
pub enum Execution {
    /// Execute via mise task (reads command from mise config)
    MiseTask {
        task_name: String,
        args: Vec<String>,
    },

    /// Direct command execution
    Command {
        command: String,
        args: Vec<String>,
        env: HashMap<String, String>,
        workdir: Option<PathBuf>,
    },
}

/// Task state
#[derive(Debug, Clone, PartialEq)]
pub enum TaskState {
    /// Pending execution
    Pending,

    /// Currently running
    Running { pid: u32, started_at: Instant },

    /// Completed successfully
    Completed {
        exit_code: i32,
        finished_at: Instant,
    },

    /// Failed with error
    Failed { error: String, failed_at: Instant },

    /// Fatal state (exceeded restart limits, requires manual intervention)
    Fatal {
        last_error: String,
        restart_count: u32,
    },
}

/// Scheduled task with trigger and execution info
#[derive(Debug)]
pub struct ScheduledTask {
    /// Task name (unique identifier)
    pub name: String,

    /// Trigger type
    pub trigger: Trigger,

    /// Execution method
    pub execution: Execution,

    /// Current state
    pub state: TaskState,

    /// Restart policy (for service-like tasks)
    pub restart_policy: RestartPolicy,

    /// Restart backoff
    pub backoff: RestartBackoff,

    /// Restart tracker
    pub tracker: RestartTracker,

    /// Timeout (None = no timeout)
    pub timeout: Option<Duration>,

    /// Running process handle (if running)
    process: Option<ProcessHandle>,
}

impl ScheduledTask {
    pub fn new(
        name: String,
        trigger: Trigger,
        execution: Execution,
        restart_policy: RestartPolicy,
    ) -> Self {
        let (backoff, tracker) = match &restart_policy {
            RestartPolicy::Always {
                delay,
                limit,
                window,
            }
            | RestartPolicy::OnFailure {
                delay,
                limit,
                window,
            } => (
                RestartBackoff::new(*delay, Duration::from_secs(300)), // Max 5min backoff
                RestartTracker::new(*limit, *window),
            ),
            RestartPolicy::Never => (
                RestartBackoff::new(Duration::from_secs(1), Duration::from_secs(1)),
                RestartTracker::new(0, Duration::from_secs(1)),
            ),
        };

        Self {
            name,
            trigger,
            execution,
            state: TaskState::Pending,
            restart_policy,
            backoff,
            tracker,
            timeout: None,
            process: None,
        }
    }

    /// Set timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
}

/// Event bus for cross-task communication
#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<EventType>,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(1024);
        Self { tx }
    }

    /// Emit an event
    pub fn emit(&self, event: EventType) -> Result<()> {
        self.tx
            .send(event)
            .map_err(|e| anyhow!("Failed to emit event: {}", e))?;
        Ok(())
    }

    /// Subscribe to events
    pub fn subscribe(&self) -> broadcast::Receiver<EventType> {
        self.tx.subscribe()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

/// Command from external API/CLI to scheduler
#[derive(Debug)]
pub enum SchedulerCommand {
    /// Start a specific task
    StartTask(String),
    /// Stop a specific task
    StopTask(String),
    /// Restart a specific task
    RestartTask(String),
    /// Shutdown scheduler
    Shutdown,
}

/// Scheduler engine - manages task lifecycle
pub struct SchedulerEngine {
    /// All registered tasks
    tasks: HashMap<String, ScheduledTask>,

    /// Event bus for cross-task events
    event_bus: EventBus,

    /// Command channel receiver
    command_rx: mpsc::Receiver<SchedulerCommand>,

    /// Command channel sender (for cloning)
    command_tx: mpsc::Sender<SchedulerCommand>,

    /// Delayed tasks queue (task_name, wake_time)
    delayed_queue: VecDeque<(String, Instant)>,

    /// Cron ticker interval
    cron_ticker: Interval,

    /// Log directory for process output
    log_dir: PathBuf,

    /// Shutdown flag
    shutdown: bool,
}

impl SchedulerEngine {
    /// Create new scheduler engine
    pub fn new(log_dir: PathBuf) -> Self {
        let (command_tx, command_rx) = mpsc::channel(100);
        let event_bus = EventBus::new();

        // Cron ticker runs every 1 second
        let cron_ticker = interval_at(
            tokio::time::Instant::now() + Duration::from_secs(1),
            Duration::from_secs(1),
        );

        Self {
            tasks: HashMap::new(),
            event_bus,
            command_rx,
            command_tx,
            delayed_queue: VecDeque::new(),
            cron_ticker,
            log_dir,
            shutdown: false,
        }
    }

    /// Get command sender for external control
    pub fn command_sender(&self) -> mpsc::Sender<SchedulerCommand> {
        self.command_tx.clone()
    }

    /// Register a new task
    pub fn register_task(&mut self, task: ScheduledTask) -> Result<()> {
        let name = task.name.clone();

        if self.tasks.contains_key(&name) {
            return Err(anyhow!("Task '{}' already registered", name));
        }

        // Insert task first
        self.tasks.insert(name.clone(), task);

        // Initialize cron trigger's next_tick
        if let Some(task) = self.tasks.get_mut(&name) {
            if let Trigger::Cron { .. } = &mut task.trigger {
                task.trigger.compute_next_tick();
            }
        }

        Ok(())
    }

    /// Unregister a task
    pub fn unregister_task(&mut self, name: &str) -> Result<()> {
        if self.tasks.remove(name).is_none() {
            return Err(anyhow!("Task '{}' not found", name));
        }
        Ok(())
    }

    /// Get task state
    pub fn get_task_state(&self, name: &str) -> Option<&TaskState> {
        self.tasks.get(name).map(|t| &t.state)
    }

    /// List all tasks
    pub fn list_tasks(&self) -> Vec<&ScheduledTask> {
        self.tasks.values().collect()
    }

    /// Start scheduler main loop
    pub async fn start(&mut self) -> Result<()> {
        tracing::info!("Scheduler engine starting...");

        // Emit SystemInit event
        self.event_bus
            .emit(EventType::SystemInit)
            .context("Failed to emit SystemInit event")?;

        // Spawn OneShot tasks immediately
        let oneshot_tasks: Vec<String> = self
            .tasks
            .iter()
            .filter_map(|(name, task)| {
                if matches!(task.trigger, Trigger::OneShot) {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect();

        for task_name in oneshot_tasks {
            self.spawn_task(&task_name).await?;
        }

        // Subscribe to events
        let mut event_rx = self.event_bus.subscribe();

        // Main event loop
        while !self.shutdown {
            tokio::select! {
                // Handle cron tickers
                _ = self.cron_ticker.tick() => {
                    self.tick_cron_tasks().await?;
                }
                // Handle events
                Ok(event) = event_rx.recv() => {
                    self.handle_event(event).await?;
                }
                // Handle commands
                Some(cmd) = self.command_rx.recv() => {
                    self.handle_command(cmd).await?;
                }

                // Check running processes + delayed tasks
                _ = sleep(Duration::from_millis(100)) => {
                    // Wake delayed tasks
                    let wake_tasks = self.check_delayed_tasks();
                    for task_name in wake_tasks {
                        if let Err(e) = self.spawn_task(&task_name).await {
                            tracing::error!("Failed to spawn delayed task '{}': {}", task_name, e);
                        }
                    }

                    // Check running tasks for exits
                    self.check_running_tasks().await?;
                }
            }
        }

        tracing::info!("Scheduler engine shutting down...");

        // Emit SystemShutdown event
        self.event_bus
            .emit(EventType::SystemShutdown)
            .context("Failed to emit SystemShutdown event")?;

        // Stop all running tasks
        self.stop_all_tasks().await?;

        Ok(())
    }

    /// Tick cron tasks (check if any are due)
    async fn tick_cron_tasks(&mut self) -> Result<()> {
        let now = Local::now();
        let mut due_tasks = Vec::new();

        for (name, task) in self.tasks.iter_mut() {
            if let Trigger::Cron { .. } = &task.trigger {
                if task.trigger.should_fire(now) {
                    due_tasks.push(name.clone());
                    // Compute next tick
                    task.trigger.compute_next_tick();
                }
            }
        }

        for task_name in due_tasks {
            self.spawn_task(&task_name).await?;
        }

        Ok(())
    }

    /// Check delayed tasks and wake if due
    /// Check delayed tasks and return tasks to wake
    fn check_delayed_tasks(&mut self) -> Vec<String> {
        let now = Instant::now();
        let mut wake_tasks = Vec::new();
        while let Some((_task_name, wake_time)) = self.delayed_queue.front() {
            if now >= *wake_time {
                let task_name = self.delayed_queue.pop_front().unwrap().0;
                wake_tasks.push(task_name);
            } else {
                break;
            }
        }

        wake_tasks
    }

    /// Handle event (trigger event-driven tasks)
    async fn handle_event(&mut self, event: EventType) -> Result<()> {
        let triggered_tasks: Vec<String> = self
            .tasks
            .iter()
            .filter_map(|(name, task)| {
                if let Trigger::Event { event_type } = &task.trigger {
                    if event_type == &event {
                        return Some(name.clone());
                    }
                }
                None
            })
            .collect();

        for task_name in triggered_tasks {
            self.spawn_task(&task_name).await?;
        }

        Ok(())
    }

    /// Handle command from external API/CLI
    async fn handle_command(&mut self, cmd: SchedulerCommand) -> Result<()> {
        match cmd {
            SchedulerCommand::StartTask(name) => {
                self.start_task(&name).await?;
            }
            SchedulerCommand::StopTask(name) => {
                self.stop_task(&name).await?;
            }
            SchedulerCommand::RestartTask(name) => {
                self.restart_task(&name).await?;
            }
            SchedulerCommand::Shutdown => {
                self.shutdown = true;
            }
        }
        Ok(())
    }

    /// Check running tasks for exits
    async fn check_running_tasks(&mut self) -> Result<()> {
        let mut exited_tasks = Vec::new();

        for (name, task) in self.tasks.iter_mut() {
            if let TaskState::Running { .. } = task.state {
                if let Some(process) = &mut task.process {
                    if !process.is_running() {
                        exited_tasks.push(name.clone());
                    }
                }
            }
        }

        for task_name in exited_tasks {
            self.handle_task_exit(&task_name).await?;
        }

        Ok(())
    }

    /// Spawn a task (execute it)
    async fn spawn_task(&mut self, task_name: &str) -> Result<()> {
        let task = self
            .tasks
            .get_mut(task_name)
            .ok_or_else(|| anyhow!("Task '{}' not found", task_name))?;

        // Skip if already running
        if matches!(task.state, TaskState::Running { .. }) {
            return Ok(());
        }

        // Skip if in fatal state (requires manual intervention)
        if matches!(task.state, TaskState::Fatal { .. }) {
            tracing::warn!("Task '{}' in FATAL state, skipping", task_name);
            return Ok(());
        }

        tracing::info!("Spawning task '{}'", task_name);

        // Extract command from execution
        let (command, args, env, workdir) = match &task.execution {
            Execution::Command {
                command,
                args,
                env,
                workdir,
            } => (command.clone(), args.clone(), env.clone(), workdir.clone()),
            Execution::MiseTask { task_name, args } => {
                // TODO: Phase 2.1 - For now, treat as direct command
                // In full implementation, this would query mise config
                (task_name.clone(), args.clone(), HashMap::new(), None)
            }
        };

        // Spawn process
        // Build full command vector
        let mut full_command: Vec<String> = vec![command];
        full_command.extend(args);

        // Spawn process
        let process = ProcessHandle::spawn(
            &task.name,
            &full_command,
            env,
            workdir,
            self.log_dir.clone(),
        )
        .await
        .context(format!("Failed to spawn task '{}'", task_name))?;

        let pid = process.pid();
        task.process = Some(process);
        task.state = TaskState::Running {
            pid,
            started_at: Instant::now(),
        };

        // Emit TaskStart event
        self.event_bus
            .emit(EventType::TaskStart {
                task_name: task_name.to_string(),
            })
            .ok();

        Ok(())
    }

    /// Handle task exit
    async fn handle_task_exit(&mut self, task_name: &str) -> Result<()> {
        let task = self
            .tasks
            .get_mut(task_name)
            .ok_or_else(|| anyhow!("Task '{}' not found", task_name))?;

        // Wait for process to get exit code
        let exit_code = if let Some(process) = task.process.take() {
            process.wait_for_exit().await.unwrap_or(-1)
        } else {
            -1
        };

        tracing::info!("Task '{}' exited with code {}", task_name, exit_code);

        // Emit TaskExit event
        self.event_bus
            .emit(EventType::TaskExit {
                task_name: task_name.to_string(),
                exit_code: Some(exit_code),
            })
            .ok();

        // Handle restart policy
        let should_restart = task.restart_policy.should_restart(exit_code);

        if should_restart && task.tracker.can_restart() {
            // Record restart
            task.tracker.record_restart();

            // Get delay with backoff
            let delay = task.backoff.next_delay();

            tracing::info!(
                "Task '{}' will restart in {:?} (attempt {})",
                task_name,
                delay,
                task.tracker.restart_count()
            );

            // Update state
            task.state = TaskState::Failed {
                error: format!("Exited with code {}", exit_code),
                failed_at: Instant::now(),
            };

            // Schedule delayed restart
            self.delayed_queue
                .push_back((task_name.to_string(), Instant::now() + delay));
        } else if should_restart {
            // Exceeded restart limit → Fatal state
            task.state = TaskState::Fatal {
                last_error: format!("Exited with code {}, exceeded restart limit", exit_code),
                restart_count: task.tracker.restart_count(),
            };
            tracing::error!("Task '{}' entered FATAL state", task_name);
        } else {
            // No restart, mark as completed
            task.state = TaskState::Completed {
                exit_code,
                finished_at: Instant::now(),
            };
        }

        Ok(())
    }

    /// Start a task manually
    pub async fn start_task(&mut self, task_name: &str) -> Result<()> {
        // Reset fatal state if present
        if let Some(task) = self.tasks.get_mut(task_name) {
            if matches!(task.state, TaskState::Fatal { .. }) {
                task.state = TaskState::Pending;
                task.tracker.reset();
                task.backoff.reset();
            }
        }

        self.spawn_task(task_name).await
    }

    /// Stop a task manually
    pub async fn stop_task(&mut self, task_name: &str) -> Result<()> {
        let task = self
            .tasks
            .get_mut(task_name)
            .ok_or_else(|| anyhow!("Task '{}' not found", task_name))?;

        if let Some(mut process) = task.process.take() {
            tracing::info!("Stopping task '{}'", task_name);
            process.kill().await?;
            task.state = TaskState::Completed {
                exit_code: -1,
                finished_at: Instant::now(),
            };
        }

        Ok(())
    }

    /// Restart a task manually
    pub async fn restart_task(&mut self, task_name: &str) -> Result<()> {
        self.stop_task(task_name).await?;
        tokio::time::sleep(Duration::from_millis(100)).await;
        self.start_task(task_name).await
    }

    /// Stop all running tasks
    async fn stop_all_tasks(&mut self) -> Result<()> {
        let running_tasks: Vec<String> = self
            .tasks
            .iter()
            .filter_map(|(name, task)| {
                if matches!(task.state, TaskState::Running { .. }) {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect();

        for task_name in running_tasks {
            self.stop_task(&task_name).await.ok();
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_register_task() {
        let log_dir = tempdir().unwrap().path().to_path_buf();
        let mut engine = SchedulerEngine::new(log_dir);

        let task = ScheduledTask::new(
            "test".to_string(),
            Trigger::OneShot,
            Execution::Command {
                command: "echo".to_string(),
                args: vec!["hello".to_string()],
                env: HashMap::new(),
                workdir: None,
            },
            RestartPolicy::Never,
        );

        assert!(engine.register_task(task).is_ok());
        assert_eq!(engine.tasks.len(), 1);
    }

    #[tokio::test]
    async fn test_event_bus() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();

        bus.emit(EventType::SystemInit).unwrap();

        let event = rx.recv().await.unwrap();
        assert_eq!(event, EventType::SystemInit);
    }

    #[tokio::test]
    async fn test_delayed_trigger() {
        let log_dir = tempdir().unwrap().path().to_path_buf();
        let mut engine = SchedulerEngine::new(log_dir);

        let task = ScheduledTask::new(
            "delayed".to_string(),
            Trigger::Delayed {
                delay: Duration::from_millis(100),
            },
            Execution::Command {
                command: "echo".to_string(),
                args: vec!["delayed".to_string()],
                env: HashMap::new(),
                workdir: None,
            },
            RestartPolicy::Never,
        );

        engine.register_task(task).unwrap();

        // Add to delayed queue
        engine.delayed_queue.push_back((
            "delayed".to_string(),
            Instant::now() + Duration::from_millis(100),
        ));

        // Wait for delay
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Check delayed tasks
        engine.check_delayed_tasks();

        // Task should have been spawned
        let state = engine.get_task_state("delayed").unwrap();
        assert!(matches!(state, TaskState::Running { .. }));
    }
}
