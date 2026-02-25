// ! Task API Basic Tests
//!
//! 简化版测试 - 验证任务管理核心功能可用性

use chrono::Utc;
use std::collections::HashMap;
use svcmgr::adapters::mock::MockMiseAdapter;
use svcmgr::mocks::mise::MiseMock;
use svcmgr::ports::{MiseVersion, mise_port::ConfigPort};
use svcmgr::web::api::task_models::*;

// ============================================================================
// Data Model Tests (10 tests)
// ============================================================================

#[test]
fn test_task_definition_creation() {
    let task = TaskDefinition {
        name: "test-task".to_string(),
        run: "echo hello".to_string(),
        description: Some("Test task".to_string()),
        env: HashMap::new(),
        dir: None,
        depends: vec![],
        alias: vec![],
        source: std::path::PathBuf::from("mise.toml"),
        current_execution: None,
    };

    assert_eq!(task.name, "test-task");
    assert_eq!(task.run, "echo hello");
    assert!(task.description.is_some());
}

#[test]
fn test_scheduled_task_creation() {
    let task = ScheduledTask {
        name: "backup".to_string(),
        execution: TaskExecution::Command {
            command: "tar -czf backup.tar.gz /data".to_string(),
            env: HashMap::new(),
            dir: None,
        },
        schedule: "0 2 * * *".to_string(), // 每天凌晨2点
        enabled: true,
        description: Some("Daily backup".to_string()),
        timeout: 3600, // 1小时超时
        limits: None,
        next_run: None,
        last_execution: None,
    };

    assert_eq!(task.name, "backup");
    assert_eq!(task.schedule, "0 2 * * *");
    assert!(task.enabled);
    assert_eq!(task.timeout, 3600);
}

#[test]
fn test_task_execution_mise_task() {
    let exec = TaskExecution::MiseTask {
        task: "deploy".to_string(),
        args: vec!["--env".to_string(), "production".to_string()],
    };

    match exec {
        TaskExecution::MiseTask { task, args } => {
            assert_eq!(task, "deploy");
            assert_eq!(args.len(), 2);
        }
        _ => panic!("Wrong variant"),
    }
}

#[test]
fn test_task_execution_command() {
    let exec = TaskExecution::Command {
        command: "systemctl restart nginx".to_string(),
        env: HashMap::from([("PATH".to_string(), "/usr/bin".to_string())]),
        dir: Some(std::path::PathBuf::from("/etc/nginx")),
    };

    match exec {
        TaskExecution::Command { command, env, dir } => {
            assert_eq!(command, "systemctl restart nginx");
            assert_eq!(env.get("PATH"), Some(&"/usr/bin".to_string()));
            assert!(dir.is_some());
        }
        _ => panic!("Wrong variant"),
    }
}

#[test]
fn test_execution_status_variants() {
    let status1 = ExecutionStatus::Running;
    let status2 = ExecutionStatus::Success;
    let status3 = ExecutionStatus::Failed;
    let status4 = ExecutionStatus::Cancelled;
    let status5 = ExecutionStatus::Timeout;

    assert_eq!(status1, ExecutionStatus::Running);
    assert_eq!(status2, ExecutionStatus::Success);
    assert_eq!(status3, ExecutionStatus::Failed);
    assert_eq!(status4, ExecutionStatus::Cancelled);
    assert_eq!(status5, ExecutionStatus::Timeout);
}

#[test]
fn test_trigger_type_variants() {
    let trigger1 = TriggerType::Manual;
    let trigger2 = TriggerType::Scheduled;
    let trigger3 = TriggerType::Event;

    assert_eq!(trigger1, TriggerType::Manual);
    assert_eq!(trigger2, TriggerType::Scheduled);
    assert_eq!(trigger3, TriggerType::Event);
}

#[test]
fn test_resource_limits_creation() {
    let limits = ResourceLimits {
        memory: Some(1024 * 1024 * 512), // 512 MB
        cpu_quota: Some(50000),          // 50% CPU
        cpu_weight: Some(500),
    };

    assert_eq!(limits.memory, Some(1024 * 1024 * 512));
    assert_eq!(limits.cpu_quota, Some(50000));
    assert_eq!(limits.cpu_weight, Some(500));
}

#[test]
fn test_task_execution_record() {
    let record = TaskExecutionRecord {
        execution_id: "exec_123".to_string(),
        task_name: "deploy".to_string(),
        trigger: TriggerType::Manual,
        started_at: Utc::now(),
        finished_at: None,
        status: ExecutionStatus::Running,
        exit_code: None,
        stdout: None,
        stderr: None,
    };

    assert_eq!(record.execution_id, "exec_123");
    assert_eq!(record.task_name, "deploy");
    assert_eq!(record.status, ExecutionStatus::Running);
    assert!(record.finished_at.is_none());
}

#[test]
fn test_validate_task_name_valid() {
    assert!(validate_task_name("valid_task").is_ok());
    assert!(validate_task_name("task-123").is_ok());
    assert!(validate_task_name("MyTask").is_ok());
}

#[test]
fn test_validate_task_name_invalid() {
    assert!(validate_task_name("123task").is_err()); // 必须以字母开头
    assert!(validate_task_name("task@name").is_err()); // 不允许特殊字符
    assert!(validate_task_name("").is_err()); // 不能为空
    assert!(validate_task_name(&"a".repeat(65)).is_err()); // 不能超过64字符
}

// ============================================================================
// CRUD Operations with MockMiseAdapter (15 tests)
// ============================================================================

#[tokio::test]
async fn test_create_scheduled_task() {
    let adapter = MockMiseAdapter::new(MiseMock::new(), MiseVersion::V2024);

    let task = ScheduledTask {
        name: "backup".to_string(),
        execution: TaskExecution::Command {
            command: "tar -czf backup.tar.gz /data".to_string(),
            env: HashMap::new(),
            dir: None,
        },
        schedule: "0 2 * * *".to_string(),
        enabled: true,
        description: Some("Daily backup".to_string()),
        timeout: 3600,
        limits: None,
        next_run: None,
        last_execution: None,
    };

    let result = adapter.create_scheduled_task(&task).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_get_scheduled_task() {
    let adapter = MockMiseAdapter::new(MiseMock::new(), MiseVersion::V2024);

    let task = ScheduledTask {
        name: "backup".to_string(),
        execution: TaskExecution::Command {
            command: "tar -czf backup.tar.gz /data".to_string(),
            env: HashMap::new(),
            dir: None,
        },
        schedule: "0 2 * * *".to_string(),
        enabled: true,
        description: Some("Daily backup".to_string()),
        timeout: 3600,
        limits: None,
        next_run: None,
        last_execution: None,
    };

    adapter.create_scheduled_task(&task).await.unwrap();

    let result = adapter.get_scheduled_task("backup").await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_some());
}

#[tokio::test]
async fn test_list_scheduled_tasks() {
    let adapter = MockMiseAdapter::new(MiseMock::new(), MiseVersion::V2024);

    let result = adapter.list_scheduled_tasks().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_update_scheduled_task() {
    let adapter = MockMiseAdapter::new(MiseMock::new(), MiseVersion::V2024);

    let mut task = ScheduledTask {
        name: "backup".to_string(),
        execution: TaskExecution::Command {
            command: "tar -czf backup.tar.gz /data".to_string(),
            env: HashMap::new(),
            dir: None,
        },
        schedule: "0 2 * * *".to_string(),
        enabled: true,
        description: Some("Daily backup".to_string()),
        timeout: 3600,
        limits: None,
        next_run: None,
        last_execution: None,
    };

    adapter.create_scheduled_task(&task).await.unwrap();

    // 更新任务
    task.schedule = "0 3 * * *".to_string(); // 改为凌晨3点
    task.enabled = false;

    let result = adapter.update_scheduled_task("backup", &task).await;
    assert!(result.is_ok());

    // 验证更新
    let updated = adapter.get_scheduled_task("backup").await.unwrap().unwrap();
    assert_eq!(updated.schedule, "0 3 * * *");
    assert!(!updated.enabled);
}

#[tokio::test]
async fn test_delete_scheduled_task() {
    let adapter = MockMiseAdapter::new(MiseMock::new(), MiseVersion::V2024);

    let task = ScheduledTask {
        name: "backup".to_string(),
        execution: TaskExecution::Command {
            command: "tar -czf backup.tar.gz /data".to_string(),
            env: HashMap::new(),
            dir: None,
        },
        schedule: "0 2 * * *".to_string(),
        enabled: true,
        description: Some("Daily backup".to_string()),
        timeout: 3600,
        limits: None,
        next_run: None,
        last_execution: None,
    };

    adapter.create_scheduled_task(&task).await.unwrap();

    let result = adapter.delete_scheduled_task("backup").await;
    assert!(result.is_ok());

    // 验证删除
    let deleted = adapter.get_scheduled_task("backup").await.unwrap();
    assert!(deleted.is_none());
}

#[tokio::test]
async fn test_scheduled_task_exists() {
    let adapter = MockMiseAdapter::new(MiseMock::new(), MiseVersion::V2024);

    let task = ScheduledTask {
        name: "backup".to_string(),
        execution: TaskExecution::Command {
            command: "tar -czf backup.tar.gz /data".to_string(),
            env: HashMap::new(),
            dir: None,
        },
        schedule: "0 2 * * *".to_string(),
        enabled: true,
        description: Some("Daily backup".to_string()),
        timeout: 3600,
        limits: None,
        next_run: None,
        last_execution: None,
    };

    assert!(adapter.scheduled_task_exists("backup").await.unwrap() == false);

    adapter.create_scheduled_task(&task).await.unwrap();

    assert!(adapter.scheduled_task_exists("backup").await.unwrap());
}

#[tokio::test]
async fn test_create_duplicate_scheduled_task_error() {
    let adapter = MockMiseAdapter::new(MiseMock::new(), MiseVersion::V2024);

    let task = ScheduledTask {
        name: "backup".to_string(),
        execution: TaskExecution::Command {
            command: "tar -czf backup.tar.gz /data".to_string(),
            env: HashMap::new(),
            dir: None,
        },
        schedule: "0 2 * * *".to_string(),
        enabled: true,
        description: Some("Daily backup".to_string()),
        timeout: 3600,
        limits: None,
        next_run: None,
        last_execution: None,
    };

    adapter.create_scheduled_task(&task).await.unwrap();

    // 尝试创建重复任务应该失败
    let result = adapter.create_scheduled_task(&task).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_update_nonexistent_task_error() {
    let adapter = MockMiseAdapter::new(MiseMock::new(), MiseVersion::V2024);

    let task = ScheduledTask {
        name: "nonexistent".to_string(),
        execution: TaskExecution::Command {
            command: "echo test".to_string(),
            env: HashMap::new(),
            dir: None,
        },
        schedule: "0 2 * * *".to_string(),
        enabled: true,
        description: None,
        timeout: 0,
        limits: None,
        next_run: None,
        last_execution: None,
    };

    // 更新不存在的任务应该失败
    let result = adapter.update_scheduled_task("nonexistent", &task).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_delete_nonexistent_task_error() {
    let adapter = MockMiseAdapter::new(MiseMock::new(), MiseVersion::V2024);

    // 删除不存在的任务应该失败
    let result = adapter.delete_scheduled_task("nonexistent").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_get_task_history_empty() {
    let adapter = MockMiseAdapter::new(MiseMock::new(), MiseVersion::V2024);

    let result = adapter.get_task_history("nonexistent", 10, 0).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 0);
}

#[tokio::test]
async fn test_get_task_history_pagination() {
    let adapter = MockMiseAdapter::new(MiseMock::new(), MiseVersion::V2024);

    // MVP: execution_history 默认为空，测试分页逻辑
    let result = adapter.get_task_history("test-task", 5, 0).await;
    assert!(result.is_ok());

    let result2 = adapter.get_task_history("test-task", 5, 5).await;
    assert!(result2.is_ok());
}

#[tokio::test]
async fn test_cancel_task_mvp() {
    let adapter = MockMiseAdapter::new(MiseMock::new(), MiseVersion::V2024);

    // MVP: cancel_task 是 no-op 实现，应该总是成功
    let result = adapter.cancel_task("exec_123").await;
    assert!(result.is_ok());
}

// ============================================================================
// Request/Response Serialization Tests (5 tests)
// ============================================================================

#[test]
fn test_create_scheduled_task_request_serialization() {
    let request = CreateScheduledTaskRequest {
        name: "backup".to_string(),
        execution: TaskExecution::Command {
            command: "tar -czf backup.tar.gz /data".to_string(),
            env: HashMap::new(),
            dir: None,
        },
        schedule: "0 2 * * *".to_string(),
        enabled: Some(true),
        description: Some("Daily backup".to_string()),
        timeout: Some(3600),
        limits: None,
    };

    let json = serde_json::to_string(&request).unwrap();
    assert!(json.contains("backup"));
    assert!(json.contains("0 2 * * *"));
}

#[test]
fn test_update_scheduled_task_request_partial() {
    let request = UpdateScheduledTaskRequest {
        execution: None,
        schedule: Some("0 3 * * *".to_string()),
        enabled: Some(false),
        description: None,
        timeout: None,
        limits: None,
    };

    let json = serde_json::to_string(&request).unwrap();
    assert!(json.contains("0 3 * * *"));
}

#[test]
fn test_task_execution_response_serialization() {
    let response = TaskExecutionResponse {
        execution_id: "exec_123".to_string(),
        task_name: "deploy".to_string(),
        trigger: TriggerType::Manual,
        started_at: Utc::now(),
        status: ExecutionStatus::Running,
    };

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("exec_123"));
    assert!(json.contains("deploy"));
}

#[test]
fn test_task_execution_record_deserialization() {
    let json = r#"{
        "execution_id": "exec_123",
        "task_name": "deploy",
        "trigger": "Manual",
        "started_at": "2024-01-01T00:00:00Z",
        "finished_at": "2024-01-01T00:05:00Z",
        "status": "Success",
        "exit_code": 0,
        "stdout": "Deployment successful",
        "stderr": null
    }"#;

    let record: TaskExecutionRecord = serde_json::from_str(json).unwrap();
    assert_eq!(record.execution_id, "exec_123");
    assert_eq!(record.task_name, "deploy");
    assert_eq!(record.status, ExecutionStatus::Success);
    assert_eq!(record.exit_code, Some(0));
}

#[test]
fn test_scheduled_task_roundtrip() {
    let task = ScheduledTask {
        name: "backup".to_string(),
        execution: TaskExecution::MiseTask {
            task: "backup".to_string(),
            args: vec!["--full".to_string()],
        },
        schedule: "0 2 * * *".to_string(),
        enabled: true,
        description: Some("Daily backup".to_string()),
        timeout: 3600,
        limits: Some(ResourceLimits {
            memory: Some(1024 * 1024 * 512),
            cpu_quota: Some(50000),
            cpu_weight: Some(500),
        }),
        next_run: None,
        last_execution: None,
    };

    let json = serde_json::to_string(&task).unwrap();
    let deserialized: ScheduledTask = serde_json::from_str(&json).unwrap();

    assert_eq!(task.name, deserialized.name);
    assert_eq!(task.schedule, deserialized.schedule);
    assert_eq!(task.timeout, deserialized.timeout);
}
