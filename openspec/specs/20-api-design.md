# API 总体设计规范

> 版本：1.0.0  
> 状态：DRAFT  
> 最后更新：2026-02-21

## 概述

本文档定义 svcmgr REST API 的整体设计规范，包括基础路径、响应格式、状态码约定、通用参数、错误处理等。所有功能模块的具体 API 端点设计必须遵循本规范。

---

## ADDED Requirements

### Requirement: API Base Path
系统 **MUST** 使用统一的 API 基础路径。

#### Scenario: Base URL
- **WHEN** 客户端访问 API
- **THEN** 所有 API 端点 **SHALL** 使用基础路径 `/svcmgr/api`
- **AND** 完整 URL 格式为：`http://localhost:8080/svcmgr/api/{module}/{resource}`

**示例**：
```
http://localhost:8080/svcmgr/api/systemd/services
http://localhost:8080/svcmgr/api/crontab/tasks
http://localhost:8080/svcmgr/api/dashboard/stats
```

---

### Requirement: HTTP Methods 语义
系统 **MUST** 按照 RESTful 约定使用 HTTP 方法。

#### Scenario: CRUD 操作映射
- **WHEN** 执行资源操作
- **THEN** 系统 **SHALL** 按以下映射：
  - `GET /resources` - 获取资源列表
  - `GET /resources/{id}` - 获取单个资源详情
  - `POST /resources` - 创建新资源
  - `PUT /resources/{id}` - 更新整个资源
  - `PATCH /resources/{id}` - 部分更新资源
  - `DELETE /resources/{id}` - 删除资源

#### Scenario: Action 操作
- **WHEN** 执行非 CRUD 操作（启动、停止、重启等）
- **THEN** 系统 **SHALL** 使用：
  - `POST /resources/{id}/{action}` - 执行资源动作
- **AND** action **MUST** 使用动词，例如：`start`, `stop`, `restart`, `toggle`, `test`, `run`

**示例**：
```
POST /svcmgr/api/systemd/services/nginx.service/start
POST /svcmgr/api/systemd/services/nginx.service/stop
POST /svcmgr/api/systemd/services/nginx.service/restart
POST /svcmgr/api/crontab/tasks/task-123/toggle
POST /svcmgr/api/nginx/proxies/proxy-456/test
```

---

### Requirement: 响应格式
系统 **MUST** 返回一致的 JSON 响应格式。

#### Scenario: 成功响应 - 单个资源
- **WHEN** 请求成功返回单个资源
- **THEN** 响应 **SHALL** 直接返回资源对象（不包装）

**示例**：
```json
// GET /svcmgr/api/systemd/services/nginx.service
{
  "name": "nginx.service",
  "status": "running",
  "enabled": true,
  "pid": 1234,
  "memory": "12.4 MB",
  "uptime": "3d 4h",
  "description": "Nginx HTTP Server"
}
```

#### Scenario: 成功响应 - 资源列表
- **WHEN** 请求成功返回资源列表
- **THEN** 响应 **SHALL** 直接返回数组（不包装）

**示例**：
```json
// GET /svcmgr/api/systemd/services
[
  {
    "name": "nginx.service",
    "status": "running",
    "enabled": true,
    ...
  },
  {
    "name": "redis.service",
    "status": "stopped",
    "enabled": false,
    ...
  }
]
```

#### Scenario: 成功响应 - 无内容
- **WHEN** 操作成功但无返回内容（如删除、启动服务）
- **THEN** 响应 **SHALL** 返回：
  - HTTP 状态码：`204 No Content`
  - 响应体：空

#### Scenario: 成功响应 - 操作结果
- **WHEN** 操作成功且需要返回简单结果（如测试连接）
- **THEN** 响应 **SHALL** 返回结果对象

**示例**：
```json
// POST /svcmgr/api/nginx/proxies/proxy-123/test
{
  "status": 200,
  "time": 42
}
```

---

### Requirement: 错误响应
系统 **MUST** 返回标准化的错误响应格式。

#### Scenario: 错误响应格式
- **WHEN** 请求失败
- **THEN** 响应 **SHALL** 返回 JSON 对象包含：
  - `error`: 错误类型（字符串，大写下划线格式）
  - `message`: 用户可读的错误描述
  - `details` (可选): 额外的错误详情对象

**示例**：
```json
{
  "error": "RESOURCE_NOT_FOUND",
  "message": "Service 'nginx.service' not found"
}
```

```json
{
  "error": "VALIDATION_ERROR",
  "message": "Invalid cron expression",
  "details": {
    "field": "expression",
    "value": "invalid cron",
    "reason": "Expected 5 fields, got 2"
  }
}
```

---

### Requirement: HTTP 状态码
系统 **MUST** 使用标准 HTTP 状态码。

#### Scenario: 状态码映射
- **WHEN** 返回响应
- **THEN** 系统 **SHALL** 使用以下状态码：

**2xx 成功**：
- `200 OK` - 成功返回资源/结果
- `201 Created` - 成功创建资源（返回新资源）
- `204 No Content` - 成功执行操作（无返回内容）

**4xx 客户端错误**：
- `400 Bad Request` - 请求参数错误、验证失败
- `404 Not Found` - 资源不存在
- `409 Conflict` - 资源冲突（如重复创建）
- `422 Unprocessable Entity` - 语义错误（如无效配置）

**5xx 服务器错误**：
- `500 Internal Server Error` - 服务器内部错误
- `503 Service Unavailable` - 依赖服务不可用

---

### Requirement: 请求参数
系统 **SHOULD** 支持通用查询参数。

#### Scenario: 列表过滤
- **WHEN** 获取资源列表
- **THEN** 系统 **SHOULD** 支持以下查询参数：
  - `filter` - 过滤条件（格式：`field:value`，多个用逗号分隔）
  - `sort` - 排序字段（格式：`field` 升序，`-field` 降序）
  - `limit` - 返回数量限制（默认 100）
  - `offset` - 分页偏移量（默认 0）

**示例**：
```
GET /svcmgr/api/systemd/services?filter=status:running&sort=-memory&limit=10
GET /svcmgr/api/crontab/tasks?filter=enabled:true&sort=expression
```

#### Scenario: 字段选择
- **WHEN** 获取资源
- **THEN** 系统 **MAY** 支持 `fields` 参数选择返回字段

**示例**：
```
GET /svcmgr/api/systemd/services?fields=name,status,enabled
```

---

### Requirement: Content-Type
系统 **MUST** 支持 JSON 请求和响应。

#### Scenario: Request Content-Type
- **WHEN** 发送请求体（POST/PUT/PATCH）
- **THEN** 客户端 **SHALL** 设置 `Content-Type: application/json`
- **AND** 服务器 **SHALL** 拒绝非 JSON 请求（返回 `400 Bad Request`）

#### Scenario: Response Content-Type
- **WHEN** 返回响应
- **THEN** 服务器 **SHALL** 设置 `Content-Type: application/json; charset=utf-8`

---

### Requirement: 错误码定义
系统 **MUST** 定义标准错误码。

#### Scenario: 通用错误码
- **WHEN** 发生错误
- **THEN** 系统 **SHALL** 使用以下错误码：

**资源错误**：
- `RESOURCE_NOT_FOUND` - 资源不存在
- `RESOURCE_CONFLICT` - 资源冲突（如重名）
- `RESOURCE_LOCKED` - 资源被锁定

**验证错误**：
- `VALIDATION_ERROR` - 参数验证失败
- `INVALID_FORMAT` - 格式错误（如 cron 表达式、JSON 格式）
- `MISSING_FIELD` - 缺少必需字段

**权限错误**：
- `PERMISSION_DENIED` - 权限不足
- `UNAUTHORIZED` - 未认证

**操作错误**：
- `OPERATION_FAILED` - 操作执行失败
- `EXTERNAL_TOOL_ERROR` - 外部工具错误（systemctl、nginx 等）
- `TIMEOUT` - 操作超时

**系统错误**：
- `INTERNAL_ERROR` - 内部服务器错误
- `SERVICE_UNAVAILABLE` - 服务不可用

---

### Requirement: API 版本控制
系统 **SHOULD** 支持 API 版本管理。

#### Scenario: 版本号
- **WHEN** 部署新版本 API
- **THEN** 系统 **MAY** 在路径中包含版本号：`/svcmgr/api/v2/{module}/{resource}`
- **AND** 默认版本（v1）**SHALL** 省略版本号：`/svcmgr/api/{module}/{resource}`

---

### Requirement: CORS 支持
系统 **MUST** 支持跨域资源共享。

#### Scenario: CORS Headers
- **WHEN** 接收跨域请求
- **THEN** 系统 **SHALL** 返回：
  - `Access-Control-Allow-Origin: *` (或配置的具体域名)
  - `Access-Control-Allow-Methods: GET, POST, PUT, PATCH, DELETE, OPTIONS`
  - `Access-Control-Allow-Headers: Content-Type, Authorization`

---

### Requirement: WebSocket 支持
系统 **MUST** 支持 WebSocket 连接（用于 TTY、日志流等）。

#### Scenario: WebSocket Upgrade
- **WHEN** 客户端请求升级到 WebSocket
- **THEN** 服务器 **SHALL** 支持 WebSocket 握手
- **AND** WebSocket 路径 **SHALL** 使用独立前缀（如 `/tty/{session-id}`，而非 API 路径）

---

## 模块划分

API 按功能模块划分，每个模块有独立的规格文档：

| 模块 | 路径前缀 | 规格文档 | 说明 |
|------|---------|---------|------|
| Dashboard | `/svcmgr/api/dashboard` | `28-feature-dashboard.md` | 统计概览、活动日志 |
| Systemd | `/svcmgr/api/systemd` | `21-feature-systemd.md` | systemd 服务管理 |
| Crontab | `/svcmgr/api/crontab` | `22-feature-crontab.md` | crontab 任务管理 |
| Mise | `/svcmgr/api/mise` | `23-feature-mise.md` | mise 依赖和任务管理 |
| Nginx | `/svcmgr/api/nginx` | `24-feature-nginx.md` | nginx 代理管理 |
| Cloudflare | `/svcmgr/api/cloudflare` | `25-feature-tunnel.md` | Cloudflare 隧道管理 |
| TTY | `/svcmgr/api/tty` | `26-feature-tty.md` | TTY 会话管理 |
| Config | `/svcmgr/api/config` | `27-feature-config.md` | 配置文件版本管理 |
| Settings | `/svcmgr/api/settings` | `29-feature-settings.md` | 系统设置 |
| Activity | `/svcmgr/api/activity` | `28-feature-dashboard.md` | 活动日志 |

---

## 技术实现

### Rust 实现框架

```rust
use axum::{
    Router,
    http::{StatusCode, Method},
    response::{IntoResponse, Json},
    extract::{Path, Query},
};
use serde::{Deserialize, Serialize};
use tower_http::cors::{CorsLayer, Any};

// ─── 通用响应类型 ───────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = match self.error.as_str() {
            "RESOURCE_NOT_FOUND" => StatusCode::NOT_FOUND,
            "VALIDATION_ERROR" | "INVALID_FORMAT" | "MISSING_FIELD" => StatusCode::BAD_REQUEST,
            "RESOURCE_CONFLICT" => StatusCode::CONFLICT,
            "PERMISSION_DENIED" => StatusCode::FORBIDDEN,
            "UNAUTHORIZED" => StatusCode::UNAUTHORIZED,
            "SERVICE_UNAVAILABLE" => StatusCode::SERVICE_UNAVAILABLE,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(self)).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

// ─── 通用查询参数 ───────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    #[serde(default)]
    pub filter: Option<String>,
    #[serde(default)]
    pub sort: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

fn default_limit() -> usize { 100 }

// ─── 路由构建 ───────────────────────────────────────

pub fn build_router() -> Router {
    Router::new()
        .nest("/svcmgr/api/dashboard", dashboard::routes())
        .nest("/svcmgr/api/systemd", systemd::routes())
        .nest("/svcmgr/api/crontab", crontab::routes())
        .nest("/svcmgr/api/mise", mise::routes())
        .nest("/svcmgr/api/nginx", nginx::routes())
        .nest("/svcmgr/api/cloudflare", cloudflare::routes())
        .nest("/svcmgr/api/tty", tty::routes())
        .nest("/svcmgr/api/config", config::routes())
        .nest("/svcmgr/api/settings", settings::routes())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([Method::GET, Method::POST, Method::PUT, Method::PATCH, Method::DELETE])
                .allow_headers(Any)
        )
}
```

---

## 配置

```toml
[api]
# 监听地址
host = "127.0.0.1"
port = 8080

# Base path
base_path = "/svcmgr/api"

# CORS 配置
[api.cors]
allow_origins = ["*"]
allow_methods = ["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"]
allow_headers = ["Content-Type", "Authorization"]

# 速率限制
[api.rate_limit]
enabled = false
requests_per_minute = 60

# 日志
[api.logging]
level = "info"
format = "json"
```

---

## 测试示例

### 成功响应测试
```bash
# 获取服务列表
curl http://localhost:8080/svcmgr/api/systemd/services
# 返回：200 OK + JSON 数组

# 获取单个服务
curl http://localhost:8080/svcmgr/api/systemd/services/nginx.service
# 返回：200 OK + JSON 对象

# 创建资源
curl -X POST http://localhost:8080/svcmgr/api/crontab/tasks \
  -H "Content-Type: application/json" \
  -d '{"expression":"0 2 * * *","command":"backup.sh"}'
# 返回：201 Created + JSON 对象

# 删除资源
curl -X DELETE http://localhost:8080/svcmgr/api/crontab/tasks/task-123
# 返回：204 No Content

# 执行操作
curl -X POST http://localhost:8080/svcmgr/api/systemd/services/nginx.service/restart
# 返回：204 No Content
```

### 错误响应测试
```bash
# 404 Not Found
curl http://localhost:8080/svcmgr/api/systemd/services/nonexistent.service
# 返回：404 + {"error":"RESOURCE_NOT_FOUND","message":"..."}

# 400 Bad Request
curl -X POST http://localhost:8080/svcmgr/api/crontab/tasks \
  -H "Content-Type: application/json" \
  -d '{"expression":"invalid"}'
# 返回：400 + {"error":"VALIDATION_ERROR","message":"...","details":{...}}
```

---

## 注意事项

1. **无包装对象**：直接返回资源对象或数组，避免 `{"data": ...}` 包装
2. **一致的错误格式**：所有错误响应使用统一的 `ApiError` 结构
3. **RESTful 约定**：严格遵循 HTTP 方法语义
4. **操作使用 POST**：非 CRUD 操作（start/stop/restart）使用 `POST /{id}/{action}`
5. **状态码语义化**：根据错误类型返回正确的 HTTP 状态码

---

## 参考资料

- [REST API Best Practices](https://restfulapi.net/)
- [HTTP Status Codes](https://httpstatuses.com/)
- [Axum Framework](https://docs.rs/axum/)
- [JSON API Specification](https://jsonapi.org/)

---

**变更历史**：
- 2026-02-21: 初始版本，基于前端原型 `.temp/` 提取设计
