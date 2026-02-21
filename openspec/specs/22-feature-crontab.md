# F02: Crontab 任务管理

> 版本：1.0.0  
> 状态：DRAFT  
> 依赖原子：T02 (模板), T07 (crontab)  
> 最后更新：2026-02-21

## 概述

Crontab 任务管理功能提供对用户级 crontab 定时任务的完整生命周期管理,包括任务的创建、更新、删除、启用/禁用、执行历史查看。通过组合模板原子和 crontab 原子实现。

---

## ADDED Requirements

### Requirement: 任务列表查询
系统 **MUST** 支持查询所有用户级 crontab 任务。

#### Scenario: 获取任务列表
- **WHEN** 用户请求任务列表
- **THEN** 系统 **SHALL** 解析 `crontab -l` 输出
- **AND** 每个任务 **SHALL** 包含：ID、cron 表达式、命令、启用状态、描述、最后运行时间

#### Scenario: 任务状态识别
- **WHEN** 解析 crontab 文件
- **THEN** 系统 **SHALL**：
  - 以 `#` 开头的行视为注释或禁用任务
  - 正常行视为启用任务
  - 使用注释中的描述标签（例如 `# Description: 备份任务`）

---

### Requirement: 任务创建
系统 **MUST** 支持创建新的 crontab 任务。

#### Scenario: 创建任务
- **WHEN** 用户创建新任务
- **THEN** 系统 **SHALL**：
  1. 验证 cron 表达式格式（5 或 6 字段）
  2. 读取当前 crontab
  3. 追加新任务行
  4. 使用 `crontab -` 更新
- **AND** 支持配置：
  - `expression`: cron 表达式（必需）
  - `command`: 执行命令（必需）
  - `description`: 任务描述（可选）
  - `enabled`: 是否启用（默认 true）

#### Scenario: Cron 表达式验证
- **WHEN** 验证 cron 表达式
- **THEN** 系统 **SHALL** 接受格式：
  - 标准 5 字段：`分 时 日 月 周`
  - 扩展 6 字段：`秒 分 时 日 月 周`（如果 cron 支持）
- **AND** 支持特殊字符：`*`, `*/N`, `N-M`, `N,M`

---

### Requirement: 任务更新
系统 **MUST** 支持修改现有任务。

#### Scenario: 更新任务
- **WHEN** 用户修改任务
- **THEN** 系统 **SHALL**：
  1. 根据任务 ID 定位原任务
  2. 替换任务行
  3. 保留其他任务不变
  4. 使用 `crontab -` 更新
- **AND** 支持修改：cron 表达式、命令、描述、启用状态

---

### Requirement: 任务删除
系统 **MUST** 支持删除任务。

#### Scenario: 删除任务
- **WHEN** 用户删除任务
- **THEN** 系统 **SHALL**：
  1. 根据任务 ID 定位任务
  2. 从 crontab 中移除该行
  3. 使用 `crontab -` 更新

---

### Requirement: 任务启用/禁用
系统 **MUST** 支持切换任务启用状态。

#### Scenario: 禁用任务
- **WHEN** 用户禁用任务
- **THEN** 系统 **SHALL** 在任务行前添加 `#` 注释符

#### Scenario: 启用任务
- **WHEN** 用户启用任务
- **THEN** 系统 **SHALL** 移除任务行前的 `#` 注释符

---

### Requirement: 预设时间表达式
系统 **SHOULD** 提供常用时间表达式预设。

#### Scenario: 预设列表
- **WHEN** 用户创建或编辑任务
- **THEN** 系统 **SHOULD** 提供预设：
  - `* * * * *` - 每分钟
  - `*/5 * * * *` - 每 5 分钟
  - `0 * * * *` - 每小时
  - `0 0 * * *` - 每天午夜
  - `0 2 * * *` - 每天凌晨 2 点
  - `0 0 * * 0` - 每周日午夜
  - `0 0 1 * *` - 每月 1 号午夜

---

### Requirement: 任务执行历史
系统 **SHOULD** 记录任务执行历史。

#### Scenario: 记录最后运行时间
- **WHEN** cron 任务执行
- **THEN** 系统 **SHOULD** 更新任务的最后运行时间
- **AND** 可通过包装命令实现（例如记录到日志文件）

---

## API 端点

### 获取任务列表

#### `GET /svcmgr/api/crontab/tasks`

**描述**: 获取所有用户级 crontab 任务列表

**请求参数**:
- Query (可选):
  - `filter`: 过滤条件（例如 `enabled:true`）
  - `sort`: 排序字段（例如 `expression`, `-last_run`）

**响应** (200 OK):
```json
[
  {
    "id": "1",
    "expression": "0 2 * * *",
    "command": "/usr/local/bin/backup.sh",
    "enabled": true,
    "description": "Daily backup at 2 AM",
    "last_run": "2026-02-21T02:00:00Z"
  },
  {
    "id": "2",
    "expression": "*/5 * * * *",
    "command": "/usr/local/bin/health-check.sh",
    "enabled": true,
    "description": "Health check every 5 minutes",
    "last_run": "2026-02-21T10:55:00Z"
  },
  {
    "id": "3",
    "expression": "0 0 * * 0",
    "command": "/usr/local/bin/cleanup.sh",
    "enabled": false,
    "description": "Weekly cleanup on Sunday"
  }
]
```

**错误响应**:
- `500 INTERNAL_ERROR`: 无法读取 crontab

---

### 获取任务详情

#### `GET /svcmgr/api/crontab/tasks/{id}`

**描述**: 获取单个任务的详细信息

**路径参数**:
- `id`: 任务 ID

**响应** (200 OK):
```json
{
  "id": "1",
  "expression": "0 2 * * *",
  "command": "/usr/local/bin/backup.sh",
  "enabled": true,
  "description": "Daily backup at 2 AM",
  "last_run": "2026-02-21T02:00:00Z"
}
```

**错误响应**:
- `404 RESOURCE_NOT_FOUND`: 任务不存在
- `500 INTERNAL_ERROR`: 查询失败

---

### 创建任务

#### `POST /svcmgr/api/crontab/tasks`

**描述**: 创建新的 crontab 任务

**请求体**:
```json
{
  "expression": "0 3 * * *",
  "command": "/usr/local/bin/sync-data.sh",
  "description": "Data sync at 3 AM",
  "enabled": true
}
```

**响应** (201 Created):
```json
{
  "id": "4",
  "expression": "0 3 * * *",
  "command": "/usr/local/bin/sync-data.sh",
  "enabled": true,
  "description": "Data sync at 3 AM"
}
```

**错误响应**:
- `400 VALIDATION_ERROR`: cron 表达式无效或命令为空
- `500 INTERNAL_ERROR`: 创建失败

---

### 更新任务

#### `PUT /svcmgr/api/crontab/tasks/{id}`

**描述**: 更新现有任务

**路径参数**:
- `id`: 任务 ID

**请求体** (部分更新):
```json
{
  "expression": "0 4 * * *",
  "command": "/usr/local/bin/sync-data.sh --full",
  "description": "Full data sync at 4 AM",
  "enabled": true
}
```

**响应** (200 OK):
```json
{
  "id": "4",
  "expression": "0 4 * * *",
  "command": "/usr/local/bin/sync-data.sh --full",
  "enabled": true,
  "description": "Full data sync at 4 AM"
}
```

**错误响应**:
- `404 RESOURCE_NOT_FOUND`: 任务不存在
- `400 VALIDATION_ERROR`: 参数验证失败
- `500 INTERNAL_ERROR`: 更新失败

---

### 删除任务

#### `DELETE /svcmgr/api/crontab/tasks/{id}`

**描述**: 删除任务

**路径参数**:
- `id`: 任务 ID

**响应** (204 No Content)

**错误响应**:
- `404 RESOURCE_NOT_FOUND`: 任务不存在
- `500 INTERNAL_ERROR`: 删除失败

---

### 切换任务启用状态

#### `POST /svcmgr/api/crontab/tasks/{id}/toggle`

**描述**: 启用或禁用任务

**路径参数**:
- `id`: 任务 ID

**请求体**:
```json
{
  "enabled": false
}
```

**响应** (204 No Content)

**错误响应**:
- `404 RESOURCE_NOT_FOUND`: 任务不存在
- `400 VALIDATION_ERROR`: 参数错误
- `500 OPERATION_FAILED`: 操作失败

---

## 数据模型

### Rust 类型定义

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrontabTask {
    /// 任务 ID（唯一标识符，基于行号或内容哈希生成）
    pub id: String,
    
    /// Cron 表达式（例如 "0 2 * * *"）
    pub expression: String,
    
    /// 执行命令
    pub command: String,
    
    /// 是否启用
    pub enabled: bool,
    
    /// 任务描述（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    
    /// 最后运行时间（ISO 8601 格式）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateCrontabRequest {
    pub expression: String,
    pub command: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateCrontabRequest {
    #[serde(default)]
    pub expression: Option<String>,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ToggleCrontabRequest {
    pub enabled: bool,
}
```

---

## 接口定义

```rust
use async_trait::async_trait;

#[async_trait]
pub trait CrontabFeature {
    /// 获取所有任务列表
    async fn list_tasks(&self) -> Result<Vec<CrontabTask>, ApiError>;
    
    /// 获取单个任务详情
    async fn get_task(&self, id: &str) -> Result<CrontabTask, ApiError>;
    
    /// 创建新任务
    async fn create_task(&self, request: CreateCrontabRequest) -> Result<CrontabTask, ApiError>;
    
    /// 更新任务
    async fn update_task(&self, id: &str, request: UpdateCrontabRequest) -> Result<CrontabTask, ApiError>;
    
    /// 删除任务
    async fn delete_task(&self, id: &str) -> Result<(), ApiError>;
    
    /// 切换任务启用状态
    async fn toggle_task(&self, id: &str, enabled: bool) -> Result<(), ApiError>;
    
    /// 验证 cron 表达式
    fn validate_expression(&self, expression: &str) -> Result<(), ApiError>;
}
```

---

## 配置项

```toml
[crontab]
# Crontab 存储路径（通常无需配置，使用系统默认）
# crontab_path = "crontab -l"

# 任务执行历史存储路径
history_file = "~/.local/share/svcmgr/crontab-history.json"

# 最大历史记录数
max_history_entries = 1000
```

---

## 内置模板

### crontab-task.j2

```jinja2
{% if not enabled %}# {% endif %}{{ expression }} {{ command }}{% if description %} # Description: {{ description }}{% endif %}
```

### crontab-file.j2

```jinja2
# svcmgr managed crontab
# Do not edit manually - use svcmgr CLI or Web UI

{% for task in tasks %}
{% if not task.enabled %}# {% endif %}{{ task.expression }} {{ task.command }}{% if task.description %} # Description: {{ task.description }}{% endif %}
{% endfor %}
```

---

## Crontab 文件格式

### 启用的任务
```
0 2 * * * /usr/local/bin/backup.sh # Description: Daily backup
```

### 禁用的任务
```
# 0 0 * * 0 /usr/local/bin/cleanup.sh # Description: Weekly cleanup
```

### 完整示例
```
# svcmgr managed crontab
# Do not edit manually

0 2 * * * /usr/local/bin/backup.sh # Description: Daily backup at 2 AM
*/5 * * * * /usr/local/bin/health-check.sh # Description: Health check every 5 minutes
# 0 0 * * 0 /usr/local/bin/cleanup.sh # Description: Weekly cleanup (disabled)
```

---

## 预设 Cron 表达式

| 描述 | Cron 表达式 | 说明 |
|-----|-------------|------|
| 每分钟 | `* * * * *` | 用于测试 |
| 每 5 分钟 | `*/5 * * * *` | 常用监控任务 |
| 每小时 | `0 * * * *` | 每小时整点 |
| 每天午夜 | `0 0 * * *` | 日常清理任务 |
| 每天凌晨 2 点 | `0 2 * * *` | 备份任务 |
| 每周日午夜 | `0 0 * * 0` | 周度汇总 |
| 每月 1 号 | `0 0 1 * *` | 月度报告 |

---

## 注意事项

1. **任务 ID 生成**: 基于任务行号或内容哈希生成唯一 ID
2. **描述标签**: 使用注释中的 `# Description:` 提取描述
3. **原子性更新**: 所有修改先读取完整 crontab，修改后整体更新
4. **备份机制**: 修改前备份当前 crontab
5. **表达式验证**: 严格验证 cron 表达式，避免错误任务
6. **执行历史**: 需要包装命令以记录执行时间

---

**变更历史**：
- 2026-02-21: 初始版本，基于前端原型提取
