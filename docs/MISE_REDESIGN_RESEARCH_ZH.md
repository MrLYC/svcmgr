# 基于 mise 重构 svcmgr 调研与设计文档

> 版本：0.1.0-draft
> 日期：2026-02-22
> 基于 main 分支 commit 821361e

---

## 目录

1. [调研背景与目标](#1-调研背景与目标)
2. [mise 核心能力调研](#2-mise-核心能力调研)
3. [pitchfork 参考分析](#3-pitchfork-参考分析)
4. [可行性分析](#4-可行性分析)
5. [新架构设计](#5-新架构设计)
6. [配置文件设计](#6-配置文件设计)
7. [多任务调度引擎设计](#7-多任务调度引擎设计)
8. [子进程管理与资源限制](#8-子进程管理与资源限制)
9. [Git 配置版本管理](#9-git-配置版本管理)
10. [Web 服务与代理设计](#10-web-服务与代理设计)
11. [功能开关机制](#11-功能开关机制)
12. [API 设计](#12-api-设计)
13. [改造影响分析](#13-改造影响分析)
14. [问题与风险](#14-问题与风险)
15. [推荐实施路径](#15-推荐实施路径)

---

## 1. 调研背景与目标

### 1.1 现状

当前 svcmgr 采用「技术原子 + 功能模块 + CLI」三层架构，包含 9 个技术原子和 7 个功能模块。其中：

- **T03/T04/T05**（依赖管理/全局任务/环境变量）已经基于 mise 实现（`atoms/mise.rs`）
- **T06/T07**（服务管理/周期任务）使用内置 supervisor/scheduler 实现（`atoms/supervisor.rs`，~1950 行）
- **T01**（Git）使用 libgit2（`atoms/git.rs`）
- **T02**（模板）使用 minijinja（`atoms/template.rs`）
- **T08**（隧道）封装 cloudflared CLI（`atoms/tunnel.rs`）
- **T09**（代理）封装 nginx CLI（`atoms/proxy.rs`）

### 1.2 改造目标

探索将 svcmgr **完全基于 mise** 重新实现的可行性，核心思路：

1. 以 **mise 配置文件**（`mise.toml`）作为核心驱动机制
2. 利用 mise 的三大核心能力：**依赖管理**、**环境变量管理**、**任务管理**
3. 新增**内存级多任务调度引擎**，统一管理一次性任务、延迟任务、定时任务、事件驱动任务
4. 以**配置文件 + Git** 实现配置版本化管理
5. 保持 Docker 非特权容器兼容性

---

## 2. mise 核心能力调研

### 2.1 依赖管理（Dev Tools）

| 维度 | 详情 |
|------|------|
| 功能 | 管理 node, python, rust, go 等工具版本，类似 asdf/nvm/pyenv |
| 配置 | `[tools]` 段：`node = "22"`, `python = "3.12"` |
| 命令 | `mise use`, `mise install`, `mise ls`, `mise ls-remote` |
| 特性 | 自动切换版本（基于目录）、版本 pinning、多版本共存 |
| svcmgr 适用性 | **完全适用** — 当前 `MiseManager` 已通过 CLI 封装 mise 命令 |

### 2.2 环境变量管理（Environments）

| 维度 | 详情 |
|------|------|
| 功能 | 管理项目级环境变量，类似 direnv |
| 配置 | `[env]` 段：`NODE_ENV = "production"` |
| 高级特性 | `_.file` 加载 `.env` 文件、`_.source` 执行脚本、模板 `{{env.HOME}}` |
| 生效方式 | `mise activate` 自动注入、`mise exec` / `mise run` 中可用 |
| svcmgr 适用性 | **完全适用** — 可用于管理服务运行时环境变量 |

### 2.3 任务管理（Tasks）

| 维度 | 详情 |
|------|------|
| 功能 | 类似 make/just 的任务运行器 |
| 配置 | `[tasks.build]` 段：`run = "cargo build"` |
| 高级特性 | 任务依赖（`depends`）、并行执行、文件监听（`mise watch`）、参数传递（`usage`）、任务模板 |
| 任务属性 | `run`, `depends`, `env`, `dir`, `sources`, `outputs`, `shell`, `hide`, `alias` |
| 执行方式 | `mise run <task>` 前台执行 |
| svcmgr 适用性 | **部分适用** — mise 任务是前台/一次性的，不支持后台常驻、定时触发、事件触发 |

### 2.4 配置文件层级

mise 原生支持多级配置文件，优先级从高到低：

```
.config/mise/conf.d/*.toml     # 按字母序加载，最灵活
.config/mise/config.toml       # 项目级配置
.config/mise.toml              # 项目级配置（简短路径）
mise.toml                      # 项目根配置
mise.local.toml                # 本地配置（不提交到 git）
~/.config/mise/config.toml     # 全局配置
```

**关键能力**：配置内容会**合并**（merge），子目录配置覆盖父目录。

### 2.5 Hooks（实验性）

| Hook | 触发时机 |
|------|----------|
| `cd` | 每次切换目录 |
| `enter` | 进入项目目录 |
| `leave` | 离开项目目录 |
| `preinstall` | 工具安装前 |
| `postinstall` | 工具安装后 |

**限制**：Hooks 依赖 `mise activate`，在 Docker 容器内非交互式场景下不可用。

### 2.6 mise 的局限性

| 局限 | 说明 |
|------|------|
| **无后台进程管理** | mise 不管理 daemon/长驻进程，官方推荐使用姊妹项目 pitchfork |
| **无定时任务** | 没有内置 cron 能力 |
| **无事件系统** | 没有事件触发机制 |
| **无 Web 服务** | 纯 CLI 工具，没有 HTTP API |
| **Hooks 需要 shell activate** | 非交互式环境下 hooks 不可用 |
| **自定义配置段** | mise 会忽略未知的 TOML 段（如 `[x-services]`），不会报错但也不会处理 |

---

## 3. pitchfork 参考分析

[pitchfork](https://github.com/jdx/pitchfork) 是 mise 作者 jdx 开发的**进程管理器**，专为开发者设计，可作为重要参考。

### 3.1 pitchfork 核心能力

| 能力 | 说明 |
|------|------|
| 后台进程管理 | 启动/停止/重启 daemon 进程 |
| 自动重启 | 进程崩溃后自动重启（可配置重试次数） |
| Ready Check | 基于延迟、输出匹配或 HTTP 响应判断服务就绪 |
| Cron 调度 | 内置 cron 表达式支持定时任务 |
| 项目感知 | 进入项目目录自动启动 daemon，离开自动停止 |
| TUI/Web Dashboard | 终端 TUI + 浏览器 Web 面板 |

### 3.2 pitchfork 配置格式

```toml
[daemons.api]
run = "mise run api:dev"
readiness.http = "http://localhost:3000/health"
autostart = true
autostop = true

[daemons.worker]
run = "node worker.js"
restart = "on-failure"
restart_limit = 3

[cron.cleanup]
run = "mise run cleanup"
schedule = "0 */6 * * *"
```

### 3.3 pitchfork 与 svcmgr 的关系

pitchfork 解决了 mise 缺乏的进程管理能力，但：

- pitchfork 是**独立工具**，不适合直接嵌入 svcmgr
- pitchfork **面向开发者本地环境**，不针对 Docker 容器场景
- pitchfork 没有 **Git 配置版本化**、**nginx 代理管理**、**cloudflare 隧道** 等功能
- 但其**架构思路**（配置文件驱动 + 进程管理 + cron）值得借鉴

---

## 4. 可行性分析

### 4.1 可以直接复用 mise 的部分

| 能力 | 当前实现 | mise 替代方案 | 可行性 |
|------|----------|--------------|--------|
| T03 依赖管理 | `MiseManager` 封装 mise CLI | 直接使用 mise 配置 `[tools]` | **完全可行** |
| T04 全局任务 | `MiseManager.run_mise(&["run", ...])` | 直接使用 mise 配置 `[tasks]` | **完全可行** |
| T05 环境变量 | `MiseManager` 读写 .mise.toml `[env]` | 直接使用 mise 配置 `[env]` | **完全可行** |

### 4.2 需要 svcmgr 自行实现的部分

| 能力 | 原因 |
|------|------|
| **多任务调度引擎** | mise 不支持后台常驻/定时/事件触发 |
| **子进程管理** | 需要进程组管理、graceful shutdown、资源限制 |
| **配置文件 Git 版本化** | mise 无此能力，需自行实现 staging → commit → rollback |
| **Web 服务 + 代理** | mise 无 HTTP 服务能力 |
| **Nginx 配置管理** | 需要生成/管理 nginx 配置文件 |
| **Cloudflare 隧道管理** | 需要封装 cloudflared CLI |

### 4.3 结论

> **可以基于 mise 重构**，但 mise 只能覆盖**依赖管理、环境变量、任务定义**三个维度。svcmgr 仍需自行实现进程管理、调度引擎、Web 服务、代理管理等核心能力。mise 的定位是**配置声明和任务定义的基础设施**，svcmgr 在此之上构建运行时管理层。

---

## 5. 新架构设计

### 5.1 整体架构

```
┌─────────────────────────────────────────────────────────┐
│                     svcmgr 进程                          │
├─────────────────────────────────────────────────────────┤
│  Web 层（axum/actix-web）                                │
│  ├── /web/*        → 静态资源                            │
│  ├── /api/*        → 管理接口                            │
│  └── /services/{task}/{port_name} → 反向代理              │
├─────────────────────────────────────────────────────────┤
│  调度引擎（Scheduler Engine）                             │
│  ├── 一次性触发器（OneShot）  → mise run                  │
│  ├── 延迟触发器（Delayed）                                │
│  ├── 定时触发器（Cron）      → 定时任务                   │
│  └── 事件触发器（Event）     → 系统事件/任务事件           │
├─────────────────────────────────────────────────────────┤
│  进程管理器（Process Manager）                            │
│  ├── 进程组管理（setsid + kill(-pgid)）                   │
│  ├── Graceful Shutdown（SIGTERM → timeout → SIGKILL）     │
│  ├── 自动重启（RestartPolicy）                            │
│  └── 资源限制（setrlimit/ulimit）                         │
├─────────────────────────────────────────────────────────┤
│  配置管理器（Config Manager）                             │
│  ├── 多配置文件解析（x- 前缀扩展段）                       │
│  ├── Git 版本化（staging → commit → rollback）            │
│  └── 功能开关（环境变量驱动）                              │
├─────────────────────────────────────────────────────────┤
│  mise 集成层                                              │
│  ├── 依赖管理 → mise install/use                          │
│  ├── 环境变量 → mise env                                  │
│  └── 任务定义 → mise tasks                                │
└─────────────────────────────────────────────────────────┘
```

### 5.2 核心原则

1. **配置文件驱动**：所有行为由 TOML 配置文件定义，svcmgr 解析并执行
2. **mise 作为基础设施**：依赖安装、环境变量、任务定义均通过 mise 实现
3. **x- 前缀扩展**：svcmgr 特有配置使用 `x-` 前缀，避免与 mise 未来特性冲突
4. **Git 版本化**：配置变更通过 Git 暂存/提交/回滚管理
5. **事件驱动**：系统生命周期和任务状态变化通过事件总线通知

### 5.3 技术原子重新划分

| 编号 | 原子名称 | 技术基础 | 变更说明 |
|------|----------|----------|----------|
| T01 | Git 配置管理 | libgit2 | 保留，增强为配置版本化核心 |
| T02 | 模板渲染 | minijinja | 保留，用于配置文件模板化 |
| T03 | 依赖管理 | mise `[tools]` | **简化** — 直接读写 mise 配置文件，不再封装 CLI |
| T04 | 任务定义 | mise `[tasks]` | **简化** — 直接读写 mise 配置文件 |
| T05 | 环境变量 | mise `[env]` | **简化** — 直接读写 mise 配置文件 |
| T06 | 调度引擎 | 内置 Rust 实现 | **新设计** — 统一的多触发器调度引擎 |
| T07 | 进程管理 | setsid + rlimit | **重构** — 从 supervisor 中提取，增加资源限制 |
| T08 | 隧道管理 | cloudflared | 保留 |
| T09 | 反向代理 | 内置 HTTP 代理 | **变更** — 从 nginx 改为内置 HTTP 代理（见下文讨论） |

---

## 6. 配置文件设计

### 6.1 配置文件层级

```
~/.config/mise.toml                    # mise 内置配置（svcmgr 自身依赖）
~/.config/mise/config.toml             # mise 全局配置（用户工具链）
.config/mise/conf.d/*.toml             # 独立场景配置（按字母序加载）
  ├── 00-base.toml                     # 基础配置
  ├── 10-services.toml                 # 服务定义
  ├── 20-cron.toml                     # 定时任务
  └── 99-local.toml                    # 本地覆盖
```

### 6.2 x- 扩展段格式

所有 svcmgr 特有的配置都使用 `x-` 前缀的 TOML 段，mise 会忽略这些段（不报错），svcmgr 自行解析。

#### 6.2.1 服务定义 `[x-services.<name>]`

```toml
[x-services.web-api]
task = "api:start"           # mise 任务名称（通用配置）
enable = true                # 是否启用（仅服务管理任务需要）
restart = "always"           # 重启策略：no | always | on-failure
restart_delay = 1            # 重启延迟（秒）
stop_timeout = 10            # 停止超时（秒），超时后 SIGKILL
workdir = ""                 # 工作目录（通用配置）
timeout = ""                 # 任务执行超时（通用配置）
cpu_limit = "50"             # CPU 时间限制（秒，基于 RLIMIT_CPU）
mem_limit = "512m"           # 地址空间限制（基于 RLIMIT_AS）
nofile_limit = 1024          # 最大打开文件数（基于 RLIMIT_NOFILE）
nproc_limit = 100            # 最大子进程数（基于 RLIMIT_NPROC）
fsize_limit = "1g"           # 最大文件大小（基于 RLIMIT_FSIZE）
http_ports = { web = 8080 }  # HTTP 端口映射（用于反向代理）

[x-services.worker]
task = "worker:run"
enable = true
restart = "on-failure"
restart_delay = 5

[x-services.docs]
task = "docs:serve"
cron = "0 */6 * * *"         # cron 表达式 → 周期执行（与 enable 互斥）
workdir = "/app/docs"
timeout = "300"              # 超时 300 秒
```

#### 6.2.2 配置目录管理 `[x-configurations.<name>]`

```toml
[x-configurations.nginx]
path = "/etc/nginx/conf.d"   # 配置目录路径
template = "nginx/default"   # 初始化模板名称（可选）

[x-configurations.app]
path = ".config/app"          # 应用配置目录
```

#### 6.2.3 功能开关 `[x-features]`

```toml
[x-features]
web_ui = true                 # 启用 Web UI
proxy = true                  # 启用反向代理
tunnel = false                # 禁用隧道管理
scheduler = true              # 启用调度引擎
```

等价的环境变量控制：

```toml
[env]
SVCMGR_FEATURE_WEB_UI = "1"
SVCMGR_FEATURE_PROXY = "1"
SVCMGR_FEATURE_TUNNEL = "0"
```

### 6.3 配置文件解析流程

```
                      ┌──────────────────┐
                      │ mise.toml 文件集   │
                      └────────┬─────────┘
                               │
                    ┌──────────┴──────────┐
                    │   TOML 解析器        │
                    │   (toml crate)       │
                    └──────────┬──────────┘
                               │
              ┌────────────────┼────────────────┐
              │                │                │
     ┌────────┴─────┐  ┌──────┴──────┐  ┌──────┴──────┐
     │ mise 原生段    │  │ x- 扩展段    │  │ 其他未知段   │
     │ [tools]       │  │ [x-services] │  │ (忽略)      │
     │ [tasks]       │  │ [x-cron]     │  │             │
     │ [env]         │  │ [x-features] │  │             │
     └──────┬───────┘  └──────┬───────┘  └─────────────┘
            │                 │
     mise 处理          svcmgr 处理
```

**重要说明**：svcmgr 需要自行解析 TOML 文件提取 `x-` 段。mise 本身不会处理这些段，也不会报错（TOML 解析器只是忽略未知字段）。但需注意：**mise 未来版本可能对未知段产生警告**。`x-` 前缀是一种约定，并非 mise 官方支持的扩展机制。

---

## 7. 多任务调度引擎设计

### 7.1 概述

调度引擎是新架构的核心组件，负责管理所有任务的生命周期。它是一个纯内存运行时，配置来自解析后的 TOML 配置文件。

### 7.2 触发器类型

```rust
/// 任务触发器类型
enum Trigger {
    /// 一次性触发 — 立即执行（等价于 `mise run`）
    OneShot,
    
    /// 延迟触发 — 延迟指定时间后执行
    Delayed { delay: Duration },
    
    /// 定时触发 — cron 表达式驱动
    Cron { expression: String, schedule: cron::Schedule },
    
    /// 事件触发 — 系统/任务事件驱动
    Event { event_type: EventType },
}

/// 事件类型
enum EventType {
    /// 系统初始化完成
    SystemInit,
    /// 系统关闭前
    SystemShutdown,
    /// 任务退出（正常或异常）
    TaskExit { task_name: String, exit_code: Option<i32> },
    /// 任务启动
    TaskStart { task_name: String },
    /// 配置变更
    ConfigChanged { path: String },
    /// 自定义事件
    Custom { name: String },
}
```

### 7.3 任务定义

```rust
/// 调度任务
struct ScheduledTask {
    /// 任务名称（唯一标识）
    name: String,
    /// 触发器
    trigger: Trigger,
    /// 执行方式
    execution: Execution,
    /// 运行状态
    state: TaskState,
    /// 资源限制
    limits: Option<ResourceLimits>,
    /// 超时
    timeout: Option<Duration>,
    /// 重启策略（仅服务类任务）
    restart_policy: RestartPolicy,
}

/// 执行方式
enum Execution {
    /// 通过 mise run 执行任务
    MiseTask { task_name: String, args: Vec<String> },
    /// 直接执行命令
    Command { command: String, args: Vec<String>, env: HashMap<String, String> },
}
```

### 7.4 调度引擎核心循环

```rust
/// 调度引擎主循环（简化伪代码）
async fn engine_loop(engine: &SchedulerEngine) {
    loop {
        tokio::select! {
            // 1. 检查 cron 定时任务
            _ = engine.next_cron_tick() => {
                for task in engine.due_cron_tasks() {
                    engine.spawn_task(task).await;
                }
            }
            
            // 2. 处理事件队列
            event = engine.event_rx.recv() => {
                for task in engine.tasks_for_event(&event) {
                    engine.spawn_task(task).await;
                }
            }
            
            // 3. 处理延迟任务到期
            _ = engine.next_delayed_tick() => {
                for task in engine.due_delayed_tasks() {
                    engine.spawn_task(task).await;
                }
            }
            
            // 4. 监控子进程退出
            exit = engine.wait_any_child() => {
                engine.handle_child_exit(exit).await;
                // 触发 TaskExit 事件 → 可能触发自动重启或事件监听任务
            }
            
            // 5. 外部命令（来自 API/CLI）
            cmd = engine.command_rx.recv() => {
                engine.handle_command(cmd).await;
            }
        }
    }
}
```

### 7.5 与 mise 的集成

```
用户定义 mise.toml:

[tasks.api-start]
run = "node server.js"
env = { PORT = "3000" }

[x-services.api]
task = "api-start"       ← 引用 mise 任务
enable = true
restart = "always"
http_ports = { web = 3000 }
```

调度引擎的工作流：
1. 解析 `[x-services.api]`，发现 `task = "api-start"` + `enable = true`
2. 注册 `Event(SystemInit)` 触发器（因为 `enable = true` 意味着开机启动）
3. 系统初始化时触发 `SystemInit` 事件
4. 引擎通过 `mise run api-start` 或直接 `Command` 执行任务
5. 进程管理器接管子进程（setsid、日志捕获、资源限制）
6. 进程退出时触发 `TaskExit` 事件 → 根据 `restart = "always"` 自动重启

### 7.6 与当前实现的差异

| 维度 | 当前（supervisor.rs） | 新设计（调度引擎） |
|------|----------------------|-------------------|
| 服务管理 | SupervisorAtom trait + SupervisorManager | SchedulerEngine + 事件触发器 |
| 定时任务 | SchedulerAtom trait（CRUD，不执行） | SchedulerEngine + Cron 触发器 |
| 任务执行 | 直接 `tokio::process::Command` | 通过 `mise run` 或直接 `Command` |
| 配置来源 | 独立 TOML 文件（每个服务一个） | 统一 mise.toml 中的 `x-services` 段 |
| 事件系统 | 无 | 内置事件总线 |
| 统一性 | 服务和定时任务分离管理 | 统一调度，通过触发器区分 |

---

## 8. 子进程管理与资源限制

### 8.1 进程组管理（当前已实现）

当前 `supervisor.rs` 已使用 `setsid()` 创建进程组，`kill(-pgid, sig)` 发送信号到整个进程树。**可完全复用**。

### 8.2 Docker 非特权容器下的资源限制

#### 8.2.1 可行方案：setrlimit (POSIX resource limits)

在非特权容器中，**`setrlimit` / `prlimit` 是可行的**。通过 Rust 的 `rlimit` crate（~1260 万下载量）或 `libc` crate 直接调用。

**可限制的资源**：

| 资源 | RLIMIT 常量 | 说明 | Docker 非特权可用 |
|------|------------|------|-----------------|
| CPU 时间 | `RLIMIT_CPU` | 进程 CPU 时间上限（秒） | **是** |
| 文件大小 | `RLIMIT_FSIZE` | 单个文件最大字节数 | **是** |
| 打开文件数 | `RLIMIT_NOFILE` | 最大文件描述符数量 | **是**（受容器 hard limit 约束） |
| 进程数 | `RLIMIT_NPROC` | 用户可创建最大进程数 | **是**（受容器 hard limit 约束） |
| 地址空间 | `RLIMIT_AS` | 虚拟内存上限 | **是** |
| 核心转储 | `RLIMIT_CORE` | core dump 大小上限 | **是** |
| 栈大小 | `RLIMIT_STACK` | 线程栈大小上限 | **是** |

**不可限制的资源（非特权容器）**：

| 资源 | 说明 | 原因 |
|------|------|------|
| cgroups CPU quota | 精确 CPU 份额 | 需要 cgroup 写权限（通常只有 root） |
| cgroups memory limit | 精确物理内存限制 | 需要 cgroup 写权限 |
| OOM score | OOM killer 优先级 | 需要 CAP_SYS_ADMIN |
| nice/priority | 进程调度优先级 | 降低优先级可以，提高需要 CAP_SYS_NICE |

#### 8.2.2 实现方式

在 `fork` 之后、`exec` 之前通过 `pre_exec` 设置资源限制：

```rust
use std::os::unix::process::CommandExt;

let mut cmd = tokio::process::Command::new(&command);

// 在子进程 exec 之前设置资源限制
unsafe {
    cmd.pre_exec(move || {
        // 1. 创建新进程组
        libc::setsid();
        
        // 2. 设置资源限制
        if let Some(nofile) = nofile_limit {
            let rlim = libc::rlimit {
                rlim_cur: nofile,
                rlim_max: nofile,
            };
            libc::setrlimit(libc::RLIMIT_NOFILE, &rlim);
        }
        
        if let Some(nproc) = nproc_limit {
            let rlim = libc::rlimit {
                rlim_cur: nproc,
                rlim_max: nproc,
            };
            libc::setrlimit(libc::RLIMIT_NPROC, &rlim);
        }
        
        if let Some(mem_bytes) = mem_limit {
            let rlim = libc::rlimit {
                rlim_cur: mem_bytes,
                rlim_max: mem_bytes,
            };
            libc::setrlimit(libc::RLIMIT_AS, &rlim);
        }
        
        // ... 其他资源限制
        
        Ok(())
    });
}
```

#### 8.2.3 配置格式

```toml
[x-services.heavy-worker]
task = "worker:process"
cpu_limit = "300"          # RLIMIT_CPU: 300 秒 CPU 时间
mem_limit = "512m"         # RLIMIT_AS: 512MB 虚拟内存
nofile_limit = 4096        # RLIMIT_NOFILE: 4096 个文件描述符
nproc_limit = 50           # RLIMIT_NPROC: 最多 50 个子进程
fsize_limit = "100m"       # RLIMIT_FSIZE: 单文件最大 100MB
```

#### 8.2.4 注意事项与限制

1. **`RLIMIT_AS` 不等于物理内存限制**：`RLIMIT_AS` 限制的是虚拟地址空间（包括共享库映射），实际物理内存使用可能远低于此值。对于使用大量 mmap 的程序（如 JVM），可能需要设置较大的值
2. **`RLIMIT_CPU` 是累计 CPU 时间**：不是 CPU 百分比。进程超出限制后收到 SIGXCPU（可捕获），之后收到 SIGKILL（不可捕获）
3. **不能超过容器的 hard limit**：Docker 容器自身可能已有 ulimit 限制（通过 `docker run --ulimit` 设置），`setrlimit` 不能超过容器的 hard limit
4. **`RLIMIT_NPROC` 是 per-UID**：限制的是当前用户的总进程数，不是每个服务独立的限制。在容器中通常只有一个用户，所以这实际上是全局限制

> **结论**：基于 `setrlimit` 的资源限制在 Docker 非特权容器中**可行但有局限性**。适合做基本的安全防护（防止文件描述符泄漏、限制 fork bomb），但无法实现精确的 CPU/内存配额控制（需要 cgroups v2 委托，见下文）。

#### 8.2.5 进阶方案：cgroups v2 委托（可选）

在 Docker 容器中，如果 host 使用 cgroups v2 且容器使用 `--cgroupns=private`（Docker 默认行为），容器内的进程**可以创建子 cgroup** 来实现更精细的资源控制：

```bash
# 检查容器内是否有 cgroup 写权限
ls -la /sys/fs/cgroup/
# 如果有 cgroup.subtree_control 等文件可写，就可以使用 cgroup 委托
```

但这**需要额外配置**，不是所有 Docker 环境都支持。建议作为**可选增强功能**，默认仍使用 `setrlimit`。

---

## 9. Git 配置版本管理

### 9.1 核心流程

用户提出的配置生命周期（5 阶段）完全映射到 Git 操作：

```
阶段 1: 配置初始化
  └── git init + git add mise.toml + git commit -m "init"
      HEAD 指向初始配置文件版本

阶段 2: 配置更新
  └── 用户修改配置文件（通过 API/CLI/直接编辑）
      工作区有未暂存的更改

阶段 3: 配置应用前（暂存）
  └── git add <changed-files>
      配置进入暂存区，工作区和暂存区一致
      此时可以 git diff --staged 查看即将应用的变更

阶段 4: 配置应用成功
  └── git commit -m "apply: <description>"
      暂存区内容提交，HEAD 前进

阶段 5: 配置应用失败
  └── git reset HEAD -- <files> + git checkout -- <files>
      将暂存区文件移出并回滚到 HEAD 版本
```

### 9.2 Git 仓库结构

```
~/.config/svcmgr/                      # Git 仓库根目录
├── .git/                              # Git 数据
├── mise.toml                          # 主配置文件
├── mise/
│   └── conf.d/
│       ├── 10-services.toml           # 服务定义
│       ├── 20-cron.toml               # 定时任务
│       └── ...
└── templates/                         # 配置模板
    ├── nginx/
    └── cloudflare/
```

### 9.3 配置管理 API

```rust
trait ConfigManager {
    /// 阶段 1: 初始化配置仓库
    fn init(&self) -> Result<()>;
    
    /// 阶段 2: 获取当前配置（HEAD 版本）
    fn current(&self) -> Result<Config>;
    
    /// 阶段 2: 更新配置文件
    fn update(&self, path: &str, content: &str) -> Result<()>;
    
    /// 阶段 3: 暂存配置变更
    fn stage(&self, paths: &[&str]) -> Result<()>;
    
    /// 阶段 3: 查看暂存区 diff
    fn staged_diff(&self) -> Result<String>;
    
    /// 阶段 4: 提交暂存的配置变更
    fn commit(&self, message: &str) -> Result<String>;  // returns commit hash
    
    /// 阶段 5: 回滚暂存区（应用失败时）
    fn reset_staged(&self, paths: &[&str]) -> Result<()>;
    
    /// 回滚到指定版本
    fn rollback(&self, commit: &str) -> Result<()>;
    
    /// 查看配置历史
    fn log(&self, limit: usize) -> Result<Vec<CommitInfo>>;
    
    /// Diff 两个版本
    fn diff(&self, from: &str, to: &str) -> Result<String>;
}
```

### 9.4 事件集成

配置变更与事件系统深度集成：

| 事件 | 触发时机 | 典型用途 |
|------|----------|---------|
| `ConfigChanged { path }` | 配置文件 commit 成功后 | 重新加载服务、更新调度 |
| `ConfigStaged { path }` | 配置文件暂存后 | 预检查、dry-run |
| `ConfigRolledBack { commit }` | 配置回滚后 | 重启受影响的服务 |

内置事件处理链：

```
配置 commit → ConfigChanged 事件 → 
  → 调度引擎重新加载 x-services 段
  → 比较新旧配置 diff
  → 对变更的服务执行 restart/stop/start
```

---

## 10. Web 服务与代理设计

### 10.1 内置 Web 服务

svcmgr 内置 HTTP 服务器（建议使用 `axum` 框架），提供：

```
路径                            功能
─────────────────────────────────────────
/web/*                         静态资源（Web UI）
/api/*                         管理 API（见 §12）
/services/{task}/{port_name}/* 反向代理到服务端口
```

### 10.2 反向代理机制

#### 当前方案：依赖 nginx

当前 svcmgr 通过 `ProxyAtom` / `NginxManager` 管理 nginx 配置，由 nginx 负责实际的反向代理。

#### 新方案选项

**方案 A：保留 nginx，svcmgr 仅管理配置**（推荐）

- 优点：nginx 成熟稳定，高性能，支持 SSL/TLS、WebSocket、负载均衡
- 缺点：需要额外安装 nginx，增加 Docker 镜像体积
- 适用：生产环境、需要高性能代理的场景

**方案 B：内置 HTTP 代理（axum/hyper）**

- 优点：零外部依赖，单二进制部署，代码简单
- 缺点：功能有限（无 SSL termination、无 upstream 健康检查等），需要自行实现 WebSocket 代理
- 适用：轻量级开发环境、简单的端口转发

**方案 C：混合方案**

- svcmgr 内置简单 HTTP 代理（端口转发 + 静态文件）
- 通过 `x-features.nginx = true` 启用 nginx 管理
- 默认使用内置代理，需要高级功能时切换到 nginx

**建议采用方案 C**，初期实现内置代理满足基本需求，后续按需启用 nginx。

### 10.3 代理路由配置

```toml
[x-services.api]
task = "api:start"
enable = true
http_ports = { web = 8080 }   # 端口名 = 端口号

[x-services.docs]
task = "docs:serve"
enable = true
http_ports = { site = 3000 }
```

路由映射：
- `/services/api/web/*` → `http://localhost:8080/*`（去掉前缀 `/services/api/web`）
- `/services/docs/site/*` → `http://localhost:3000/*`

### 10.4 静态文件转发

```toml
[x-static.public]
path = "/var/www/public"
location = "/static"
autoindex = false
index = ["index.html"]
```

---

## 11. 功能开关机制

### 11.1 设计

功能开关通过两种方式控制：

**方式 1：配置文件**

```toml
[x-features]
web_ui = true
proxy = true
tunnel = false
scheduler = true
git_versioning = true
```

**方式 2：环境变量**（优先级高于配置文件）

```bash
SVCMGR_FEATURE_WEB_UI=1
SVCMGR_FEATURE_PROXY=0
SVCMGR_FEATURE_TUNNEL=1    # 覆盖配置文件中的 false
```

### 11.2 功能开关列表

| 开关名 | 默认值 | 说明 |
|--------|-------|------|
| `web_ui` | `true` | 启用 Web UI 静态资源服务 |
| `proxy` | `true` | 启用内置反向代理 |
| `nginx` | `false` | 启用 nginx 配置管理（需要 nginx 已安装） |
| `tunnel` | `false` | 启用 Cloudflare 隧道管理（需要 cloudflared） |
| `scheduler` | `true` | 启用调度引擎（定时任务、事件触发） |
| `git_versioning` | `true` | 启用配置 Git 版本化 |
| `resource_limits` | `true` | 启用子进程资源限制 |

### 11.3 实现

```rust
struct FeatureFlags {
    web_ui: bool,
    proxy: bool,
    nginx: bool,
    tunnel: bool,
    scheduler: bool,
    git_versioning: bool,
    resource_limits: bool,
}

impl FeatureFlags {
    fn load(config: &TomlConfig, env: &HashMap<String, String>) -> Self {
        // 1. 从 [x-features] 段加载默认值
        // 2. 环境变量 SVCMGR_FEATURE_* 覆盖
    }
}
```

---

## 12. API 设计

### 12.1 API 路由

```
/web/*                                  静态资源
/api/v1/services                        GET    列出所有服务
/api/v1/services/{name}                 GET    获取服务详情
/api/v1/services/{name}/start           POST   启动服务
/api/v1/services/{name}/stop            POST   停止服务
/api/v1/services/{name}/restart         POST   重启服务
/api/v1/services/{name}/logs            GET    获取服务日志
/api/v1/tasks                           GET    列出所有定时任务
/api/v1/tasks/{name}                    GET    获取任务详情
/api/v1/tasks/{name}/trigger            POST   手动触发任务
/api/v1/config                          GET    获取当前配置
/api/v1/config                          PUT    更新配置
/api/v1/config/stage                    POST   暂存配置变更
/api/v1/config/apply                    POST   应用（commit）配置
/api/v1/config/rollback                 POST   回滚配置
/api/v1/config/diff                     GET    查看配置 diff
/api/v1/config/log                      GET    查看配置历史
/api/v1/tools                           GET    列出已安装工具（mise ls）
/api/v1/tools                           POST   安装工具（mise use）
/api/v1/env                             GET    列出环境变量
/api/v1/env                             PUT    设置环境变量
/api/v1/features                        GET    获取功能开关状态
/api/v1/events                          GET    SSE 事件流
/services/{task}/{port_name}/*          代理   反向代理到服务端口
```

### 12.2 与现有 CLI 的映射

| 当前 CLI 命令 | 新 API | 变更说明 |
|--------------|--------|---------|
| `svcmgr setup` | 保留 CLI | 初始化 mise 环境 + git init 配置仓库 |
| `svcmgr run` | 保留 CLI | 启动 svcmgr 主进程（调度引擎 + Web 服务） |
| `svcmgr teardown` | 保留 CLI | 停止所有服务，清理 |
| `svcmgr service *` | `/api/v1/services/*` | 服务管理改为 API 驱动，CLI 调用 API |
| `svcmgr cron *` | `/api/v1/tasks/*` | 定时任务管理改为 API 驱动 |
| `svcmgr mise *` | `/api/v1/tools/*` + `/api/v1/env/*` | mise 管理改为 API 驱动 |
| `svcmgr nginx *` | 按 x-features.nginx 决定是否保留 | 可选 |
| `svcmgr tunnel *` | 按 x-features.tunnel 决定是否保留 | 可选 |
| `svcmgr config *` | `/api/v1/config/*` | 配置管理改为 API 驱动 |
| `svcmgr tty *` | 视需求保留或合并为 x-services | TTY 本质是一个服务 |

---

## 13. 改造影响分析

### 13.1 保留的模块

| 模块 | 文件 | 原因 |
|------|------|------|
| Git 原子 | `atoms/git.rs` | 配置版本化核心，需增强 |
| 模板原子 | `atoms/template.rs` | 配置模板化，保留 |
| 隧道原子 | `atoms/tunnel.rs` | cloudflared 封装，按需保留 |
| 错误处理 | `error.rs` | 通用基础设施 |

### 13.2 需要重写的模块

| 模块 | 当前文件 | 变更 |
|------|---------|------|
| supervisor/scheduler | `atoms/supervisor.rs`（1950 行） | 拆分为调度引擎 + 进程管理器 |
| mise 管理器 | `atoms/mise.rs`（608 行） | 简化为配置文件读写，去掉 CLI 封装 |
| nginx 管理 | `atoms/proxy.rs` | 视 x-features.nginx 保留或替换为内置代理 |
| 所有 feature 模块 | `features/*.rs` | 合并到调度引擎配置驱动模型 |
| CLI | `cli/*.rs` | 简化，核心逻辑移到 API 层 |
| 配置 | `config.rs` | 重写为基于 mise.toml 的配置管理 |
| 主入口 | `main.rs` | 重写为调度引擎 + Web 服务启动 |

### 13.3 新增模块

| 模块 | 职责 |
|------|------|
| `engine/scheduler.rs` | 多任务调度引擎（触发器、事件总线） |
| `engine/process.rs` | 进程管理器（进程组、资源限制、日志） |
| `engine/events.rs` | 事件系统（EventBus、EventType） |
| `config/parser.rs` | mise.toml + x- 扩展段解析器 |
| `config/git.rs` | Git 版本化管理（staging/commit/rollback） |
| `web/server.rs` | HTTP 服务器（axum） |
| `web/api.rs` | REST API handlers |
| `web/proxy.rs` | 内置反向代理 |
| `web/static.rs` | 静态文件服务 |

### 13.4 依赖变更

| 当前依赖 | 状态 | 新增依赖 | 用途 |
|----------|------|---------|------|
| `clap` | 保留 | `axum` | Web 框架 |
| `tokio` | 保留 | `hyper` | HTTP 客户端（代理） |
| `serde` / `toml` | 保留 | `tower` | 中间件 |
| `git2` | 保留 | `rlimit` | 资源限制 |
| `cron` | 保留 | `tower-http` | 静态文件、CORS |
| `chrono` | 保留 | | |
| `libc` | 保留 | | |
| `minijinja` | 保留 | | |
| `futures` | 保留 | | |
| `regex` | 保留 | | |
| `tracing` | 保留 | | |

---

## 14. 问题与风险

### 14.1 关键问题

#### P1: mise 对未知 TOML 段的行为

**问题**：`x-` 前缀段是自定义约定，mise 目前会忽略，但**未来版本可能产生警告或报错**。

**建议**：
- 短期：使用 `x-` 前缀，监控 mise 的 changelog
- 长期：向 mise 社区提议官方支持 `x-` 前缀的扩展机制（类似 Docker Compose 的 `x-` 扩展）
- 备选：将 svcmgr 配置放在独立文件中（如 `svcmgr.toml`），通过 `[env]` 段间接引用

#### P2: mise run 的进程管理边界

**问题**：通过 `mise run <task>` 启动的进程，mise 会接管 stdin/stdout/stderr。svcmgr 需要拿到子进程的 PID 来管理其生命周期。

**建议**：
- 方案 A：不通过 `mise run`，而是直接从 mise 配置中读取 `[tasks.<name>].run` 的命令，然后自行 spawn（**推荐**）
- 方案 B：通过 `mise run` 启动，再 parse `/proc` 找到子进程 PID（不可靠）
- 方案 C：`mise run` 只用于一次性任务，服务类任务直接 spawn

#### P3: 多配置文件合并的确定性

**问题**：mise 的配置合并规则复杂（目录层级、conf.d 字母序），svcmgr 需要确保解析结果与 mise 一致。

**建议**：
- 先执行 `mise cfg` 获取 mise 加载的文件列表和优先级
- svcmgr 按相同规则加载和合并 x- 段
- 或者统一使用 `mise env --json` 获取最终环境，配置解析只读自己的 x- 段

#### P4: `RLIMIT_NPROC` 是 per-UID 而非 per-process

**问题**：`RLIMIT_NPROC` 限制的是当前用户的总进程数，不能为每个服务设置独立限制。在容器中只有一个用户的情况下，这实际上是全局限制。

**建议**：
- 将 `nproc_limit` 文档为全局限制（非服务独立限制）
- 使用 `RLIMIT_NPROC` 作为安全兜底（防止 fork bomb），不作为精确配额
- 如需精确限制，提示用户使用 cgroups v2 委托方案

#### P5: `RLIMIT_AS` vs 实际内存

**问题**：`RLIMIT_AS` 限制虚拟地址空间，不等于物理内存。JVM 等程序的虚拟地址空间可能远大于实际使用。

**建议**：
- 配置字段使用 `mem_limit`（映射到 RLIMIT_AS），但文档中明确说明是虚拟内存限制
- 如需精确物理内存限制，提示用户通过 Docker `--memory` 参数设置

#### P6: 事件触发器的复杂度

**问题**：事件系统（尤其是 `TaskExit` → 自动重启 → 可能触发更多事件）可能产生事件风暴。

**建议**：
- 实现事件去抖动（debounce）和速率限制
- 自动重启需要指数退避（exponential backoff）
- 设置最大重启次数（`restart_limit`）
- 检测重启循环（短时间内多次重启）并自动降级

#### P7: TTY 服务的定位

**问题**：当前 WebTtyManager 是独立的功能模块，如果改为 x-services 驱动，需要额外处理 ttyd 的端口分配和 nginx 路由。

**建议**：
- TTY 本质上是一个服务（运行 ttyd 进程），可以定义为 `x-services.tty-<name>`
- 端口通过 `http_ports` 配置，代理通过统一的反向代理机制处理
- 创建 TTY 等于创建一个 ttyd 服务 + 配置代理路由

### 14.2 风险矩阵

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|---------|
| mise 未来版本不兼容 x- 段 | 中 | 高 | 监控 mise changelog，备选独立配置文件 |
| 进程管理稳定性 | 低 | 高 | 复用已验证的 setsid/kill 逻辑 |
| 资源限制在某些容器不可用 | 低 | 中 | 设为可选功能，优雅降级 |
| 事件风暴 | 中 | 中 | 去抖动 + 速率限制 + 指数退避 |
| 配置合并不一致 | 低 | 中 | 使用 `mise cfg` 获取权威文件列表 |
| 改造工作量大 | 高 | 中 | 分阶段实施，保持向后兼容 |

---

## 15. 推荐实施路径

### Phase 1: 基础设施（预计 1-2 周）

1. **配置解析器**：实现 mise.toml + x- 扩展段解析
2. **Git 配置管理**：实现 5 阶段配置生命周期
3. **功能开关**：实现 x-features + 环境变量驱动

### Phase 2: 调度引擎（预计 2-3 周）

4. **调度引擎核心**：实现 Trigger 系统（OneShot、Delayed、Cron、Event）
5. **进程管理器**：从 supervisor.rs 提取并增强（加入资源限制）
6. **事件总线**：实现 EventBus 和内置事件

### Phase 3: Web 服务（预计 1-2 周）

7. **HTTP 服务器**：axum 框架搭建
8. **REST API**：实现 /api/v1/* 端点
9. **内置反向代理**：实现 /services/{task}/{port}/* 转发
10. **静态文件服务**：实现 /web/* 服务

### Phase 4: 集成与迁移（预计 1-2 周）

11. **CLI 改造**：CLI 命令调用 API
12. **mise 集成**：`mise run` 任务执行 + 环境变量注入
13. **向后兼容**：迁移脚本，将旧格式转换为新配置格式
14. **文档更新**：openspec 和 wiki 更新

### Phase 5: 可选增强（按需）

15. nginx 管理（x-features.nginx）
16. Cloudflare 隧道管理（x-features.tunnel）
17. cgroups v2 委托（高级资源限制）
18. Web UI 前端

---

## 附录 A: 完整配置文件示例

```toml
# ~/.config/mise/config.toml（全局配置）
# 或 项目根 mise.toml

# ============================================================
# mise 原生段（mise 处理）
# ============================================================

[tools]
node = "22"
python = "3.12"

[env]
NODE_ENV = "production"
DATABASE_URL = "postgres://localhost:5432/mydb"
SVCMGR_FEATURE_PROXY = "1"
SVCMGR_FEATURE_TUNNEL = "0"

[tasks.api-start]
description = "Start API server"
run = "node dist/server.js"
depends = ["api-build"]
env = { PORT = "3000" }

[tasks.api-build]
description = "Build API server"
run = "npm run build"
sources = ["src/**/*.ts"]
outputs = ["dist/**/*.js"]

[tasks.worker-run]
description = "Start background worker"
run = "python worker.py"

[tasks.cleanup]
description = "Cleanup old data"
run = "python scripts/cleanup.py"

# ============================================================
# svcmgr 扩展段（svcmgr 处理，mise 忽略）
# ============================================================

[x-features]
web_ui = true
proxy = true
nginx = false
tunnel = false
scheduler = true
git_versioning = true
resource_limits = true

[x-services.api]
task = "api-start"
enable = true
restart = "always"
restart_delay = 2
stop_timeout = 10
http_ports = { web = 3000 }
nofile_limit = 4096

[x-services.worker]
task = "worker-run"
enable = true
restart = "on-failure"
restart_delay = 5
mem_limit = "512m"
nproc_limit = 50

[x-services.cleanup]
task = "cleanup"
cron = "0 2 * * *"               # 每天凌晨 2 点执行
timeout = "600"                   # 10 分钟超时

[x-services.health-check]
task = "api-health"
cron = "*/5 * * * *"              # 每 5 分钟执行
timeout = "30"

[x-configurations.app]
path = ".config/app"

[x-static.public]
path = "/var/www/public"
location = "/static"
```

## 附录 B: 参考资料

| 资料 | 链接 |
|------|------|
| mise 官方文档 | https://mise.jdx.dev |
| mise 配置文档 | https://mise.jdx.dev/configuration.html |
| mise 任务文档 | https://mise.jdx.dev/tasks/ |
| mise Hooks | https://mise.jdx.dev/hooks.html |
| pitchfork | https://pitchfork.jdx.dev |
| pitchfork GitHub | https://github.com/jdx/pitchfork |
| rlimit crate | https://crates.io/crates/rlimit |
| Docker resource constraints | https://docs.docker.com/engine/containers/resource_constraints/ |
| cgroups v2 文档 | https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v2.html |
| tokio-cron-scheduler | https://crates.io/crates/tokio-cron-scheduler |
