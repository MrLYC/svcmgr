# 05 - Web 服务与内置反向代理

> 版本：2.0.0-draft
> 状态：设计中

## 1. 设计目标

### 1.1 为什么内置 HTTP 代理

**替代 nginx 的原因**：
- **简化部署**：无需安装和配置 nginx，减少外部依赖
- **配置一体化**：代理规则直接在 svcmgr.toml 中定义，与服务定义紧密结合
- **零配置文件生成**：无需生成 nginx 配置文件，避免模板渲染和 reload 的复杂性
- **更好的集成**：与 svcmgr 进程管理器深度集成，服务启停自动更新路由
- **统一的健康检查**：代理层可直接感知服务健康状态，实现智能路由

**保留的能力**：
- HTTP/HTTPS 反向代理
- WebSocket 支持
- 静态文件服务
- 路径路由和主机路由
- TLS 终止
- 健康检查集成

**不支持的高级功能**（可通过 mise 任务启动外部 nginx 补充）：
- TCP/UDP 代理（Layer 4）
- 复杂的负载均衡策略（轮询、最少连接等）
- 缓存、限流、WAF 等高级功能
- nginx 的完整 Lua/njs 脚本能力

### 1.2 核心功能

| 功能 | 说明 |
|------|------|
| **HTTP 反向代理** | 将外部请求转发到后端服务的指定端口 |
| **路径路由** | 根据 URL 路径前缀分发请求 |
| **主机路由** | 根据 Host 头部分发请求 |
| **静态文件服务** | 直接服务静态资源（前端页面、文档等） |
| **WebSocket 支持** | 透传 WebSocket 连接到后端服务 |
| **TLS 终止** | HTTPS 请求在代理层解密，与后端通过 HTTP 通信 |
| **健康检查集成** | 自动从健康检查结果更新后端可用性 |
| **自动路由更新** | 服务启停时动态更新路由表，无需重启 |

---

## 2. 配置格式

### 2.1 服务端口定义

在 svcmgr.toml 中，服务通过 `ports` 字段定义对外暴露的端口：

```toml
# .config/mise/svcmgr/config.toml

[services.api]
run_task = "api-start"  # 引用 mise 任务
ports = [
  { name = "http", internal = 3000, external = 8080 },
]
health_check = { type = "http", path = "/health", interval_secs = 10 }

[services.frontend]
run_task = "frontend-serve"
ports = [
  { name = "http", internal = 5173, external = 8081 },
]

[services.worker]
run_task = "worker-run"
# 不暴露端口的后台服务
```

**字段说明**：
- `name`：端口名称（用于路由引用）
- `internal`：服务内部监听的端口
- `external`：svcmgr 代理监听的端口（可选，如果只通过路径路由访问则不需要）

### 2.2 HTTP 路由配置

#### 2.2.1 路径路由（推荐）

通过 `[http.routes]` 定义基于路径的路由规则：

```toml
# .config/mise/svcmgr/config.toml

[[http.routes]]
path = "/api"              # 匹配路径前缀
strip_prefix = true        # 转发前去除 /api 前缀
backend = "api:http"       # 格式: "服务名:端口名"
timeout_secs = 30

[[http.routes]]
path = "/ws"               # WebSocket 路由
backend = "api:http"
websocket = true           # 启用 WebSocket 支持

[[http.routes]]
path = "/"                 # 默认路由（静态文件或前端应用）
backend = "frontend:http"
```

**处理流程**：
1. 请求 `http://localhost/api/users` 匹配到第一个路由
2. 如果 `strip_prefix = true`，转发时路径变为 `/users`
3. 转发到 `api` 服务的 `http` 端口（实际地址 `http://127.0.0.1:3000/users`）

#### 2.2.2 主机路由（多域名场景）

通过 `host` 字段实现虚拟主机路由：

```toml
[[http.routes]]
host = "api.example.com"
backend = "api:http"

[[http.routes]]
host = "www.example.com"
backend = "frontend:http"

[[http.routes]]
host = "*"                 # 默认主机（兜底路由）
path = "/"
backend = "frontend:http"
```

**优先级**：主机路由 > 路径路由 > 默认路由

#### 2.2.3 静态文件服务

```toml
[[http.routes]]
path = "/static"
serve_dir = "/path/to/static"  # 直接服务本地目录
index = "index.html"           # 目录索引文件（可选）

[[http.routes]]
path = "/docs"
serve_dir = "./docs-output"
strip_prefix = true            # 访问 /docs/guide.html → 查找 docs-output/guide.html
```

### 2.3 HTTPS 配置

```toml
[http]
bind = "0.0.0.0:8080"      # HTTP 监听地址
https_bind = "0.0.0.0:8443" # HTTPS 监听地址（可选）

[http.tls]
cert = "/path/to/cert.pem"
key = "/path/to/key.pem"
# 或者使用 mise env 中的路径变量
# cert = "{{ env.TLS_CERT_PATH }}"
# key = "{{ env.TLS_KEY_PATH }}"
```

**自动 HTTPS 重定向**（可选）：

```toml
[http.tls]
cert = "/path/to/cert.pem"
key = "/path/to/key.pem"
redirect_http = true       # HTTP 请求自动重定向到 HTTPS
```

---

## 3. 实现设计

### 3.1 技术选型

**HTTP 框架**：axum（基于 hyper + tokio）

**选择理由**：
- 轻量级，性能优秀
- 与 tokio 生态深度集成
- 支持 WebSocket、SSE、反向代理等场景
- 中间件系统灵活，易于实现健康检查集成
- 社区活跃，文档完善

**依赖库**：
```toml
# Cargo.toml
[dependencies]
axum = { version = "0.7", features = ["ws"] }
hyper = "1.0"
tokio = { version = "1", features = ["full"] }
tower = "0.4"                # 中间件和服务抽象
tower-http = { version = "0.5", features = ["fs", "trace"] }
```

### 3.2 核心架构

```rust
// src/web/mod.rs

use axum::{Router, extract::State, routing::any};
use std::sync::Arc;
use tokio::sync::RwLock;

/// HTTP 代理服务主结构
pub struct ProxyServer {
    /// 路由表（动态更新）
    router: Arc<RwLock<Router>>,
    /// 后端服务状态（健康检查结果）
    backends: Arc<RwLock<BackendRegistry>>,
    /// 配置
    config: ProxyConfig,
}

/// 后端服务注册表
pub struct BackendRegistry {
    /// 服务名 -> 后端地址映射
    services: HashMap<String, Backend>,
}

#[derive(Clone)]
pub struct Backend {
    /// 服务名
    name: String,
    /// 端口名
    port_name: String,
    /// 实际监听地址（127.0.0.1:port）
    address: SocketAddr,
    /// 健康状态
    healthy: bool,
    /// 最后健康检查时间
    last_check: Instant,
}

impl ProxyServer {
    /// 启动 HTTP 代理服务
    pub async fn start(config: ProxyConfig) -> Result<Self> {
        let backends = Arc::new(RwLock::new(BackendRegistry::new()));
        let router = Arc::new(RwLock::new(Self::build_router(&config, backends.clone())?));

        let server = Self { router, backends, config };

        // 启动 HTTP 监听
        tokio::spawn(server.clone().serve_http());

        // 如果配置了 HTTPS，启动 HTTPS 监听
        if let Some(tls_config) = &server.config.tls {
            tokio::spawn(server.clone().serve_https(tls_config.clone()));
        }

        Ok(server)
    }

    /// 构建路由表
    fn build_router(
        config: &ProxyConfig,
        backends: Arc<RwLock<BackendRegistry>>,
    ) -> Result<Router> {
        let mut router = Router::new();

        for route in &config.routes {
            match &route.target {
                RouteTarget::Backend(backend) => {
                    // 反向代理路由
                    router = router.route(
                        &format!("{}*path", route.path),
                        any(proxy_handler)
                    );
                },
                RouteTarget::ServeDir(dir) => {
                    // 静态文件服务
                    let serve_dir = tower_http::services::ServeDir::new(dir)
                        .append_index_html_on_directories(route.index.is_some());
                    router = router.nest_service(&route.path, serve_dir);
                },
            }
        }

        Ok(router.with_state(AppState { backends }))
    }

    /// 动态更新路由（服务启停时调用）
    pub async fn update_backend(&self, service_name: &str, port_name: &str, addr: Option<SocketAddr>) {
        let mut backends = self.backends.write().await;
        
        if let Some(addr) = addr {
            // 服务启动：注册后端
            backends.register(service_name, port_name, addr);
        } else {
            // 服务停止：移除后端
            backends.unregister(service_name, port_name);
        }
    }

    /// 更新健康检查状态
    pub async fn update_health(&self, service_name: &str, healthy: bool) {
        let mut backends = self.backends.write().await;
        backends.update_health(service_name, healthy);
    }
}
```

### 3.3 反向代理处理器

```rust
// src/web/proxy_handler.rs

use axum::{
    extract::{State, Request, Path},
    response::{Response, IntoResponse},
    http::StatusCode,
};
use hyper::body::Incoming;
use hyper_util::client::legacy::Client;

/// 反向代理请求处理
async fn proxy_handler(
    State(state): State<AppState>,
    Path(path): Path<String>,
    req: Request,
) -> Result<Response, ProxyError> {
    // 1. 解析目标后端（从路由配置中匹配）
    let route = state.match_route(&req)?;
    let backend = state.backends.read().await
        .get(&route.backend)
        .ok_or(ProxyError::BackendNotFound)?
        .clone();

    // 2. 检查后端健康状态
    if !backend.healthy {
        return Err(ProxyError::BackendUnhealthy);
    }

    // 3. 构建后端请求 URL
    let backend_url = format!(
        "http://{}{}",
        backend.address,
        if route.strip_prefix {
            path.strip_prefix(&route.path).unwrap_or(&path)
        } else {
            &path
        }
    );

    // 4. 转发请求
    let client = Client::builder(TokioExecutor::new()).build_http();
    
    let (mut parts, body) = req.into_parts();
    parts.uri = backend_url.parse()?;
    
    let backend_req = Request::from_parts(parts, body);
    let backend_resp = client.request(backend_req).await?;

    // 5. 返回后端响应
    Ok(backend_resp.into_response())
}

/// WebSocket 代理处理
async fn websocket_proxy_handler(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
    req: Request,
) -> Result<Response, ProxyError> {
    let route = state.match_route(&req)?;
    let backend = state.backends.read().await
        .get(&route.backend)
        .ok_or(ProxyError::BackendNotFound)?
        .clone();

    // 升级到 WebSocket 连接
    ws.on_upgrade(move |socket| async move {
        // 连接到后端 WebSocket
        let backend_url = format!("ws://{}{}", backend.address, req.uri().path());
        let (backend_ws, _) = tokio_tungstenite::connect_async(backend_url).await.unwrap();

        // 双向转发 WebSocket 消息
        proxy_websocket(socket, backend_ws).await;
    })
}

/// WebSocket 消息双向转发
async fn proxy_websocket(
    client: WebSocket,
    backend: WebSocketStream<MaybeTlsStream<TcpStream>>,
) {
    let (client_tx, client_rx) = client.split();
    let (backend_tx, backend_rx) = backend.split();

    // 客户端 -> 后端
    let client_to_backend = client_rx.forward(backend_tx);
    // 后端 -> 客户端
    let backend_to_client = backend_rx.forward(client_tx);

    // 任一方向断开则结束
    tokio::select! {
        _ = client_to_backend => {},
        _ = backend_to_client => {},
    }
}
```

### 3.4 健康检查集成

```rust
// src/web/health_integration.rs

use crate::process::HealthChecker;

impl ProxyServer {
    /// 订阅健康检查事件，自动更新后端状态
    pub fn subscribe_health_events(&self, health_checker: &HealthChecker) {
        let backends = self.backends.clone();
        
        health_checker.on_health_change(move |service_name, healthy| {
            let backends = backends.clone();
            tokio::spawn(async move {
                backends.write().await.update_health(service_name, healthy);
            });
        });
    }

    /// 主动健康检查（补充进程管理器的健康检查）
    pub async fn active_health_check(&self) {
        let backends = self.backends.read().await;
        
        for backend in backends.list() {
            let healthy = self.check_backend_health(&backend).await;
            
            if backend.healthy != healthy {
                drop(backends); // 释放读锁
                self.update_health(&backend.name, healthy).await;
            }
        }
    }

    async fn check_backend_health(&self, backend: &Backend) -> bool {
        // 简单的 TCP 连接检查
        tokio::net::TcpStream::connect(backend.address)
            .await
            .is_ok()
    }
}
```

---

## 4. 与进程管理器集成

### 4.1 服务启动时注册后端

```rust
// src/scheduler/mod.rs

impl SchedulerEngine {
    async fn start_service(&mut self, service_name: &str) -> Result<()> {
        let service = self.config.services.get(service_name)?;
        let process = self.process_manager.spawn(service).await?;

        // 等待进程启动并监听端口
        tokio::time::sleep(Duration::from_millis(500)).await;

        // 注册后端到代理服务器
        for port in &service.ports {
            let addr = SocketAddr::new(
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                port.internal,
            );
            self.proxy_server.update_backend(
                service_name,
                &port.name,
                Some(addr),
            ).await;
        }

        Ok(())
    }

    async fn stop_service(&mut self, service_name: &str) -> Result<()> {
        let service = self.config.services.get(service_name)?;

        // 从代理服务器移除后端
        for port in &service.ports {
            self.proxy_server.update_backend(
                service_name,
                &port.name,
                None, // None 表示移除后端
            ).await;
        }

        // 停止进程
        self.process_manager.stop(service_name).await?;

        Ok(())
    }
}
```

### 4.2 健康检查失败时自动摘除后端

```rust
// src/process/health_check.rs

impl HealthChecker {
    async fn check_loop(&self, service_name: String, config: HealthCheckConfig) {
        let mut interval = tokio::time::interval(Duration::from_secs(config.interval_secs));
        let mut consecutive_failures = 0;

        loop {
            interval.tick().await;

            let healthy = self.perform_check(&service_name, &config).await;

            if healthy {
                consecutive_failures = 0;
                self.emit_event(HealthEvent::Healthy(service_name.clone()));
            } else {
                consecutive_failures += 1;
                
                if consecutive_failures >= config.unhealthy_threshold {
                    // 通知代理服务器摘除后端
                    self.emit_event(HealthEvent::Unhealthy(service_name.clone()));
                    
                    // 如果配置了自动重启，触发重启
                    if config.auto_restart {
                        self.emit_event(HealthEvent::RestartRequired(service_name.clone()));
                    }
                }
            }
        }
    }
}
```

---

## 5. 配置示例

### 5.1 简单的前后端分离应用

```toml
# .config/mise/svcmgr/config.toml

[services.backend]
run_task = "backend-start"
ports = [{ name = "http", internal = 3000 }]
health_check = { type = "http", path = "/health", interval_secs = 10 }

[services.frontend]
run_task = "frontend-start"
ports = [{ name = "http", internal = 5173 }]

[http]
bind = "0.0.0.0:8080"

[[http.routes]]
path = "/api"
strip_prefix = true
backend = "backend:http"

[[http.routes]]
path = "/"
backend = "frontend:http"
```

**访问方式**：
- `http://localhost:8080/` → 前端应用（实际 5173 端口）
- `http://localhost:8080/api/users` → 后端 API（实际 3000 端口，路径变为 `/users`）

### 5.2 多域名 + 静态文件

```toml
[services.api]
run_task = "api-start"
ports = [{ name = "http", internal = 3000 }]

[http]
bind = "0.0.0.0:80"
https_bind = "0.0.0.0:443"

[http.tls]
cert = "/etc/ssl/certs/example.com.crt"
key = "/etc/ssl/private/example.com.key"
redirect_http = true

[[http.routes]]
host = "api.example.com"
backend = "api:http"

[[http.routes]]
host = "www.example.com"
path = "/"
serve_dir = "/var/www/html"
index = "index.html"

[[http.routes]]
host = "docs.example.com"
path = "/"
serve_dir = "/var/www/docs"
```

### 5.3 WebSocket 支持

```toml
[services.realtime]
run_task = "realtime-server"
ports = [{ name = "ws", internal = 3001 }]

[http]
bind = "0.0.0.0:8080"

[[http.routes]]
path = "/ws"
backend = "realtime:ws"
websocket = true          # 启用 WebSocket 支持
timeout_secs = 300        # WebSocket 连接超时（5 分钟）
```

---

## 6. 性能与优化

### 6.1 性能目标

| 指标 | 目标值 |
|------|--------|
| **单请求延迟** | < 5ms（P99，本地后端） |
| **吞吐量** | > 10k req/s（单核，简单代理） |
| **WebSocket 连接数** | > 10k 并发连接 |
| **内存占用** | < 50MB（空闲），< 500MB（10k 连接） |

### 6.2 优化措施

**1. 连接复用**：使用 HTTP/1.1 Keep-Alive 和 HTTP/2 多路复用

```rust
let client = Client::builder(TokioExecutor::new())
    .pool_idle_timeout(Duration::from_secs(90))
    .pool_max_idle_per_host(100)
    .build_http();
```

**2. 零拷贝转发**：直接转发 HTTP body，不解析内容

```rust
// 避免缓冲整个 body
let backend_resp = client.request(backend_req).await?;
Ok(backend_resp.into_response()) // 直接流式转发
```

**3. 路由缓存**：使用 matchit 或 radix tree 快速匹配路由

```rust
use matchit::Router as MatchRouter;

let mut router = MatchRouter::new();
for route in &config.routes {
    router.insert(&route.path, route.clone())?;
}
```

**4. 后端状态缓存**：避免每次请求都读取 RwLock

```rust
// 使用 Arc<AtomicBool> 存储健康状态
#[derive(Clone)]
pub struct Backend {
    name: String,
    address: SocketAddr,
    healthy: Arc<AtomicBool>, // 无锁读取
}
```

### 6.3 监控指标

通过 Prometheus metrics 暴露关键指标：

```rust
// src/web/metrics.rs

use prometheus::{register_histogram_vec, register_int_counter_vec};

lazy_static! {
    static ref HTTP_REQUEST_DURATION: HistogramVec = register_histogram_vec!(
        "svcmgr_http_request_duration_seconds",
        "HTTP request duration",
        &["route", "method", "status"]
    ).unwrap();

    static ref HTTP_REQUESTS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "svcmgr_http_requests_total",
        "Total HTTP requests",
        &["route", "method", "status"]
    ).unwrap();

    static ref BACKEND_ERRORS: IntCounterVec = register_int_counter_vec!(
        "svcmgr_backend_errors_total",
        "Backend connection errors",
        &["backend"]
    ).unwrap();
}
```

---

## 7. 错误处理

### 7.1 错误类型

```rust
#[derive(Debug, thiserror::Error)]
pub enum ProxyError {
    #[error("Backend not found: {0}")]
    BackendNotFound(String),

    #[error("Backend unhealthy: {0}")]
    BackendUnhealthy(String),

    #[error("Backend connection failed: {0}")]
    BackendConnectionFailed(#[from] hyper::Error),

    #[error("Request timeout")]
    Timeout,

    #[error("Invalid route configuration: {0}")]
    InvalidRoute(String),

    #[error("TLS configuration error: {0}")]
    TlsError(String),
}

impl IntoResponse for ProxyError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ProxyError::BackendNotFound(_) => (StatusCode::SERVICE_UNAVAILABLE, self.to_string()),
            ProxyError::BackendUnhealthy(_) => (StatusCode::SERVICE_UNAVAILABLE, self.to_string()),
            ProxyError::BackendConnectionFailed(_) => (StatusCode::BAD_GATEWAY, "Backend connection failed".to_string()),
            ProxyError::Timeout => (StatusCode::GATEWAY_TIMEOUT, "Request timeout".to_string()),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string()),
        };

        (status, message).into_response()
    }
}
```

### 7.2 优雅降级

**场景 1：后端服务全部不健康**

```rust
async fn proxy_handler(/* ... */) -> Result<Response, ProxyError> {
    let backends = state.backends.read().await;
    let healthy_backends: Vec<_> = backends.list()
        .filter(|b| b.healthy)
        .collect();

    if healthy_backends.is_empty() {
        // 返回维护页面
        return Ok(Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .body(Body::from(include_str!("../static/maintenance.html")))
            .unwrap());
    }

    // ... 正常代理逻辑
}
```

**场景 2：TLS 证书配置错误**

```rust
impl ProxyServer {
    pub async fn start(config: ProxyConfig) -> Result<Self> {
        // ... HTTP 服务启动

        // HTTPS 启动失败不影响 HTTP 服务
        if let Some(tls_config) = &config.tls {
            match Self::load_tls_config(tls_config) {
                Ok(tls) => {
                    tokio::spawn(server.clone().serve_https(tls));
                }
                Err(e) => {
                    eprintln!("HTTPS启动失败: {}, 仅使用HTTP", e);
                }
            }
        }

        Ok(server)
    }
}
```

---

## 8. 测试策略

### 8.1 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_backend_registration() {
        let mut registry = BackendRegistry::new();
        let addr = "127.0.0.1:3000".parse().unwrap();

        registry.register("api", "http", addr);

        let backend = registry.get("api:http").unwrap();
        assert_eq!(backend.address, addr);
        assert_eq!(backend.healthy, true);
    }

    #[tokio::test]
    async fn test_health_update() {
        let mut registry = BackendRegistry::new();
        registry.register("api", "http", "127.0.0.1:3000".parse().unwrap());

        registry.update_health("api", false);
        assert_eq!(registry.get("api:http").unwrap().healthy, false);
    }
}
```

### 8.2 集成测试

```rust
#[tokio::test]
async fn test_proxy_request() {
    // 启动测试后端服务
    let backend = axum::Router::new()
        .route("/test", get(|| async { "backend response" }));
    let backend_addr = spawn_server(backend).await;

    // 配置代理服务器
    let config = ProxyConfig {
        bind: "127.0.0.1:0".parse().unwrap(),
        routes: vec![
            RouteConfig {
                path: "/api".to_string(),
                strip_prefix: true,
                target: RouteTarget::Backend("test:http".to_string()),
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    let proxy = ProxyServer::start(config).await.unwrap();
    proxy.update_backend("test", "http", Some(backend_addr)).await;

    // 发送请求到代理
    let client = reqwest::Client::new();
    let resp = client
        .get(&format!("http://{}/api/test", proxy.addr()))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "backend response");
}
```

### 8.3 性能测试

使用 `criterion` 进行基准测试：

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_route_matching(c: &mut Criterion) {
    let router = build_test_router();
    
    c.bench_function("route_match", |b| {
        b.iter(|| {
            router.match_route(black_box("/api/users/123"))
        })
    });
}

criterion_group!(benches, bench_route_matching);
criterion_main!(benches);
```

---

## 9. 安全考虑

### 9.1 请求验证

```rust
// 限制请求大小
const MAX_BODY_SIZE: usize = 10 * 1024 * 1024; // 10MB

async fn validate_request(req: &Request) -> Result<(), ProxyError> {
    // 检查 Content-Length
    if let Some(content_length) = req.headers().get(CONTENT_LENGTH) {
        let size: usize = content_length.to_str()?.parse()?;
        if size > MAX_BODY_SIZE {
            return Err(ProxyError::RequestTooLarge);
        }
    }

    // 检查 Host 头部（防止 Host 头部注入）
    let host = req.headers().get(HOST)
        .ok_or(ProxyError::MissingHost)?
        .to_str()?;
    
    if !is_valid_host(host) {
        return Err(ProxyError::InvalidHost);
    }

    Ok(())
}
```

### 9.2 后端隔离

```rust
// 强制后端只能绑定 localhost
fn validate_backend_address(addr: &SocketAddr) -> Result<(), ProxyError> {
    if !addr.ip().is_loopback() {
        return Err(ProxyError::InvalidBackendAddress(
            "Backend must bind to localhost".to_string()
        ));
    }
    Ok(())
}
```

### 9.3 TLS 安全配置

```rust
use rustls::{ServerConfig, version::TLS13};

fn build_tls_config(cert_path: &str, key_path: &str) -> Result<ServerConfig> {
    let certs = load_certs(cert_path)?;
    let key = load_private_key(key_path)?;

    let config = ServerConfig::builder()
        .with_safe_default_cipher_suites()
        .with_safe_default_kx_groups()
        .with_protocol_versions(&[&TLS13])? // 仅 TLS 1.3
        .with_no_client_auth()
        .with_single_cert(certs, key)?;

    Ok(config)
}
```

---

## 10. 相关规范

- **02-scheduler-engine.md** - 调度引擎如何启动 Web 服务
- **03-process-manager.md** - 进程管理器如何通知代理服务器后端状态变化
- **06-feature-flags.md** - 通过功能开关禁用内置代理，改用外部 nginx
- **11-api-services.md** - 通过 API 动态添加/删除路由规则
