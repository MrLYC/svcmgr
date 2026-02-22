# T06: Supervisor 统一进程管理原子

> 版本：3.0.0
> 技术基础：内置 Rust 进程管理器 + cron 调度器（替代 systemd --user 和 crontab）

## 概述

提供内置的统一进程管理能力，类似 Python supervisor，包括：
- **进程组管理**：通过 `setsid()` 为每个子进程创建独立进程组
- **优雅关停**：SIGTERM → 等待超时 → SIGKILL 整个进程组
- **自动重启**：后台 watchdog 监控进程退出并按策略自动重启
- **日志捕获**：stdout/stderr 环形缓冲区
- **定时任务调度**：cron 表达式解析和周期任务管理（替代 crontab）

不依赖 systemd 或 crontab，适用于 Docker 容器等受限环境。

---

## ADDED Requirements

### Requirement: 进程组管理
系统 **MUST** 使用进程组隔离每个管理的服务。

#### Scenario: 进程组创建
- **WHEN** 启动一个服务进程时
- **THEN** 系统 **SHALL** 通过 `setsid()` 使子进程成为新进程组的 leader
- **AND** 子进程的 PID == PGID

#### Scenario: 进程组信号投递
- **WHEN** 需要停止服务时
- **THEN** 系统 **SHALL** 使用 `kill(-pgid, sig)` 向整个进程组发送信号
- **AND** 确保所有子进程（包括 fork 出的孙进程）都能收到信号

#### Scenario: 优雅关停
- **WHEN** 请求停止服务时
- **THEN** 系统 **SHALL** 先发送 SIGTERM 到进程组
- **AND** 等待 `stop_timeout_sec` 秒
- **THEN** 如果进程仍在运行，发送 SIGKILL 到进程组

---

### Requirement: 自动重启
系统 **MUST** 支持按策略自动重启服务。

#### Scenario: 重启策略配置
- **WHEN** 创建服务定义时
- **THEN** 系统 **SHALL** 支持以下重启策略：
  - `No`: 不自动重启
  - `Always`: 任何退出都重启
  - `OnFailure`: 仅非零退出码时重启

#### Scenario: watchdog 自动重启
- **WHEN** 服务进程意外退出且策略允许重启时
- **THEN** 系统 **SHALL** 等待 `restart_sec` 秒后自动重启
- **AND** 不会在用户主动 stop 时触发重启

---

### Requirement: 服务单元管理
系统 **MUST** 支持服务定义文件的增删改查。

#### Scenario: 创建服务
- **WHEN** 用户请求创建新服务
- **THEN** 系统 **SHALL** 在服务目录创建 TOML 格式的服务定义文件
- **AND** 定义文件包含命令、描述、重启策略、超时配置等

#### Scenario: 服务定义格式
- **WHEN** 创建服务定义文件
- **THEN** 文件必须包含以下字段：
  - `name`: 服务名称
  - `description`: 服务描述
  - `command`: 启动命令
  - `args`: 命令参数
  - `working_directory`: 工作目录（可选）
  - `env`: 环境变量映射
  - `restart_policy`: 重启策略（No/Always/OnFailure）
  - `restart_sec`: 重启延迟秒数（默认 1）
  - `enabled`: 是否启用
  - `stop_timeout_sec`: 优雅停止超时秒数（默认 10）

#### Scenario: 列出服务
- **WHEN** 用户请求列出服务
- **THEN** 系统 **SHALL** 返回 svcmgr 管理的服务列表
- **AND** 包含：名称、状态、描述、启用状态

---

### Requirement: 服务生命周期控制
系统 **MUST** 支持服务的启动、停止、重启、重载。

#### Scenario: 启动服务
- **WHEN** 用户请求启动服务
- **THEN** 系统 **SHALL** 读取服务定义，通过 setsid 启动对应进程
- **AND** 跟踪进程 PID 和状态
- **AND** 启动 watchdog（如重启策略不为 No）

#### Scenario: 停止服务
- **WHEN** 用户请求停止服务
- **THEN** 系统 **SHALL** 标记 stopping 状态（阻止 watchdog 重启）
- **AND** 执行优雅关停（SIGTERM → 超时 → SIGKILL 进程组）

---

### Requirement: 日志查询
系统 **MUST** 支持查询服务日志。

#### Scenario: 查看日志
- **WHEN** 用户请求查看服务日志
- **THEN** 系统 **SHALL** 从内存环形缓冲区返回日志
- **AND** 支持时间范围过滤、行数限制、优先级过滤

#### Scenario: 日志捕获
- **WHEN** 服务进程运行时
- **THEN** 系统 **SHALL** 自动捕获 stdout（Info 级别）和 stderr（Error 级别）
- **AND** 存储到内存环形缓冲区（可配置容量）

---

### Requirement: 周期任务调度（替代 crontab）
系统 **MUST** 支持 cron 表达式的周期任务管理。

#### Scenario: 添加定时任务
- **WHEN** 用户请求添加定时任务
- **THEN** 系统 **SHALL** 验证 cron 表达式并添加到 cron-tasks.toml

#### Scenario: Cron 表达式支持
- **THEN** 系统 **SHALL** 支持：
  - 标准 5 字段 cron 表达式（自动补充秒字段）
  - 预定义表达式：`@hourly`, `@daily`, `@weekly`, `@monthly`, `@yearly`

#### Scenario: 任务管理
- **THEN** 系统 **SHALL** 支持：
  - 添加、更新、删除、查询任务
  - 预测下 N 次执行时间
  - 全局和任务级环境变量

---

## 接口定义

```rust
// 服务管理接口
pub trait SupervisorAtom {
    async fn create_unit(&self, name: &str, content: &str) -> Result<()>;
    async fn update_unit(&self, name: &str, content: &str) -> Result<()>;
    async fn delete_unit(&self, name: &str) -> Result<()>;
    async fn get_unit(&self, name: &str) -> Result<UnitFile>;
    async fn list_units(&self) -> Result<Vec<UnitInfo>>;
    async fn start(&self, name: &str) -> Result<()>;
    async fn stop(&self, name: &str) -> Result<()>;
    async fn restart(&self, name: &str) -> Result<()>;
    async fn reload(&self, name: &str) -> Result<()>;
    async fn enable(&self, name: &str) -> Result<()>;
    async fn disable(&self, name: &str) -> Result<()>;
    async fn status(&self, name: &str) -> Result<UnitStatus>;
    async fn process_tree(&self, name: &str) -> Result<ProcessTree>;
    async fn logs(&self, name: &str, opts: &LogOptions) -> Result<Vec<LogEntry>>;
    fn logs_stream(&self, name: &str) -> Result<impl Stream<Item = LogEntry>>;
    async fn run_transient(&self, opts: &TransientOptions) -> Result<TransientUnit>;
    async fn list_transient(&self) -> Result<Vec<TransientUnit>>;
    async fn stop_transient(&self, name: &str) -> Result<()>;
    async fn daemon_reload(&self) -> Result<()>;
}

// 周期任务调度接口（同一个 SupervisorManager 实现）
pub trait SchedulerAtom {
    fn add(&self, task: &CronTask) -> Result<String>;
    fn update(&self, task_id: &str, task: &CronTask) -> Result<()>;
    fn remove(&self, task_id: &str) -> Result<()>;
    fn get(&self, task_id: &str) -> Result<CronTask>;
    fn list(&self) -> Result<Vec<CronTask>>;
    fn next_runs(&self, task_id: &str, count: usize) -> Result<Vec<DateTime<Utc>>>;
    fn validate_expression(&self, expr: &str) -> Result<bool>;
    fn set_env(&self, key: &str, value: &str) -> Result<()>;
    fn get_env(&self) -> Result<HashMap<String, String>>;
    fn reload(&self) -> Result<()>;
}
```

---

## 配置项

```toml
[supervisor]
# 服务定义文件和 cron 任务存储目录
service_dir = "~/.config/svcmgr/managed/supervisor"
# 日志环形缓冲区容量（每个服务）
log_capacity = 1000
# 是否将服务定义文件纳入 Git 管理
git_managed = true
```

---

## 与 systemd / crontab 的区别

| 特性 | systemd (旧) | crontab (旧) | supervisor (新) |
|------|-------------|-------------|-----------------|
| 依赖 | systemd --user | cron daemon | 无外部依赖 |
| 容器兼容性 | 有限 | 需要 cron daemon | 完全兼容 |
| 服务定义 | .service unit 文件 | crontab 文件 | TOML 配置文件 |
| 进程管理 | cgroup + systemd | 无 | setsid 进程组 |
| 信号投递 | systemd 管理 | 无 | kill(-pgid, sig) |
| 优雅关停 | TimeoutStopSec | 无 | SIGTERM→wait→SIGKILL |
| 自动重启 | Restart= | 无 | watchdog + RestartPolicy |
| 日志系统 | journalctl | 邮件/日志文件 | 内存环形缓冲区 |
| 定时任务 | systemd-timer | cron 表达式 | cron crate 解析 |

---

## 目录结构

```
~/.config/svcmgr/managed/supervisor/
├── my-service.toml        # 服务定义文件
├── another-service.toml   # 另一个服务定义
└── cron-tasks.toml        # 周期任务存储（统一管理）
```
