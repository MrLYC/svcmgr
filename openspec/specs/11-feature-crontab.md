# Feature Spec: Crontab Management

**版本**: 1.0.0  
**状态**: Draft  
**创建日期**: 2026-02-21

## ADDED Requirements

### Requirement: Crontab CRUD Operations
系统 MUST 提供 crontab 任务的增删改查能力。

#### Scenario: Create Crontab Entry
- **WHEN** 用户通过 API 创建定时任务,指定调度表达式和命令
- **THEN** 系统应验证 cron 表达式语法正确性
- **AND** 系统应读取当前用户的 crontab
- **AND** 系统应添加新任务并写回 crontab
- **AND** 系统应返回任务 ID 和创建结果

#### Scenario: List All Crontab Entries
- **WHEN** 用户请求定时任务列表
- **THEN** 系统应执行 `crontab -l` 获取当前任务
- **AND** 系统应解析 crontab 格式并返回结构化列表
- **AND** 列表应包含:任务 ID、调度表达式、命令、描述(如果有)

#### Scenario: Update Crontab Entry
- **WHEN** 用户更新定时任务
- **THEN** 系统应读取当前 crontab
- **AND** 系统应定位到指定任务并更新
- **AND** 系统应使用 `crontab -` 写回更新后的配置

#### Scenario: Delete Crontab Entry
- **WHEN** 用户删除定时任务
- **THEN** 系统应读取当前 crontab
- **AND** 系统应移除指定任务
- **AND** 系统应写回更新后的配置

### Requirement: Cron Expression Validation
系统 MUST 验证 cron 表达式的正确性。

#### Scenario: Validate Cron Syntax
- **WHEN** 用户输入 cron 表达式
- **THEN** 系统应验证表达式格式(5 或 6 字段)
- **AND** 系统应验证每个字段的取值范围
- **AND** 如果表达式错误,系统应返回具体错误位置和原因

#### Scenario: Preview Next Executions
- **WHEN** 用户查看任务的下次执行时间
- **THEN** 系统应解析 cron 表达式
- **AND** 系统应返回未来 N 次执行的时间戳列表
- **AND** 系统应考虑系统时区设置

### Requirement: Template-based Task Creation
系统 MUST 提供常用定时任务模板。

#### Scenario: Use Daily Template
- **WHEN** 用户选择"每日任务"模板
- **THEN** 系统应提供时间选择器(小时:分钟)
- **AND** 系统应生成对应的 cron 表达式 `{minute} {hour} * * *`
- **AND** 用户应能预览生成的表达式

#### Scenario: Use Weekly Template
- **WHEN** 用户选择"每周任务"模板
- **THEN** 系统应提供星期选择器和时间选择器
- **AND** 系统应生成对应的 cron 表达式 `{minute} {hour} * * {day-of-week}`
- **AND** 支持选择多个星期几(逗号分隔)

#### Scenario: Use Monthly Template
- **WHEN** 用户选择"每月任务"模板
- **THEN** 系统应提供日期选择器(1-31)和时间选择器
- **AND** 系统应生成对应的 cron 表达式 `{minute} {hour} {day} * *`
- **AND** 系统应警告用户关于 31 日的特殊情况

### Requirement: Task Description Management
系统 MUST 支持为 crontab 任务添加描述。

#### Scenario: Add Task Description
- **WHEN** 用户创建任务时提供描述信息
- **THEN** 系统应在 crontab 文件中以注释形式保存描述
- **AND** 注释应紧邻任务行上方
- **AND** 注释格式应为 `# [svcmgr:id={id}] {description}`

#### Scenario: Parse Task Descriptions
- **WHEN** 系统读取 crontab 文件
- **THEN** 系统应识别 svcmgr 格式的注释
- **AND** 系统应将描述与对应任务关联
- **AND** 系统应为无描述的任务生成默认 ID

### Requirement: Task Execution History
系统 SHOULD 提供任务执行历史记录。

#### Scenario: Log Task Execution
- **WHEN** crontab 任务执行
- **THEN** 系统应通过日志文件记录执行时间
- **AND** 系统应记录任务的退出码
- **AND** 系统应记录标准输出和标准错误(可选,通过配置启用)

#### Scenario: Query Execution History
- **WHEN** 用户查询任务执行历史
- **THEN** 系统应返回最近 N 次执行记录
- **AND** 记录应包含:执行时间、退出码、执行耗时
- **AND** 如果启用了输出记录,系统应提供输出内容查看

### Requirement: Environment Variable Support
系统 MUST 支持为 crontab 任务设置环境变量。

#### Scenario: Set Task-specific Environment
- **WHEN** 用户为任务配置环境变量
- **THEN** 系统应在 crontab 文件中添加环境变量定义
- **AND** 环境变量应在任务行之前定义
- **AND** 格式应为标准 crontab 环境变量语法 `NAME=value`

#### Scenario: Use Global Environment
- **WHEN** 用户配置全局环境变量(所有任务共享)
- **THEN** 系统应在 crontab 文件顶部添加环境变量
- **AND** 系统应支持引用 mise 管理的环境变量

### Requirement: Integration with Config Management
系统 MUST 将 crontab 配置纳入 git 版本管理。

#### Scenario: Backup Crontab Configuration
- **WHEN** 用户修改 crontab
- **THEN** 系统应将当前 crontab 导出为文件
- **AND** 系统应保存到配置目录 `config/crontab/user.cron`
- **AND** 系统应自动提交变更到 git 仓库

#### Scenario: Restore Crontab from Git
- **WHEN** 用户回滚 crontab 配置
- **THEN** 系统应从 git 历史中恢复指定版本的 crontab 文件
- **AND** 系统应使用 `crontab <file>` 应用配置
- **AND** 系统应验证恢复是否成功

### Requirement: Integration with Mise Tasks
系统 SHOULD 支持将 mise 任务注册为 crontab 任务。

#### Scenario: Schedule Mise Task
- **WHEN** 用户将 mise 任务添加到 crontab
- **THEN** 系统应生成调用 `mise run <task>` 的 cron 命令
- **AND** 系统应确保 mise 环境正确配置
- **AND** 系统应设置正确的工作目录

## Technical Notes

### Implementation Dependencies
- 技术原子: Template Management (02)
- 技术原子: Periodic Task Management (05)
- 技术原子: Git Repository Management (01)
- 可选: Mise Integration (03)

### Crontab Format Reference
```
# ┌───────────── minute (0 - 59)
# │ ┌───────────── hour (0 - 23)
# │ │ ┌───────────── day of the month (1 - 31)
# │ │ │ ┌───────────── month (1 - 12)
# │ │ │ │ ┌───────────── day of the week (0 - 6) (Sunday to Saturday)
# │ │ │ │ │
# * * * * * command to execute
```

### Cron Expression Parser
- Rust crate: `cron` or `croner` for parsing and validation
- Support extended syntax: `@daily`, `@hourly`, `@reboot`, etc.

### Execution Logging Strategy
- Option 1: 重定向输出到日志文件 `>> /path/to/log 2>&1`
- Option 2: 包装脚本记录执行状态
- Option 3: 使用 systemd timer 替代(更好的日志集成)

### ID Generation Strategy
- 使用 UUID 或者 hash(schedule + command) 作为任务 ID
- ID 在注释中保存,用于更新和删除操作
