// 服务管理 API 处理器
//
// 实现 OpenSpec 11-api-services.md 定义的 11 个端点:
// - GET    /api/v1/services           - 列出所有服务
// - POST   /api/v1/services           - 创建服务
// - GET    /api/v1/services/{name}    - 获取服务详情
// - PUT    /api/v1/services/{name}    - 更新服务
// - DELETE /api/v1/services/{name}    - 删除服务
// - POST   /api/v1/services/{name}/start   - 启动服务
// - POST   /api/v1/services/{name}/stop    - 停止服务
// - POST   /api/v1/services/{name}/restart - 重启服务
// - GET    /api/v1/services/{name}/logs    - 获取服务日志
// - GET    /api/v1/services/{name}/health  - 获取健康状态
// - GET    /api/v1/services/{name}/status  - 获取服务状态

use axum::{
    Json, Router,
    extract::{Path, Query},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::web::server::{ApiError, ApiResponse};

/// 服务定义请求体(用于创建和更新)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDefinition {
    pub name: String,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
    #[serde(default)]
    pub autostart: bool,
}

/// 服务状态响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub name: String,
    pub state: String, // "running", "stopped", "failed"
    pub pid: Option<u32>,
    pub uptime_seconds: Option<u64>,
}

/// 服务列表查询参数
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ListServicesQuery {
    #[serde(default)]
    pub page: usize,
    #[serde(default = "default_per_page")]
    pub per_page: usize,
    #[serde(default)]
    pub state: Option<String>, // 过滤: "running", "stopped"
}

fn default_per_page() -> usize {
    20
}

/// 日志查询参数
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct LogsQuery {
    #[serde(default = "default_lines")]
    pub lines: usize,
    #[serde(default)]
    pub follow: bool,
}

fn default_lines() -> usize {
    100
}

// ============================================================================
// 处理器函数
// ============================================================================

/// GET /api/v1/services - 列出所有服务
async fn list_services(
    Query(_params): Query<ListServicesQuery>,
) -> Result<Json<ApiResponse<Vec<ServiceStatus>>>, ApiError> {
    // TODO: 从调度引擎查询服务列表
    Ok(Json(ApiResponse {
        data: vec![],
        pagination: None,
    }))
}

/// POST /api/v1/services - 创建服务
async fn create_service(
    Json(_definition): Json<ServiceDefinition>,
) -> Result<Json<ApiResponse<ServiceStatus>>, ApiError> {
    // TODO: 实现服务创建逻辑
    Err(ApiError::new(
        "NOT_IMPLEMENTED",
        "Service creation not implemented yet",
    ))
}

/// GET /api/v1/services/{name} - 获取服务详情
async fn get_service(
    Path(_name): Path<String>,
) -> Result<Json<ApiResponse<ServiceStatus>>, ApiError> {
    // TODO: 从调度引擎查询服务详情
    Err(ApiError::new(
        "NOT_IMPLEMENTED",
        "Service query not implemented yet",
    ))
}

/// PUT /api/v1/services/{name} - 更新服务
async fn update_service(
    Path(_name): Path<String>,
    Json(_definition): Json<ServiceDefinition>,
) -> Result<Json<ApiResponse<ServiceStatus>>, ApiError> {
    // TODO: 实现服务更新逻辑
    Err(ApiError::new(
        "NOT_IMPLEMENTED",
        "Service update not implemented yet",
    ))
}

/// DELETE /api/v1/services/{name} - 删除服务
async fn delete_service(Path(_name): Path<String>) -> Result<Json<ApiResponse<()>>, ApiError> {
    // TODO: 实现服务删除逻辑
    Err(ApiError::new(
        "NOT_IMPLEMENTED",
        "Service deletion not implemented yet",
    ))
}

/// POST /api/v1/services/{name}/start - 启动服务
async fn start_service(
    Path(_name): Path<String>,
) -> Result<Json<ApiResponse<ServiceStatus>>, ApiError> {
    // TODO: 实现服务启动逻辑
    Err(ApiError::new(
        "NOT_IMPLEMENTED",
        "Service start not implemented yet",
    ))
}

/// POST /api/v1/services/{name}/stop - 停止服务
async fn stop_service(
    Path(_name): Path<String>,
) -> Result<Json<ApiResponse<ServiceStatus>>, ApiError> {
    // TODO: 实现服务停止逻辑
    Err(ApiError::new(
        "NOT_IMPLEMENTED",
        "Service stop not implemented yet",
    ))
}

/// POST /api/v1/services/{name}/restart - 重启服务
async fn restart_service(
    Path(_name): Path<String>,
) -> Result<Json<ApiResponse<ServiceStatus>>, ApiError> {
    // TODO: 实现服务重启逻辑
    Err(ApiError::new(
        "NOT_IMPLEMENTED",
        "Service restart not implemented yet",
    ))
}

/// GET /api/v1/services/{name}/logs - 获取服务日志
async fn get_service_logs(
    Path(_name): Path<String>,
    Query(_params): Query<LogsQuery>,
) -> Result<Json<ApiResponse<Vec<String>>>, ApiError> {
    // TODO: 实现日志查询逻辑
    Err(ApiError::new(
        "NOT_IMPLEMENTED",
        "Service logs not implemented yet",
    ))
}

/// GET /api/v1/services/{name}/health - 获取健康状态
async fn get_service_health(
    Path(_name): Path<String>,
) -> Result<Json<ApiResponse<HashMap<String, serde_json::Value>>>, ApiError> {
    // TODO: 实现健康检查查询逻辑
    Err(ApiError::new(
        "NOT_IMPLEMENTED",
        "Health check not implemented yet",
    ))
}

/// GET /api/v1/services/{name}/status - 获取服务状态
async fn get_service_status(
    Path(_name): Path<String>,
) -> Result<Json<ApiResponse<ServiceStatus>>, ApiError> {
    // TODO: 从调度引擎查询服务状态
    Err(ApiError::new(
        "NOT_IMPLEMENTED",
        "Service status query not implemented yet",
    ))
}

// ============================================================================
// 路由注册
// ============================================================================

/// 创建服务管理路由
pub fn routes() -> Router {
    Router::new()
        .route("/", get(list_services).post(create_service))
        .route("/:name/start", post(start_service))
        .route("/:name/stop", post(stop_service))
        .route("/:name/restart", post(restart_service))
        .route("/:name/logs", get(get_service_logs))
        .route("/:name/health", get(get_service_health))
        .route("/:name/status", get(get_service_status))
        .route(
            "/:name",
            get(get_service).put(update_service).delete(delete_service),
        )
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_definition_serialization() {
        let def = ServiceDefinition {
            name: "test-service".to_string(),
            command: "node server.js".to_string(),
            working_dir: Some("/app".to_string()),
            env: Some([("PORT".to_string(), "3000".to_string())].into()),
            autostart: true,
        };

        let json = serde_json::to_string(&def).unwrap();
        assert!(json.contains("test-service"));
        assert!(json.contains("node server.js"));
    }

    #[test]
    fn test_default_per_page() {
        assert_eq!(default_per_page(), 20);
    }

    #[test]
    fn test_default_lines() {
        assert_eq!(default_lines(), 100);
    }
}
