// 任务管理 API 处理器
//
// 实现 OpenSpec 12-api-tasks.md 定义的 13 个端点:
//
// 即时任务:
// - GET    /api/v1/tasks                    - 列出所有 mise 任务
// - GET    /api/v1/tasks/{name}             - 获取任务详情
// - POST   /api/v1/tasks/{name}/run         - 立即执行任务
// - POST   /api/v1/tasks/{name}/cancel      - 取消正在运行的任务
// - GET    /api/v1/tasks/{name}/history     - 查询任务执行历史
//
// 定时任务:
// - GET    /api/v1/scheduled-tasks          - 列出所有定时任务
// - GET    /api/v1/scheduled-tasks/{name}   - 获取定时任务详情
// - POST   /api/v1/scheduled-tasks          - 创建定时任务
// - PUT    /api/v1/scheduled-tasks/{name}   - 更新定时任务
// - DELETE /api/v1/scheduled-tasks/{name}   - 删除定时任务
// - POST   /api/v1/scheduled-tasks/{name}/enable  - 启用定时任务
// - POST   /api/v1/scheduled-tasks/{name}/disable - 禁用定时任务
// - POST   /api/v1/scheduled-tasks/{name}/run     - 立即执行(不改变定时计划)

use axum::{
    Json, Router,
    extract::{Path, Query},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::web::server::{ApiError, ApiResponse};

/// mise 任务定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDefinition {
    pub name: String,
    pub run: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

/// 定时任务定义(用于创建和更新)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTaskDefinition {
    pub name: String,
    pub command: String,  // 可以是 mise 任务引用或直接命令
    pub schedule: String, // Cron 表达式
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
}

fn default_enabled() -> bool {
    true
}

/// 任务执行记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskExecution {
    pub id: String,
    pub task_name: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub exit_code: Option<i32>,
    pub status: String, // "running", "success", "failed", "cancelled"
}

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
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct TaskHistoryQuery {
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub status: Option<String>, // 过滤: "success", "failed"
}

fn default_limit() -> usize {
    50
}

// ============================================================================
// 即时任务处理器
// ============================================================================

/// GET /api/v1/tasks - 列出所有 mise 任务
async fn list_tasks(
    Query(_params): Query<ListTasksQuery>,
) -> Result<Json<ApiResponse<Vec<TaskDefinition>>>, ApiError> {
    // TODO: 从 mise 配置解析任务列表
    Ok(Json(ApiResponse {
        data: vec![],
        pagination: None,
    }))
}

/// GET /api/v1/tasks/{name} - 获取任务详情
async fn get_task(
    Path(_name): Path<String>,
) -> Result<Json<ApiResponse<TaskDefinition>>, ApiError> {
    // TODO: 从 mise 配置解析指定任务
    Err(ApiError::new(
        "NOT_IMPLEMENTED",
        "Task query not implemented yet",
    ))
}

/// POST /api/v1/tasks/{name}/run - 立即执行任务
async fn run_task(Path(_name): Path<String>) -> Result<Json<ApiResponse<TaskExecution>>, ApiError> {
    // TODO: 通过调度引擎提交 OneShot 任务
    Err(ApiError::new(
        "NOT_IMPLEMENTED",
        "Task execution not implemented yet",
    ))
}

/// POST /api/v1/tasks/{name}/cancel - 取消正在运行的任务
async fn cancel_task(Path(_name): Path<String>) -> Result<Json<ApiResponse<()>>, ApiError> {
    // TODO: 实现任务取消逻辑
    Err(ApiError::new(
        "NOT_IMPLEMENTED",
        "Task cancellation not implemented yet",
    ))
}

/// GET /api/v1/tasks/{name}/history - 查询任务执行历史
async fn get_task_history(
    Path(_name): Path<String>,
    Query(_params): Query<TaskHistoryQuery>,
) -> Result<Json<ApiResponse<Vec<TaskExecution>>>, ApiError> {
    // TODO: 从数据库或日志查询执行历史
    Ok(Json(ApiResponse {
        data: vec![],
        pagination: None,
    }))
}

// ============================================================================
// 定时任务处理器
// ============================================================================

/// GET /api/v1/scheduled-tasks - 列出所有定时任务
async fn list_scheduled_tasks(
    Query(_params): Query<ListTasksQuery>,
) -> Result<Json<ApiResponse<Vec<ScheduledTaskDefinition>>>, ApiError> {
    // TODO: 从 svcmgr 配置解析定时任务列表
    Ok(Json(ApiResponse {
        data: vec![],
        pagination: None,
    }))
}

/// GET /api/v1/scheduled-tasks/{name} - 获取定时任务详情
async fn get_scheduled_task(
    Path(_name): Path<String>,
) -> Result<Json<ApiResponse<ScheduledTaskDefinition>>, ApiError> {
    // TODO: 从 svcmgr 配置解析指定定时任务
    Err(ApiError::new(
        "NOT_IMPLEMENTED",
        "Scheduled task query not implemented yet",
    ))
}

/// POST /api/v1/scheduled-tasks - 创建定时任务
async fn create_scheduled_task(
    Json(_definition): Json<ScheduledTaskDefinition>,
) -> Result<Json<ApiResponse<ScheduledTaskDefinition>>, ApiError> {
    // TODO: 实现定时任务创建逻辑
    Err(ApiError::new(
        "NOT_IMPLEMENTED",
        "Scheduled task creation not implemented yet",
    ))
}

/// PUT /api/v1/scheduled-tasks/{name} - 更新定时任务
async fn update_scheduled_task(
    Path(_name): Path<String>,
    Json(_definition): Json<ScheduledTaskDefinition>,
) -> Result<Json<ApiResponse<ScheduledTaskDefinition>>, ApiError> {
    // TODO: 实现定时任务更新逻辑
    Err(ApiError::new(
        "NOT_IMPLEMENTED",
        "Scheduled task update not implemented yet",
    ))
}

/// DELETE /api/v1/scheduled-tasks/{name} - 删除定时任务
async fn delete_scheduled_task(
    Path(_name): Path<String>,
) -> Result<Json<ApiResponse<()>>, ApiError> {
    // TODO: 实现定时任务删除逻辑
    Err(ApiError::new(
        "NOT_IMPLEMENTED",
        "Scheduled task deletion not implemented yet",
    ))
}

/// POST /api/v1/scheduled-tasks/{name}/enable - 启用定时任务
async fn enable_scheduled_task(
    Path(_name): Path<String>,
) -> Result<Json<ApiResponse<ScheduledTaskDefinition>>, ApiError> {
    // TODO: 实现定时任务启用逻辑
    Err(ApiError::new(
        "NOT_IMPLEMENTED",
        "Scheduled task enable not implemented yet",
    ))
}

/// POST /api/v1/scheduled-tasks/{name}/disable - 禁用定时任务
async fn disable_scheduled_task(
    Path(_name): Path<String>,
) -> Result<Json<ApiResponse<ScheduledTaskDefinition>>, ApiError> {
    // TODO: 实现定时任务禁用逻辑
    Err(ApiError::new(
        "NOT_IMPLEMENTED",
        "Scheduled task disable not implemented yet",
    ))
}

/// POST /api/v1/scheduled-tasks/{name}/run - 立即执行(不改变定时计划)
async fn run_scheduled_task(
    Path(_name): Path<String>,
) -> Result<Json<ApiResponse<TaskExecution>>, ApiError> {
    // TODO: 实现定时任务立即执行逻辑
    Err(ApiError::new(
        "NOT_IMPLEMENTED",
        "Scheduled task run not implemented yet",
    ))
}

// ============================================================================
// 路由注册
// ============================================================================

/// 创建即时任务路由
pub fn routes() -> Router {
    Router::new()
        .route("/", get(list_tasks))
        .route("/:name/run", post(run_task))
        .route("/:name/cancel", post(cancel_task))
        .route("/:name/history", get(get_task_history))
        .route("/:name", get(get_task))
}

/// 创建定时任务路由
pub fn scheduled_routes() -> Router {
    Router::new()
        .route("/", get(list_scheduled_tasks).post(create_scheduled_task))
        .route("/:name/enable", post(enable_scheduled_task))
        .route("/:name/disable", post(disable_scheduled_task))
        .route("/:name/run", post(run_scheduled_task))
        .route(
            "/:name",
            get(get_scheduled_task)
                .put(update_scheduled_task)
                .delete(delete_scheduled_task),
        )
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_definition_serialization() {
        let task = TaskDefinition {
            name: "test-task".to_string(),
            run: "echo 'Hello'".to_string(),
            description: Some("Test task".to_string()),
            env: [("FOO".to_string(), "bar".to_string())].into(),
        };

        let json = serde_json::to_string(&task).unwrap();
        assert!(json.contains("test-task"));
    }

    #[test]
    fn test_scheduled_task_defaults() {
        let json = r#"{"name":"backup","command":"backup.sh","schedule":"0 2 * * *"}"#;
        let task: ScheduledTaskDefinition = serde_json::from_str(json).unwrap();
        assert!(task.enabled); // default = true
    }

    #[test]
    fn test_default_per_page() {
        assert_eq!(default_per_page(), 20);
    }

    #[test]
    fn test_default_limit() {
        assert_eq!(default_limit(), 50);
    }
}
