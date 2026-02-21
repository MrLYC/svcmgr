# F01: Systemd 服务管理

> 版本：1.0.0  
> 状态：DRAFT  
> 依赖原子：T02 (模板), T06 (systemd)  
> 最后更新：2026-02-21

## 概述

Systemd 服务管理功能提供对用户级 systemd 服务的完整生命周期管理，包括服务的创建、启动、停止、重启、启用/禁用、日志查看和配置修改。通过组合模板原子和 systemd 原子实现。

---

## ADDED Requirements

### Requirement: 服务列表查询
系统 **MUST** 支持查询所有用户级 systemd 服务。

#### Scenario: 获取服务列表
- **WHEN** 用户请求服务列表
- **THEN** 系统 **SHALL** 返回所有用户级 systemd 服务
- **AND** 每个服务 **SHALL** 包含：名称、状态（running/stopped/failed）、启用状态、PID、内存使用、运行时长、描述

#### Scenario: 服务状态映射
- **WHEN** 解析 systemd 服务状态
- **THEN** 系统 **SHALL** 映射为：
  - `active (running)` → `"running"`
  - `inactive (dead)` → `"stopped"`
  - `failed` → `"failed"`

---

### Requirement: 服务生命周期控制
系统 **MUST** 支持服务的启动、停止、重启操作。

#### Scenario: 启动服务
- **WHEN** 用户启动服务
- **THEN** 系统 **SHALL** 执行 `systemctl --user start {service-name}`
- **AND** 返回操作结果

#### Scenario: 停止服务
- **WHEN** 用户停止服务
- **THEN** 系统 **SHALL** 执行 `systemctl --user stop {service-name}`

#### Scenario: 重启服务
- **WHEN** 用户重启服务
- **THEN** 系统 **SHALL** 执行 `systemctl --user restart {service-name}`

---

### Requirement: 服务启用/禁用
系统 **MUST** 支持设置服务开机自启。

#### Scenario: 启用服务
- **WHEN** 用户启用服务
- **THEN** 系统 **SHALL** 执行 `systemctl --user enable {service-name}`
- **AND** 创建符号链接到 `default.target.wants/`

#### Scenario: 禁用服务
- **WHEN** 用户禁用服务
- **THEN** 系统 **SHALL** 执行 `systemctl --user disable {service-name}`
- **AND** 移除符号链接

---

### Requirement: 服务创建
系统 **MUST** 支持通过模板创建新服务。

#### Scenario: 创建服务
- **WHEN** 用户创建新服务
- **THEN** 系统 **SHALL**：
  1. 使用 **T02** 渲染 systemd service 模板
  2. 写入文件到 `~/.config/systemd/user/{name}.service`
  3. 执行 `systemctl --user daemon-reload`
- **AND** 支持配置：
  - `name`: 服务名称（必需）
  - `description`: 服务描述
  - `exec_start`: 启动命令（必需）
  - `working_directory`: 工作目录
  - `restart_policy`: 重启策略（no/on-failure/always）
  - `environment`: 环境变量（键值对）

---

### Requirement: 服务更新
系统 **MUST** 支持修改现有服务配置。

#### Scenario: 更新服务
- **WHEN** 用户修改服务配置
- **THEN** 系统 **SHALL**：
  1. 使用 **T02** 重新渲染模板
  2. 覆写服务文件
  3. 执行 `systemctl --user daemon-reload`
- **AND** 提示用户重启服务以应用更改

---

### Requirement: 服务删除
系统 **MUST** 支持删除服务。

#### Scenario: 删除服务
- **WHEN** 用户删除服务
- **THEN** 系统 **SHALL**：
  1. 停止服务（如果正在运行）
  2. 禁用服务（如果已启用）
  3. 删除服务文件 `~/.config/systemd/user/{name}.service`
  4. 执行 `systemctl --user daemon-reload`

---

### Requirement: 服务日志查看
系统 **MUST** 支持查看服务日志。

#### Scenario: 获取服务日志
- **WHEN** 用户查看服务日志
- **THEN** 系统 **SHALL** 执行 `journalctl --user -u {service-name} -n 100`
- **AND** 返回日志条目，包含：时间戳、日志级别、消息内容、单元名称

#### Scenario: 日志级别映射
- **WHEN** 解析 journalctl 输出
- **THEN** 系统 **SHALL** 映射优先级：
  - `0-3` (emerg/alert/crit/err) → `"error"`
  - `4` (warning) → `"warning"`
  - `5-6` (notice/info) → `"info"`
  - `7` (debug) → `"debug"`

---

### Requirement: 服务详情查询
系统 **MUST** 支持查询单个服务的详细信息。

#### Scenario: 获取服务详情
- **WHEN** 用户查询服务详情
- **THEN** 系统 **SHALL** 执行 `systemctl --user show {service-name}`
- **AND** 返回详细属性，包括：
  - 基本信息：名称、描述、状态、PID
  - 资源使用：内存、CPU
  - 配置信息：ExecStart、WorkingDirectory、Restart 策略、环境变量

---

## API 端点

### 获取服务列表

#### `GET /svcmgr/api/systemd/services`

**描述**: 获取所有用户级 systemd 服务列表

**请求参数**:
- Query (可选):
  - `filter`: 过滤条件（例如 `status:running`）
  - `sort`: 排序字段（例如 `name`, `-memory`）

**响应** (200 OK):
```json
[
  {
    "name": "nginx.service",
    "status": "running",
    "enabled": true,
    "pid": 1234,
    "memory": "12.4 MB",
    "uptime": "3d 4h",
    "description": "Nginx HTTP Server",
    "exec_start": "/usr/bin/nginx -g 'daemon off;'",
    "working_directory": "/var/www",
    "environment": {
      "PORT": "8080"
    },
    "restart_policy": "on-failure"
  },
  {
    "name": "redis.service",
    "status": "stopped",
    "enabled": false,
    "description": "Redis In-Memory Store"
  }
]
```

**错误响应**:
- `500 INTERNAL_ERROR`: 无法查询 systemd 服务

---

### 获取服务详情

#### `GET /svcmgr/api/systemd/services/{name}`

**描述**: 获取单个服务的详细信息

**路径参数**:
- `name`: 服务名称（例如 `nginx.service`）

**响应** (200 OK):
```json
{
  "name": "nginx.service",
  "status": "running",
  "enabled": true,
  "pid": 1234,
  "memory": "12.4 MB",
  "uptime": "3d 4h",
  "description": "Nginx HTTP Server",
  "exec_start": "/usr/bin/nginx -g 'daemon off;'",
  "working_directory": "/var/www",
  "environment": {
    "PORT": "8080"
  },
  "restart_policy": "on-failure"
}
```

**错误响应**:
- `404 RESOURCE_NOT_FOUND`: 服务不存在
- `500 INTERNAL_ERROR`: 查询失败

---

### 创建服务

#### `POST /svcmgr/api/systemd/services`

**描述**: 创建新的 systemd 服务

**请求体**:
```json
{
  "name": "my-app.service",
  "description": "My Application",
  "exec_start": "/usr/bin/node /opt/my-app/server.js",
  "working_directory": "/opt/my-app",
  "restart_policy": "on-failure",
  "environment": {
    "NODE_ENV": "production",
    "PORT": "3000"
  }
}
```

**响应** (201 Created):
```json
{
  "name": "my-app.service",
  "status": "stopped",
  "enabled": false,
  "description": "My Application",
  "exec_start": "/usr/bin/node /opt/my-app/server.js",
  "working_directory": "/opt/my-app",
  "restart_policy": "on-failure",
  "environment": {
    "NODE_ENV": "production",
    "PORT": "3000"
  }
}
```

**错误响应**:
- `400 VALIDATION_ERROR`: 参数验证失败（缺少必需字段或格式错误）
- `409 RESOURCE_CONFLICT`: 服务名称已存在
- `500 INTERNAL_ERROR`: 创建失败

---

### 更新服务

#### `PUT /svcmgr/api/systemd/services/{name}`

**描述**: 更新现有服务的配置

**路径参数**:
- `name`: 服务名称

**请求体** (部分更新):
```json
{
  "description": "Updated description",
  "exec_start": "/usr/bin/node /opt/my-app/server.js --port 3001",
  "working_directory": "/opt/my-app",
  "restart_policy": "always",
  "environment": {
    "NODE_ENV": "production",
    "PORT": "3001"
  }
}
```

**响应** (200 OK):
```json
{
  "name": "my-app.service",
  "status": "running",
  "enabled": true,
  "description": "Updated description",
  "exec_start": "/usr/bin/node /opt/my-app/server.js --port 3001",
  "working_directory": "/opt/my-app",
  "restart_policy": "always",
  "environment": {
    "NODE_ENV": "production",
    "PORT": "3001"
  }
}
```

**错误响应**:
- `404 RESOURCE_NOT_FOUND`: 服务不存在
- `400 VALIDATION_ERROR`: 参数验证失败
- `500 INTERNAL_ERROR`: 更新失败

---

### 删除服务

#### `DELETE /svcmgr/api/systemd/services/{name}`

**描述**: 删除服务

**路径参数**:
- `name`: 服务名称

**响应** (204 No Content)

**错误响应**:
- `404 RESOURCE_NOT_FOUND`: 服务不存在
- `500 INTERNAL_ERROR`: 删除失败

---

### 服务控制操作

#### `POST /svcmgr/api/systemd/services/{name}/start`

**描述**: 启动服务

**响应** (204 No Content)

**错误响应**:
- `404 RESOURCE_NOT_FOUND`: 服务不存在
- `500 OPERATION_FAILED`: 启动失败

---

#### `POST /svcmgr/api/systemd/services/{name}/stop`

**描述**: 停止服务

**响应** (204 No Content)

**错误响应**:
- `404 RESOURCE_NOT_FOUND`: 服务不存在
- `500 OPERATION_FAILED`: 停止失败

---

#### `POST /svcmgr/api/systemd/services/{name}/restart`

**描述**: 重启服务

**响应** (204 No Content)

**错误响应**:
- `404 RESOURCE_NOT_FOUND`: 服务不存在
- `500 OPERATION_FAILED`: 重启失败

---

### 启用/禁用服务

#### `POST /svcmgr/api/systemd/services/{name}/enable`

**描述**: 设置服务开机自启状态

**请求体**:
```json
{
  "enabled": true
}
```

**响应** (204 No Content)

**错误响应**:
- `404 RESOURCE_NOT_FOUND`: 服务不存在
- `400 VALIDATION_ERROR`: 参数错误
- `500 OPERATION_FAILED`: 操作失败

---

### 获取服务日志

#### `GET /svcmgr/api/systemd/services/{name}/logs`

**描述**: 获取服务日志

**路径参数**:
- `name`: 服务名称

**请求参数**:
- Query (可选):
  - `limit`: 返回日志条数（默认 100）
  - `since`: 起始时间（ISO 8601 格式）

**响应** (200 OK):
```json
[
  {
    "timestamp": "2026-02-21T10:55:00Z",
    "level": "info",
    "message": "nginx.service started successfully",
    "unit": "nginx.service"
  },
  {
    "timestamp": "2026-02-21T10:54:59Z",
    "level": "info",
    "message": "Listening on port 80",
    "unit": "nginx.service"
  },
  {
    "timestamp": "2026-02-21T10:54:58Z",
    "level": "error",
    "message": "Configuration file not found",
    "unit": "nginx.service"
  }
]
```

**错误响应**:
- `404 RESOURCE_NOT_FOUND`: 服务不存在
- `500 INTERNAL_ERROR`: 日志查询失败

---

## 数据模型

### Rust 类型定义

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemdService {
    /// 服务名称（例如 nginx.service）
    pub name: String,
    
    /// 服务状态
    pub status: ServiceStatus,
    
    /// 是否开机自启
    pub enabled: bool,
    
    /// 进程 ID（仅运行时）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    
    /// 内存使用（人类可读格式，例如 "12.4 MB"）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<String>,
    
    /// 运行时长（人类可读格式，例如 "3d 4h"）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uptime: Option<String>,
    
    /// 服务描述
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    
    /// 启动命令
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exec_start: Option<String>,
    
    /// 工作目录
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,
    
    /// 环境变量
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<HashMap<String, String>>,
    
    /// 重启策略
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart_policy: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ServiceStatus {
    Running,
    Stopped,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemdLog {
    /// 时间戳（ISO 8601 格式）
    pub timestamp: String,
    
    /// 日志级别
    pub level: LogLevel,
    
    /// 日志消息
    pub message: String,
    
    /// 单元名称
    pub unit: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Info,
    Warning,
    Error,
    Debug,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateServiceRequest {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub exec_start: String,
    #[serde(default)]
    pub working_directory: Option<String>,
    #[serde(default = "default_restart_policy")]
    pub restart_policy: String,
    #[serde(default)]
    pub environment: Option<HashMap<String, String>>,
}

fn default_restart_policy() -> String {
    "on-failure".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateServiceRequest {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub exec_start: Option<String>,
    #[serde(default)]
    pub working_directory: Option<String>,
    #[serde(default)]
    pub restart_policy: Option<String>,
    #[serde(default)]
    pub environment: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EnableServiceRequest {
    pub enabled: bool,
}
```

---

## 接口定义

```rust
use async_trait::async_trait;
use std::sync::Arc;

#[async_trait]
pub trait SystemdFeature {
    /// 获取所有服务列表
    async fn list_services(&self) -> Result<Vec<SystemdService>, ApiError>;
    
    /// 获取单个服务详情
    async fn get_service(&self, name: &str) -> Result<SystemdService, ApiError>;
    
    /// 创建新服务
    async fn create_service(&self, request: CreateServiceRequest) -> Result<SystemdService, ApiError>;
    
    /// 更新服务配置
    async fn update_service(&self, name: &str, request: UpdateServiceRequest) -> Result<SystemdService, ApiError>;
    
    /// 删除服务
    async fn delete_service(&self, name: &str) -> Result<(), ApiError>;
    
    /// 启动服务
    async fn start_service(&self, name: &str) -> Result<(), ApiError>;
    
    /// 停止服务
    async fn stop_service(&self, name: &str) -> Result<(), ApiError>;
    
    /// 重启服务
    async fn restart_service(&self, name: &str) -> Result<(), ApiError>;
    
    /// 设置服务启用状态
    async fn set_enabled(&self, name: &str, enabled: bool) -> Result<(), ApiError>;
    
    /// 获取服务日志
    async fn get_logs(&self, name: &str, limit: usize) -> Result<Vec<SystemdLog>, ApiError>;
}
```

---

## 配置项

```toml
[systemd]
# systemd 用户目录
user_dir = "~/.config/systemd/user"

# 日志查询默认条数
default_log_lines = 100

# 服务模板路径
template_path = "templates/systemd-service.j2"
```

---

## 内置模板

### systemd-service.j2

```jinja2
[Unit]
Description={{ description | default(name) }}
After=network.target

[Service]
Type=simple
ExecStart={{ exec_start }}
{% if working_directory %}
WorkingDirectory={{ working_directory }}
{% endif %}
Restart={{ restart_policy | default("on-failure") }}
RestartSec=5

{% if environment %}
{% for key, value in environment.items() %}
Environment="{{ key }}={{ value }}"
{% endfor %}
{% endif %}

[Install]
WantedBy=default.target
```

---

## 注意事项

1. **用户级服务**: 所有操作使用 `systemctl --user`
2. **服务名称约定**: 建议使用 `.service` 后缀
3. **daemon-reload**: 创建/修改服务后必须执行 reload
4. **重启提示**: 修改配置后需要手动重启服务才能生效
5. **日志保留**: journald 日志由系统自动管理

---

**变更历史**：
- 2026-02-21: 初始版本，基于前端原型提取
