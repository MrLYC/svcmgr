# Feature: Config File Management (F06)

**特性代号**: F06  
**原子依赖**: A01 (Git 配置版本)  
**功能目标**: 提供配置文件 Git 版本管理能力，支持多目录追踪、变更监控、提交历史和回滚

---

## 需求说明

### 概述

系统 **必须** 提供配置文件管理功能，允许用户通过 Web 界面和 REST API 管理多个配置目录的 Git 版本控制。所有配置变更通过 Git 仓库记录完整历史，支持选择性提交和版本回滚。

### 核心能力

1. **托管目录管理**: 添加/删除需要版本控制的配置目录
2. **目录列表查询**: 获取所有托管目录及其 Git 状态
3. **状态查询**: 查看目录的 Git 状态（clean/dirty、分支、未提交变更数）
4. **变更查询**: 获取目录中所有未提交的文件变更（added/modified/deleted）
5. **提交历史查询**: 查看目录的 Git 提交历史
6. **提交变更**: 选择性提交文件变更到 Git 仓库
7. **版本回滚**: 回滚到指定的历史提交

### 技术约束

- **API 基础路径**: `/svcmgr/api/config`
- **配置存储**: 用户指定的配置目录（如 `~/.config/svcmgr`、`/etc/nginx` 等）
- **版本管理**: 每个托管目录维护独立的 Git 仓库
- **用户级**: 仅操作用户权限内的配置文件
- **Git 原子**: 所有 Git 操作通过 A01 原子完成

---

## ADDED Requirements

### Requirement: 托管目录管理
系统 **必须** 支持添加和删除需要版本控制的配置目录。

#### Scenario: 添加托管目录
- **WHEN** 客户端发送 `POST /svcmgr/api/config/dirs` 请求
- **AND** 请求体包含 `path` 和 `label` 字段
- **AND** 目录路径存在且可访问
- **THEN** 系统 **应当** 在该目录中初始化 Git 仓库（如果尚未初始化）
- **AND** 系统 **应当** 创建托管目录配置记录
- **AND** HTTP 响应状态码 **应当** 为 `201 Created`

#### Scenario: 目录路径不存在
- **WHEN** 客户端尝试添加不存在的目录
- **THEN** 系统 **应当** 返回错误响应
- **AND** 错误类型 **应当** 为 `NOT_FOUND`
- **AND** HTTP 响应状态码 **应当** 为 `404 Not Found`

#### Scenario: 目录权限不足
- **WHEN** 客户端尝试添加无访问权限的目录
- **THEN** 系统 **应当** 返回错误响应
- **AND** 错误类型 **应当** 为 `PERMISSION_DENIED`
- **AND** HTTP 响应状态码 **应当** 为 `403 Forbidden`

#### Scenario: 删除托管目录
- **WHEN** 客户端发送 `DELETE /svcmgr/api/config/dirs/{id}` 请求
- **AND** 托管目录 `{id}` 存在
- **THEN** 系统 **应当** 删除托管目录配置记录
- **AND** 系统 **不应当** 删除目录本身或其 Git 仓库
- **AND** HTTP 响应状态码 **应当** 为 `204 No Content`

---

### Requirement: 目录列表查询
系统 **必须** 提供 REST API 端点用于查询所有托管目录及其 Git 状态。

#### Scenario: 查询所有托管目录
- **WHEN** 客户端发送 `GET /svcmgr/api/config/dirs` 请求
- **THEN** 系统 **应当** 返回所有托管目录的 JSON 数组
- **AND** 每个目录对象 **应当** 包含 `id, path, label, git_status, branch, uncommitted_changes, last_commit_message, last_commit_time` 字段
- **AND** `git_status` 字段 **必须** 为 `"clean" | "dirty"` 之一
- **AND** HTTP 响应状态码 **应当** 为 `200 OK`

#### Scenario: 空托管目录列表
- **WHEN** 系统中无任何托管目录
- **THEN** 系统 **应当** 返回空数组 `[]`
- **AND** HTTP 响应状态码 **应当** 为 `200 OK`

---

### Requirement: 状态查询
系统 **必须** 提供 REST API 端点用于查询目录的详细 Git 状态。

#### Scenario: 查询目录状态
- **WHEN** 客户端发送 `GET /svcmgr/api/config/status?dir={dirId}` 请求
- **AND** 托管目录 `{dirId}` 存在
- **THEN** 系统 **应当** 通过 Git 原子查询仓库状态
- **AND** 系统 **应当** 返回包含 `path, git_status, branch, uncommitted_changes, last_commit` 的对象
- **AND** HTTP 响应状态码 **应当** 为 `200 OK`

#### Scenario: 查询所有目录总状态
- **WHEN** 客户端发送 `GET /svcmgr/api/config/status` 请求（不指定 `dir` 参数）
- **THEN** 系统 **应当** 返回所有托管目录的聚合状态
- **AND** HTTP 响应状态码 **应当** 为 `200 OK`

---

### Requirement: 变更查询
系统 **必须** 提供 REST API 端点用于查询目录中所有未提交的文件变更。

#### Scenario: 查询目录变更
- **WHEN** 客户端发送 `GET /svcmgr/api/config/changes?dir={dirId}` 请求
- **AND** 托管目录 `{dirId}` 存在
- **THEN** 系统 **应当** 通过 Git 原子查询未提交的文件变更
- **AND** 系统 **应当** 返回变更文件列表，每个对象包含 `file, status, diff` 字段
- **AND** `status` 字段 **必须** 为 `"modified" | "added" | "deleted"` 之一
- **AND** HTTP 响应状态码 **应当** 为 `200 OK`

#### Scenario: 查询所有目录变更
- **WHEN** 客户端发送 `GET /svcmgr/api/config/changes` 请求（不指定 `dir` 参数）
- **THEN** 系统 **应当** 返回所有托管目录的聚合变更列表
- **AND** HTTP 响应状态码 **应当** 为 `200 OK`

#### Scenario: 无变更
- **WHEN** 查询的目录无任何未提交变更
- **THEN** 系统 **应当** 返回空数组 `[]`
- **AND** HTTP 响应状态码 **应当** 为 `200 OK`

---

### Requirement: 提交历史查询
系统 **必须** 提供 REST API 端点用于查询目录的 Git 提交历史。

#### Scenario: 查询目录提交历史
- **WHEN** 客户端发送 `GET /svcmgr/api/config/commits?dir={dirId}` 请求
- **AND** 托管目录 `{dirId}` 存在
- **THEN** 系统 **应当** 通过 Git 原子查询提交历史
- **AND** 系统 **应当** 返回提交列表（按时间倒序），每个对象包含 `hash, message, author, timestamp` 字段
- **AND** HTTP 响应状态码 **应当** 为 `200 OK`

#### Scenario: 查询所有目录提交历史
- **WHEN** 客户端发送 `GET /svcmgr/api/config/commits` 请求（不指定 `dir` 参数）
- **THEN** 系统 **应当** 返回所有托管目录的聚合提交历史
- **AND** HTTP 响应状态码 **应当** 为 `200 OK`

---

### Requirement: 提交变更
系统 **必须** 提供 REST API 端点用于选择性提交文件变更到 Git 仓库。

#### Scenario: 提交指定文件
- **WHEN** 客户端发送 `POST /svcmgr/api/config/commit` 请求
- **AND** 请求体包含 `message` (提交消息) 和 `files` (文件路径列表)
- **AND** 可选参数 `dir_id` 指定目录
- **THEN** 系统 **应当** 通过 Git 原子将指定文件暂存（git add）
- **AND** 系统 **应当** 创建 Git 提交并使用提供的消息
- **AND** HTTP 响应状态码 **应当** 为 `201 Created`

#### Scenario: 提交消息缺失
- **WHEN** 客户端尝试提交时未提供 `message` 字段
- **THEN** 系统 **应当** 返回错误响应
- **AND** 错误类型 **应当** 为 `VALIDATION_ERROR`
- **AND** HTTP 响应状态码 **应当** 为 `422 Unprocessable Entity`

#### Scenario: 提交文件不存在或无变更
- **WHEN** 客户端尝试提交不存在变更的文件
- **THEN** 系统 **应当** 返回错误响应
- **AND** 错误类型 **应当** 为 `INVALID_REQUEST`
- **AND** HTTP 响应状态码 **应当** 为 `400 Bad Request`

---

### Requirement: 版本回滚
系统 **必须** 提供 REST API 端点用于回滚到指定的历史提交。

#### Scenario: 回滚到指定提交
- **WHEN** 客户端发送 `POST /svcmgr/api/config/rollback/{hash}` 请求
- **AND** 请求体包含可选参数 `dir_id` 指定目录
- **AND** 提交哈希 `{hash}` 存在于目录的 Git 历史中
- **THEN** 系统 **应当** 通过 Git 原子回滚到指定提交（git reset --hard）
- **AND** HTTP 响应状态码 **应当** 为 `204 No Content`

#### Scenario: 提交哈希不存在
- **WHEN** 客户端尝试回滚到不存在的提交
- **THEN** 系统 **应当** 返回错误响应
- **AND** 错误类型 **应当** 为 `NOT_FOUND`
- **AND** HTTP 响应状态码 **应当** 为 `404 Not Found`

#### Scenario: 回滚导致未提交变更丢失
- **WHEN** 执行回滚操作时目录存在未提交变更
- **THEN** 系统 **应当** 返回错误响应
- **AND** 错误类型 **应当** 为 `CONFLICT`
- **AND** 错误消息 **应当** 提示用户先提交或丢弃未提交变更
- **AND** HTTP 响应状态码 **应当** 为 `409 Conflict`

---

## REST API 接口规范

### 1. 获取所有托管目录

#### `GET /svcmgr/api/config/dirs`

**描述**: 获取所有托管配置目录及其 Git 状态

**请求参数**: 无

**响应** (200):
```json
[
  {
    "id": "dir-001",
    "path": "/home/user/.config/svcmgr",
    "label": "svcmgr",
    "git_status": "dirty",
    "branch": "main",
    "uncommitted_changes": 3,
    "last_commit_message": "feat: add nginx proxy configuration",
    "last_commit_time": "2026-02-21T10:30:00Z"
  },
  {
    "id": "dir-002",
    "path": "/home/user/.config/systemd/user",
    "label": "systemd services",
    "git_status": "clean",
    "branch": "main",
    "uncommitted_changes": 0,
    "last_commit_message": "fix: update ttyd service port",
    "last_commit_time": "2026-02-20T18:45:00Z"
  }
]
```

**错误响应**:
- `500 INTERNAL_ERROR`: 系统内部错误

---

### 2. 添加托管目录

#### `POST /svcmgr/api/config/dirs`

**描述**: 添加新的配置目录到版本控制

**请求体**:
```json
{
  "path": "/home/user/.config/svcmgr",
  "label": "svcmgr"
}
```

**字段说明**:
- `path` (string, **必需**): 配置目录绝对路径
- `label` (string, **必需**): 目录标签（用于标识）

**响应** (201):
```json
{
  "id": "dir-003",
  "path": "/home/user/.config/svcmgr",
  "label": "svcmgr",
  "git_status": "clean",
  "branch": "main",
  "uncommitted_changes": 0
}
```

**错误响应**:
- `400 INVALID_REQUEST`: 请求格式错误
- `403 PERMISSION_DENIED`: 目录权限不足
- `404 NOT_FOUND`: 目录不存在
- `422 VALIDATION_ERROR`: 字段验证失败
- `500 INTERNAL_ERROR`: 添加失败

---

### 3. 删除托管目录

#### `DELETE /svcmgr/api/config/dirs/{id}`

**描述**: 从托管列表移除目录（不删除目录本身）

**路径参数**:
- `id` (string): 托管目录 ID

**响应** (204):
无响应体

**错误响应**:
- `404 NOT_FOUND`: 托管目录不存在
- `500 INTERNAL_ERROR`: 删除失败

---

### 4. 查询目录状态

#### `GET /svcmgr/api/config/status`

**描述**: 查询目录的 Git 状态

**查询参数**:
- `dir` (string, 可选): 托管目录 ID（不提供则返回所有目录的聚合状态）

**响应** (200):
```json
{
  "path": "/home/user/.config/svcmgr",
  "git_status": "dirty",
  "branch": "main",
  "uncommitted_changes": 3,
  "last_commit": "feat: add nginx proxy configuration"
}
```

**错误响应**:
- `404 NOT_FOUND`: 托管目录不存在
- `500 INTERNAL_ERROR`: 查询失败

---

### 5. 查询未提交变更

#### `GET /svcmgr/api/config/changes`

**描述**: 查询目录中所有未提交的文件变更

**查询参数**:
- `dir` (string, 可选): 托管目录 ID（不提供则返回所有目录的聚合变更）

**响应** (200):
```json
[
  {
    "file": "nginx/conf.d/proxy-001.conf",
    "status": "added",
    "diff": "+location /tty/session-001 {\n+    proxy_pass http://127.0.0.1:7681;\n+}"
  },
  {
    "file": "systemd/user/svcmgr-tty-001.service",
    "status": "modified",
    "diff": "-ExecStart=/usr/bin/ttyd --port 7680\n+ExecStart=/usr/bin/ttyd --port 7681"
  },
  {
    "file": "old-config.toml",
    "status": "deleted"
  }
]
```

**错误响应**:
- `404 NOT_FOUND`: 托管目录不存在
- `500 INTERNAL_ERROR`: 查询失败

---

### 6. 查询提交历史

#### `GET /svcmgr/api/config/commits`

**描述**: 查询目录的 Git 提交历史

**查询参数**:
- `dir` (string, 可选): 托管目录 ID（不提供则返回所有目录的聚合历史）

**响应** (200):
```json
[
  {
    "hash": "a1b2c3d4",
    "message": "feat: add nginx proxy configuration",
    "author": "user <user@example.com>",
    "timestamp": "2026-02-21T10:30:00Z"
  },
  {
    "hash": "e5f6g7h8",
    "message": "fix: update ttyd service port",
    "author": "user <user@example.com>",
    "timestamp": "2026-02-20T18:45:00Z"
  }
]
```

**错误响应**:
- `404 NOT_FOUND`: 托管目录不存在
- `500 INTERNAL_ERROR`: 查询失败

---

### 7. 提交变更

#### `POST /svcmgr/api/config/commit`

**描述**: 选择性提交文件变更到 Git 仓库

**请求体**:
```json
{
  "message": "feat: add new TTY session configuration",
  "files": [
    "systemd/user/svcmgr-tty-002.service",
    "nginx/conf.d/tty-002.conf"
  ],
  "dir_id": "dir-001"
}
```

**字段说明**:
- `message` (string, **必需**): 提交消息
- `files` (string[], **必需**): 要提交的文件路径列表
- `dir_id` (string, 可选): 托管目录 ID（不提供则提交所有托管目录的变更）

**响应** (201):
```json
{
  "hash": "i9j0k1l2",
  "message": "feat: add new TTY session configuration",
  "author": "user <user@example.com>",
  "timestamp": "2026-02-21T12:00:00Z"
}
```

**错误响应**:
- `400 INVALID_REQUEST`: 请求格式错误或文件无变更
- `422 VALIDATION_ERROR`: 字段验证失败
- `500 INTERNAL_ERROR`: 提交失败

---

### 8. 回滚版本

#### `POST /svcmgr/api/config/rollback/{hash}`

**描述**: 回滚到指定的历史提交

**路径参数**:
- `hash` (string): Git 提交哈希（短哈希或完整哈希）

**请求体**:
```json
{
  "dir_id": "dir-001"
}
```

**字段说明**:
- `dir_id` (string, 可选): 托管目录 ID（不提供则回滚所有托管目录）

**响应** (204):
无响应体

**错误响应**:
- `404 NOT_FOUND`: 提交哈希不存在
- `409 CONFLICT`: 存在未提交变更，无法回滚
- `500 INTERNAL_ERROR`: 回滚失败

---

## Rust 数据类型定义

### 托管目录对象

```rust
use serde::{Deserialize, Serialize};

/// 托管配置目录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedDirectory {
    /// 托管目录唯一标识符
    pub id: String,
    
    /// 目录绝对路径
    pub path: String,
    
    /// 目录标签
    pub label: String,
    
    /// Git 状态
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_status: Option<GitStatus>,
    
    /// 当前分支名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    
    /// 未提交变更数量
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uncommitted_changes: Option<i32>,
    
    /// 最后提交消息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_commit_message: Option<String>,
    
    /// 最后提交时间 (ISO 8601)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_commit_time: Option<String>,
}

/// Git 状态
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GitStatus {
    /// 工作区干净，无未提交变更
    Clean,
    
    /// 工作区脏，存在未提交变更
    Dirty,
}
```

### 配置状态对象

```rust
/// 配置目录状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigStatus {
    /// 目录路径
    pub path: String,
    
    /// Git 状态
    pub git_status: GitStatus,
    
    /// 当前分支
    pub branch: String,
    
    /// 未提交变更数量
    pub uncommitted_changes: i32,
    
    /// 最后提交消息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_commit: Option<String>,
}
```

### 变更对象

```rust
/// 配置文件变更
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigChange {
    /// 文件路径（相对于目录根）
    pub file: String,
    
    /// 变更状态
    pub status: ChangeStatus,
    
    /// 变更差异（git diff 输出）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
}

/// 变更状态
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeStatus {
    /// 文件已修改
    Modified,
    
    /// 文件已添加
    Added,
    
    /// 文件已删除
    Deleted,
}
```

### 提交对象

```rust
/// Git 提交记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigCommit {
    /// 提交哈希（短哈希）
    pub hash: String,
    
    /// 提交消息
    pub message: String,
    
    /// 提交作者
    pub author: String,
    
    /// 提交时间 (ISO 8601)
    pub timestamp: String,
}
```

### 请求类型

```rust
/// 添加托管目录请求
#[derive(Debug, Clone, Deserialize)]
pub struct AddManagedDirRequest {
    /// 目录绝对路径
    pub path: String,
    
    /// 目录标签
    pub label: String,
}

/// 提交变更请求
#[derive(Debug, Clone, Deserialize)]
pub struct CommitConfigRequest {
    /// 提交消息
    pub message: String,
    
    /// 要提交的文件列表
    pub files: Vec<String>,
    
    /// 托管目录 ID（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir_id: Option<String>,
}

/// 回滚版本请求
#[derive(Debug, Clone, Deserialize)]
pub struct RollbackConfigRequest {
    /// 托管目录 ID（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir_id: Option<String>,
}
```

---

## Rust Trait 接口定义

```rust
use async_trait::async_trait;
use crate::error::ApiError;

/// 配置文件管理特性
#[async_trait]
pub trait ConfigFeature {
    /// 获取所有托管目录
    async fn list_managed_dirs(&self) -> Result<Vec<ManagedDirectory>, ApiError>;
    
    /// 添加托管目录
    async fn add_managed_dir(&self, request: AddManagedDirRequest) -> Result<ManagedDirectory, ApiError>;
    
    /// 删除托管目录
    async fn remove_managed_dir(&self, id: &str) -> Result<(), ApiError>;
    
    /// 查询目录状态
    async fn get_config_status(&self, dir_id: Option<&str>) -> Result<ConfigStatus, ApiError>;
    
    /// 查询未提交变更
    async fn get_config_changes(&self, dir_id: Option<&str>) -> Result<Vec<ConfigChange>, ApiError>;
    
    /// 查询提交历史
    async fn get_config_commits(&self, dir_id: Option<&str>) -> Result<Vec<ConfigCommit>, ApiError>;
    
    /// 提交变更
    async fn commit_config(&self, request: CommitConfigRequest) -> Result<ConfigCommit, ApiError>;
    
    /// 回滚版本
    async fn rollback_config(&self, hash: &str, dir_id: Option<&str>) -> Result<(), ApiError>;
}
```

### 实现说明

```rust
/// 配置管理功能实现
pub struct ConfigManager {
    managed_dirs: HashMap<String, PathBuf>,
    git_atom: Arc<dyn GitAtom>,
}

#[async_trait]
impl ConfigFeature for ConfigManager {
    async fn list_managed_dirs(&self) -> Result<Vec<ManagedDirectory>, ApiError> {
        // 1. 遍历所有托管目录
        // 2. 对每个目录调用 Git 原子查询状态
        // 3. 组装 ManagedDirectory 对象
        // 4. 返回目录列表
        todo!()
    }
    
    async fn add_managed_dir(&self, request: AddManagedDirRequest) -> Result<ManagedDirectory, ApiError> {
        // 1. 验证目录路径存在且可访问
        // 2. 检查目录是否已存在 Git 仓库
        // 3. 如果不存在，通过 Git 原子初始化仓库
        // 4. 生成托管目录 ID
        // 5. 保存托管目录配置
        // 6. 返回 ManagedDirectory 对象
        todo!()
    }
    
    async fn remove_managed_dir(&self, id: &str) -> Result<(), ApiError> {
        // 1. 检查托管目录是否存在
        // 2. 删除托管目录配置记录
        // 3. 不删除目录本身或其 Git 仓库
        todo!()
    }
    
    async fn get_config_status(&self, dir_id: Option<&str>) -> Result<ConfigStatus, ApiError> {
        // 1. 如果指定 dir_id，查询单个目录状态
        // 2. 如果不指定，返回所有目录的聚合状态
        // 3. 通过 Git 原子查询仓库状态（git status --porcelain）
        // 4. 解析未提交变更数量
        // 5. 返回 ConfigStatus 对象
        todo!()
    }
    
    async fn get_config_changes(&self, dir_id: Option<&str>) -> Result<Vec<ConfigChange>, ApiError> {
        // 1. 如果指定 dir_id，查询单个目录变更
        // 2. 如果不指定，返回所有目录的聚合变更
        // 3. 通过 Git 原子查询未提交变更（git status --porcelain）
        // 4. 解析变更文件和状态（added/modified/deleted）
        // 5. 可选：通过 Git 原子获取文件差异（git diff）
        // 6. 返回 ConfigChange 列表
        todo!()
    }
    
    async fn get_config_commits(&self, dir_id: Option<&str>) -> Result<Vec<ConfigCommit>, ApiError> {
        // 1. 如果指定 dir_id，查询单个目录历史
        // 2. 如果不指定，返回所有目录的聚合历史
        // 3. 通过 Git 原子查询提交历史（git log --pretty=format:...)
        // 4. 解析提交哈希、消息、作者、时间
        // 5. 返回 ConfigCommit 列表（按时间倒序）
        todo!()
    }
    
    async fn commit_config(&self, request: CommitConfigRequest) -> Result<ConfigCommit, ApiError> {
        // 1. 验证请求数据（message 非空，files 非空）
        // 2. 如果指定 dir_id，在该目录执行提交
        // 3. 如果不指定，在所有托管目录执行提交
        // 4. 通过 Git 原子暂存文件（git add）
        // 5. 通过 Git 原子创建提交（git commit -m）
        // 6. 返回新创建的 ConfigCommit 对象
        todo!()
    }
    
    async fn rollback_config(&self, hash: &str, dir_id: Option<&str>) -> Result<(), ApiError> {
        // 1. 检查是否存在未提交变更
        // 2. 如果存在，返回 CONFLICT 错误
        // 3. 如果指定 dir_id，在该目录执行回滚
        // 4. 如果不指定，在所有托管目录执行回滚
        // 5. 通过 Git 原子回滚到指定提交（git reset --hard {hash}）
        todo!()
    }
}
```

---

## 错误码定义

```rust
#[derive(Debug, Serialize)]
#[serde(tag = "error", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConfigError {
    /// 托管目录或提交不存在
    NotFound { message: String },
    
    /// 目录权限不足
    PermissionDenied { message: String },
    
    /// 存在未提交变更，无法回滚
    Conflict { message: String },
    
    /// 验证错误
    ValidationError { message: String, field: Option<String> },
    
    /// 无效请求（如文件无变更）
    InvalidRequest { message: String },
    
    /// 内部错误
    InternalError { message: String },
}
```

---

## 实施检查清单

### Phase 1: 基础托管目录管理
- [ ] 实现 `ConfigFeature` trait
- [ ] 实现托管目录添加/删除
- [ ] 实现托管目录列表查询
- [ ] 集成 Git 原子初始化仓库

### Phase 2: 状态和变更查询
- [ ] 实现 Git 状态查询（git status）
- [ ] 实现未提交变更查询（git status --porcelain）
- [ ] 实现文件差异查询（git diff）
- [ ] 实现提交历史查询（git log）

### Phase 3: 提交和回滚
- [ ] 实现选择性文件提交（git add + git commit）
- [ ] 实现版本回滚（git reset --hard）
- [ ] 实现回滚前的未提交变更检测

### Phase 4: 高级功能
- [ ] 支持多目录聚合查询
- [ ] 支持提交消息验证
- [ ] 支持回滚冲突检测和提示

### Phase 5: 测试
- [ ] 单元测试：托管目录 CRUD 操作
- [ ] 单元测试：Git 状态和变更查询
- [ ] 单元测试：提交和回滚逻辑
- [ ] 集成测试：Git 原子集成
- [ ] 端到端测试：完整配置管理流程

---

## 相关文档

- [API 设计规范](./20-api-design.md)
- [Git 配置版本原子 (A01)](./01-atom-git.md)
- [前端 UI 设计](./30-frontend-ui.md)
