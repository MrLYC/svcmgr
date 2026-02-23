# Web 框架设计决策

> 版本：1.0.0
> 日期：2026-02-23
> 相关文档：MISE_REDESIGN_RESEARCH_ZH.md

## 背景

### 问题

在基于 mise 重新设计 svcmgr 时，发现：

1. **pitchfork 内置 Web 框架**：pitchfork 的 `web` 模块使用 axum 实现 Web Dashboard
2. **原设计建议**：MISE_REDESIGN_RESEARCH_ZH.md 建议使用 axum 框架搭建 HTTP 服务器
3. **重复问题**：如果 svcmgr 独立实现 axum Web 层，可能与 pitchfork 的 Web 基础设施重复

### 调研发现

| 维度 | pitchfork | svcmgr 需求 |
|------|-----------|------------|
| Web 框架 | axum（已内置） | axum（计划独立实现） |
| 库可用性 | `pitchfork-cli` crate v1.6.0 | 可作为依赖引入 |
| API 文档覆盖率 | 28.61%（低） | N/A |
| Web 模块功能 | Dashboard（进程监控） | Dashboard + API + 反向代理 + Git 版本化 |
| 可扩展性 | 未知（文档不足） | 需要高度定制 |

---

## 方案对比

### 方案 A：完全复用 pitchfork Web 模块

**实现方式**：
```rust
use pitchfork_cli::web::WebServer;

// 假设 pitchfork 提供扩展接口
let mut app = pitchfork_cli::web::create_router();
app = app.nest("/api", svcmgr_api_router());
app = app.nest("/services", proxy_router());
```

**优势**：
- ✅ 零重复实现，直接复用 pitchfork 的 Web 基础设施
- ✅ 代码量最小
- ✅ 与 pitchfork Dashboard 原生集成

**劣势**：
- ❌ **高耦合**：强依赖 pitchfork Web 模块的 API 稳定性
- ❌ **灵活性受限**：扩展能力取决于 pitchfork 提供的接口
- ❌ **文档不足**：pitchfork 库 API 文档覆盖率仅 28.61%
- ❌ **版本风险**：pitchfork 内部 API 变更可能导致破坏性变更
- ❌ **功能缺失**：pitchfork 没有反向代理、Git 版本化等功能

**可行性评估**：⚠️ **不推荐**（依赖未知 API 稳定性，风险高）

---

### 方案 B：独立实现 axum Web 层（推荐）

**实现方式**：
```rust
use axum::{Router, routing::{get, post}};
use pitchfork_cli::{supervisor::Supervisor, daemon::Daemon};  // 仅复用核心模块

let app = Router::new()
    // svcmgr 核心 API
    .nest("/api/services", services_api())
    .nest("/api/tasks", tasks_api())
    .nest("/api/config", config_api())
    
    // 反向代理（svcmgr 特有）
    .nest("/services", proxy_router())
    
    // Web UI
    .nest("/web", static_files())
    
    // 可选：内嵌 pitchfork Dashboard（如果提供）
    .nest("/pitchfork", pitchfork_dashboard());  // 可选
```

**依赖策略**：
```toml
[dependencies]
# 仅引入 pitchfork 核心模块（非 Web）
pitchfork-cli = { version = "1.6", default-features = false, features = ["supervisor", "daemon", "procs"] }

# 独立实现 Web 层
axum = "0.7"
hyper = { version = "1.0", features = ["full"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["fs", "trace"] }
```

**复用策略**：

| pitchfork 模块 | 复用方式 | 理由 |
|---------------|---------|------|
| `supervisor` | ✅ **直接复用** | 进程监控器（核心能力） |
| `daemon` | ✅ **直接复用** | Daemon 数据结构 |
| `procs` | ✅ **直接复用** | 进程管理（启动/停止/信号） |
| `pitchfork_toml` | ⚠️ **参考** | 配置解析（但 svcmgr 用自己的 TOML 格式） |
| `web` | ❌ **不复用** | 独立实现 axum Web 层 |
| `ipc` | ⚠️ **可选** | 进程间通信（按需使用） |
| `state_file` | ⚠️ **可选** | 状态持久化（按需使用） |
| `boot_manager` | ❌ **不适用** | 开机自启（Docker 场景不需要） |

**优势**：
- ✅ **高灵活性**：完全自主控制 Web 架构和路由
- ✅ **低耦合**：仅依赖 pitchfork 的核心模块（稳定性高）
- ✅ **版本独立**：pitchfork Web 变更不影响 svcmgr
- ✅ **功能完整**：可实现反向代理、Git 版本化等 pitchfork 没有的功能
- ✅ **参考学习**：可参考 pitchfork Web 的架构设计（开源代码）

**劣势**：
- ⚠️ 需要独立实现 Web 路由和中间件（约 500-800 行代码）
- ⚠️ 无法直接使用 pitchfork Dashboard UI（需自行开发或移植）

**可行性评估**：✅ **强烈推荐**（平衡灵活性和复用性）

---

### 方案 C：混合方案（内嵌 pitchfork Dashboard）

**实现方式**：
```rust
let app = Router::new()
    // svcmgr 自有功能
    .nest("/api", svcmgr_api())
    .nest("/services", proxy_router())
    .nest("/web", svcmgr_ui())
    
    // 可选：内嵌 pitchfork Dashboard（如果 API 允许）
    .nest("/pitchfork", pitchfork_dashboard_router());
```

**优势**：
- ✅ svcmgr 完全自主控制 Web 架构
- ✅ 可选择性暴露 pitchfork Dashboard（用于调试）

**劣势**：
- ⚠️ 依赖 pitchfork Web 模块提供公开 API
- ⚠️ 如果 pitchfork Dashboard 不可内嵌，方案退化为方案 B

**可行性评估**：⚠️ **可选**（需验证 pitchfork Web API）

---

## 最终决策

### ✅ 采用方案 B：独立实现 axum Web 层

**核心理由**：

1. **低风险**：仅依赖 pitchfork 核心模块（`supervisor`、`daemon`、`procs`），这些模块成熟且稳定
2. **高灵活性**：完全自主设计 Web 架构，支持反向代理、Git 版本化等高级功能
3. **版本独立**：pitchfork 版本更新不影响 svcmgr Web 层
4. **参考价值**：可参考 pitchfork 开源代码学习 Web 架构设计

**不采用方案 A 的原因**：

- pitchfork 库 API 文档覆盖率低（28.61%）
- Web 模块稳定性未知
- svcmgr 需要高度定制的功能（反向代理、Git 集成）

---

## 实现架构

### 依赖关系

```
svcmgr
├── pitchfork-cli (核心模块)
│   ├── supervisor    ← 复用
│   ├── daemon        ← 复用
│   └── procs         ← 复用
│
├── axum (独立实现)
│   ├── Router        ← 自行构建
│   ├── Middleware    ← 自行实现
│   └── Handlers      ← 自行实现
│
└── svcmgr Web 层
    ├── API 路由      ← /api/*
    ├── 反向代理      ← /services/*
    ├── Web UI        ← /web/*
    └── (可选) pitchfork Dashboard ← /pitchfork/*
```

### 模块划分

```
src/
├── web/
│   ├── mod.rs              # Web 服务器入口
│   ├── router.rs           # 路由定义
│   ├── middleware.rs       # 中间件（CORS、日志、认证）
│   ├── api/                # API handlers
│   │   ├── services.rs     # 服务管理 API
│   │   ├── tasks.rs        # 任务管理 API
│   │   ├── config.rs       # 配置管理 API
│   │   └── env.rs          # 环境变量 API
│   ├── proxy.rs            # 反向代理逻辑
│   └── static_files.rs     # 静态文件服务
│
├── scheduler/
│   └── engine.rs           # 使用 pitchfork supervisor
│
└── process/
    └── manager.rs          # 使用 pitchfork daemon + procs
```

### Web 服务器实现示例

```rust
// src/web/mod.rs
use axum::{Router, routing::{get, post, delete}};
use tower_http::trace::TraceLayer;
use std::sync::Arc;

pub struct WebServer {
    config: Arc<SvcmgrConfig>,
    scheduler: Arc<Scheduler>,
}

impl WebServer {
    pub fn new(config: SvcmgrConfig, scheduler: Scheduler) -> Self {
        Self {
            config: Arc::new(config),
            scheduler: Arc::new(scheduler),
        }
    }
    
    pub fn router(&self) -> Router {
        Router::new()
            // API 路由
            .route("/api/services", get(api::list_services))
            .route("/api/services", post(api::create_service))
            .route("/api/services/:name", get(api::get_service))
            .route("/api/services/:name", delete(api::delete_service))
            .route("/api/services/:name/start", post(api::start_service))
            .route("/api/services/:name/stop", post(api::stop_service))
            
            // 反向代理（动态路由）
            .nest("/services", proxy::create_router(self.config.clone()))
            
            // 静态文件
            .nest_service("/web", static_files::create_service())
            
            // 中间件
            .layer(TraceLayer::new_for_http())
            .with_state(self.scheduler.clone())
    }
    
    pub async fn serve(self, addr: &str) -> anyhow::Result<()> {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, self.router()).await?;
        Ok(())
    }
}
```

### 反向代理实现示例

```rust
// src/web/proxy.rs
use axum::{extract::{Path, State}, http::Request, response::IntoResponse};
use hyper::Client;

pub async fn proxy_handler(
    Path((service, port_name, path)): Path<(String, String, String)>,
    State(config): State<Arc<SvcmgrConfig>>,
    req: Request<Body>,
) -> impl IntoResponse {
    // 1. 查找服务端口
    let port = config
        .services
        .get(&service)
        .and_then(|s| s.ports.get(&port_name))
        .ok_or_else(|| "Service not found")?;
    
    // 2. 构造目标 URL（去掉前缀）
    let target_url = format!("http://localhost:{}/{}", port, path);
    
    // 3. 转发请求
    let client = Client::new();
    let mut forwarded_req = Request::builder()
        .uri(target_url)
        .method(req.method())
        .body(req.into_body())?;
    
    // 移除 Host 头
    forwarded_req.headers_mut().remove("host");
    
    client.request(forwarded_req).await
}
```

---

## 参考 pitchfork 的价值

虽然不复用 pitchfork Web 模块，但可以**参考其架构设计**：

### 可参考的设计模式

1. **路由组织**：
   - pitchfork 如何组织 Dashboard 路由
   - API 版本化策略（如 `/v1/daemons`）

2. **中间件设计**：
   - 日志中间件
   - 错误处理中间件

3. **WebSocket 集成**（如果有）：
   - 实时日志流
   - 进程状态推送

4. **前端集成**：
   - 静态文件服务
   - SPA 路由支持

### 参考方法

```bash
# 克隆 pitchfork 源码
git clone https://github.com/jdx/pitchfork.git
cd pitchfork

# 查看 Web 模块实现
rg "axum::" src/web/
rg "Router" src/web/

# 参考路由定义
cat src/web/mod.rs
cat src/web/api.rs  # 如果存在
```

---

## 未来扩展

### 可选：内嵌 pitchfork Dashboard

如果未来发现 pitchfork Web 模块提供了稳定的公开 API，可以考虑内嵌：

```rust
// 未来可能的扩展
let app = Router::new()
    .nest("/", svcmgr_router())
    .nest("/pitchfork", pitchfork_cli::web::router());  // 如果 API 可用
```

**触发条件**：
- pitchfork 官方文档明确支持 Web 模块作为库使用
- API 稳定性得到保证（发布 1.0+ 版本）

---

## 总结

| 决策 | 结果 |
|------|------|
| Web 框架 | 独立实现 axum Web 层 |
| pitchfork 复用范围 | 仅核心模块（supervisor, daemon, procs） |
| pitchfork Web 模块 | 不复用，但参考其架构设计 |
| 代码量估算 | 约 500-800 行（Web 层） |
| 风险等级 | 低（依赖稳定模块） |
| 灵活性 | 高（完全自主） |

**下一步行动**：
1. 实现 `src/web/mod.rs` 基础框架
2. 实现 API 路由（`/api/*`）
3. 实现反向代理（`/services/*`）
4. 实现静态文件服务（`/web/*`）
5. 添加中间件（CORS、日志、认证）
