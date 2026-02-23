//! Process management for service execution
//!
//! Phase 1.4: Basic process management
//! - Spawn child processes with environment variable injection
//! - Capture and redirect stdout/stderr to log files
//! - Detect process exit and record status

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::fs::OpenOptions;

use tokio::process::{Child, Command};
use tracing::{debug, error, info, warn};

/// Handle to a managed process
#[derive(Debug)]
pub struct ProcessHandle {
    /// Process ID
    pub pid: u32,
    /// Service name
    pub name: String,
    /// Process start time
    pub start_time: DateTime<Utc>,
    /// Path to stdout log file
    pub stdout: Option<PathBuf>,
    /// Path to stderr log file
    pub stderr: Option<PathBuf>,
    /// Child process handle
    child: Child,
}

impl ProcessHandle {
    /// Spawn a new process with environment variables and log redirection
    ///
    /// # Arguments
    /// * `name` - Service name
    /// * `command` - Command to execute (first element is program, rest are args)
    /// * `env_vars` - Environment variables to inject
    /// * `workdir` - Working directory
    /// * `log_dir` - Directory for log files
    ///
    /// # Returns
    /// ProcessHandle with running child process
    pub async fn spawn(
        name: &str,
        command: &[String],
        env_vars: HashMap<String, String>,
        workdir: Option<PathBuf>,
        log_dir: PathBuf,
    ) -> Result<Self> {
        if command.is_empty() {
            anyhow::bail!("Command cannot be empty");
        }

        // Create log directory if not exists
        tokio::fs::create_dir_all(&log_dir)
            .await
            .context("Failed to create log directory")?;

        // Prepare log file paths
        let stdout_path = log_dir.join(format!("{}.stdout.log", name));
        let stderr_path = log_dir.join(format!("{}.stderr.log", name));

        debug!(
            "Spawning process: {} with command: {:?}",
            name,
            command.join(" ")
        );

        // Open log files
        let stdout_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&stdout_path)
            .await
            .context("Failed to open stdout log file")?;

        let stderr_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&stderr_path)
            .await
            .context("Failed to open stderr log file")?;

        // Build command
        let mut cmd = Command::new(&command[0]);
        if command.len() > 1 {
            cmd.args(&command[1..]);
        }

        // Set working directory
        if let Some(wd) = workdir {
            cmd.current_dir(wd);
        }

        // Inject environment variables
        cmd.envs(env_vars);

        // Redirect stdout/stderr to log files
        cmd.stdout(Stdio::from(stdout_file.into_std().await));
        cmd.stderr(Stdio::from(stderr_file.into_std().await));

        // Spawn the process
        let child = cmd
            .spawn()
            .context(format!("Failed to spawn process: {}", command[0]))?;

        let pid = child.id().context("Failed to get process ID")?;
        let start_time = Utc::now();

        info!(
            "Process spawned: {} (PID: {}, logs: {}, {})",
            name,
            pid,
            stdout_path.display(),
            stderr_path.display()
        );

        Ok(Self {
            pid,
            name: name.to_string(),
            start_time,
            stdout: Some(stdout_path),
            stderr: Some(stderr_path),
            child,
        })
    }

    /// Wait for the process to exit and return the exit status
    ///
    /// This is a blocking operation that waits until the process terminates.
    pub async fn wait_for_exit(mut self) -> Result<i32> {
        debug!(
            "Waiting for process {} (PID: {}) to exit",
            self.name, self.pid
        );

        let status = self
            .child
            .wait()
            .await
            .context("Failed to wait for process")?;

        let exit_code = status.code().unwrap_or(-1);

        if status.success() {
            info!(
                "Process {} (PID: {}) exited successfully",
                self.name, self.pid
            );
        } else {
            warn!(
                "Process {} (PID: {}) exited with code: {}",
                self.name, self.pid, exit_code
            );
        }

        Ok(exit_code)
    }

    /// Try to kill the process (sends SIGTERM)
    pub async fn kill(&mut self) -> Result<()> {
        info!("Killing process {} (PID: {})", self.name, self.pid);

        self.child.kill().await.context("Failed to kill process")?;

        Ok(())
    }

    /// Check if the process is still running
    pub fn is_running(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(Some(_status)) => false, // Process has exited
            Ok(None) => true,           // Process is still running
            Err(e) => {
                error!("Error checking process status: {}", e);
                false
            }
        }
    }

    /// Get the process ID
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Get the service name
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_spawn_simple_process() {
        let temp_dir = TempDir::new().unwrap();
        let log_dir = temp_dir.path().to_path_buf();

        // Spawn a simple echo command
        let command = vec!["echo".to_string(), "Hello, World!".to_string()];
        let env_vars = HashMap::new();

        let handle = ProcessHandle::spawn("test-echo", &command, env_vars, None, log_dir.clone())
            .await
            .unwrap();

        assert_eq!(handle.name(), "test-echo");
        assert!(handle.pid() > 0);

        // Wait for process to complete
        let exit_code = handle.wait_for_exit().await.unwrap();
        assert_eq!(exit_code, 0);

        // Verify log file exists
        let stdout_path = log_dir.join("test-echo.stdout.log");
        assert!(stdout_path.exists());
    }

    #[tokio::test]
    async fn test_process_with_env_vars() {
        let temp_dir = TempDir::new().unwrap();
        let log_dir = temp_dir.path().to_path_buf();

        // Spawn bash command that echoes an environment variable
        let command = vec![
            "bash".to_string(),
            "-c".to_string(),
            "echo $TEST_VAR".to_string(),
        ];
        let mut env_vars = HashMap::new();
        env_vars.insert("TEST_VAR".to_string(), "test_value_123".to_string());

        let handle = ProcessHandle::spawn("test-env", &command, env_vars, None, log_dir.clone())
            .await
            .unwrap();

        let exit_code = handle.wait_for_exit().await.unwrap();
        assert_eq!(exit_code, 0);

        // Read stdout log and verify environment variable was injected
        let stdout_path = log_dir.join("test-env.stdout.log");
        let content = tokio::fs::read_to_string(stdout_path).await.unwrap();
        assert!(content.contains("test_value_123"));
    }

    #[tokio::test]
    async fn test_process_is_running() {
        let temp_dir = TempDir::new().unwrap();
        let log_dir = temp_dir.path().to_path_buf();

        // Spawn a long-running process (sleep 2 seconds)
        let command = vec!["sleep".to_string(), "2".to_string()];
        let env_vars = HashMap::new();

        let mut handle =
            ProcessHandle::spawn("test-sleep", &command, env_vars, None, log_dir.clone())
                .await
                .unwrap();

        // Process should be running
        assert!(handle.is_running());

        // Kill the process
        handle.kill().await.unwrap();

        // Wait a bit for process to terminate
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Process should no longer be running
        assert!(!handle.is_running());
    }
}
