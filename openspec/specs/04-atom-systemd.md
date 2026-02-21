# T06: Systemd 服务管理原子

> 版本：1.0.0
> 技术基础：systemd --user, DBus API

## 概述

提供用户级别 systemd 服务的完整管理能力，包括服务定义、生命周期控制、日志查询和临时任务。

---

## ADDED Requirements

### Requirement: 服务单元管理
系统 **MUST** 支持用户级 systemd unit 文件的增删改查。

#### Scenario: 创建服务
- **WHEN** 用户请求创建新服务
- **THEN** 系统 **SHALL** 在 `~/.config/systemd/user/` 创建 unit 文件
- **AND** 执行 `systemctl --user daemon-reload`

#### Scenario: 更新服务
- **WHEN** 用户修改服务配置
- **THEN** 系统 **SHALL** 更新 unit 文件
- **AND** 执行 daemon-reload
- **AND** 可选重启服务

#### Scenario: 删除服务
- **WHEN** 用户请求删除服务
- **THEN** 系统 **SHALL** 先停止服务（如正在运行）
- **AND** 执行 `systemctl --user disable {service}`
- **AND** 删除 unit 文件
- **AND** 执行 daemon-reload

#### Scenario: 列出服务
- **WHEN** 用户请求列出服务
- **THEN** 系统 **SHALL** 返回 svcmgr 管理的服务列表
- **AND** 包含：名称、状态、描述、启用状态

---

### Requirement: 服务生命周期控制
系统 **MUST** 支持服务的启动、停止、重启、重载。

#### Scenario: 启动服务
- **WHEN** 用户请求启动服务
- **THEN** 系统 **SHALL** 执行 `systemctl --user start {service}`
- **AND** 返回启动结果

#### Scenario: 停止服务
- **WHEN** 用户请求停止服务
- **THEN** 系统 **SHALL** 执行 `systemctl --user stop {service}`
- **AND** 等待服务完全停止

#### Scenario: 重启服务
- **WHEN** 用户请求重启服务
- **THEN** 系统 **SHALL** 执行 `systemctl --user restart {service}`
- **AND** 返回新的 PID 信息

#### Scenario: 重载配置
- **WHEN** 服务支持配置重载且用户请求重载
- **THEN** 系统 **SHALL** 执行 `systemctl --user reload {service}`

---

### Requirement: 服务状态查询
系统 **MUST** 提供详细的服务状态查询能力。

#### Scenario: 查看状态
- **WHEN** 用户请求查看服务状态
- **THEN** 系统 **SHALL** 返回：
  - 活动状态（active/inactive/failed）
  - 子状态（running/exited/dead）
  - PID（如正在运行）
  - 内存/CPU 使用（如可用）
  - 启动时间
  - 最近日志摘要

#### Scenario: 查看进程树
- **WHEN** 用户请求查看服务进程树
- **THEN** 系统 **SHALL** 返回 cgroup 内所有进程
- **AND** 以树形结构显示父子关系

---

### Requirement: 日志查询
系统 **MUST** 支持查询服务日志。

#### Scenario: 查看日志
- **WHEN** 用户请求查看服务日志
- **THEN** 系统 **SHALL** 执行 `journalctl --user -u {service}`
- **AND** 支持时间范围过滤
- **AND** 支持行数限制

#### Scenario: 实时日志
- **WHEN** 用户请求实时查看日志
- **THEN** 系统 **SHALL** 返回日志流
- **AND** 支持 WebSocket 推送（Web API）

#### Scenario: 日志导出
- **WHEN** 用户请求导出日志
- **THEN** 系统 **SHALL** 支持导出为 JSON 或纯文本格式

---

### Requirement: 临时任务
系统 **MUST** 支持通过 systemd-run 运行临时任务。

#### Scenario: 运行临时命令
- **WHEN** 用户请求运行临时命令
- **THEN** 系统 **SHALL** 执行 `systemd-run --user --unit={name} {command}`
- **AND** 返回临时 unit 名称

#### Scenario: 临时命令选项
- **WHEN** 创建临时任务时
- **THEN** 系统 **SHOULD** 支持以下选项：
  - `--scope`: 在 scope 中运行
  - `--remain-after-exit`: 退出后保留状态
  - `--collect`: 退出后自动清理
  - `-E KEY=VALUE`: 环境变量
  - `--working-directory`: 工作目录

#### Scenario: 查看临时任务
- **WHEN** 用户请求查看临时任务
- **THEN** 系统 **SHALL** 列出当前运行的临时 unit
- **AND** 标识为 svcmgr 创建的临时任务

---

### Requirement: 服务启用管理
系统 **MUST** 支持配置服务开机自启。

#### Scenario: 启用服务
- **WHEN** 用户请求启用服务开机自启
- **THEN** 系统 **SHALL** 执行 `systemctl --user enable {service}`

#### Scenario: 禁用服务
- **WHEN** 用户请求禁用服务开机自启
- **THEN** 系统 **SHALL** 执行 `systemctl --user disable {service}`

---

## 接口定义

```rust
pub trait SystemdAtom {
    // Unit 文件管理
    async fn create_unit(&self, name: &str, content: &str) -> Result<()>;
    async fn update_unit(&self, name: &str, content: &str) -> Result<()>;
    async fn delete_unit(&self, name: &str) -> Result<()>;
    async fn get_unit(&self, name: &str) -> Result<UnitFile>;
    async fn list_units(&self) -> Result<Vec<UnitInfo>>;
    
    // 生命周期控制
    async fn start(&self, name: &str) -> Result<()>;
    async fn stop(&self, name: &str) -> Result<()>;
    async fn restart(&self, name: &str) -> Result<()>;
    async fn reload(&self, name: &str) -> Result<()>;
    
    // 启用管理
    async fn enable(&self, name: &str) -> Result<()>;
    async fn disable(&self, name: &str) -> Result<()>;
    
    // 状态查询
    async fn status(&self, name: &str) -> Result<UnitStatus>;
    async fn process_tree(&self, name: &str) -> Result<ProcessTree>;
    
    // 日志
    async fn logs(&self, name: &str, opts: &LogOptions) -> Result<Vec<LogEntry>>;
    fn logs_stream(&self, name: &str) -> Result<impl Stream<Item = LogEntry>>;
    
    // 临时任务
    async fn run_transient(&self, opts: &TransientOptions) -> Result<TransientUnit>;
    async fn list_transient(&self) -> Result<Vec<TransientUnit>>;
    async fn stop_transient(&self, name: &str) -> Result<()>;
    
    // daemon-reload
    async fn daemon_reload(&self) -> Result<()>;
}

pub struct UnitInfo {
    pub name: String,
    pub description: String,
    pub load_state: LoadState,
    pub active_state: ActiveState,
    pub sub_state: String,
    pub enabled: bool,
}

pub struct UnitStatus {
    pub name: String,
    pub active_state: ActiveState,
    pub sub_state: String,
    pub pid: Option<u32>,
    pub memory: Option<u64>,
    pub cpu_time: Option<Duration>,
    pub started_at: Option<DateTime<Utc>>,
    pub recent_logs: Vec<String>,
}

pub struct TransientOptions {
    pub name: String,
    pub command: Vec<String>,
    pub scope: bool,
    pub remain_after_exit: bool,
    pub collect: bool,
    pub env: HashMap<String, String>,
    pub working_directory: Option<PathBuf>,
}

pub struct LogOptions {
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
    pub lines: Option<usize>,
    pub priority: Option<LogPriority>,
}

pub enum ActiveState {
    Active,
    Inactive,
    Activating,
    Deactivating,
    Failed,
    Reloading,
}
```

---

## 配置项

```toml
[systemd]
# Unit 文件存放目录
unit_dir = "~/.config/systemd/user"

# 是否将 unit 文件也纳入 Git 管理
git_managed = true

# 日志保留时间
log_retention_days = 30

# 默认 transient 选项
[systemd.transient_defaults]
collect = true
remain_after_exit = false
```

---

## 服务模板

详见 `02-atom-template.md` 中的 systemd 模板部分。
