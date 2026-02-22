# svcmgr 架构设计文档

> 版本：1.0.0 | 基于 main 分支 (821361e)
> 最后更新：2026-02-22

---

## 目录

- [1. 系统总体架构](#1-系统总体架构)
- [2. 设计原则](#2-设计原则)
- [3. 技术原子层 (Atoms)](#3-技术原子层-atoms)
  - [T01 Git 版本管理](#t01-git-版本管理)
  - [T02 模板管理](#t02-模板管理)
  - [T03 依赖管理 / T04 全局任务 / T05 环境变量](#t03-依赖管理--t04-全局任务--t05-环境变量)
  - [T06 服务管理 (Supervisor)](#t06-服务管理-supervisor)
  - [T07 周期任务 (Scheduler)](#t07-周期任务-scheduler)
  - [T08 隧道管理](#t08-隧道管理)
  - [T09 服务代理](#t09-服务代理)
- [4. 功能模块层 (Features)](#4-功能模块层-features)
  - [F01 服务管理](#f01-服务管理)
  - [F02 周期任务管理](#f02-周期任务管理)
  - [F03 Mise 依赖管理](#f03-mise-依赖管理)
  - [F04 Nginx 代理管理](#f04-nginx-代理管理)
  - [F05 Cloudflare 隧道管理](#f05-cloudflare-隧道管理)
  - [F06 配置文件管理](#f06-配置文件管理)
  - [F07 Web TTY](#f07-web-tty)
- [5. CLI 命令层](#5-cli-命令层)
- [6. 数据流与模块关系](#6-数据流与模块关系)
- [7. 目录结构](#7-目录结构)
- [8. 配置管理](#8-配置管理)
- [9. 依赖清单](#9-依赖清单)

---

## 1. 系统总体架构

svcmgr 是一个用于远程管理 Linux 服务环境的工具，专为 Docker 容器等受限环境设计。系统采用 **"技术原子与功能正交"** 的分层架构，自底向上分为三层：

```
┌─────────────────────────────────────────────────────────────┐
│                      CLI 命令层                              │
│  setup | run | teardown | service | cron | mise | nginx     │
│  tunnel | config | tty                                      │
├─────────────────────────────────────────────────────────────┤
│                     功能模块层 (Features)                     │
│  F01 服务管理 │ F02 周期任务 │ F03 Mise │ F04 Nginx          │
│  F05 Tunnel  │ F06 Config  │ F07 WebTTY                     │
├─────────────────────────────────────────────────────────────┤
│                    技术原子层 (Atoms)                         │
│  T01 Git │ T02 Template │ T03 Dependency │ T04 Task         │
│  T05 Env │ T06 Supervisor│ T07 Scheduler │ T08 Tunnel       │
│  T09 Proxy                                                   │
└─────────────────────────────────────────────────────────────┘
```

**核心源码结构：**

```
src/backend/
├── main.rs            # 入口：CLI 解析与命令路由
├── lib.rs             # 库入口
├── config.rs          # 全局配置（XDG 路径）
├── error.rs           # 统一错误类型
├── atoms/             # 技术原子层
│   ├── mod.rs         # 原子模块导出
│   ├── git.rs         # T01 Git 版本管理
│   ├── template.rs    # T02 模板引擎
│   ├── mise.rs        # T03/T04/T05 Mise 依赖/任务/环境变量
│   ├── supervisor.rs  # T06/T07 内置进程管理 + 定时调度（统一模块）
│   ├── tunnel.rs      # T08 Cloudflare Tunnel
│   └── proxy.rs       # T09 Nginx 代理
├── features/          # 功能模块层
│   ├── mod.rs         # 功能模块导出
│   ├── systemd_service.rs  # F01 服务管理
│   ├── crontab_mgmt.rs     # F02 周期任务管理
│   ├── config_mgmt.rs      # F06 配置文件管理
│   └── webtty.rs           # F07 Web TTY
└── cli/               # CLI 命令层
    ├── mod.rs         # 命令定义（clap）
    ├── setup.rs       # setup 命令
    ├── run.rs         # run 命令
    ├── teardown.rs    # teardown 命令
    ├── service.rs     # service 子命令
    ├── cron.rs        # cron 子命令
    ├── mise.rs        # mise 子命令
    ├── nginx.rs       # nginx 子命令
    ├── tunnel.rs      # tunnel 子命令
    ├── config.rs      # config 子命令
    └── webtty.rs      # tty 子命令
```

---

## 2. 设计原则

### 2.1 技术原子正交性

每个技术原子只负责 **单一技术领域**，不与其他原子耦合。功能模块通过 **组合多个原子** 来实现业务逻辑，而非在功能模块中重复实现原子的能力。

例如：F07 Web TTY 功能同时组合了 T06 Supervisor（进程管理）、T09 Proxy（Nginx 反向代理）等多个原子。

### 2.2 Docker 容器兼容

传统方案依赖 systemd 和 crontab，但 Docker 容器中通常没有完整的 init 系统。svcmgr 使用 **Rust 内置实现** 替代：

| 传统方案 | svcmgr 替代方案 | 说明 |
|----------|----------------|------|
| systemd (systemctl) | 内置 Supervisor | 基于 tokio 的进程组管理 |
| crontab | 内置 Scheduler | 基于 cron 表达式的定时调度 |

### 2.3 XDG 基目录标准

所有配置和数据均遵循 [XDG Base Directory Specification](https://specifications.freedesktop.org/basedir-spec/latest/)：

- **配置目录**: `~/.config/svcmgr/`
- **数据目录**: `~/.local/share/svcmgr/`

### 2.4 三阶段生命周期

系统通过 `setup` / `run` / `teardown` 三个顶层命令管理整体生命周期：

```
setup ──▶ 初始化环境（nginx、mise、cloudflared、目录结构）
  │
run ────▶ 启动 svcmgr 服务（Web API / UI）
  │
teardown ▶ 停止所有服务，可选清理配置
```

---

## 3. 技术原子层 (Atoms)

技术原子是系统的最底层构建块。每个原子封装一种具体技术的操作接口。

| 编号 | 原子名称 | 技术基础 | 源文件 | Trait/结构体 |
|------|----------|----------|--------|-------------|
| T01 | Git 版本管理 | libgit2 | `atoms/git.rs` | `GitAtom` |
| T02 | 模板管理 | minijinja | `atoms/template.rs` | `TemplateAtom` / `TemplateEngine` |
| T03 | 依赖管理 | mise CLI | `atoms/mise.rs` | `DependencyAtom` / `MiseManager` |
| T04 | 全局任务 | mise tasks | `atoms/mise.rs` | `TaskAtom` / `MiseManager` |
| T05 | 环境变量 | mise env | `atoms/mise.rs` | `EnvAtom` / `MiseManager` |
| T06 | 服务管理 | 内置 supervisor | `atoms/supervisor.rs` | `SupervisorAtom` / `SupervisorManager` |
| T07 | 周期任务 | 内置 scheduler | `atoms/supervisor.rs` | `SchedulerAtom` / `SupervisorManager` |
| T08 | 隧道管理 | cloudflared CLI | `atoms/tunnel.rs` | `TunnelAtom` / `TunnelManager` |
| T09 | 服务代理 | nginx CLI | `atoms/proxy.rs` | `ProxyAtom` / `NginxManager` |

> **注意**：T06 和 T07 在实现层面合并到同一个文件 `supervisor.rs` 中，由 `SupervisorManager` 统一实现 `SupervisorAtom` 和 `SchedulerAtom` 两个 trait。这是因为定时任务的执行最终也需要进程管理能力。

---

### T01 Git 版本管理

**源文件**: `src/backend/atoms/git.rs` (296 行)

**职责**: 通过 `libgit2` (git2 crate) 提供本地 Git 仓库操作，不依赖系统 git CLI。

**核心结构体**: `GitAtom`

```rust
pub struct GitAtom {
    repo_path: PathBuf,
}
```

**主要接口**:

| 方法 | 说明 |
|------|------|
| `init_repo()` | 初始化 Git 仓库（含默认 .gitignore） |
| `commit(message, files)` | 提交文件变更 |
| `log(limit, path)` | 查询提交历史（支持按路径过滤） |
| `diff(from, to, path)` | 生成两个提交之间的差异 |
| `checkout_file(commit, file)` | 从指定提交恢复单个文件 |
| `revert(commit_id)` | 撤销指定提交 |
| `push(remote, branch)` | 推送到远程仓库 |
| `pull(remote, branch)` | 从远程拉取（仅支持 fast-forward） |

**数据结构**:
- `RepoStatus`: 仓库状态（是否初始化、HEAD 分支、是否干净）
- `CommitInfo`: 提交信息（ID、消息、作者、时间戳、变更文件列表）

---

### T02 模板管理

**源文件**: `src/backend/atoms/template.rs` (525 行)

**职责**: 基于 minijinja（Jinja2 的 Rust 实现）提供模板渲染能力。支持内置模板和用户自定义模板。

**核心 Trait**: `TemplateAtom`

```rust
pub trait TemplateAtom {
    fn list_templates(&self, category: Option<&str>) -> Result<Vec<TemplateInfo>>;
    fn get_template(&self, name: &str) -> Result<String>;
    fn render(&self, template: &str, context: &TemplateContext) -> Result<String>;
    fn render_to_file(&self, template: &str, context: &TemplateContext, output: &Path) -> Result<()>;
    fn validate(&self, template: &str) -> Result<ValidationResult>;
    fn add_user_template(&self, name: &str, content: &str) -> Result<()>;
    fn remove_user_template(&self, name: &str) -> Result<()>;
}
```

**实现**: `TemplateEngine`

**内置模板**:

| 模板路径 | 用途 |
|---------|------|
| `systemd/simple-service.service.j2` | 生成 TOML 格式的 ServiceDef（供 Supervisor 使用） |
| `crontab/daily-task.cron.j2` | 生成定时任务配置 |

**自定义过滤器**:
- `toml_escape`: 转义字符串中的反斜杠、引号、换行符等，确保嵌入 TOML 基础字符串时合法。

**模板来源**:
- `BuiltIn`: 编译时嵌入（通过 `include_str!`）
- `User`: 运行时从用户模板目录加载（`.j2` 后缀）

---

### T03 依赖管理 / T04 全局任务 / T05 环境变量

**源文件**: `src/backend/atoms/mise.rs` (608 行)

**职责**: 通过 mise CLI 管理开发工具版本、全局任务和环境变量。三个原子共用 `MiseManager` 实现。

**核心 Trait**:

| Trait | 原子编号 | 职责 |
|-------|---------|------|
| `DependencyAtom` | T03 | 工具安装/卸载/版本切换 |
| `TaskAtom` | T04 | 任务增删改查/前台执行/后台执行 |
| `EnvAtom` | T05 | 环境变量设置/删除/列表/文件加载 |

**实现**: `MiseManager`

```rust
pub struct MiseManager {
    config_path: PathBuf,  // mise 配置文件路径（.mise.toml）
}
```

**配置路径**: `~/.config/svcmgr/managed/mise/.mise.toml`

**主要数据结构**:
- `ToolInfo`: 工具信息（名称、版本、路径、是否激活）
- `TaskConfig`: 任务配置（命令列表、描述、依赖、环境变量、工作目录）
- `EnvVar`: 环境变量（键、值、来源）

---

### T06 服务管理 (Supervisor)

**源文件**: `src/backend/atoms/supervisor.rs` (~1950 行)

**职责**: 提供类似 Python supervisor 的内置进程管理能力，替代 systemd。这是 svcmgr 的核心模块。

**核心 Trait**: `SupervisorAtom`

```rust
pub trait SupervisorAtom {
    async fn create_unit(&self, name: &str, content: &str) -> Result<()>;
    async fn remove_unit(&self, name: &str) -> Result<()>;
    async fn get_unit(&self, name: &str) -> Result<UnitFile>;
    async fn list_units(&self) -> Result<Vec<UnitInfo>>;
    async fn start(&self, name: &str) -> Result<()>;
    async fn stop(&self, name: &str) -> Result<()>;
    async fn restart(&self, name: &str) -> Result<()>;
    async fn enable(&self, name: &str) -> Result<()>;
    async fn disable(&self, name: &str) -> Result<()>;
    async fn status(&self, name: &str) -> Result<UnitStatus>;
    async fn logs(&self, name: &str, options: &LogOptions) -> Result<Vec<LogEntry>>;
    async fn run_transient(&self, options: &TransientOptions) -> Result<TransientUnit>;
}
```

**实现**: `SupervisorManager`

```rust
pub struct SupervisorManager {
    services_dir: PathBuf,    // 服务定义目录
    state: Arc<Mutex<SupervisorState>>,  // 运行时状态
}
```

**关键技术实现**:

#### 进程组管理

- 使用 `libc::setsid()` 为每个服务创建独立的进程组（会话领导者）
- 终止进程时使用 `libc::kill(-pgid, sig)` 向整个进程组发送信号
- 确保子进程树被完整终止，避免孤儿进程

#### 优雅停止

```
SIGTERM ──(等待 stop_timeout_sec)──▶ SIGKILL
```

1. 先发送 `SIGTERM` 请求进程优雅退出
2. 等待配置的超时时间（默认 10 秒）
3. 超时后发送 `SIGKILL` 强制终止

#### 自动重启看门狗

后台 tokio 任务监控进程状态，根据 `RestartPolicy` 决定是否重启：

| 策略 | 说明 |
|------|------|
| `No` | 不自动重启 |
| `Always` | 总是重启 |
| `OnFailure` | 仅在非零退出码时重启 |

重启间隔由 `restart_sec` 控制。

#### 日志捕获

- 捕获子进程的 stdout/stderr 到内存环形缓冲区（Ring Buffer）
- 支持按行数和优先级查询日志
- 日志条目包含时间戳、优先级、消息内容

#### 服务定义格式 (TOML)

```toml
name = "my-service"
description = "My Service Description"
command = "/usr/bin/my-app"
args = ["--port", "8080"]
env = { KEY = "value" }
restart_policy = "OnFailure"
restart_sec = 5
enabled = true
stop_timeout_sec = 10
```

**核心数据结构**:

| 结构体 | 说明 |
|--------|------|
| `ServiceDef` | 服务定义（命令、参数、环境变量、重启策略等） |
| `UnitFile` | 磁盘上的单元文件 |
| `UnitInfo` | 单元摘要信息 |
| `UnitStatus` | 单元详细状态（加载状态、活动状态、PID、内存等） |
| `ProcessInfo` | 进程信息（PID、命令、CPU、内存） |
| `ProcessTree` | 进程树 |
| `LogEntry` | 日志条目 |
| `TransientUnit` | 临时单元（一次性任务） |

**状态枚举**:
- `LoadState`: `Loaded` / `NotFound` / `Error`
- `ActiveState`: `Active` / `Inactive` / `Failed` / `Activating` / `Deactivating`

---

### T07 周期任务 (Scheduler)

**源文件**: `src/backend/atoms/supervisor.rs`（与 T06 同文件）

**职责**: 内置 cron 调度器，替代系统 crontab。基于 `cron` crate 解析标准 cron 表达式。

**核心 Trait**: `SchedulerAtom`

```rust
pub trait SchedulerAtom {
    async fn add_task(&self, task: &CronTask) -> Result<()>;
    async fn remove_task(&self, id: &str) -> Result<()>;
    async fn get_task(&self, id: &str) -> Result<CronTask>;
    async fn list_tasks(&self) -> Result<Vec<CronTask>>;
    async fn update_task(&self, task: &CronTask) -> Result<()>;
    async fn next_executions(&self, id: &str, count: usize) -> Result<Vec<chrono::DateTime<chrono::Utc>>>;
    async fn validate_expression(&self, expression: &str) -> Result<bool>;
    async fn set_env(&self, key: &str, value: &str) -> Result<()>;
    async fn get_env(&self) -> Result<std::collections::HashMap<String, String>>;
}
```

**实现**: `SupervisorManager`（同时实现 `SupervisorAtom` 和 `SchedulerAtom`）

**任务存储格式** (TOML):

```toml
[[tasks]]
id = "backup-db"
expression = "0 2 * * *"
command = "/usr/local/bin/backup.sh"
description = "每日数据库备份"
enabled = true

[env]
DB_HOST = "localhost"
```

**预定义表达式**:

| 别名 | 等价表达式 | 说明 |
|------|-----------|------|
| `@yearly` / `@annually` | `0 0 1 1 *` | 每年 |
| `@monthly` | `0 0 1 * *` | 每月 |
| `@weekly` | `0 0 * * 0` | 每周 |
| `@daily` / `@midnight` | `0 0 * * *` | 每天 |
| `@hourly` | `0 * * * *` | 每小时 |

**任务存储路径**: `~/.config/svcmgr/managed/scheduler/tasks.toml`

---

### T08 隧道管理

**源文件**: `src/backend/atoms/tunnel.rs` (875 行)

**职责**: 管理 Cloudflare Tunnel，提供安全的外部访问通道。

**核心 Trait**: `TunnelAtom`

| 方法分组 | 方法 | 说明 |
|---------|------|------|
| 认证 | `login()` | 运行 `cloudflared tunnel login` |
| | `is_authenticated()` | 检查 `cert.pem` 是否存在 |
| 隧道管理 | `create(name)` | 创建新隧道 |
| | `delete(name)` | 删除隧道（先停止关联服务） |
| | `list()` | 列出所有隧道 |
| | `get(name)` | 获取指定隧道信息 |
| Ingress | `set_ingress(tunnel, rules)` | 设置路由规则 |
| | `add_ingress_rule(tunnel, rule)` | 添加单条规则 |
| | `remove_ingress_rule(tunnel, hostname)` | 删除规则 |
| DNS | `route_dns(tunnel, hostname)` | 添加 DNS CNAME 路由 |
| 运行控制 | `start(tunnel)` / `stop(tunnel)` / `status(tunnel)` | 委托给 SupervisorAtom |

**实现**: `TunnelManager`

```rust
pub struct TunnelManager {
    config_dir: PathBuf,        // 隧道配置目录
    credentials_dir: PathBuf,   // 凭证目录（~/.cloudflared）
    supervisor: SupervisorManager,  // 运行控制委托
}
```

**隧道启停机制**: TunnelManager 内部持有 `SupervisorManager` 实例，隧道启动时会自动创建 Supervisor 服务定义（TOML 格式），然后通过 Supervisor 管理 cloudflared 进程的生命周期。

---

### T09 服务代理

**源文件**: `src/backend/atoms/proxy.rs` (912 行)

**职责**: 管理 Nginx 反向代理，提供统一的 HTTP 入口。

**核心 Trait**: `ProxyAtom`

| 方法分组 | 方法 | 说明 |
|---------|------|------|
| 生命周期 | `start()` / `stop()` / `reload()` | Nginx 服务控制（委托 Supervisor） |
| | `status()` | 查询运行状态 |
| | `test_config()` | 验证配置语法 |
| HTTP 代理 | `add_http_proxy()` / `remove_http_proxy()` / `list_http_proxies()` | HTTP 反向代理 |
| TCP 代理 | `add_tcp_proxy()` / `remove_tcp_proxy()` / `list_tcp_proxies()` | TCP 流代理 |
| 静态站点 | `add_static_site()` / `remove_static_site()` / `list_static_sites()` | 静态文件服务 |
| TTY 路由 | `add_tty_route()` / `remove_tty_route()` / `list_tty_routes()` | Web TTY 路由 |

**实现**: `NginxManager`

```rust
pub struct NginxManager {
    config_dir: PathBuf,   // nginx 配置目录
    data_dir: PathBuf,     // nginx 运行时数据
    supervisor: SupervisorManager,  // 进程管理委托
}
```

**配置文件结构**:

```
~/.config/svcmgr/nginx/
├── nginx.conf              # 主配置文件
└── conf.d/
    ├── http-proxies.conf   # HTTP 反向代理规则
    ├── tcp-proxies.conf    # TCP 代理规则
    ├── static-sites.conf   # 静态站点配置
    └── tty-routes.conf     # TTY 路由配置
```

**Nginx 启动机制**: `NginxManager.start()` 会自动检查 Supervisor 中是否存在名为 `nginx` 的服务单元，如果不存在则自动创建，然后通过 Supervisor 启动 nginx。

**统一路径路由规范**:

| 路径模式 | 目标 | 说明 |
|----------|------|------|
| `/` | 重定向到 `/svcmgr` | 默认入口 |
| `/svcmgr/*` | svcmgr API/UI | 管理服务 |
| `/tty/{name}` | ttyd 实例 | Web 终端 |
| `/port/{port}` | localhost:{port} | 端口转发 |
| `/static/*` | 静态文件目录 | 文件服务 |

---

## 4. 功能模块层 (Features)

功能模块是面向用户需求的高层抽象，通过组合多个技术原子来实现业务功能。

| 编号 | 功能名称 | 依赖原子 | 源文件 |
|------|----------|----------|--------|
| F01 | 服务管理 | T02, T06 | `features/systemd_service.rs` |
| F02 | 周期任务管理 | T02, T07 | `features/crontab_mgmt.rs` |
| F03 | Mise 依赖管理 | T03, T04, T05 | (直接使用 `atoms/mise.rs` 的 `MiseManager`) |
| F04 | Nginx 代理管理 | T09 | (直接使用 `atoms/proxy.rs` 的 `NginxManager`) |
| F05 | Cloudflare 隧道管理 | T08 | (直接使用 `atoms/tunnel.rs` 的 `TunnelManager`) |
| F06 | 配置文件管理 | T01 | `features/config_mgmt.rs` |
| F07 | Web TTY | T06, T09 | `features/webtty.rs` |

> **注意**: F03/F04/F05 的功能管理器直接封装在对应的原子模块中（`MiseManager`/`NginxManager`/`TunnelManager`），通过 `features/mod.rs` 的 `pub use` 重导出。

---

### F01 服务管理

**源文件**: `src/backend/features/systemd_service.rs`

**职责**: 高层服务管理，组合 `SupervisorAtom` 和 `TemplateAtom`，提供基于模板的服务创建与完整的生命周期管理。

**核心结构体**: `SystemdServiceManager`

```rust
pub struct SystemdServiceManager {
    supervisor: SupervisorManager,
    template_engine: TemplateEngine,
}
```

**主要功能**:

| 功能 | 说明 |
|------|------|
| 服务创建 | 通过模板渲染生成 TOML 服务定义，再调用 SupervisorAtom 创建单元 |
| 生命周期 | start / stop / restart / enable / disable |
| 状态监控 | 查询服务状态、进程信息 |
| 日志查看 | 查看服务日志（从 Supervisor 环形缓冲区） |
| 服务删除 | 先停止再移除服务定义 |
| 临时任务 | 运行一次性命令（transient unit） |

**数据结构**: `ServiceConfig` — 对应模板渲染后的服务配置。

---

### F02 周期任务管理

**源文件**: `src/backend/features/crontab_mgmt.rs`

**职责**: 高层定时任务管理，组合 `SchedulerAtom` 和 `TemplateAtom`，提供任务 CRUD 和调度管理。

**核心结构体**: `CrontabTaskManager`

```rust
pub struct CrontabTaskManager {
    supervisor: SupervisorManager,
    template_engine: TemplateEngine,
}
```

**主要功能**:

| 功能 | 说明 |
|------|------|
| 任务创建 | 支持直接命令或通过模板渲染生成命令 |
| 任务更新 | 修改 cron 表达式、命令、描述 |
| 任务删除 | 移除定时任务 |
| 状态查询 | 查看任务详情 |
| 执行预测 | 显示未来 N 次执行时间 |
| 表达式验证 | 验证 cron 表达式合法性 |
| 环境变量 | 为定时任务设置/查看全局环境变量 |

**数据结构**: `TaskConfig` — 定时任务的配置参数。

---

### F03 Mise 依赖管理

**源文件**: 直接使用 `atoms/mise.rs` 中的 `MiseManager`

**重导出**: `features/mod.rs` → `pub use crate::atoms::mise::MiseManager`

**职责**: 管理开发工具版本（node, python, rust 等）、全局任务和环境变量。

---

### F04 Nginx 代理管理

**源文件**: 直接使用 `atoms/proxy.rs` 中的 `NginxManager`

**重导出**: `features/mod.rs` → `pub use crate::atoms::proxy::NginxManager`

**职责**: 管理 Nginx HTTP/TCP 反向代理、静态站点和 TTY 路由。

---

### F05 Cloudflare 隧道管理

**源文件**: 直接使用 `atoms/tunnel.rs` 中的 `TunnelManager`

**重导出**: `features/mod.rs` → `pub use crate::atoms::tunnel::TunnelManager`

**职责**: 管理 Cloudflare Tunnel 的认证、创建、Ingress 配置、DNS 路由和运行控制。

---

### F06 配置文件管理

**源文件**: `src/backend/features/config_mgmt.rs`

**职责**: 基于 GitAtom 的配置文件版本控制，提供自动提交、备份/恢复、回滚等功能。

**核心结构体**: `ConfigManager`

```rust
pub struct ConfigManager {
    git: GitAtom,
    config_dir: PathBuf,
}
```

**主要功能**:

| 功能 | 说明 |
|------|------|
| 初始化 | 初始化配置 Git 仓库 |
| 自动提交 | 对配置变更进行自动版本控制 |
| 历史查看 | 查看配置变更历史 |
| 差异对比 | 比较两个版本之间的差异 |
| 回滚 | 恢复到指定版本 |
| 备份/恢复 | 基于 Git tag 的备份点 |

---

### F07 Web TTY

**源文件**: `src/backend/features/webtty.rs`

**职责**: 管理基于 ttyd 的 Web 终端实例，组合 Supervisor（进程管理）和 Nginx（反向代理路由）。

**核心结构体**: `WebTtyManager`

```rust
pub struct WebTtyManager {
    supervisor: SupervisorManager,
    nginx: NginxManager,
    instances: HashMap<String, TtyInstance>,
}
```

**主要功能**:

| 功能 | 说明 |
|------|------|
| 创建实例 | 启动 ttyd 进程 + 配置 Nginx 路由 |
| 端口分配 | 自动分配端口（范围 9000-9100） |
| 瞬态/持久 | 支持临时和持久两种 TTY 实例模式 |
| 安全控制 | 可选只读模式、用户名密码认证 |
| 实例管理 | 列表、删除、持久化 |

**TTY 实例数据**:

```rust
pub struct TtyInstance {
    pub name: String,
    pub port: u16,
    pub command: String,
    pub readonly: bool,
    pub credential: Option<String>,
    pub persistent: bool,
    pub status: TtyStatus,
}
```

**端口范围**: 9000 - 9100（自动扫描可用端口）

**Nginx 路由**: 每个 TTY 实例自动注册 `/tty/{name}/` 路径到对应端口，支持 WebSocket 升级。

---

## 5. CLI 命令层

CLI 基于 [clap](https://docs.rs/clap) 框架构建，通过 derive 宏自动生成命令行解析。入口在 `main.rs`，命令定义在 `cli/mod.rs`。

### 命令路由 (main.rs)

```rust
match cli.command {
    Commands::Setup { force } => cli::setup::run(force).await,
    Commands::Run => cli::run::run().await,
    Commands::Teardown { force } => cli::teardown::run(force).await,
    Commands::Service { action } => cli::service::handle_service_command(action).await,
    Commands::Cron { action } => cli::cron::handle_cron_command(action).await,
    Commands::Mise { action } => cli::mise::handle_mise_command(action).await,
    Commands::Nginx { action } => cli::nginx::handle_nginx_command(action).await,
    Commands::Tunnel { action } => cli::tunnel::handle_tunnel_command(action).await,
    Commands::Config { action } => cli::config::handle_config_command(action).await,
    Commands::Tty { action } => cli::webtty::handle_tty_command(action).await,
}
```

### 完整命令结构

```
svcmgr
├── setup [--force]                    # 初始化环境
├── run                                # 启动服务
├── teardown [--force]                 # 卸载环境
│
├── service                            # 服务管理 (F01)
│   ├── list                           # 列出所有服务
│   ├── add <name> -t <template> [-v key=value...]  # 从模板创建服务
│   ├── status <name>                  # 查看服务状态
│   ├── start <name>                   # 启动服务
│   ├── stop <name>                    # 停止服务
│   ├── restart <name>                 # 重启服务
│   ├── enable <name>                  # 启用自动启动
│   ├── disable <name>                 # 禁用自动启动
│   ├── logs <name> [-l lines] [-f]    # 查看日志
│   ├── remove <name> [--force]        # 删除服务
│   └── run <command...> [-w workdir]  # 运行临时任务
│
├── cron                               # 定时任务管理 (F02)
│   ├── list                           # 列出所有任务
│   ├── add <id> -e <expr> -c <cmd> [-d desc] [-t tmpl] [-v k=v...]  # 创建任务
│   ├── status <id>                    # 查看任务详情
│   ├── update <id> [-e expr] [-c cmd] [-d desc]  # 更新任务
│   ├── remove <id> [--force]          # 删除任务
│   ├── next <id> [-n count]           # 预测下次执行时间
│   ├── validate <expression>          # 验证 cron 表达式
│   ├── set-env <key> <value>          # 设置环境变量
│   └── get-env                        # 查看环境变量
│
├── mise                               # Mise 管理 (F03)
│   ├── install <tool> <version>       # 安装工具
│   ├── list-tools                     # 列出已安装工具
│   ├── update <tool> <version>        # 更新工具版本
│   ├── remove <tool> <version> [--force]  # 卸载工具
│   ├── add-task <name> [-r cmd...] [-d desc] [--depends...] [-t tmpl] [-v k=v...]
│   ├── list-tasks                     # 列出任务
│   ├── run-task <name> [args...]      # 执行任务
│   ├── delete-task <name> [--force]   # 删除任务
│   ├── set-env <key> <value>          # 设置环境变量
│   ├── get-env                        # 查看环境变量
│   └── delete-env <key>               # 删除环境变量
│
├── nginx                              # Nginx 管理 (F04)
│   ├── start                          # 启动 nginx
│   ├── stop                           # 停止 nginx
│   ├── reload                         # 重载配置
│   ├── status                         # 查看状态
│   ├── test                           # 测试配置
│   ├── add-proxy <location> <upstream> [-w]  # 添加 HTTP 代理
│   ├── add-static <location> <root> [-a] [-i index]  # 添加静态站点
│   ├── add-tcp <port> <upstream>      # 添加 TCP 代理
│   ├── add-tty <name> <port>          # 添加 TTY 路由
│   ├── list [-t type]                 # 列出配置
│   ├── remove-proxy <location>        # 删除 HTTP 代理
│   ├── remove-static <location>       # 删除静态站点
│   ├── remove-tcp <port>              # 删除 TCP 代理
│   ├── remove-tty <name>              # 删除 TTY 路由
│   └── logs [-e] [-l lines]           # 查看日志
│
├── tunnel                             # Cloudflare Tunnel 管理 (F05)
│   ├── login                          # Cloudflare 认证
│   ├── create <name>                  # 创建隧道
│   ├── list                           # 列出隧道
│   ├── delete <tunnel_id>             # 删除隧道
│   ├── info <tunnel_id>               # 查看隧道信息
│   ├── add-ingress <tunnel_id> <hostname> <service> [-p path]  # 添加路由
│   ├── remove-ingress <tunnel_id> <hostname>  # 删除路由
│   ├── route-dns <tunnel_id> <hostname>  # DNS 路由
│   ├── start <tunnel_id>              # 启动隧道
│   ├── stop <tunnel_id>               # 停止隧道
│   └── status <tunnel_id>             # 查看状态
│
├── config                             # 配置管理 (F06)
│   ├── init                           # 初始化配置仓库
│   ├── log [-l limit]                 # 查看变更历史
│   ├── show <commit>                  # 查看指定提交
│   ├── diff <from> <to>               # 对比差异
│   ├── rollback <commit>              # 回滚到指定版本
│   ├── backup                         # 创建备份点
│   └── restore <name>                 # 恢复备份
│
└── tty                                # Web TTY 管理 (F07)
    ├── create <name> [-c cmd] [-p port] [-r] [-u user:pass]  # 创建实例
    ├── list                           # 列出实例
    ├── remove <name>                  # 删除实例
    └── persist <name>                 # 持久化临时实例
```

---

## 6. 数据流与模块关系

### 6.1 原子层依赖关系

```
                        ┌─────────────┐
                        │  supervisor  │
                        │  (T06/T07)   │
                        └──────┬───────┘
                  ┌────────────┼────────────┐
                  ▼            ▼            ▼
           ┌──────────┐ ┌──────────┐ ┌──────────┐
           │  proxy    │ │  tunnel  │ │  webtty  │
           │  (T09)    │ │  (T08)   │ │  (F07)   │
           └──────────┘ └──────────┘ └──────────┘
```

`SupervisorManager` 是核心枢纽，被 `NginxManager`、`TunnelManager` 和 `WebTtyManager` 共同依赖。

### 6.2 功能模块组合关系

```
F01 服务管理 ─────── SupervisorAtom (T06) + TemplateAtom (T02)
F02 周期任务 ─────── SchedulerAtom (T07)  + TemplateAtom (T02)
F03 Mise 管理 ────── DependencyAtom + TaskAtom + EnvAtom (T03/T04/T05)
F04 Nginx 管理 ───── ProxyAtom (T09) ←── SupervisorAtom (T06)
F05 隧道管理 ─────── TunnelAtom (T08) ←── SupervisorAtom (T06)
F06 配置管理 ─────── GitAtom (T01)
F07 Web TTY ──────── SupervisorAtom (T06) + ProxyAtom (T09)
```

### 6.3 典型数据流

#### 创建并启动服务的流程

```
用户输入: svcmgr service add my-app -t simple-service -v command=/usr/bin/app
    │
    ▼
CLI 层 (cli/service.rs)
    │  解析参数，调用 SystemdServiceManager
    ▼
功能层 (features/systemd_service.rs)
    │  1. TemplateEngine.render("systemd/simple-service.service.j2", ctx)
    │     → 渲染生成 TOML 格式的 ServiceDef
    │  2. SupervisorManager.create_unit("my-app", toml_content)
    │     → 写入服务定义文件
    ▼
原子层 (atoms/supervisor.rs)
    │  写入 ~/.config/svcmgr/managed/supervisor/my-app.toml
    ▼
用户输入: svcmgr service start my-app
    │
    ▼
SupervisorManager.start("my-app")
    │  1. 读取 ServiceDef
    │  2. tokio::process::Command 启动子进程
    │  3. setsid() 创建独立进程组
    │  4. 启动日志捕获 (stdout/stderr → ring buffer)
    │  5. 启动看门狗任务（按 RestartPolicy 自动重启）
    ▼
进程运行中...
```

#### 创建 Web TTY 实例的流程

```
用户输入: svcmgr tty create dev-shell -c bash -p 9001
    │
    ▼
CLI 层 (cli/webtty.rs)
    │
    ▼
功能层 (features/webtty.rs) - WebTtyManager
    │  1. 分配端口（指定 9001 或自动 9000-9100）
    │  2. SupervisorManager.run_transient(ttyd 命令)
    │     → 启动 ttyd 进程
    │  3. NginxManager.add_tty_route("dev-shell", 9001)
    │     → 注册 /tty/dev-shell/ 路由
    │  4. NginxManager.reload()
    │     → 重载 nginx 使路由生效
    ▼
可通过 http://host/tty/dev-shell/ 访问 Web 终端
```

#### 隧道启动流程

```
用户输入: svcmgr tunnel start my-tunnel
    │
    ▼
CLI 层 (cli/tunnel.rs)
    │
    ▼
原子层 (atoms/tunnel.rs) - TunnelManager
    │  1. 检查隧道配置是否存在
    │  2. create_supervisor_service() → 生成 TOML ServiceDef:
    │     command = "/usr/bin/cloudflared"
    │     args = ["tunnel", "--config", "...", "run", "my-tunnel"]
    │  3. SupervisorManager.start("cloudflared-my-tunnel")
    ▼
cloudflared 进程由 Supervisor 管理，支持自动重启
```

---

## 7. 目录结构

### 运行时目录

```
~/.config/svcmgr/                    # XDG 配置目录
├── config.toml                      # 主配置文件
├── templates/                       # 用户自定义模板
│   ├── supervisor/                  # 服务模板
│   ├── scheduler/                   # 调度模板
│   ├── nginx/                       # Nginx 模板
│   └── mise/                        # Mise 模板
├── nginx/                           # Nginx 配置
│   ├── nginx.conf                   # 主配置
│   └── conf.d/                      # 子配置
│       ├── http-proxies.conf
│       ├── tcp-proxies.conf
│       ├── static-sites.conf
│       └── tty-routes.conf
└── managed/                         # 托管配置（Git 仓库）
    ├── .git/                        # 配置版本控制
    ├── supervisor/                  # 服务定义文件 (*.toml)
    ├── scheduler/                   # 调度任务配置 (tasks.toml)
    ├── cloudflared/                 # 隧道配置 (*.yaml)
    ├── nginx/                       # Nginx 配置片段
    └── mise/                        # Mise 任务定义 (.mise.toml)

~/.local/share/svcmgr/              # XDG 数据目录
├── web/                             # Web UI 静态文件
├── nginx/                           # Nginx 运行时数据
│   ├── logs/                        # 访问/错误日志
│   ├── run/                         # PID 文件
│   └── cache/                       # 缓存
├── logs/                            # 全局日志
└── state/                           # 状态文件

~/.cloudflared/                      # Cloudflare 凭证（标准位置）
├── cert.pem                         # 认证证书
└── <tunnel-id>.json                 # 隧道凭证
```

### 项目源码目录

```
svcmgr/
├── Cargo.toml                       # Rust 项目配置
├── src/
│   ├── backend/                     # 后端源码
│   │   ├── main.rs                  # 入口
│   │   ├── lib.rs                   # 库
│   │   ├── config.rs                # 配置
│   │   ├── error.rs                 # 错误处理
│   │   ├── atoms/                   # 技术原子
│   │   ├── features/                # 功能模块
│   │   └── cli/                     # 命令行
│   └── frontend/                    # 前端 (Web UI)
├── templates/                       # 内置模板
│   ├── systemd/
│   │   └── simple-service.service.j2
│   └── crontab/
│       └── daily-task.cron.j2
├── openspec/                        # 规格文档
│   └── specs/
│       ├── 00-architecture-overview.md
│       ├── 04-atom-supervisor.md
│       ├── 05-atom-scheduler.md
│       └── ...
└── docs/                            # 项目文档
```

---

## 8. 配置管理

### 全局配置 (config.rs)

```rust
pub struct Config {
    pub data_dir: PathBuf,       // ~/.local/share/svcmgr
    pub web_dir: PathBuf,        // ~/.local/share/svcmgr/web
    pub nginx_dir: PathBuf,      // ~/.local/share/svcmgr/nginx
    pub config_repo: Option<PathBuf>,  // 配置仓库路径（可选）
}
```

配置通过 `Config::new()` 自动生成，基于用户 HOME 目录推导 XDG 路径。

### 配置版本控制

`managed/` 目录是一个 Git 仓库，由 `ConfigManager` (F06) 管理。所有通过 svcmgr 创建的服务定义、任务配置等均存放在此目录下，支持：

- 自动提交变更
- 查看变更历史
- 差异对比
- 回滚到任意版本
- 基于 Git tag 的备份/恢复

---

## 9. 依赖清单

### Rust 依赖 (Cargo.toml)

| 依赖 | 版本 | 用途 |
|------|------|------|
| `tokio` | 1.49 (full) | 异步运行时 |
| `clap` | 4.5 (derive) | CLI 框架 |
| `serde` / `serde_json` / `serde_yaml` | 1.0 / 1.0 / 0.9 | 序列化/反序列化 |
| `toml` | 0.8 | TOML 解析（服务定义、任务配置） |
| `git2` | 0.20 | libgit2 绑定（Git 操作） |
| `minijinja` | 2.5 | Jinja2 模板引擎 |
| `cron` | 0.15 | cron 表达式解析 |
| `chrono` | 0.4 (serde) | 时间处理 |
| `libc` | 0.2 | POSIX 系统调用（setsid、kill） |
| `dirs` | 6.0 | XDG 目录检测 |
| `regex` | 1.11 | 正则表达式（模板变量提取） |
| `anyhow` | 1.0 | 错误处理 |
| `tracing` / `tracing-subscriber` | 0.1 / 0.3 | 结构化日志 |
| `futures` | 0.3 | 异步工具 |

### 外部工具依赖

| 工具 | 用途 | 是否必须 |
|------|------|---------|
| `nginx` | HTTP/TCP 反向代理 | 是 |
| `mise` | 开发工具版本管理 | 是 |
| `cloudflared` | Cloudflare Tunnel | 可选 |
| `ttyd` | Web 终端 | 可选（仅 F07） |

---

> 本文档基于 svcmgr main 分支 (821361e) 的实际代码生成，与 `openspec/specs/` 下的规格文档保持一致。
