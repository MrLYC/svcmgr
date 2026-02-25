// 配置管理 API 数据模型
//
// 本文件定义配置管理相关的所有数据结构，包括：
// - 配置表示（Config, ConfigSection）
// - 功能开关（Features, FeatureMode）
// - HTTP 配置（HttpConfig, HttpRoute）
// - 配置历史（ConfigHistory, ConfigDiff）
// - 验证结果（ValidationResult, ValidationError）
//
// 参考: OpenSpec 14-api-config.md

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

// ============================================================================
// 配置数据结构
// ============================================================================

/// 完整配置（mise.toml + svcmgr.toml 合并视图）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// mise 工具版本（[tools] 段）
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub tools: HashMap<String, String>,

    /// 环境变量（[env] 段）
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub env: HashMap<String, String>,

    /// mise 任务定义（[tasks] 段）
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub tasks: HashMap<String, JsonValue>,

    /// systemd 服务定义（[services] 段）
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub services: HashMap<String, JsonValue>,

    /// cron 定时任务（[scheduled_tasks] 段）
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub scheduled_tasks: HashMap<String, JsonValue>,

    /// 功能开关（[features] 段）
    #[serde(default)]
    pub features: Features,

    /// HTTP 配置（[http] 段）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http: Option<HttpConfig>,
}

/// 配置段落枚举（用于路径参数）
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ConfigSection {
    Tools,
    Env,
    Tasks,
    Services,
    ScheduledTasks,
    Features,
    Http,
}

impl ConfigSection {
    /// 将段落枚举转换为 TOML 段落名称
    pub fn to_toml_key(&self) -> &'static str {
        match self {
            Self::Tools => "tools",
            Self::Env => "env",
            Self::Tasks => "tasks",
            Self::Services => "services",
            Self::ScheduledTasks => "scheduled_tasks",
            Self::Features => "features",
            Self::Http => "http",
        }
    }

    /// 从字符串解析段落名称
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "tools" => Some(Self::Tools),
            "env" => Some(Self::Env),
            "tasks" => Some(Self::Tasks),
            "services" => Some(Self::Services),
            "scheduled_tasks" | "scheduledtasks" => Some(Self::ScheduledTasks),
            "features" => Some(Self::Features),
            "http" => Some(Self::Http),
            _ => None,
        }
    }
}

// ============================================================================
// 功能开关
// ============================================================================

/// svcmgr 功能开关配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Features {
    /// systemd 集成
    #[serde(default)]
    pub systemd: FeatureMode,

    /// cgroups 资源限制
    #[serde(default)]
    pub cgroups: FeatureMode,

    /// 内置 HTTP 代理
    #[serde(default)]
    pub http_proxy: FeatureMode,

    /// Git 自动提交
    #[serde(default)]
    pub git_auto_commit: FeatureMode,
}

impl Default for Features {
    fn default() -> Self {
        Self {
            systemd: FeatureMode::Auto,
            cgroups: FeatureMode::Auto,
            http_proxy: FeatureMode::Auto,
            git_auto_commit: FeatureMode::Enabled, // 默认启用
        }
    }
}

/// 功能开关模式
#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FeatureMode {
    /// 自动检测（默认）
    #[default]
    Auto,

    /// 强制启用
    Enabled,

    /// 完全禁用
    Disabled,
}

// ============================================================================
// HTTP 配置
// ============================================================================

/// HTTP 配置（[[http.routes]]）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    /// 监听地址
    #[serde(default = "default_listen_addr")]
    pub listen: String,

    /// 路由规则
    #[serde(default)]
    pub routes: Vec<HttpRoute>,
}

fn default_listen_addr() -> String {
    "127.0.0.1:3080".to_string()
}

/// HTTP 路由规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpRoute {
    /// 路由路径（如 /api/*）
    pub path: String,

    /// 目标服务名
    pub target: String,

    /// 目标端口名（services.*.ports 中的 key）
    pub port: String,

    /// 路径重写规则（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rewrite: Option<String>,
}

// ============================================================================
// 配置历史
// ============================================================================

/// 配置变更历史项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigHistory {
    /// Git commit hash
    pub commit: String,

    /// 提交信息
    pub message: String,

    /// 提交时间
    pub timestamp: DateTime<Utc>,

    /// 提交者
    pub author: String,

    /// 变更文件列表
    pub files: Vec<String>,
}

// ============================================================================
// 配置差异
// ============================================================================

/// 配置差异
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigDiff {
    /// 起始版本（commit hash）
    pub from: String,

    /// 目标版本（commit hash）
    pub to: String,

    /// 差异内容（unified diff 格式）
    pub diff: String,

    /// 变更统计
    pub stats: DiffStats,
}

/// 差异统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffStats {
    /// 变更文件数
    pub files_changed: usize,

    /// 新增行数
    pub insertions: usize,

    /// 删除行数
    pub deletions: usize,
}

// ============================================================================
// 验证结果
// ============================================================================

/// 配置验证结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// 是否有效
    pub valid: bool,

    /// 错误列表（语法错误 + 语义错误）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<ValidationError>,

    /// 警告列表
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<ValidationWarning>,
}

/// 验证错误
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    /// 错误类型
    pub kind: ValidationErrorKind,

    /// 错误位置（段落.键名）
    pub path: String,

    /// 错误信息
    pub message: String,
}

/// 验证错误类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ValidationErrorKind {
    /// 语法错误（TOML 解析失败）
    Syntax,

    /// 类型错误（字段类型不匹配）
    Type,

    /// 缺失必填字段
    MissingField,

    /// 依赖缺失（服务依赖的工具不存在）
    MissingDependency,

    /// 循环依赖
    CircularDependency,

    /// 端口冲突
    PortConflict,

    /// 路径无效
    InvalidPath,

    /// 其他错误
    Other,
}

/// 验证警告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationWarning {
    /// 警告位置（段落.键名）
    pub path: String,

    /// 警告信息
    pub message: String,
}

// ============================================================================
// 请求体 / 响应体
// ============================================================================

/// GET /api/v1/config 响应
pub type GetConfigResponse = Config;

/// GET /api/v1/config/{section} 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetConfigSectionResponse {
    pub section: String,
    pub data: JsonValue,
}

/// PUT /api/v1/config 请求
pub type UpdateConfigRequest = Config;

/// PATCH /api/v1/config/{section} 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchConfigSectionRequest {
    /// 操作类型
    pub op: PatchOperation,

    /// 数据
    pub data: JsonValue,
}

/// PATCH 操作类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PatchOperation {
    /// 合并（深度合并，保留未提及的键）
    Merge,

    /// 覆盖（完全替换整个段落）
    Replace,

    /// 删除（删除指定键）
    Remove,
}

/// POST /api/v1/config/validate 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateConfigRequest {
    pub config: Config,
}

/// POST /api/v1/config/validate 响应
pub type ValidateConfigResponse = ValidationResult;

/// GET /api/v1/config/history 查询参数
#[derive(Debug, Deserialize)]
pub struct ConfigHistoryQuery {
    #[serde(default = "default_limit")]
    pub limit: usize,

    #[serde(default)]
    pub offset: usize,

    /// 只显示特定文件的历史
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}

fn default_limit() -> usize {
    50
}

/// GET /api/v1/config/history 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigHistoryResponse {
    pub history: Vec<ConfigHistory>,
    pub total: usize,
}

/// POST /api/v1/config/rollback 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackConfigRequest {
    /// 目标 commit hash
    pub commit: String,
}

/// GET /api/v1/config/diff 查询参数
#[derive(Debug, Deserialize)]
pub struct ConfigDiffQuery {
    /// 起始版本（默认：当前工作树）
    pub from: Option<String>,

    /// 目标版本（默认：HEAD）
    pub to: Option<String>,
}

/// GET /api/v1/config/diff 响应
pub type ConfigDiffResponse = ConfigDiff;

/// GET /api/v1/config/export 查询参数
#[derive(Debug, Deserialize)]
pub struct ConfigExportQuery {
    /// 导出格式
    #[serde(default = "default_export_format")]
    pub format: ExportFormat,

    /// 是否包含注释
    #[serde(default)]
    pub include_comments: bool,
}

fn default_export_format() -> ExportFormat {
    ExportFormat::Toml
}

/// 导出格式
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Json,
    Toml,
}

/// POST /api/v1/config/import 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportConfigRequest {
    /// 配置数据（JSON 或 TOML 字符串）
    pub config: String,

    /// 数据格式
    pub format: ExportFormat,

    /// 是否覆盖现有配置（false 表示合并）
    #[serde(default)]
    pub overwrite: bool,
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_section_to_toml_key() {
        assert_eq!(ConfigSection::Tools.to_toml_key(), "tools");
        assert_eq!(ConfigSection::Env.to_toml_key(), "env");
        assert_eq!(
            ConfigSection::ScheduledTasks.to_toml_key(),
            "scheduled_tasks"
        );
    }

    #[test]
    fn test_config_section_from_str() {
        assert_eq!(ConfigSection::from_str("tools"), Some(ConfigSection::Tools));
        assert_eq!(ConfigSection::from_str("TOOLS"), Some(ConfigSection::Tools));
        assert_eq!(
            ConfigSection::from_str("scheduled_tasks"),
            Some(ConfigSection::ScheduledTasks)
        );
        assert_eq!(
            ConfigSection::from_str("scheduledtasks"),
            Some(ConfigSection::ScheduledTasks)
        );
        assert_eq!(ConfigSection::from_str("invalid"), None);
    }

    #[test]
    fn test_feature_mode_default() {
        assert_eq!(FeatureMode::default(), FeatureMode::Auto);
    }

    #[test]
    fn test_features_default() {
        let features = Features::default();
        assert_eq!(features.systemd, FeatureMode::Auto);
        assert_eq!(features.git_auto_commit, FeatureMode::Enabled);
    }

    #[test]
    fn test_http_config_default_listen() {
        assert_eq!(default_listen_addr(), "127.0.0.1:3080");
    }

    #[test]
    fn test_patch_operation_variants() {
        let merge = PatchOperation::Merge;
        let replace = PatchOperation::Replace;
        let remove = PatchOperation::Remove;

        assert_eq!(merge, PatchOperation::Merge);
        assert_eq!(replace, PatchOperation::Replace);
        assert_eq!(remove, PatchOperation::Remove);
    }

    #[test]
    fn test_validation_error_kind_variants() {
        let syntax = ValidationErrorKind::Syntax;
        let missing = ValidationErrorKind::MissingField;
        let conflict = ValidationErrorKind::PortConflict;

        assert_eq!(syntax, ValidationErrorKind::Syntax);
        assert_eq!(missing, ValidationErrorKind::MissingField);
        assert_eq!(conflict, ValidationErrorKind::PortConflict);
    }

    #[test]
    fn test_export_format_default() {
        assert_eq!(default_export_format(), ExportFormat::Toml);
    }

    #[test]
    fn test_config_serialization() {
        let config = Config {
            tools: HashMap::from([("node".to_string(), "20.0.0".to_string())]),
            env: HashMap::from([("PATH".to_string(), "/usr/bin".to_string())]),
            tasks: HashMap::new(),
            services: HashMap::new(),
            scheduled_tasks: HashMap::new(),
            features: Features::default(),
            http: None,
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("node"));
        assert!(json.contains("20.0.0"));
    }

    #[test]
    fn test_validation_result_valid() {
        let result = ValidationResult {
            valid: true,
            errors: vec![],
            warnings: vec![],
        };

        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_config_diff_stats() {
        let stats = DiffStats {
            files_changed: 2,
            insertions: 10,
            deletions: 5,
        };

        assert_eq!(stats.files_changed, 2);
        assert_eq!(stats.insertions, 10);
        assert_eq!(stats.deletions, 5);
    }
}
