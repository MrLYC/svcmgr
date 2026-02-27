//! mise CLI command builder
//!
//! Constructs mise CLI commands for subprocess execution.

use std::path::Path;
use tokio::process::Command;

#[derive(Debug, Clone, Default)]
pub struct MiseCommand {}

impl MiseCommand {
    pub fn install(tool: &str, version: &str) -> Command {
        let mut cmd = Command::new("mise");
        cmd.arg("install").arg(format!("{}@{}", tool, version));
        cmd
    }

    pub fn list_installed() -> Command {
        let mut cmd = Command::new("mise");
        cmd.arg("ls").arg("--json");
        cmd
    }

    pub fn use_tool(tool: &str, version: &str) -> Command {
        let mut cmd = Command::new("mise");
        cmd.arg("use").arg(format!("{}@{}", tool, version));
        cmd
    }

    pub fn uninstall(tool: &str, version: &str) -> Command {
        let mut cmd = Command::new("mise");
        cmd.arg("uninstall").arg(format!("{}@{}", tool, version));
        cmd
    }

    pub fn run_task(name: &str, args: &[String]) -> Command {
        let mut cmd = Command::new("mise");
        cmd.arg("run").arg(name);
        cmd.args(args);
        cmd
    }

    pub fn list_tasks() -> Command {
        let mut cmd = Command::new("mise");
        cmd.arg("tasks").arg("ls").arg("--json");
        cmd
    }

    pub fn env_json() -> Command {
        let mut cmd = Command::new("mise");
        cmd.arg("env").arg("--json");
        cmd
    }

    pub fn env_for_dir(dir: &Path) -> Command {
        let mut cmd = Command::new("mise");
        cmd.arg("env").arg("--json");
        cmd.current_dir(dir);
        cmd
    }

    pub fn config_ls() -> Command {
        let mut cmd = Command::new("mise");
        cmd.arg("config").arg("ls");
        cmd
    }
}

#[cfg(test)]
mod tests {
    // use super::*; // Removed: unused import
    // Note: tokio::process::Command doesn't expose get_program()/get_args() methods
    // (unlike std::process::Command). These are intentionally simple builders,
    // so we rely on integration tests rather than unit tests for command construction.
    //
    // The command construction logic is straightforward and low-risk:
    // - Each method creates a Command with mise binary name
    // - Appends specific arguments
    // - Returns the Command for execution
    //
    // Integration tests in tests/port_adapter_integration.rs verify the full
    // command execution flow end-to-end.
}
