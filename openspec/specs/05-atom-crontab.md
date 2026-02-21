# T07: Crontab 周期任务原子

> 版本：1.0.0
> 技术基础：crontab (用户级)

## 概述

提供用户级别 crontab 的管理能力，支持周期任务的增删改查。

---

## ADDED Requirements

### Requirement: Crontab 条目管理
系统 **MUST** 支持用户 crontab 条目的增删改查。

#### Scenario: 添加任务
- **WHEN** 用户请求添加定时任务
- **THEN** 系统 **SHALL** 向用户 crontab 添加条目
- **AND** 为条目添加标识注释以便管理

#### Scenario: 条目格式
- **WHEN** 添加 crontab 条目时
- **THEN** 系统 **MUST** 使用以下格式：
```cron
# [svcmgr:{task_id}] {description}
{minute} {hour} {day} {month} {weekday} {command}
```

#### Scenario: 更新任务
- **WHEN** 用户请求更新定时任务
- **THEN** 系统 **SHALL** 通过标识注释定位并更新条目

#### Scenario: 删除任务
- **WHEN** 用户请求删除定时任务
- **THEN** 系统 **SHALL** 移除对应条目和标识注释

#### Scenario: 列出任务
- **WHEN** 用户请求列出定时任务
- **THEN** 系统 **SHALL** 返回 svcmgr 管理的 crontab 条目
- **AND** 包含：ID、描述、调度表达式、命令、下次执行时间

---

### Requirement: Cron 表达式支持
系统 **MUST** 支持标准 cron 表达式。

#### Scenario: 标准格式
- **WHEN** 用户提供 cron 表达式
- **THEN** 系统 **SHALL** 验证表达式格式
- **AND** 支持：分钟(0-59)、小时(0-23)、日(1-31)、月(1-12)、星期(0-7)

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

---

### Requirement: 执行预测
系统 **SHOULD** 支持预测任务执行时间。

#### Scenario: 下次执行
- **WHEN** 用户请求查看任务下次执行时间
- **THEN** 系统 **SHALL** 计算并返回下 N 次执行时间

#### Scenario: 执行历史
- **WHEN** 系统配置了执行日志
- **THEN** 系统 **SHOULD** 记录并返回历史执行记录

---

### Requirement: 环境变量
系统 **MUST** 支持 crontab 环境变量设置。

#### Scenario: 设置环境
- **WHEN** 用户需要为任务设置环境变量
- **THEN** 系统 **SHALL** 在 crontab 中添加环境变量行
- **AND** 格式为 `KEY=value`

#### Scenario: 常用环境变量
- **WHEN** 创建 crontab 条目时
- **THEN** 系统 **SHOULD** 默认设置：
  - `SHELL=/bin/bash`
  - `PATH` (包含 mise 管理的工具路径)
  - `MAILTO` (可选，用于邮件通知)

---

## 接口定义

```rust
pub trait CrontabAtom {
    /// 添加定时任务
    fn add(&self, task: &CronTask) -> Result<String>;  // 返回 task_id
    
    /// 更新定时任务
    fn update(&self, task_id: &str, task: &CronTask) -> Result<()>;
    
    /// 删除定时任务
    fn remove(&self, task_id: &str) -> Result<()>;
    
    /// 获取单个任务
    fn get(&self, task_id: &str) -> Result<CronTask>;
    
    /// 列出所有任务
    fn list(&self) -> Result<Vec<CronTask>>;
    
    /// 计算下次执行时间
    fn next_runs(&self, task_id: &str, count: usize) -> Result<Vec<DateTime<Utc>>>;
    
    /// 验证 cron 表达式
    fn validate_expression(&self, expr: &str) -> Result<bool>;
    
    /// 设置环境变量
    fn set_env(&self, key: &str, value: &str) -> Result<()>;
    
    /// 获取环境变量
    fn get_env(&self) -> Result<HashMap<String, String>>;
    
    /// 重载 crontab
    fn reload(&self) -> Result<()>;
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
[crontab]
# 是否将 crontab 导出备份到 Git 管理目录
backup_to_git = true

# 备份文件路径
backup_file = "crontab/user.cron"

# 默认环境变量
[crontab.env]
SHELL = "/bin/bash"
MAILTO = ""
```

---

## 内置模板

### daily-task.cron.j2
```jinja2
# [svcmgr:{{ id }}] {{ description | default(name ~ " daily task") }}
# Runs at {{ hour | default("03") }}:{{ minute | default("00") }} every day
{{ minute | default("0") }} {{ hour | default("3") }} * * * {{ command }}
```

### weekly-task.cron.j2
```jinja2
# [svcmgr:{{ id }}] {{ description | default(name ~ " weekly task") }}
# Runs at {{ hour | default("03") }}:{{ minute | default("00") }} every {{ weekday | default("Sunday") }}
{{ minute | default("0") }} {{ hour | default("3") }} * * {{ weekday_num | default("0") }} {{ command }}
```

### monthly-task.cron.j2
```jinja2
# [svcmgr:{{ id }}] {{ description | default(name ~ " monthly task") }}
# Runs at {{ hour | default("03") }}:{{ minute | default("00") }} on day {{ day | default("1") }} of each month
{{ minute | default("0") }} {{ hour | default("3") }} {{ day | default("1") }} * * {{ command }}
```
