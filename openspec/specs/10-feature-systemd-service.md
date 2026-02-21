# Feature Spec: Systemd Service Management

**版本**: 1.0.0  
**状态**: Draft  
**创建日期**: 2026-02-21

## ADDED Requirements

### Requirement: Service CRUD Operations
系统 MUST 提供 systemd 用户服务的增删改查能力。

#### Scenario: Create Service from Template
- **WHEN** 用户通过 API 创建新服务,指定服务名称、命令和模板参数
- **THEN** 系统应使用模板管理能力渲染 systemd service 文件
- **AND** 系统应将渲染后的配置写入 `~/.config/systemd/user/` 目录
- **AND** 系统应自动执行 `systemctl --user daemon-reload`
- **AND** 系统应返回服务创建成功状态

#### Scenario: List All Services
- **WHEN** 用户请求服务列表
- **THEN** 系统应调用 `systemctl --user list-units --type=service`
- **AND** 系统应返回包含服务名、状态、启动时间的结构化列表

#### Scenario: Update Service Configuration
- **WHEN** 用户更新服务配置
- **THEN** 系统应使用模板管理能力重新渲染配置文件
- **AND** 系统应执行 `systemctl --user daemon-reload`
- **AND** 如果服务正在运行,系统应提示用户是否重启服务

#### Scenario: Delete Service
- **WHEN** 用户删除服务
- **THEN** 系统应先停止服务(如果正在运行)
- **AND** 系统应删除 `~/.config/systemd/user/` 下的服务文件
- **AND** 系统应执行 `systemctl --user daemon-reload`

### Requirement: Service Lifecycle Management
系统 MUST 支持服务的启动、停止、重启、重载操作。

#### Scenario: Start Service
- **WHEN** 用户启动服务
- **THEN** 系统应执行 `systemctl --user start <service>`
- **AND** 系统应等待操作完成并返回结果
- **AND** 如果启动失败,系统应返回详细错误信息

#### Scenario: Enable Service at Boot
- **WHEN** 用户设置服务开机自启
- **THEN** 系统应执行 `systemctl --user enable <service>`
- **AND** 系统应返回操作结果

#### Scenario: Restart Service
- **WHEN** 用户重启服务
- **THEN** 系统应执行 `systemctl --user restart <service>`
- **AND** 系统应等待服务完全启动
- **AND** 系统应返回新的服务状态

### Requirement: Service Status Monitoring
系统 MUST 提供实时服务状态查询能力。

#### Scenario: Get Service Status
- **WHEN** 用户查询服务状态
- **THEN** 系统应执行 `systemctl --user status <service>`
- **AND** 系统应解析输出并返回结构化状态信息
- **AND** 状态信息应包含:运行状态、PID、内存使用、启动时间、最近日志

#### Scenario: Get Service Process Tree
- **WHEN** 用户请求服务进程树
- **THEN** 系统应通过 cgroup 获取服务的所有子进程
- **AND** 系统应返回进程树结构,包含 PID、命令、内存、CPU 使用率

### Requirement: Service Log Management
系统 MUST 支持 journalctl 日志查询和实时跟踪。

#### Scenario: Query Service Logs
- **WHEN** 用户查询服务日志,指定时间范围和行数限制
- **THEN** 系统应执行 `journalctl --user -u <service> --since <time> -n <lines>`
- **AND** 系统应返回日志条目列表,包含时间戳、级别、消息

#### Scenario: Stream Service Logs
- **WHEN** 用户请求实时日志流
- **THEN** 系统应执行 `journalctl --user -u <service> -f`
- **AND** 系统应通过 WebSocket 将日志实时推送给客户端
- **AND** 系统应支持客户端主动断开连接

### Requirement: Transient Service Execution
系统 MUST 支持通过 systemd-run 运行临时任务。

#### Scenario: Run Temporary Task
- **WHEN** 用户提交临时任务,指定命令和参数
- **THEN** 系统应执行 `systemd-run --user --scope <command>`
- **AND** 系统应返回临时服务的 unit 名称和 PID
- **AND** 任务执行完成后,systemd 应自动清理资源

#### Scenario: Run Scheduled Task
- **WHEN** 用户创建定时临时任务
- **THEN** 系统应使用 `systemd-run --user --on-calendar=<schedule>` 创建 timer
- **AND** 系统应返回 timer unit 名称
- **AND** 任务应在指定时间自动执行

### Requirement: Built-in Service Templates
系统 MUST 提供常用服务的内置模板。

#### Scenario: List Available Templates
- **WHEN** 用户请求服务模板列表
- **THEN** 系统应返回所有内置模板
- **AND** 每个模板应包含名称、描述、必需参数列表

#### Scenario: Use Built-in Template
- **WHEN** 用户使用内置模板创建服务
- **THEN** 系统应验证所有必需参数已提供
- **AND** 系统应使用模板管理能力渲染配置
- **AND** 内置模板应包含:
  - simple-service: 简单的常驻服务
  - oneshot-task: 一次性任务
  - web-app: Web 应用服务(自动配置 nginx 代理)
  - python-app: Python 应用(自动配置 mise 环境)
  - node-app: Node.js 应用(自动配置 mise 环境)

### Requirement: Integration with Config Management
系统 MUST 将服务配置文件纳入 git 版本管理。

#### Scenario: Auto-commit Service Changes
- **WHEN** 用户创建、修改或删除服务
- **THEN** 系统应自动提交配置变更到 git 仓库
- **AND** commit message 应包含操作类型和服务名称
- **AND** 用户应能通过配置管理能力回滚服务配置

## Technical Notes

### Implementation Dependencies
- 技术原子: Git Repository Management (01)
- 技术原子: Template Management (02)
- 技术原子: Service Management (04)

### Systemd User Service Path
- Service files: `~/.config/systemd/user/`
- User linger enabled: `loginctl enable-linger $USER`

### DBus Integration (Optional Enhancement)
- 可选使用 DBus API 替代 systemctl 命令
- Rust crate: `zbus` for DBus communication
- 优点: 更快的响应速度,避免进程创建开销

### Error Handling
- 服务启动失败应返回 journalctl 错误日志
- 配置语法错误应在 daemon-reload 时捕获
- 权限错误应给出明确提示
