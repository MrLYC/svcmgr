# Systemd 服务管理原子

## Overview

基于 systemd --user 提供用户级服务的完整生命周期管理，包括服务配置、状态控制、日志查询和进程树管理。

---

## ADDED Requirements

### Requirement: 用户级 systemd 环境

系统 **MUST** 使用 systemd 用户模式。

#### Scenario: 确保用户 systemd 实例运行

- **WHEN** 系统启动或执行 setup 命令
- **THEN** 系统必须验证 `systemctl --user` 可用
- **AND** 确保用户 systemd 实例正在运行

#### Scenario: 启用 lingering

- **WHEN** 需要服务在用户未登录时继续运行
- **THEN** 系统必须检查并提示启用 `loginctl enable-linger`

---

### Requirement: 服务单元管理

系统 **MUST** 管理用户级 systemd 单元文件。

#### Scenario: 创建服务单元

- **GIVEN** 服务配置（名称、命令、重启策略等）
- **WHEN** 请求创建服务
- **THEN** 系统必须调用 template 原子渲染单元文件
- **AND** 将文件写入 `~/.config/systemd/user/{name}.service`
- **AND** 执行 `systemctl --user daemon-reload`

#### Scenario: 更新服务单元

- **GIVEN** 服务名称和新配置
- **WHEN** 请求更新服务
- **THEN** 系统必须更新单元文件内容
- **AND** 触发 daemon-reload
- **AND** 通过 git 原子记录变更

#### Scenario: 删除服务单元

- **GIVEN** 服务名称
- **WHEN** 请求删除服务
- **THEN** 系统必须停止并禁用服务
- **AND** 删除单元文件
- **AND** 执行 daemon-reload

---

### Requirement: 服务状态管理

系统 **MUST** 提供服务生命周期控制。

#### Scenario: 启动服务

- **GIVEN** 服务名称
- **WHEN** 请求启动服务
- **THEN** 系统必须执行 `systemctl --user start {name}`
- **AND** 返回启动结果

#### Scenario: 停止服务

- **GIVEN** 服务名称
- **WHEN** 请求停止服务
- **THEN** 系统必须执行 `systemctl --user stop {name}`

#### Scenario: 重启服务

- **GIVEN** 服务名称
- **WHEN** 请求重启服务
- **THEN** 系统必须执行 `systemctl --user restart {name}`

#### Scenario: 启用服务开机自启

- **GIVEN** 服务名称
- **WHEN** 请求启用服务
- **THEN** 系统必须执行 `systemctl --user enable {name}`

#### Scenario: 禁用服务开机自启

- **GIVEN** 服务名称
- **WHEN** 请求禁用服务
- **THEN** 系统必须执行 `systemctl --user disable {name}`

---

### Requirement: 服务状态查询

系统 **MUST** 提供服务状态查询。

#### Scenario: 查询单个服务状态

- **GIVEN** 服务名称
- **WHEN** 请求服务状态
- **THEN** 系统必须返回：运行状态、PID、启动时间、内存/CPU 使用

#### Scenario: 列出所有托管服务

- **WHEN** 请求服务列表
- **THEN** 系统必须返回所有由 svcmgr 创建的服务
- **AND** 包含每个服务的当前状态

---

### Requirement: 服务日志管理

系统 **MUST** 提供服务日志查询。

#### Scenario: 查询服务日志

- **GIVEN** 服务名称
- **WHEN** 请求服务日志
- **THEN** 系统必须调用 `journalctl --user -u {name}`
- **AND** 返回日志内容

#### Scenario: 实时跟踪日志

- **GIVEN** 服务名称
- **WHEN** 请求实时日志流
- **THEN** 系统必须提供类似 `journalctl -f` 的流式输出

#### Scenario: 按时间范围查询日志

- **GIVEN** 服务名称、开始时间、结束时间
- **WHEN** 请求时间范围日志
- **THEN** 系统必须返回指定时间范围内的日志

---

### Requirement: 进程树管理

系统 **MUST** 提供服务进程树查看和管理。

#### Scenario: 查看服务进程树

- **GIVEN** 服务名称
- **WHEN** 请求进程树
- **THEN** 系统必须返回服务及其所有子进程
- **AND** 以树状结构展示 PID、命令、资源使用

#### Scenario: 终止子进程

- **GIVEN** 服务名称和子进程 PID
- **WHEN** 请求终止子进程
- **THEN** 系统必须向该 PID 发送 SIGTERM
- **AND** 确认进程属于该服务的进程树

---

### Requirement: 临时任务（systemd-run）

系统 **MUST** 支持通过 systemd-run 运行临时任务。

#### Scenario: 运行临时任务

- **GIVEN** 命令和可选的任务名称
- **WHEN** 请求运行临时任务
- **THEN** 系统必须调用 `systemd-run --user --unit={name} {command}`
- **AND** 返回任务的运行状态和 PID

#### Scenario: 运行带 TTY 的临时任务

- **GIVEN** 命令和 TTY 需求
- **WHEN** 请求运行带终端的临时任务
- **THEN** 系统必须使用 `systemd-run --user --pty` 选项

#### Scenario: 查询临时任务状态

- **GIVEN** 临时任务名称
- **WHEN** 请求任务状态
- **THEN** 系统必须返回临时单元的运行状态

#### Scenario: 停止临时任务

- **GIVEN** 临时任务名称
- **WHEN** 请求停止任务
- **THEN** 系统必须调用 `systemctl --user stop {name}`

---

### Requirement: 内置服务模板

系统 **MUST** 提供常用服务模板。

#### Scenario: 简单服务模板

- **WHEN** 创建服务时选择 `simple` 模板
- **THEN** 生成的单元文件必须包含：
  - Type=simple
  - 可配置的 ExecStart
  - 可配置的 Restart 策略
  - Environment 支持

#### Scenario: 一次性任务模板

- **WHEN** 创建服务时选择 `oneshot` 模板
- **THEN** 生成的单元文件必须包含：
  - Type=oneshot
  - RemainAfterExit 选项

#### Scenario: 定时器服务模板

- **WHEN** 创建服务时选择 `timer` 模板
- **THEN** 系统必须同时生成 `.service` 和 `.timer` 文件
