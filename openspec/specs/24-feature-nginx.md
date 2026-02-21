# F04: Nginx 代理管理

> 版本：1.0.0  
> 状态：DRAFT  
> 依赖原子：T02 (模板), T09 (proxy)  
> 最后更新：2026-02-21

## 概述

Nginx 代理管理功能提供对 nginx 反向代理配置的完整管理,包括 HTTP 代理、TCP 代理和静态文件服务。支持创建、修改、删除代理规则,以及连接测试。

---

## ADDED Requirements

### Requirement: 代理列表查询
系统 **MUST** 支持查询所有 nginx 代理配置。

#### Scenario: 获取代理列表
- **WHEN** 用户请求代理列表
- **THEN** 系统 **SHALL** 解析 nginx 配置文件
- **AND** 每个代理 **SHALL** 包含：ID、路径、代理类型、目标地址、状态、是否内置

#### Scenario: 内置代理保护
- **WHEN** 显示代理列表
- **THEN** 系统 **SHALL** 标记内置代理（built_in = true）
- **AND** 内置代理包括：`/svcmgr` (API), `/tty` (Web TTY)
- **AND** 内置代理 **MUST NOT** 允许删除

---

### Requirement: 代理类型支持
系统 **MUST** 支持三种代理类型。

#### Scenario: HTTP 代理
- **WHEN** 代理类型为 `http`
- **THEN** 系统 **SHALL** 生成 `proxy_pass` 配置
- **AND** 目标格式为 `http://host:port`

#### Scenario: TCP 代理
- **WHEN** 代理类型为 `tcp`
- **THEN** 系统 **SHALL** 生成 `stream` 块配置
- **AND** 监听指定端口并转发到目标

#### Scenario: 静态文件服务
- **WHEN** 代理类型为 `static`
- **THEN** 系统 **SHALL** 生成 `root` 配置
- **AND** 目标为本地文件系统路径

---

### Requirement: 代理创建
系统 **MUST** 支持创建新的代理规则。

#### Scenario: 创建代理
- **WHEN** 用户创建新代理
- **THEN** 系统 **SHALL**：
  1. 使用 **T02** 渲染 nginx 配置模板
  2. 写入配置文件到 nginx 配置目录
  3. 执行 `nginx -t` 验证配置
  4. 执行 `nginx -s reload` 重载配置
- **AND** 支持配置：
  - `path`: URL 路径（必需）
  - `proxy_type`: 代理类型（http/tcp/static，必需）
  - `target`: 目标地址或路径（必需）
  - `root`: 静态文件根目录（static 类型时）
  - `port`: 监听端口（tcp 类型时）

---

### Requirement: 代理更新
系统 **MUST** 支持修改现有代理规则。

#### Scenario: 更新代理
- **WHEN** 用户修改代理
- **THEN** 系统 **SHALL**：
  1. 使用 **T02** 重新渲染配置
  2. 覆写配置文件
  3. 验证并重载 nginx

---

### Requirement: 代理删除
系统 **MUST** 支持删除代理规则。

#### Scenario: 删除代理
- **WHEN** 用户删除代理
- **THEN** 系统 **SHALL**：
  1. 检查是否为内置代理（禁止删除）
  2. 删除配置文件
  3. 重载 nginx
- **AND** 返回错误如果是内置代理

---

### Requirement: 连接测试
系统 **SHOULD** 支持测试代理连接。

#### Scenario: 测试 HTTP 代理
- **WHEN** 用户测试代理
- **THEN** 系统 **SHALL** 发送 HTTP 请求到代理路径
- **AND** 返回响应状态码和响应时间

---

## API 端点

### 获取代理列表

#### `GET /svcmgr/api/nginx/proxies`

**描述**: 获取所有 nginx 代理配置列表

**请求参数**:
- Query (可选):
  - `filter`: 过滤条件（例如 `proxy_type:http`, `status:active`）
  - `sort`: 排序字段（例如 `path`, `-status`）

**响应** (200 OK):
```json
[
  {
    "id": "sys-1",
    "path": "/svcmgr",
    "proxy_type": "http",
    "target": "http://127.0.0.1:8080",
    "status": "active",
    "built_in": true
  },
  {
    "id": "sys-2",
    "path": "/tty",
    "proxy_type": "http",
    "target": "http://127.0.0.1:7681",
    "status": "active",
    "built_in": true
  },
  {
    "id": "1",
    "path": "/",
    "proxy_type": "static",
    "target": "/var/www/html",
    "status": "active",
    "root": "/var/www/html"
  },
  {
    "id": "2",
    "path": "/api",
    "proxy_type": "http",
    "target": "http://127.0.0.1:3000",
    "status": "active"
  }
]
```

**错误响应**:
- `500 INTERNAL_ERROR`: 无法读取 nginx 配置

---

### 获取代理详情

#### `GET /svcmgr/api/nginx/proxies/{id}`

**描述**: 获取单个代理的详细信息

**路径参数**:
- `id`: 代理 ID

**响应** (200 OK):
```json
{
  "id": "2",
  "path": "/api",
  "proxy_type": "http",
  "target": "http://127.0.0.1:3000",
  "status": "active"
}
```

**错误响应**:
- `404 RESOURCE_NOT_FOUND`: 代理不存在
- `500 INTERNAL_ERROR`: 查询失败

---

### 创建代理

#### `POST /svcmgr/api/nginx/proxies`

**描述**: 创建新的 nginx 代理规则

**请求体** (HTTP 代理):
```json
{
  "path": "/backend",
  "proxy_type": "http",
  "target": "http://127.0.0.1:4000"
}
```

**请求体** (静态文件):
```json
{
  "path": "/static",
  "proxy_type": "static",
  "target": "/var/www/static",
  "root": "/var/www/static"
}
```

**请求体** (TCP 代理):
```json
{
  "path": "/ws",
  "proxy_type": "tcp",
  "target": "127.0.0.1:9090",
  "port": 9090
}
```

**响应** (201 Created):
```json
{
  "id": "3",
  "path": "/backend",
  "proxy_type": "http",
  "target": "http://127.0.0.1:4000",
  "status": "active"
}
```

**错误响应**:
- `400 VALIDATION_ERROR`: 参数验证失败
- `409 RESOURCE_CONFLICT`: 路径冲突
- `422 UNPROCESSABLE_ENTITY`: nginx 配置验证失败
- `500 INTERNAL_ERROR`: 创建失败

---

### 更新代理

#### `PUT /svcmgr/api/nginx/proxies/{id}`

**描述**: 更新现有代理配置

**路径参数**:
- `id`: 代理 ID

**请求体** (部分更新):
```json
{
  "target": "http://127.0.0.1:5000"
}
```

**响应** (200 OK):
```json
{
  "id": "3",
  "path": "/backend",
  "proxy_type": "http",
  "target": "http://127.0.0.1:5000",
  "status": "active"
}
```

**错误响应**:
- `404 RESOURCE_NOT_FOUND`: 代理不存在
- `400 VALIDATION_ERROR`: 参数验证失败
- `422 UNPROCESSABLE_ENTITY`: nginx 配置验证失败
- `500 INTERNAL_ERROR`: 更新失败

---

### 删除代理

#### `DELETE /svcmgr/api/nginx/proxies/{id}`

**描述**: 删除代理规则

**路径参数**:
- `id`: 代理 ID

**响应** (204 No Content)

**错误响应**:
- `404 RESOURCE_NOT_FOUND`: 代理不存在
- `400 VALIDATION_ERROR`: 不能删除内置代理
- `500 INTERNAL_ERROR`: 删除失败

---

### 测试代理连接

#### `POST /svcmgr/api/nginx/proxies/{id}/test`

**描述**: 测试代理连接

**路径参数**:
- `id`: 代理 ID

**响应** (200 OK):
```json
{
  "status": 200,
  "time": 42
}
```

**错误响应**:
- `404 RESOURCE_NOT_FOUND`: 代理不存在
- `500 OPERATION_FAILED`: 测试失败

---

## 数据模型

### Rust 类型定义

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NginxProxy {
    /// 代理 ID（唯一标识符）
    pub id: String,
    
    /// URL 路径（例如 /api, /static）
    pub path: String,
    
    /// 代理类型
    pub proxy_type: ProxyType,
    
    /// 目标地址或路径
    pub target: String,
    
    /// 代理状态
    pub status: ProxyStatus,
    
    /// 静态文件根目录（仅 static 类型）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    
    /// 监听端口（仅 tcp 类型）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    
    /// 是否为内置代理（不可删除）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub built_in: Option<bool>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProxyType {
    Static,
    Http,
    Tcp,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProxyStatus {
    Active,
    Inactive,
    Error,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateProxyRequest {
    pub path: String,
    pub proxy_type: ProxyType,
    pub target: String,
    #[serde(default)]
    pub root: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateProxyRequest {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub proxy_type: Option<ProxyType>,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub root: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProxyTestResult {
    pub status: u16,
    pub time: u64,
}
```

---

## 接口定义

```rust
use async_trait::async_trait;

#[async_trait]
pub trait NginxFeature {
    /// 获取所有代理列表
    async fn list_proxies(&self) -> Result<Vec<NginxProxy>, ApiError>;
    
    /// 获取单个代理详情
    async fn get_proxy(&self, id: &str) -> Result<NginxProxy, ApiError>;
    
    /// 创建代理
    async fn create_proxy(&self, request: CreateProxyRequest) -> Result<NginxProxy, ApiError>;
    
    /// 更新代理
    async fn update_proxy(&self, id: &str, request: UpdateProxyRequest) -> Result<NginxProxy, ApiError>;
    
    /// 删除代理
    async fn delete_proxy(&self, id: &str) -> Result<(), ApiError>;
    
    /// 测试代理连接
    async fn test_proxy(&self, id: &str) -> Result<ProxyTestResult, ApiError>;
}
```

---

## 配置项

```toml
[nginx]
# nginx 配置文件目录
config_dir = "~/.config/nginx"

# nginx 主配置文件
main_config = "~/.config/nginx/nginx.conf"

# 代理配置目录
proxy_config_dir = "~/.config/nginx/conf.d"

# 内置代理（不可删除）
[[nginx.built_in_proxies]]
path = "/svcmgr"
target = "http://127.0.0.1:8080"

[[nginx.built_in_proxies]]
path = "/tty"
target = "http://127.0.0.1:7681"
```

---

## 内置模板

### nginx-http-proxy.conf.j2

```jinja2
# {{ path }} - HTTP Proxy
location {{ path }} {
    proxy_pass {{ target }};
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
}
```

### nginx-static.conf.j2

```jinja2
# {{ path }} - Static Files
location {{ path }} {
    root {{ root }};
    index index.html index.htm;
    try_files $uri $uri/ =404;
}
```

### nginx-tcp-proxy.conf.j2

```jinja2
# {{ path }} - TCP Proxy
stream {
    upstream {{ upstream_name }} {
        server {{ target }};
    }
    
    server {
        listen {{ port }};
        proxy_pass {{ upstream_name }};
    }
}
```

---

## 注意事项

1. **内置代理保护**: `/svcmgr` 和 `/tty` 为系统内置代理,不可删除
2. **配置验证**: 每次修改后必须执行 `nginx -t` 验证
3. **原子性重载**: 配置验证通过后才执行 `nginx -s reload`
4. **路径冲突检测**: 创建前检查路径是否已存在
5. **WebSocket 支持**: HTTP 代理自动支持 WebSocket 升级

---

**变更历史**：
- 2026-02-21: 初始版本,基于前端原型提取
