# T08: Cloudflare 隧道管理原子

> 版本：1.0.0
> 技术基础：cloudflared CLI

## 概述

提供 Cloudflare Tunnel 的管理能力，用于将本地服务安全地暴露到互联网。

---

## ADDED Requirements

### Requirement: 隧道认证
系统 **MUST** 支持 Cloudflare 账户认证。

#### Scenario: 初次认证
- **WHEN** 用户首次使用隧道功能
- **THEN** 系统 **SHALL** 引导执行 `cloudflared tunnel login`
- **AND** 保存认证凭证

#### Scenario: 凭证验证
- **WHEN** 系统启动时
- **THEN** 系统 **SHALL** 验证现有凭证是否有效
- **AND** 凭证无效时提示重新认证

---

### Requirement: 隧道管理
系统 **MUST** 支持创建和管理 Cloudflare 隧道。

#### Scenario: 创建隧道
- **WHEN** 用户请求创建隧道
- **THEN** 系统 **SHALL** 执行 `cloudflared tunnel create {name}`
- **AND** 保存隧道凭证文件

#### Scenario: 列出隧道
- **WHEN** 用户请求列出隧道
- **THEN** 系统 **SHALL** 返回隧道列表
- **AND** 包含：名称、ID、创建时间、状态

#### Scenario: 删除隧道
- **WHEN** 用户请求删除隧道
- **THEN** 系统 **SHALL** 先停止隧道（如正在运行）
- **AND** 执行 `cloudflared tunnel delete {name}`
- **AND** 清理相关配置文件

---

### Requirement: 隧道配置
系统 **MUST** 支持配置隧道的 ingress 规则。

#### Scenario: 添加 Ingress 规则
- **WHEN** 用户配置服务映射
- **THEN** 系统 **SHALL** 在隧道配置文件中添加 ingress 规则
- **AND** 格式遵循 cloudflared 配置规范

#### Scenario: Ingress 规则格式
- **WHEN** 配置 ingress 规则时
- **THEN** 系统 **MUST** 支持：
  - `hostname`: 域名匹配
  - `path`: 路径前缀匹配
  - `service`: 目标服务（http://localhost:port）
  - 兜底规则：`service: http_status:404`

#### Scenario: 配置示例
- **WHEN** 配置多服务映射
- **THEN** 配置文件格式应为：
```yaml
tunnel: {tunnel-id}
credentials-file: /path/to/credentials.json

ingress:
  - hostname: app.example.com
    service: http://localhost:8080
  - hostname: api.example.com
    path: /v1/*
    service: http://localhost:3000
  - service: http_status:404
```

---

### Requirement: DNS 管理
系统 **SHOULD** 支持自动配置 DNS 记录。

#### Scenario: 创建 DNS 记录
- **WHEN** 用户请求为隧道创建 DNS 记录
- **THEN** 系统 **SHALL** 执行 `cloudflared tunnel route dns {tunnel} {hostname}`

#### Scenario: 列出 DNS 记录
- **WHEN** 用户请求列出隧道关联的 DNS 记录
- **THEN** 系统 **SHALL** 返回 DNS 记录列表

---

### Requirement: 隧道运行
系统 **MUST** 支持运行和管理隧道进程。

#### Scenario: 启动隧道
- **WHEN** 用户请求启动隧道
- **THEN** 系统 **SHALL** 通过 systemd --user 启动 cloudflared
- **AND** 使用 T06 (SystemdAtom) 创建服务 unit

#### Scenario: 停止隧道
- **WHEN** 用户请求停止隧道
- **THEN** 系统 **SHALL** 通过 systemd 停止服务

#### Scenario: 隧道状态
- **WHEN** 用户查询隧道状态
- **THEN** 系统 **SHALL** 返回：
  - 运行状态
  - 连接数
  - 延迟信息
  - 最近错误（如有）

---

## 接口定义

```rust
pub trait TunnelAtom {
    // 认证
    async fn login(&self) -> Result<()>;
    async fn is_authenticated(&self) -> Result<bool>;
    
    // 隧道管理
    async fn create(&self, name: &str) -> Result<TunnelInfo>;
    async fn delete(&self, name: &str) -> Result<()>;
    async fn list(&self) -> Result<Vec<TunnelInfo>>;
    async fn get(&self, name: &str) -> Result<TunnelInfo>;
    
    // 配置管理
    fn set_ingress(&self, tunnel: &str, rules: &[IngressRule]) -> Result<()>;
    fn get_ingress(&self, tunnel: &str) -> Result<Vec<IngressRule>>;
    fn add_ingress_rule(&self, tunnel: &str, rule: &IngressRule) -> Result<()>;
    fn remove_ingress_rule(&self, tunnel: &str, hostname: &str) -> Result<()>;
    
    // DNS 管理
    async fn route_dns(&self, tunnel: &str, hostname: &str) -> Result<()>;
    async fn list_dns_routes(&self, tunnel: &str) -> Result<Vec<DnsRoute>>;
    
    // 运行控制（通过 SystemdAtom 委托）
    async fn start(&self, tunnel: &str) -> Result<()>;
    async fn stop(&self, tunnel: &str) -> Result<()>;
    async fn status(&self, tunnel: &str) -> Result<TunnelStatus>;
}

pub struct TunnelInfo {
    pub id: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub connections: u32,
}

pub struct IngressRule {
    pub hostname: Option<String>,
    pub path: Option<String>,
    pub service: String,
}

pub struct TunnelStatus {
    pub running: bool,
    pub connections: u32,
    pub latency_ms: Option<u32>,
    pub errors: Vec<String>,
}
```

---

## 配置项

```toml
[tunnel]
# cloudflared 配置目录
config_dir = "~/.config/svcmgr/managed/cloudflared"

# 凭证文件位置
credentials_dir = "~/.cloudflared"

# 默认隧道名称
default_tunnel = "svcmgr"

# 是否自动创建 DNS 记录
auto_dns = true
```

---

## 与 Systemd 集成

隧道服务通过 systemd unit 管理，模板如下：

### cloudflared.service.j2
```jinja2
[Unit]
Description=Cloudflare Tunnel {{ tunnel_name }}
After=network-online.target
Wants=network-online.target

[Service]
Type=notify
ExecStart={{ cloudflared_path }} tunnel --config {{ config_file }} run {{ tunnel_name }}
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
```
