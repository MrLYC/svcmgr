# Supervisor 统一进程管理原子

## Overview

内置 Rust 进程管理器，提供用户级服务的完整生命周期管理和周期任务调度。
通过 `setsid()` 实现进程组管理，支持优雅关停（SIGTERM→SIGKILL）和自动重启。
同时集成 cron 表达式调度，替代 systemd --user 和 crontab，适用于 Docker 容器等受限环境。

---

## ADDED Requirements

### Requirement: 进程组管理

系统 **MUST** 使用进程组隔离每个管理的服务。

#### Scenario: 进程组创建

- **WHEN** 启动一个服务进程时
- **THEN** 系统通过 `setsid()` 使子进程成为新进程组 leader（PID == PGID）

#### Scenario: 优雅关停

- **WHEN** 请求停止服务时
- **THEN** 系统发送 SIGTERM 到进程组，等待超时后发送 SIGKILL

---

### Requirement: 自动重启

系统 **MUST** 支持按策略自动重启服务。

#### Scenario: watchdog 监控

- **WHEN** 服务进程意外退出时
- **THEN** 后台 watchdog 根据 RestartPolicy 决定是否重启
- **AND** 用户主动 stop 时不会触发重启

---

### Requirement: 服务单元管理

系统 **MUST** 管理服务定义文件。

#### Scenario: 创建服务单元

- **GIVEN** 服务配置（名称、命令、重启策略等）
- **WHEN** 请求创建服务
- **THEN** 系统必须将服务定义写入 TOML 文件
- **AND** 存储在服务目录 `{service_dir}/{name}.toml`

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
  - `restart_sec`: 重启延迟秒数
  - `enabled`: 是否启用
  - `stop_timeout_sec`: 优雅停止超时秒数

---

### Requirement: 服务状态管理

系统 **MUST** 提供服务生命周期控制。

#### Scenario: 启动服务

- **GIVEN** 服务名称
- **WHEN** 请求启动服务
- **THEN** 系统必须通过 setsid 启动进程并跟踪 PID/状态

#### Scenario: 停止服务

- **GIVEN** 服务名称
- **WHEN** 请求停止服务
- **THEN** 系统必须优雅关停（SIGTERM → 超时 → SIGKILL 进程组）

---

### Requirement: 服务日志管理

系统 **MUST** 提供服务日志查询。

#### Scenario: 日志捕获机制

- **WHEN** 服务进程运行时
- **THEN** 系统必须捕获 stdout（Info 级别）和 stderr（Error 级别）
- **AND** 存储到可配置容量的环形缓冲区

---

### Requirement: 进程树管理

系统 **MUST** 提供服务进程树查看。

#### Scenario: 查看服务进程树

- **GIVEN** 服务名称
- **WHEN** 请求进程树
- **THEN** 系统通过读取 /proc 枚举同 PGID 的所有进程

---

### Requirement: 周期任务调度

系统 **MUST** 支持 cron 表达式的周期任务管理。

#### Scenario: 任务管理

- **THEN** 系统支持任务的增删改查，存储在 cron-tasks.toml
- **AND** 支持标准 5 字段 cron 表达式和预定义表达式
- **AND** 支持全局和任务级环境变量

---

### Requirement: 临时任务

系统 **MUST** 支持运行临时任务。

#### Scenario: 运行临时任务

- **GIVEN** 命令和可选的任务名称
- **WHEN** 请求运行临时任务
- **THEN** 系统必须在独立进程组中启动进程
- **AND** 任务以 `transient-` 前缀命名
