//! 环境变量管理 API 数据模型
//!
//! 根据 OpenSpec 15-api-env.md 定义的数据结构

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::env::EnvScope;

// ============================================================================
// 请求参数定义
// ============================================================================

/// GET /api/v1/env 查询参数
#[derive(Debug, Deserialize)]
pub struct ListEnvVarsParams {
    /// 作用域过滤: ["global", "service:api", "task:build"]
    #[serde(default)]
    pub scopes: Vec<String>,
    /// 按前缀过滤
    pub prefix: Option<String>,
    /// 按值搜索
    pub search: Option<String>,
    /// 是否展开变量引用
    #[serde(default)]
    pub expand: bool,
    /// 页码(从1开始)
    #[serde(default = "default_page")]
    pub page: u32,
    /// 每页条数
    #[serde(default = "default_per_page")]
    pub per_page: u32,
}

fn default_page() -> u32 {
    1
}

fn default_per_page() -> u32 {
    20
}

/// GET /api/v1/env/{key} 查询参数
#[derive(Debug, Deserialize)]
pub struct GetEnvVarParams {
    /// 是否展开变量引用
    #[serde(default)]
    pub expand: bool,
}

/// PUT /api/v1/env/{key} 请求体
#[derive(Debug, Deserialize)]
pub struct SetEnvVarRequest {
    /// 变量值
    pub value: String,
    /// 作用域: "global", "service:api", "task:build"
    pub scope: String,
}

/// DELETE /api/v1/env/{key} 查询参数
#[derive(Debug, Deserialize)]
pub struct DeleteEnvVarQuery {
    /// 作用域: "global", "service:api", "task:build"
    pub scope: String,
}

// ============================================================================
// 批量操作数据结构
// ============================================================================

/// POST /api/v1/env/batch 请求体
#[derive(Debug, Deserialize)]
pub struct EnvBatchRequest {
    /// 设置操作列表
    #[serde(default)]
    pub set: Vec<EnvBatchSetItem>,
    /// 删除操作列表
    #[serde(default)]
    pub delete: Vec<EnvBatchDeleteItem>,
}

/// 批量设置项
#[derive(Debug, Deserialize)]
pub struct EnvBatchSetItem {
    /// 变量名
    pub key: String,
    /// 变量值
    pub value: String,
    /// 作用域: "global", "service:api", "task:build"
    pub scope: String,
}

/// 批量删除项
#[derive(Debug, Deserialize)]
pub struct EnvBatchDeleteItem {
    /// 变量名
    pub key: String,
    /// 作用域: "global", "service:api", "task:build"
    pub scope: String,
}

/// 批量操作结果
#[derive(Debug, Serialize)]
pub struct EnvBatchResult {
    /// 成功设置的变量数
    pub set_count: usize,
    /// 成功删除的变量数
    pub delete_count: usize,
    /// 受影响的配置文件列表
    pub affected_files: Vec<String>,
    /// Git 提交哈希(如果启用自动提交)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_sha: Option<String>,
}

// ============================================================================
// 导入操作数据结构
// ============================================================================

/// POST /api/v1/env/import 请求体
#[derive(Debug, Deserialize)]
pub struct EnvImportRequest {
    /// Base64 编码的 .env 文件内容
    pub content: String,
    /// 导入目标作用域: "global", "service:api", "task:build"
    pub scope: String,
    /// 冲突解决策略
    #[serde(default)]
    pub conflict_strategy: ConflictStrategy,
}

/// 冲突解决策略
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ConflictStrategy {
    /// 跳过已存在的变量(默认)
    #[default]
    Skip,
    /// 覆盖已存在的变量
    Overwrite,
    /// 遇到冲突即中止
    Abort,
}

/// 导入操作结果
#[derive(Debug, Serialize)]
pub struct EnvImportResult {
    /// 成功导入的变量数
    pub imported_count: usize,
    /// 跳过的变量数
    pub skipped_count: usize,
    /// 失败的变量数
    pub failed_count: usize,
    /// 详细结果列表
    pub details: Vec<EnvImportItemResult>,
    /// Git 提交哈希(如果启用自动提交)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_sha: Option<String>,
}

/// 导入项结果
#[derive(Debug, Serialize)]
pub struct EnvImportItemResult {
    /// 变量名
    pub key: String,
    /// 导入状态
    pub status: EnvImportStatus,
}

/// 导入状态
#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EnvImportStatus {
    /// 成功导入
    Imported,
    /// 跳过(已存在)
    Skipped,
    /// 导入失败
    Failed,
}

// ============================================================================
// 导出操作数据结构
// ============================================================================

/// GET /api/v1/env/export 查询参数
#[derive(Debug, Deserialize)]
pub struct EnvExportParams {
    /// 过滤作用域列表
    #[serde(default)]
    pub scopes: Vec<String>,
    /// 是否包含注释(默认true)
    #[serde(default = "default_true")]
    pub include_comments: bool,
    /// 是否展开变量引用(默认false)
    #[serde(default)]
    pub expand: bool,
}

fn default_true() -> bool {
    true
}

// ============================================================================
// 响应数据结构
// ============================================================================

/// 环境变量条目(用于列表)
#[derive(Debug, Clone, Serialize)]
pub struct EnvVar {
    /// 变量名
    pub key: String,
    /// 变量值(原始值)
    pub value: String,
    /// 所属作用域
    pub scope: EnvScope,
    /// 来源配置文件路径
    pub source_file: String,
    /// 是否包含变量引用(如 ${OTHER_VAR})
    pub has_references: bool,
    /// 展开后的值(仅当 expand=true 时存在)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expanded_value: Option<String>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 最后更新时间
    pub updated_at: DateTime<Utc>,
}

/// 环境变量详情(用于单个查询)
#[derive(Debug, Serialize)]
pub struct EnvVarDetail {
    /// 变量名
    pub key: String,
    /// 生效值(根据优先级计算的最终值)
    pub effective_value: String,
    /// 生效作用域
    pub effective_scope: EnvScope,
    /// 所有作用域的定义(按优先级排序: Task > Service > Global)
    pub definitions: Vec<ScopeDefinition>,
    /// 是否包含变量引用
    pub has_references: bool,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 最后更新时间
    pub updated_at: DateTime<Utc>,
}

/// 作用域定义(用于显示变量在不同作用域的覆盖情况)
#[derive(Debug, Serialize)]
pub struct ScopeDefinition {
    /// 作用域
    pub scope: EnvScope,
    /// 该作用域的值
    pub value: String,
    /// 来源配置文件
    pub source_file: String,
    /// 优先级(Task=3, Service=2, Global=1)
    pub priority: u8,
}

/// 列表响应(带分页)
#[derive(Debug, Serialize)]
pub struct ListResponse<T> {
    /// 数据列表
    pub data: Vec<T>,
    /// 分页信息
    pub pagination: Pagination,
}

/// 分页信息
#[derive(Debug, Serialize)]
pub struct Pagination {
    /// 当前页码
    pub page: u32,
    /// 每页条数
    pub per_page: u32,
    /// 总条数
    pub total: u64,
    /// 总页数
    pub total_pages: u64,
}

impl Pagination {
    pub fn new(page: u32, per_page: u32, total: u64) -> Self {
        let total_pages = total.div_ceil(per_page as u64);
        Self {
            page,
            per_page,
            total,
            total_pages,
        }
    }
}

// ============================================================================
// 辅助函数
// ============================================================================

/// 解析作用域字符串
///
/// 支持格式:
/// - "global"
/// - "service:api"
/// - "task:build"
pub fn parse_scope(scope_str: &str) -> Result<EnvScope, String> {
    if scope_str == "global" {
        return Ok(EnvScope::Global);
    }

    if let Some(service_name) = scope_str.strip_prefix("service:") {
        if service_name.is_empty() {
            return Err("Service name cannot be empty".to_string());
        }
        return Ok(EnvScope::Service {
            name: service_name.to_string(),
        });
    }

    if let Some(task_name) = scope_str.strip_prefix("task:") {
        if task_name.is_empty() {
            return Err("Task name cannot be empty".to_string());
        }
        return Ok(EnvScope::Task {
            name: task_name.to_string(),
        });
    }

    Err(format!(
        "Invalid scope format: {}. Expected 'global', 'service:name', or 'task:name'",
        scope_str
    ))
}

/// 获取作用域优先级
pub fn scope_priority(scope: &EnvScope) -> u8 {
    match scope {
        EnvScope::Task { .. } => 3,
        EnvScope::Service { .. } => 2,
        EnvScope::Global => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_scope_global() {
        let scope = parse_scope("global").unwrap();
        assert_eq!(scope, EnvScope::Global);
    }

    #[test]
    fn test_parse_scope_service() {
        let scope = parse_scope("service:api").unwrap();
        assert_eq!(
            scope,
            EnvScope::Service {
                name: "api".to_string()
            }
        );
    }

    #[test]
    fn test_parse_scope_task() {
        let scope = parse_scope("task:build").unwrap();
        assert_eq!(
            scope,
            EnvScope::Task {
                name: "build".to_string()
            }
        );
    }

    #[test]
    fn test_parse_scope_invalid() {
        assert!(parse_scope("invalid").is_err());
        assert!(parse_scope("service:").is_err());
        assert!(parse_scope("task:").is_err());
    }

    #[test]
    fn test_scope_priority() {
        assert_eq!(scope_priority(&EnvScope::Global), 1);
        assert_eq!(
            scope_priority(&EnvScope::Service {
                name: "api".to_string()
            }),
            2
        );
        assert_eq!(
            scope_priority(&EnvScope::Task {
                name: "build".to_string()
            }),
            3
        );
    }

    #[test]
    fn test_pagination_calculation() {
        let p1 = Pagination::new(1, 20, 100);
        assert_eq!(p1.total_pages, 5);

        let p2 = Pagination::new(1, 20, 99);
        assert_eq!(p2.total_pages, 5);

        let p3 = Pagination::new(1, 20, 101);
        assert_eq!(p3.total_pages, 6);
    }
}
