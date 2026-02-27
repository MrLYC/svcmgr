//! Service lifecycle management commands
//!
//! Phase 1.4: Process management with logging
//! - start: Start a service with log redirection
//! - stop: Stop a running service
//! - list: List all services and their status

use crate::config::models::SvcmgrConfig;
use crate::runtime::ProcessHandle;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tokio::process::Command;
/// Start a service
///
/// Phase 1.4 implementation: Run service with ProcessHandle
/// - Executes mise task if run_mode="mise"
/// - Executes direct command if run_mode="script"
/// - Records PID to ~/.config/svcmgr/pids/<service>.pid
/// - Redirects stdout/stderr to ~/.local/share/svcmgr/logs/<service>.{stdout,stderr}.log
pub async fn start(service_name: &str, config: &SvcmgrConfig) -> Result<()> {
    // Find service in config
    let service = config
        .services
        .get(service_name)
        .with_context(|| format!("Service '{}' not found in configuration", service_name))?;

    // Check if service is enabled
    if !service.enable {
        anyhow::bail!(
            "Service '{}' is disabled. Set enable=true in config to start it.",
            service_name
        );
    }

    // Check if already running
    let pid_file = get_pid_file(service_name)?;
    if pid_file.exists() {
        let pid = fs::read_to_string(&pid_file)?;
        if is_process_running(&pid) {
            anyhow::bail!(
                "Service '{}' is already running (PID: {})",
                service_name,
                pid.trim()
            );
        } else {
            // Stale PID file, remove it
            fs::remove_file(&pid_file)?;
        }
    }

    println!("🚀 Starting service '{}'...", service_name);
    // Prepare log directory
    let log_dir = dirs::data_local_dir()
        .context("Cannot determine data directory")?
        .join("svcmgr/logs");
    fs::create_dir_all(&log_dir)
        .with_context(|| format!("Failed to create log directory: {}", log_dir.display()))?;

    // Build command based on run_mode
    let command: Vec<String> = match service.run_mode {
        crate::config::models::RunMode::Mise => {
            let task_name = service.task.as_ref().with_context(|| {
                format!(
                    "Service '{}' uses run_mode='mise' but 'task' is not specified",
                    service_name
                )
            })?;
            vec!["mise".to_string(), "run".to_string(), task_name.clone()]
        }
        crate::config::models::RunMode::Script => {
            let cmd = service.command.as_ref().with_context(|| {
                format!(
                    "Service '{}' uses run_mode='script' but 'command' is not specified",
                    service_name
                )
            })?;
            // Parse command (simple split by whitespace - not shell-aware)
            let parts: Vec<&str> = cmd.split_whitespace().collect();
            if parts.is_empty() {
                anyhow::bail!("Service '{}' has empty command", service_name);
            }
            parts.iter().map(|s| s.to_string()).collect()
        }
    };

    // Prepare environment variables
    let env_vars: HashMap<String, String> = service.env.clone();

    // Prepare working directory
    let workdir = service.workdir.clone();

    // Spawn process using ProcessHandle
    let handle = ProcessHandle::spawn(service_name, &command, env_vars, workdir, log_dir.clone())
        .await
        .with_context(|| format!("Failed to start service '{}'", service_name))?;
    let pid = handle.pid();
    fs::write(&pid_file, pid.to_string())
        .with_context(|| format!("Failed to write PID file: {}", pid_file.display()))?;

    println!("✅ Service '{}' started (PID: {})", service_name, pid);
    println!("   PID file: {}", pid_file.display());
    println!("   Logs: {}/{}.stdout.log", log_dir.display(), service_name);
    println!("        {}/{}.stderr.log", log_dir.display(), service_name);

    // Wait for process to complete (foreground mode - Phase 1.3 compatibility)
    let exit_code = handle
        .wait_for_exit()
        .await
        .with_context(|| format!("Failed to wait for service '{}'", service_name))?;
    if pid_file.exists() {
        fs::remove_file(&pid_file)?;
    }

    if exit_code == 0 {
        println!("✅ Service '{}' exited successfully", service_name);
        Ok(())
    } else {
        anyhow::bail!("Service '{}' exited with code: {}", service_name, exit_code);
    }
}
/// Stop a running service
///
/// Reads PID from ~/.config/svcmgr/pids/<service>.pid and sends SIGTERM
pub async fn stop(service_name: &str) -> Result<()> {
    let pid_file = get_pid_file(service_name)?;

    if !pid_file.exists() {
        anyhow::bail!(
            "Service '{}' is not running (no PID file found)",
            service_name
        );
    }

    let pid = fs::read_to_string(&pid_file)
        .with_context(|| format!("Failed to read PID file: {}", pid_file.display()))?;
    let pid = pid.trim();

    if !is_process_running(pid) {
        println!(
            "⚠️  Service '{}' PID file exists but process is not running",
            service_name
        );
        fs::remove_file(&pid_file)?;
        anyhow::bail!("Service '{}' is not running", service_name);
    }

    println!("🛑 Stopping service '{}' (PID: {})...", service_name, pid);

    // Send SIGTERM using kill command
    let output = Command::new("kill")
        .arg("-TERM")
        .arg(pid)
        .output()
        .await
        .context("Failed to execute 'kill' command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to stop service '{}': {}", service_name, stderr);
    }

    // Wait briefly for process to exit
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Check if process is still running
    if is_process_running(pid) {
        println!(
            "⚠️  Service '{}' did not stop gracefully. You may need to use 'kill -9 {}'.",
            service_name, pid
        );
    } else {
        // Clean up PID file
        if pid_file.exists() {
            fs::remove_file(&pid_file)?;
        }
        println!("✅ Service '{}' stopped successfully", service_name);
    }

    Ok(())
}

/// List all services and their status
///
/// Shows service name, enabled status, and running status (based on PID file)
pub async fn list(config: &SvcmgrConfig) -> Result<()> {
    if config.services.is_empty() {
        println!("No services configured.");
        return Ok(());
    }

    println!("\n{:<20} {:<10} {:<10}", "SERVICE", "ENABLED", "STATUS");
    println!("{}", "-".repeat(42));

    for (name, service) in &config.services {
        let enabled_str = if service.enable {
            "enabled"
        } else {
            "disabled"
        };

        let status_str = if let Ok(pid_file) = get_pid_file(name) {
            if pid_file.exists() {
                if let Ok(pid) = fs::read_to_string(&pid_file) {
                    if is_process_running(pid.trim()) {
                        format!("running ({})", pid.trim())
                    } else {
                        "stopped (stale PID)".to_string()
                    }
                } else {
                    "stopped".to_string()
                }
            } else {
                "stopped".to_string()
            }
        } else {
            "unknown".to_string()
        };

        println!("{:<20} {:<10} {}", name, enabled_str, status_str);
    }

    println!();
    Ok(())
}

// Helper functions

/// Get PID file path for a service
fn get_pid_file(service_name: &str) -> Result<PathBuf> {
    let pid_dir = dirs::config_dir()
        .context("Cannot determine config directory")?
        .join("svcmgr/pids");
    Ok(pid_dir.join(format!("{}.pid", service_name)))
}

/// Check if a process with given PID is running
fn is_process_running(pid: &str) -> bool {
    std::process::Command::new("kill")
        .arg("-0")
        .arg(pid)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}
