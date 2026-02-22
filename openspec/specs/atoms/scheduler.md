# Scheduler 周期任务原子

## Overview

周期任务调度功能已整合到 Supervisor 统一模块中。
`SupervisorManager` 同时实现 `SchedulerAtom` trait，提供 cron 表达式任务管理。
替代 crontab CLI，适用于 Docker 容器等受限环境。

详细规格请参见 `atoms/supervisor.md`。

---

## ADDED Requirements

### Requirement: 任务管理

系统 **MUST** 管理周期任务定义。

#### Scenario: 添加定时任务

- **GIVEN** cron 表达式和执行命令
- **WHEN** 请求添加定时任务
- **THEN** 系统必须将任务添加到 cron-tasks.toml
- **AND** 为任务生成唯一 ID

#### Scenario: 任务存储

- **WHEN** 管理定时任务
- **THEN** 系统通过 SupervisorManager 统一读写 cron-tasks.toml
- **AND** 与服务定义文件共享同一目录

---

### Requirement: Cron 表达式验证

系统 **MUST** 验证 cron 表达式有效性。

#### Scenario: 预定义表达式

- **WHEN** 用户使用预定义名称
- **THEN** 系统必须转换为标准 cron 表达式：
  - `@hourly` → `0 * * * *`
  - `@daily` / `@midnight` → `0 0 * * *`
  - `@weekly` → `0 0 * * 1`
  - `@monthly` → `0 0 1 * *`
  - `@yearly` / `@annually` → `0 0 1 1 *`

---

### Requirement: 执行时间预测

系统 **SHOULD** 支持预测任务执行时间。

#### Scenario: 计算下次执行时间

- **GIVEN** 任务 ID 和所需次数
- **WHEN** 请求下次执行时间
- **THEN** 系统必须计算并返回指定次数的未来执行时间点
