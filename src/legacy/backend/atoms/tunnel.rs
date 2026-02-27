#![allow(dead_code)]

/// Cloudflare Tunnel 隧道管理原子
///
/// 本模块提供 Cloudflare Tunnel 管理功能：
/// - 隧道认证（登录、验证）
/// - 隧道生命周期管理（创建、删除、列表、查询）
/// - Ingress 配置管理（添加、删除、查询路由规则）
/// - DNS 路由管理（DNS CNAME 记录）
/// - 运行控制（启动、停止、状态查询，通过 SupervisorAtom 委托）
use crate::atoms::supervisor::{SupervisorAtom, SupervisorManager};
use crate::error::{Error, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use tokio::fs;

// ========================================
// 数据结构
// ========================================

/// 隧道信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelInfo {
    /// 隧道 ID
    pub id: String,
    /// 隧道名称
    pub name: String,
    /// 创建时间
    #[serde(with = "datetime_format")]
    pub created_at: DateTime<Utc>,
    /// 活跃连接数
    pub connections: u32,
}

/// Ingress 路由规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngressRule {
    /// 主机名（None 表示兜底规则）
    pub hostname: Option<String>,
    /// 路径匹配（可选）
    pub path: Option<String>,
    /// 后端服务地址
    pub service: String,
}

/// DNS 路由记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsRoute {
    /// 主机名
    pub hostname: String,
    /// 隧道 ID
    pub tunnel_id: String,
}

/// 隧道运行状态
#[derive(Debug, Clone)]
pub struct TunnelStatus {
    /// 是否运行中
    pub running: bool,
    /// 活跃连接数
    pub connections: u32,
    /// 延迟（毫秒）
    pub latency_ms: Option<u32>,
    /// 错误信息列表
    pub errors: Vec<String>,
}

/// Ingress 配置结构（用于 YAML 序列化）
#[derive(Debug, Clone, Serialize, Deserialize)]
struct IngressConfig {
    tunnel: String,
    #[serde(rename = "credentials-file")]
    credentials_file: String,
    ingress: Vec<IngressRule>,
}

// 辅助模块：DateTime 序列化/反序列化
mod datetime_format {
    use chrono::{DateTime, Utc};
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(date: &DateTime<Utc>, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = date.to_rfc3339();
        serializer.serialize_str(&s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> std::result::Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse::<DateTime<Utc>>().map_err(serde::de::Error::custom)
    }
}

// ========================================
// TunnelAtom Trait
// ========================================

/// Cloudflare Tunnel 管理 trait
pub trait TunnelAtom {
    // ===== 认证 =====

    /// 执行 cloudflared 认证登录
    ///
    /// 运行 `cloudflared tunnel login`，引导用户在浏览器中完成认证。
    /// 认证成功后，凭证保存在 `~/.cloudflared/cert.pem`。
    fn login(&self) -> impl std::future::Future<Output = Result<()>> + Send;

    /// 检查是否已认证
    ///
    /// 验证 `~/.cloudflared/cert.pem` 是否存在且有效。
    fn is_authenticated(&self) -> impl std::future::Future<Output = Result<bool>> + Send;

    // ===== 隧道管理 =====

    /// 创建新隧道
    ///
    /// # 参数
    /// - `name`: 隧道名称（唯一标识）
    ///
    /// # 返回
    /// - 隧道信息（包含 tunnel_id）
    fn create(&self, name: &str) -> impl std::future::Future<Output = Result<TunnelInfo>> + Send;

    /// 删除隧道
    ///
    /// # 参数
    /// - `name`: 隧道名称
    ///
    /// # 注意
    /// - 会先停止关联的 supervisor 服务
    /// - 删除隧道配置文件
    /// - 删除 Cloudflare 服务器上的隧道记录
    fn delete(&self, name: &str) -> impl std::future::Future<Output = Result<()>> + Send;

    /// 列出所有隧道
    ///
    /// # 返回
    /// - 隧道信息列表
    fn list(&self) -> impl std::future::Future<Output = Result<Vec<TunnelInfo>>> + Send;

    /// 获取指定隧道信息
    ///
    /// # 参数
    /// - `name`: 隧道名称
    fn get(&self, name: &str) -> impl std::future::Future<Output = Result<TunnelInfo>> + Send;

    // ===== Ingress 配置 =====

    /// 设置隧道的 Ingress 规则
    ///
    /// # 参数
    /// - `tunnel`: 隧道名称
    /// - `rules`: Ingress 规则列表（必须包含兜底规则）
    ///
    /// # 注意
    /// - 规则列表必须以兜底规则结尾（hostname 为 None）
    /// - 会覆盖现有配置
    fn set_ingress(&self, tunnel: &str, rules: &[IngressRule]) -> Result<()>;

    /// 获取隧道的 Ingress 规则
    ///
    /// # 参数
    /// - `tunnel`: 隧道名称
    fn get_ingress(&self, tunnel: &str) -> Result<Vec<IngressRule>>;

    /// 添加单条 Ingress 规则
    ///
    /// # 参数
    /// - `tunnel`: 隧道名称
    /// - `rule`: 新规则（不能是兜底规则）
    ///
    /// # 注意
    /// - 新规则会插入到兜底规则之前
    fn add_ingress_rule(&self, tunnel: &str, rule: &IngressRule) -> Result<()>;

    /// 删除 Ingress 规则
    ///
    /// # 参数
    /// - `tunnel`: 隧道名称
    /// - `hostname`: 要删除的主机名
    fn remove_ingress_rule(&self, tunnel: &str, hostname: &str) -> Result<()>;

    // ===== DNS 管理 =====

    /// 添加 DNS 路由
    ///
    /// # 参数
    /// - `tunnel`: 隧道名称
    /// - `hostname`: 主机名（例如：app.example.com）
    ///
    /// # 注意
    /// - 会在 Cloudflare DNS 中创建 CNAME 记录
    fn route_dns(
        &self,
        tunnel: &str,
        hostname: &str,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    /// 列出隧道的 DNS 路由
    ///
    /// # 参数
    /// - `tunnel`: 隧道名称
    fn list_dns_routes(
        &self,
        tunnel: &str,
    ) -> impl std::future::Future<Output = Result<Vec<DnsRoute>>> + Send;

    // ===== 运行控制（委托给 SupervisorAtom） =====

    /// 启动隧道服务
    ///
    /// # 参数
    /// - `tunnel`: 隧道名称
    ///
    /// # 注意
    /// - 通过 SupervisorAtom 启动 cloudflared 服务
    fn start(&self, tunnel: &str) -> impl std::future::Future<Output = Result<()>> + Send;

    /// 停止隧道服务
    ///
    /// # 参数
    /// - `tunnel`: 隧道名称
    fn stop(&self, tunnel: &str) -> impl std::future::Future<Output = Result<()>> + Send;

    /// 查询隧道运行状态
    ///
    /// # 参数
    /// - `tunnel`: 隧道名称
    fn status(
        &self,
        tunnel: &str,
    ) -> impl std::future::Future<Output = Result<TunnelStatus>> + Send;
}

// ========================================
// TunnelManager 实现
// ========================================

/// Cloudflare Tunnel 管理器
pub struct TunnelManager {
    config_dir: PathBuf,
    credentials_dir: PathBuf,
    supervisor: SupervisorManager,
}

impl TunnelManager {
    /// 创建新的 Tunnel 管理器
    ///
    /// # 参数
    /// - `config_dir`: 配置文件目录
    /// - `credentials_dir`: 凭证目录
    /// - `supervisor`: SupervisorAtom 实现
    pub fn new(
        config_dir: PathBuf,
        credentials_dir: PathBuf,
        supervisor: SupervisorManager,
    ) -> Self {
        Self {
            config_dir,
            credentials_dir,
            supervisor,
        }
    }

    pub fn default_config(supervisor: SupervisorManager) -> Result<Self> {
        let home = std::env::var("HOME")
            .map_err(|_| Error::Config("HOME environment variable not set".to_string()))?;
        let config_dir = PathBuf::from(&home)
            .join(".config")
            .join("svcmgr")
            .join("managed")
            .join("cloudflared");
        let credentials_dir = PathBuf::from(&home).join(".cloudflared");

        Ok(Self::new(config_dir, credentials_dir, supervisor))
    }

    /// 运行 cloudflared 命令
    fn run_cloudflared(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("cloudflared").args(args).output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::CommandFailed {
                command: format!("cloudflared {}", args.join(" ")),
                exit_code: output.status.code(),
                stderr: stderr.to_string(),
            });
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// 确保配置目录存在
    async fn ensure_config_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.config_dir).await?;
        Ok(())
    }

    /// 获取隧道配置文件路径
    fn config_path(&self, tunnel: &str) -> PathBuf {
        self.config_dir.join(format!("{}.yaml", tunnel))
    }

    /// 获取隧道凭证文件路径
    fn credentials_path(&self, tunnel_id: &str) -> PathBuf {
        self.credentials_dir.join(format!("{}.json", tunnel_id))
    }

    /// 获取认证凭证文件路径
    fn cert_path(&self) -> PathBuf {
        self.credentials_dir.join("cert.pem")
    }

    /// 解析 cloudflared tunnel list 输出（JSON 格式）
    fn parse_tunnel_list(&self, output: &str) -> Result<Vec<TunnelInfo>> {
        // cloudflared tunnel list --output json 输出格式：
        // [{"id":"xxx","name":"yyy","created_at":"2021-01-01T00:00:00Z","connections":[...]}]
        #[derive(Deserialize)]
        struct TunnelListItem {
            id: String,
            name: String,
            #[serde(rename = "createdAt")]
            created_at: String,
            #[serde(default)]
            conns: Vec<serde_json::Value>,
        }

        let items: Vec<TunnelListItem> = serde_json::from_str(output)
            .map_err(|e| Error::Other(format!("Failed to parse tunnel list: {}", e)))?;

        let tunnels = items
            .into_iter()
            .map(|item| {
                let created_at = item
                    .created_at
                    .parse::<DateTime<Utc>>()
                    .unwrap_or_else(|_| Utc::now());
                TunnelInfo {
                    id: item.id,
                    name: item.name,
                    created_at,
                    connections: item.conns.len() as u32,
                }
            })
            .collect();

        Ok(tunnels)
    }

    /// 从 cloudflared tunnel create 输出中提取 tunnel_id
    fn extract_tunnel_id(&self, output: &str) -> Result<String> {
        // 输出格式示例：
        // Tunnel credentials written to /home/user/.cloudflared/xxx-xxx-xxx.json. cloudflared chose this file based on where your origin certificate was found. Keep this file secret. To revoke these credentials, delete the tunnel.
        // Created tunnel my-tunnel with id xxx-xxx-xxx
        for line in output.lines() {
            if line.contains("Created tunnel") && line.contains("with id") {
                if let Some(id_start) = line.rfind("with id ") {
                    let id = line[id_start + 8..].trim();
                    return Ok(id.to_string());
                }
            }
        }

        Err(Error::Other(
            "Failed to extract tunnel ID from cloudflared output".to_string(),
        ))
    }

    /// 验证 Ingress 规则
    fn validate_ingress_rules(&self, rules: &[IngressRule]) -> Result<()> {
        if rules.is_empty() {
            return Err(Error::InvalidArgument(
                "Ingress rules cannot be empty".to_string(),
            ));
        }

        // 最后一条规则必须是兜底规则
        let last_rule = rules.last().unwrap();
        if last_rule.hostname.is_some() {
            return Err(Error::InvalidArgument(
                "Last ingress rule must be a catch-all (hostname: None)".to_string(),
            ));
        }

        // 检查兜底规则的 service
        if !last_rule.service.starts_with("http_status:") {
            return Err(Error::InvalidArgument(
                "Catch-all rule service must be http_status:xxx".to_string(),
            ));
        }

        Ok(())
    }

    /// 构建 Ingress 配置
    fn build_ingress_config(&self, tunnel_id: &str, rules: &[IngressRule]) -> Result<String> {
        self.validate_ingress_rules(rules)?;

        let credentials_file = self.credentials_path(tunnel_id);
        let config = IngressConfig {
            tunnel: tunnel_id.to_string(),
            credentials_file: credentials_file.to_string_lossy().to_string(),
            ingress: rules.to_vec(),
        };

        serde_yaml::to_string(&config)
            .map_err(|e| Error::Other(format!("Failed to serialize ingress config: {}", e)))
    }

    /// 解析 Ingress 配置
    fn parse_ingress_config(&self, content: &str) -> Result<(String, Vec<IngressRule>)> {
        let config: IngressConfig = serde_yaml::from_str(content)
            .map_err(|e| Error::Other(format!("Failed to parse ingress config: {}", e)))?;

        Ok((config.tunnel, config.ingress))
    }

    /// 格式化隧道运行命令
    #[cfg(test)]
    fn format_tunnel_run_command(&self, tunnel: &str) -> String {
        let config_file = self.config_path(tunnel);
        format!(
            "/usr/bin/cloudflared tunnel --config {} run {}",
            config_file.display(),
            tunnel
        )
    }

    /// 生成服务名称
    fn service_name(&self, tunnel: &str) -> String {
        format!("cloudflared-{}", tunnel)
    }

    /// 创建 supervisor 服务单元
    async fn create_supervisor_service(&self, tunnel: &str) -> Result<()> {
        let service_name = self.service_name(tunnel);
        let config_file = self.config_path(tunnel);

        // ServiceDef TOML content (used by built-in supervisor)
        let unit_content = format!(
            r#"name = "{}"
description = "Cloudflare Tunnel - {}"
command = "/usr/bin/cloudflared"
args = ["tunnel", "--config", "{}", "run", "{}"]
env = {{}}
restart_policy = "OnFailure"
restart_sec = 5
enabled = true
stop_timeout_sec = 10
"#,
            service_name,
            tunnel,
            config_file.display(),
            tunnel
        );

        self.supervisor
            .create_unit(&service_name, &unit_content)
            .await?;
        Ok(())
    }
}

impl TunnelAtom for TunnelManager {
    async fn login(&self) -> Result<()> {
        // 运行 cloudflared tunnel login（会打开浏览器）
        let output = Command::new("cloudflared")
            .args(["tunnel", "login"])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::CommandFailed {
                command: "cloudflared tunnel login".to_string(),
                exit_code: output.status.code(),
                stderr: stderr.to_string(),
            });
        }

        // 验证凭证文件是否创建成功
        if !self.cert_path().exists() {
            return Err(Error::Other(
                "Authentication failed: cert.pem not found".to_string(),
            ));
        }

        Ok(())
    }

    async fn is_authenticated(&self) -> Result<bool> {
        Ok(self.cert_path().exists())
    }

    async fn create(&self, name: &str) -> Result<TunnelInfo> {
        self.ensure_config_dir().await?;

        // 创建隧道
        let output = self.run_cloudflared(&["tunnel", "create", name])?;

        // 提取 tunnel_id
        let tunnel_id = self.extract_tunnel_id(&output)?;

        // 创建默认 Ingress 配置（仅兜底规则）
        let default_rules = vec![IngressRule {
            hostname: None,
            path: None,
            service: "http_status:404".to_string(),
        }];

        let config_content = self.build_ingress_config(&tunnel_id, &default_rules)?;
        let config_path = self.config_path(name);
        fs::write(&config_path, config_content).await?;

        // 创建 supervisor 服务
        self.create_supervisor_service(name).await?;

        Ok(TunnelInfo {
            id: tunnel_id,
            name: name.to_string(),
            created_at: Utc::now(),
            connections: 0,
        })
    }

    async fn delete(&self, name: &str) -> Result<()> {
        // 停止服务
        let service_name = self.service_name(name);
        let _ = self.supervisor.stop(&service_name).await;
        let _ = self.supervisor.delete_unit(&service_name).await;

        // 删除隧道
        self.run_cloudflared(&["tunnel", "delete", name])?;

        // 删除配置文件
        let config_path = self.config_path(name);
        if config_path.exists() {
            fs::remove_file(&config_path).await?;
        }

        Ok(())
    }

    async fn list(&self) -> Result<Vec<TunnelInfo>> {
        let output = self.run_cloudflared(&["tunnel", "list", "--output", "json"])?;
        self.parse_tunnel_list(&output)
    }

    async fn get(&self, name: &str) -> Result<TunnelInfo> {
        let tunnels = self.list().await?;
        tunnels
            .into_iter()
            .find(|t| t.name == name)
            .ok_or_else(|| Error::NotSupported(format!("Tunnel {} not found", name)))
    }

    fn set_ingress(&self, tunnel: &str, rules: &[IngressRule]) -> Result<()> {
        self.validate_ingress_rules(rules)?;

        // 读取现有配置获取 tunnel_id
        let config_path = self.config_path(tunnel);
        if !config_path.exists() {
            return Err(Error::NotSupported(format!("Tunnel {} not found", tunnel)));
        }

        let existing_content = std::fs::read_to_string(&config_path)?;
        let (tunnel_id, _) = self.parse_ingress_config(&existing_content)?;

        // 构建新配置
        let new_content = self.build_ingress_config(&tunnel_id, rules)?;
        std::fs::write(&config_path, new_content)?;

        Ok(())
    }

    fn get_ingress(&self, tunnel: &str) -> Result<Vec<IngressRule>> {
        let config_path = self.config_path(tunnel);
        if !config_path.exists() {
            return Err(Error::NotSupported(format!("Tunnel {} not found", tunnel)));
        }

        let content = std::fs::read_to_string(&config_path)?;
        let (_, rules) = self.parse_ingress_config(&content)?;
        Ok(rules)
    }

    fn add_ingress_rule(&self, tunnel: &str, rule: &IngressRule) -> Result<()> {
        if rule.hostname.is_none() {
            return Err(Error::InvalidArgument(
                "Cannot add catch-all rule (use set_ingress instead)".to_string(),
            ));
        }

        let mut rules = self.get_ingress(tunnel)?;

        // 移除兜底规则
        let catch_all = rules.pop().ok_or_else(|| {
            Error::Other("Invalid ingress config: missing catch-all rule".to_string())
        })?;

        // 添加新规则
        rules.push(rule.clone());

        // 恢复兜底规则
        rules.push(catch_all);

        self.set_ingress(tunnel, &rules)
    }

    fn remove_ingress_rule(&self, tunnel: &str, hostname: &str) -> Result<()> {
        let mut rules = self.get_ingress(tunnel)?;

        let original_len = rules.len();
        rules.retain(|r| r.hostname.as_deref() != Some(hostname));

        if rules.len() == original_len {
            return Err(Error::NotSupported(format!(
                "Ingress rule for {} not found",
                hostname
            )));
        }

        self.set_ingress(tunnel, &rules)
    }

    async fn route_dns(&self, tunnel: &str, hostname: &str) -> Result<()> {
        self.run_cloudflared(&["tunnel", "route", "dns", tunnel, hostname])?;
        Ok(())
    }

    async fn list_dns_routes(&self, tunnel: &str) -> Result<Vec<DnsRoute>> {
        // 获取隧道信息
        let tunnel_info = self.get(tunnel).await?;

        // cloudflared tunnel route list 输出示例：
        // app.example.com -> xxx-xxx-xxx
        let output = self.run_cloudflared(&["tunnel", "route", "list"])?;

        let mut routes = Vec::new();
        for line in output.lines() {
            if line.contains(&tunnel_info.id) || line.contains(&tunnel_info.name) {
                if let Some(hostname) = line.split("->").next() {
                    routes.push(DnsRoute {
                        hostname: hostname.trim().to_string(),
                        tunnel_id: tunnel_info.id.clone(),
                    });
                }
            }
        }

        Ok(routes)
    }

    async fn start(&self, tunnel: &str) -> Result<()> {
        let service_name = self.service_name(tunnel);
        self.supervisor.start(&service_name).await
    }

    async fn stop(&self, tunnel: &str) -> Result<()> {
        let service_name = self.service_name(tunnel);
        self.supervisor.stop(&service_name).await
    }

    async fn status(&self, tunnel: &str) -> Result<TunnelStatus> {
        let service_name = self.service_name(tunnel);
        let unit_status = self.supervisor.status(&service_name).await?;

        // 解析日志获取连接数和错误信息
        let logs = self
            .supervisor
            .logs(
                &service_name,
                &crate::atoms::supervisor::LogOptions::default(),
            )
            .await?;

        let mut connections = 0;
        let mut errors = Vec::new();

        for entry in logs {
            let msg = entry.message.to_lowercase();
            if msg.contains("connection") {
                connections += 1;
            }
            if msg.contains("error") || msg.contains("failed") {
                errors.push(entry.message.clone());
            }
        }

        Ok(TunnelStatus {
            running: matches!(
                unit_status.active_state,
                crate::atoms::supervisor::ActiveState::Active
            ),
            connections,
            latency_ms: None, // TODO: 解析延迟信息
            errors,
        })
    }
}

// ========================================
// 单元测试
// ========================================

#[cfg(test)]
mod tests {
    use super::*;

    // Mock SupervisorAtom 用于测试
    fn create_test_manager() -> TunnelManager {
        let tmpdir = std::env::temp_dir().join("svcmgr-test-tunnel");
        let credentials_dir = tmpdir.join("credentials");
        let supervisor_dir = tmpdir.join("supervisor");
        TunnelManager::new(
            tmpdir.clone(),
            credentials_dir,
            SupervisorManager::new(supervisor_dir, false),
        )
    }

    #[test]
    fn test_parse_tunnel_list() {
        let manager = create_test_manager();
        let json = r#"[
            {
                "id": "abc-123",
                "name": "my-tunnel",
                "createdAt": "2021-01-01T00:00:00Z",
                "conns": [{"id": "conn1"}]
            }
        ]"#;

        let tunnels = manager.parse_tunnel_list(json).unwrap();
        assert_eq!(tunnels.len(), 1);
        assert_eq!(tunnels[0].id, "abc-123");
        assert_eq!(tunnels[0].name, "my-tunnel");
        assert_eq!(tunnels[0].connections, 1);
    }

    #[test]
    fn test_build_ingress_config() {
        let manager = create_test_manager();
        let rules = vec![
            IngressRule {
                hostname: Some("app.example.com".to_string()),
                path: None,
                service: "http://localhost:8080".to_string(),
            },
            IngressRule {
                hostname: None,
                path: None,
                service: "http_status:404".to_string(),
            },
        ];

        let config = manager
            .build_ingress_config("test-tunnel-id", &rules)
            .unwrap();

        assert!(config.contains("tunnel: test-tunnel-id"));
        assert!(config.contains("hostname: app.example.com"));
        assert!(config.contains("service: http://localhost:8080"));
        assert!(config.contains("service: http_status:404"));
    }

    #[test]
    fn test_parse_ingress_config() {
        let manager = create_test_manager();
        let yaml = r#"
tunnel: test-tunnel-id
credentials-file: /path/to/creds.json
ingress:
  - hostname: app.example.com
    service: http://localhost:8080
  - service: http_status:404
"#;

        let (tunnel_id, rules) = manager.parse_ingress_config(yaml).unwrap();
        assert_eq!(tunnel_id, "test-tunnel-id");
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].hostname, Some("app.example.com".to_string()));
        assert_eq!(rules[1].hostname, None);
    }

    #[test]
    fn test_validate_ingress_rules() {
        let manager = create_test_manager();

        // 有效规则：带兜底
        let valid_rules = vec![
            IngressRule {
                hostname: Some("test.com".to_string()),
                path: None,
                service: "http://localhost:8080".to_string(),
            },
            IngressRule {
                hostname: None,
                path: None,
                service: "http_status:404".to_string(),
            },
        ];
        assert!(manager.validate_ingress_rules(&valid_rules).is_ok());

        // 无效规则：缺少兜底
        let invalid_rules = vec![IngressRule {
            hostname: Some("test.com".to_string()),
            path: None,
            service: "http://localhost:8080".to_string(),
        }];
        assert!(manager.validate_ingress_rules(&invalid_rules).is_err());

        // 无效规则：兜底规则 service 错误
        let invalid_catch_all = vec![IngressRule {
            hostname: None,
            path: None,
            service: "http://localhost:8080".to_string(),
        }];
        assert!(manager.validate_ingress_rules(&invalid_catch_all).is_err());
    }

    #[tokio::test]
    async fn test_check_authentication() {
        let manager = create_test_manager();
        // cert.pem 不存在时返回 false
        let result = manager.is_authenticated().await.unwrap();
        assert!(!result);
    }

    #[test]
    fn test_format_tunnel_run_command() {
        let manager = create_test_manager();
        let cmd = manager.format_tunnel_run_command("my-tunnel");
        assert!(cmd.contains("cloudflared tunnel"));
        assert!(cmd.contains("my-tunnel"));
        assert!(cmd.contains(".yaml"));
    }

    #[test]
    fn test_extract_tunnel_id() {
        let manager = create_test_manager();
        let output = "Tunnel credentials written to /home/user/.cloudflared/xxx-yyy-zzz.json.\nCreated tunnel my-tunnel with id xxx-yyy-zzz\n";
        let tunnel_id = manager.extract_tunnel_id(output).unwrap();
        assert_eq!(tunnel_id, "xxx-yyy-zzz");
    }

    #[test]
    fn test_service_name() {
        let manager = create_test_manager();
        assert_eq!(manager.service_name("test"), "cloudflared-test");
    }

    #[test]
    fn test_config_path() {
        let manager = create_test_manager();
        let path = manager.config_path("test");
        assert!(path.to_string_lossy().contains("test.yaml"));
    }

    #[test]
    fn test_credentials_path() {
        let manager = create_test_manager();
        let path = manager.credentials_path("abc-123");
        assert!(path.to_string_lossy().contains("abc-123.json"));
    }
}
