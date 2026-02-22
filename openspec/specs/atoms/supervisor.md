# Supervisor 服务管理原子

## Overview

内置 Rust 进程管理器，提供用户级服务的完整生命周期管理，包括服务配置、状态控制、日志查询和进程树管理。
替代 systemd --user，适用于 Docker 容器等无 systemd 环境。

---

## ADDED Requirements

### Requirement: 进程管理环境

系统 **MUST** 使用内置进程管理器。

#### Scenario: 无外部依赖

- **WHEN** 系统启动或执行 setup 命令
- **THEN** 系统无需验证外部服务管理器
- **AND** 直接使用内置 tokio 子进程管理

#### Scenario: 容器兼容

- **WHEN** 在 Docker 容器中运行
- **THEN** 系统可以正常管理所有服务
- **AND** 无需 systemd、init 系统或特殊权限

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
  - `description`: 服务描述
  - `exec_start`: 启动命令
  - `working_directory`: 工作目录（可选）
  - `environment`: 环境变量映射
  - `restart`: 重启策略（Always/OnFailure/No）
  - `restart_sec`: 重启延迟秒数
  - `enabled`: 是否启用

#### Scenario: 更新服务单元

- **GIVEN** 服务名称和新配置
- **WHEN** 请求更新服务
- **THEN** 系统必须更新定义文件
- **AND** 如果服务正在运行，可选重启

#### Scenario: 删除服务单元

- **GIVEN** 服务名称
- **WHEN** 请求删除服务
- **THEN** 系统必须停止并删除服务
- **AND** 删除定义文件

---

### Requirement: 服务状态管理

系统 **MUST** 提供服务生命周期控制。

#### Scenario: 启动服务

- **GIVEN** 服务名称
- **WHEN** 请求启动服务
- **THEN** 系统必须读取服务定义并启动进程
- **AND** 跟踪进程 PID 和状态

#### Scenario: 停止服务

- **GIVEN** 服务名称
- **WHEN** 请求停止服务
- **THEN** 系统必须终止进程（kill + wait）

#### Scenario: 重启服务

- **GIVEN** 服务名称
- **WHEN** 请求重启服务
- **THEN** 系统必须先停止再启动服务

#### Scenario: 启用/禁用服务

- **GIVEN** 服务名称
- **WHEN** 请求启用/禁用服务
- **THEN** 系统必须更新定义文件中的 enabled 字段

---

### Requirement: 服务状态查询

系统 **MUST** 提供服务状态查询。

#### Scenario: 查询单个服务状态

- **GIVEN** 服务名称
- **WHEN** 请求服务状态
- **THEN** 系统必须返回：运行状态、PID、启动时间

#### Scenario: 列出所有托管服务

- **WHEN** 请求服务列表
- **THEN** 系统必须返回所有服务定义文件对应的服务
- **AND** 包含每个服务的当前状态

---

### Requirement: 服务日志管理

系统 **MUST** 提供服务日志查询。

#### Scenario: 查询服务日志

- **GIVEN** 服务名称
- **WHEN** 请求服务日志
- **THEN** 系统必须从内存环形缓冲区返回日志
- **AND** 支持时间范围、行数和优先级过滤

#### Scenario: 实时跟踪日志

- **GIVEN** 服务名称
- **WHEN** 请求实时日志流
- **THEN** 系统必须提供流式日志输出

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
- **THEN** 系统必须返回服务主进程信息
- **AND** 包含 PID 和命令

---

### Requirement: 临时任务

系统 **MUST** 支持运行临时任务。

#### Scenario: 运行临时任务

- **GIVEN** 命令和可选的任务名称
- **WHEN** 请求运行临时任务
- **THEN** 系统必须启动进程并返回任务信息
- **AND** 任务以 `transient-` 前缀命名

#### Scenario: 查询临时任务状态

- **WHEN** 请求临时任务列表
- **THEN** 系统必须返回所有以 `transient-` 前缀命名的运行中任务

#### Scenario: 停止临时任务

- **GIVEN** 临时任务名称
- **WHEN** 请求停止任务
- **THEN** 系统必须终止对应进程

---

### Requirement: 内置服务模板

系统 **MUST** 支持通过 TOML 定义文件创建服务。

#### Scenario: 简单服务

- **WHEN** 创建服务时
- **THEN** 定义文件必须包含：
  - exec_start 命令
  - 可配置的 restart 策略
  - environment 支持
