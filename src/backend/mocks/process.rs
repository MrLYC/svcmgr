//! FakeProcessManager - 虚拟进程管理器用于测试
//!
//! 模拟进程生命周期、状态转换、重启行为和健康检查,
//! 用于测试调度引擎和进程管理逻辑,无需启动真实进程。

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessState {
    Starting,
    Running,
    Stopping,
    Stopped,
    Failed,
}

#[derive(Debug, Clone)]
pub struct FakeProcess {
    pub name: String,
    pub pid: u32,
    pub state: ProcessState,
    pub command: String,
    pub start_time: DateTime<Utc>,
    pub exit_code: Option<i32>,
    pub restart_count: u32,
}

#[derive(Debug, Clone)]
pub enum ProcessEvent {
    Started {
        name: String,
        pid: u32,
        time: DateTime<Utc>,
    },
    Stopped {
        name: String,
        exit_code: i32,
        time: DateTime<Utc>,
    },
    Restarted {
        name: String,
        attempt: u32,
        time: DateTime<Utc>,
    },
    HealthCheckFailed {
        name: String,
        time: DateTime<Utc>,
    },
}

pub struct FakeProcessManager {
    processes: Arc<Mutex<HashMap<String, FakeProcess>>>,
    history: Arc<Mutex<Vec<ProcessEvent>>>,
    next_pid: Arc<Mutex<u32>>,
}

impl Default for FakeProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeProcessManager {
    pub fn new() -> Self {
        Self {
            processes: Arc::new(Mutex::new(HashMap::new())),
            history: Arc::new(Mutex::new(Vec::new())),
            next_pid: Arc::new(Mutex::new(1000)),
        }
    }

    fn allocate_pid(&self) -> u32 {
        let mut next_pid = self.next_pid.lock().unwrap();
        let pid = *next_pid;
        *next_pid += 1;
        pid
    }

    pub async fn start(&self, name: &str, command: &str) -> Result<u32> {
        let mut processes = self.processes.lock().unwrap();

        if processes.contains_key(name) {
            anyhow::bail!("Process '{}' already exists", name);
        }

        let pid = self.allocate_pid();
        let now = Utc::now();

        let process = FakeProcess {
            name: name.to_string(),
            pid,
            state: ProcessState::Running,
            command: command.to_string(),
            start_time: now,
            exit_code: None,
            restart_count: 0,
        };

        processes.insert(name.to_string(), process);

        let mut history = self.history.lock().unwrap();
        history.push(ProcessEvent::Started {
            name: name.to_string(),
            pid,
            time: now,
        });

        Ok(pid)
    }

    pub async fn stop(&self, name: &str) -> Result<()> {
        let mut processes = self.processes.lock().unwrap();

        let process = processes
            .get_mut(name)
            .context(format!("Process '{}' not found", name))?;

        process.state = ProcessState::Stopped;
        process.exit_code = Some(0);

        let mut history = self.history.lock().unwrap();
        history.push(ProcessEvent::Stopped {
            name: name.to_string(),
            exit_code: 0,
            time: Utc::now(),
        });

        Ok(())
    }

    pub fn get_state(&self, name: &str) -> Option<ProcessState> {
        let processes = self.processes.lock().unwrap();
        processes.get(name).map(|p| p.state.clone())
    }

    pub fn get_process(&self, name: &str) -> Option<FakeProcess> {
        let processes = self.processes.lock().unwrap();
        processes.get(name).cloned()
    }

    pub fn get_history(&self) -> Vec<ProcessEvent> {
        let history = self.history.lock().unwrap();
        history.clone()
    }

    pub fn simulate_crash(&self, name: &str, exit_code: i32) -> Result<()> {
        let mut processes = self.processes.lock().unwrap();

        let process = processes
            .get_mut(name)
            .context(format!("Process '{}' not found", name))?;

        process.state = ProcessState::Failed;
        process.exit_code = Some(exit_code);

        let mut history = self.history.lock().unwrap();
        history.push(ProcessEvent::Stopped {
            name: name.to_string(),
            exit_code,
            time: Utc::now(),
        });

        Ok(())
    }

    pub fn simulate_restart(&self, name: &str) -> Result<u32> {
        let mut processes = self.processes.lock().unwrap();

        let process = processes
            .get_mut(name)
            .context(format!("Process '{}' not found", name))?;

        let new_pid = self.allocate_pid();
        let now = Utc::now();

        process.pid = new_pid;
        process.state = ProcessState::Running;
        process.exit_code = None;
        process.start_time = now;
        process.restart_count += 1;

        let restart_count = process.restart_count;

        let mut history = self.history.lock().unwrap();
        history.push(ProcessEvent::Restarted {
            name: name.to_string(),
            attempt: restart_count,
            time: now,
        });

        Ok(new_pid)
    }

    pub fn simulate_health_check_failure(&self, name: &str) -> Result<()> {
        let processes = self.processes.lock().unwrap();

        if !processes.contains_key(name) {
            anyhow::bail!("Process '{}' not found", name);
        }

        let mut history = self.history.lock().unwrap();
        history.push(ProcessEvent::HealthCheckFailed {
            name: name.to_string(),
            time: Utc::now(),
        });

        Ok(())
    }

    pub fn list_processes(&self) -> Vec<FakeProcess> {
        let processes = self.processes.lock().unwrap();
        processes.values().cloned().collect()
    }

    pub fn clear(&self) {
        let mut processes = self.processes.lock().unwrap();
        processes.clear();
        let mut history = self.history.lock().unwrap();
        history.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_start_process() {
        let manager = FakeProcessManager::new();
        let pid = manager.start("test-service", "echo hello").await.unwrap();

        assert!(pid >= 1000);
        assert_eq!(
            manager.get_state("test-service"),
            Some(ProcessState::Running)
        );

        let process = manager.get_process("test-service").unwrap();
        assert_eq!(process.name, "test-service");
        assert_eq!(process.command, "echo hello");
        assert_eq!(process.restart_count, 0);
    }

    #[tokio::test]
    async fn test_stop_process() {
        let manager = FakeProcessManager::new();
        manager.start("test-service", "echo hello").await.unwrap();
        manager.stop("test-service").await.unwrap();

        assert_eq!(
            manager.get_state("test-service"),
            Some(ProcessState::Stopped)
        );

        let process = manager.get_process("test-service").unwrap();
        assert_eq!(process.exit_code, Some(0));
    }

    #[tokio::test]
    async fn test_duplicate_start() {
        let manager = FakeProcessManager::new();
        manager.start("test-service", "echo hello").await.unwrap();

        let result = manager.start("test-service", "echo hello").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    #[tokio::test]
    async fn test_simulate_crash() {
        let manager = FakeProcessManager::new();
        manager.start("test-service", "echo hello").await.unwrap();
        manager.simulate_crash("test-service", 137).unwrap();

        assert_eq!(
            manager.get_state("test-service"),
            Some(ProcessState::Failed)
        );

        let process = manager.get_process("test-service").unwrap();
        assert_eq!(process.exit_code, Some(137));
    }

    #[tokio::test]
    async fn test_simulate_restart() {
        let manager = FakeProcessManager::new();
        let original_pid = manager.start("test-service", "echo hello").await.unwrap();

        manager.simulate_crash("test-service", 1).unwrap();
        let new_pid = manager.simulate_restart("test-service").unwrap();

        assert_ne!(original_pid, new_pid);
        assert_eq!(
            manager.get_state("test-service"),
            Some(ProcessState::Running)
        );

        let process = manager.get_process("test-service").unwrap();
        assert_eq!(process.restart_count, 1);
        assert_eq!(process.exit_code, None);
    }

    #[tokio::test]
    async fn test_event_history() {
        let manager = FakeProcessManager::new();
        let pid = manager.start("test-service", "echo hello").await.unwrap();
        manager.stop("test-service").await.unwrap();

        let history = manager.get_history();
        assert_eq!(history.len(), 2);

        match &history[0] {
            ProcessEvent::Started {
                name,
                pid: event_pid,
                ..
            } => {
                assert_eq!(name, "test-service");
                assert_eq!(*event_pid, pid);
            }
            _ => panic!("Expected Started event"),
        }

        match &history[1] {
            ProcessEvent::Stopped {
                name, exit_code, ..
            } => {
                assert_eq!(name, "test-service");
                assert_eq!(*exit_code, 0);
            }
            _ => panic!("Expected Stopped event"),
        }
    }

    #[tokio::test]
    async fn test_health_check_failure() {
        let manager = FakeProcessManager::new();
        manager.start("test-service", "echo hello").await.unwrap();
        manager
            .simulate_health_check_failure("test-service")
            .unwrap();

        let history = manager.get_history();
        assert!(
            history
                .iter()
                .any(|e| matches!(e, ProcessEvent::HealthCheckFailed { .. }))
        );
    }

    #[tokio::test]
    async fn test_list_processes() {
        let manager = FakeProcessManager::new();
        manager.start("service-1", "cmd1").await.unwrap();
        manager.start("service-2", "cmd2").await.unwrap();

        let processes = manager.list_processes();
        assert_eq!(processes.len(), 2);

        let names: Vec<String> = processes.iter().map(|p| p.name.clone()).collect();
        assert!(names.contains(&"service-1".to_string()));
        assert!(names.contains(&"service-2".to_string()));
    }
}
