// 服务管理 API 数据模型
//
// 根据 OpenSpec 11-api-services.md 定义的完整数据模型

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// 服务定义 (Service Definition)
// ============================================================================

/// 服务定义（创建/更新请求使用）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDefinition {
    /// 服务名称（唯一标识）
    pub name: String,

    /// 执行命令（可以是 mise task 引用或直接命令）
    pub command: String,

    /// 工作目录（默认为当前项目目录）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>,

    /// 环境变量（会与 mise 的 [env] 合并）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,

    /// 暴露的端口配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ports: Option<Vec<PortMapping>>,

    /// 健康检查配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health_check: Option<HealthCheckConfig>,

    /// 资源限制（可选，cgroups v2 支持）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourceLimits>,

    /// 重启策略
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart_policy: Option<RestartPolicy>,

    /// 自动启动（进程管理器启动时自动启动此服务）
    #[serde(default)]
    pub autostart: bool,

    /// 依赖的其他服务（启动前确保依赖服务已运行）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depends_on: Option<Vec<String>>,
}

/// 端口映射配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortMapping {
    /// 主机端口（监听端口）
    pub host: u16,

    /// 容器/服务端口（服务实际监听端口）
    pub container: u16,

    /// 协议（tcp/udp）
    #[serde(default = "default_protocol")]
    pub protocol: String,
}

fn default_protocol() -> String {
    "tcp".to_string()
}

/// 健康检查配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum HealthCheckConfig {
    /// HTTP 健康检查
    Http {
        /// 健康检查 URL
        url: String,
        /// 期望的 HTTP 状态码
        #[serde(default = "default_http_status")]
        expected_status: u16,
        /// 超时时间（秒）
        #[serde(default = "default_timeout")]
        timeout: u64,
        /// 检查间隔（秒）
        #[serde(default = "default_interval")]
        interval: u64,
    },

    /// TCP 端口检查
    Tcp {
        /// 主机地址
        #[serde(default = "default_host")]
        host: String,
        /// 端口号
        port: u16,
        /// 超时时间（秒）
        #[serde(default = "default_timeout")]
        timeout: u64,
        /// 检查间隔（秒）
        #[serde(default = "default_interval")]
        interval: u64,
    },

    /// 命令执行检查
    Command {
        /// 执行的命令
        command: String,
        /// 超时时间（秒）
        #[serde(default = "default_timeout")]
        timeout: u64,
        /// 检查间隔（秒）
        #[serde(default = "default_interval")]
        interval: u64,
    },
}

fn default_http_status() -> u16 {
    200
}
fn default_timeout() -> u64 {
    5
}
fn default_interval() -> u64 {
    10
}
fn default_host() -> String {
    "127.0.0.1".to_string()
}

/// 资源限制配置（cgroups v2）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// CPU 限制（核心数，如 1.5）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu: Option<f64>,

    /// 内存限制（字节数）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<u64>,

    /// 内存限制（人类可读格式，如 "512M", "2G"）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_str: Option<String>,
}

/// 重启策略
#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum RestartPolicy {
    /// 永不重启
    No,

    /// 失败时重启（退出码非 0）
    #[default]
    OnFailure,

    /// 总是重启（无论退出码）
    Always,
}

// ============================================================================
// 服务状态 (Service Status)
// ============================================================================

/// 服务完整状态（包含定义 + 运行时状态）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatus {
    /// 服务定义
    #[serde(flatten)]
    pub definition: ServiceDefinition,

    /// 运行时状态
    pub runtime: ServiceRuntime,
}

/// 服务运行时状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceRuntime {
    /// 状态（stopped, starting, running, stopping, failed）
    pub state: ServiceState,

    /// 进程 PID（仅 running 状态有值）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,

    /// 运行时长（秒，仅 running 状态有值）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uptime: Option<u64>,

    /// 启动时间（Unix timestamp）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<i64>,

    /// 停止时间（Unix timestamp，仅 stopped/failed 状态有值）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stopped_at: Option<i64>,

    /// 退出码（仅 stopped/failed 状态有值）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,

    /// 重启次数
    pub restart_count: u32,

    /// 健康状态（仅配置了健康检查且服务 running 时有值）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health: Option<HealthStatus>,

    /// 资源使用情况（仅 running 状态有值）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourceUsage>,

    /// 错误消息（仅 failed 状态有值）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 服务定义与运行时状态的组合(用于 API 响应)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceWithRuntime {
    /// 服务名称(唯一标识符)
    pub name: String,

    /// 启动命令
    pub command: String,

    /// 工作目录
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>,

    /// 环境变量
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<std::collections::HashMap<String, String>>,

    /// 端口映射
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ports: Option<Vec<PortMapping>>,

    /// 健康检查配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health_check: Option<HealthCheckConfig>,

    /// 资源限制
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourceLimits>,

    /// 重启策略
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart_policy: Option<RestartPolicy>,

    /// 是否自动启动
    pub autostart: bool,

    /// 依赖的其他服务
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depends_on: Option<Vec<String>>,

    /// 运行时状态
    pub runtime: ServiceRuntime,
}

/// 服务状态枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ServiceState {
    /// 已停止
    Stopped,

    /// 正在启动
    Starting,

    /// 正在运行
    Running,

    /// 正在停止
    Stopping,

    /// 失败（进程异常退出或启动失败）
    Failed,
}

/// 健康状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    /// 健康状态（healthy, unhealthy, unknown）
    pub status: HealthState,

    /// 最后检查时间（Unix timestamp）
    pub last_check: i64,

    /// 连续成功次数
    pub consecutive_successes: u32,

    /// 连续失败次数
    pub consecutive_failures: u32,

    /// 检查消息（失败时的错误信息）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HealthState {
    Healthy,
    Unhealthy,
    Unknown,
}

/// 资源使用情况
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// CPU 使用率（百分比，0.0-100.0）
    pub cpu_percent: f64,

    /// 内存使用量（字节）
    pub memory_bytes: u64,

    /// 内存使用量（人类可读）
    pub memory_str: String,
}

// ============================================================================
// 输入验证 (Input Validation)
// ============================================================================

impl ServiceDefinition {
    /// 验证服务定义的所有字段
    pub fn validate(&self) -> Result<(), String> {
        // 1. 验证 name（必须符合正则表达式）
        if !is_valid_name(&self.name) {
            return Err(format!(
                "Invalid service name '{}': must match ^[a-zA-Z0-9_-]{{1,64}}$",
                self.name
            ));
        }

        // 2. 验证 command 非空
        if self.command.trim().is_empty() {
            return Err("Service command cannot be empty".to_string());
        }

        // 3. 验证端口范围
        if let Some(ref ports) = self.ports {
            for port in ports {
                if !is_valid_port(port.host) {
                    return Err(format!("Invalid host port {}: must be 1-65535", port.host));
                }
                if !is_valid_port(port.container) {
                    return Err(format!(
                        "Invalid container port {}: must be 1-65535",
                        port.container
                    ));
                }
                if port.protocol != "tcp" && port.protocol != "udp" {
                    return Err(format!(
                        "Invalid protocol '{}': must be tcp or udp",
                        port.protocol
                    ));
                }
            }
        }

        // 4. 验证健康检查配置
        if let Some(ref health) = self.health_check {
            match health {
                HealthCheckConfig::Http {
                    url,
                    timeout,
                    interval,
                    ..
                } => {
                    if url.is_empty() {
                        return Err("Health check URL cannot be empty".to_string());
                    }
                    validate_timeout(*timeout)?;
                    validate_interval(*interval)?;
                }
                HealthCheckConfig::Tcp {
                    port,
                    timeout,
                    interval,
                    ..
                } => {
                    if !is_valid_port(*port) {
                        return Err(format!(
                            "Invalid health check port {}: must be 1-65535",
                            port
                        ));
                    }
                    validate_timeout(*timeout)?;
                    validate_interval(*interval)?;
                }
                HealthCheckConfig::Command {
                    command,
                    timeout,
                    interval,
                } => {
                    if command.trim().is_empty() {
                        return Err("Health check command cannot be empty".to_string());
                    }
                    validate_timeout(*timeout)?;
                    validate_interval(*interval)?;
                }
            }
        }

        Ok(())
    }
}

/// 验证服务名称格式
fn is_valid_name(name: &str) -> bool {
    let re = regex::Regex::new(r"^[a-zA-Z0-9_-]{1,64}$").unwrap();
    re.is_match(name)
}

/// 验证端口范围
fn is_valid_port(port: u16) -> bool {
    port > 0
}

/// 验证超时时间
fn validate_timeout(timeout: u64) -> Result<(), String> {
    if !(1..=300).contains(&timeout) {
        return Err(format!(
            "Invalid timeout {}: must be 1-300 seconds",
            timeout
        ));
    }
    Ok(())
}

/// 验证检查间隔
fn validate_interval(interval: u64) -> Result<(), String> {
    if !(5..=3600).contains(&interval) {
        return Err(format!(
            "Invalid interval {}: must be 5-3600 seconds",
            interval
        ));
    }
    Ok(())
}

// ============================================================================
// 请求/响应模型 (Request/Response Models)
// ============================================================================

/// 服务列表查询参数
#[derive(Debug, Deserialize)]
pub struct ListServicesQuery {
    /// 页码（默认 1）
    #[serde(default = "default_page")]
    pub page: u32,

    /// 每页数量（默认 20）
    #[serde(default = "default_per_page")]
    pub per_page: u32,

    /// 按状态过滤（逗号分隔多个状态）
    #[serde(default)]
    pub status: Option<String>,

    /// 仅显示自动启动的服务
    #[serde(default)]
    pub autostart: Option<bool>,

    /// 排序字段（name, uptime, restart_count）
    #[serde(default)]
    pub sort: Option<String>,
}

fn default_page() -> u32 {
    1
}
fn default_per_page() -> u32 {
    20
}

/// 服务创建请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateServiceRequest {
    /// 服务名称
    pub name: String,

    /// 执行命令
    pub command: String,

    /// 工作目录
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>,

    /// 环境变量
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,

    /// 端口映射
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ports: Option<Vec<PortMapping>>,

    /// 健康检查配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health_check: Option<HealthCheckConfig>,

    /// 资源限制
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourceLimits>,

    /// 重启策略
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart_policy: Option<RestartPolicy>,

    /// 是否自动启动
    #[serde(default)]
    pub autostart: bool,

    /// 依赖的其他服务
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depends_on: Option<Vec<String>>,
}

/// 服务更新请求（PUT - 完全替换）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateServiceRequest {
    /// 执行命令
    pub command: String,

    /// 工作目录
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>,

    /// 环境变量
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,

    /// 端口映射
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ports: Option<Vec<PortMapping>>,

    /// 健康检查配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health_check: Option<HealthCheckConfig>,

    /// 资源限制
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourceLimits>,

    /// 重启策略
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart_policy: Option<RestartPolicy>,

    /// 是否自动启动
    #[serde(default)]
    pub autostart: bool,

    /// 依赖的其他服务
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depends_on: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServicePatchRequest {
    /// 执行命令
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,

    /// 工作目录
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>,

    /// 环境变量
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,

    /// 端口配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ports: Option<Vec<PortMapping>>,

    /// 健康检查配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health_check: Option<HealthCheckConfig>,

    /// 资源限制
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourceLimits>,

    /// 重启策略
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart_policy: Option<RestartPolicy>,

    /// 自动启动
    #[serde(skip_serializing_if = "Option::is_none")]
    pub autostart: Option<bool>,

    /// 依赖服务列表
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depends_on: Option<Vec<String>>,
}

/// 服务操作响应(start/stop/restart/enable/disable)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceActionResponse {
    /// 操作成功消息
    pub message: String,

    /// 服务名称
    pub service: String,

    /// 操作类型(start, stop, restart, enable, disable)
    pub action: String,

    /// 操作时间戳(Unix timestamp)
    pub timestamp: i64,
}

/// 服务日志响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceLogsResponse {
    /// 服务名称
    pub service: String,

    /// 日志条目列表
    pub lines: Vec<LogEntry>,

    /// 总行数
    pub total: usize,

    /// 是否还有更多日志
    pub has_more: bool,
}

/// 依赖树响应（别名类型）
pub type DependencyTreeResponse = DependencyTree;

/// 服务删除查询参数
#[derive(Debug, Deserialize)]
pub struct DeleteServiceQuery {
    /// 强制删除（即使服务正在运行）
    #[serde(default)]
    pub force: bool,
}

/// 服务停止查询参数
#[derive(Debug, Deserialize)]
pub struct StopServiceQuery {
    /// 停止超时时间（秒，默认 10）
    #[serde(default = "default_stop_timeout")]
    pub timeout: u64,

    /// 停止信号（SIGTERM, SIGKILL 等，默认 SIGTERM）
    #[serde(default = "default_stop_signal")]
    pub signal: String,
}

fn default_stop_timeout() -> u64 {
    10
}
fn default_stop_signal() -> String {
    "SIGTERM".to_string()
}

/// 服务日志查询参数
#[derive(Debug, Deserialize)]
pub struct LogsQuery {
    /// 行数（默认 100）
    #[serde(default = "default_log_lines")]
    pub lines: usize,

    /// 实时跟踪（SSE）
    #[serde(default)]
    pub follow: bool,

    /// 日志级别过滤（可选）
    #[serde(default)]
    pub level: Option<String>,

    /// 开始时间戳（Unix timestamp）
    #[serde(default)]
    pub since: Option<i64>,

    /// 结束时间戳（Unix timestamp）
    #[serde(default)]
    pub until: Option<i64>,
}

fn default_log_lines() -> usize {
    100
}

/// 服务日志条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// 时间戳（Unix timestamp）
    pub timestamp: i64,

    /// 日志级别（info, warn, error 等）
    pub level: String,

    /// 日志内容
    pub message: String,
}

/// 服务依赖树响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyTree {
    /// 服务名称
    pub service: String,

    /// 直接依赖的服务
    pub depends_on: Vec<String>,

    /// 被哪些服务依赖
    pub required_by: Vec<String>,

    /// 完整依赖树（递归展开）
    pub tree: Vec<DependencyNode>,
}

/// 依赖节点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyNode {
    /// 服务名称
    pub name: String,

    /// 当前状态
    pub state: ServiceState,

    /// 子依赖
    pub children: Vec<DependencyNode>,
}

/// 批量启动请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchStartRequest {
    /// 要启动的服务列表
    pub services: Vec<String>,
}

/// 批量停止请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchStopRequest {
    /// 要停止的服务列表
    pub services: Vec<String>,

    /// 停止超时时间（秒）
    #[serde(default = "default_stop_timeout")]
    pub timeout: u64,
}

/// 批量操作结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchOperationResult {
    /// 成功的服务列表
    pub succeeded: Vec<String>,

    /// 失败的服务列表
    pub failed: Vec<String>,

    /// 详细结果
    pub details: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_definition_validate_valid() {
        let def = ServiceDefinition {
            name: "test-service".to_string(),
            command: "node server.js".to_string(),
            working_dir: Some("/app".to_string()),
            env: Some([("PORT".to_string(), "3000".to_string())].into()),
            ports: Some(vec![PortMapping {
                host: 8080,
                container: 8080,
                protocol: "tcp".to_string(),
            }]),
            health_check: None,
            resources: None,
            restart_policy: Some(RestartPolicy::OnFailure),
            autostart: true,
            depends_on: None,
        };

        assert!(def.validate().is_ok());
    }

    #[test]
    fn test_service_definition_validate_invalid_name() {
        let def = ServiceDefinition {
            name: "invalid name!".to_string(),
            command: "echo test".to_string(),
            working_dir: None,
            env: None,
            ports: None,
            health_check: None,
            resources: None,
            restart_policy: None,
            autostart: false,
            depends_on: None,
        };

        assert!(def.validate().is_err());
    }

    #[test]
    fn test_service_definition_validate_invalid_port() {
        let def = ServiceDefinition {
            name: "test-service".to_string(),
            command: "node server.js".to_string(),
            working_dir: None,
            env: None,
            ports: Some(vec![PortMapping {
                host: 0, // 无效端口（0 不在有效范围）
                container: 8080,
                protocol: "tcp".to_string(),
            }]),
            health_check: None,
            resources: None,
            restart_policy: None,
            autostart: false,
            depends_on: None,
        };

        assert!(def.validate().is_err());
    }

    #[test]
    fn test_restart_policy_default() {
        assert_eq!(RestartPolicy::default(), RestartPolicy::OnFailure);
    }

    #[test]
    fn test_port_mapping_default_protocol() {
        let json = r#"{"host": 8080, "container": 8080}"#;
        let port: PortMapping = serde_json::from_str(json).unwrap();
        assert_eq!(port.protocol, "tcp");
    }
}
