# Feature: TTY Session Management (F07)

**特性代号**: F07  
**原子依赖**: A02 (模板引擎), A04 (Systemd 服务管理), A07 (Nginx 代理)  
**功能目标**: 提供基于 ttyd 的 Web 终端会话管理能力，支持多实例、认证、生命周期管理

---

## 需求说明

### 概述

系统 **必须** 提供 Web TTY 会话管理功能，允许用户通过 Web 界面和 REST API 管理基于 ttyd 的终端会话。每个会话运行在独立的 systemd 用户服务中，通过 Nginx 代理提供统一访问入口。

### 核心能力

1. **会话列表查询**: 获取所有 TTY 会话及其状态
2. **会话详情查询**: 获取单个会话的完整信息
3. **会话创建**: 创建新的 Web 终端会话（支持自定义命令）
4. **会话启动**: 启动已停止的会话
5. **会话更新**: 修改会话配置（名称、命令、密码保护）
6. **会话删除**: 停止并移除会话
7. **状态监控**: 实时查看会话运行状态

### 技术约束

- **API 基础路径**: `/svcmgr/api/tty`
- **Web 访问路径**: `/tty/{session-id}` (通过 Nginx 代理)
- **后端进程**: ttyd (通过 systemd --user 管理)
- **配置存储**: `~/.local/share/svcmgr/tty/sessions/`
- **版本管理**: 所有配置变更必须通过 Git 原子提交
- **用户级**: 使用用户级 systemd 服务（不使用 sudo）
- **模板驱动**: systemd 单元文件和 nginx 配置通过 Jinja2 模板生成

---

## ADDED Requirements

### Requirement: 会话列表查询
系统 **必须** 提供 REST API 端点用于查询所有 TTY 会话配置和状态。

#### Scenario: 查询所有会话
- **WHEN** 客户端发送 `GET /svcmgr/api/tty/sessions` 请求
- **THEN** 系统 **应当** 返回所有会话配置的 JSON 数组
- **AND** 每个会话对象 **应当** 包含 `id, name, command, url, status, created_at, password` 字段
- **AND** `status` 字段 **必须** 为 `"running" | "stopped"` 之一
- **AND** HTTP 响应状态码 **应当** 为 `200 OK`

#### Scenario: 空会话列表
- **WHEN** 系统中无任何已创建会话
- **THEN** 系统 **应当** 返回空数组 `[]`
- **AND** HTTP 响应状态码 **应当** 为 `200 OK`

---

### Requirement: 会话详情查询
系统 **必须** 提供 REST API 端点用于查询单个会话的详细信息。

#### Scenario: 查询已存在会话
- **WHEN** 客户端发送 `GET /svcmgr/api/tty/sessions/{id}` 请求
- **AND** 会话 `{id}` 存在
- **THEN** 系统 **应当** 返回该会话的完整配置信息
- **AND** HTTP 响应状态码 **应当** 为 `200 OK`

#### Scenario: 查询不存在会话
- **WHEN** 客户端发送 `GET /svcmgr/api/tty/sessions/{id}` 请求
- **AND** 会话 `{id}` 不存在
- **THEN** 系统 **应当** 返回错误响应
- **AND** 错误类型 **应当** 为 `NOT_FOUND`
- **AND** HTTP 响应状态码 **应当** 为 `404 Not Found`

---

### Requirement: 会话创建
系统 **必须** 提供 REST API 端点用于创建新的 TTY 会话。

#### Scenario: 创建合法会话
- **WHEN** 客户端发送 `POST /svcmgr/api/tty/sessions` 请求
- **AND** 请求体包含必需字段 `name, command`
- **AND** `command` 为合法的 shell 命令
- **THEN** 系统 **应当** 创建新的会话配置文件
- **AND** 系统 **应当** 通过模板引擎生成 systemd 单元文件
- **AND** 系统 **应当** 通过模板引擎生成 nginx 代理配置
- **AND** 系统 **应当** 通过 Git 原子提交配置变更
- **AND** 系统 **应当** 返回创建成功的会话对象（状态为 `stopped`）
- **AND** HTTP 响应状态码 **应当** 为 `201 Created`

#### Scenario: 会话名称冲突
- **WHEN** 客户端创建会话
- **AND** 请求的 `name` 已被其他会话占用
- **THEN** 系统 **应当** 返回错误响应
- **AND** 错误类型 **应当** 为 `CONFLICT`
- **AND** 错误消息 **应当** 说明名称冲突
- **AND** HTTP 响应状态码 **应当** 为 `409 Conflict`

#### Scenario: 必需字段缺失
- **WHEN** 客户端创建会话
- **AND** 请求体缺少必需字段（`name` 或 `command`）
- **THEN** 系统 **应当** 返回错误响应
- **AND** 错误类型 **应当** 为 `VALIDATION_ERROR`
- **AND** HTTP 响应状态码 **应当** 为 `422 Unprocessable Entity`

#### Scenario: 密码保护会话
- **WHEN** 客户端创建会话
- **AND** 请求体包含 `password: true`
- **THEN** 系统 **应当** 配置 ttyd 启用 HTTP 基本认证
- **AND** 系统 **应当** 生成随机密码
- **AND** 会话对象 **应当** 包含 `password: true` 字段

---

### Requirement: 会话启动
系统 **必须** 提供 REST API 端点用于启动已停止的会话。

#### Scenario: 启动已停止会话
- **WHEN** 客户端发送 `POST /svcmgr/api/tty/sessions/{id}/start` 请求
- **AND** 会话 `{id}` 存在且状态为 `stopped`
- **THEN** 系统 **应当** 通过 systemd 原子启动会话服务
- **AND** 系统 **应当** 重新加载 nginx 配置
- **AND** 会话状态 **应当** 变为 `running`
- **AND** HTTP 响应状态码 **应当** 为 `204 No Content`

#### Scenario: 启动已运行会话
- **WHEN** 客户端尝试启动已运行的会话
- **THEN** 系统 **应当** 返回成功响应（幂等操作）
- **AND** HTTP 响应状态码 **应当** 为 `204 No Content`

#### Scenario: 启动不存在会话
- **WHEN** 客户端尝试启动不存在的会话
- **THEN** 系统 **应当** 返回错误响应
- **AND** 错误类型 **应当** 为 `NOT_FOUND`
- **AND** HTTP 响应状态码 **应当** 为 `404 Not Found`

---

### Requirement: 会话更新
系统 **必须** 提供 REST API 端点用于更新已存在会话的配置。

#### Scenario: 更新会话配置
- **WHEN** 客户端发送 `PUT /svcmgr/api/tty/sessions/{id}` 请求
- **AND** 会话 `{id}` 存在
- **AND** 请求体包含需要更新的字段（`name`, `command`, `password`）
- **THEN** 系统 **应当** 更新会话配置文件
- **AND** 如果会话正在运行，**应当** 重启会话服务使配置生效
- **AND** 系统 **应当** 通过 Git 原子提交配置变更
- **AND** 系统 **应当** 返回更新后的会话对象
- **AND** HTTP 响应状态码 **应当** 为 `200 OK`

#### Scenario: 更新不存在会话
- **WHEN** 客户端尝试更新不存在的会话
- **THEN** 系统 **应当** 返回错误响应
- **AND** 错误类型 **应当** 为 `NOT_FOUND`
- **AND** HTTP 响应状态码 **应当** 为 `404 Not Found`

#### Scenario: 更新导致名称冲突
- **WHEN** 客户端更新会话的 `name` 字段
- **AND** 新名称已被其他会话占用
- **THEN** 系统 **应当** 返回错误响应
- **AND** 错误类型 **应当** 为 `CONFLICT`
- **AND** HTTP 响应状态码 **应当** 为 `409 Conflict`

---

### Requirement: 会话删除
系统 **必须** 提供 REST API 端点用于删除已存在的会话。

#### Scenario: 删除会话
- **WHEN** 客户端发送 `DELETE /svcmgr/api/tty/sessions/{id}` 请求
- **AND** 会话 `{id}` 存在
- **THEN** 系统 **应当** 通过 systemd 原子停止并禁用会话服务
- **AND** 系统 **应当** 删除 systemd 单元文件
- **AND** 系统 **应当** 删除 nginx 代理配置
- **AND** 系统 **应当** 重新加载 nginx
- **AND** 系统 **应当** 删除会话配置文件
- **AND** 系统 **应当** 通过 Git 原子提交配置变更
- **AND** HTTP 响应状态码 **应当** 为 `204 No Content`

#### Scenario: 删除不存在会话
- **WHEN** 客户端尝试删除不存在的会话
- **THEN** 系统 **应当** 返回错误响应
- **AND** 错误类型 **应当** 为 `NOT_FOUND`
- **AND** HTTP 响应状态码 **应当** 为 `404 Not Found`

---

### Requirement: 状态监控
系统 **必须** 实时监控所有会话的运行状态。

#### Scenario: 状态检测
- **WHEN** 系统查询会话列表或详情
- **THEN** 系统 **应当** 通过 systemd 原子检测服务状态
- **AND** 如果 systemd 服务处于 `active (running)` 状态，**应当** 设置会话状态为 `"running"`
- **AND** 如果 systemd 服务处于 `inactive (dead)` 或 `failed` 状态，**应当** 设置会话状态为 `"stopped"`

---

## REST API 接口规范

### 1. 获取所有会话

#### `GET /svcmgr/api/tty/sessions`

**描述**: 获取所有 TTY 会话配置及状态

**请求参数**: 无

**响应** (200):
```json
[
  {
    "id": "session-001",
    "name": "Main Terminal",
    "command": "/bin/bash",
    "url": "/tty/session-001",
    "status": "running",
    "created_at": "2026-02-21T10:30:00Z",
    "password": false
  },
  {
    "id": "session-002",
    "name": "Python Dev",
    "command": "/bin/bash -c 'cd ~/projects/myapp && python'",
    "url": "/tty/session-002",
    "status": "stopped",
    "created_at": "2026-02-20T15:20:00Z",
    "password": true
  }
]
```

**错误响应**:
- `500 INTERNAL_ERROR`: 系统内部错误

---

### 2. 获取会话详情

#### `GET /svcmgr/api/tty/sessions/{id}`

**描述**: 获取指定会话的详细配置信息

**路径参数**:
- `id` (string): 会话 ID

**响应** (200):
```json
{
  "id": "session-001",
  "name": "Main Terminal",
  "command": "/bin/bash",
  "url": "/tty/session-001",
  "status": "running",
  "created_at": "2026-02-21T10:30:00Z",
  "password": false
}
```

**错误响应**:
- `404 NOT_FOUND`: 会话不存在
- `500 INTERNAL_ERROR`: 系统内部错误

---

### 3. 创建会话

#### `POST /svcmgr/api/tty/sessions`

**描述**: 创建新的 TTY 会话（初始状态为 `stopped`）

**请求体**:
```json
{
  "name": "Main Terminal",
  "command": "/bin/bash",
  "password": false
}
```

**字段说明**:
- `name` (string, **必需**): 会话名称
- `command` (string, **必需**): 启动的 shell 命令（默认 `/bin/bash`）
- `password` (boolean, 可选): 是否启用密码保护（默认 `false`）

**响应** (201):
```json
{
  "id": "session-003",
  "name": "Main Terminal",
  "command": "/bin/bash",
  "url": "/tty/session-003",
  "status": "stopped",
  "created_at": "2026-02-21T12:00:00Z",
  "password": false
}
```

**错误响应**:
- `400 INVALID_REQUEST`: 请求格式错误
- `409 CONFLICT`: 会话名称已被占用
- `422 VALIDATION_ERROR`: 字段验证失败（缺少必需字段、格式错误）
- `500 INTERNAL_ERROR`: 创建失败

---

### 4. 启动会话

#### `POST /svcmgr/api/tty/sessions/{id}/start`

**描述**: 启动已停止的会话（幂等操作）

**路径参数**:
- `id` (string): 会话 ID

**响应** (204):
无响应体

**错误响应**:
- `404 NOT_FOUND`: 会话不存在
- `500 INTERNAL_ERROR`: 启动失败

---

### 5. 更新会话

#### `PUT /svcmgr/api/tty/sessions/{id}`

**描述**: 更新会话配置（支持部分更新，运行中会话将重启）

**路径参数**:
- `id` (string): 会话 ID

**请求体** (支持部分字段):
```json
{
  "name": "Updated Terminal",
  "command": "/bin/zsh",
  "password": true
}
```

**响应** (200):
```json
{
  "id": "session-001",
  "name": "Updated Terminal",
  "command": "/bin/zsh",
  "url": "/tty/session-001",
  "status": "running",
  "created_at": "2026-02-21T10:30:00Z",
  "password": true
}
```

**错误响应**:
- `400 INVALID_REQUEST`: 请求格式错误
- `404 NOT_FOUND`: 会话不存在
- `409 CONFLICT`: 名称冲突
- `422 VALIDATION_ERROR`: 字段验证失败
- `500 INTERNAL_ERROR`: 更新失败

---

### 6. 删除会话

#### `DELETE /svcmgr/api/tty/sessions/{id}`

**描述**: 停止并删除指定会话（移除所有配置）

**路径参数**:
- `id` (string): 会话 ID

**响应** (204):
无响应体

**错误响应**:
- `404 NOT_FOUND`: 会话不存在
- `500 INTERNAL_ERROR`: 删除失败

---

## Rust 数据类型定义

### 会话对象

```rust
use serde::{Deserialize, Serialize};

/// TTY 会话配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TTYSession {
    /// 会话唯一标识符
    pub id: String,
    
    /// 会话名称
    pub name: String,
    
    /// 启动的 shell 命令
    pub command: String,
    
    /// Web 访问 URL (如 /tty/session-001)
    pub url: String,
    
    /// 会话运行状态
    pub status: SessionStatus,
    
    /// 创建时间 (ISO 8601 格式)
    pub created_at: String,
    
    /// 是否启用密码保护
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<bool>,
}

/// 会话运行状态
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    /// 正在运行
    Running,
    
    /// 已停止
    Stopped,
}
```

### 请求类型

```rust
/// 创建会话请求
#[derive(Debug, Clone, Deserialize)]
pub struct CreateSessionRequest {
    /// 会话名称
    pub name: String,
    
    /// 启动的 shell 命令
    pub command: String,
    
    /// 是否启用密码保护
    #[serde(default)]
    pub password: bool,
}

/// 更新会话请求（所有字段可选）
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateSessionRequest {
    /// 新的会话名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    
    /// 新的启动命令
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    
    /// 新的密码保护设置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<bool>,
}
```

### 验证逻辑

```rust
impl CreateSessionRequest {
    /// 验证请求数据
    pub fn validate(&self) -> Result<(), ValidationError> {
        // 验证 name 非空
        if self.name.trim().is_empty() {
            return Err(ValidationError::EmptyField("name"));
        }
        
        // 验证 command 非空
        if self.command.trim().is_empty() {
            return Err(ValidationError::EmptyField("command"));
        }
        
        Ok(())
    }
}
```

---

## Rust Trait 接口定义

```rust
use async_trait::async_trait;
use crate::error::ApiError;

/// TTY 会话管理特性
#[async_trait]
pub trait TTYFeature {
    /// 获取所有会话配置
    async fn list_sessions(&self) -> Result<Vec<TTYSession>, ApiError>;
    
    /// 获取指定会话详情
    async fn get_session(&self, id: &str) -> Result<TTYSession, ApiError>;
    
    /// 创建新会话（初始状态为 stopped）
    async fn create_session(&self, request: CreateSessionRequest) -> Result<TTYSession, ApiError>;
    
    /// 启动会话
    async fn start_session(&self, id: &str) -> Result<(), ApiError>;
    
    /// 更新会话配置（如果正在运行，将重启）
    async fn update_session(&self, id: &str, request: UpdateSessionRequest) -> Result<TTYSession, ApiError>;
    
    /// 删除会话（停止并移除所有配置）
    async fn delete_session(&self, id: &str) -> Result<(), ApiError>;
}
```

### 实现说明

```rust
/// TTY 会话功能实现
pub struct TTYManager {
    config_dir: PathBuf,
    git_atom: Arc<dyn GitAtom>,
    template_engine: Arc<dyn TemplateEngine>,
    systemd_atom: Arc<dyn SystemdAtom>,
    proxy_atom: Arc<dyn ProxyAtom>,
}

#[async_trait]
impl TTYFeature for TTYManager {
    async fn list_sessions(&self) -> Result<Vec<TTYSession>, ApiError> {
        // 1. 扫描配置目录 (~/.local/share/svcmgr/tty/sessions/)
        // 2. 解析每个会话配置文件
        // 3. 通过 systemd 原子检测服务状态
        // 4. 返回会话列表
        todo!()
    }
    
    async fn get_session(&self, id: &str) -> Result<TTYSession, ApiError> {
        // 1. 检查配置文件是否存在
        // 2. 解析会话配置
        // 3. 通过 systemd 原子检测服务状态
        // 4. 返回会话对象
        todo!()
    }
    
    async fn create_session(&self, request: CreateSessionRequest) -> Result<TTYSession, ApiError> {
        // 1. 验证请求数据
        // 2. 检查名称冲突
        // 3. 生成会话 ID (UUID)
        // 4. 分配 ttyd 端口（自动查找可用端口）
        // 5. 通过模板引擎生成 systemd 单元文件
        // 6. 通过模板引擎生成 nginx 代理配置
        // 7. 写入配置文件到 ~/.local/share/svcmgr/tty/sessions/{id}.toml
        // 8. 通过 systemd 原子加载单元文件（但不启动）
        // 9. 通过 proxy 原子重新加载 nginx
        // 10. 通过 Git 原子提交配置
        // 11. 返回创建的会话对象（状态为 stopped）
        todo!()
    }
    
    async fn start_session(&self, id: &str) -> Result<(), ApiError> {
        // 1. 检查会话是否存在
        // 2. 通过 systemd 原子启动服务
        // 3. 幂等：如果已运行，直接返回成功
        todo!()
    }
    
    async fn update_session(&self, id: &str, request: UpdateSessionRequest) -> Result<TTYSession, ApiError> {
        // 1. 检查会话是否存在
        // 2. 验证更新数据
        // 3. 如果更新 name，检查名称冲突
        // 4. 更新配置文件（保留未修改字段）
        // 5. 重新生成 systemd 单元文件和 nginx 配置
        // 6. 如果会话正在运行，通过 systemd 原子重启服务
        // 7. 通过 proxy 原子重新加载 nginx
        // 8. 通过 Git 原子提交配置
        // 9. 返回更新后的会话对象
        todo!()
    }
    
    async fn delete_session(&self, id: &str) -> Result<(), ApiError> {
        // 1. 检查会话是否存在
        // 2. 通过 systemd 原子停止并禁用服务
        // 3. 删除 systemd 单元文件
        // 4. 删除 nginx 代理配置
        // 5. 通过 proxy 原子重新加载 nginx
        // 6. 删除会话配置文件
        // 7. 通过 Git 原子提交配置
        todo!()
    }
}
```

---

## 配置文件示例

### 会话配置 (TOML)

**路径**: `~/.local/share/svcmgr/tty/sessions/{session-id}.toml`

```toml
# TTY Session Configuration
# Generated by svcmgr at 2026-02-21T10:30:00Z

id = "session-001"
name = "Main Terminal"
command = "/bin/bash"
port = 7681
url = "/tty/session-001"
password = false
created_at = "2026-02-21T10:30:00Z"
```

### Systemd 单元文件

**路径**: `~/.config/systemd/user/svcmgr-tty-{session-id}.service`

```ini
[Unit]
Description=svcmgr TTY Session: Main Terminal
After=network.target

[Service]
Type=simple
ExecStart=/usr/bin/ttyd --port 7681 --writable /bin/bash
Restart=on-failure
RestartSec=5s

[Install]
WantedBy=default.target
```

### Nginx 代理配置

**路径**: `~/.local/share/svcmgr/nginx/conf.d/tty-{session-id}.conf`

```nginx
location /tty/session-001 {
    proxy_pass http://127.0.0.1:7681;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
}
```

---

## 内置模板 (Jinja2)

### Systemd 单元文件模板

**路径**: `src/templates/tty/session.service.j2`

```jinja2
[Unit]
Description=svcmgr TTY Session: {{ session_name }}
After=network.target

[Service]
Type=simple
ExecStart=/usr/bin/ttyd --port {{ port }} --writable{% if password %} --credential {{ username }}:{{ password_hash }}{% endif %} {{ command }}
Restart=on-failure
RestartSec=5s

[Install]
WantedBy=default.target
```

**模板变量**:
- `session_name` (string): 会话名称
- `port` (int): ttyd 监听端口
- `command` (string): 启动的 shell 命令
- `password` (boolean): 是否启用密码保护
- `username` (string, 可选): 认证用户名（固定为 "admin"）
- `password_hash` (string, 可选): 认证密码的 bcrypt 哈希

### Nginx 代理配置模板

**路径**: `src/templates/tty/proxy.conf.j2`

```jinja2
# TTY Session: {{ session_name }}
# Generated at {{ timestamp }}

location {{ url_path }} {
    proxy_pass http://127.0.0.1:{{ port }};
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
}
```

**模板变量**:
- `session_name` (string): 会话名称
- `url_path` (string): URL 访问路径（如 `/tty/session-001`）
- `port` (int): ttyd 监听端口
- `timestamp` (string): 生成时间戳（ISO 8601 格式）

---

## 错误码定义

```rust
#[derive(Debug, Serialize)]
#[serde(tag = "error", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TTYError {
    /// 会话不存在
    NotFound { message: String },
    
    /// 会话名称冲突
    Conflict { message: String, conflicting_name: String },
    
    /// 验证错误
    ValidationError { message: String, field: Option<String> },
    
    /// 内部错误
    InternalError { message: String },
}
```

---

## 实施检查清单

### Phase 1: 基础会话管理
- [ ] 实现 `TTYFeature` trait
- [ ] 实现会话配置文件扫描和解析
- [ ] 实现会话 CRUD 操作
- [ ] 集成 Git 原子提交配置变更
- [ ] 实现模板驱动的配置文件生成

### Phase 2: Systemd 和 Nginx 集成
- [ ] 实现 systemd 单元文件生成和管理
- [ ] 实现 nginx 代理配置生成和管理
- [ ] 实现会话启动/停止/重启
- [ ] 实现端口自动分配（查找可用端口）

### Phase 3: 状态监控
- [ ] 实现 systemd 服务状态检测
- [ ] 实现会话状态枚举逻辑 (running/stopped)

### Phase 4: 高级功能
- [ ] 支持密码保护（生成随机密码 + bcrypt 哈希）
- [ ] 支持名称冲突检测
- [ ] 支持会话配置热重载（更新时自动重启）

### Phase 5: 测试
- [ ] 单元测试：会话 CRUD 操作
- [ ] 单元测试：请求验证逻辑
- [ ] 单元测试：名称冲突检测
- [ ] 集成测试：Systemd 集成
- [ ] 集成测试：Nginx 集成
- [ ] 集成测试：Git 版本管理集成
- [ ] 端到端测试：完整会话生命周期

---

## 相关文档

- [API 设计规范](./20-api-design.md)
- [Systemd 服务管理原子 (A04)](./04-atom-systemd.md)
- [Nginx 代理管理原子 (A07)](./07-atom-proxy.md)
- [模板引擎原子 (A02)](./02-atom-template.md)
- [Git 配置版本原子 (A01)](./01-atom-git.md)
- [前端 UI 设计](./30-frontend-ui.md)
