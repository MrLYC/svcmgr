# F03: Mise 工具管理

> 版本：1.0.0  
> 状态：DRAFT  
> 依赖原子：T02 (模板), T03 (mise 依赖), T04 (mise 任务), T05 (mise 环境变量)  
> 最后更新：2026-02-21

## 概述

Mise 工具管理功能提供对 mise 依赖管理器的完整集成,包括依赖安装、版本管理、任务定义和执行。Mise 是现代化的开发工具版本管理器,支持多版本并存和项目级配置。

---

## ADDED Requirements

### Requirement: 依赖列表查询
系统 **MUST** 支持查询所有已安装的 mise 依赖。

#### Scenario: 获取依赖列表
- **WHEN** 用户请求依赖列表
- **THEN** 系统 **SHALL** 执行 `mise list`
- **AND** 每个依赖 **SHALL** 包含：名称、当前版本、最新可用版本、来源、已安装版本列表

#### Scenario: 查询最新版本
- **WHEN** 显示依赖信息
- **THEN** 系统 **SHOULD** 执行 `mise latest {name}` 查询最新版本
- **AND** 与当前版本比较,显示是否有更新

---

### Requirement: 依赖安装
系统 **MUST** 支持安装新的 mise 依赖或特定版本。

#### Scenario: 安装依赖
- **WHEN** 用户安装依赖
- **THEN** 系统 **SHALL**：
  1. 执行 `mise install {name}@{version}`
  2. 显示安装进度日志
  3. 更新 mise 配置文件
- **AND** 支持配置：
  - `name`: 工具名称（必需）
  - `version`: 版本号（必需）

#### Scenario: 查询可用版本
- **WHEN** 用户查看可安装版本
- **THEN** 系统 **SHALL** 执行 `mise list-all {name}`
- **AND** 返回所有可用版本列表

---

### Requirement: 版本切换
系统 **MUST** 支持切换依赖的当前版本。

#### Scenario: 切换全局版本
- **WHEN** 用户切换依赖版本
- **THEN** 系统 **SHALL**：
  1. 执行 `mise use -g {name}@{version}`
  2. 更新全局配置 `~/.config/mise/config.toml`
  3. 刷新 shim 链接

---

### Requirement: 版本卸载
系统 **MUST** 支持卸载特定版本或完全移除依赖。

#### Scenario: 卸载单个版本
- **WHEN** 用户卸载特定版本
- **THEN** 系统 **SHALL**：
  1. 执行 `mise uninstall {name}@{version}`
  2. 移除安装目录
  3. 清理 shim 链接

#### Scenario: 完全移除依赖
- **WHEN** 用户删除依赖
- **THEN** 系统 **SHALL**：
  1. 卸载所有已安装版本
  2. 从配置文件中移除
  3. 清理所有相关文件

---

### Requirement: 任务列表查询
系统 **MUST** 支持查询所有定义的 mise 任务。

#### Scenario: 获取任务列表
- **WHEN** 用户请求任务列表
- **THEN** 系统 **SHALL** 解析 mise 配置文件中的 `[tasks]` 部分
- **AND** 每个任务 **SHALL** 包含：名称、描述、命令、来源文件

---

### Requirement: 任务创建和管理
系统 **MUST** 支持创建、修改、删除 mise 任务。

#### Scenario: 创建任务
- **WHEN** 用户创建新任务
- **THEN** 系统 **SHALL**：
  1. 使用 **T02** 渲染任务配置模板
  2. 追加到 mise 配置文件 `~/.config/mise/config.toml`
- **AND** 支持配置：
  - `name`: 任务名称（必需）
  - `command`: 执行命令（必需）
  - `description`: 任务描述（可选）
  - `source`: 来源文件（可选）

#### Scenario: 更新任务
- **WHEN** 用户修改任务
- **THEN** 系统 **SHALL**：
  1. 定位任务定义
  2. 使用 **T02** 重新渲染
  3. 更新配置文件

#### Scenario: 删除任务
- **WHEN** 用户删除任务
- **THEN** 系统 **SHALL** 从配置文件中移除任务定义

---

### Requirement: 任务执行
系统 **MUST** 支持执行 mise 任务。

#### Scenario: 运行任务
- **WHEN** 用户运行任务
- **THEN** 系统 **SHALL** 执行 `mise run {task-name}`
- **AND** 捕获并返回执行输出

---

### Requirement: 安装进度日志
系统 **SHOULD** 提供实时安装/卸载日志。

#### Scenario: 流式日志输出
- **WHEN** 执行安装或卸载操作
- **THEN** 系统 **SHOULD** 捕获命令输出
- **AND** 以流式方式返回给前端
- **AND** 包含进度指示和状态信息

---

## API 端点

### 获取依赖列表

#### `GET /svcmgr/api/mise/dependencies`

**描述**: 获取所有已安装的 mise 依赖列表

**请求参数**:
- Query (可选):
  - `filter`: 过滤条件（例如 `source:mise`）
  - `sort`: 排序字段（例如 `name`, `-current_version`）

**响应** (200 OK):
```json
[
  {
    "id": "1",
    "name": "node",
    "current_version": "20.11.0",
    "latest_version": "22.4.0",
    "source": "mise",
    "installed_versions": ["18.20.0", "20.11.0"]
  },
  {
    "id": "2",
    "name": "python",
    "current_version": "3.12.1",
    "latest_version": "3.13.0",
    "source": "mise",
    "installed_versions": ["3.11.0", "3.12.1"]
  },
  {
    "id": "3",
    "name": "rust",
    "current_version": "1.82.0",
    "latest_version": "1.84.0",
    "source": "mise",
    "installed_versions": ["1.80.0", "1.82.0"]
  }
]
```

**错误响应**:
- `500 INTERNAL_ERROR`: 无法查询 mise 依赖

---

### 获取依赖详情

#### `GET /svcmgr/api/mise/dependencies/{id}`

**描述**: 获取单个依赖的详细信息

**路径参数**:
- `id`: 依赖 ID

**响应** (200 OK):
```json
{
  "id": "1",
  "name": "node",
  "current_version": "20.11.0",
  "latest_version": "22.4.0",
  "source": "mise",
  "installed_versions": ["18.20.0", "20.11.0"]
}
```

**错误响应**:
- `404 RESOURCE_NOT_FOUND`: 依赖不存在
- `500 INTERNAL_ERROR`: 查询失败

---

### 查询可用版本

#### `GET /svcmgr/api/mise/dependencies/{name}/versions`

**描述**: 查询依赖的所有可用版本

**路径参数**:
- `name`: 工具名称（例如 `node`）

**响应** (200 OK):
```json
[
  "22.4.0",
  "22.0.0",
  "21.7.0",
  "20.11.0",
  "20.0.0",
  "18.20.0",
  "18.0.0"
]
```

**错误响应**:
- `404 RESOURCE_NOT_FOUND`: 工具不存在
- `500 INTERNAL_ERROR`: 查询失败

---

### 安装依赖

#### `POST /svcmgr/api/mise/dependencies`

**描述**: 安装新的依赖或特定版本

**请求体**:
```json
{
  "name": "node",
  "version": "22.4.0"
}
```

**响应** (201 Created):
```json
{
  "id": "4",
  "name": "node",
  "current_version": "22.4.0",
  "latest_version": "22.4.0",
  "source": "mise",
  "installed_versions": ["18.20.0", "20.11.0", "22.4.0"]
}
```

**错误响应**:
- `400 VALIDATION_ERROR`: 参数验证失败（名称或版本无效）
- `409 RESOURCE_CONFLICT`: 版本已安装
- `500 OPERATION_FAILED`: 安装失败

---

### 切换依赖版本

#### `POST /svcmgr/api/mise/dependencies/{id}/switch`

**描述**: 切换依赖的当前版本

**路径参数**:
- `id`: 依赖 ID

**请求体**:
```json
{
  "version": "22.4.0"
}
```

**响应** (204 No Content)

**错误响应**:
- `404 RESOURCE_NOT_FOUND`: 依赖或版本不存在
- `400 VALIDATION_ERROR`: 版本未安装
- `500 OPERATION_FAILED`: 切换失败

---

### 卸载依赖版本

#### `DELETE /svcmgr/api/mise/dependencies/{id}/versions/{version}`

**描述**: 卸载特定版本

**路径参数**:
- `id`: 依赖 ID
- `version`: 版本号

**响应** (204 No Content)

**错误响应**:
- `404 RESOURCE_NOT_FOUND`: 依赖或版本不存在
- `400 VALIDATION_ERROR`: 不能卸载当前正在使用的版本
- `500 OPERATION_FAILED`: 卸载失败

---

### 删除依赖

#### `DELETE /svcmgr/api/mise/dependencies/{id}`

**描述**: 完全移除依赖（卸载所有版本）

**路径参数**:
- `id`: 依赖 ID

**响应** (204 No Content)

**错误响应**:
- `404 RESOURCE_NOT_FOUND`: 依赖不存在
- `500 OPERATION_FAILED`: 删除失败

---

### 获取任务列表

#### `GET /svcmgr/api/mise/tasks`

**描述**: 获取所有定义的 mise 任务

**响应** (200 OK):
```json
[
  {
    "id": "1",
    "name": "db:backup",
    "description": "Backup PostgreSQL database",
    "command": "pg_dump -U postgres mydb > backup.sql",
    "source": "~/.config/mise/config.toml"
  },
  {
    "id": "2",
    "name": "logs:clean",
    "description": "Clean old log files",
    "command": "find /var/log -name '*.log' -mtime +30 -delete",
    "source": "~/.config/mise/config.toml"
  }
]
```

**错误响应**:
- `500 INTERNAL_ERROR`: 无法读取 mise 配置

---

### 创建任务

#### `POST /svcmgr/api/mise/tasks`

**描述**: 创建新的 mise 任务

**请求体**:
```json
{
  "name": "test:unit",
  "description": "Run unit tests",
  "command": "cargo test --lib"
}
```

**响应** (201 Created):
```json
{
  "id": "3",
  "name": "test:unit",
  "description": "Run unit tests",
  "command": "cargo test --lib",
  "source": "~/.config/mise/config.toml"
}
```

**错误响应**:
- `400 VALIDATION_ERROR`: 参数验证失败
- `409 RESOURCE_CONFLICT`: 任务名称已存在
- `500 INTERNAL_ERROR`: 创建失败

---

### 更新任务

#### `PUT /svcmgr/api/mise/tasks/{id}`

**描述**: 更新任务配置

**路径参数**:
- `id`: 任务 ID

**请求体** (部分更新):
```json
{
  "description": "Run all unit tests",
  "command": "cargo test --lib --all"
}
```

**响应** (200 OK):
```json
{
  "id": "3",
  "name": "test:unit",
  "description": "Run all unit tests",
  "command": "cargo test --lib --all",
  "source": "~/.config/mise/config.toml"
}
```

**错误响应**:
- `404 RESOURCE_NOT_FOUND`: 任务不存在
- `400 VALIDATION_ERROR`: 参数验证失败
- `500 INTERNAL_ERROR`: 更新失败

---

### 删除任务

#### `DELETE /svcmgr/api/mise/tasks/{id}`

**描述**: 删除任务

**路径参数**:
- `id`: 任务 ID

**响应** (204 No Content)

**错误响应**:
- `404 RESOURCE_NOT_FOUND`: 任务不存在
- `500 INTERNAL_ERROR`: 删除失败

---

### 运行任务

#### `POST /svcmgr/api/mise/tasks/{name}/run`

**描述**: 执行 mise 任务

**路径参数**:
- `name`: 任务名称

**响应** (200 OK):
```json
{
  "output": "Running unit tests...\ntest result: ok. 42 passed; 0 failed; 0 ignored\n",
  "exit_code": 0,
  "duration_ms": 1234
}
```

**错误响应**:
- `404 RESOURCE_NOT_FOUND`: 任务不存在
- `500 OPERATION_FAILED`: 执行失败

---

## 数据模型

### Rust 类型定义

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiseDependency {
    /// 依赖 ID（唯一标识符）
    pub id: String,
    
    /// 工具名称（例如 node, python, rust）
    pub name: String,
    
    /// 当前激活版本
    pub current_version: String,
    
    /// 最新可用版本（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_version: Option<String>,
    
    /// 来源（mise, asdf-plugin 等）
    pub source: String,
    
    /// 已安装版本列表（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installed_versions: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiseTask {
    /// 任务 ID（唯一标识符）
    pub id: String,
    
    /// 任务名称（例如 db:backup）
    pub name: String,
    
    /// 任务描述（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    
    /// 执行命令
    pub command: String,
    
    /// 来源文件（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InstallDependencyRequest {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SwitchVersionRequest {
    pub version: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateTaskRequest {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateTaskRequest {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub command: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskExecutionResult {
    pub output: String,
    pub exit_code: i32,
    pub duration_ms: u64,
}
```

---

## 接口定义

```rust
use async_trait::async_trait;

#[async_trait]
pub trait MiseFeature {
    /// 获取所有依赖列表
    async fn list_dependencies(&self) -> Result<Vec<MiseDependency>, ApiError>;
    
    /// 获取单个依赖详情
    async fn get_dependency(&self, id: &str) -> Result<MiseDependency, ApiError>;
    
    /// 查询可用版本
    async fn list_available_versions(&self, name: &str) -> Result<Vec<String>, ApiError>;
    
    /// 安装依赖
    async fn install_dependency(&self, request: InstallDependencyRequest) -> Result<MiseDependency, ApiError>;
    
    /// 切换版本
    async fn switch_version(&self, id: &str, version: &str) -> Result<(), ApiError>;
    
    /// 卸载版本
    async fn uninstall_version(&self, id: &str, version: &str) -> Result<(), ApiError>;
    
    /// 删除依赖
    async fn delete_dependency(&self, id: &str) -> Result<(), ApiError>;
    
    /// 获取所有任务列表
    async fn list_tasks(&self) -> Result<Vec<MiseTask>, ApiError>;
    
    /// 创建任务
    async fn create_task(&self, request: CreateTaskRequest) -> Result<MiseTask, ApiError>;
    
    /// 更新任务
    async fn update_task(&self, id: &str, request: UpdateTaskRequest) -> Result<MiseTask, ApiError>;
    
    /// 删除任务
    async fn delete_task(&self, id: &str) -> Result<(), ApiError>;
    
    /// 运行任务
    async fn run_task(&self, name: &str) -> Result<TaskExecutionResult, ApiError>;
}
```

---

## 配置项

```toml
[mise]
# mise 配置文件路径
config_path = "~/.config/mise/config.toml"

# mise 安装目录
install_dir = "~/.local/share/mise"

# 任务执行超时（秒）
task_timeout = 300

# 是否自动查询最新版本
auto_check_updates = true
```

---

## 内置模板

### mise-task.toml.j2

```jinja2
[tasks.{{ name }}]
run = "{{ command }}"
{% if description %}
description = "{{ description }}"
{% endif %}
```

---

## Mise 配置文件示例

### ~/.config/mise/config.toml

```toml
# Global tool versions
[tools]
node = "20.11.0"
python = "3.12.1"
rust = "1.82.0"

# Task definitions
[tasks.db-backup]
run = "pg_dump -U postgres mydb > backup.sql"
description = "Backup PostgreSQL database"

[tasks.logs-clean]
run = "find /var/log -name '*.log' -mtime +30 -delete"
description = "Clean old log files"

[tasks.test-unit]
run = "cargo test --lib"
description = "Run unit tests"
```

---

## 注意事项

1. **版本管理**: mise 支持多版本并存,切换版本不会删除旧版本
2. **全局 vs 项目**: 本规范关注全局配置 `~/.config/mise/config.toml`
3. **Shim 机制**: mise 使用 shim 实现版本切换,需要正确配置 PATH
4. **任务命名**: 建议使用 `category:action` 格式（例如 `db:backup`）
5. **日志输出**: 安装/卸载操作应提供实时进度反馈

---

**变更历史**：
- 2026-02-21: 初始版本,基于前端原型提取
