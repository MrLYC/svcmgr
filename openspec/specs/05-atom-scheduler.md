# T07: 周期任务调度（Supervisor 统一模块）

> 版本：3.0.0
> 技术基础：内置 Rust cron 调度器，集成在 Supervisor 统一模块中

## 概述

周期任务调度功能已整合到 Supervisor 统一模块（`supervisor.rs`）中。
`SupervisorManager` 同时实现了 `SupervisorAtom`（服务管理）和 `SchedulerAtom`（定时任务）两个 trait。

详细规格请参见 `04-atom-supervisor.md`。

---

## 与 crontab 的区别

| 特性 | crontab (旧) | supervisor scheduler (新) |
|------|-------------|--------------------------|
| 依赖 | crontab CLI + cron daemon | 无外部依赖 |
| 容器兼容性 | 需要 cron daemon | 完全兼容 |
| 存储格式 | crontab 文件 | TOML (cron-tasks.toml) |
| 表达式解析 | 系统 cron | cron crate (Rust) |
| 任务标识 | 注释标记 | 结构化 ID |
| 环境变量 | crontab 内行 | TOML env 段 |
| 集成 | 独立模块 | 与进程管理统一 |

---

## 任务存储格式

```toml
# cron-tasks.toml

[env]
SHELL = "/bin/bash"
PATH = "/usr/local/bin:/usr/bin"

[[tasks]]
id = "daily-backup"
description = "Daily database backup"
expression = "@daily"
command = "/opt/scripts/backup.sh"
enabled = true

[[tasks]]
id = "hourly-check"
description = "Hourly health check"
expression = "0 * * * *"
command = "curl -s http://localhost:8080/health"
enabled = true
```
