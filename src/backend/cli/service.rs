//! Service lifecycle management commands
//!
//! Phase 1.3: Basic service management
//! - start: Start a service in foreground
//! - stop: Stop a running service
//! - list: List all services and their status

use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use tokio::process::Command;

use crate::config::models::SvcmgrConfig;

/// Start a service
///
/// Phase 1.3 implementation: Run service in foreground (no systemd)
/// - Executes mise task if run_mode="mise"
/// - Executes direct command if run_mode="script"
/// - Records PID to ~/.config/svcmgr/pids/<service>.pid
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

    // Build command based on run_mode
    // Build command based on run_mode
    let mut cmd = match service.run_mode {
        crate::config::models::RunMode::Mise => {
            let task_name = service.task.as_ref().with_context(|| {
                format!(
                    "Service '{}' uses run_mode='mise' but 'task' is not specified",
                    service_name
                )
            })?;
            let mut cmd = Command::new("mise");
            cmd.arg("run").arg(task_name);
            cmd
        }

        crate::config::models::RunMode::Script => {
            let command = service.command.as_ref().with_context(|| {
                format!(
                    "Service '{}' uses run_mode='script' but 'command' is not specified",
                    service_name
                )
            })?;
            // Parse command (simple split by whitespace - not shell-aware)
            let parts: Vec<&str> = command.split_whitespace().collect();
            if parts.is_empty() {
                anyhow::bail!("Service '{}' has empty command", service_name);
            }
            let mut cmd = Command::new(parts[0]);
            for arg in &parts[1..] {
                cmd.arg(arg);
            }
            cmd
        }
    };

    // Set working directory if specified
    if let Some(workdir) = &service.workdir {
        cmd.current_dir(workdir);
    }

    // Spawn process
    let mut child = cmd
        .spawn()
        .with_context(|| format!("Failed to start service '{}'", service_name))?;

    // Get PID and write to file
    let pid = child
        .id()
        .with_context(|| format!("Failed to get PID for service '{}'", service_name))?;
    fs::write(&pid_file, pid.to_string())
        .with_context(|| format!("Failed to write PID file: {}", pid_file.display()))?;

    println!("✅ Service '{}' started (PID: {})", service_name, pid);
    println!("   PID file: {}", pid_file.display());

    // Wait for process to complete (foreground mode)
    let status = child
        .wait()
        .await
        .with_context(|| format!("Failed to wait for service '{}'", service_name))?;

    // Clean up PID file
    if pid_file.exists() {
        fs::remove_file(&pid_file)?;
    }

    if status.success() {
        println!("✅ Service '{}' exited successfully", service_name);
        Ok(())
    } else {
        anyhow::bail!("Service '{}' exited with status: {}", service_name, status);
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
