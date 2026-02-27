# 服务管理 API

## Design Goal

定义 svcmgr 服务（Service）管理的完整 REST API 规范，涵盖服务的创建、查询、更新、删除、生命周期控制（启动/停止/重启）、日志访问和健康检查。

## Why

服务管理是 svcmgr 的核心功能之一，API 设计需要：
- **明确性**：清晰定义服务的生命周期状态转换和操作语义
- **安全性**:确保非法状态转换被拒绝，防止资源泄漏
- **可观测性**：提供日志、状态、资源使用等观测能力
- **一致性**：与内部 Process Manager 和 Scheduler Engine 行为保持一致
- **RESTful**：遵循 10-api-overview.md 中定义的 REST 原则

## Service Data Model

### 1. Service Definition

**服务定义结构**（对应 `svcmgr.toml` 中的 `[services.*]` 段）：

```rust
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

/// 服务定义（创建/更新请求使用）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDefinition {
    /// 服务名称（唯一标识）
    pub name: String,
    
    /// 执行命令（可以是 mise task 引用或直接命令）
    pub command: String,
    
    /// 工作目录（默认为当前项目目录）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>,
    
    /// 环境变量（会与 mise 的 [env] 合并）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
    
    /// 暴露的端口配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ports: Option<Vec<PortMapping>>,
    
    /// 健康检查配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health_check: Option<HealthCheckConfig>,
    
    /// 资源限制（可选，cgroups v2 支持）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourceLimits>,
    
    /// 重启策略
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart_policy: Option<RestartPolicy>,
    
    /// 自动启动（进程管理器启动时自动启动此服务）
    #[serde(default)]
    pub autostart: bool,
    
    /// 依赖的其他服务（启动前确保依赖服务已运行）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depends_on: Option<Vec<String>>,
}

/// 端口映射配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortMapping {
    /// 主机端口（监听端口）
    pub host: u16,
    
    /// 容器/服务端口（服务实际监听端口）
    pub container: u16,
    
    /// 协议（tcp/udp）
    #[serde(default = "default_protocol")]
    pub protocol: String,
}

fn default_protocol() -> String {
    "tcp".to_string()
}

/// 健康检查配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum HealthCheckConfig {
    /// HTTP 健康检查
    Http {
        /// 健康检查 URL
        url: String,
        /// 期望的 HTTP 状态码
        #[serde(default = "default_http_status")]
        expected_status: u16,
        /// 超时时间（秒）
        #[serde(default = "default_timeout")]
        timeout: u64,
        /// 检查间隔（秒）
        #[serde(default = "default_interval")]
        interval: u64,
    },
    
    /// TCP 端口检查
    Tcp {
        /// 主机地址
        #[serde(default = "default_host")]
        host: String,
        /// 端口号
        port: u16,
        /// 超时时间（秒）
        #[serde(default = "default_timeout")]
        timeout: u64,
        /// 检查间隔（秒）
        #[serde(default = "default_interval")]
        interval: u64,
    },
    
    /// 命令执行检查
    Command {
        /// 执行的命令
        command: String,
        /// 超时时间（秒）
        #[serde(default = "default_timeout")]
        timeout: u64,
        /// 检查间隔（秒）
        #[serde(default = "default_interval")]
        interval: u64,
    },
}

fn default_http_status() -> u16 { 200 }
fn default_timeout() -> u64 { 5 }
fn default_interval() -> u64 { 10 }
fn default_host() -> String { "127.0.0.1".to_string() }

/// 资源限制配置（cgroups v2）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// CPU 限制（核心数，如 1.5）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu: Option<f64>,
    
    /// 内存限制（字节数）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<u64>,
    
    /// 内存限制（人类可读格式，如 "512M", "2G"）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_str: Option<String>,
}

/// 重启策略
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum RestartPolicy {
    /// 永不重启
    No,
    
    /// 失败时重启（退出码非 0）
    OnFailure,
    
    /// 总是重启（无论退出码）
    Always,
}

impl Default for RestartPolicy {
    fn default() -> Self {
        Self::OnFailure
    }
}
```

### 2. Service Status

**服务运行时状态**（查询服务时返回）：

```rust
/// 服务完整状态（包含定义 + 运行时状态）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatus {
    /// 服务定义
    #[serde(flatten)]
    pub definition: ServiceDefinition,
    
    /// 运行时状态
    pub runtime: ServiceRuntime,
}

/// 服务运行时状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceRuntime {
    /// 状态（stopped, starting, running, stopping, failed）
    pub state: ServiceState,
    
    /// 进程 PID（仅 running 状态有值）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    
    /// 运行时长（秒，仅 running 状态有值）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uptime: Option<u64>,
    
    /// 启动时间（Unix timestamp）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<i64>,
    
    /// 停止时间（Unix timestamp，仅 stopped/failed 状态有值）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stopped_at: Option<i64>,
    
    /// 退出码（仅 stopped/failed 状态有值）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    
    /// 重启次数
    pub restart_count: u32,
    
    /// 健康状态（仅配置了健康检查且服务 running 时有值）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health: Option<HealthStatus>,
    
    /// 资源使用情况（仅 running 状态有值）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourceUsage>,
    
    /// 错误消息（仅 failed 状态有值）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 服务状态枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ServiceState {
    /// 已停止
    Stopped,
    
    /// 正在启动
    Starting,
    
    /// 正在运行
    Running,
    
    /// 正在停止
    Stopping,
    
    /// 失败（进程异常退出或启动失败）
    Failed,
}

/// 健康状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    /// 健康状态（healthy, unhealthy, unknown）
    pub status: HealthState,
    
    /// 最后检查时间（Unix timestamp）
    pub last_check: i64,
    
    /// 连续成功次数
    pub consecutive_successes: u32,
    
    /// 连续失败次数
    pub consecutive_failures: u32,
    
    /// 检查消息（失败时的错误信息）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HealthState {
    Healthy,
    Unhealthy,
    Unknown,
}

/// 资源使用情况
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// CPU 使用率（百分比，0.0-100.0）
    pub cpu_percent: f64,
    
    /// 内存使用量（字节）
    pub memory_bytes: u64,
    
    /// 内存使用量（人类可读）
    pub memory_str: String,
}
```

## API Endpoints

### 1. List Services

**列出所有服务**

```http
GET /api/v1/services
```

**Query Parameters**:
```
?status=running,stopped   # 按状态过滤（逗号分隔多个状态）
?autostart=true           # 仅显示自动启动的服务
?page=1                   # 页码（默认 1）
&per_page=20              # 每页数量（默认 20）
?sort=name                # 排序字段（name, uptime, restart_count）
?fields=name,status,pid   # 字段选择（减少响应体大小）
```

**Response** (200 OK):
```json
{
  "data": [
    {
      "name": "web-server",
      "command": "mise run server",
      "working_dir": "/home/user/myproject",
      "env": {
        "PORT": "8080"
      },
      "ports": [
        {
          "host": 8080,
          "container": 8080,
          "protocol": "tcp"
        }
      ],
      "health_check": {
        "type": "http",
        "url": "http://localhost:8080/health",
        "expected_status": 200,
        "timeout": 5,
        "interval": 10
      },
      "restart_policy": "on-failure",
      "autostart": true,
      "runtime": {
        "state": "running",
        "pid": 12345,
        "uptime": 3600,
        "started_at": 1709654400,
        "restart_count": 0,
        "health": {
          "status": "healthy",
          "last_check": 1709658000,
          "consecutive_successes": 15,
          "consecutive_failures": 0
        },
        "resources": {
          "cpu_percent": 2.5,
          "memory_bytes": 134217728,
          "memory_str": "128 MB"
        }
      }
    },
    {
      "name": "background-worker",
      "command": "python worker.py",
      "working_dir": "/home/user/myproject",
      "restart_policy": "always",
      "autostart": false,
      "runtime": {
        "state": "stopped",
        "stopped_at": 1709654300,
        "exit_code": 0,
        "restart_count": 0
      }
    }
  ],
  "pagination": {
    "page": 1,
    "per_page": 20,
    "total": 2,
    "total_pages": 1,
    "has_next": false,
    "has_prev": false
  }
}
```

**Error Responses**:
- `400 Bad Request` - 无效的查询参数（如 `status=invalid`）
- `500 Internal Server Error` - 服务器内部错误

---

### 2. Get Service

**获取单个服务的详细信息**

```http
GET /api/v1/services/{name}
```

**Path Parameters**:
- `name` (string, required) - 服务名称

**Response** (200 OK):
```json
{
  "data": {
    "name": "web-server",
    "command": "mise run server",
    "working_dir": "/home/user/myproject",
    "env": {
      "PORT": "8080",
      "NODE_ENV": "production"
    },
    "ports": [
      {
        "host": 8080,
        "container": 8080,
        "protocol": "tcp"
      }
    ],
    "health_check": {
      "type": "http",
      "url": "http://localhost:8080/health",
      "expected_status": 200,
      "timeout": 5,
      "interval": 10
    },
    "resources": {
      "cpu": 2.0,
      "memory": 2147483648,
      "memory_str": "2G"
    },
    "restart_policy": "on-failure",
    "autostart": true,
    "depends_on": ["database"],
    "runtime": {
      "state": "running",
      "pid": 12345,
      "uptime": 7200,
      "started_at": 1709654400,
      "restart_count": 0,
      "health": {
        "status": "healthy",
        "last_check": 1709661600,
        "consecutive_successes": 72,
        "consecutive_failures": 0
      },
      "resources": {
        "cpu_percent": 3.2,
        "memory_bytes": 536870912,
        "memory_str": "512 MB"
      }
    }
  }
}
```

**Error Responses**:
- `404 Not Found` - 服务不存在
```json
{
  "error": {
    "code": "RESOURCE_NOT_FOUND",
    "message": "Service 'invalid-name' does not exist",
    "details": {
      "service": "invalid-name",
      "available_services": ["web-server", "background-worker"]
    },
    "request_id": "req_7f8a9b2c"
  }
}
```

---

### 3. Create Service

**创建新服务**

```http
POST /api/v1/services
Content-Type: application/json
```

**Request Body**:
```json
{
  "name": "api-server",
  "command": "mise run api",
  "working_dir": "/home/user/myproject",
  "env": {
    "PORT": "3000",
    "DB_URL": "postgresql://localhost/mydb"
  },
  "ports": [
    {
      "host": 3000,
      "container": 3000,
      "protocol": "tcp"
    }
  ],
  "health_check": {
    "type": "http",
    "url": "http://localhost:3000/health",
    "expected_status": 200,
    "timeout": 5,
    "interval": 10
  },
  "resources": {
    "cpu": 1.0,
    "memory_str": "1G"
  },
  "restart_policy": "on-failure",
  "autostart": true,
  "depends_on": ["database"]
}
```

**Validation Rules**:
- `name`: 必填，1-64 字符，仅允许字母、数字、连字符、下划线
- `command`: 必填，非空字符串
- `working_dir`: 可选，必须是绝对路径且存在
- `env`: 可选，键和值都是非空字符串
- `ports`: 可选，端口号 1-65535
- `health_check.timeout`: 1-300 秒
- `health_check.interval`: 5-3600 秒
- `resources.cpu`: 0.1-CPU核心数
- `resources.memory_str`: 合法格式（如 "1G", "512M"）
- `depends_on`: 可选，引用的服务必须存在

**Response** (201 Created):
```json
{
  "data": {
    "name": "api-server",
    "command": "mise run api",
    "working_dir": "/home/user/myproject",
    "env": {
      "PORT": "3000",
      "DB_URL": "postgresql://localhost/mydb"
    },
    "ports": [
      {
        "host": 3000,
        "container": 3000,
        "protocol": "tcp"
      }
    ],
    "health_check": {
      "type": "http",
      "url": "http://localhost:3000/health",
      "expected_status": 200,
      "timeout": 5,
      "interval": 10
    },
    "resources": {
      "cpu": 1.0,
      "memory": 1073741824,
      "memory_str": "1G"
    },
    "restart_policy": "on-failure",
    "autostart": true,
    "depends_on": ["database"],
    "runtime": {
      "state": "stopped",
      "restart_count": 0
    }
  }
}
```

**Error Responses**:
- `400 Bad Request` - 请求数据格式错误
- `409 Conflict` - 服务名称已存在
```json
{
  "error": {
    "code": "CONFLICT",
    "message": "Service 'api-server' already exists",
    "details": {
      "service": "api-server"
    },
    "request_id": "req_abc123"
  }
}
```
- `422 Unprocessable Entity` - 验证失败
```json
{
  "error": {
    "code": "VALIDATION_FAILED",
    "message": "Validation failed for service definition",
    "details": {
      "fields": {
        "name": "Must be 1-64 characters and contain only alphanumeric, hyphen, underscore",
        "depends_on": "Service 'database' does not exist"
      }
    },
    "request_id": "req_abc123"
  }
}
```

**Side Effects**:
1. 在 `.config/mise/svcmgr/config.toml` 中创建 `[services.api-server]` 段
2. 触发 Git 自动提交（如果 Git 功能启用）
3. 如果 `autostart=true` 且进程管理器正在运行，立即启动服务

---

### 4. Update Service

**完整更新服务配置**（幂等操作）

```http
PUT /api/v1/services/{name}
Content-Type: application/json
```

**Path Parameters**:
- `name` (string, required) - 服务名称

**Request Body**:
```json
{
  "command": "mise run api:prod",
  "working_dir": "/home/user/myproject",
  "env": {
    "PORT": "3000",
    "DB_URL": "postgresql://localhost/mydb",
    "NODE_ENV": "production"
  },
  "ports": [
    {
      "host": 3000,
      "container": 3000,
      "protocol": "tcp"
    }
  ],
  "health_check": {
    "type": "http",
    "url": "http://localhost:3000/health",
    "expected_status": 200,
    "timeout": 10,
    "interval": 15
  },
  "resources": {
    "cpu": 2.0,
    "memory_str": "2G"
  },
  "restart_policy": "always",
  "autostart": true,
  "depends_on": ["database", "redis"]
}
```

**Validation Rules**: 与 `POST /api/v1/services` 相同（除了 `name` 不可修改）

**Response** (200 OK):
```json
{
  "data": {
    "name": "api-server",
    "command": "mise run api:prod",
    "working_dir": "/home/user/myproject",
    "env": {
      "PORT": "3000",
      "DB_URL": "postgresql://localhost/mydb",
      "NODE_ENV": "production"
    },
    "ports": [
      {
        "host": 3000,
        "container": 3000,
        "protocol": "tcp"
      }
    ],
    "health_check": {
      "type": "http",
      "url": "http://localhost:3000/health",
      "expected_status": 200,
      "timeout": 10,
      "interval": 15
    },
    "resources": {
      "cpu": 2.0,
      "memory": 2147483648,
      "memory_str": "2G"
    },
    "restart_policy": "always",
    "autostart": true,
    "depends_on": ["database", "redis"],
    "runtime": {
      "state": "running",
      "pid": 12345,
      "uptime": 3600,
      "started_at": 1709654400,
      "restart_count": 0
    }
  }
}
```

**Error Responses**:
- `404 Not Found` - 服务不存在
- `422 Unprocessable Entity` - 验证失败

**Side Effects**:
1. 更新 `.config/mise/svcmgr/config.toml` 中的 `[services.{name}]` 段
2. 触发 Git 自动提交
3. **如果服务正在运行**：
   - 配置变更不会立即生效（需要手动重启）
   - 响应头包含 `X-Service-Restart-Required: true`
   - 响应 `data` 中包含 `restart_required: true` 字段

---

### 5. Partial Update Service

**部分更新服务配置**（仅更新指定字段）

```http
PATCH /api/v1/services/{name}
Content-Type: application/json
```

**Path Parameters**:
- `name` (string, required) - 服务名称

**Request Body** (仅包含要更新的字段):
```json
{
  "env": {
    "NODE_ENV": "production",
    "LOG_LEVEL": "info"
  },
  "restart_policy": "always"
}
```

**Response** (200 OK): 与 `PUT` 相同（返回完整服务定义）

**Error Responses**: 与 `PUT` 相同

**Side Effects**: 与 `PUT` 相同

---

### 6. Delete Service

**删除服务**

```http
DELETE /api/v1/services/{name}
```

**Path Parameters**:
- `name` (string, required) - 服务名称

**Query Parameters**:
```
?force=true   # 强制删除（即使服务正在运行）
```

**Response** (200 OK):
```json
{
  "message": "Service 'api-server' deleted successfully"
}
```

**Error Responses**:
- `404 Not Found` - 服务不存在
- `409 Conflict` - 服务正在运行且未指定 `force=true`
```json
{
  "error": {
    "code": "CONFLICT",
    "message": "Cannot delete running service 'api-server' without force flag",
    "details": {
      "service": "api-server",
      "state": "running",
      "hint": "Use ?force=true to forcefully stop and delete, or stop the service first"
    },
    "request_id": "req_abc123"
  }
}
```

**Side Effects**:
1. 如果服务正在运行且 `force=true`，先执行 `POST /api/v1/services/{name}/stop`
2. 从 `.config/mise/svcmgr/config.toml` 中删除 `[services.{name}]` 段
3. 触发 Git 自动提交
4. 如果服务配置了端口映射且内置代理启用，删除相关路由规则

---

## Service Lifecycle Operations

### 7. Start Service

**启动服务**

```http
POST /api/v1/services/{name}/start
```

**Path Parameters**:
- `name` (string, required) - 服务名称

**Response** (200 OK):
```json
{
  "message": "Service 'api-server' started successfully",
  "data": {
    "name": "api-server",
    "runtime": {
      "state": "running",
      "pid": 54321,
      "started_at": 1709661600,
      "restart_count": 0
    }
  }
}
```

**Error Responses**:
- `404 Not Found` - 服务不存在
- `409 Conflict` - 服务已经在运行
```json
{
  "error": {
    "code": "CONFLICT",
    "message": "Service 'api-server' is already running",
    "details": {
      "service": "api-server",
      "state": "running",
      "pid": 12345
    },
    "request_id": "req_abc123"
  }
}
```
- `500 Internal Server Error` - 启动失败
```json
{
  "error": {
    "code": "INTERNAL_ERROR",
    "message": "Failed to start service 'api-server'",
    "details": {
      "service": "api-server",
      "error": "Command not found: mise run api"
    },
    "request_id": "req_abc123"
  }
}
```

**State Transition**:
```
stopped → starting → running
stopped → starting → failed (启动失败)
```

**Side Effects**:
1. 如果服务配置了 `depends_on`，先启动依赖服务（递归）
2. 通过 Process Manager 启动进程
3. 如果配置了健康检查，启动健康检查定时器
4. 如果配置了端口映射且内置代理启用，注册路由规则

---

### 8. Stop Service

**停止服务**

```http
POST /api/v1/services/{name}/stop
```

**Path Parameters**:
- `name` (string, required) - 服务名称

**Query Parameters**:
```
?timeout=30   # 优雅停止超时时间（秒，默认 30）
?signal=TERM  # 停止信号（TERM, INT, KILL，默认 TERM）
```

**Response** (200 OK):
```json
{
  "message": "Service 'api-server' stopped successfully",
  "data": {
    "name": "api-server",
    "runtime": {
      "state": "stopped",
      "stopped_at": 1709661600,
      "exit_code": 0,
      "restart_count": 0
    }
  }
}
```

**Error Responses**:
- `404 Not Found` - 服务不存在
- `409 Conflict` - 服务已经停止
- `504 Gateway Timeout` - 停止超时（进程未响应 signal）
```json
{
  "error": {
    "code": "TIMEOUT",
    "message": "Service 'api-server' did not stop within 30 seconds",
    "details": {
      "service": "api-server",
      "timeout": 30,
      "signal": "TERM",
      "hint": "Try using signal=KILL for forceful termination"
    },
    "request_id": "req_abc123"
  }
}
```

**State Transition**:
```
running → stopping → stopped (优雅停止)
running → stopping → stopped (强制 KILL)
```

**Side Effects**:
1. 向进程发送指定信号
2. 等待进程退出（最多 `timeout` 秒）
3. 如果超时，发送 SIGKILL 强制终止
4. 停止健康检查定时器
5. 如果配置了端口映射且内置代理启用，注销路由规则

---

### 9. Restart Service

**重启服务**（先停止后启动）

```http
POST /api/v1/services/{name}/restart
```

**Path Parameters**:
- `name` (string, required) - 服务名称

**Query Parameters**:
```
?timeout=30   # 停止超时时间（秒，默认 30）
```

**Response** (200 OK):
```json
{
  "message": "Service 'api-server' restarted successfully",
  "data": {
    "name": "api-server",
    "runtime": {
      "state": "running",
      "pid": 98765,
      "started_at": 1709661700,
      "restart_count": 1
    }
  }
}
```

**Error Responses**:
- `404 Not Found` - 服务不存在
- `500 Internal Server Error` - 重启失败（停止成功但启动失败）

**State Transition**:
```
running → stopping → stopped → starting → running
stopped → starting → running (服务已停止时等效于 start)
```

**Idempotency**: 如果服务已停止，等效于 `start` 操作（不返回错误）

---

## Service Logs

### 10. Get Service Logs

**获取服务日志**

```http
GET /api/v1/services/{name}/logs
```

**Path Parameters**:
- `name` (string, required) - 服务名称

**Query Parameters**:
```
?follow=true          # 流式输出（Server-Sent Events）
?tail=100             # 仅返回最后 N 行（默认全部）
?since=1709654400     # 仅返回指定时间戳之后的日志
?until=1709661600     # 仅返回指定时间戳之前的日志
?timestamps=true      # 在每行前添加时间戳
?filter=ERROR         # 仅返回包含指定关键词的行
```

**Response** (200 OK, `follow=false`):
```json
{
  "data": {
    "service": "api-server",
    "logs": [
      {
        "timestamp": 1709654400,
        "stream": "stdout",
        "line": "Server listening on port 3000"
      },
      {
        "timestamp": 1709654410,
        "stream": "stdout",
        "line": "Database connected"
      },
      {
        "timestamp": 1709654420,
        "stream": "stderr",
        "line": "WARN: Cache miss for key 'user:123'"
      }
    ],
    "total_lines": 3
  }
}
```

**Response** (200 OK, `follow=true`, Server-Sent Events):
```http
Content-Type: text/event-stream
Cache-Control: no-cache
Connection: keep-alive

event: log
data: {"timestamp":1709654400,"stream":"stdout","line":"Server listening on port 3000"}

event: log
data: {"timestamp":1709654410,"stream":"stdout","line":"Database connected"}

event: log
data: {"timestamp":1709654420,"stream":"stderr","line":"WARN: Cache miss for key 'user:123'"}
```

**Error Responses**:
- `404 Not Found` - 服务不存在
- `409 Conflict` - 服务从未启动过（没有日志）
```json
{
  "error": {
    "code": "CONFLICT",
    "message": "Service 'api-server' has no logs yet",
    "details": {
      "service": "api-server",
      "hint": "Start the service to generate logs"
    },
    "request_id": "req_abc123"
  }
}
```

**Implementation Notes**:
- 日志存储在 `.local/share/svcmgr/logs/{service_name}.log`
- 日志文件按日轮转（最多保留 7 天）
- 单个日志文件最大 100MB
- `follow=true` 使用 Server-Sent Events 实现实时推送

---

## Service Dependencies

### 11. Get Service Dependency Graph

**获取服务依赖关系图**

```http
GET /api/v1/services/{name}/dependencies
```

**Path Parameters**:
- `name` (string, required) - 服务名称

**Response** (200 OK):
```json
{
  "data": {
    "service": "web-app",
    "dependencies": {
      "direct": [
        {
          "name": "api-server",
          "state": "running"
        },
        {
          "name": "redis",
          "state": "running"
        }
      ],
      "transitive": [
        {
          "name": "database",
          "state": "running",
          "depth": 2,
          "path": ["web-app", "api-server", "database"]
        }
      ]
    },
    "dependents": [
      {
        "name": "proxy",
        "state": "running"
      }
    ]
  }
}
```

**Error Responses**:
- `404 Not Found` - 服务不存在

**Use Cases**:
- 可视化服务依赖关系图
- 决定启动/停止顺序
- 诊断依赖问题

---

## Batch Operations

### 12. Start Multiple Services

**批量启动服务**

```http
POST /api/v1/services/batch/start
Content-Type: application/json
```

**Request Body**:
```json
{
  "services": ["api-server", "worker", "scheduler"],
  "respect_dependencies": true,
  "parallel": false
}
```

**Response** (200 OK):
```json
{
  "data": {
    "results": [
      {
        "service": "api-server",
        "status": "success",
        "pid": 12345
      },
      {
        "service": "worker",
        "status": "success",
        "pid": 12346
      },
      {
        "service": "scheduler",
        "status": "failed",
        "error": "Command not found: mise run schedule"
      }
    ],
    "summary": {
      "total": 3,
      "success": 2,
      "failed": 1
    }
  }
}
```

**Error Responses**:
- `400 Bad Request` - 请求格式错误
- `404 Not Found` - 部分服务不存在（返回 `details.missing_services`）

---

### 13. Stop Multiple Services

**批量停止服务**

```http
POST /api/v1/services/batch/stop
Content-Type: application/json
```

**Request Body**:
```json
{
  "services": ["api-server", "worker", "scheduler"],
  "timeout": 30
}
```

**Response**: 与 `batch/start` 格式相同

---

## Implementation Example

### Handler Implementation

```rust
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use serde_json::json;
use std::sync::Arc;

/// 服务管理 API 状态
pub struct ServiceApiState {
    /// Process Manager 引用
    process_manager: Arc<ProcessManager>,
    
    /// Scheduler Engine 引用
    scheduler: Arc<SchedulerEngine>,
    
    /// Config Manager 引用
    config: Arc<ConfigManager>,
}

/// GET /api/v1/services
pub async fn list_services(
    State(state): State<Arc<ServiceApiState>>,
    Query(params): Query<ListServicesQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // 1. 从 config 读取所有服务定义
    let definitions = state.config.list_services().await?;
    
    // 2. 从 process_manager 读取运行时状态
    let mut services = Vec::new();
    for def in definitions {
        // 应用状态过滤
        if let Some(ref status_filter) = params.status {
            let runtime = state.process_manager.get_service_runtime(&def.name).await?;
            if !status_filter.contains(&runtime.state) {
                continue;
            }
        }
        
        // 应用 autostart 过滤
        if let Some(autostart) = params.autostart {
            if def.autostart != autostart {
                continue;
            }
        }
        
        services.push(ServiceStatus {
            definition: def,
            runtime: state.process_manager.get_service_runtime(&def.name).await?,
        });
    }
    
    // 3. 排序
    if let Some(ref sort_field) = params.sort {
        services.sort_by(|a, b| match sort_field.as_str() {
            "name" => a.definition.name.cmp(&b.definition.name),
            "uptime" => {
                let a_uptime = a.runtime.uptime.unwrap_or(0);
                let b_uptime = b.runtime.uptime.unwrap_or(0);
                b_uptime.cmp(&a_uptime) // 降序
            }
            "restart_count" => b.runtime.restart_count.cmp(&a.runtime.restart_count),
            _ => std::cmp::Ordering::Equal,
        });
    }
    
    // 4. 分页
    let total = services.len();
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(20).min(100); // 最大 100
    let start = ((page - 1) * per_page).min(total);
    let end = (start + per_page).min(total);
    let paginated = &services[start..end];
    
    // 5. 字段选择（如果指定了 fields）
    let data = if let Some(ref fields) = params.fields {
        paginated.iter().map(|s| {
            filter_fields(&serde_json::to_value(s).unwrap(), fields)
        }).collect::<Vec<_>>()
    } else {
        paginated.iter().map(|s| serde_json::to_value(s).unwrap()).collect()
    };
    
    Ok(Json(json!({
        "data": data,
        "pagination": {
            "page": page,
            "per_page": per_page,
            "total": total,
            "total_pages": (total + per_page - 1) / per_page,
            "has_next": end < total,
            "has_prev": page > 1,
        }
    })))
}

#[derive(Deserialize)]
pub struct ListServicesQuery {
    status: Option<Vec<ServiceState>>,
    autostart: Option<bool>,
    page: Option<usize>,
    per_page: Option<usize>,
    sort: Option<String>,
    fields: Option<Vec<String>>,
}

/// POST /api/v1/services/{name}/start
pub async fn start_service(
    State(state): State<Arc<ServiceApiState>>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // 1. 检查服务是否存在
    let definition = state.config.get_service(&name).await?
        .ok_or_else(|| ApiError::not_found(&format!("Service '{}' does not exist", name)))?;
    
    // 2. 检查服务是否已经在运行
    let runtime = state.process_manager.get_service_runtime(&name).await?;
    if runtime.state == ServiceState::Running {
        return Err(ApiError::conflict(&format!(
            "Service '{}' is already running",
            name
        )));
    }
    
    // 3. 启动依赖服务（如果配置了 depends_on）
    if let Some(ref depends_on) = definition.depends_on {
        for dep in depends_on {
            let dep_runtime = state.process_manager.get_service_runtime(dep).await?;
            if dep_runtime.state != ServiceState::Running {
                // 递归启动依赖
                start_service_internal(&state, dep).await?;
            }
        }
    }
    
    // 4. 通过 Process Manager 启动服务
    state.process_manager.start_service(&definition).await?;
    
    // 5. 如果配置了健康检查，启动健康检查定时器
    if let Some(ref health_check) = definition.health_check {
        state.scheduler.schedule_health_check(&name, health_check).await?;
    }
    
    // 6. 如果配置了端口映射且内置代理启用，注册路由
    if let Some(ref ports) = definition.ports {
        if state.config.is_proxy_enabled().await? {
            for port in ports {
                state.proxy.register_route(&name, port).await?;
            }
        }
    }
    
    // 7. 返回启动后的状态
    let updated_runtime = state.process_manager.get_service_runtime(&name).await?;
    
    Ok(Json(json!({
        "message": format!("Service '{}' started successfully", name),
        "data": {
            "name": name,
            "runtime": updated_runtime,
        }
    })))
}

/// 字段选择辅助函数
fn filter_fields(value: &serde_json::Value, fields: &[String]) -> serde_json::Value {
    if let Some(obj) = value.as_object() {
        let mut filtered = serde_json::Map::new();
        for field in fields {
            if let Some(v) = obj.get(field) {
                filtered.insert(field.clone(), v.clone());
            }
        }
        serde_json::Value::Object(filtered)
    } else {
        value.clone()
    }
}
```

### Error Handling

```rust
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};

/// API 错误类型
pub struct ApiError {
    code: ApiErrorCode,
    message: String,
    details: Option<serde_json::Value>,
    request_id: String,
}

impl ApiError {
    pub fn not_found(message: &str) -> Self {
        Self {
            code: ApiErrorCode::ResourceNotFound,
            message: message.to_string(),
            details: None,
            request_id: generate_request_id(),
        }
    }
    
    pub fn conflict(message: &str) -> Self {
        Self {
            code: ApiErrorCode::Conflict,
            message: message.to_string(),
            details: None,
            request_id: generate_request_id(),
        }
    }
    
    pub fn validation_failed(fields: HashMap<String, String>) -> Self {
        Self {
            code: ApiErrorCode::ValidationFailed,
            message: "Validation failed for service definition".to_string(),
            details: Some(json!({ "fields": fields })),
            request_id: generate_request_id(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.code.status_code();
        let body = Json(json!({
            "error": {
                "code": self.code,
                "message": self.message,
                "details": self.details,
                "request_id": self.request_id,
            }
        }));
        
        (status, body).into_response()
    }
}
```

## Testing Strategy

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum_test::TestServer;
    
    #[tokio::test]
    async fn test_create_service_success() {
        let app = build_api_router();
        let server = TestServer::new(app).unwrap();
        
        let response = server
            .post("/api/v1/services")
            .json(&json!({
                "name": "test-service",
                "command": "echo hello",
                "restart_policy": "on-failure",
                "autostart": false,
            }))
            .await;
        
        assert_eq!(response.status_code(), StatusCode::CREATED);
        
        let body: serde_json::Value = response.json();
        assert_eq!(body["data"]["name"], "test-service");
        assert_eq!(body["data"]["runtime"]["state"], "stopped");
    }
    
    #[tokio::test]
    async fn test_start_service_success() {
        let app = build_api_router();
        let server = TestServer::new(app).unwrap();
        
        // 1. 创建服务
        server
            .post("/api/v1/services")
            .json(&json!({
                "name": "test-service",
                "command": "sleep 60",
            }))
            .await;
        
        // 2. 启动服务
        let response = server
            .post("/api/v1/services/test-service/start")
            .await;
        
        assert_eq!(response.status_code(), StatusCode::OK);
        
        let body: serde_json::Value = response.json();
        assert_eq!(body["data"]["runtime"]["state"], "running");
        assert!(body["data"]["runtime"]["pid"].is_number());
        
        // 3. 验证幂等性（重复启动应返回 409 Conflict）
        let response2 = server
            .post("/api/v1/services/test-service/start")
            .await;
        
        assert_eq!(response2.status_code(), StatusCode::CONFLICT);
        
        // 4. 清理
        server
            .post("/api/v1/services/test-service/stop")
            .await;
        server
            .delete("/api/v1/services/test-service")
            .query("force", "true")
            .await;
    }
    
    #[tokio::test]
    async fn test_service_dependencies() {
        let app = build_api_router();
        let server = TestServer::new(app).unwrap();
        
        // 1. 创建数据库服务
        server
            .post("/api/v1/services")
            .json(&json!({
                "name": "database",
                "command": "sleep 120",
            }))
            .await;
        
        // 2. 创建依赖数据库的 API 服务
        server
            .post("/api/v1/services")
            .json(&json!({
                "name": "api-server",
                "command": "sleep 120",
                "depends_on": ["database"],
            }))
            .await;
        
        // 3. 启动 API 服务（应自动启动 database）
        let response = server
            .post("/api/v1/services/api-server/start")
            .await;
        
        assert_eq!(response.status_code(), StatusCode::OK);
        
        // 4. 验证 database 也被启动了
        let db_response = server
            .get("/api/v1/services/database")
            .await;
        
        let db_body: serde_json::Value = db_response.json();
        assert_eq!(db_body["data"]["runtime"]["state"], "running");
        
        // 5. 清理
        server.post("/api/v1/services/api-server/stop").await;
        server.post("/api/v1/services/database/stop").await;
        server.delete("/api/v1/services/api-server").query("force", "true").await;
        server.delete("/api/v1/services/database").query("force", "true").await;
    }
}
```

## Related Specifications

- **10-api-overview.md** - API 设计总览（认证、版本、响应格式）
- **02-scheduler-engine.md** - 统一调度引擎（服务使用 OneShot 触发器）
- **03-process-manager.md** - 进程管理器（服务生命周期控制）
- **04-git-versioning.md** - Git 配置版本管理（服务配置变更的提交）
- **05-web-service.md** - 内置 HTTP 代理（端口映射和路由注册）
- **06-feature-flags.md** - 功能开关（cgroups 资源限制可选）
- **12-api-tasks.md** - 任务管理 API（任务与服务的区别）

## Future Enhancements

1. **服务模板**：预定义服务模板（如 `web-server`, `background-worker`）
2. **服务组**：批量管理一组相关服务（如 `production`, `development`）
3. **服务指标**：导出 Prometheus 指标（CPU、内存、请求数等）
4. **服务网关**：自动配置服务间通信的网关规则
5. **服务发现**：自动注册到 Consul/etcd 等服务发现系统
6. **蓝绿部署**：支持服务的蓝绿部署和金丝雀发布
