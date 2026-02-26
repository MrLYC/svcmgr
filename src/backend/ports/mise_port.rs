//! mise Port-Adapter 接口定义
//!
//! 根据 OpenSpec 07-mise-integration.md 定义的 Port 层接口。
//! Port 层是纯 Rust trait 定义,不包含任何实现细节。

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ============================================================================
// 数据结构定义
// ============================================================================

/// mise 版本信息 (CalVer: year.minor.patch)
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MiseVersion {
    pub year: u16,
    pub minor: u16,
    pub patch: u16,
}

impl MiseVersion {
    pub fn new(year: u16, minor: u16, patch: u16) -> Self {
        Self { year, minor, patch }
    }

    /// 从字符串解析版本号 (格式: "2026.2.17", "mise 2026.2.17", "2026.2.19 linux-x64 (2026-02-22)")
    pub fn parse(s: &str) -> Result<Self> {
        // Find the first token that looks like a version (contains dots)
        let version_part = s
            .split_whitespace()
            .find(|token| token.contains('.'))
            .ok_or_else(|| anyhow::anyhow!("No version number found in: {}", s))?;

        let parts: Vec<&str> = version_part.split('.').collect();
        if parts.len() != 3 {
            anyhow::bail!("Invalid version format: expected X.Y.Z, got {}", s);
        }

        Ok(Self {
            year: parts[0]
                .parse()
                .with_context(|| format!("Invalid year: {}", parts[0]))?,
            minor: parts[1]
                .parse()
                .with_context(|| format!("Invalid minor: {}", parts[1]))?,
            patch: parts[2]
                .parse()
                .with_context(|| format!("Invalid patch: {}", parts[2]))?,
        })
    }

    /// 检查是否支持特定特性
    pub fn supports(&self, feature: MiseFeature) -> bool {
        match feature {
            MiseFeature::ConfD => self >= &Self::new(2024, 12, 0),
            MiseFeature::TaskDepends => self >= &Self::new(2024, 1, 0),
            MiseFeature::Lockfiles => self >= &Self::new(2026, 2, 0),
            MiseFeature::McpRunTask => self >= &Self::new(2026, 2, 16),
        }
    }
}

impl std::fmt::Display for MiseVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.year, self.minor, self.patch)
    }
}

/// mise 特性枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MiseFeature {
    /// conf.d 目录支持
    ConfD,
    /// 任务依赖
    TaskDepends,
    /// 锁文件
    Lockfiles,
    /// MCP run_task 工具
    McpRunTask,
}

/// 工具信息
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolInfo {
    /// 工具名称 (e.g., "node", "python")
    pub name: String,
    /// 工具版本 (e.g., "20.11.0")
    pub version: String,
    /// 工具来源 (e.g., "asdf", "core")
    pub source: String,
}

/// 任务命令定义
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskCommand {
    /// 执行的命令
    pub command: String,
    /// 任务特定环境变量
    pub env: HashMap<String, String>,
    /// 工作目录
    pub workdir: Option<PathBuf>,
}

/// 任务执行输出
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskOutput {
    /// 退出码
    pub exit_code: i32,
    /// 标准输出
    pub stdout: String,
    /// 标准错误
    pub stderr: String,
}

/// 任务信息
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskInfo {
    /// 任务名称
    pub name: String,
    /// 任务描述
    pub description: Option<String>,
    /// 任务命令
    pub command: String,
    /// 依赖的其他任务
    pub depends: Vec<String>,
}

// ============================================================================
// Port Trait 定义
// ============================================================================

/// 依赖管理端口 (Tool management)
#[async_trait]
pub trait DependencyPort: Send + Sync {
    /// 安装指定工具和版本
    async fn install(&self, tool: &str, version: &str) -> Result<()>;

    /// 列出已安装的工具
    async fn list_installed(&self) -> Result<Vec<ToolInfo>>;

    /// 设置当前目录使用的工具版本
    async fn use_tool(&self, tool: &str, version: &str) -> Result<()>;

    /// 移除工具
    async fn remove(&self, tool: &str, version: &str) -> Result<()>;

    /// 获取 mise 版本信息
    fn mise_version(&self) -> &MiseVersion;
}

/// 任务管理端口 (Task execution)
#[async_trait]
pub trait TaskPort: Send + Sync {
    /// 运行指定任务 (一次性前台执行)
    async fn run_task(&self, name: &str, args: &[String]) -> Result<TaskOutput>;

    /// 获取任务定义 (从 mise 配置中读取 run 命令)
    async fn get_task_command(&self, name: &str) -> Result<TaskCommand>;

    /// 列出所有任务
    async fn list_tasks(&self) -> Result<Vec<TaskInfo>>;
}

/// 环境变量端口 (Environment variables)
#[async_trait]
pub trait EnvPort: Send + Sync {
    /// 获取 mise 解析后的完整环境变量
    async fn get_env(&self) -> Result<HashMap<String, String>>;

    /// 获取指定目录下的环境变量
    async fn get_env_for_dir(&self, dir: &Path) -> Result<HashMap<String, String>>;
}

/// 配置文件端口 (Configuration files)
#[async_trait]
pub trait ConfigPort: TaskPort + Send + Sync {
    /// 获取 mise 当前加载的配置文件列表 (按优先级排序)
    async fn list_config_files(&self) -> Result<Vec<PathBuf>>;

    /// 读取指定配置文件的原始 TOML
    async fn read_config(&self, path: &Path) -> Result<toml::Value>;

    /// 写入配置文件 (仅写 mise 原生段)
    async fn write_config(&self, path: &Path, value: &toml::Value) -> Result<()>;

    async fn get_global_env_var(&self, key: &str) -> Result<Option<String>>;

    async fn get_service_env_var(&self, service_name: &str, key: &str) -> Result<Option<String>>;

    async fn get_task_env_var(&self, task_name: &str, key: &str) -> Result<Option<String>>;

    async fn get_global_env(&self) -> Result<HashMap<String, String>>;

    async fn get_service_envs(&self) -> Result<HashMap<String, HashMap<String, String>>>;

    async fn get_task_envs(&self) -> Result<HashMap<String, HashMap<String, String>>>;

    async fn set_env_var(&self, key: &str, value: &str, scope: &crate::env::EnvScope)
        -> Result<()>;

    async fn delete_env_var(&self, key: &str, scope: &crate::env::EnvScope) -> Result<()>;

    // ========================================================================
    // Task Management - 任务管理
    // ========================================================================

    /// 取消正在运行的任务
    async fn cancel_task(&self, execution_id: &str) -> Result<()>;

    /// 获取任务执行历史
    async fn get_task_history(
        &self,
        task_name: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<crate::web::api::task_models::TaskExecutionRecord>>;

    // ========================================================================
    // Scheduled Tasks - 定时任务管理
    // ========================================================================

    /// 列出所有定时任务
    async fn list_scheduled_tasks(
        &self,
    ) -> Result<Vec<crate::web::api::task_models::ScheduledTask>>;

    /// 获取指定定时任务
    async fn get_scheduled_task(
        &self,
        name: &str,
    ) -> Result<Option<crate::web::api::task_models::ScheduledTask>>;

    /// 检查定时任务是否存在
    async fn scheduled_task_exists(&self, name: &str) -> Result<bool>;

    /// 创建定时任务
    async fn create_scheduled_task(
        &self,
        task: &crate::web::api::task_models::ScheduledTask,
    ) -> Result<()>;

    /// 更新定时任务
    async fn update_scheduled_task(
        &self,
        name: &str,
        task: &crate::web::api::task_models::ScheduledTask,
    ) -> Result<()>;

    /// 删除定时任务
    async fn delete_scheduled_task(&self, name: &str) -> Result<()>;

    // ========================================================================
    // Service Management - 服务管理
    // ========================================================================

    /// 获取服务定义
    async fn get_service(
        &self,
        name: &str,
    ) -> Result<crate::web::api::service_models::ServiceDefinition>;

    /// 列出所有服务
    async fn list_services(
        &self,
    ) -> Result<Vec<crate::web::api::service_models::ServiceDefinition>>;

    /// 创建服务
    async fn create_service(
        &self,
        service: &crate::web::api::service_models::ServiceDefinition,
    ) -> Result<()>;

    /// 更新服务（完全替换）
    async fn update_service(
        &self,
        name: &str,
        service: &crate::web::api::service_models::ServiceDefinition,
    ) -> Result<()>;

    /// 部分更新服务
    async fn patch_service(&self, name: &str, updates: &serde_json::Value) -> Result<()>;

    /// 删除服务
    async fn delete_service(&self, name: &str) -> Result<()>;
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mise_version_parse() {
        let v1 = MiseVersion::parse("2026.2.17").unwrap();
        assert_eq!(v1.year, 2026);
        assert_eq!(v1.minor, 2);
        assert_eq!(v1.patch, 17);

        let v2 = MiseVersion::parse("mise 2025.12.5").unwrap();
        assert_eq!(v2.year, 2025);
        assert_eq!(v2.minor, 12);
        assert_eq!(v2.patch, 5);
    }

    #[test]
    fn test_mise_version_parse_with_platform() {
        // Test new mise 2026.2.19+ format with platform and build date
        let v = MiseVersion::parse("2026.2.19 linux-x64 (2026-02-22)").unwrap();
        assert_eq!(v.year, 2026);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 19);
    }

    #[test]
    fn test_mise_version_invalid() {
        assert!(MiseVersion::parse("invalid").is_err());
        assert!(MiseVersion::parse("2026.2").is_err());
        assert!(MiseVersion::parse("").is_err());
    }

    #[test]
    fn test_mise_version_compare() {
        let v1 = MiseVersion::new(2026, 2, 17);
        let v2 = MiseVersion::new(2025, 12, 5);
        let v3 = MiseVersion::new(2026, 2, 17);

        assert!(v1 > v2);
        assert!(v2 < v1);
        assert_eq!(v1, v3);
    }

    #[test]
    fn test_mise_version_supports() {
        let v_old = MiseVersion::new(2024, 1, 0);
        let v_new = MiseVersion::new(2026, 2, 17);

        // TaskDepends support (>= 2024.1.0)
        assert!(v_old.supports(MiseFeature::TaskDepends));
        assert!(v_new.supports(MiseFeature::TaskDepends));

        // Lockfiles support (>= 2026.2.0)
        assert!(!v_old.supports(MiseFeature::Lockfiles));
        assert!(v_new.supports(MiseFeature::Lockfiles));

        // McpRunTask support (>= 2026.2.16)
        assert!(!v_old.supports(MiseFeature::McpRunTask));
        assert!(v_new.supports(MiseFeature::McpRunTask));
    }

    #[test]
    fn test_mise_version_display() {
        let v = MiseVersion::new(2026, 2, 17);
        assert_eq!(v.to_string(), "2026.2.17");
    }
}
