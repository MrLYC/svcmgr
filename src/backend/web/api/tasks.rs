// 任务管理 API 处理器
//
// 实现 OpenSpec 12-api-tasks.md 定义的 13 个端点:
//
// 即时任务 (Immediate Tasks):
// - GET    /api/v1/tasks                    - 列出所有 mise 任务
// - GET    /api/v1/tasks/{name}             - 获取任务详情
// - POST   /api/v1/tasks/{name}/run         - 立即执行任务
// - POST   /api/v1/tasks/{name}/cancel      - 取消正在运行的任务
// - GET    /api/v1/tasks/{name}/history     - 查询任务执行历史
//
// 定时任务 (Scheduled Tasks):
// - GET    /api/v1/scheduled-tasks          - 列出所有定时任务
// - GET    /api/v1/scheduled-tasks/{name}   - 获取定时任务详情
// - POST   /api/v1/scheduled-tasks          - 创建定时任务
// - PUT    /api/v1/scheduled-tasks/{name}   - 更新定时任务
// - DELETE /api/v1/scheduled-tasks/{name}   - 删除定时任务
// - POST   /api/v1/scheduled-tasks/{name}/enable  - 启用定时任务
// - POST   /api/v1/scheduled-tasks/{name}/disable - 禁用定时任务
// - POST   /api/v1/scheduled-tasks/{name}/run     - 立即执行(不改变定时计划)

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use uuid::Uuid;

use std::collections::HashMap;
use std::path::PathBuf;

use super::task_models::*;
use crate::web::server::{ApiError, ApiResponse, AppState};
// ============================================================================
// 查询参数结构体
// ============================================================================

/// 任务列表查询参数
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ListTasksQuery {
    #[serde(default)]
    pub page: usize,
    #[serde(default = "default_per_page")]
    pub per_page: usize,
}

fn default_per_page() -> usize {
    20
}

/// 任务历史查询参数
#[derive(Debug, Deserialize)]
pub struct TaskHistoryQuery {
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

fn default_limit() -> u32 {
    50
}

// ============================================================================
// 即时任务处理器 (Immediate Tasks)
// ============================================================================

/// GET /api/v1/tasks - 列出所有 mise 任务
async fn list_tasks(
    State(state): State<AppState>,
    Query(_params): Query<ListTasksQuery>,
) -> Result<Json<ApiResponse<Vec<TaskDefinition>>>, ApiError> {
    // 从 ConfigPort 获取任务列表
    let config_port = &state.config_port;

    let tasks = config_port
        .list_tasks()
        .await
        .map_err(|e| ApiError::new("TASK_LIST_FAILED", format!("Failed to list tasks: {}", e)))?;

    // 将 TaskInfo 转换为 TaskDefinition
    let task_defs: Vec<TaskDefinition> = tasks
        .into_iter()
        .map(|info| TaskDefinition {
            name: info.name,
            run: info.command,
            description: info.description, // TaskInfo.description 已经是 Option<String>
            env: HashMap::new(),           // TaskInfo 无 env 字段
            dir: None,
            depends: info.depends, // 使用 TaskInfo 的 depends
            alias: Vec::new(),
            source: PathBuf::from("<mise>"), // TaskInfo 无 source
            current_execution: None,
        })
        .collect();

    Ok(Json(ApiResponse {
        data: task_defs,
        pagination: None,
    }))
}

/// POST /api/v1/tasks - 创建并立即执行任务
#[derive(Debug, Deserialize)]
pub struct CreateImmediateTaskRequest {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

async fn create_immediate_task(
    State(state): State<AppState>,
    Json(req): Json<CreateImmediateTaskRequest>,
) -> Result<Json<ApiResponse<ImmediateTaskState>>, ApiError> {
    // 创建并执行任务
    let task_id = state.task_executor.create_task(req.command, req.args).await;

    // 获取任务状态
    let task_state = state
        .task_executor
        .get_task(&task_id)
        .await
        .ok_or_else(|| ApiError::internal_error("Failed to create task"))?;

    Ok(Json(ApiResponse {
        data: task_state,
        pagination: None,
    }))
}

/// GET /api/v1/tasks/{id_or_name} - 获取任务详情（即时任务或 mise 任务）
async fn get_task(
    State(state): State<AppState>,
    Path(id_or_name): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    // 先尝试作为 UUID 查询即时任务
    if Uuid::parse_str(&id_or_name).is_ok() {
        if let Some(task_state) = state.task_executor.get_task(&id_or_name).await {
            return Ok(Json(ApiResponse {
                data: serde_json::to_value(task_state).unwrap(),
                pagination: None,
            }));
        }
    }

    // 否则作为 name 查询 mise 任务
    let config_port = &state.config_port;

    // 获取任务命令
    let task_cmd = config_port
        .get_task_command(&id_or_name)
        .await
        .map_err(|e| {
            if e.to_string().to_lowercase().contains("not found") {
                ApiError::not_found(format!("Task '{}'", id_or_name))
            } else {
                ApiError::internal_error(e.to_string())
            }
        })?;

    // 构造 TaskDefinition
    let task_def = TaskDefinition {
        name: id_or_name.clone(),
        run: task_cmd.command,
        description: None,
        env: task_cmd.env,
        dir: task_cmd.workdir,
        depends: Vec::new(),
        alias: Vec::new(),
        source: std::path::PathBuf::from("mise.toml"), // MVP: 默认路径
        current_execution: None,
    };

    Ok(Json(ApiResponse {
        data: serde_json::to_value(task_def).unwrap(),
        pagination: None,
    }))
}

/// POST /api/v1/tasks/{name}/run - 立即执行任务
async fn run_task(
    State(_state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<ApiResponse<TaskExecutionRecord>>, ApiError> {
    // MVP: 返回占位符执行记录
    // TODO: 实现真实的任务执行逻辑
    let exec = TaskExecutionRecord {
        execution_id: format!("exec_{}", Uuid::new_v4()),
        task_name: name,
        trigger: TriggerType::Manual,
        started_at: chrono::Utc::now(),
        status: ExecutionStatus::Running,
        finished_at: None,
        exit_code: None,
        pid: None,
        stdout_preview: String::new(),
        stderr_preview: String::new(),
        log_file: std::path::PathBuf::from("/tmp/placeholder.log"),
    };

    Ok(Json(ApiResponse {
        data: exec,
        pagination: None,
    }))
}

/// POST /api/v1/tasks/{id_or_name}/cancel - 取消正在运行的任务（即时任务或 mise 任务）
async fn cancel_task(
    State(state): State<AppState>,
    Path(id_or_name): Path<String>,
) -> Result<StatusCode, ApiError> {
    // 先尝试作为 UUID 取消即时任务
    if Uuid::parse_str(&id_or_name).is_ok() {
        match state.task_executor.cancel_task(&id_or_name).await {
            Ok(_) => return Ok(StatusCode::NO_CONTENT),
            Err(_) => {
                // 如果即时任务不存在，继续尝试 mise 任务
            }
        }
    }

    // 否则作为 execution_id 取消 mise 任务
    let config_port = &state.config_port;

    // 调用 TaskPort::cancel_task (MVP: no-op implementation)
    config_port
        .cancel_task(&id_or_name)
        .await
        .map_err(|e| ApiError::new("CANCEL_FAILED", format!("Failed to cancel task: {}", e)))?;

    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/v1/tasks/{name}/history - 查询任务执行历史
async fn get_task_history(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(params): Query<TaskHistoryQuery>,
) -> Result<Json<ApiResponse<Vec<TaskExecutionRecord>>>, ApiError> {
    let config_port = &state.config_port;

    let history = config_port
        .get_task_history(&name, params.limit, params.offset)
        .await
        .map_err(|e| {
            ApiError::new(
                "HISTORY_QUERY_FAILED",
                format!("Failed to query history: {}", e),
            )
        })?;

    Ok(Json(ApiResponse {
        data: history,
        pagination: None,
    }))
}

// ============================================================================
// 定时任务处理器 (Scheduled Tasks)
// ============================================================================

/// GET /api/v1/scheduled-tasks - 列出所有定时任务
async fn list_scheduled_tasks(
    State(state): State<AppState>,
    Query(_params): Query<ListTasksQuery>,
) -> Result<Json<ApiResponse<Vec<ScheduledTask>>>, ApiError> {
    let config_port = &state.config_port;

    let tasks = config_port.list_scheduled_tasks().await.map_err(|e| {
        ApiError::new(
            "SCHEDULED_TASK_LIST_FAILED",
            format!("Failed to list scheduled tasks: {}", e),
        )
    })?;

    Ok(Json(ApiResponse {
        data: tasks,
        pagination: None,
    }))
}

/// GET /api/v1/scheduled-tasks/{name} - 获取定时任务详情
async fn get_scheduled_task(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<ApiResponse<ScheduledTask>>, ApiError> {
    let config_port = &state.config_port;

    let task = config_port
        .get_scheduled_task(&name)
        .await
        .map_err(|e| {
            ApiError::new(
                "QUERY_FAILED",
                format!("Failed to query scheduled task: {}", e),
            )
        })?
        .ok_or_else(|| {
            ApiError::new(
                "SCHEDULED_TASK_NOT_FOUND",
                format!("Scheduled task '{}' not found", name),
            )
        })?;

    Ok(Json(ApiResponse {
        data: task,
        pagination: None,
    }))
}

/// POST /api/v1/scheduled-tasks - 创建定时任务
async fn create_scheduled_task(
    State(state): State<AppState>,
    Json(request): Json<CreateScheduledTaskRequest>,
) -> Result<(StatusCode, Json<ApiResponse<ScheduledTask>>), ApiError> {
    let config_port = &state.config_port;

    // 验证任务名称
    validate_task_name(&request.name).map_err(|e| ApiError::new("VALIDATION_ERROR", e))?;

    // 验证 cron 表达式
    validate_cron_expression(&request.schedule)
        .map_err(|e| ApiError::new("VALIDATION_ERROR", e))?;

    // 验证超时设置
    validate_timeout(request.timeout).map_err(|e| ApiError::new("VALIDATION_ERROR", e))?;

    // 验证资源限制
    if let Some(ref limits) = request.limits {
        validate_resource_limits(limits).map_err(|e| ApiError::new("VALIDATION_ERROR", e))?;
    }

    // 检查任务是否已存在
    if config_port
        .scheduled_task_exists(&request.name)
        .await
        .unwrap_or(false)
    {
        return Err(ApiError::new(
            "SCHEDULED_TASK_ALREADY_EXISTS",
            format!("Scheduled task '{}' already exists", request.name),
        ));
    }

    // 构造 ScheduledTask
    let task = ScheduledTask {
        name: request.name.clone(),
        execution: request.execution,
        schedule: request.schedule,
        enabled: request.enabled,
        description: request.description,
        timeout: request.timeout,
        limits: request.limits,
        next_run: None, // 运行时计算
        last_execution: None,
    };

    // 创建定时任务
    config_port
        .create_scheduled_task(&task)
        .await
        .map_err(|e| {
            ApiError::new(
                "CREATE_FAILED",
                format!("Failed to create scheduled task: {}", e),
            )
        })?;

    // Git 自动暂存和提交
    let mut git = state.git_versioning.lock().await;
    if let Err(e) = git.auto_stage() {
        eprintln!("Warning: Failed to stage git changes: {}", e);
    }
    if let Err(e) = git.commit(&format!("feat: 创建定时任务 '{}'", task.name), None) {
        eprintln!("Warning: Failed to commit git changes: {}", e);
    }

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse {
            data: task,
            pagination: None,
        }),
    ))
}

/// PUT /api/v1/scheduled-tasks/{name} - 更新定时任务
async fn update_scheduled_task(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(request): Json<UpdateScheduledTaskRequest>,
) -> Result<Json<ApiResponse<ScheduledTask>>, ApiError> {
    let config_port = &state.config_port;

    // 验证 cron 表达式
    if let Some(ref schedule) = request.schedule {
        validate_cron_expression(schedule).map_err(|e| ApiError::new("VALIDATION_ERROR", e))?;
    }

    // 验证超时设置
    if let Some(timeout) = request.timeout {
        validate_timeout(timeout).map_err(|e| ApiError::new("VALIDATION_ERROR", e))?;
    }

    // 验证资源限制
    if let Some(ref limits) = request.limits {
        validate_resource_limits(limits).map_err(|e| ApiError::new("VALIDATION_ERROR", e))?;
    }

    // 获取现有任务
    let mut task = config_port
        .get_scheduled_task(&name)
        .await
        .map_err(|e| {
            ApiError::new(
                "QUERY_FAILED",
                format!("Failed to query scheduled task: {}", e),
            )
        })?
        .ok_or_else(|| {
            ApiError::new(
                "SCHEDULED_TASK_NOT_FOUND",
                format!("Scheduled task '{}' not found", name),
            )
        })?;

    if let Some(schedule) = request.schedule {
        task.schedule = schedule;
    }
    if let Some(enabled) = request.enabled {
        task.enabled = enabled;
    }
    if let Some(timeout) = request.timeout {
        task.timeout = timeout;
    }
    if let Some(limits) = request.limits {
        task.limits = Some(limits);
    }

    // 更新定时任务
    config_port
        .update_scheduled_task(&name, &task)
        .await
        .map_err(|e| {
            ApiError::new(
                "UPDATE_FAILED",
                format!("Failed to update scheduled task: {}", e),
            )
        })?;

    // Git 自动暂存和提交
    let mut git = state.git_versioning.lock().await;
    if let Err(e) = git.auto_stage() {
        eprintln!("Warning: Failed to stage git changes: {}", e);
    }
    if let Err(e) = git.commit(&format!("feat: 更新定时任务 '{}'", name), None) {
        eprintln!("Warning: Failed to commit git changes: {}", e);
    }

    Ok(Json(ApiResponse {
        data: task,
        pagination: None,
    }))
}

/// DELETE /api/v1/scheduled-tasks/{name} - 删除定时任务
async fn delete_scheduled_task(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<StatusCode, ApiError> {
    let config_port = &state.config_port;

    // 检查任务是否存在
    if !config_port
        .scheduled_task_exists(&name)
        .await
        .unwrap_or(false)
    {
        return Err(ApiError::new(
            "SCHEDULED_TASK_NOT_FOUND",
            format!("Scheduled task '{}' not found", name),
        ));
    }

    // 删除定时任务
    config_port
        .delete_scheduled_task(&name)
        .await
        .map_err(|e| {
            ApiError::new(
                "DELETE_FAILED",
                format!("Failed to delete scheduled task: {}", e),
            )
        })?;

    // Git 自动暂存和提交
    let mut git = state.git_versioning.lock().await;
    if let Err(e) = git.auto_stage() {
        eprintln!("Warning: Failed to stage git changes: {}", e);
    }
    if let Err(e) = git.commit(&format!("feat: 删除定时任务 '{}'", name), None) {
        eprintln!("Warning: Failed to commit git changes: {}", e);
    }

    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/v1/scheduled-tasks/{name}/enable - 启用定时任务
async fn enable_scheduled_task(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<ApiResponse<ScheduledTask>>, ApiError> {
    let config_port = &state.config_port;

    // 获取现有任务
    let mut task = config_port
        .get_scheduled_task(&name)
        .await
        .map_err(|e| {
            ApiError::new(
                "QUERY_FAILED",
                format!("Failed to query scheduled task: {}", e),
            )
        })?
        .ok_or_else(|| {
            ApiError::new(
                "SCHEDULED_TASK_NOT_FOUND",
                format!("Scheduled task '{}' not found", name),
            )
        })?;

    // 设置 enabled = true
    task.enabled = true;

    // 更新定时任务
    config_port
        .update_scheduled_task(&name, &task)
        .await
        .map_err(|e| {
            ApiError::new(
                "UPDATE_FAILED",
                format!("Failed to enable scheduled task: {}", e),
            )
        })?;

    // Git 自动暂存和提交
    let mut git = state.git_versioning.lock().await;
    if let Err(e) = git.auto_stage() {
        eprintln!("Warning: Failed to stage git changes: {}", e);
    }
    if let Err(e) = git.commit(&format!("feat: 启用定时任务 '{}'", name), None) {
        eprintln!("Warning: Failed to commit git changes: {}", e);
    }

    Ok(Json(ApiResponse {
        data: task,
        pagination: None,
    }))
}

/// POST /api/v1/scheduled-tasks/{name}/disable - 禁用定时任务
async fn disable_scheduled_task(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<ApiResponse<ScheduledTask>>, ApiError> {
    let config_port = &state.config_port;

    // 获取现有任务
    let mut task = config_port
        .get_scheduled_task(&name)
        .await
        .map_err(|e| {
            ApiError::new(
                "QUERY_FAILED",
                format!("Failed to query scheduled task: {}", e),
            )
        })?
        .ok_or_else(|| {
            ApiError::new(
                "SCHEDULED_TASK_NOT_FOUND",
                format!("Scheduled task '{}' not found", name),
            )
        })?;

    // 设置 enabled = false
    task.enabled = false;

    // 更新定时任务
    config_port
        .update_scheduled_task(&name, &task)
        .await
        .map_err(|e| {
            ApiError::new(
                "UPDATE_FAILED",
                format!("Failed to disable scheduled task: {}", e),
            )
        })?;

    // Git 自动暂存和提交
    let mut git = state.git_versioning.lock().await;
    if let Err(e) = git.auto_stage() {
        eprintln!("Warning: Failed to stage git changes: {}", e);
    }
    if let Err(e) = git.commit(&format!("feat: 禁用定时任务 '{}'", name), None) {
        eprintln!("Warning: Failed to commit git changes: {}", e);
    }

    Ok(Json(ApiResponse {
        data: task,
        pagination: None,
    }))
}

/// POST /api/v1/scheduled-tasks/{name}/run - 立即执行(不改变定时计划)
async fn run_scheduled_task(
    State(_state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<ApiResponse<TaskExecutionRecord>>, ApiError> {
    // MVP: 返回占位符执行记录
    // TODO: 实现真实的任务执行逻辑
    let exec = TaskExecutionRecord {
        execution_id: format!("exec_{}", Uuid::new_v4()),
        task_name: name,
        trigger: TriggerType::Manual,
        started_at: chrono::Utc::now(),
        status: ExecutionStatus::Running,
        finished_at: None,
        exit_code: None,
        pid: None,
        stdout_preview: String::new(),
        stderr_preview: String::new(),
        log_file: std::path::PathBuf::from("/tmp/placeholder.log"),
    };

    Ok(Json(ApiResponse {
        data: exec,
        pagination: None,
    }))
}

// ============================================================================
// 路由注册
// ============================================================================

/// 创建即时任务路由
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_tasks).post(create_immediate_task))
        .route("/:name", get(get_task))
        .route("/:name/run", post(run_task))
        .route("/:name/cancel", post(cancel_task))
        .route("/:name/history", get(get_task_history))
}

/// 创建定时任务路由
pub fn scheduled_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_scheduled_tasks).post(create_scheduled_task))
        .route(
            "/:name",
            get(get_scheduled_task)
                .put(update_scheduled_task)
                .delete(delete_scheduled_task),
        )
        .route("/:name/enable", post(enable_scheduled_task))
        .route("/:name/disable", post(disable_scheduled_task))
        .route("/:name/run", post(run_scheduled_task))
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_per_page() {
        assert_eq!(default_per_page(), 20);
    }

    #[test]
    fn test_default_limit() {
        assert_eq!(default_limit(), 50);
    }

    #[test]
    fn test_list_tasks_query_deserialization() {
        let json = r#"{"page": 2, "per_page": 50}"#;
        let query: ListTasksQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.page, 2);
        assert_eq!(query.per_page, 50);
    }

    #[test]
    fn test_task_history_query_defaults() {
        let json = r#"{}"#;
        let query: TaskHistoryQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.limit, 50);
        assert_eq!(query.offset, 0);
    }
}
