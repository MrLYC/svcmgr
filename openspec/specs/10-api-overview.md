# API 设计总览

## Design Goal

定义 svcmgr 的 REST API 设计原则、认证机制、版本策略和通用规范，为所有具体 API 端点提供统一的设计基础。

## Why

统一的 API 设计原则确保：
- **一致性**：所有端点遵循相同的命名、响应格式和错误处理规范
- **可预测性**：开发者能够根据已知端点推断未知端点的行为
- **可扩展性**：新增 API 端点时不会破坏现有约定
- **安全性**：认证、授权、速率限制等安全措施在所有端点统一实施
- **可维护性**：清晰的版本策略支持 API 演进而不破坏兼容性

## Core API Principles

### 1. RESTful 设计

遵循 REST 架构风格，使用标准 HTTP 方法表达操作语义：

```http
# 资源集合操作
GET    /api/v1/services          # 列出所有服务
POST   /api/v1/services          # 创建新服务

# 单个资源操作
GET    /api/v1/services/{name}   # 获取服务详情
PUT    /api/v1/services/{name}   # 完整更新服务
PATCH  /api/v1/services/{name}   # 部分更新服务
DELETE /api/v1/services/{name}   # 删除服务

# 子资源操作
POST   /api/v1/services/{name}/start   # 启动服务（操作类端点）
GET    /api/v1/services/{name}/logs    # 获取服务日志（子资源）
```

**HTTP 方法语义**：
- `GET` - 幂等只读操作，不改变系统状态
- `POST` - 非幂等创建或操作，可能改变系统状态
- `PUT` - 幂等完整替换，需提供完整资源表示
- `PATCH` - 幂等部分更新，仅提供变更字段
- `DELETE` - 幂等删除操作

### 2. 资源命名规范

- **使用复数名词**：`/services` 而非 `/service`
- **小写字母 + 连字符**：`/scheduled-tasks` 而非 `/scheduledTasks`
- **层级关系明确**：`/services/{name}/logs` 表达"服务的日志"
- **避免动词**：用 HTTP 方法表达动作，资源名应为名词

**例外**：操作类端点（非 CRUD）可使用动词：
```http
POST /api/v1/services/{name}/restart   # 重启服务
POST /api/v1/tasks/{name}/cancel       # 取消任务
POST /api/v1/config/rollback           # 回滚配置
```

### 3. 版本策略

**URL 路径版本控制**：
```http
GET /api/v1/services          # v1 API
GET /api/v2/services          # v2 API（未来）
```

**版本生命周期**：
- **v1（当前版本）**：稳定支持，向后兼容变更允许
- **v2（未来）**：引入破坏性变更时启用，与 v1 并存至少 6 个月
- **弃用策略**：通过响应头 `Deprecated: true` 和 `Sunset: 2026-12-31` 通知客户端

**向后兼容变更**（v1 内允许）：
- 添加新的可选字段
- 添加新的端点
- 放宽验证规则

**破坏性变更**（需升级到 v2）：
- 删除或重命名字段
- 改变字段类型或语义
- 删除端点
- 收紧验证规则

## Authentication & Authorization

### 1. 认证机制

**本地模式（默认）**：
```http
# Unix Socket 连接（本地用户隐式认证）
curl --unix-socket /run/svcmgr/api.sock http://localhost/api/v1/services
```

**网络模式（可选）**：
```http
# Bearer Token 认证
GET /api/v1/services
Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...
```

**Token 管理**：
```bash
# 生成 API Token（需要本地访问权限）
svcmgr token create --name "ci-pipeline" --expires 30d

# 撤销 Token
svcmgr token revoke <token-id>

# 列出所有 Token
svcmgr token list
```

**Token 格式**：
```rust
// JWT Token Claims
#[derive(Serialize, Deserialize)]
struct TokenClaims {
    /// Token 唯一标识
    jti: String,
    /// 签发时间（Unix timestamp）
    iat: i64,
    /// 过期时间（Unix timestamp）
    exp: i64,
    /// Token 名称（用户指定）
    name: String,
    /// 权限范围（可选）
    scopes: Option<Vec<String>>,
}

// Token 存储（加密保存在 ~/.config/mise/svcmgr/tokens.db）
struct TokenStore {
    tokens: HashMap<String, StoredToken>,
}

#[derive(Serialize, Deserialize)]
struct StoredToken {
    id: String,
    name: String,
    created_at: i64,
    expires_at: i64,
    last_used: Option<i64>,
    scopes: Vec<String>,
}
```

### 2. 权限模型

**当前版本（v1）**：简化权限模型，所有 API 操作需要完整权限。

**未来版本（v2）**：基于 Scope 的细粒度权限控制：
```yaml
# Scope 示例（未来功能）
scopes:
  - services:read       # 读取服务状态
  - services:write      # 启动/停止/重启服务
  - config:read         # 读取配置
  - config:write        # 修改配置
  - tasks:execute       # 执行任务
  - admin               # 完整管理权限
```

### 3. 安全措施

**速率限制**：
```toml
# .config/mise/svcmgr/config.toml
[api.rate_limit]
# 每分钟最大请求数（默认 60）
requests_per_minute = 60

# 突发流量允许的额外请求数（默认 10）
burst_size = 10

# 超限后的响应
# "reject" - 立即返回 429 Too Many Requests
# "queue" - 排队等待（最多等待 5 秒）
overflow_strategy = "reject"
```

**CORS 配置**（仅网络模式）：
```toml
[api.cors]
# 允许的来源（默认禁止跨域）
allowed_origins = ["http://localhost:3000", "https://admin.example.com"]

# 允许的 HTTP 方法
allowed_methods = ["GET", "POST", "PUT", "PATCH", "DELETE"]

# 允许的请求头
allowed_headers = ["Authorization", "Content-Type"]

# 预检请求缓存时间（秒）
max_age = 3600
```

**TLS 配置**（仅网络模式）：
```toml
[api.tls]
# 启用 TLS（默认 false）
enabled = true

# 证书文件路径
cert = "/etc/svcmgr/tls/cert.pem"
key = "/etc/svcmgr/tls/key.pem"

# 客户端证书验证（可选）
client_ca = "/etc/svcmgr/tls/ca.pem"
require_client_cert = false
```

## Response Formats

### 1. 成功响应

**单个资源**：
```json
{
  "data": {
    "name": "web-server",
    "status": "running",
    "pid": 12345,
    "uptime": 3600
  }
}
```

**资源集合**：
```json
{
  "data": [
    {
      "name": "web-server",
      "status": "running"
    },
    {
      "name": "background-worker",
      "status": "stopped"
    }
  ],
  "pagination": {
    "page": 1,
    "per_page": 20,
    "total": 42,
    "total_pages": 3
  }
}
```

**操作结果**（无返回数据）：
```json
{
  "message": "Service 'web-server' started successfully"
}
```

### 2. 错误响应

**统一错误格式**：
```json
{
  "error": {
    "code": "SERVICE_NOT_FOUND",
    "message": "Service 'invalid-name' does not exist",
    "details": {
      "service": "invalid-name",
      "available_services": ["web-server", "background-worker"]
    },
    "request_id": "req_7f8a9b2c"
  }
}
```

**错误代码分类**：
```rust
/// API 错误代码（遵循 UPPER_SNAKE_CASE 命名）
#[derive(Serialize, Deserialize, Debug)]
pub enum ApiErrorCode {
    // 4xx 客户端错误
    #[serde(rename = "INVALID_REQUEST")]
    InvalidRequest,              // 400 - 请求格式错误
    
    #[serde(rename = "UNAUTHORIZED")]
    Unauthorized,                // 401 - 缺少或无效的认证凭证
    
    #[serde(rename = "FORBIDDEN")]
    Forbidden,                   // 403 - 认证成功但权限不足
    
    #[serde(rename = "RESOURCE_NOT_FOUND")]
    ResourceNotFound,            // 404 - 资源不存在
    
    #[serde(rename = "METHOD_NOT_ALLOWED")]
    MethodNotAllowed,            // 405 - HTTP 方法不支持
    
    #[serde(rename = "CONFLICT")]
    Conflict,                    // 409 - 资源状态冲突
    
    #[serde(rename = "VALIDATION_FAILED")]
    ValidationFailed,            // 422 - 请求数据验证失败
    
    #[serde(rename = "RATE_LIMIT_EXCEEDED")]
    RateLimitExceeded,           // 429 - 超出速率限制
    
    // 5xx 服务器错误
    #[serde(rename = "INTERNAL_ERROR")]
    InternalError,               // 500 - 服务器内部错误
    
    #[serde(rename = "SERVICE_UNAVAILABLE")]
    ServiceUnavailable,          // 503 - 服务暂时不可用
    
    #[serde(rename = "TIMEOUT")]
    Timeout,                     // 504 - 操作超时
}

/// 错误响应结构
#[derive(Serialize)]
pub struct ApiError {
    pub error: ErrorDetails,
}

#[derive(Serialize)]
pub struct ErrorDetails {
    /// 错误代码（机器可读）
    pub code: ApiErrorCode,
    
    /// 错误消息（人类可读）
    pub message: String,
    
    /// 附加详情（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    
    /// 请求追踪 ID
    pub request_id: String,
}
```

**HTTP 状态码映射**：
```rust
impl ApiErrorCode {
    /// 获取对应的 HTTP 状态码
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::InvalidRequest => StatusCode::BAD_REQUEST,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::ResourceNotFound => StatusCode::NOT_FOUND,
            Self::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
            Self::Conflict => StatusCode::CONFLICT,
            Self::ValidationFailed => StatusCode::UNPROCESSABLE_ENTITY,
            Self::RateLimitExceeded => StatusCode::TOO_MANY_REQUESTS,
            Self::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
            Self::ServiceUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            Self::Timeout => StatusCode::GATEWAY_TIMEOUT,
        }
    }
}
```

### 3. 响应头

**通用响应头**：
```http
# 请求追踪
X-Request-ID: req_7f8a9b2c

# API 版本
X-API-Version: v1

# 速率限制信息
X-RateLimit-Limit: 60
X-RateLimit-Remaining: 42
X-RateLimit-Reset: 1709654400

# 弃用警告
Deprecated: true
Sunset: 2026-12-31T23:59:59Z
Link: <https://docs.svcmgr.dev/migration/v2>; rel="successor-version"
```

**分页响应头**（可选，与响应体中的 pagination 字段冗余）：
```http
Link: <https://api.example.com/services?page=2>; rel="next",
      <https://api.example.com/services?page=3>; rel="last"
X-Total-Count: 42
```

## Pagination

**查询参数**：
```http
GET /api/v1/services?page=2&per_page=20
```

**响应格式**：
```json
{
  "data": [...],
  "pagination": {
    "page": 2,
    "per_page": 20,
    "total": 42,
    "total_pages": 3,
    "has_next": true,
    "has_prev": true
  }
}
```

**默认值**：
- `page`: 1（从 1 开始计数）
- `per_page`: 20（可配置，最大 100）

## Filtering & Sorting

**过滤**：
```http
# 简单过滤（相等匹配）
GET /api/v1/services?status=running

# 多值过滤（OR 逻辑）
GET /api/v1/services?status=running,stopped

# 复杂过滤（未来功能）
GET /api/v1/services?filter[status]=running&filter[tag]=production
```

**排序**：
```http
# 单字段排序
GET /api/v1/services?sort=name

# 降序排序
GET /api/v1/services?sort=-uptime

# 多字段排序
GET /api/v1/services?sort=status,-uptime
```

**字段选择**（减少响应体大小）：
```http
# 仅返回指定字段
GET /api/v1/services?fields=name,status,pid
```

## Content Negotiation

**请求格式**：
```http
# JSON（默认且唯一支持）
POST /api/v1/services
Content-Type: application/json

{"name": "web-server", "command": "python app.py"}
```

**响应格式**：
```http
# JSON（默认）
Accept: application/json

# YAML（未来功能）
Accept: application/x-yaml
```

**当前版本仅支持 JSON**，未来可根据需求扩展支持 YAML、TOML 等格式。

## Idempotency

**幂等操作**（多次执行结果相同）：
- `GET`, `PUT`, `DELETE` - 天然幂等
- `POST` - 通过 `Idempotency-Key` 实现幂等

**Idempotency-Key 用法**：
```http
POST /api/v1/services
Idempotency-Key: unique-key-12345
Content-Type: application/json

{"name": "web-server", "command": "python app.py"}
```

**实现机制**：
```rust
/// 幂等性密钥存储（内存 + 持久化）
struct IdempotencyStore {
    /// 内存缓存（最近 1000 个请求）
    cache: Arc<RwLock<LruCache<String, IdempotencyRecord>>>,
    
    /// 持久化存储（SQLite）
    db: SqlitePool,
}

#[derive(Clone)]
struct IdempotencyRecord {
    /// 幂等性密钥
    key: String,
    
    /// 原始请求的响应（完整响应体 + 状态码）
    response: CachedResponse,
    
    /// 首次请求时间
    created_at: i64,
    
    /// 过期时间（24 小时）
    expires_at: i64,
}

impl IdempotencyStore {
    /// 检查幂等性密钥是否存在
    async fn check(&self, key: &str) -> Option<CachedResponse> {
        // 1. 先查内存缓存
        if let Some(record) = self.cache.read().await.get(key) {
            if record.expires_at > now() {
                return Some(record.response.clone());
            }
        }
        
        // 2. 查持久化存储
        if let Some(record) = self.db.get(key).await {
            if record.expires_at > now() {
                // 回填内存缓存
                self.cache.write().await.put(key.to_string(), record.clone());
                return Some(record.response);
            }
        }
        
        None
    }
    
    /// 记录请求响应
    async fn store(&self, key: String, response: CachedResponse) {
        let record = IdempotencyRecord {
            key: key.clone(),
            response: response.clone(),
            created_at: now(),
            expires_at: now() + 86400, // 24 小时
        };
        
        // 同时写入内存和持久化存储
        self.cache.write().await.put(key.clone(), record.clone());
        self.db.insert(record).await.ok();
    }
}
```

**过期策略**：
- 幂等性记录保留 **24 小时**
- 定期清理过期记录（每小时执行一次）

## Long-Running Operations

**异步操作模式**（适用于长时间运行的操作）：

```http
# 1. 发起异步操作
POST /api/v1/tasks/backup/run
Content-Type: application/json

{"params": {"target": "s3://backup-bucket"}}

# 2. 立即返回操作 ID（202 Accepted）
HTTP/1.1 202 Accepted
Location: /api/v1/operations/op_abc123

{
  "operation_id": "op_abc123",
  "status": "pending",
  "created_at": "2026-02-23T11:00:00Z"
}

# 3. 轮询操作状态
GET /api/v1/operations/op_abc123

# 4. 操作完成时返回结果
HTTP/1.1 200 OK

{
  "operation_id": "op_abc123",
  "status": "completed",
  "result": {
    "backup_size": 1073741824,
    "duration": 120.5
  },
  "created_at": "2026-02-23T11:00:00Z",
  "completed_at": "2026-02-23T11:02:00Z"
}
```

**WebSocket 实时更新**（未来功能）：
```http
# 订阅操作进度
ws://localhost/api/v1/operations/op_abc123/stream

# 服务器推送进度更新
{"status": "running", "progress": 0.35, "message": "Uploading files..."}
{"status": "running", "progress": 0.70, "message": "Verifying backup..."}
{"status": "completed", "result": {...}}
```

## Health Check & Readiness

**健康检查端点**（无需认证）：
```http
# Liveness - 进程是否存活
GET /health/live

HTTP/1.1 200 OK
{"status": "ok"}

# Readiness - 是否准备好接收请求
GET /health/ready

HTTP/1.1 200 OK
{
  "status": "ready",
  "checks": {
    "database": "ok",
    "mise_integration": "ok",
    "git_repo": "ok"
  }
}

# 降级状态（部分功能不可用但仍可服务）
HTTP/1.1 200 OK
{
  "status": "degraded",
  "checks": {
    "database": "ok",
    "mise_integration": "ok",
    "git_repo": "failed"
  },
  "message": "Git integration unavailable, configuration versioning disabled"
}
```

**Kubernetes 集成**：
```yaml
# 部署清单示例（未来功能）
apiVersion: v1
kind: Pod
spec:
  containers:
  - name: svcmgr
    livenessProbe:
      httpGet:
        path: /health/live
        port: 8080
      initialDelaySeconds: 5
      periodSeconds: 10
    readinessProbe:
      httpGet:
        path: /health/ready
        port: 8080
      initialDelaySeconds: 10
      periodSeconds: 5
```

## API Documentation

**OpenAPI Spec 生成**：
```bash
# 生成 OpenAPI 3.1 规范文件
svcmgr api spec --format openapi > openapi.yaml

# 生成 Postman Collection
svcmgr api spec --format postman > svcmgr.postman_collection.json
```

**内置文档服务器**：
```toml
# .config/mise/svcmgr/config.toml
[api.docs]
# 启用内置文档服务器（默认 true）
enabled = true

# 文档服务路径
path = "/docs"

# 使用的文档渲染器
# "redoc" - ReDoc (默认)
# "swagger-ui" - Swagger UI
# "rapidoc" - RapiDoc
renderer = "redoc"
```

访问 `http://localhost:8080/docs` 查看交互式 API 文档。

## Implementation Example

**API 路由器结构**：
```rust
use axum::{
    routing::{get, post, put, patch, delete},
    Router,
    middleware,
};

/// 构建 API 路由器
pub fn build_api_router() -> Router {
    Router::new()
        // Health check（无需认证）
        .route("/health/live", get(handlers::health::liveness))
        .route("/health/ready", get(handlers::health::readiness))
        
        // API v1
        .nest("/api/v1", api_v1_router())
        
        // API 文档（无需认证）
        .route("/docs", get(handlers::docs::redoc))
        .route("/openapi.yaml", get(handlers::docs::openapi_spec))
        
        // 全局中间件
        .layer(middleware::from_fn(middlewares::request_id))
        .layer(middleware::from_fn(middlewares::rate_limit))
        .layer(middleware::from_fn(middlewares::logging))
}

/// API v1 路由（需要认证）
fn api_v1_router() -> Router {
    Router::new()
        // 服务管理
        .route("/services", get(handlers::services::list))
        .route("/services", post(handlers::services::create))
        .route("/services/:name", get(handlers::services::get))
        .route("/services/:name", put(handlers::services::update))
        .route("/services/:name", delete(handlers::services::delete))
        .route("/services/:name/start", post(handlers::services::start))
        .route("/services/:name/stop", post(handlers::services::stop))
        .route("/services/:name/restart", post(handlers::services::restart))
        .route("/services/:name/logs", get(handlers::services::logs))
        
        // 任务管理
        .route("/tasks", get(handlers::tasks::list))
        .route("/tasks/:name/run", post(handlers::tasks::run))
        .route("/tasks/:name/cancel", post(handlers::tasks::cancel))
        
        // 工具管理
        .route("/tools", get(handlers::tools::list))
        .route("/tools", post(handlers::tools::install))
        .route("/tools/:name", delete(handlers::tools::uninstall))
        
        // 配置管理
        .route("/config", get(handlers::config::get))
        .route("/config", put(handlers::config::update))
        .route("/config/validate", post(handlers::config::validate))
        .route("/config/rollback", post(handlers::config::rollback))
        
        // 环境变量管理
        .route("/env", get(handlers::env::list))
        .route("/env/:key", get(handlers::env::get))
        .route("/env/:key", put(handlers::env::set))
        .route("/env/:key", delete(handlers::env::delete))
        
        // 认证中间件（仅网络模式需要）
        .layer(middleware::from_fn(middlewares::auth))
}
```

**请求追踪中间件**：
```rust
use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

/// 为每个请求生成唯一 ID
pub async fn request_id(
    mut request: Request,
    next: Next,
) -> Response {
    // 1. 尝试从请求头获取客户端提供的 Request ID
    let request_id = request
        .headers()
        .get("X-Request-ID")
        .and_then(|v| v.to_str().ok())
        .map(String::from)
        .unwrap_or_else(|| format!("req_{}", Uuid::new_v4().simple()));
    
    // 2. 将 Request ID 存入 request extensions
    request.extensions_mut().insert(RequestId(request_id.clone()));
    
    // 3. 继续处理请求
    let mut response = next.run(request).await;
    
    // 4. 在响应头中返回 Request ID
    response.headers_mut().insert(
        "X-Request-ID",
        request_id.parse().unwrap(),
    );
    
    response
}

/// Request ID 类型（用于依赖注入）
#[derive(Clone)]
pub struct RequestId(pub String);
```

**速率限制中间件**：
```rust
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::{Response, IntoResponse},
    http::StatusCode,
};
use governor::{Quota, RateLimiter};
use std::sync::Arc;

/// 速率限制状态
pub struct RateLimitState {
    limiter: Arc<RateLimiter<governor::state::InMemoryState, governor::clock::DefaultClock>>,
}

impl RateLimitState {
    pub fn new(requests_per_minute: u32, burst_size: u32) -> Self {
        let quota = Quota::per_minute(requests_per_minute)
            .allow_burst(burst_size);
        let limiter = Arc::new(RateLimiter::direct(quota));
        Self { limiter }
    }
}

/// 速率限制中间件
pub async fn rate_limit(
    State(state): State<Arc<RateLimitState>>,
    request: Request,
    next: Next,
) -> Response {
    // 1. 检查速率限制
    match state.limiter.check() {
        Ok(_) => {
            // 2. 通过速率限制，继续处理
            let mut response = next.run(request).await;
            
            // 3. 添加速率限制响应头
            let headers = response.headers_mut();
            headers.insert("X-RateLimit-Limit", "60".parse().unwrap());
            headers.insert("X-RateLimit-Remaining", "42".parse().unwrap());
            
            response
        }
        Err(_) => {
            // 4. 超出速率限制
            (
                StatusCode::TOO_MANY_REQUESTS,
                [
                    ("X-RateLimit-Limit", "60"),
                    ("X-RateLimit-Remaining", "0"),
                    ("Retry-After", "60"),
                ],
                Json(json!({
                    "error": {
                        "code": "RATE_LIMIT_EXCEEDED",
                        "message": "Too many requests, please try again later"
                    }
                })),
            ).into_response()
        }
    }
}
```

## Testing Strategy

**API 测试覆盖**：

1. **单元测试**：测试单个 handler 函数
2. **集成测试**：测试完整 HTTP 请求-响应流
3. **契约测试**：验证 API 响应符合 OpenAPI 规范
4. **负载测试**：验证速率限制和并发处理能力

**示例测试**：
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum_test::TestServer;
    
    #[tokio::test]
    async fn test_list_services_success() {
        // 1. 创建测试服务器
        let app = build_api_router();
        let server = TestServer::new(app).unwrap();
        
        // 2. 发送请求
        let response = server.get("/api/v1/services").await;
        
        // 3. 验证响应
        assert_eq!(response.status_code(), StatusCode::OK);
        
        let body: serde_json::Value = response.json();
        assert!(body["data"].is_array());
        assert!(body["pagination"].is_object());
    }
    
    #[tokio::test]
    async fn test_rate_limit() {
        let app = build_api_router();
        let server = TestServer::new(app).unwrap();
        
        // 发送 61 个请求（超出速率限制 60/min）
        for i in 0..61 {
            let response = server.get("/api/v1/services").await;
            
            if i < 60 {
                assert_eq!(response.status_code(), StatusCode::OK);
            } else {
                assert_eq!(response.status_code(), StatusCode::TOO_MANY_REQUESTS);
                assert!(response.headers().contains_key("Retry-After"));
            }
        }
    }
    
    #[tokio::test]
    async fn test_idempotency() {
        let app = build_api_router();
        let server = TestServer::new(app).unwrap();
        
        let idempotency_key = "test-key-12345";
        
        // 1. 首次请求
        let response1 = server
            .post("/api/v1/services")
            .header("Idempotency-Key", idempotency_key)
            .json(&json!({
                "name": "test-service",
                "command": "echo hello"
            }))
            .await;
        
        assert_eq!(response1.status_code(), StatusCode::CREATED);
        let body1: serde_json::Value = response1.json();
        
        // 2. 重复请求（相同 Idempotency-Key）
        let response2 = server
            .post("/api/v1/services")
            .header("Idempotency-Key", idempotency_key)
            .json(&json!({
                "name": "test-service",
                "command": "echo hello"
            }))
            .await;
        
        // 3. 验证返回相同响应
        assert_eq!(response2.status_code(), StatusCode::CREATED);
        let body2: serde_json::Value = response2.json();
        assert_eq!(body1, body2);
    }
}
```

## Performance Targets

**响应时间**：
- **简单查询**（如 `GET /services`）：< 50ms (p95)
- **资源操作**（如 `POST /services/{name}/start`）：< 200ms (p95)
- **长时间操作**（如备份任务）：立即返回 202 + 操作 ID

**并发能力**：
- **Unix Socket 模式**：1000+ req/s（单核）
- **网络模式 + TLS**：500+ req/s（单核）

**资源占用**：
- **内存**：< 50MB（空闲）, < 200MB（高负载）
- **CPU**：< 1%（空闲）, < 10%（高负载）

## Related Specifications

- **11-api-services.md** - 服务管理 API 详细定义
- **12-api-tasks.md** - 任务管理 API 详细定义
- **13-api-tools.md** - 工具管理 API 详细定义
- **14-api-config.md** - 配置管理 API 详细定义
- **15-api-env.md** - 环境变量 API 详细定义
- **05-web-service.md** - 内置 HTTP 服务器实现
- **06-feature-flags.md** - 功能开关机制（影响 API 可用性）

## Future Enhancements

1. **GraphQL 支持**：提供 `/api/graphql` 端点，支持复杂查询和订阅
2. **Webhook 通知**：服务状态变化时主动推送到指定 URL
3. **Audit Log API**：查询所有 API 操作的审计日志
4. **批量操作 API**：单次请求启动/停止多个服务
5. **服务依赖图 API**：返回服务间依赖关系的可视化数据
6. **指标导出 API**：Prometheus/OpenTelemetry 格式的指标数据
