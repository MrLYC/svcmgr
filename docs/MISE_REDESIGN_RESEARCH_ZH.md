# 基于 mise 重构 svcmgr 调研与设计文档

> 版本：0.2.0-draft
> 日期：2026-02-22
> 基于 main 分支 commit 821361e

---

## 目录

1. [调研背景与目标](#1-调研背景与目标)
2. [mise 核心能力调研](#2-mise-核心能力调研)
3. [pitchfork 参考分析](#3-pitchfork-参考分析)
4. [可行性分析](#4-可行性分析)
5. [新架构设计](#5-新架构设计)
6. [mise 解耦架构设计](#6-mise-解耦架构设计)
7. [配置文件设计](#7-配置文件设计)
8. [多任务调度引擎设计](#8-多任务调度引擎设计)
9. [子进程管理与资源限制](#9-子进程管理与资源限制)
10. [Git 配置版本管理](#10-git-配置版本管理)
11. [Web 服务与代理设计](#11-web-服务与代理设计)
12. [功能开关机制](#12-功能开关机制)
13. [API 设计](#13-api-设计)
14. [改造影响分析](#14-改造影响分析)
15. [问题与风险](#15-问题与风险)
16. [推荐实施路径](#16-推荐实施路径)

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
| svcmgr 适用性 | **完全适用** — 可通过 mise 配置文件声明工具依赖，svcmgr 解析配置并调用 mise 能力 |

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
| **自定义配置段** | mise 会忽略未知的 TOML 段；svcmgr 不应依赖此行为，推荐使用独立配置文件（见 §6.5、§7） |

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

### 3.3 pitchfork 库内嵌可行性分析

pitchfork 的 `Cargo.toml` 中同时定义了 `[[bin]]` 和 `[lib]` 段，发布为 `pitchfork-cli` crate（v1.6.0），**可以作为 Rust 库依赖内嵌到 svcmgr 中**。

**pitchfork_cli 库暴露的公开模块**（docs.rs 文档覆盖率 28.61%）：

| 模块 | 说明 | svcmgr 可复用性 |
|------|------|----------------|
| `supervisor` | 进程监控器，含 IPC 调度、文件监听、重试退避、自动停止、生命周期管理 | **核心复用** |
| `daemon` | Daemon 数据结构（`Daemon`、`RunOptions`）和 ID 验证 | **直接使用** |
| `daemon_list` | Daemon 列表管理 | **直接使用** |
| `pitchfork_toml` | 配置文件解析（`PitchforkToml`、`PitchforkTomlDaemon`、`PitchforkTomlCron`、`Retry`） | **参考或复用** |
| `procs` | 进程管理（启动/停止/信号） | **核心复用** |
| `web` | 内置 Web Dashboard（axum） | **参考架构** |
| `ipc` | 进程间通信 | 可选 |
| `state_file` | 状态持久化 | 可选 |
| `watch_files` | 文件变更监听 | 可选 |
| `boot_manager` | 开机自启管理 | 不适用（Docker 场景） |

**内嵌方案**：

```toml
# Cargo.toml
[dependencies]
pitchfork-cli = { version = "1.6", default-features = false }
```

```rust
// 直接使用 pitchfork 的进程管理能力
use pitchfork_cli::supervisor::Supervisor;
use pitchfork_cli::daemon::{Daemon, RunOptions};
use pitchfork_cli::pitchfork_toml::PitchforkTomlDaemon;
```

**优势**：
- 零外部二进制依赖，单二进制部署
- 直接调用 Rust API，无 CLI 解析开销和格式兼容性问题
- 可复用 pitchfork 成熟的进程管理、重试退避、cron 调度逻辑
- 与 svcmgr 共享 tokio runtime，资源效率更高

**风险与注意事项**：
- pitchfork 库 API 文档覆盖率低（28.61%），部分内部实现可能不稳定
- pitchfork 版本更新可能导致内部 API 破坏性变更（需 pin 版本 + 跟踪 changelog）
- 部分模块（如 `boot_manager`、`tui`）不适用于 Docker 容器场景
- 建议通过 trait 封装 pitchfork API，保留替换为自实现的灵活性

### 3.4 pitchfork 与 svcmgr 的关系

pitchfork 解决了 mise 缺乏的进程管理能力：

- pitchfork 提供 `[lib]` 库接口，**可以作为 Rust 依赖内嵌到 svcmgr**
- svcmgr 可直接复用 pitchfork 的 `supervisor`、`daemon`、`procs` 模块
- pitchfork 没有 **Git 配置版本化**、**cloudflare 隧道** 等功能，这些仍需 svcmgr 自行实现
- pitchfork 的 `web` 模块可作为 svcmgr Web Dashboard 的参考

---

## 4. 可行性分析

### 4.1 可以直接复用 mise 的部分

| 能力 | 当前实现 | mise 替代方案 | 可行性 |
|------|----------|--------------|--------|
| T03 依赖管理 | `MiseManager` 封装 mise CLI | 内嵌 mise 库调用（见 §4.4） | **完全可行** |
| T04 全局任务 | `MiseManager.run_mise(&["run", ...])` | 内嵌 mise 库调用任务执行 | **完全可行** |
| T05 环境变量 | `MiseManager` 读写 .mise.toml `[env]` | 内嵌 mise 库读取环境变量 | **完全可行** |

### 4.2 需要 svcmgr 自行实现的部分

| 能力 | 原因 |
|------|------|
| **多任务调度引擎** | mise 不支持后台常驻/定时/事件触发 |
| **子进程管理** | 需要进程组管理、graceful shutdown、资源限制 |
| **配置文件 Git 版本化** | mise 无此能力，需自行实现 staging → commit → rollback |
| **Web 服务 + 代理** | mise 无 HTTP 服务能力，使用内置 HTTP 代理 |
| **Cloudflare 隧道管理** | 需要封装 cloudflared CLI |

### 4.3 结论

> **可以基于 mise 重构**，但 mise 只能覆盖**依赖管理、环境变量、任务定义**三个维度。svcmgr 仍需自行实现进程管理、调度引擎、Web 服务、代理管理等核心能力。mise 的定位是**配置声明和任务定义的基础设施**，svcmgr 在此之上构建运行时管理层。

### 4.4 mise 内嵌可行性分析

#### mise 库状态

mise 的 `Cargo.toml` 中**仅定义了 `[[bin]]`，没有 `[lib]` 段**。这意味着 mise 是一个纯 CLI 二进制工具，**不能直接作为 Rust 库依赖内嵌**。

```toml
# mise/Cargo.toml（摘录）
[[bin]]
name = "mise"
path = "src/main.rs"
# 没有 [lib] 段
```

mise 的代码规模为 ~86K SLoC，内部模块（`config`、`toolset`、`task`、`env` 等）虽然功能丰富，但：
- 未发布为独立的库 crate
- 内部 API 高度耦合 CLI 逻辑（如 `clap` 参数解析、TUI 交互等）
- 不适合直接 fork 拆分为库（维护成本极高）

#### 推荐方案：配置文件驱动 + 选择性 API 调用

由于 mise 不提供库 API，svcmgr 与 mise 的集成应采用以下分层策略：

| 层级 | 交互方式 | 说明 |
|------|----------|------|
| **配置层（主要）** | 直接解析 mise TOML 配置文件 | svcmgr 用 `toml` crate 解析 `[tools]`/`[tasks]`/`[env]` 段，获取任务命令、环境变量、工具定义 |
| **执行层（必要时）** | 调用 mise 子进程 | 仅在需要 mise 特有运行时能力时调用（如 `mise install` 安装工具、`mise env --json` 获取解析后的环境变量） |
| **观察层（可选）** | MCP 接口 | mise v2026.2.16+ 提供 Model Context Protocol 接口（如 `run_task`），提供比 CLI 更稳定的程序化交互 |

**核心原则**：能通过配置文件获取的信息，不调用 mise 进程；必须调用 mise 进程的场景，通过 Port-Adapter 抽象层隔离。

```rust
/// 配置文件驱动：直接解析 mise.toml 获取任务命令
fn get_task_command_from_config(config: &toml::Value, task_name: &str) -> Option<String> {
    config.get("tasks")
        .and_then(|t| t.get(task_name))
        .and_then(|t| t.get("run"))
        .and_then(|v| v.as_str())
        .map(String::from)
}

/// 必要时调用 mise 进程（如工具安装）
async fn install_tool(tool: &str, version: &str) -> Result<()> {
    // 通过 Port trait 抽象，实际调用 mise install
    let adapter = adapter_factory.create();
    adapter.install(tool, version).await
}
```

#### pitchfork 内嵌方案（对比）

与 mise 不同，pitchfork **提供 `[lib]` 段**，可直接作为 Rust 库依赖内嵌（详见 §3.3）。因此进程管理相关能力可通过 `pitchfork-cli` crate 内嵌实现，无需调用外部二进制。

| 组件 | 集成方式 | 原因 |
|------|----------|------|
| **mise** | 配置文件解析 + 必要时子进程调用 | 无 `[lib]`，纯 CLI 工具 |
| **pitchfork** | Rust 库依赖内嵌 | 提供 `[lib]`，暴露 20+ 公开模块 |

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
│  ├── procs 模块 → 进程启停信号                            │
│  └── pitchfork_toml 模块 → 配置解析参考                   │
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
| T07 | 进程管理 | pitchfork 库内嵌 + cgroups | **重构** — 内嵌 pitchfork 库，cgroups 资源限制（可关闭） |
| T08 | 隧道管理 | cloudflared | 保留 |
| T09 | 反向代理 | 内置 HTTP 代理 | **变更** — 从 nginx 改为内置 HTTP 代理，不依赖外部组件 |

---

## 6. mise 解耦架构设计

mise 采用 CalVer 版本号（如 `v2026.2.17`），迭代频繁（近乎每周发布），且历史上有过多次配置格式调整（如 `.mise.toml` → `mise.toml`、`task_*` 平铺设置合并为 `task.*` 嵌套结构）。svcmgr 深度依赖 mise 的配置格式和 CLI 接口，必须在架构层面做好解耦设计，以跟随 mise 的演进而不被破坏。

### 6.1 核心设计原则

| 原则 | 说明 |
|------|------|
| **面向接口而非实现** | svcmgr 内部通过 trait 抽象 mise 的每项能力，不直接耦合 CLI 命令字符串或 TOML 段名 |
| **适配器隔离** | 所有 mise 交互集中在独立的适配器模块（`adapters/mise/`），业务逻辑层不直接调用 mise |
| **配置分层** | svcmgr 自有配置（`x-` 段或独立文件）与 mise 原生配置在解析层分离 |
| **版本感知** | 运行时检测 mise 版本，根据版本选择兼容的交互策略 |
| **优雅降级** | 当 mise 行为变化导致特定功能不可用时，降级到备选方案而非崩溃 |

### 6.2 适配器层架构（Anti-Corruption Layer）

采用**端口-适配器（Port-Adapter）**模式，在 svcmgr 核心与 mise 之间建立防腐层：

```
┌─────────────────────────────────────────────────────────────┐
│                    svcmgr 核心业务逻辑                        │
│  （调度引擎、进程管理、Web 服务、配置管理）                      │
├──────────────────────┬──────────────────────────────────────┤
│      Port 层         │  Rust trait 定义（纯接口契约）          │
│  ┌─────────────────┐ │  ┌──────────────────────────────┐    │
│  │ DependencyPort  │ │  │ fn install(tool, ver)         │    │
│  │ TaskPort        │ │  │ fn run_task(name, args)       │    │
│  │ EnvPort         │ │  │ fn get_env() -> HashMap       │    │
│  │ ConfigPort      │ │  │ fn list_config_files()        │    │
│  └─────────────────┘ │  └──────────────────────────────┘    │
├──────────────────────┴──────────────────────────────────────┤
│      Adapter 层（adapters/mise/）                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │
│  │ MiseV2026    │  │ MiseV2025    │  │ MockAdapter  │       │
│  │ Adapter      │  │ Adapter      │  │ (测试用)      │       │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘       │
│         │                 │                 │               │
│  ┌──────┴─────────────────┴─────────────────┴──────┐        │
│  │          AdapterFactory（版本检测 + 路由）         │        │
│  └─────────────────────────────────────────────────┘        │
├─────────────────────────────────────────────────────────────┤
│  mise CLI / mise 配置文件 / mise 环境变量                      │
└─────────────────────────────────────────────────────────────┘
```

### 6.3 Port 接口定义

为 mise 的三大核心能力各定义一个 Port trait：

```rust
/// 依赖管理端口
#[async_trait]
pub trait DependencyPort: Send + Sync {
    /// 安装指定工具和版本
    async fn install(&self, tool: &str, version: &str) -> Result<()>;
    /// 列出已安装的工具
    async fn list_installed(&self) -> Result<Vec<ToolInfo>>;
    /// 设置当前目录使用的工具版本
    async fn use_tool(&self, tool: &str, version: &str) -> Result<()>;
    /// 移除工具
    async fn remove(&self, tool: &str, version: &str) -> Result<()>;
    /// 获取 mise 版本信息（用于兼容性检测）
    fn mise_version(&self) -> &MiseVersion;
}

/// 任务管理端口
#[async_trait]
pub trait TaskPort: Send + Sync {
    /// 运行指定任务（一次性前台执行）
    async fn run_task(&self, name: &str, args: &[String]) -> Result<TaskOutput>;
    /// 获取任务定义（从 mise 配置中读取 run 命令）
    async fn get_task_command(&self, name: &str) -> Result<TaskCommand>;
    /// 列出所有任务
    async fn list_tasks(&self) -> Result<Vec<TaskInfo>>;
}

/// 环境变量端口
#[async_trait]
pub trait EnvPort: Send + Sync {
    /// 获取 mise 解析后的完整环境变量
    async fn get_env(&self) -> Result<HashMap<String, String>>;
    /// 获取指定目录下的环境变量
    async fn get_env_for_dir(&self, dir: &Path) -> Result<HashMap<String, String>>;
}

/// 配置文件端口
#[async_trait]
pub trait ConfigPort: Send + Sync {
    /// 获取 mise 当前加载的配置文件列表（按优先级排序）
    async fn list_config_files(&self) -> Result<Vec<PathBuf>>;
    /// 读取指定配置文件的原始 TOML
    async fn read_config(&self, path: &Path) -> Result<toml::Value>;
    /// 写入配置文件（仅写 mise 原生段）
    async fn write_config(&self, path: &Path, value: &toml::Value) -> Result<()>;
}
```

**设计要点**：
- Port trait 不暴露任何 mise 特有的概念（如 CLI 参数格式、配置段名称）
- `TaskPort::get_task_command()` 返回解析后的命令而非 `mise run` 的封装，使进程管理器可直接 spawn
- `MiseVersion` 结构体用于运行时兼容性检测

### 6.4 版本检测与兼容策略

```rust
/// mise 版本信息
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MiseVersion {
    pub year: u16,    // e.g. 2026
    pub minor: u16,   // e.g. 2
    pub patch: u16,   // e.g. 17
}

impl MiseVersion {
    /// 从 `mise --version` 输出解析
    pub fn detect() -> Result<Self> { /* ... */ }

    /// 检查是否支持特定特性
    pub fn supports(&self, feature: MiseFeature) -> bool {
        match feature {
            MiseFeature::ConfD       => self >= &Self::new(2024, 12, 0),
            MiseFeature::TaskDepends => self >= &Self::new(2024, 1, 0),
            MiseFeature::Lockfiles   => self >= &Self::new(2026, 2, 0),
            MiseFeature::McpRunTask  => self >= &Self::new(2026, 2, 16),
            // ...
        }
    }
}

/// mise 特性枚举
/// 
/// **用途说明**：MiseFeature 用于运行时检测当前 mise 版本是否支持特定能力。
/// svcmgr 启动时会调用 `mise --version` 获取当前版本，然后通过
/// `MiseVersion::supports(feature)` 判断是否可以使用某个特性。
/// 这是解耦架构的核心机制：根据版本自动选择不同的交互策略，
/// 而不是硬编码某个版本的行为。
/// 
/// 示例：
/// ```rust
/// let version = MiseVersion::detect()?;
/// if version.supports(MiseFeature::McpRunTask) {
///     // 使用 MCP 接口调用任务（更稳定）
/// } else {
///     // 回退到 CLI 调用 mise run
/// }
/// ```
pub enum MiseFeature {
    ConfD,          // conf.d 目录支持（用于多配置文件加载）
    TaskDepends,    // 任务依赖（用于确定任务执行顺序）
    Lockfiles,      // 锁文件稳定版（用于确定性构建）
    McpRunTask,     // MCP run_task 工具（用于程序化任务调用替代 CLI）
    // 随 mise 版本增加新特性
}
```

**兼容策略矩阵**：

| mise 版本范围 | 策略 | 说明 |
|--------------|------|------|
| < 最低支持版本 | **拒绝启动** | 给出明确错误信息和升级指引 |
| 最低版本 ~ 推荐版本 | **兼容模式** | 关闭依赖新特性的功能，使用备选实现 |
| 推荐版本 ~ 当前最新 | **完整模式** | 所有功能可用 |
| > 已知最新版本 | **乐观模式** | 假设向后兼容，记录警告日志 |

svcmgr 应在启动时执行 `mise --version` 检测，并将版本信息注入到 `AdapterFactory`，由工厂方法选择对应的 Adapter 实现。

### 6.5 配置隔离策略

mise 对未知 TOML 段的行为是当前架构中最大的耦合风险点。mise 当前会对未知字段发出 WARN 级别日志（如 `unknown field in xxx.toml: xxx`），未来版本可能拒绝加载。

#### 采用方案：独立配置文件，存放于 mise 配置目录下

svcmgr 配置与 mise 配置物理分离，但**存放在同一父目录**下，便于统一管理和 Git 版本化：

```
.config/mise/                           # mise 配置目录
├── config.toml                        # 纯 mise 配置（tools, env, tasks）
├── conf.d/
│   ├── 00-base.toml                   # 纯 mise 配置
│   └── ...                            # 纯 mise 配置
└── svcmgr/                            # svcmgr 配置（与 mise 配置同级目录）
    ├── config.toml                    # svcmgr 核心配置（服务定义、功能开关等）
    └── conf.d/
        ├── services.toml              # 服务定义
        └── local.toml                 # 本地覆盖
```

**优点**：
- 完全消除 mise 未知段警告/报错风险（svcmgr 配置不会被 mise 解析）
- svcmgr 配置格式独立演进，不受 mise 约束
- 两套配置在同一目录下，便于 Git 版本化管理
- 清晰的职责分离

**svcmgr 配置文件不再使用 `x-` 前缀**（因为已是独立文件，无需区分）：

```toml
# .config/mise/svcmgr/config.toml
[services.api]
task = "api-start"           # 引用 mise 任务名
restart = "always"
http_ports = { web = 3000 }

[features]
web_ui = true
proxy = true
resource_limits = true       # cgroups 资源限制功能开关
```

svcmgr 启动时：
1. 读取 `.config/mise/svcmgr/config.toml` 获取自身配置
2. 解析 `.config/mise/config.toml` 和 `conf.d/*.toml` 获取 mise 任务/工具/环境变量定义
3. 将两者关联（svcmgr 服务引用 mise 任务名）后驱动调度引擎

**配置文件之间的引用关系**：

```
.config/mise/svcmgr/config.toml         .config/mise/config.toml
┌────────────────────┐                 ┌────────────────────┐
│ [services.api]     │ ──引用任务名──→  │ [tasks.api-start]  │
│ task = "api-start" │                 │ run = "node ..."   │
│ restart = "always" │                 │ env = { ... }      │
└────────────────────┘                 └────────────────────┘
```

#### 备选方案：共享配置文件 + x- 前缀

如果未来需要更简化的配置体验（单文件配置），可保留 `x-` 前缀作为备选：

```toml
# mise.toml — mise 和 svcmgr 共享（备选模式）
[tools]
node = "22"

[tasks.api-start]
run = "node server.js"

[x-services.api]
task = "api-start"
restart = "always"
```

此方案仅作为备选，默认不推荐。

#### 配置拆分的管理难度评估

独立配置文件引入的额外复杂度：

| 方面 | 影响 | 缓解措施 |
|------|------|----------|
| 任务名一致性 | svcmgr 引用的任务名必须在 mise 中存在 | 启动时校验，不存在的任务名给出明确错误 |
| 文件数量 | 从 1 个文件变为 2+ 个文件 | 通过 `svcmgr init` CLI 自动生成模板 |
| Git 版本化 | 两套文件需同时纳入 Git | 都在 `.config/mise/` 目录下，自然包含 |
| 认知负担 | 用户需理解两套配置的职责 | 清晰的文档 + 示例配置 |

**综合评估**：拆分配置的管理难度可控，且完全消除了 mise 未知段警告/报错风险。配合降级机制（§6.8），当检测到 mise 对未知段行为变化时可自动调整。

### 6.6 mise 交互策略

由于 mise 不提供 Rust 库 API（见 §4.4），svcmgr 与 mise 的交互采用**配置文件驱动为主、必要时子进程调用为辅**的分层策略：

#### 层级 1：配置文件直接解析（主要交互方式）

```rust
/// 直接解析 mise.toml 获取任务定义、工具定义、环境变量
fn parse_mise_config(path: &Path) -> Result<MiseConfig> {
    let content = std::fs::read_to_string(path)?;
    let value: toml::Value = toml::from_str(&content)?;
    
    Ok(MiseConfig {
        tools: parse_tools_section(&value),
        tasks: parse_tasks_section(&value),
        env: parse_env_section(&value),
    })
}

/// 从解析后的配置中获取任务命令（用于直接 spawn，而非通过 mise run）
fn get_task_command(config: &MiseConfig, task_name: &str) -> Option<TaskCommand> {
    config.tasks.get(task_name).map(|t| TaskCommand {
        command: t.run.clone(),
        env: t.env.clone(),
        dir: t.dir.clone(),
    })
}
```

这是 svcmgr 获取任务命令、环境变量、工具定义的主要方式。直接解析 TOML 配置文件，无需启动 mise 进程。

#### 层级 2：mise 子进程调用（仅必要时）

仅在需要 mise 特有运行时能力时调用 mise 子进程：

```rust
/// mise CLI 命令构造器（仅用于必须调用 mise 进程的场景）
pub struct MiseCommand {
    version: MiseVersion,
}

impl MiseCommand {
    /// 安装工具（必须调用 mise，因为工具安装逻辑在 mise 内部）
    pub fn install(&self, tool: &str, version: &str) -> Command {
        let mut cmd = Command::new("mise");
        cmd.arg("install").arg(format!("{}@{}", tool, version));
        cmd
    }

    /// 获取解析后的环境变量（mise 支持模板、_.file 等复杂解析，直接解析 TOML 不足时使用）
    pub fn env_json(&self) -> Command {
        let mut cmd = Command::new("mise");
        cmd.arg("env").arg("--json");
        cmd
    }

    /// 工具卸载
    pub fn uninstall(&self, tool: &str, version: &str) -> Command {
        let mut cmd = Command::new("mise");
        cmd.arg("uninstall").arg(format!("{}@{}", tool, version));
        cmd
    }

    /// 激活工具版本（mise use 设置当前目录工具版本）
    pub fn activate_tool(&self, tool: &str, version: &str) -> Command {
        let mut cmd = Command::new("mise");
        cmd.arg("use").arg(format!("{}@{}", tool, version));
        cmd
    }

    /// 查询可安装的远程版本
    pub fn ls_remote(&self, tool: &str) -> Command {
        let mut cmd = Command::new("mise");
        cmd.arg("ls-remote").arg(tool);
        cmd
    }

    /// 获取已安装工具列表
    pub fn list_installed(&self) -> Command {
        let mut cmd = Command::new("mise");
        cmd.arg("ls").arg("--json");
        cmd
    }

    /// 获取插件列表（mise 支持通过插件安装工具）
    pub fn list_plugins(&self) -> Command {
        let mut cmd = Command::new("mise");
        cmd.arg("plugins").arg("ls");
        cmd
    }
}
```

**关键设计**：
- **配置文件解析优先**：能通过解析 TOML 获取的信息（任务命令、基本环境变量、工具定义），不调用 mise 进程
- **子进程调用仅用于**：工具安装/卸载/激活、复杂环境变量解析、插件管理、远程版本查询
- 所有 mise CLI 调用集中在 `MiseCommand`，当 mise 命令格式变化时只需修改此处
- 通过 `MiseVersion` 可在构造命令时选择不同参数格式

#### 层级 3：MCP 接口（可选、未来）

mise v2026.2.16+ 提供 Model Context Protocol 接口（如 `run_task`），提供比 CLI 更稳定的程序化交互。可作为未来优化方向。

### 6.7 配置格式适配

mise 配置格式可能发生的变化类型及应对：

| 变化类型 | 示例 | 应对策略 |
|----------|------|----------|
| **段名重命名** | `[plugins]` → `[tools]`（历史上已发生） | Adapter 中映射新旧段名 |
| **字段合并/拆分** | `task_*` → `task.*` 嵌套（v2026.2.17） | 版本化解析逻辑 |
| **新增必填字段** | 假设未来 `[tasks]` 需要 `shell` 字段 | 检测并自动填充默认值 |
| **废弃字段** | 旧字段标记 deprecated | 日志警告 + 自动迁移 |
| **配置文件路径变化** | `.mise.toml` → `mise.toml` | 检测两种路径，按 mise 版本选择 |
| **新增 x- 冲突** | 假设 mise 官方占用 `x-` 前缀 | 切换到独立配置文件策略 |

配置解析流程：

```
mise.toml / svcmgr.toml
       │
       ▼
┌─────────────────┐
│  TOML Raw Parse │  ← toml crate 解析为 toml::Value
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Version Router │  ← 根据 MiseVersion 选择解析器
└────────┬────────┘
         │
    ┌────┴────┐
    ▼         ▼
┌────────┐ ┌────────┐
│ V2026  │ │ V2025  │  ← 版本化解析器
│ Parser │ │ Parser │
└────┬───┘ └────┬───┘
     │          │
     ▼          ▼
┌─────────────────┐
│  Unified Config │  ← svcmgr 内部统一配置模型
│  Model          │
└─────────────────┘
```

### 6.8 优雅降级机制

当 mise 行为变化导致某些操作失败时，svcmgr 应能降级而非崩溃：

| 场景 | 降级策略 |
|------|----------|
| `mise env --json` 输出格式变化 | 回退到 `mise env` 文本输出解析 |
| `mise cfg` 不可用 | 手动扫描已知配置文件路径 |
| `mise tasks info` 失败 | 直接从 TOML 文件解析 `[tasks]` 段 |
| `mise install` 参数变化 | 尝试 `mise use` 作为备选 |
| `x-` 段导致 mise 报错 | 自动切换到独立配置文件模式 |
| mise 二进制不存在 | 提示用户安装，仅启动 svcmgr 核心功能（不含依赖管理） |

降级实现模式：

```rust
async fn get_env_with_fallback(&self) -> Result<HashMap<String, String>> {
    // 优先：结构化 JSON 输出
    match self.get_env_json().await {
        Ok(env) => return Ok(env),
        Err(e) => {
            tracing::warn!("mise env --json failed, falling back: {e}");
        }
    }
    // 降级：文本输出解析
    match self.get_env_text().await {
        Ok(env) => return Ok(env),
        Err(e) => {
            tracing::warn!("mise env text parse failed, falling back: {e}");
        }
    }
    // 最终降级：直接解析配置文件中的 [env] 段
    self.parse_env_from_config().await
}
```

### 6.9 测试策略

为确保 mise 版本兼容性，建议多层测试：

#### 单元测试：MockAdapter

```rust
/// 测试用 Mock 适配器，不依赖真实 mise 安装
pub struct MockMiseAdapter {
    tools: HashMap<String, String>,
    env: HashMap<String, String>,
    tasks: HashMap<String, TaskCommand>,
}

impl DependencyPort for MockMiseAdapter { /* ... */ }
impl TaskPort for MockMiseAdapter { /* ... */ }
impl EnvPort for MockMiseAdapter { /* ... */ }
```

核心业务逻辑的单元测试全部使用 `MockMiseAdapter`，确保测试速度和稳定性。

#### 集成测试：多版本矩阵

```yaml
# CI 矩阵测试
strategy:
  matrix:
    mise-version:
      - "latest"         # 最新版
      - "2026.2.0"       # 当前推荐版本
      - "2025.12.0"      # 最低支持版本
```

#### 契约测试：版本兼容性检查

```rust
#[test]
fn test_mise_cli_contract() {
    // 验证当前 mise 版本的 CLI 输出格式是否符合预期
    let output = Command::new("mise").arg("env").arg("--json").output().unwrap();
    let env: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(env.is_object(), "mise env --json should return JSON object");
}

#[test]
fn test_mise_config_accepts_unknown_sections() {
    // 验证当前 mise 版本是否接受 x- 前缀段（不报错退出）
    let config = "[tools]\nnode = \"22\"\n[x-services.test]\ntask = \"test\"\n";
    // 写入临时文件，运行 mise cfg，检查 exit code
}
```

### 6.10 升级与迁移流程

当 mise 发布新版本时，svcmgr 的响应流程：

```
mise 新版本发布
      │
      ▼
┌──────────────────┐
│ CI 矩阵测试触发   │ ← GitHub Actions cron / mise release webhook
└────────┬─────────┘
         │
    ┌────┴────┐
    │ 测试通过？│
    └────┬────┘
     是  │  否
     │   │
     │   ▼
     │  ┌─────────────────────┐
     │  │ 创建 Issue 标记兼容性 │
     │  │ 问题并开始适配        │
     │  └──────────┬──────────┘
     │             │
     │             ▼
     │  ┌─────────────────────┐
     │  │ 新增/修改 Adapter    │
     │  │ 更新 MiseFeature 枚举 │
     │  │ 更新版本兼容矩阵      │
     │  └──────────┬──────────┘
     │             │
     ▼             ▼
┌──────────────────────┐
│ 更新 svcmgr 最低/推荐│
│ mise 版本号          │
│ 发布 svcmgr 新版本   │
└──────────────────────┘
```

**版本兼容性声明**（在 svcmgr 配置/README 中维护）：

```toml
# svcmgr 内置版本兼容表
[mise-compat]
minimum = "2025.12.0"   # 最低支持版本
recommended = "2026.2.0" # 推荐版本
tested_up_to = "2026.2.17" # 已测试的最高版本
```

### 6.11 目录结构建议

```
src/
├── ports/                    # Port 接口定义（纯 trait）
│   ├── mod.rs
│   ├── dependency.rs         # DependencyPort trait
│   ├── task.rs               # TaskPort trait
│   ├── env.rs                # EnvPort trait
│   └── config.rs             # ConfigPort trait
├── adapters/                 # Adapter 实现
│   ├── mod.rs
│   ├── mise/
│   │   ├── mod.rs            # AdapterFactory + MiseVersion
│   │   ├── command.rs        # MiseCommand 构造器
│   │   ├── v2026.rs          # 2026.x 版本适配器
│   │   ├── v2025.rs          # 2025.x 版本适配器（最低支持）
│   │   └── parser.rs         # 版本化配置解析
│   └── mock.rs               # 测试用 Mock 适配器
├── core/                     # 核心业务逻辑
│   ├── scheduler.rs          # 调度引擎（通过 Port 调用 mise）
│   ├── process.rs            # 进程管理
│   ├── config_manager.rs     # 配置管理 + Git 版本化
│   └── ...
└── ...
```

### 6.12 关键问题补充

#### P8: mise CLI 输出格式稳定性

**问题**：svcmgr 通过解析 mise CLI 的 stdout/stderr 获取信息，但 mise 未承诺 CLI 输出格式的稳定性。例如 `mise env --json` 的 JSON 结构、`mise tasks info` 的文本格式都可能变化。

**建议**：
- 优先使用结构化输出（`--json` 标志），其稳定性通常高于文本输出
- 对解析结果做宽松匹配（容忍新增字段，不因未知字段报错）
- 考虑通过 mise 的 MCP（Model Context Protocol）接口交互，该接口（v2026.2.16+ 新增 `run_task`）可能提供更稳定的程序化调用方式
- 为每种 CLI 输出维护契约测试

#### P9: mise 自身升级的原子性

**问题**：如果用 mise 管理自身的依赖，mise 在升级自身时可能导致 svcmgr 依赖的 mise 版本不可预期地变化。

**建议**：
- svcmgr 应在 `[mise-compat]` 中 pin mise 版本范围
- 启动时检测 mise 版本是否在兼容范围内，不在范围则发出警告
- mise 升级应通过 svcmgr 的配置管理流程（Git staging → apply → commit），而非自动升级

---

## 7. 配置文件设计

### 7.1 配置文件层级（mise 与 svcmgr 分离）

为了避免与 mise 未来配置格式/字段演进产生冲突，svcmgr **不再把自定义段落写入 `mise.toml`**，而是使用独立配置文件。

```
# mise 配置（由 mise 处理）
~/.config/mise/config.toml              # mise 全局配置
<repo>/mise.toml                        # 项目配置（可选）
.config/mise/conf.d/*.toml              # mise 场景配置（可选）

# svcmgr 配置（由 svcmgr 处理）
~/.config/mise/svcmgr/config.toml       # svcmgr 全局配置（建议）
~/.config/mise/svcmgr/conf.d/*.toml     # svcmgr 场景配置（可选，按字母序合并）
```

### 7.2 svcmgr 配置格式

#### 7.2.1 服务/定时任务定义 `[services.<name>]`

同一个结构既可表示「长期运行服务」，也可表示「定时任务」：
- **服务**：`enable=true` + `restart` 策略
- **定时任务**：配置 `cron`（可与 `enable` 并存，但语义是「按计划触发一次性任务」）

```toml
[services.web-api]
task = "api:start"           # mise 任务名称（或 task alias）
enable = true
restart = "always"           # no | always | on-failure
restart_delay = "2s"         # 指数退避的初始值
restart_limit = 10            # 最大重启次数（窗口内）
restart_window = "60s"       # 统计窗口
stop_timeout = "10s"
workdir = "/app"
timeout = "0"                # 0 表示不超时（长期服务）
http_ports = { web = 8080 }

# cgroups v2 资源限制（可选；features.resource_limits=true 且检测到可用时才生效）
cpu_max_percent = 50          # 50 = 50% CPU
memory_max = "512m"          # 物理内存上限（memory.max）
pids_max = 100

[services.cleanup]
task = "cleanup"
cron = "0 */6 * * *"         # cron 表达式 → 周期触发
workdir = "/app"
timeout = "300s"
```

#### 7.2.2 配置目录管理 `[configurations.<name>]`

```toml
[configurations.app]
path = ".config/app"         # 受 Git 版本化管理的目录

[configurations.mise]
path = ".config/mise"        # 项目 mise 配置（可选纳入版本化）
```

#### 7.2.3 功能开关 `[features]`

```toml
[features]
web_ui = true
proxy = true
tunnel = false
scheduler = true
git_versioning = true
resource_limits = true
```

等价的环境变量控制：

```toml
[env]
SVCMGR_FEATURE_WEB_UI = "1"
SVCMGR_FEATURE_PROXY = "1"
SVCMGR_FEATURE_TUNNEL = "0"
```

### 7.3 配置文件解析流程

```
          ┌──────────────────────┐           ┌────────────────────────────┐
          │ mise 配置文件集        │           │ svcmgr 配置文件集            │
          │ (mise.toml/config.toml)│           │ (~/.config/mise/svcmgr/*.toml)│
          └───────────┬──────────┘           └──────────────┬─────────────┘
                      │                                      │
               Mise Adapter/Port                       svcmgr TOML parser
                      │                                      │
                      └───────────────┬──────────────────────┘
                                      │
                              运行时配置对象
                         （任务命令 + 环境 + 服务/触发器）
```

---

## 8. 多任务调度引擎设计

### 8.1 概述

调度引擎是新架构的核心组件，负责管理所有任务的生命周期。它是一个纯内存运行时，配置来自解析后的 TOML 配置文件。

### 8.2 触发器类型

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

### 8.3 任务定义

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

### 8.4 调度引擎核心循环

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

### 8.5 与 mise 的集成

```
用户定义 mise 配置（`mise.toml` 或 `~/.config/mise/config.toml`）：

```toml
[tasks.api-start]
run = "node server.js"
env = { PORT = "3000" }
```

用户定义 svcmgr 配置（`.config/mise/svcmgr/config.toml`）：

```toml
[services.api]
task = "api-start"       # 引用 mise 任务
enable = true
restart = "always"
http_ports = { web = 3000 }
```

调度引擎的工作流：
1. 解析 `[services.api]`，发现 `task = "api-start"` + `enable = true`
2. 注册 `Event(SystemInit)` 触发器（因为 `enable = true` 意味着开机启动）
3. 系统初始化时触发 `SystemInit` 事件
4. 引擎从 mise 配置中读取 `[tasks.api-start].run`，直接 spawn 任务（必要时才调用 `mise run`）
5. 进程管理器接管子进程（pitchfork/setsid、日志捕获、资源限制）
6. 进程退出时触发 `TaskExit` 事件 → 根据 `restart = "always"` 自动重启

### 8.6 与当前实现的差异

| 维度 | 当前（supervisor.rs） | 新设计（调度引擎） |
|------|----------------------|-------------------|
| 服务管理 | SupervisorAtom trait + SupervisorManager | SchedulerEngine + 事件触发器 |
| 定时任务 | SchedulerAtom trait（CRUD，不执行） | SchedulerEngine + Cron 触发器 |
| 任务执行 | 直接 `tokio::process::Command` | 通过 `mise run` 或直接 `Command` |
| 配置来源 | 独立 TOML 文件（每个服务一个） | svcmgr 独立配置文件（services 段）+ mise 配置文件（tasks/env/tools） |
| 事件系统 | 无 | 内置事件总线 |
| 统一性 | 服务和定时任务分离管理 | 统一调度，通过触发器区分 |

---

## 9. 子进程管理与资源限制

### 9.1 进程管理

进程管理采用双层方案：

1. **pitchfork 库内嵌**：通过 `pitchfork-cli` crate 的 `supervisor`、`daemon`、`procs` 模块实现进程监控、重试退避、生命周期管理（见 §3.3）
2. **进程组隔离**：使用 `setsid()` 创建进程组，`kill(-pgid, sig)` 发送信号到整个进程树（当前 `supervisor.rs` 已实现，可复用）

### 9.2 Docker 非特权容器下的资源限制

#### 采用方案：cgroups v2（功能开关可关闭）

资源限制**统一使用 cgroups v2**，不使用 setrlimit。通过功能开关控制，关闭后忽略资源限制的配置。

**原因**：
- setrlimit 的局限性太大（`RLIMIT_AS` 不等于物理内存，`RLIMIT_NPROC` 是 per-UID 而非 per-process）
- cgroups v2 提供精确的 CPU/内存/IO 限制，且是 per-process-group
- Docker 默认使用 `--cgroupns=private`，容器内可创建子 cgroup

**cgroups v2 可限制的资源**：

| 资源 | cgroup 控制器 | 说明 |
|------|---------------|------|
| CPU 配额 | `cpu.max` | 精确的 CPU 时间配额（如 100000/100000 = 100% CPU） |
| 内存限制 | `memory.max` | 物理内存限制（非虚拟地址空间） |
| IO 带宽 | `io.max` | 磁盘 IO 限制 |
| 进程数 | `pids.max` | 精确的 per-cgroup 进程数限制 |

#### 9.2.1 实现方式

```rust
use std::path::PathBuf;
use std::fs;

/// cgroups v2 资源限制管理器
struct CgroupManager {
    base_path: PathBuf,  // /sys/fs/cgroup/svcmgr/
}

impl CgroupManager {
    /// 为服务创建独立的 cgroup
    fn create_cgroup(&self, service_name: &str) -> Result<PathBuf> {
        let cgroup_path = self.base_path.join(service_name);
        fs::create_dir_all(&cgroup_path)?;
        Ok(cgroup_path)
    }

    /// 设置 CPU 限制
    fn set_cpu_limit(&self, cgroup: &Path, cpu_percent: u32) -> Result<()> {
        // cpu.max 格式: "quota period"，如 "50000 100000" = 50% CPU
        let quota = cpu_percent * 1000;
        fs::write(cgroup.join("cpu.max"), format!("{} 100000", quota))?;
        Ok(())
    }

    /// 设置内存限制
    fn set_memory_limit(&self, cgroup: &Path, bytes: u64) -> Result<()> {
        fs::write(cgroup.join("memory.max"), bytes.to_string())?;
        Ok(())
    }

    /// 设置进程数限制
    fn set_pids_limit(&self, cgroup: &Path, max_pids: u32) -> Result<()> {
        fs::write(cgroup.join("pids.max"), max_pids.to_string())?;
        Ok(())
    }

    /// 将进程加入 cgroup
    fn add_process(&self, cgroup: &Path, pid: u32) -> Result<()> {
        fs::write(cgroup.join("cgroup.procs"), pid.to_string())?;
        Ok(())
    }

    /// 清理 cgroup（服务停止时）
    fn remove_cgroup(&self, service_name: &str) -> Result<()> {
        let cgroup_path = self.base_path.join(service_name);
        fs::remove_dir(&cgroup_path)?;
        Ok(())
    }
}
```

#### 9.2.2 功能开关控制

资源限制通过功能开关控制，关闭后忽略配置文件中的资源限制字段：

```toml
# svcmgr 配置
[features]
resource_limits = true    # 启用 cgroups 资源限制
```

```rust
// 启动时检查
if features.resource_limits {
    // 检查 cgroups v2 是否可用
    if !cgroup_available() {
        tracing::warn!("cgroups v2 not available, disabling resource limits");
        features.resource_limits = false;
    }
}

// 启动服务时
if features.resource_limits {
    let cgroup = cgroup_mgr.create_cgroup(&service.name)?;
    if let Some(cpu_percent) = service.cpu_max_percent {
        cgroup_mgr.set_cpu_limit(&cgroup, cpu_percent)?;
    }
    if let Some(mem_bytes) = service.memory_max {
        cgroup_mgr.set_memory_limit(&cgroup, mem_bytes)?;
    }
    if let Some(pids) = service.pids_max {
        cgroup_mgr.set_pids_limit(&cgroup, pids)?;
    }
    cgroup_mgr.add_process(&cgroup, child_pid)?;
} else {
    // 功能开关关闭，忽略资源限制配置
    tracing::debug!("resource_limits disabled, ignoring limits for {}", service.name);
}
```

#### 9.2.3 配置格式

```toml
# svcmgr 服务配置
[services.heavy-worker]
task = "worker:process"
cpu_max_percent = 50       # CPU 配额（%），cgroups cpu.max
memory_max = "512m"        # 物理内存限制，cgroups memory.max
pids_max = 50              # 最大进程数，cgroups pids.max
```

#### 9.2.4 注意事项

1. **需要 cgroup 写权限**：容器内需要对 `/sys/fs/cgroup/` 有写权限，Docker 默认 `--cgroupns=private` 通常满足
2. **功能开关控制**：关闭 `resource_limits` 后，配置中的 `cpu_max_percent`、`memory_max` 等字段被忽略，不会报错
3. **自动检测**：启动时自动检测 cgroups 是否可用，不可用时自动关闭功能开关并记录警告
4. **per-service 隔离**：每个服务创建独立的 cgroup，资源限制是 per-service 的（不同于 setrlimit 的 per-UID 局限）

---

## 10. Git 配置版本管理

### 10.1 核心流程

用户提出的配置生命周期（5 阶段）完全映射到 Git 操作：

```
阶段 1: 配置初始化
  └── git init + git add config.toml svcmgr/config.toml + git commit -m "init"
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

### 10.2 Git 仓库结构

```
~/.config/mise/                        # Git 仓库根目录（建议）
├── .git/                              # Git 数据
├── config.toml                        # mise 全局配置（可选纳入版本化）
├── conf.d/                            # mise 场景配置（可选）
│   └── ...
├── svcmgr/
│   ├── config.toml                    # svcmgr 配置
│   └── conf.d/                        # svcmgr 场景配置（可选）
│       └── ...
└── templates/                         # 配置模板（可选）
    └── cloudflare/
```

### 10.3 配置管理 API

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

### 10.4 事件集成

配置变更与事件系统深度集成：

| 事件 | 触发时机 | 典型用途 |
|------|----------|---------|
| `ConfigChanged { path }` | 配置文件 commit 成功后 | 重新加载服务、更新调度 |
| `ConfigStaged { path }` | 配置文件暂存后 | 预检查、dry-run |
| `ConfigRolledBack { commit }` | 配置回滚后 | 重启受影响的服务 |

内置事件处理链：

```
配置 commit → ConfigChanged 事件 → 
  → 调度引擎重新加载 svcmgr/config.toml 中的 [services] 段
  → 比较新旧配置 diff
  → 对变更的服务执行 restart/stop/start
```

---

## 11. Web 服务与代理设计

### 11.1 内置 Web 服务

svcmgr 内置 HTTP 服务器（建议使用 `axum` 框架），**不依赖 nginx 等外部组件**。提供：

```
路径                            功能
─────────────────────────────────────────
/web/*                         Web UI（前端应用）
/api/*                         管理 API（见 §13）
/services/{task}/{port_name}/* 反向代理到服务端口
```

### 11.2 反向代理机制

**采用方案 B：内置 HTTP 代理（axum/hyper）**

svcmgr 内置 HTTP 反向代理，零外部依赖，单二进制部署：

```rust
use axum::{Router, extract::Path};
use hyper::Client;

/// 反向代理 handler
async fn proxy_handler(
    Path((task, port_name)): Path<(String, String)>,
    req: axum::extract::Request,
) -> impl IntoResponse {
    // 1. 从配置中查找 task 对应的 port 映射
    let target_port = config.get_service_port(&task, &port_name)?;
    
    // 2. 构造目标 URL（去掉前缀路径，转发到 localhost:port）
    let target_url = format!("http://localhost:{}{}", target_port, remaining_path);
    
    // 3. 转发请求（去掉 /services/{task}/{port_name} 前缀和 Host 头）
    let mut forwarded_req = req;
    forwarded_req.headers_mut().remove("host");
    
    client.request(forwarded_req).await
}
```

**特性**：
- HTTP 端口转发（去掉子路径前缀和 Host 头）
- WebSocket 代理支持（通过 `axum` 的 WebSocket 升级）
- 不提供静态文件转发功能（不需要）
- 不依赖 nginx

### 11.3 代理路由配置

```toml
# svcmgr 服务配置
[services.api]
task = "api:start"
enable = true
http_ports = { web = 8080 }   # 端口名 = 端口号

[services.docs]
task = "docs:serve"
enable = true
http_ports = { site = 3000 }
```

路由映射：
- `/services/api/web/*` → `http://localhost:8080/*`（去掉前缀 `/services/api/web`，去掉 Host）
- `/services/docs/site/*` → `http://localhost:3000/*`

---

## 12. 功能开关机制

### 12.1 设计

功能开关通过两种方式控制：

**方式 1：配置文件**

```toml
[features]
web_ui = true
proxy = true
tunnel = false
scheduler = true
git_versioning = true
resource_limits = true
```

**方式 2：环境变量**（优先级高于配置文件）

```bash
SVCMGR_FEATURE_WEB_UI=1
SVCMGR_FEATURE_PROXY=0
SVCMGR_FEATURE_TUNNEL=1    # 覆盖配置文件中的 false
```

### 12.2 功能开关列表

| 开关名 | 默认值 | 说明 |
|--------|-------|------|
| `web_ui` | `true` | 启用 Web UI |
| `proxy` | `true` | 启用内置 HTTP 反向代理 |
| `tunnel` | `false` | 启用 Cloudflare 隧道管理（需要 cloudflared） |
| `scheduler` | `true` | 启用调度引擎（定时任务、事件触发） |
| `git_versioning` | `true` | 启用配置 Git 版本化 |
| `resource_limits` | `true` | 启用 cgroups 资源限制（不可用时自动关闭） |

### 12.3 实现

```rust
struct FeatureFlags {
    web_ui: bool,
    proxy: bool,
    tunnel: bool,
    scheduler: bool,
    git_versioning: bool,
    resource_limits: bool,
}

impl FeatureFlags {
    fn load(config: &TomlConfig, env: &HashMap<String, String>) -> Self {
        // 1. 从 [features] 段加载默认值
        // 2. 环境变量 SVCMGR_FEATURE_* 覆盖
    }
}
```

---

## 13. API 设计

### 13.1 API 路由

```
/web/*                                  Web UI
/api/v1/services                        GET    列出所有服务（services 段中 enable=true 的）
/api/v1/services?trigger=cron           GET    按触发器类型过滤（定时任务）
/api/v1/services?trigger=event          GET    按触发器类型过滤（事件触发）
/api/v1/services/{name}                 GET    获取服务详情
/api/v1/services/{name}/start           POST   启动服务
/api/v1/services/{name}/stop            POST   停止服务
/api/v1/services/{name}/restart         POST   重启服务
/api/v1/services/{name}/logs            GET    获取服务日志
/api/v1/tasks                           GET    列出所有 mise 任务（mise tasks 概念）
/api/v1/tasks/{name}                    GET    获取 mise 任务详情（命令、依赖、环境变量）
/api/v1/tasks/{name}/run                POST   手动运行 mise 任务
/api/v1/tools                           GET    列出工具（含已安装和未安装状态）
/api/v1/tools/{name}/install            POST   安装工具（mise install）
/api/v1/tools/{name}/activate           POST   激活工具版本（mise use）
/api/v1/tools/{name}/uninstall          POST   卸载工具（mise uninstall）
/api/v1/tools/{name}/versions           GET    查询可安装的远程版本（mise ls-remote）
/api/v1/tools/plugins                   GET    列出 mise 插件（mise plugins ls）
/api/v1/config                          GET    获取当前配置
/api/v1/config                          PUT    更新配置
/api/v1/config/stage                    POST   暂存配置变更
/api/v1/config/apply                    POST   应用（commit）配置
/api/v1/config/rollback                 POST   回滚配置
/api/v1/config/diff                     GET    查看配置 diff
/api/v1/config/log                      GET    查看配置历史
/api/v1/env                             GET    列出环境变量
/api/v1/env                             PUT    设置环境变量
/api/v1/features                        GET    获取功能开关状态
/api/v1/events                          GET    SSE 事件流
/services/{task}/{port_name}/*          代理   反向代理到服务端口
```

**说明**：
- `/api/v1/tasks` 是 **mise 任务**的概念（对应 `mise.toml` 中的 `[tasks]`），不是定时任务
- 定时任务/事件触发任务属于 svcmgr 的 `services` 配置，可通过 `/api/v1/services?trigger=cron|event` 过滤
- `/api/v1/tools` 的工具管理区分三类操作：安装（install）、激活（activate）、卸载（uninstall）
- 未安装的工具通过 `/api/v1/tools/{name}/versions` 查询可安装版本
- mise 插件通过 `/api/v1/tools/plugins` 展示（插件是安装工具的基础能力）

### 13.2 与现有 CLI 的映射

| 当前 CLI 命令 | 新 API | 变更说明 |
|--------------|--------|---------|
| `svcmgr setup` | 保留 CLI | 初始化 mise 环境 + git init 配置仓库 |
| `svcmgr run` | 保留 CLI | 启动 svcmgr 主进程（调度引擎 + Web 服务） |
| `svcmgr teardown` | 保留 CLI | 停止所有服务，清理 |
| `svcmgr service *` | `/api/v1/services/*` | 服务管理改为 API 驱动，CLI 调用 API |
| `svcmgr cron *` | `/api/v1/services?trigger=cron` | 定时任务通过 services API + 触发器过滤 |
| `svcmgr mise *` | `/api/v1/tools/*` + `/api/v1/env/*` | mise 管理改为 API 驱动 |
| `svcmgr tunnel *` | 按功能开关 tunnel 决定是否保留 | 可选 |
| `svcmgr config *` | `/api/v1/config/*` | 配置管理改为 API 驱动 |
| `svcmgr tty *` | 视需求保留或合并为服务 | TTY 本质是一个服务 |

---

## 14. 改造影响分析

### 14.1 保留的模块

| 模块 | 文件 | 原因 |
|------|------|------|
| Git 原子 | `atoms/git.rs` | 配置版本化核心，需增强 |
| 模板原子 | `atoms/template.rs` | 配置模板化，保留 |
| 隧道原子 | `atoms/tunnel.rs` | cloudflared 封装，按需保留 |
| 错误处理 | `error.rs` | 通用基础设施 |

### 14.2 需要重写的模块

| 模块 | 当前文件 | 变更 |
|------|---------|------|
| supervisor/scheduler | `atoms/supervisor.rs`（1950 行） | 拆分为调度引擎 + 进程管理器 |
| mise 管理器 | `atoms/mise.rs`（608 行） | 简化为配置文件读写，去掉 CLI 封装 |
| 代理管理 | `atoms/proxy.rs` | 从 nginx 管理改为内置 HTTP 代理（见 §11） |
| 所有 feature 模块 | `features/*.rs` | 合并到调度引擎配置驱动模型 |
| CLI | `cli/*.rs` | 简化，核心逻辑移到 API 层 |
| 配置 | `config.rs` | 重写为 svcmgr 独立配置 + mise 配置关联（见 §6.5、§7） |
| 主入口 | `main.rs` | 重写为调度引擎 + Web 服务启动 |

### 14.3 新增模块

| 模块 | 职责 |
|------|------|
| `engine/scheduler.rs` | 多任务调度引擎（触发器、事件总线） |
| `engine/process.rs` | 进程管理器（进程组、资源限制、日志） |
| `engine/events.rs` | 事件系统（EventBus、EventType） |
| `config/parser.rs` | svcmgr 配置解析器 + mise 配置关联解析 |
| `config/git.rs` | Git 版本化管理（staging/commit/rollback） |
| `web/server.rs` | HTTP 服务器（axum） |
| `web/api.rs` | REST API handlers |
| `web/proxy.rs` | 内置反向代理 |
| `web/static.rs` | 静态文件服务 |

### 14.4 依赖变更

| 当前依赖 | 状态 | 新增依赖 | 用途 |
|----------|------|---------|------|
| `clap` | 保留 | `axum` | Web 框架 |
| `tokio` | 保留 | `hyper` | HTTP 客户端（代理） |
| `serde` / `toml` | 保留 | `tower` | 中间件 |
| `git2` | 保留 | | 资源限制：cgroups v2 直接写 /sys/fs/cgroup（无需额外依赖） |
| `cron` | 保留 | `tower-http` | 静态文件、CORS |
| `chrono` | 保留 | | |
| `libc` | 保留 | | |
| `minijinja` | 保留 | | |
| `futures` | 保留 | | |
| `regex` | 保留 | | |
| `tracing` | 保留 | | |

---

## 15. 问题与风险

### 15.1 关键问题

#### P1: mise API/配置持续迭代带来的适配成本

**问题**：mise 可能持续迭代配置结构、CLI/MCP API、合并规则与默认行为；svcmgr 如果与 mise 配置强耦合，维护成本会随版本增长。

**建议**：
- **边界清晰化**：svcmgr 配置与 mise 配置分离（见 §6.5、§7），避免把 svcmgr 字段写入 `mise.toml`
- **Port-Adapter 防腐层**：所有对 mise 的读取/执行都经由 Port 接口；新增适配器时不影响核心业务（见 §6）
- **CI 多版本矩阵 + 契约测试**：用 `mise env --json`、`mise cfg` 等关键输出做契约测试，尽早发现破坏性变化

#### P2: mise run 的进程管理边界

**问题**：通过 `mise run <task>` 启动的进程，mise 会接管 stdin/stdout/stderr。svcmgr 需要拿到子进程的 PID 来管理其生命周期。

**建议**：
- 方案 A：不通过 `mise run`，而是直接从 mise 配置中读取 `[tasks.<name>].run` 的命令，然后自行 spawn（**推荐**）
- 方案 B：通过 `mise run` 启动，再 parse `/proc` 找到子进程 PID（不可靠）
- 方案 C：`mise run` 只用于一次性任务，服务类任务直接 spawn

#### P3: mise 多配置文件合并的确定性

**问题**：mise 的配置合并规则复杂（目录层级、conf.d 字母序、局部覆盖），svcmgr 如果自行复刻 merge 规则，容易与 mise 产生偏差。

**建议**：
- 用 `mise cfg` 获取 **权威的加载文件列表与优先级**
- 获取最终环境变量优先使用 `mise env --json`（当直接解析 TOML 不足以覆盖 mise 的动态能力时）
- svcmgr 只解析自己的配置文件（`svcmgr/config.toml`），避免与 mise merge 规则耦合

#### P4: cgroups v2 在容器内不可用/不可写

**问题**：Docker 非特权容器里不一定允许创建子 cgroup，或者 `/sys/fs/cgroup` 不可写，导致资源限制无法生效。

**建议**：
- `features.resource_limits` 做功能开关，且启动时自动检测可用性（不可用则自动关闭并告警，见 §9.2.2）
- 文档明确运行条件（cgroupns、写权限、宿主机 cgroups v2）
- 保持「不影响核心功能」：资源限制失败不应阻断服务启动

#### P5: cgroups 资源语义与配置转换

**问题**：CPU 配额（`cpu.max`）、内存（`memory.max`）、进程数（`pids.max`）都需要从用户友好的配置转换为内核语义；错误转换会造成不可预期的资源行为。

**建议**：
- 配置字段显式表达语义（如 `cpu_max_percent`、`memory_max`、`pids_max`），避免歧义
- 提供严格校验与清晰错误信息（例如百分比范围、单位解析）
- 在 Web/API 中返回实际生效的 cgroup 值，便于排障

#### P6: 事件触发器的复杂度（事件风暴）

**问题**：事件系统（尤其是 `TaskExit` → 自动重启 → 触发更多事件）可能产生事件风暴。

**建议**（参考 pitchfork 的设计思路）：
- 实现事件去抖动（debounce）与速率限制
- 自动重启使用指数退避（exponential backoff）
- **设置最大重启次数 + 时间窗口**（`restart_limit` + `restart_window`）
- 超过阈值后进入 `FATAL`/`PAUSED` 状态，需要人工 `start` 才能恢复，避免无限循环

#### P7: TTY 服务的定位

**问题**：当前 WebTtyManager 是独立的功能模块；如果统一到调度引擎，需要把 ttyd 作为普通服务进行生命周期管理与端口代理。

**建议**：
- TTY 本质上是一个服务（运行 ttyd 进程），可以定义为 `services.tty-<name>`
- 端口通过 `http_ports` 配置，代理通过统一的内置反向代理处理（见 §11）
- 创建 TTY = 创建 ttyd 服务 + 配置代理路由

#### P8: mise CLI 输出格式稳定性

> 详见 [§6.12 关键问题补充](#612-关键问题补充)。

#### P9: mise 自身升级的原子性

> 详见 [§6.12 关键问题补充](#612-关键问题补充)。

### 15.2 风险矩阵

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|---------|
| mise 配置/合并规则变化导致 svcmgr 解析偏差 | 低 | 中 | 用 `mise cfg`/`mise env --json` 作为权威输入（§15.1 P3） |
| 进程管理稳定性 | 低 | 高 | 复用已验证的 setsid/kill 逻辑 |
| 资源限制在某些容器不可用 | 低 | 中 | 设为可选功能，优雅降级 |
| 事件风暴 | 中 | 中 | 去抖动 + 速率限制 + 指数退避 |
| mise CLI 输出格式变化 | 中 | 中 | 适配器层 + 降级机制 + 契约测试（§6） |
| 配置合并不一致 | 低 | 中 | 使用 `mise cfg` 获取权威文件列表 |
| mise 版本升级导致不兼容 | 中 | 高 | 版本检测 + CI 矩阵测试 + 兼容性声明（§6.4/§6.10） |
| 改造工作量大 | 高 | 中 | 分阶段实施，保持向后兼容 |

---

## 16. 推荐实施路径

### Phase 0: mise 解耦基础设施（预计 1 周）

0. **Port 接口定义**：定义 DependencyPort / TaskPort / EnvPort / ConfigPort trait
1. **Adapter 实现**：实现 MiseV2026Adapter + MockMiseAdapter
2. **版本检测**：实现 MiseVersion 检测与 AdapterFactory
3. **契约测试**：编写 mise CLI 契约测试和 CI 矩阵配置

### Phase 1: 基础设施（预计 1-2 周）

4. **配置解析器**：实现 svcmgr 独立配置 + mise 配置关联解析
5. **Git 配置管理**：实现 5 阶段配置生命周期
6. **功能开关**：实现 features + 环境变量驱动

### Phase 2: 调度引擎（预计 2-3 周）

7. **调度引擎核心**：实现 Trigger 系统（OneShot、Delayed、Cron、Event）
8. **进程管理器**：从 supervisor.rs 提取并增强（加入资源限制）
9. **事件总线**：实现 EventBus 和内置事件

### Phase 3: Web 服务（预计 1-2 周）

10. **HTTP 服务器**：axum 框架搭建
11. **REST API**：实现 /api/v1/* 端点
12. **内置反向代理**：实现 /services/{task}/{port}/* 转发
13. **静态文件服务**：实现 /web/* 服务

### Phase 4: 集成与迁移（预计 1-2 周）

14. **CLI 改造**：CLI 命令调用 API
15. **mise 集成**：通过 Port 接口获取任务命令 + 环境变量注入
16. **向后兼容**：迁移脚本，将旧格式转换为新配置格式
17. **文档更新**：openspec 和 wiki 更新

### Phase 5: 可选增强（按需）

18. Cloudflare 隧道管理（features.tunnel）
19. 更完整的 cgroups v2 支持（io.max、memory.high 等高级控制）
20. Web UI 前端
21. mise MCP 接口集成（替代 CLI 调用，更稳定的程序化交互）

---

## 附录 A: 完整配置文件示例

### A.1 mise 配置（mise 处理）

```toml
# ~/.config/mise/config.toml（全局配置）或项目根 mise.toml

[tools]
node = "22"
python = "3.12"

[env]
NODE_ENV = "production"
DATABASE_URL = "postgres://localhost:5432/mydb"
# 功能开关也可用环境变量控制（见 §12）
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
```

### A.2 svcmgr 配置（svcmgr 处理）

```toml
# ~/.config/mise/svcmgr/config.toml

[features]
web_ui = true
proxy = true
tunnel = false
scheduler = true
git_versioning = true
resource_limits = true

[services.api]
task = "api-start"
enable = true
restart = "always"
restart_delay = "2s"
restart_limit = 10
restart_window = "60s"
stop_timeout = "10s"
http_ports = { web = 3000 }
# cgroups v2
cpu_max_percent = 50
memory_max = "512m"
pids_max = 100

[services.worker]
task = "worker-run"
enable = true
restart = "on-failure"
restart_delay = "5s"
memory_max = "512m"
pids_max = 50

[services.cleanup]
task = "cleanup"
cron = "0 2 * * *"               # 每天凌晨 2 点执行
timeout = "600s"                 # 10 分钟超时

[services.health-check]
task = "api-health"
cron = "*/5 * * * *"             # 每 5 分钟执行
timeout = "30s"

[configurations.app]
path = ".config/app"
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
