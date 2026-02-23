// 配置管理 API 处理器
//
// 实现 OpenSpec 14-api-config.md 定义的 10 个端点:
// - GET    /api/v1/config                - 获取完整配置
// - GET    /api/v1/config/{section}      - 获取特定段落
// - PUT    /api/v1/config                - 完整替换配置
// - PATCH  /api/v1/config/{section}      - 部分更新特定段落
// - POST   /api/v1/config/validate       - 验证配置(不实际应用)
// - GET    /api/v1/config/history        - 获取配置变更历史
// - POST   /api/v1/config/rollback       - 回滚到指定版本
// - GET    /api/v1/config/diff           - 对比两个版本的差异
// - GET    /api/v1/config/export         - 导出配置为 JSON
// - POST   /api/v1/config/import         - 导入配置并应用

use axum::{
    Json, Router,
    extract::{Path, Query},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::web::server::{ApiError, ApiResponse};

/// 完整配置(mise.toml + svcmgr.toml)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub mise: MiseConfig,
    pub svcmgr: SvcmgrConfig,
}

/// mise 配置段
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiseConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<JsonValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<JsonValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tasks: Option<JsonValue>,
}

/// svcmgr 配置段
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvcmgrConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub services: Option<JsonValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduled_tasks: Option<JsonValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub features: Option<JsonValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http: Option<JsonValue>,
}

/// 配置验证请求
#[derive(Debug, Deserialize)]
pub struct ValidateConfigRequest {
    pub config: Config,
    #[serde(default)]
    pub strict: bool, // 是否启用严格验证
}

/// 配置验证结果
#[derive(Debug, Serialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

#[derive(Debug, Serialize)]
pub struct ValidationError {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ValidationWarning {
    pub path: String,
    pub message: String,
}

/// 配置历史记录
#[derive(Debug, Serialize)]
pub struct ConfigHistoryEntry {
    pub commit_id: String,
    pub timestamp: String,
    pub author: String,
    pub message: String,
}

/// 配置回滚请求
#[derive(Debug, Deserialize)]
pub struct RollbackRequest {
    pub commit_id: String,
}

/// 配置差异查询参数
#[derive(Debug, Deserialize)]
pub struct DiffQuery {
    pub from: String, // commit_id or "HEAD~1"
    pub to: String,   // commit_id or "HEAD"
}

/// 配置差异结果
#[derive(Debug, Serialize)]
pub struct ConfigDiff {
    pub from_commit: String,
    pub to_commit: String,
    pub changes: Vec<ConfigChange>,
}

#[derive(Debug, Serialize)]
pub struct ConfigChange {
    pub section: String,
    pub key: String,
    pub change_type: String, // "added", "removed", "modified"
    pub old_value: Option<JsonValue>,
    pub new_value: Option<JsonValue>,
}

/// 配置历史查询参数
#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

fn default_limit() -> usize {
    50
}

// ============================================================================
// 处理器函数
// ============================================================================

/// GET /api/v1/config - 获取完整配置
async fn get_config() -> Result<Json<ApiResponse<Config>>, ApiError> {
    // TODO: 从文件系统读取 mise.toml 和 svcmgr.toml
    Err(ApiError::new(
        "NOT_IMPLEMENTED",
        "Config read not implemented yet",
    ))
}

/// GET /api/v1/config/{section} - 获取特定段落
async fn get_config_section(
    Path(_section): Path<String>,
) -> Result<Json<ApiResponse<JsonValue>>, ApiError> {
    // TODO: 解析指定段落(tools/env/services等)
    Err(ApiError::new(
        "NOT_IMPLEMENTED",
        "Config section query not implemented yet",
    ))
}

/// PUT /api/v1/config - 完整替换配置
async fn update_config(Json(_config): Json<Config>) -> Result<Json<ApiResponse<Config>>, ApiError> {
    // TODO: 验证配置 -> 写入文件 -> Git commit -> 触发 ConfigChanged 事件
    Err(ApiError::new(
        "NOT_IMPLEMENTED",
        "Config update not implemented yet",
    ))
}

/// PATCH /api/v1/config/{section} - 部分更新特定段落
async fn patch_config_section(
    Path(_section): Path<String>,
    Json(_data): Json<JsonValue>,
) -> Result<Json<ApiResponse<JsonValue>>, ApiError> {
    // TODO: 读取配置 -> 更新指定段落 -> 验证 -> 写入 -> Git commit
    Err(ApiError::new(
        "NOT_IMPLEMENTED",
        "Config patch not implemented yet",
    ))
}

/// POST /api/v1/config/validate - 验证配置(不实际应用)
async fn validate_config(
    Json(_request): Json<ValidateConfigRequest>,
) -> Result<Json<ApiResponse<ValidationResult>>, ApiError> {
    // TODO: 语法验证 + 语义验证(依赖检查、循环依赖、端口冲突等)
    Err(ApiError::new(
        "NOT_IMPLEMENTED",
        "Config validation not implemented yet",
    ))
}

/// GET /api/v1/config/history - 获取配置变更历史
async fn get_config_history(
    Query(_params): Query<HistoryQuery>,
) -> Result<Json<ApiResponse<Vec<ConfigHistoryEntry>>>, ApiError> {
    // TODO: 从 Git log 读取提交历史
    Ok(Json(ApiResponse {
        data: vec![],
        pagination: None,
    }))
}

/// POST /api/v1/config/rollback - 回滚到指定版本
async fn rollback_config(
    Json(_request): Json<RollbackRequest>,
) -> Result<Json<ApiResponse<Config>>, ApiError> {
    // TODO: Git reset --hard <commit> -> 重新加载配置 -> 触发 ConfigChanged 事件
    Err(ApiError::new(
        "NOT_IMPLEMENTED",
        "Config rollback not implemented yet",
    ))
}

/// GET /api/v1/config/diff - 对比两个版本的差异
async fn get_config_diff(
    Query(_params): Query<DiffQuery>,
) -> Result<Json<ApiResponse<ConfigDiff>>, ApiError> {
    // TODO: Git diff <from>..<to> -> 解析差异 -> 结构化返回
    Err(ApiError::new(
        "NOT_IMPLEMENTED",
        "Config diff not implemented yet",
    ))
}

/// GET /api/v1/config/export - 导出配置为 JSON
async fn export_config() -> Result<Json<ApiResponse<Config>>, ApiError> {
    // TODO: 读取配置 -> 序列化为 JSON
    Err(ApiError::new(
        "NOT_IMPLEMENTED",
        "Config export not implemented yet",
    ))
}

/// POST /api/v1/config/import - 导入配置并应用
async fn import_config(Json(_config): Json<Config>) -> Result<Json<ApiResponse<Config>>, ApiError> {
    // TODO: 验证配置 -> 写入文件 -> Git commit -> 重新加载
    Err(ApiError::new(
        "NOT_IMPLEMENTED",
        "Config import not implemented yet",
    ))
}

// ============================================================================
// 路由注册
// ============================================================================

/// 创建配置管理路由
pub fn routes() -> Router {
    Router::new()
        .route("/", get(get_config).put(update_config))
        .route("/validate", post(validate_config))
        .route("/history", get(get_config_history))
        .route("/rollback", post(rollback_config))
        .route("/diff", get(get_config_diff))
        .route("/export", get(export_config))
        .route("/import", post(import_config))
        .route(
            "/:section",
            get(get_config_section).patch(patch_config_section),
        )
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_serialization() {
        let config = Config {
            mise: MiseConfig {
                tools: Some(serde_json::json!({"node": "20"})),
                env: None,
                tasks: None,
            },
            svcmgr: SvcmgrConfig {
                services: None,
                scheduled_tasks: None,
                features: None,
                http: None,
            },
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("mise"));
        assert!(json.contains("svcmgr"));
    }

    #[test]
    fn test_default_limit() {
        assert_eq!(default_limit(), 50);
    }
}
