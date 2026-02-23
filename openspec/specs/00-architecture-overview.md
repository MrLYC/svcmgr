# 00 - 整体架构概览

> 版本：2.0.0-draft
> 状态：设计中

## 1. 架构分层

```
┌─────────────────────────────────────────────────────────┐
│                     svcmgr 进程                          │
├─────────────────────────────────────────────────────────┤
│  Web 层（axum）                                          │
│  ├── /web/*        → 静态资源（Web UI）                   │
│  ├── /api/*        → 管理接口（REST API）                 │
│  └── /services/{task}/{port_name} → 反向代理              │
├─────────────────────────────────────────────────────────┤
│  调度引擎（Scheduler Engine）                             │
│  ├── 一次性触发器（OneShot）  → mise run                  │
│  ├── 延迟触发器（Delayed）                                │
│  ├── 定时触发器（Cron）      → 定时任务                   │
│  └── 事件触发器（Event）     → 系统事件/任务事件           │
├─────────────────────────────────────────────────────────┤
│  进程管理器（Process Manager）                            │
│  ├── pitchfork 库内嵌（supervisor/daemon/procs 模块）     │
│  ├── 进程组管理（setsid + kill(-pgid)）                   │
│  ├── Graceful Shutdown（SIGTERM → timeout → SIGKILL）     │
│  ├── 自动重启（RestartPolicy + max_restarts）             │
│  └── 资源限制（cgroups v2，功能开关可关闭）                │
├─────────────────────────────────────────────────────────┤
│  配置管理器（Config Manager）                             │
│  ├── 多配置文件解析（svcmgr 独立文件 + mise 配置目录）      │
│  ├── Git 版本化（staging → commit → rollback）            │
│  └── 功能开关（环境变量驱动）                              │
├─────────────────────────────────────────────────────────┤
│  mise 集成层（配置文件驱动 + 必要时子进程调用）              │
│  ├── 依赖管理 → 解析 [tools] + mise install               │
│  ├── 环境变量 → 解析 [env] + mise env --json              │
│  └── 任务定义 → 解析 [tasks] 获取命令                     │
├─────────────────────────────────────────────────────────┤
│  pitchfork 库（Rust crate 内嵌）                          │
│  ├── supervisor 模块 → 进程监控                           │
│  ├── daemon 模块 → Daemon 管理                            │
│  └── procs 模块 → 进程启停信号                            │
└─────────────────────────────────────────────────────────┘
```

## 2. 核心设计原则

1. **配置文件驱动**：所有行为由 TOML 配置文件定义，svcmgr 解析并执行
2. **mise 作为基础设施**：依赖安装、环境变量、任务定义均通过 mise 实现
3. **配置分离**：svcmgr 特有配置使用独立文件，避免与 mise 未来特性冲突
4. **Git 版本化**：配置变更通过 Git 暂存/提交/回滚管理
5. **事件驱动**：系统生命周期和任务状态变化通过事件总线通知

## 3. 技术原子（重新划分）

| 编号 | 原子名称 | 技术基础 | 变更说明 |
|------|----------|----------|----------|
| T01 | Git 配置管理 | libgit2 | 保留，增强为配置版本化核心 |
| T02 | 模板渲染 | minijinja | 保留，用于配置文件模板化 |
| T03 | 依赖管理 | mise `[tools]` | **简化** — 直接读写 mise 配置文件，不再封装 CLI |
| T04 | 任务定义 | mise `[tasks]` | **简化** — 直接读写 mise 配置文件 |
| T05 | 环境变量 | mise `[env]` | **简化** — 直接读写 mise 配置文件 |
| T06 | 调度引擎 | 内置 Rust 实现 | **新设计** — 统一的多触发器调度引擎 |
| T07 | 进程管理 | pitchfork 库内嵌 + cgroups | **重构** — 内嵌 pitchfork 库，cgroups 资源限制（可关闭） |
| T08 | 隧道管理 | cloudflared | 保留（功能开关可关闭）|
| T09 | 反向代理 | 内置 HTTP 代理 | **变更** — 从 nginx 改为内置 HTTP 代理，不依赖外部组件 |

## 4. 配置文件层级

svcmgr 与 mise 配置物理分离：

```
.config/mise/                           # mise 配置目录
├── config.toml                        # 纯 mise 配置（tools, env, tasks）
├── conf.d/                            # mise 场景配置
│   └── *.toml
└── svcmgr/                            # svcmgr 配置（独立）
    ├── config.toml                    # svcmgr 核心配置
    └── conf.d/                        # svcmgr 场景配置
        └── *.toml
```

**关键关系**：
- svcmgr 配置通过 `task = "api-start"` 引用 mise 任务名
- mise 配置定义任务命令：`[tasks.api-start]`
- 两套配置在同一父目录下，便于 Git 版本化管理

## 5. 调度引擎核心流程

```
用户配置（mise.toml）        用户配置（svcmgr/config.toml）
    ↓                               ↓
[tasks.api-start]          [services.api]
run = "node server.js"     task = "api-start"
env = { PORT="3000" }      enable = true
                           restart = "always"
    ↓                               ↓
┌─────────────────────────────────────┐
│      调度引擎（Scheduler Engine）      │
├─────────────────────────────────────┤
│ 1. 解析 services.api 配置            │
│ 2. 注册 Event(SystemInit) 触发器     │
│ 3. 系统初始化 → 触发 SystemInit       │
│ 4. 从 mise 配置读取任务命令           │
│ 5. spawn 进程（注入环境变量）         │
│ 6. 进程管理器接管（日志/资源限制）     │
│ 7. 进程退出 → TaskExit 事件          │
│ 8. 根据 restart="always" 自动重启    │
└─────────────────────────────────────┘
```

## 6. mise 集成策略

由于 mise 不提供 Rust 库 API，采用**配置文件驱动为主、必要时子进程调用为辅**的分层策略：

### 层级 1：配置文件直接解析（主要交互）

```rust
// 直接解析 mise.toml 获取任务命令
fn get_task_command_from_config(config: &toml::Value, task_name: &str) -> Option<String> {
    config.get("tasks")
        .and_then(|t| t.get(task_name))
        .and_then(|t| t.get("run"))
        .and_then(|v| v.as_str())
        .map(String::from)
}
```

### 层级 2：mise 子进程调用（仅必要时）

```rust
// 仅在需要 mise 特有能力时调用
async fn install_tool(tool: &str, version: &str) -> Result<()> {
    Command::new("mise")
        .args(&["install", &format!("{}@{}", tool, version)])
        .status()
        .await?;
    Ok(())
}
```

## 7. Port-Adapter 防腐层

采用**端口-适配器（Port-Adapter）**模式，在 svcmgr 核心与 mise 之间建立防腐层：

```
svcmgr 核心业务逻辑
         ↓
    Port 层（trait 定义）
    ├── DependencyPort  → fn install(tool, ver)
    ├── TaskPort        → fn run_task(name, args)
    ├── EnvPort         → fn get_env() -> HashMap
    └── ConfigPort      → fn list_config_files()
         ↓
   Adapter 层（具体实现）
    ├── MiseV2026Adapter  → 适配当前 mise 版本
    ├── MiseV2025Adapter  → 适配旧版本
    └── MockAdapter       → 测试用
         ↓
  AdapterFactory（版本检测 + 路由）
         ↓
  mise CLI / mise 配置文件
```

## 8. 事件系统

系统通过事件总线实现组件解耦：

```rust
enum EventType {
    SystemInit,                         // 系统初始化完成
    SystemShutdown,                     // 系统关闭前
    TaskStart { task_name: String },    // 任务启动
    TaskExit { task_name: String, exit_code: Option<i32> },  // 任务退出
    ConfigChanged { path: String },     // 配置变更
    Custom { name: String },            // 自定义事件
}
```

**事件流示例**：

```
配置 commit → ConfigChanged 事件
    ↓
调度引擎重新加载 svcmgr/config.toml
    ↓
比较新旧配置 diff
    ↓
对变更的服务执行 restart/stop/start
```

## 9. 功能开关

核心功能可通过配置或环境变量开关：

```toml
# svcmgr/config.toml
[features]
web_ui = true
proxy = true
tunnel = false
scheduler = true
git_versioning = true
resource_limits = true
```

环境变量覆盖：

```bash
SVCMGR_FEATURE_PROXY=0       # 关闭反向代理
SVCMGR_FEATURE_TUNNEL=1      # 启用隧道管理
```

## 10. 目录结构（新架构）

```
src/
├── ports/                    # Port 接口定义（纯 trait）
│   ├── dependency.rs
│   ├── task.rs
│   ├── env.rs
│   └── config.rs
├── adapters/                 # Adapter 实现
│   ├── mise/
│   │   ├── mod.rs
│   │   ├── command.rs
│   │   ├── v2026.rs
│   │   └── parser.rs
│   └── mock.rs
├── engine/                   # 核心引擎
│   ├── scheduler.rs          # 调度引擎
│   ├── process.rs            # 进程管理
│   └── events.rs             # 事件系统
├── config/                   # 配置管理
│   ├── parser.rs
│   └── git.rs
├── web/                      # Web 服务
│   ├── server.rs
│   ├── api.rs
│   └── proxy.rs
└── main.rs
```

## 参考

- [01-config-design.md](./01-config-design.md) - 配置文件设计
- [02-scheduler-engine.md](./02-scheduler-engine.md) - 调度引擎设计
- [07-mise-integration.md](./07-mise-integration.md) - mise 集成层设计
