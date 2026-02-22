# T07: Scheduler 周期任务原子

> 版本：2.0.0
> 技术基础：内置 Rust cron 调度器（替代 crontab CLI）

## 概述

提供内置的周期任务调度能力，使用 `cron` crate 解析和调度 cron 表达式。
不依赖系统 crontab，适用于 Docker 容器等受限环境。

---

## ADDED Requirements

### Requirement: 任务管理
系统 **MUST** 支持周期任务的增删改查。

#### Scenario: 添加任务
- **WHEN** 用户请求添加定时任务
- **THEN** 系统 **SHALL** 将任务添加到 TOML 格式的任务存储文件
- **AND** 为任务生成唯一 ID（如未提供）

#### Scenario: 任务定义格式
- **WHEN** 添加任务时
- **THEN** 系统 **MUST** 存储以下信息：
  - `id`: 唯一标识符
  - `description`: 任务描述
  - `expression`: cron 表达式或预定义名称
  - `command`: 执行命令
  - `env`: 任务级环境变量
  - `enabled`: 是否启用

#### Scenario: 更新任务
- **WHEN** 用户请求更新定时任务
- **THEN** 系统 **SHALL** 通过 ID 定位并更新任务定义

#### Scenario: 删除任务
- **WHEN** 用户请求删除定时任务
- **THEN** 系统 **SHALL** 从任务存储中移除对应条目

#### Scenario: 列出任务
- **WHEN** 用户请求列出定时任务
- **THEN** 系统 **SHALL** 返回所有管理的任务
- **AND** 包含：ID、描述、调度表达式、命令、下次执行时间

---

### Requirement: Cron 表达式支持
系统 **MUST** 支持标准 cron 表达式。

#### Scenario: 标准格式
- **WHEN** 用户提供 cron 表达式
- **THEN** 系统 **SHALL** 验证表达式格式
- **AND** 支持：秒(0-59)、分钟(0-59)、小时(0-23)、日(1-31)、月(1-12)、星期(0-7)

#### Scenario: 特殊字符
- **WHEN** cron 表达式包含特殊字符
- **THEN** 系统 **SHALL** 支持：`*`(任意)、`,`(列表)、`-`(范围)、`/`(步进)

#### Scenario: 预定义调度
- **WHEN** 用户使用预定义调度名称
- **THEN** 系统 **SHALL** 支持：
  - `@hourly`: 每小时
  - `@daily`/`@midnight`: 每天 00:00
  - `@weekly`: 每周日 00:00
  - `@monthly`: 每月 1 日 00:00
  - `@yearly`/`@annually`: 每年 1 月 1 日 00:00

#### Scenario: 表达式规范化
- **WHEN** 用户提供 5 字段 cron 表达式
- **THEN** 系统 **SHALL** 自动补充秒字段（默认 "0"）转为 7 字段格式

---

### Requirement: 执行预测
系统 **SHOULD** 支持预测任务执行时间。

#### Scenario: 下次执行
- **WHEN** 用户请求查看任务下次执行时间
- **THEN** 系统 **SHALL** 计算并返回下 N 次执行时间

---

### Requirement: 环境变量
系统 **MUST** 支持环境变量设置。

#### Scenario: 设置全局环境
- **WHEN** 用户需要设置全局环境变量
- **THEN** 系统 **SHALL** 将环境变量存储到任务存储文件的 env 段

#### Scenario: 任务级环境
- **WHEN** 创建任务时指定环境变量
- **THEN** 系统 **SHALL** 将环境变量存储在任务定义中

---

## 接口定义

```rust
pub trait SchedulerAtom {
    /// 添加定时任务
    async fn add(&self, task: &CronTask) -> Result<String>;  // 返回 task_id
    
    /// 更新定时任务
    async fn update(&self, task_id: &str, task: &CronTask) -> Result<()>;
    
    /// 删除定时任务
    async fn remove(&self, task_id: &str) -> Result<()>;
    
    /// 获取单个任务
    async fn get(&self, task_id: &str) -> Result<CronTask>;
    
    /// 列出所有任务
    async fn list(&self) -> Result<Vec<CronTask>>;
    
    /// 计算下次执行时间
    async fn next_runs(&self, task_id: &str, count: usize) -> Result<Vec<DateTime<Utc>>>;
    
    /// 验证 cron 表达式
    async fn validate_expression(&self, expr: &str) -> Result<bool>;
    
    /// 设置全局环境变量
    async fn set_env(&self, key: &str, value: &str) -> Result<()>;
    
    /// 获取全局环境变量
    async fn get_env(&self) -> Result<HashMap<String, String>>;
    
    /// 重载任务定义
    async fn reload(&self) -> Result<()>;
}

pub struct CronTask {
    pub id: Option<String>,
    pub description: String,
    pub expression: String,  // cron 表达式或预定义名称
    pub command: String,
    pub env: HashMap<String, String>,
    pub enabled: bool,
}
```

---

## 配置项

```toml
[scheduler]
# 任务存储文件路径
task_store = "~/.config/svcmgr/scheduler/tasks.toml"

# 是否将任务定义纳入 Git 管理
git_managed = true

# 默认环境变量
[scheduler.env]
SHELL = "/bin/bash"
```

---

## 与 crontab 的区别

| 特性 | crontab (旧) | scheduler (新) |
|------|-------------|----------------|
| 依赖 | crontab CLI | 无外部依赖 |
| 容器兼容性 | 需要 cron daemon | 完全兼容 |
| 存储格式 | crontab 文件 | TOML 配置文件 |
| 表达式解析 | 系统 cron | cron crate (Rust) |
| 任务标识 | 注释标记 | 结构化 ID |
| 环境变量 | crontab 内行 | TOML env 段 |

---

## 内置模板

### daily-task
```toml
[task]
description = "Daily task"
expression = "@daily"
command = ""
enabled = true
```

### weekly-task
```toml
[task]
description = "Weekly task"
expression = "@weekly"
command = ""
enabled = true
```

### monthly-task
```toml
[task]
description = "Monthly task"
expression = "@monthly"
command = ""
enabled = true
```
