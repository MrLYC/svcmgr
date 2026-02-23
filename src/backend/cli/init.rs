//! svcmgr init command
//!
//! Initializes svcmgr configuration directory and Git repository
//! Configuration path: ~/.config/mise/svcmgr/

use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use tokio::process::Command;

/// Initialize svcmgr configuration directory
///
/// Creates:
/// - ~/.config/mise/svcmgr/config.toml (if not exists)
/// - ~/.config/svcmgr/pids/ (PID file directory)
/// - Git repository for configuration tracking
pub async fn init() -> Result<()> {
    let config_dir = get_config_dir()?;

    // Create config directory
    fs::create_dir_all(&config_dir).context("Failed to create config directory")?;

    // Create PID directory
    let pid_dir = dirs::config_dir()
        .context("Cannot determine config directory")?
        .join("svcmgr/pids");
    fs::create_dir_all(&pid_dir).context("Failed to create PID directory")?;

    // Generate default config.toml if not exists
    let config_path = config_dir.join("config.toml");
    if !config_path.exists() {
        let default_config = get_default_config();
        fs::write(&config_path, default_config).context("Failed to write default config")?;
        println!(
            "✅ Created default configuration: {}",
            config_path.display()
        );
    } else {
        println!(
            "⏭️  Configuration file already exists: {}",
            config_path.display()
        );
    }

    // Initialize Git repository if not exists
    let git_dir = config_dir.join(".git");
    if !git_dir.exists() {
        // git init
        let output = Command::new("git")
            .arg("init")
            .current_dir(&config_dir)
            .output()
            .await
            .context("Failed to execute 'git init'")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("git init failed: {}", stderr);
        }

        // git add config.toml
        let output = Command::new("git")
            .args(["add", "config.toml"])
            .current_dir(&config_dir)
            .output()
            .await
            .context("Failed to execute 'git add'")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("git add failed: {}", stderr);
        }

        // git commit -m "Initial svcmgr configuration"
        let output = Command::new("git")
            .args(["commit", "-m", "Initial svcmgr configuration"])
            .current_dir(&config_dir)
            .output()
            .await
            .context("Failed to execute 'git commit'")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("git commit failed: {}", stderr);
        }

        println!("✅ Initialized Git repository");
    } else {
        println!("⏭️  Git repository already exists");
    }

    println!("\n🎉 svcmgr initialized successfully!");
    println!("   Config directory: {}", config_dir.display());
    println!("   PID directory: {}", pid_dir.display());
    println!("\n📝 Next steps:");
    println!("   1. Edit {} to configure services", config_path.display());
    println!("   2. Run 'svcmgr service start <name>' to start a service");

    Ok(())
}

/// Get svcmgr configuration directory
fn get_config_dir() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .context("Cannot determine config directory")?
        .join("mise/svcmgr");
    Ok(config_dir)
}

/// Get default configuration template
fn get_default_config() -> &'static str {
    r#"# svcmgr configuration file
# Documentation: https://github.com/yourusername/svcmgr

[services.example]
# Service run mode: "mise" (use mise task) or "script" (direct command)
run_mode = "mise"

# For mise mode: specify mise task name
task = "example:run"

# For script mode (uncomment to use):
# run_mode = "script"
# command = "python -m http.server 8080"

# Lifecycle settings
enable = false  # Set to true to enable this service
restart = "on-failure"  # "no" | "on-failure" | "always"
restart_delay = "5s"  # duration format
stop_timeout = "10s"  # duration format
workdir = "."  # Working directory (relative to config dir)

# Resource limits (optional)
# cpu_max_percent = 80.0
# memory_max = 1073741824  # 1GB in bytes
# pids_max = 100

# Port management (optional)
#[services.example.ports]
#web = 8080
#api = 8081
"#
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_default_config_is_valid_toml() {
        let config = get_default_config();
        let parsed: toml::Value =
            toml::from_str(config).expect("Default config should be valid TOML");

        // Verify basic structure
        assert!(parsed.get("services").is_some());
        assert!(parsed["services"].get("example").is_some());
    }
}
