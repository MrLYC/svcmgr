# T06: Supervisor 服务管理原子

> 版本：2.0.0
> 技术基础：内置 Rust 进程管理器（替代 systemd --user）

## 概述

提供内置的进程管理能力，类似 supervisor，包括服务定义、生命周期控制、日志查询和临时任务。
不依赖 systemd，适用于 Docker 容器等无 systemd 环境。

---

## ADDED Requirements

### Requirement: 服务单元管理
系统 **MUST** 支持服务定义文件的增删改查。

#### Scenario: 创建服务
- **WHEN** 用户请求创建新服务
- **THEN** 系统 **SHALL** 在服务目录创建 TOML 格式的服务定义文件
- **AND** 定义文件包含命令、描述、重启策略等配置

#### Scenario: 更新服务
- **WHEN** 用户修改服务配置
- **THEN** 系统 **SHALL** 更新服务定义文件
- **AND** 如果服务正在运行，可选重启服务

#### Scenario: 删除服务
- **WHEN** 用户请求删除服务
- **THEN** 系统 **SHALL** 先停止服务（如正在运行）
- **AND** 删除服务定义文件

#### Scenario: 列出服务
- **WHEN** 用户请求列出服务
- **THEN** 系统 **SHALL** 返回 svcmgr 管理的服务列表
- **AND** 包含：名称、状态、描述、启用状态

---

### Requirement: 服务生命周期控制
系统 **MUST** 支持服务的启动、停止、重启、重载。

#### Scenario: 启动服务
- **WHEN** 用户请求启动服务
- **THEN** 系统 **SHALL** 读取服务定义，启动对应进程
- **AND** 跟踪进程 PID 和状态
- **AND** 返回启动结果

#### Scenario: 停止服务
- **WHEN** 用户请求停止服务
- **THEN** 系统 **SHALL** 向进程发送 SIGKILL 终止进程
- **AND** 等待服务完全停止

#### Scenario: 重启服务
- **WHEN** 用户请求重启服务
- **THEN** 系统 **SHALL** 先停止再启动服务
- **AND** 返回新的 PID 信息

#### Scenario: 重载配置
- **WHEN** 用户请求重载服务
- **THEN** 系统 **SHALL** 重新读取服务定义
- **AND** 如果服务正在运行，自动重启以应用新配置

---

### Requirement: 服务状态查询
系统 **MUST** 提供详细的服务状态查询能力。

#### Scenario: 查看状态
- **WHEN** 用户请求查看服务状态
- **THEN** 系统 **SHALL** 返回：
  - 活动状态（active/inactive/failed）
  - 子状态（running/exited/dead）
  - PID（如正在运行）
  - 启动时间
  - 最近日志摘要

#### Scenario: 查看进程树
- **WHEN** 用户请求查看服务进程树
- **THEN** 系统 **SHALL** 返回进程及其子进程信息
- **AND** 以树形结构显示父子关系

---

### Requirement: 日志查询
系统 **MUST** 支持查询服务日志。

#### Scenario: 查看日志
- **WHEN** 用户请求查看服务日志
- **THEN** 系统 **SHALL** 从内存环形缓冲区返回日志
- **AND** 支持时间范围过滤
- **AND** 支持行数限制
- **AND** 支持日志优先级过滤

#### Scenario: 实时日志
- **WHEN** 用户请求实时查看日志
- **THEN** 系统 **SHALL** 返回日志流
- **AND** 支持 WebSocket 推送（Web API）

#### Scenario: 日志捕获
- **WHEN** 服务进程运行时
- **THEN** 系统 **SHALL** 自动捕获 stdout 和 stderr 输出
- **AND** 存储到内存环形缓冲区（可配置容量）

---

### Requirement: 临时任务
系统 **MUST** 支持运行临时任务。

#### Scenario: 运行临时命令
- **WHEN** 用户请求运行临时命令
- **THEN** 系统 **SHALL** 启动进程并返回临时任务信息
- **AND** 返回临时 unit 名称和 PID

#### Scenario: 临时命令选项
- **WHEN** 创建临时任务时
- **THEN** 系统 **SHOULD** 支持以下选项：
  - 环境变量
  - 工作目录
  - 退出后保留状态
  - 自动清理

#### Scenario: 查看临时任务
- **WHEN** 用户请求查看临时任务
- **THEN** 系统 **SHALL** 列出当前运行的临时任务

---

### Requirement: 服务启用管理
系统 **MUST** 支持配置服务自启状态。

#### Scenario: 启用服务
- **WHEN** 用户请求启用服务
- **THEN** 系统 **SHALL** 在服务定义中设置 enabled = true

#### Scenario: 禁用服务
- **WHEN** 用户请求禁用服务
- **THEN** 系统 **SHALL** 在服务定义中设置 enabled = false

---

## 接口定义

```rust
pub trait SupervisorAtom {
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
    
    // 刷新进程状态
    async fn daemon_reload(&self) -> Result<()>;
}

pub struct ServiceDef {
    pub description: String,
    pub exec_start: String,
    pub working_directory: Option<String>,
    pub environment: HashMap<String, String>,
    pub restart: RestartPolicy,
    pub restart_sec: u64,
    pub enabled: bool,
}

pub enum RestartPolicy {
    Always,
    OnFailure,
    No,
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
[supervisor]
# 服务定义文件存放目录
service_dir = "~/.config/svcmgr/services"

# 是否为用户级（user scope）
user_mode = true

# 日志环形缓冲区容量（每个服务）
log_capacity = 1000

# 是否将服务定义文件纳入 Git 管理
git_managed = true
```

---

## 与 systemd 的区别

| 特性 | systemd (旧) | supervisor (新) |
|------|-------------|-----------------|
| 依赖 | systemd --user, systemctl | 无外部依赖 |
| 容器兼容性 | 有限 | 完全兼容 |
| 服务定义 | .service unit 文件 | TOML 配置文件 |
| 日志系统 | journalctl | 内存环形缓冲区 |
| 进程管理 | cgroup + systemd | tokio 子进程 |
| 开机自启 | systemctl enable | 定义文件中 enabled 字段 |

---

## 服务定义模板

详见 `02-atom-template.md` 中的服务模板部分。
