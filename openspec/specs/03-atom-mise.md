# T03-T05: Mise 原子（依赖/任务/环境）

> 版本：1.0.0
> 技术基础：mise CLI

## 概述

基于 mise 提供三个紧密相关的原子能力：
- **T03 依赖管理**：工具和运行时版本管理
- **T04 全局任务**：任务定义和执行
- **T05 环境变量**：环境变量管理

---

## T03: 依赖管理

### Requirement: 工具版本管理
系统 **MUST** 通过 mise 管理用户级别的工具版本。

#### Scenario: 安装工具
- **WHEN** 用户指定需要安装某个工具及版本
- **THEN** 系统 **SHALL** 执行 `mise install {tool}@{version}`
- **AND** 记录到 `.mise.toml` 配置

#### Scenario: 列出已安装
- **WHEN** 用户请求列出已安装工具
- **THEN** 系统 **SHALL** 返回工具名、版本、安装状态

#### Scenario: 版本切换
- **WHEN** 用户请求切换工具版本
- **THEN** 系统 **SHALL** 更新 `.mise.toml` 中的版本
- **AND** 执行 `mise install` 确保版本可用

#### Scenario: 卸载工具
- **WHEN** 用户请求卸载工具
- **THEN** 系统 **SHALL** 从配置中移除
- **AND** 可选执行 `mise uninstall` 清理

---

## T04: 全局任务

### Requirement: 任务定义
系统 **MUST** 支持在 mise 中定义可执行任务。

#### Scenario: 添加任务
- **WHEN** 用户定义新任务
- **THEN** 系统 **SHALL** 在 `.mise.toml` 的 `[tasks]` 节添加配置
- **AND** 验证任务语法正确性

#### Scenario: 任务格式
- **WHEN** 定义任务时
- **THEN** 系统 **MUST** 支持以下格式：
  - 简单命令：`run = "echo hello"`
  - 多行脚本：`run = ["cmd1", "cmd2"]`
  - 带依赖：`depends = ["other-task"]`
  - 带环境：`env = { KEY = "value" }`

#### Scenario: 列出任务
- **WHEN** 用户请求列出任务
- **THEN** 系统 **SHALL** 返回任务名、描述、依赖关系

---

### Requirement: 任务执行
系统 **MUST** 支持执行 mise 任务。

#### Scenario: 直接执行
- **WHEN** 用户请求执行任务
- **THEN** 系统 **SHALL** 执行 `mise run {task}`
- **AND** 捕获并返回输出

#### Scenario: 带参数执行
- **WHEN** 用户提供任务参数
- **THEN** 系统 **SHALL** 传递参数给任务
- **AND** 支持位置参数和命名参数

#### Scenario: 后台执行
- **WHEN** 用户请求后台执行任务
- **THEN** 系统 **SHALL** 使用 `systemd-run --user` 启动
- **AND** 返回临时 unit 名称用于追踪

---

## T05: 环境变量

### Requirement: 环境变量管理
系统 **MUST** 通过 mise 管理环境变量。

#### Scenario: 设置环境变量
- **WHEN** 用户设置环境变量
- **THEN** 系统 **SHALL** 在 `.mise.toml` 的 `[env]` 节添加配置

#### Scenario: 列出环境变量
- **WHEN** 用户请求列出环境变量
- **THEN** 系统 **SHALL** 返回当前配置的环境变量
- **AND** 包含：变量名、值、来源（文件/配置）

#### Scenario: 环境变量文件
- **WHEN** 配置指向 `.env` 文件
- **THEN** 系统 **SHALL** 支持 `_.file = ".env"` 语法加载

#### Scenario: 删除环境变量
- **WHEN** 用户请求删除环境变量
- **THEN** 系统 **SHALL** 从配置中移除该变量

---

## 接口定义

```rust
/// T03: 依赖管理
pub trait DependencyAtom {
    /// 安装工具
    async fn install(&self, tool: &str, version: &str) -> Result<()>;
    
    /// 卸载工具
    async fn uninstall(&self, tool: &str) -> Result<()>;
    
    /// 列出已安装工具
    async fn list(&self) -> Result<Vec<ToolInfo>>;
    
    /// 获取工具可用版本
    async fn available_versions(&self, tool: &str) -> Result<Vec<String>>;
    
    /// 切换版本
    async fn use_version(&self, tool: &str, version: &str) -> Result<()>;
}

/// T04: 全局任务
pub trait TaskAtom {
    /// 添加任务
    fn add_task(&self, name: &str, config: &TaskConfig) -> Result<()>;
    
    /// 移除任务
    fn remove_task(&self, name: &str) -> Result<()>;
    
    /// 列出任务
    fn list_tasks(&self) -> Result<Vec<TaskInfo>>;
    
    /// 执行任务（前台）
    async fn run(&self, name: &str, args: &[String]) -> Result<Output>;
    
    /// 执行任务（后台，通过 systemd-run）
    async fn run_background(&self, name: &str, args: &[String]) -> Result<TransientUnit>;
}

/// T05: 环境变量
pub trait EnvAtom {
    /// 设置环境变量
    fn set(&self, key: &str, value: &str) -> Result<()>;
    
    /// 删除环境变量
    fn unset(&self, key: &str) -> Result<()>;
    
    /// 列出环境变量
    fn list(&self) -> Result<Vec<EnvVar>>;
    
    /// 加载 .env 文件
    fn load_file(&self, path: &Path) -> Result<()>;
    
    /// 获取当前环境
    fn get_env(&self) -> Result<HashMap<String, String>>;
}

pub struct TaskConfig {
    pub run: Vec<String>,
    pub description: Option<String>,
    pub depends: Vec<String>,
    pub env: HashMap<String, String>,
    pub dir: Option<PathBuf>,
}

pub struct TransientUnit {
    pub unit_name: String,
    pub pid: u32,
}
```

---

## 配置文件格式

### ~/.config/svcmgr/managed/mise/.mise.toml
```toml
[tools]
node = "20"
python = "3.12"
rust = "1.75"

[env]
NODE_ENV = "production"
DATABASE_URL = "postgres://localhost/app"
_.file = ".env.local"

[tasks.build]
run = "cargo build --release"
description = "Build the project"

[tasks.serve]
run = ["mise", "run", "build", "&&", "./target/release/app"]
depends = ["build"]
env = { PORT = "8080" }

[tasks.tty]
run = "ttyd -W bash"
description = "Start web terminal"
```

---

## 内置任务模板

### shell-task.mise.j2
```toml
[tasks.{{ name }}]
run = "{{ command }}"
{% if description %}
description = "{{ description }}"
{% endif %}
{% if env %}
env = { {% for k, v in env.items() %}{{ k }} = "{{ v }}"{% if not loop.last %}, {% endif %}{% endfor %} }
{% endif %}
{% if depends %}
depends = {{ depends | tojson }}
{% endif %}
```
