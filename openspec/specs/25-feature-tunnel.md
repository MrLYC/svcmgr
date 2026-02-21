# Feature: Cloudflare Tunnel Management (F05)

**特性代号**: F05  
**原子依赖**: A02 (模板引擎), A01 (Git 配置版本)  
**功能目标**: 提供 Cloudflare Tunnel 隧道管理能力，支持隧道创建、配置、状态监控和生命周期管理

---

## 需求说明

### 概述

系统 **必须** 提供 Cloudflare Tunnel 管理功能，允许用户通过 Web 界面和 REST API 管理 Cloudflare Tunnel 隧道配置。所有隧道配置变更 **必须** 通过 Git 仓库进行版本管理。

### 核心能力

1. **隧道列表查询**: 获取所有已配置隧道及其状态
2. **隧道详情查询**: 获取单个隧道的完整信息
3. **隧道创建**: 配置新的 Cloudflare Tunnel 隧道
4. **隧道更新**: 修改隧道的域名、服务地址等配置
5. **隧道删除**: 移除已配置的隧道
6. **状态监控**: 实时查看隧道连接状态和运行时间

### 技术约束

- **API 基础路径**: `/svcmgr/api/cloudflare`
- **配置存储**: `~/.local/share/svcmgr/cloudflare/tunnels/`
- **版本管理**: 所有配置变更必须通过 Git 原子提交
- **用户级**: 使用用户级 cloudflared 进程（不使用 sudo）
- **模板驱动**: 隧道配置文件通过 Jinja2 模板生成

---

## ADDED Requirements

### Requirement: 隧道列表查询
系统 **必须** 提供 REST API 端点用于查询所有 Cloudflare Tunnel 隧道配置和状态。

#### Scenario: 查询所有隧道
- **WHEN** 客户端发送 `GET /svcmgr/api/cloudflare/tunnels` 请求
- **THEN** 系统 **应当** 返回所有隧道配置的 JSON 数组
- **AND** 每个隧道对象 **应当** 包含 `id, name, domain, service_url, status, uptime` 字段
- **AND** `status` 字段 **必须** 为 `"connected" | "disconnected" | "degraded"` 之一
- **AND** HTTP 响应状态码 **应当** 为 `200 OK`

#### Scenario: 空隧道列表
- **WHEN** 系统中无任何已配置隧道
- **THEN** 系统 **应当** 返回空数组 `[]`
- **AND** HTTP 响应状态码 **应当** 为 `200 OK`

---

### Requirement: 隧道详情查询
系统 **必须** 提供 REST API 端点用于查询单个隧道的详细信息。

#### Scenario: 查询已存在隧道
- **WHEN** 客户端发送 `GET /svcmgr/api/cloudflare/tunnels/{id}` 请求
- **AND** 隧道 `{id}` 存在
- **THEN** 系统 **应当** 返回该隧道的完整配置信息
- **AND** HTTP 响应状态码 **应当** 为 `200 OK`

#### Scenario: 查询不存在隧道
- **WHEN** 客户端发送 `GET /svcmgr/api/cloudflare/tunnels/{id}` 请求
- **AND** 隧道 `{id}` 不存在
- **THEN** 系统 **应当** 返回错误响应
- **AND** 错误类型 **应当** 为 `NOT_FOUND`
- **AND** HTTP 响应状态码 **应当** 为 `404 Not Found`

---

### Requirement: 隧道创建
系统 **必须** 提供 REST API 端点用于创建新的 Cloudflare Tunnel 隧道。

#### Scenario: 创建合法隧道
- **WHEN** 客户端发送 `POST /svcmgr/api/cloudflare/tunnels` 请求
- **AND** 请求体包含必需字段 `name, domain, service_url`
- **AND** `domain` 格式合法（有效的域名）
- **AND** `service_url` 格式合法（有效的 URL）
- **THEN** 系统 **应当** 创建新的隧道配置文件
- **AND** 系统 **应当** 通过 Git 原子提交配置变更
- **AND** 系统 **应当** 返回创建成功的隧道对象
- **AND** HTTP 响应状态码 **应当** 为 `201 Created`

#### Scenario: 域名冲突
- **WHEN** 客户端创建隧道
- **AND** 请求的 `domain` 已被其他隧道占用
- **THEN** 系统 **应当** 返回错误响应
- **AND** 错误类型 **应当** 为 `CONFLICT`
- **AND** 错误消息 **应当** 说明域名冲突
- **AND** HTTP 响应状态码 **应当** 为 `409 Conflict`

#### Scenario: 必需字段缺失
- **WHEN** 客户端创建隧道
- **AND** 请求体缺少必需字段（`name`, `domain`, `service_url` 之一）
- **THEN** 系统 **应当** 返回错误响应
- **AND** 错误类型 **应当** 为 `VALIDATION_ERROR`
- **AND** HTTP 响应状态码 **应当** 为 `422 Unprocessable Entity`

---

### Requirement: 隧道更新
系统 **必须** 提供 REST API 端点用于更新已存在隧道的配置。

#### Scenario: 更新隧道配置
- **WHEN** 客户端发送 `PUT /svcmgr/api/cloudflare/tunnels/{id}` 请求
- **AND** 隧道 `{id}` 存在
- **AND** 请求体包含需要更新的字段
- **THEN** 系统 **应当** 更新隧道配置文件
- **AND** 系统 **应当** 通过 Git 原子提交配置变更
- **AND** 系统 **应当** 返回更新后的隧道对象
- **AND** HTTP 响应状态码 **应当** 为 `200 OK`

#### Scenario: 更新不存在隧道
- **WHEN** 客户端尝试更新不存在的隧道
- **THEN** 系统 **应当** 返回错误响应
- **AND** 错误类型 **应当** 为 `NOT_FOUND`
- **AND** HTTP 响应状态码 **应当** 为 `404 Not Found`

#### Scenario: 更新导致域名冲突
- **WHEN** 客户端更新隧道的 `domain` 字段
- **AND** 新域名已被其他隧道占用
- **THEN** 系统 **应当** 返回错误响应
- **AND** 错误类型 **应当** 为 `CONFLICT`
- **AND** HTTP 响应状态码 **应当** 为 `409 Conflict`

---

### Requirement: 隧道删除
系统 **必须** 提供 REST API 端点用于删除已存在的隧道配置。

#### Scenario: 删除隧道
- **WHEN** 客户端发送 `DELETE /svcmgr/api/cloudflare/tunnels/{id}` 请求
- **AND** 隧道 `{id}` 存在
- **THEN** 系统 **应当** 删除隧道配置文件
- **AND** 系统 **应当** 停止相关 cloudflared 进程（如果正在运行）
- **AND** 系统 **应当** 通过 Git 原子提交配置变更
- **AND** HTTP 响应状态码 **应当** 为 `204 No Content`

#### Scenario: 删除不存在隧道
- **WHEN** 客户端尝试删除不存在的隧道
- **THEN** 系统 **应当** 返回错误响应
- **AND** 错误类型 **应当** 为 `NOT_FOUND`
- **AND** HTTP 响应状态码 **应当** 为 `404 Not Found`

---

### Requirement: 状态监控
系统 **必须** 实时监控所有隧道的连接状态和运行时间。

#### Scenario: 状态检测
- **WHEN** 系统查询隧道列表或详情
- **THEN** 系统 **应当** 检测 cloudflared 进程状态
- **AND** 如果进程运行且连接正常，**应当** 设置状态为 `"connected"`
- **AND** 如果进程未运行或连接失败，**应当** 设置状态为 `"disconnected"`
- **AND** 如果连接不稳定或性能下降，**应当** 设置状态为 `"degraded"`
- **AND** 如果进程运行，**应当** 计算并返回 `uptime` 字段（格式如 "2h 34m"）

---

## REST API 接口规范

### 1. 获取所有隧道

#### `GET /svcmgr/api/cloudflare/tunnels`

**描述**: 获取所有 Cloudflare Tunnel 隧道配置及状态

**请求参数**: 无

**响应** (200):
```json
[
  {
    "id": "tunnel-001",
    "name": "my-web-app",
    "domain": "app.example.com",
    "service_url": "http://localhost:8080",
    "status": "connected",
    "uptime": "2h 34m"
  },
  {
    "id": "tunnel-002",
    "name": "api-backend",
    "domain": "api.example.com",
    "service_url": "http://localhost:3000",
    "status": "disconnected"
  }
]
```

**错误响应**:
- `500 INTERNAL_ERROR`: 系统内部错误

---

### 2. 获取隧道详情

#### `GET /svcmgr/api/cloudflare/tunnels/{id}`

**描述**: 获取指定隧道的详细配置信息

**路径参数**:
- `id` (string): 隧道 ID

**响应** (200):
```json
{
  "id": "tunnel-001",
  "name": "my-web-app",
  "domain": "app.example.com",
  "service_url": "http://localhost:8080",
  "status": "connected",
  "uptime": "2h 34m"
}
```

**错误响应**:
- `404 NOT_FOUND`: 隧道不存在
- `500 INTERNAL_ERROR`: 系统内部错误

---

### 3. 创建隧道

#### `POST /svcmgr/api/cloudflare/tunnels`

**描述**: 创建新的 Cloudflare Tunnel 隧道

**请求体**:
```json
{
  "name": "my-web-app",
  "domain": "app.example.com",
  "service_url": "http://localhost:8080"
}
```

**字段说明**:
- `name` (string, **必需**): 隧道名称（用于标识）
- `domain` (string, **必需**): 公开访问的域名（如 `app.example.com`）
- `service_url` (string, **必需**): 本地服务地址（如 `http://localhost:8080`）

**响应** (201):
```json
{
  "id": "tunnel-003",
  "name": "my-web-app",
  "domain": "app.example.com",
  "service_url": "http://localhost:8080",
  "status": "disconnected"
}
```

**错误响应**:
- `400 INVALID_REQUEST`: 请求格式错误
- `409 CONFLICT`: 域名已被占用
- `422 VALIDATION_ERROR`: 字段验证失败（缺少必需字段、格式错误）
- `500 INTERNAL_ERROR`: 创建失败

---

### 4. 更新隧道

#### `PUT /svcmgr/api/cloudflare/tunnels/{id}`

**描述**: 更新隧道配置（支持部分更新）

**路径参数**:
- `id` (string): 隧道 ID

**请求体** (支持部分字段):
```json
{
  "name": "updated-app-name",
  "domain": "new.example.com",
  "service_url": "http://localhost:9000"
}
```

**响应** (200):
```json
{
  "id": "tunnel-001",
  "name": "updated-app-name",
  "domain": "new.example.com",
  "service_url": "http://localhost:9000",
  "status": "connected",
  "uptime": "0m"
}
```

**错误响应**:
- `400 INVALID_REQUEST`: 请求格式错误
- `404 NOT_FOUND`: 隧道不存在
- `409 CONFLICT`: 域名冲突
- `422 VALIDATION_ERROR`: 字段验证失败
- `500 INTERNAL_ERROR`: 更新失败

---

### 5. 删除隧道

#### `DELETE /svcmgr/api/cloudflare/tunnels/{id}`

**描述**: 删除指定隧道（停止进程并删除配置）

**路径参数**:
- `id` (string): 隧道 ID

**响应** (204):
无响应体

**错误响应**:
- `404 NOT_FOUND`: 隧道不存在
- `500 INTERNAL_ERROR`: 删除失败

---

## Rust 数据类型定义

### 隧道对象

```rust
use serde::{Deserialize, Serialize};

/// Cloudflare Tunnel 隧道配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudflareTunnel {
    /// 隧道唯一标识符
    pub id: String,
    
    /// 隧道名称
    pub name: String,
    
    /// 公开访问域名 (如 app.example.com)
    pub domain: String,
    
    /// 本地服务地址 (如 http://localhost:8080)
    pub service_url: String,
    
    /// 隧道连接状态
    pub status: TunnelStatus,
    
    /// 运行时间 (格式如 "2h 34m")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uptime: Option<String>,
}

/// 隧道连接状态
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TunnelStatus {
    /// 已连接正常运行
    Connected,
    
    /// 未连接或已停止
    Disconnected,
    
    /// 连接不稳定或性能下降
    Degraded,
}
```

### 请求类型

```rust
/// 创建隧道请求
#[derive(Debug, Clone, Deserialize)]
pub struct CreateTunnelRequest {
    /// 隧道名称
    pub name: String,
    
    /// 公开访问域名
    pub domain: String,
    
    /// 本地服务地址
    pub service_url: String,
}

/// 更新隧道请求（所有字段可选）
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateTunnelRequest {
    /// 新的隧道名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    
    /// 新的公开访问域名
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    
    /// 新的本地服务地址
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_url: Option<String>,
}
```

### 验证逻辑

```rust
impl CreateTunnelRequest {
    /// 验证请求数据
    pub fn validate(&self) -> Result<(), ValidationError> {
        // 验证 name 非空
        if self.name.trim().is_empty() {
            return Err(ValidationError::EmptyField("name"));
        }
        
        // 验证 domain 格式
        if !is_valid_domain(&self.domain) {
            return Err(ValidationError::InvalidDomain(self.domain.clone()));
        }
        
        // 验证 service_url 格式
        if !is_valid_url(&self.service_url) {
            return Err(ValidationError::InvalidUrl(self.service_url.clone()));
        }
        
        Ok(())
    }
}

/// 验证域名格式
fn is_valid_domain(domain: &str) -> bool {
    // 简单验证：包含点且不含空格
    domain.contains('.') && !domain.contains(' ')
}

/// 验证 URL 格式
fn is_valid_url(url: &str) -> bool {
    // 简单验证：以 http:// 或 https:// 开头
    url.starts_with("http://") || url.starts_with("https://")
}
```

---

## Rust Trait 接口定义

```rust
use async_trait::async_trait;
use crate::error::ApiError;

/// Cloudflare Tunnel 管理特性
#[async_trait]
pub trait CloudflareFeature {
    /// 获取所有隧道配置
    async fn list_tunnels(&self) -> Result<Vec<CloudflareTunnel>, ApiError>;
    
    /// 获取指定隧道详情
    async fn get_tunnel(&self, id: &str) -> Result<CloudflareTunnel, ApiError>;
    
    /// 创建新隧道
    async fn create_tunnel(&self, request: CreateTunnelRequest) -> Result<CloudflareTunnel, ApiError>;
    
    /// 更新隧道配置
    async fn update_tunnel(&self, id: &str, request: UpdateTunnelRequest) -> Result<CloudflareTunnel, ApiError>;
    
    /// 删除隧道
    async fn delete_tunnel(&self, id: &str) -> Result<(), ApiError>;
}
```

### 实现说明

```rust
/// Cloudflare Tunnel 功能实现
pub struct CloudflareManager {
    config_dir: PathBuf,
    git_atom: Arc<dyn GitAtom>,
    template_engine: Arc<dyn TemplateEngine>,
}

#[async_trait]
impl CloudflareFeature for CloudflareManager {
    async fn list_tunnels(&self) -> Result<Vec<CloudflareTunnel>, ApiError> {
        // 1. 扫描配置目录 (~/.local/share/svcmgr/cloudflare/tunnels/)
        // 2. 解析每个隧道配置文件
        // 3. 检测 cloudflared 进程状态（使用 systemd --user status 或 ps）
        // 4. 计算运行时间
        // 5. 返回隧道列表
        todo!()
    }
    
    async fn get_tunnel(&self, id: &str) -> Result<CloudflareTunnel, ApiError> {
        // 1. 检查配置文件是否存在
        // 2. 解析隧道配置
        // 3. 检测进程状态和运行时间
        // 4. 返回隧道对象
        todo!()
    }
    
    async fn create_tunnel(&self, request: CreateTunnelRequest) -> Result<CloudflareTunnel, ApiError> {
        // 1. 验证请求数据
        // 2. 检查域名冲突
        // 3. 生成隧道 ID (UUID)
        // 4. 通过模板引擎生成配置文件
        // 5. 写入配置文件到 ~/.local/share/svcmgr/cloudflare/tunnels/{id}.yaml
        // 6. 通过 Git 原子提交配置
        // 7. 返回创建的隧道对象
        todo!()
    }
    
    async fn update_tunnel(&self, id: &str, request: UpdateTunnelRequest) -> Result<CloudflareTunnel, ApiError> {
        // 1. 检查隧道是否存在
        // 2. 验证更新数据
        // 3. 如果更新 domain，检查域名冲突
        // 4. 更新配置文件（保留未修改字段）
        // 5. 如果隧道正在运行，重启 cloudflared 进程
        // 6. 通过 Git 原子提交配置
        // 7. 返回更新后的隧道对象
        todo!()
    }
    
    async fn delete_tunnel(&self, id: &str) -> Result<(), ApiError> {
        // 1. 检查隧道是否存在
        // 2. 停止相关 cloudflared 进程（如果运行中）
        // 3. 删除配置文件
        // 4. 通过 Git 原子提交配置
        todo!()
    }
}
```

---

## 配置文件示例

### 隧道配置 (YAML)

**路径**: `~/.local/share/svcmgr/cloudflare/tunnels/{tunnel-id}.yaml`

```yaml
# Cloudflare Tunnel Configuration
# Generated by svcmgr at 2026-02-21T10:30:00Z

tunnel: my-tunnel-uuid
credentials-file: /home/user/.cloudflared/my-tunnel-uuid.json

ingress:
  - hostname: app.example.com
    service: http://localhost:8080
  - service: http_status:404
```

---

## 内置模板 (Jinja2)

### 隧道配置模板

**路径**: `src/templates/cloudflare/tunnel.yaml.j2`

```jinja2
# Cloudflare Tunnel Configuration
# Generated by svcmgr at {{ timestamp }}

tunnel: {{ tunnel_id }}
credentials-file: {{ credentials_path }}

ingress:
  - hostname: {{ domain }}
    service: {{ service_url }}
  - service: http_status:404
```

**模板变量**:
- `tunnel_id` (string): Cloudflare Tunnel UUID
- `credentials_path` (string): 凭证文件路径 (`~/.cloudflared/{tunnel_id}.json`)
- `domain` (string): 公开访问域名
- `service_url` (string): 本地服务地址
- `timestamp` (string): 生成时间戳（ISO 8601 格式）

---

## 错误码定义

```rust
#[derive(Debug, Serialize)]
#[serde(tag = "error", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CloudflareError {
    /// 隧道不存在
    NotFound { message: String },
    
    /// 域名冲突
    Conflict { message: String, conflicting_domain: String },
    
    /// 验证错误
    ValidationError { message: String, field: Option<String> },
    
    /// 内部错误
    InternalError { message: String },
}
```

---

## 实施检查清单

### Phase 1: 基础隧道管理
- [ ] 实现 `CloudflareFeature` trait
- [ ] 实现隧道配置文件扫描和解析
- [ ] 实现隧道 CRUD 操作
- [ ] 集成 Git 原子提交配置变更
- [ ] 实现模板驱动的配置文件生成

### Phase 2: 状态监控
- [ ] 实现 cloudflared 进程状态检测
- [ ] 实现运行时间计算
- [ ] 实现状态枚举逻辑 (connected/disconnected/degraded)

### Phase 3: 高级功能
- [ ] 支持域名格式验证
- [ ] 支持 URL 格式验证
- [ ] 支持域名冲突检测
- [ ] 支持隧道配置热重载（更新时自动重启进程）

### Phase 4: 测试
- [ ] 单元测试：隧道 CRUD 操作
- [ ] 单元测试：请求验证逻辑
- [ ] 单元测试：域名冲突检测
- [ ] 集成测试：Git 版本管理集成
- [ ] 集成测试：模板引擎集成
- [ ] 端到端测试：完整隧道生命周期

---

## 相关文档

- [API 设计规范](./20-api-design.md)
- [Git 配置版本原子 (A01)](./11-atom-git-config.md)
- [模板引擎原子 (A02)](./12-atom-template-engine.md)
- [前端 UI 设计](./30-frontend-ui.md)
