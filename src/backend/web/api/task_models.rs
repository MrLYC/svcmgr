use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// ============================================================================
// 核心数据模型
// ============================================================================

/// mise 任务定义（从 .config/mise/config.toml 解析）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskDefinition {
    /// 任务名称（唯一标识）
    pub name: String,

    /// 运行命令（run 字段）
    pub run: String,

    /// 任务描述（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// 环境变量
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// 工作目录
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir: Option<PathBuf>,

    /// 依赖任务（在此任务运行前先运行）
    #[serde(default)]
    pub depends: Vec<String>,

    /// 别名（可通过 alias 名称调用任务）
    #[serde(default)]
    pub alias: Vec<String>,

    /// 任务来源（mise 文件路径）
    pub source: PathBuf,

    /// 当前执行状态（运行时字段，不序列化到配置）
    #[serde(skip)]
    pub current_execution: Option<CurrentExecution>,
}

/// 当前执行状态（简要信息）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CurrentExecution {
    pub execution_id: String,
    pub started_at: DateTime<Utc>,
    pub status: ExecutionStatus,
    pub pid: Option<u32>,
}

/// 定时任务配置（存储在 .config/mise/svcmgr/config.toml）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScheduledTask {
    /// 任务名称（唯一标识）
    pub name: String,

    /// 执行方式
    #[serde(flatten)]
    pub execution: TaskExecution,

    /// cron 表达式
    pub schedule: String,

    /// 是否启用
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// 任务描述
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// 超时时间（秒，0 = 无超时）
    #[serde(default)]
    pub timeout: u64,

    /// 资源限制（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limits: Option<ResourceLimits>,

    /// 下次运行时间（运行时计算，不存储）
    #[serde(skip)]
    pub next_run: Option<DateTime<Utc>>,

    /// 最近一次执行（运行时字段，不序列化到配置）
    #[serde(skip)]
    pub last_execution: Option<LastExecutionSummary>,
}

/// 任务执行方式
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum TaskExecution {
    /// 执行 mise 任务
    #[serde(rename = "mise_task")]
    MiseTask {
        /// mise 任务名称
        task: String,
        /// 参数
        #[serde(default)]
        args: Vec<String>,
    },

    /// 直接执行命令
    #[serde(rename = "command")]
    Command {
        /// Shell 命令
        command: String,
        /// 环境变量
        #[serde(default)]
        env: HashMap<String, String>,
        /// 工作目录
        #[serde(skip_serializing_if = "Option::is_none")]
        dir: Option<PathBuf>,
    },
}

/// 资源限制（cgroups v2）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResourceLimits {
    /// 内存限制（字节）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<u64>,

    /// CPU 配额（微秒/100ms，例如 50000 = 50% CPU）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_quota: Option<u64>,

    /// CPU 权重（1-10000，默认 100）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_weight: Option<u64>,
}

/// 任务执行记录
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskExecutionRecord {
    /// 执行 ID（UUID）
    pub execution_id: String,

    /// 任务名称
    pub task_name: String,

    /// 开始时间
    pub started_at: DateTime<Utc>,

    /// 结束时间（None = 仍在运行）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,

    /// 退出码（None = 仍在运行或被取消）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,

    /// 执行状态
    pub status: ExecutionStatus,

    /// 触发方式
    pub trigger: TriggerType,

    /// 进程 PID（运行时）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,

    /// 标准输出（前 10KB，完整日志见 log_file）
    #[serde(default)]
    pub stdout_preview: String,

    /// 标准错误（前 10KB，完整日志见 log_file）
    #[serde(default)]
    pub stderr_preview: String,

    /// 完整日志文件路径
    pub log_file: PathBuf,
}

/// 执行状态
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionStatus {
    /// 运行中
    Running,
    /// 成功（退出码 0）
    Success,
    /// 失败（退出码非 0）
    Failed,
    /// 被取消
    Cancelled,
    /// 超时
    Timeout,
}

/// 触发类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TriggerType {
    /// 手动触发（API 调用）
    Manual,
    /// 定时触发（cron）
    Scheduled,
    /// 事件触发
    Event,
}

/// 最近执行摘要（用于列表响应）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LastExecutionSummary {
    pub execution_id: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub status: ExecutionStatus,
}

fn default_true() -> bool {
    true
}

// ============================================================================
// 请求/响应类型
// ============================================================================

/// 列出 mise 任务的查询参数
#[derive(Debug, Clone, Deserialize)]
pub struct ListTasksQuery {
    /// 任务来源过滤
    #[serde(default = "default_source_all")]
    pub source: String, // "all" | "global" | "local"

    /// 执行状态过滤
    #[serde(default = "default_status_all")]
    pub status: String, // "all" | "running" | "idle"

    /// 分页参数
    #[serde(default = "default_page")]
    pub page: u32,

    #[serde(default = "default_per_page")]
    pub per_page: u32,
}

fn default_source_all() -> String {
    "all".to_string()
}
fn default_status_all() -> String {
    "all".to_string()
}
fn default_page() -> u32 {
    1
}
fn default_per_page() -> u32 {
    20
}

/// 列出 mise 任务的响应
#[derive(Debug, Clone, Serialize)]
pub struct ListTasksResponse {
    pub tasks: Vec<TaskDefinition>,
}

/// 获取任务详情的响应
#[derive(Debug, Clone, Serialize)]
pub struct GetTaskResponse {
    #[serde(flatten)]
    pub task: TaskDefinition,

    /// 最近一次执行记录（无论成功失败）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_execution: Option<TaskExecutionRecord>,

    /// 引用此任务的所有定时任务
    #[serde(default)]
    pub schedules: Vec<ScheduledTaskSummary>,
}

/// 定时任务摘要（用于关联显示）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTaskSummary {
    pub name: String,
    pub schedule: String,
    pub enabled: bool,
    pub next_run: Option<DateTime<Utc>>,
}

/// 运行任务请求
#[derive(Debug, Clone, Deserialize)]
pub struct RunTaskRequest {
    /// 传递给任务的参数（追加到 mise run 后）
    #[serde(default)]
    pub args: Vec<String>,

    /// 额外环境变量（合并到任务 env）
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// 超时时间（秒，0 = 无超时）
    #[serde(default)]
    pub timeout: u64,

    /// 是否等待任务完成（默认 false）
    #[serde(default)]
    pub wait: bool,
}

/// 运行任务响应
#[derive(Debug, Clone, Serialize)]
pub struct RunTaskResponse {
    pub execution_id: String,
    pub task_name: String,
    pub started_at: DateTime<Utc>,
    pub status: ExecutionStatus,
    pub pid: Option<u32>,
    pub log_file: PathBuf,

    // wait=true 时包含以下字段
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
}

/// 取消任务请求
#[derive(Debug, Clone, Deserialize)]
pub struct CancelTaskRequest {
    /// 发送信号（默认 SIGTERM）
    #[serde(default = "default_sigterm")]
    pub signal: String, // "SIGTERM" | "SIGKILL"

    /// 等待进程退出的超时（秒，0 = 立即 SIGKILL）
    #[serde(default = "default_cancel_timeout")]
    pub timeout: u64,
}

fn default_sigterm() -> String {
    "SIGTERM".to_string()
}
fn default_cancel_timeout() -> u64 {
    10
}

/// 取消任务响应
#[derive(Debug, Clone, Serialize)]
pub struct CancelTaskResponse {
    pub execution_id: String,
    pub task_name: String,
    pub status: ExecutionStatus,
    pub finished_at: DateTime<Utc>,
    pub signal_sent: String,
}

/// 查询任务执行历史的参数
#[derive(Debug, Clone, Deserialize)]
pub struct TaskHistoryQuery {
    /// 返回记录数（默认 20，最大 100）
    #[serde(default = "default_history_limit")]
    pub limit: u32,

    /// 分页偏移（默认 0）
    #[serde(default)]
    pub offset: u32,

    /// 状态过滤
    #[serde(default = "default_status_all")]
    pub status: String, // "all" | "success" | "failed" | "cancelled"

    /// 触发方式过滤
    #[serde(default = "default_trigger_all")]
    pub trigger: String, // "all" | "manual" | "scheduled"
}

fn default_history_limit() -> u32 {
    20
}
fn default_trigger_all() -> String {
    "all".to_string()
}

/// 任务执行历史响应
#[derive(Debug, Clone, Serialize)]
pub struct TaskHistoryResponse {
    pub task_name: String,
    pub executions: Vec<TaskExecutionRecord>,
}

/// 列出定时任务的查询参数
#[derive(Debug, Clone, Deserialize)]
pub struct ListScheduledTasksQuery {
    /// 启用状态过滤
    #[serde(default = "default_enabled_all")]
    pub enabled: String, // "all" | "true" | "false"

    /// 分页参数
    #[serde(default = "default_page")]
    pub page: u32,

    #[serde(default = "default_per_page")]
    pub per_page: u32,
}

fn default_enabled_all() -> String {
    "all".to_string()
}

/// 列出定时任务的响应
#[derive(Debug, Clone, Serialize)]
pub struct ListScheduledTasksResponse {
    pub scheduled_tasks: Vec<ScheduledTask>,
}

/// 创建定时任务请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateScheduledTaskRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub execution: TaskExecution,
    pub schedule: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub timeout: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limits: Option<ResourceLimits>,
}

/// 更新定时任务请求（所有字段可选）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateScheduledTaskRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limits: Option<ResourceLimits>,
}

/// 启用/禁用定时任务响应
#[derive(Debug, Clone, Serialize)]
pub struct ToggleScheduledTaskResponse {
    pub name: String,
    pub enabled: bool,
    pub next_run: Option<DateTime<Utc>>,
}

/// 批量操作请求
#[derive(Debug, Clone, Deserialize)]
pub struct BatchScheduledTaskRequest {
    /// 操作类型
    pub operation: String, // "enable" | "disable"

    /// 任务名称数组（最多 50 个）
    pub names: Vec<String>,
}

/// 批量操作响应
#[derive(Debug, Clone, Serialize)]
pub struct BatchScheduledTaskResponse {
    pub succeeded: Vec<ToggleScheduledTaskResponse>,
    pub failed: Vec<BatchOperationFailure>,
}

/// 批量操作失败项
#[derive(Debug, Clone, Serialize)]
pub struct BatchOperationFailure {
    pub name: String,
    pub error: String,
}

// ============================================================================
// 验证函数
// ============================================================================

impl TaskDefinition {
    /// 验证任务定义的有效性
    pub fn validate(&self) -> Result<(), String> {
        validate_task_name(&self.name)?;

        if self.run.trim().is_empty() {
            return Err("run command cannot be empty".to_string());
        }

        // 验证依赖任务名称格式
        for dep in &self.depends {
            validate_task_name(dep)?;
        }

        Ok(())
    }
}

impl ScheduledTask {
    /// 验证定时任务配置的有效性
    pub fn validate(&self) -> Result<(), String> {
        validate_task_name(&self.name)?;
        validate_cron_expression(&self.schedule)?;
        validate_timeout(self.timeout)?;

        if let Some(ref limits) = self.limits {
            validate_resource_limits(limits)?;
        }

        // 验证执行方式
        match &self.execution {
            TaskExecution::MiseTask { task, .. } => {
                validate_task_name(task)?;
            }
            TaskExecution::Command { command, .. } => {
                if command.trim().is_empty() {
                    return Err("command cannot be empty".to_string());
                }
            }
        }

        Ok(())
    }
}

impl CreateScheduledTaskRequest {
    /// 验证创建请求
    pub fn validate(&self) -> Result<(), String> {
        validate_task_name(&self.name)?;
        validate_cron_expression(&self.schedule)?;
        validate_timeout(self.timeout)?;

        if let Some(ref limits) = self.limits {
            validate_resource_limits(limits)?;
        }

        match &self.execution {
            TaskExecution::MiseTask { task, .. } => {
                validate_task_name(task)?;
            }
            TaskExecution::Command { command, .. } => {
                if command.trim().is_empty() {
                    return Err("command cannot be empty".to_string());
                }
            }
        }

        Ok(())
    }
}

impl BatchScheduledTaskRequest {
    /// 验证批量操作请求
    pub fn validate(&self) -> Result<(), String> {
        if self.operation != "enable" && self.operation != "disable" {
            return Err(format!(
                "operation must be 'enable' or 'disable', got '{}'",
                self.operation
            ));
        }

        if self.names.is_empty() {
            return Err("names array cannot be empty".to_string());
        }

        if self.names.len() > 50 {
            return Err("cannot operate on more than 50 tasks at once".to_string());
        }

        for name in &self.names {
            validate_task_name(name)?;
        }

        Ok(())
    }
}

/// 验证任务名称
pub fn validate_task_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("task name cannot be empty".to_string());
    }

    if name.len() > 64 {
        return Err(format!("task name too long: {} chars (max 64)", name.len()));
    }

    let first_char = name.chars().next().unwrap();
    if !first_char.is_ascii_alphabetic() {
        return Err("task name must start with a letter".to_string());
    }

    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err("task name can only contain letters, numbers, and underscores".to_string());
    }

    Ok(())
}

/// 验证 cron 表达式（简单格式检查）
pub fn validate_cron_expression(schedule: &str) -> Result<(), String> {
    let fields: Vec<&str> = schedule.split_whitespace().collect();
    if fields.len() != 5 && fields.len() != 6 {
        return Err(format!(
            "cron expression must have 5 or 6 fields, got {}",
            fields.len()
        ));
    }
    Ok(())
}

/// 验证超时时间
pub fn validate_timeout(timeout: u64) -> Result<(), String> {
    if timeout > 86400 {
        return Err(format!(
            "timeout too large: {} seconds (max 86400 = 24 hours)",
            timeout
        ));
    }
    Ok(())
}

/// 验证资源限制
pub fn validate_resource_limits(limits: &ResourceLimits) -> Result<(), String> {
    if let Some(memory) = limits.memory {
        if memory < 1_048_576 {
            return Err(format!(
                "memory limit too small: {} bytes (min 1MB = 1048576 bytes)",
                memory
            ));
        }
    }

    if let Some(cpu_quota) = limits.cpu_quota {
        if !(1000..=100000).contains(&cpu_quota) {
            return Err(format!(
                "cpu_quota out of range: {} (must be 1000-100000 for 1%-100% CPU)",
                cpu_quota
            ));
        }
    }

    if let Some(cpu_weight) = limits.cpu_weight {
        if !(1..=10000).contains(&cpu_weight) {
            return Err(format!(
                "cpu_weight out of range: {} (must be 1-10000)",
                cpu_weight
            ));
        }
    }

    Ok(())
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_task_name_valid() {
        assert!(validate_task_name("backup").is_ok());
        assert!(validate_task_name("deploy_prod").is_ok());
        assert!(validate_task_name("test123").is_ok());
        assert!(validate_task_name("a").is_ok());
    }

    #[test]
    fn test_validate_task_name_invalid() {
        assert!(validate_task_name("").is_err());
        assert!(validate_task_name("123task").is_err()); // 数字开头
        assert!(validate_task_name("task-name").is_err()); // 含连字符
        assert!(validate_task_name("task.name").is_err()); // 含点号
        assert!(validate_task_name(&"a".repeat(65)).is_err()); // 超长
    }

    #[test]
    fn test_validate_cron_expression_valid() {
        assert!(validate_cron_expression("0 3 * * *").is_ok()); // 5字段
        assert!(validate_cron_expression("0 0 3 * * *").is_ok()); // 6字段
        assert!(validate_cron_expression("*/5 * * * *").is_ok());
    }

    #[test]
    fn test_validate_cron_expression_invalid() {
        assert!(validate_cron_expression("0 3 * *").is_err()); // 4字段
        assert!(validate_cron_expression("0 3 * * * * *").is_err()); // 7字段
        assert!(validate_cron_expression("").is_err());
    }

    #[test]
    fn test_validate_timeout() {
        assert!(validate_timeout(0).is_ok());
        assert!(validate_timeout(3600).is_ok());
        assert!(validate_timeout(86400).is_ok());
        assert!(validate_timeout(86401).is_err());
    }

    #[test]
    fn test_validate_resource_limits_valid() {
        let limits = ResourceLimits {
            memory: Some(1_048_576), // 1MB
            cpu_quota: Some(50000),  // 50%
            cpu_weight: Some(100),
        };
        assert!(validate_resource_limits(&limits).is_ok());
    }

    #[test]
    fn test_validate_resource_limits_invalid_memory() {
        let limits = ResourceLimits {
            memory: Some(1000), // < 1MB
            cpu_quota: None,
            cpu_weight: None,
        };
        assert!(validate_resource_limits(&limits).is_err());
    }

    #[test]
    fn test_validate_resource_limits_invalid_cpu_quota() {
        let limits = ResourceLimits {
            memory: None,
            cpu_quota: Some(500), // < 1%
            cpu_weight: None,
        };
        assert!(validate_resource_limits(&limits).is_err());

        let limits2 = ResourceLimits {
            memory: None,
            cpu_quota: Some(150000), // > 100%
            cpu_weight: None,
        };
        assert!(validate_resource_limits(&limits2).is_err());
    }

    #[test]
    fn test_task_definition_validate() {
        let task = TaskDefinition {
            name: "backup".to_string(),
            run: "pg_dump mydb".to_string(),
            description: Some("Database backup".to_string()),
            env: HashMap::new(),
            dir: Some(PathBuf::from("/data")),
            depends: vec![],
            alias: vec!["db:backup".to_string()],
            source: PathBuf::from("/home/user/.config/mise/config.toml"),
            current_execution: None,
        };
        assert!(task.validate().is_ok());
    }

    #[test]
    fn test_task_definition_validate_invalid_name() {
        let task = TaskDefinition {
            name: "123task".to_string(), // 数字开头
            run: "echo test".to_string(),
            description: None,
            env: HashMap::new(),
            dir: None,
            depends: vec![],
            alias: vec![],
            source: PathBuf::from("/test/config.toml"),
            current_execution: None,
        };
        assert!(task.validate().is_err());
    }

    #[test]
    fn test_task_definition_validate_empty_run() {
        let task = TaskDefinition {
            name: "test".to_string(),
            run: "   ".to_string(), // 空白命令
            description: None,
            env: HashMap::new(),
            dir: None,
            depends: vec![],
            alias: vec![],
            source: PathBuf::from("/test/config.toml"),
            current_execution: None,
        };
        assert!(task.validate().is_err());
    }

    #[test]
    fn test_scheduled_task_validate() {
        let task = ScheduledTask {
            name: "nightly_backup".to_string(),
            description: Some("Daily backup".to_string()),
            execution: TaskExecution::MiseTask {
                task: "backup".to_string(),
                args: vec!["--incremental".to_string()],
            },
            schedule: "0 3 * * *".to_string(),
            enabled: true,
            timeout: 3600,
            limits: None,
            next_run: None,
            last_execution: None,
        };
        assert!(task.validate().is_ok());
    }

    #[test]
    fn test_scheduled_task_validate_invalid_cron() {
        let task = ScheduledTask {
            name: "invalid".to_string(),
            description: None,
            execution: TaskExecution::Command {
                command: "echo test".to_string(),
                env: HashMap::new(),
                dir: None,
            },
            schedule: "invalid cron".to_string(),
            enabled: true,
            timeout: 0,
            limits: None,
            next_run: None,
            last_execution: None,
        };
        assert!(task.validate().is_err());
    }

    #[test]
    fn test_batch_request_validate() {
        let req = BatchScheduledTaskRequest {
            operation: "enable".to_string(),
            names: vec!["task1".to_string(), "task2".to_string()],
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_batch_request_validate_invalid_operation() {
        let req = BatchScheduledTaskRequest {
            operation: "delete".to_string(),
            names: vec!["task1".to_string()],
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_batch_request_validate_too_many() {
        let req = BatchScheduledTaskRequest {
            operation: "enable".to_string(),
            names: (0..51).map(|i| format!("task{}", i)).collect(),
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn test_execution_status_serde() {
        let status = ExecutionStatus::Running;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"running\"");

        let status2: ExecutionStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, status2);
    }

    #[test]
    fn test_trigger_type_serde() {
        let trigger = TriggerType::Scheduled;
        let json = serde_json::to_string(&trigger).unwrap();
        assert_eq!(json, "\"scheduled\"");

        let trigger2: TriggerType = serde_json::from_str(&json).unwrap();
        assert_eq!(trigger, trigger2);
    }

    #[test]
    fn test_task_execution_serde_mise_task() {
        let exec = TaskExecution::MiseTask {
            task: "backup".to_string(),
            args: vec!["--full".to_string()],
        };
        let json = serde_json::to_string(&exec).unwrap();
        assert!(json.contains("\"type\":\"mise_task\""));
        assert!(json.contains("\"task\":\"backup\""));

        let exec2: TaskExecution = serde_json::from_str(&json).unwrap();
        assert_eq!(exec, exec2);
    }

    #[test]
    fn test_task_execution_serde_command() {
        let mut env = HashMap::new();
        env.insert("VAR1".to_string(), "value1".to_string());

        let exec = TaskExecution::Command {
            command: "echo test".to_string(),
            env,
            dir: Some(PathBuf::from("/tmp")),
        };
        let json = serde_json::to_string(&exec).unwrap();
        assert!(json.contains("\"type\":\"command\""));
        assert!(json.contains("\"command\":\"echo test\""));

        let exec2: TaskExecution = serde_json::from_str(&json).unwrap();
        assert_eq!(exec, exec2);
    }
}

// ============================================================================
// 即时任务 (Immediate Task) 数据模型
// ============================================================================

/// 即时任务状态
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ImmediateTaskStatus {
    /// 等待执行
    Pending,
    /// 执行中
    Running,
    /// 成功完成
    Succeeded,
    /// 执行失败
    Failed,
    /// 已取消
    Cancelled,
}

/// 即时任务状态信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImmediateTaskState {
    /// 任务 ID (UUID v4)
    pub id: String,
    /// 任务状态
    pub status: ImmediateTaskStatus,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 开始时间
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,
    /// 完成时间
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,
    /// 退出码
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// 标准输出
    #[serde(default)]
    pub stdout: String,
    /// 标准错误
    #[serde(default)]
    pub stderr: String,
    /// 错误信息（执行失败或取消时）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
