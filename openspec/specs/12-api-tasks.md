# 12 - 任务管理 API

> 版本：2.0.0-draft  
> 状态：设计中

## 设计目标

任务管理 API 提供以下能力：

1. **即时任务执行**：通过 `mise run` 触发一次性任务（OneShot 触发器）
2. **定时任务管理**：创建/更新/删除 cron 表达式驱动的周期性任务（Cron 触发器）
3. **执行历史查询**：查看任务执行记录（开始时间、结束时间、退出码、日志）
4. **运行时控制**：取消正在运行的任务、启用/禁用定时任务

## 为什么需要任务管理 API？

### 与服务的区别

| 特性 | 任务（Task） | 服务（Service） |
|------|-------------|----------------|
| **生命周期** | 一次性执行或定时执行 | 长期运行（守护进程） |
| **触发器** | OneShot（立即）/ Cron（定时）/ Event（事件） | OneShot（自启动） |
| **重启策略** | 不自动重启 | 支持 always/on-failure 重启 |
| **资源限制** | 可选（cgroups v2） | 可选（cgroups v2） |
| **日志管理** | 执行历史归档 | 实时流式日志 |
| **依赖关系** | 可依赖其他任务完成 | 可依赖其他服务启动 |

**示例场景**：

- **任务**：数据库备份（每天凌晨 3 点）、日志清理（每周日）、一次性部署脚本
- **服务**：Web 服务器（nginx）、API 后端（Node.js）、数据库（PostgreSQL）

### 任务的两种形式

1. **mise 任务**（推荐）：
   ```toml
   # .config/mise/config.toml
   [tasks.backup]
   run = "pg_dump mydb > backup.sql"
   env = { PGPASSWORD = "secret" }
   dir = "/data"
   ```
   - 任务定义在 mise 配置中，可通过 `mise run backup` 独立执行
   - svcmgr 调用时无需重复定义命令，只需引用任务名

2. **直接命令任务**：
   ```toml
   # .config/mise/svcmgr/config.toml
   [scheduled_tasks.cleanup]
   command = "find /tmp -mtime +7 -delete"
   schedule = "0 2 * * 0"  # 每周日凌晨 2 点
   ```
   - 不依赖 mise 任务定义，直接指定 shell 命令
   - 适用于简单脚本或不需要环境变量管理的场景

## API 端点概览

| HTTP 方法 | 路径 | 用途 |
|-----------|------|------|
| **即时任务** |
| GET | `/api/v1/tasks` | 列出所有 mise 任务（可执行） |
| GET | `/api/v1/tasks/{name}` | 获取任务详情（配置 + 最近执行） |
| POST | `/api/v1/tasks/{name}/run` | 立即执行任务（OneShot） |
| POST | `/api/v1/tasks/{name}/cancel` | 取消正在运行的任务 |
| GET | `/api/v1/tasks/{name}/history` | 查询任务执行历史 |
| **定时任务** |
| GET | `/api/v1/scheduled-tasks` | 列出所有定时任务 |
| GET | `/api/v1/scheduled-tasks/{name}` | 获取定时任务详情 |
| POST | `/api/v1/scheduled-tasks` | 创建定时任务 |
| PUT | `/api/v1/scheduled-tasks/{name}` | 更新定时任务（schedule/enabled） |
| DELETE | `/api/v1/scheduled-tasks/{name}` | 删除定时任务 |
| POST | `/api/v1/scheduled-tasks/{name}/enable` | 启用定时任务 |
| POST | `/api/v1/scheduled-tasks/{name}/disable` | 禁用定时任务 |
| POST | `/api/v1/scheduled-tasks/{name}/run` | 立即执行（不改变定时计划） |

---

## 数据模型

### TaskDefinition（mise 任务定义）

```rust
/// mise 任务定义（从 .config/mise/config.toml 解析）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDefinition {
    /// 任务名称（唯一标识）
    pub name: String,
    
    /// 运行命令（run 字段）
    pub run: String,
    
    /// 任务描述（可选）
    pub description: Option<String>,
    
    /// 环境变量
    #[serde(default)]
    pub env: HashMap<String, String>,
    
    /// 工作目录
    pub dir: Option<PathBuf>,
    
    /// 依赖任务（在此任务运行前先运行）
    #[serde(default)]
    pub depends: Vec<String>,
    
    /// 别名（可通过 alias 名称调用任务）
    #[serde(default)]
    pub alias: Vec<String>,
    
    /// 任务来源（mise 文件路径）
    pub source: PathBuf,
}
```

### ScheduledTask（定时任务配置）

```rust
/// 定时任务配置（存储在 .config/mise/svcmgr/config.toml）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTask {
    /// 任务名称（唯一标识）
    pub name: String,
    
    /// 执行方式
    #[serde(flatten)]
    pub execution: TaskExecution,
    
    /// cron 表达式
    pub schedule: String,
    
    /// 是否启用
    #[serde(default = "default_true")]
    pub enabled: bool,
    
    /// 任务描述
    pub description: Option<String>,
    
    /// 超时时间（秒，0 = 无超时）
    #[serde(default)]
    pub timeout: u64,
    
    /// 资源限制（可选）
    pub limits: Option<ResourceLimits>,
    
    /// 下次运行时间（运行时计算，不存储）
    #[serde(skip)]
    pub next_run: Option<DateTime<Utc>>,
}

/// 任务执行方式
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TaskExecution {
    /// 执行 mise 任务
    #[serde(rename = "mise_task")]
    MiseTask {
        /// mise 任务名称
        task: String,
        /// 参数
        #[serde(default)]
        args: Vec<String>,
    },
    
    /// 直接执行命令
    #[serde(rename = "command")]
    Command {
        /// Shell 命令
        command: String,
        /// 环境变量
        #[serde(default)]
        env: HashMap<String, String>,
        /// 工作目录
        dir: Option<PathBuf>,
    },
}

fn default_true() -> bool { true }
```

### TaskExecution（执行记录）

```rust
/// 任务执行记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskExecutionRecord {
    /// 执行 ID（UUID）
    pub execution_id: String,
    
    /// 任务名称
    pub task_name: String,
    
    /// 开始时间
    pub started_at: DateTime<Utc>,
    
    /// 结束时间（None = 仍在运行）
    pub finished_at: Option<DateTime<Utc>>,
    
    /// 退出码（None = 仍在运行或被取消）
    pub exit_code: Option<i32>,
    
    /// 执行状态
    pub status: ExecutionStatus,
    
    /// 触发方式
    pub trigger: TriggerType,
    
    /// 进程 PID（运行时）
    pub pid: Option<u32>,
    
    /// 标准输出（前 10KB，完整日志见 log_file）
    pub stdout_preview: String,
    
    /// 标准错误（前 10KB，完整日志见 log_file）
    pub stderr_preview: String,
    
    /// 完整日志文件路径
    pub log_file: PathBuf,
}

/// 执行状态
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionStatus {
    /// 运行中
    Running,
    /// 成功（退出码 0）
    Success,
    /// 失败（退出码非 0）
    Failed,
    /// 被取消
    Cancelled,
    /// 超时
    Timeout,
}

/// 触发类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TriggerType {
    /// 手动触发（API 调用）
    Manual,
    /// 定时触发（cron）
    Scheduled,
    /// 事件触发
    Event,
}
```

### ResourceLimits（资源限制）

```rust
/// 资源限制（cgroups v2）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// 内存限制（字节）
    pub memory: Option<u64>,
    
    /// CPU 配额（微秒/100ms，例如 50000 = 50% CPU）
    pub cpu_quota: Option<u64>,
    
    /// CPU 权重（1-10000，默认 100）
    pub cpu_weight: Option<u64>,
}
```

---

## API 详细设计

### 1. 列出所有 mise 任务

**请求**:
```http
GET /api/v1/tasks?source=all&status=all HTTP/1.1
```

**查询参数**:
- `source` (可选): 任务来源过滤
  - `all` (默认): 所有任务
  - `global`: 全局 mise 配置
  - `local`: 当前工作目录配置
- `status` (可选): 执行状态过滤
  - `all` (默认): 所有任务
  - `running`: 正在运行
  - `idle`: 未运行

**响应**:
```json
{
  "data": {
    "tasks": [
      {
        "name": "backup",
        "description": "Database backup",
        "run": "pg_dump mydb > backup.sql",
        "env": {
          "PGPASSWORD": "***"
        },
        "dir": "/data",
        "depends": [],
        "alias": ["db:backup"],
        "source": "/home/user/.config/mise/config.toml",
        "current_execution": null
      },
      {
        "name": "deploy",
        "description": "Deploy to production",
        "run": "./scripts/deploy.sh",
        "env": {},
        "dir": "/home/user/project",
        "depends": ["test"],
        "alias": [],
        "source": "/home/user/project/.mise.toml",
        "current_execution": {
          "execution_id": "exec_abc123",
          "started_at": "2026-02-23T03:00:00Z",
          "status": "running",
          "pid": 12345
        }
      }
    ]
  },
  "pagination": {
    "total": 2,
    "page": 1,
    "per_page": 20
  }
}
```

**说明**:
- `current_execution`: 如果任务正在运行，包含简要执行信息
- `env` 中敏感值（包含 password/secret/token）自动脱敏为 `***`

---

### 2. 获取任务详情

**请求**:
```http
GET /api/v1/tasks/backup HTTP/1.1
```

**响应**:
```json
{
  "data": {
    "name": "backup",
    "description": "Database backup",
    "run": "pg_dump mydb > backup.sql",
    "env": {
      "PGPASSWORD": "***"
    },
    "dir": "/data",
    "depends": [],
    "alias": ["db:backup"],
    "source": "/home/user/.config/mise/config.toml",
    "current_execution": null,
    "last_execution": {
      "execution_id": "exec_xyz789",
      "started_at": "2026-02-22T03:00:00Z",
      "finished_at": "2026-02-22T03:05:23Z",
      "exit_code": 0,
      "status": "success",
      "trigger": "scheduled",
      "stdout_preview": "Backup completed: 1.2GB\n",
      "stderr_preview": "",
      "log_file": "/home/user/.local/share/svcmgr/logs/backup/exec_xyz789.log"
    },
    "schedules": [
      {
        "name": "nightly_backup",
        "schedule": "0 3 * * *",
        "enabled": true,
        "next_run": "2026-02-24T03:00:00Z"
      }
    ]
  }
}
```

**说明**:
- `last_execution`: 最近一次执行记录（无论成功失败）
- `schedules`: 引用此任务的所有定时任务

**错误响应**:
```json
{
  "error": {
    "code": "TASK_NOT_FOUND",
    "message": "Task 'backup' not found in mise configuration",
    "details": {
      "task_name": "backup",
      "searched_paths": [
        "/home/user/.config/mise/config.toml",
        "/home/user/project/.mise.toml"
      ]
    },
    "request_id": "req_abc123"
  }
}
```

---

### 3. 立即执行任务

**请求**:
```http
POST /api/v1/tasks/backup/run HTTP/1.1
Content-Type: application/json

{
  "args": ["--full"],
  "env": {
    "BACKUP_TYPE": "full"
  },
  "timeout": 3600,
  "wait": false
}
```

**请求体**:
- `args` (可选): 传递给任务的参数（追加到 mise run 后）
- `env` (可选): 额外环境变量（合并到任务 env）
- `timeout` (可选): 超时时间（秒，0 = 无超时）
- `wait` (可选): 是否等待任务完成（默认 false）
  - `true`: 阻塞直到任务完成，返回最终状态
  - `false`: 立即返回 execution_id，客户端轮询状态

**响应（wait=false，立即返回）**:
```json
{
  "data": {
    "execution_id": "exec_def456",
    "task_name": "backup",
    "started_at": "2026-02-23T10:30:00Z",
    "status": "running",
    "pid": 23456,
    "log_file": "/home/user/.local/share/svcmgr/logs/backup/exec_def456.log"
  }
}
```

**响应（wait=true，任务完成后返回）**:
```json
{
  "data": {
    "execution_id": "exec_def456",
    "task_name": "backup",
    "started_at": "2026-02-23T10:30:00Z",
    "finished_at": "2026-02-23T10:35:12Z",
    "exit_code": 0,
    "status": "success",
    "trigger": "manual",
    "stdout_preview": "Backup completed successfully\n",
    "stderr_preview": "",
    "log_file": "/home/user/.local/share/svcmgr/logs/backup/exec_def456.log"
  }
}
```

**错误响应**:
```json
{
  "error": {
    "code": "TASK_ALREADY_RUNNING",
    "message": "Task 'backup' is already running",
    "details": {
      "task_name": "backup",
      "execution_id": "exec_abc123",
      "started_at": "2026-02-23T10:29:00Z",
      "pid": 23400
    },
    "request_id": "req_abc124"
  }
}
```

**副作用**:
1. 在调度引擎中创建 OneShot 触发的任务实例
2. 生成唯一 execution_id（UUID v4）
3. 创建日志文件 `~/.local/share/svcmgr/logs/{task_name}/{execution_id}.log`
4. 执行完成后，记录存入历史（内存 + 可选持久化）

---

### 4. 取消正在运行的任务

**请求**:
```http
POST /api/v1/tasks/backup/cancel HTTP/1.1
Content-Type: application/json

{
  "signal": "SIGTERM",
  "timeout": 10
}
```

**请求体**:
- `signal` (可选): 发送信号（默认 `SIGTERM`）
  - `SIGTERM`: 优雅终止
  - `SIGKILL`: 强制终止
- `timeout` (可选): 等待进程退出的超时（秒，0 = 立即 SIGKILL）

**响应**:
```json
{
  "data": {
    "execution_id": "exec_def456",
    "task_name": "backup",
    "status": "cancelled",
    "finished_at": "2026-02-23T10:31:00Z",
    "signal_sent": "SIGTERM"
  }
}
```

**错误响应**:
```json
{
  "error": {
    "code": "TASK_NOT_RUNNING",
    "message": "Task 'backup' is not currently running",
    "details": {
      "task_name": "backup"
    },
    "request_id": "req_abc125"
  }
}
```

**副作用**:
1. 向任务进程发送信号（SIGTERM 或 SIGKILL）
2. 等待进程退出（最多 `timeout` 秒）
3. 更新执行记录状态为 `cancelled`
4. 如果超时未退出，发送 SIGKILL 强制终止

---

### 5. 查询任务执行历史

**请求**:
```http
GET /api/v1/tasks/backup/history?limit=10&status=all&trigger=all HTTP/1.1
```

**查询参数**:
- `limit` (可选): 返回记录数（默认 20，最大 100）
- `offset` (可选): 分页偏移（默认 0）
- `status` (可选): 状态过滤
  - `all` (默认): 所有状态
  - `success`: 成功
  - `failed`: 失败
  - `cancelled`: 被取消
- `trigger` (可选): 触发方式过滤
  - `all` (默认): 所有触发方式
  - `manual`: 手动触发
  - `scheduled`: 定时触发

**响应**:
```json
{
  "data": {
    "task_name": "backup",
    "executions": [
      {
        "execution_id": "exec_xyz789",
        "started_at": "2026-02-22T03:00:00Z",
        "finished_at": "2026-02-22T03:05:23Z",
        "exit_code": 0,
        "status": "success",
        "trigger": "scheduled",
        "stdout_preview": "Backup completed: 1.2GB\n",
        "stderr_preview": "",
        "log_file": "/home/user/.local/share/svcmgr/logs/backup/exec_xyz789.log"
      },
      {
        "execution_id": "exec_uvw456",
        "started_at": "2026-02-21T03:00:00Z",
        "finished_at": "2026-02-21T03:04:12Z",
        "exit_code": 1,
        "status": "failed",
        "trigger": "scheduled",
        "stdout_preview": "",
        "stderr_preview": "Error: Connection refused\n",
        "log_file": "/home/user/.local/share/svcmgr/logs/backup/exec_uvw456.log"
      }
    ]
  },
  "pagination": {
    "total": 45,
    "offset": 0,
    "limit": 10
  }
}
```

**说明**:
- 历史记录按 `started_at` 降序排列（最新在前）
- `stdout_preview` / `stderr_preview` 截取前 10KB
- 完整日志通过 `log_file` 路径访问（可能需要单独的日志 API）

---

### 6. 列出所有定时任务

**请求**:
```http
GET /api/v1/scheduled-tasks?enabled=all HTTP/1.1
```

**查询参数**:
- `enabled` (可选): 启用状态过滤
  - `all` (默认): 所有任务
  - `true`: 仅启用
  - `false`: 仅禁用

**响应**:
```json
{
  "data": {
    "scheduled_tasks": [
      {
        "name": "nightly_backup",
        "description": "Daily database backup",
        "execution": {
          "type": "mise_task",
          "task": "backup",
          "args": ["--incremental"]
        },
        "schedule": "0 3 * * *",
        "enabled": true,
        "timeout": 3600,
        "limits": {
          "memory": 2147483648,
          "cpu_quota": 50000
        },
        "next_run": "2026-02-24T03:00:00Z",
        "last_execution": {
          "execution_id": "exec_xyz789",
          "started_at": "2026-02-23T03:00:00Z",
          "finished_at": "2026-02-23T03:05:23Z",
          "status": "success"
        }
      },
      {
        "name": "weekly_cleanup",
        "description": "Clean temporary files",
        "execution": {
          "type": "command",
          "command": "find /tmp -mtime +7 -delete",
          "env": {},
          "dir": null
        },
        "schedule": "0 2 * * 0",
        "enabled": true,
        "timeout": 0,
        "limits": null,
        "next_run": "2026-02-29T02:00:00Z",
        "last_execution": null
      }
    ]
  },
  "pagination": {
    "total": 2,
    "page": 1,
    "per_page": 20
  }
}
```

**说明**:
- `next_run`: 下次运行时间（根据 cron 表达式计算）
- `last_execution`: 最近一次执行简要信息（无论成功失败）

---

### 7. 获取定时任务详情

**请求**:
```http
GET /api/v1/scheduled-tasks/nightly_backup HTTP/1.1
```

**响应**:
```json
{
  "data": {
    "name": "nightly_backup",
    "description": "Daily database backup",
    "execution": {
      "type": "mise_task",
      "task": "backup",
      "args": ["--incremental"]
    },
    "schedule": "0 3 * * *",
    "enabled": true,
    "timeout": 3600,
    "limits": {
      "memory": 2147483648,
      "cpu_quota": 50000
    },
    "next_run": "2026-02-24T03:00:00Z",
    "history": [
      {
        "execution_id": "exec_xyz789",
        "started_at": "2026-02-23T03:00:00Z",
        "finished_at": "2026-02-23T03:05:23Z",
        "exit_code": 0,
        "status": "success",
        "trigger": "scheduled"
      },
      {
        "execution_id": "exec_uvw456",
        "started_at": "2026-02-22T03:00:00Z",
        "finished_at": "2026-02-22T03:04:12Z",
        "exit_code": 1,
        "status": "failed",
        "trigger": "scheduled"
      }
    ]
  }
}
```

**说明**:
- `history`: 最近 10 次执行记录（默认），可通过查询参数调整

---

### 8. 创建定时任务

**请求（mise 任务类型）**:
```http
POST /api/v1/scheduled-tasks HTTP/1.1
Content-Type: application/json
Idempotency-Key: idempotency_abc123

{
  "name": "nightly_backup",
  "description": "Daily database backup",
  "execution": {
    "type": "mise_task",
    "task": "backup",
    "args": ["--incremental"]
  },
  "schedule": "0 3 * * *",
  "enabled": true,
  "timeout": 3600,
  "limits": {
    "memory": 2147483648,
    "cpu_quota": 50000
  }
}
```

**请求（直接命令类型）**:
```http
POST /api/v1/scheduled-tasks HTTP/1.1
Content-Type: application/json

{
  "name": "weekly_cleanup",
  "description": "Clean temporary files",
  "execution": {
    "type": "command",
    "command": "find /tmp -mtime +7 -delete"
  },
  "schedule": "0 2 * * 0",
  "enabled": true
}
```

**请求体验证规则**:
- `name`: 必填，字母数字下划线，不能以数字开头，最长 64 字符
- `execution`: 必填，二选一：
  - `mise_task`: `task` 字段必须存在于 mise 配置中
  - `command`: `command` 字段必须非空
- `schedule`: 必填，符合 cron 表达式语法（5 或 6 字段）
- `timeout`: 可选，0-86400（24小时），0 = 无超时
- `limits.memory`: 可选，1MB - 系统内存上限
- `limits.cpu_quota`: 可选，1000-100000（1%-100% CPU）

**响应**:
```json
{
  "data": {
    "name": "nightly_backup",
    "description": "Daily database backup",
    "execution": {
      "type": "mise_task",
      "task": "backup",
      "args": ["--incremental"]
    },
    "schedule": "0 3 * * *",
    "enabled": true,
    "timeout": 3600,
    "limits": {
      "memory": 2147483648,
      "cpu_quota": 50000
    },
    "next_run": "2026-02-24T03:00:00Z"
  }
}
```

**错误响应**:
```json
{
  "error": {
    "code": "SCHEDULED_TASK_ALREADY_EXISTS",
    "message": "Scheduled task 'nightly_backup' already exists",
    "details": {
      "name": "nightly_backup",
      "existing_schedule": "0 3 * * *"
    },
    "request_id": "req_abc126"
  }
}
```

```json
{
  "error": {
    "code": "INVALID_CRON_EXPRESSION",
    "message": "Invalid cron expression: '0 3 * *'",
    "details": {
      "schedule": "0 3 * *",
      "error": "Expected 5 or 6 fields, got 4"
    },
    "request_id": "req_abc127"
  }
}
```

**副作用**:
1. 写入 `.config/mise/svcmgr/config.toml` 的 `[scheduled_tasks.{name}]` 段
2. Git 自动暂存变更（`git add .config/mise/svcmgr/config.toml`）
3. 在调度引擎中注册 Cron 触发器
4. 计算并缓存 `next_run` 时间

---

### 9. 更新定时任务

**请求**:
```http
PUT /api/v1/scheduled-tasks/nightly_backup HTTP/1.1
Content-Type: application/json

{
  "schedule": "0 4 * * *",
  "enabled": true,
  "timeout": 7200
}
```

**请求体**:
- 所有字段可选（未提供的字段保持不变）
- `schedule`: 更新 cron 表达式
- `enabled`: 启用/禁用任务
- `timeout`: 更新超时时间
- `limits`: 更新资源限制

**响应**:
```json
{
  "data": {
    "name": "nightly_backup",
    "description": "Daily database backup",
    "execution": {
      "type": "mise_task",
      "task": "backup",
      "args": ["--incremental"]
    },
    "schedule": "0 4 * * *",
    "enabled": true,
    "timeout": 7200,
    "limits": {
      "memory": 2147483648,
      "cpu_quota": 50000
    },
    "next_run": "2026-02-24T04:00:00Z"
  }
}
```

**副作用**:
1. 更新 `.config/mise/svcmgr/config.toml` 对应段
2. Git 自动暂存变更
3. 重新计算 `next_run` 时间
4. 如果 `schedule` 变更，取消旧触发器并注册新触发器

---

### 10. 删除定时任务

**请求**:
```http
DELETE /api/v1/scheduled-tasks/nightly_backup HTTP/1.1
```

**响应**:
```http
HTTP/1.1 204 No Content
```

**错误响应**:
```json
{
  "error": {
    "code": "SCHEDULED_TASK_NOT_FOUND",
    "message": "Scheduled task 'nightly_backup' not found",
    "details": {
      "name": "nightly_backup"
    },
    "request_id": "req_abc128"
  }
}
```

**副作用**:
1. 删除 `.config/mise/svcmgr/config.toml` 的 `[scheduled_tasks.{name}]` 段
2. Git 自动暂存变更
3. 从调度引擎中注销触发器
4. 如果任务正在运行，发送 SIGTERM 终止

---

### 11. 启用定时任务

**请求**:
```http
POST /api/v1/scheduled-tasks/nightly_backup/enable HTTP/1.1
```

**响应**:
```json
{
  "data": {
    "name": "nightly_backup",
    "enabled": true,
    "next_run": "2026-02-24T03:00:00Z"
  }
}
```

**副作用**:
1. 更新配置 `enabled = true`
2. 重新注册 Cron 触发器
3. 计算 `next_run`

---

### 12. 禁用定时任务

**请求**:
```http
POST /api/v1/scheduled-tasks/nightly_backup/disable HTTP/1.1
```

**响应**:
```json
{
  "data": {
    "name": "nightly_backup",
    "enabled": false,
    "next_run": null
  }
}
```

**副作用**:
1. 更新配置 `enabled = false`
2. 从调度引擎注销触发器
3. 清除 `next_run`

---

### 13. 立即执行定时任务（不改变定时计划）

**请求**:
```http
POST /api/v1/scheduled-tasks/nightly_backup/run HTTP/1.1
Content-Type: application/json

{
  "wait": false
}
```

**响应**:
```json
{
  "data": {
    "execution_id": "exec_ghi789",
    "task_name": "nightly_backup",
    "started_at": "2026-02-23T10:45:00Z",
    "status": "running",
    "pid": 34567,
    "log_file": "/home/user/.local/share/svcmgr/logs/nightly_backup/exec_ghi789.log"
  }
}
```

**说明**:
- 与 `POST /api/v1/tasks/{name}/run` 行为相同，但触发类型标记为 `manual`
- 不影响定时计划的 `next_run` 时间

---

## 批量操作

### 14. 批量启用/禁用定时任务

**请求**:
```http
POST /api/v1/scheduled-tasks/batch HTTP/1.1
Content-Type: application/json

{
  "operation": "enable",
  "names": ["nightly_backup", "weekly_cleanup"]
}
```

**请求体**:
- `operation`: `enable` 或 `disable`
- `names`: 任务名称数组（最多 50 个）

**响应**:
```json
{
  "data": {
    "succeeded": [
      {
        "name": "nightly_backup",
        "enabled": true,
        "next_run": "2026-02-24T03:00:00Z"
      },
      {
        "name": "weekly_cleanup",
        "enabled": true,
        "next_run": "2026-02-29T02:00:00Z"
      }
    ],
    "failed": []
  }
}
```

---

## 错误码清单

| 错误码 | HTTP 状态 | 说明 |
|--------|-----------|------|
| `TASK_NOT_FOUND` | 404 | mise 任务不存在 |
| `TASK_ALREADY_RUNNING` | 409 | 任务已在运行 |
| `TASK_NOT_RUNNING` | 400 | 任务未运行（取消失败） |
| `SCHEDULED_TASK_NOT_FOUND` | 404 | 定时任务不存在 |
| `SCHEDULED_TASK_ALREADY_EXISTS` | 409 | 定时任务名称冲突 |
| `INVALID_CRON_EXPRESSION` | 400 | cron 表达式格式错误 |
| `INVALID_TASK_NAME` | 400 | 任务名称格式非法 |
| `MISE_TASK_NOT_FOUND` | 400 | 引用的 mise 任务不存在 |
| `EXECUTION_NOT_FOUND` | 404 | 执行记录不存在 |
| `RESOURCE_LIMIT_EXCEEDED` | 400 | 资源限制值超出系统上限 |
| `TIMEOUT_OUT_OF_RANGE` | 400 | 超时值超出范围（0-86400） |

---

## Handler 实现示例

### 立即执行任务 Handler

```rust
use axum::{extract::{Path, State}, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::sync::Arc;
use tokio::time::Duration;

#[derive(Debug, Deserialize)]
pub struct RunTaskRequest {
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub timeout: u64,
    #[serde(default)]
    pub wait: bool,
}

#[derive(Debug, Serialize)]
pub struct RunTaskResponse {
    pub execution_id: String,
    pub task_name: String,
    pub started_at: DateTime<Utc>,
    pub status: ExecutionStatus,
    pub pid: Option<u32>,
    pub log_file: PathBuf,
    // wait=true 时包含以下字段
    pub finished_at: Option<DateTime<Utc>>,
    pub exit_code: Option<i32>,
}

/// POST /api/v1/tasks/{name}/run
pub async fn run_task(
    State(app): State<Arc<AppState>>,
    Path(task_name): Path<String>,
    Json(req): Json<RunTaskRequest>,
) -> Result<Json<ApiResponse<RunTaskResponse>>, ApiError> {
    // 1. 从 mise 配置解析任务定义
    let task_def = app.mise_adapter.get_task_definition(&task_name).await
        .map_err(|_| ApiError::not_found("TASK_NOT_FOUND", 
            format!("Task '{}' not found in mise configuration", task_name)))?;
    
    // 2. 检查任务是否已在运行
    if app.scheduler.is_task_running(&task_name).await {
        let current = app.scheduler.get_running_execution(&task_name).await?;
        return Err(ApiError::conflict("TASK_ALREADY_RUNNING",
            format!("Task '{}' is already running", task_name))
            .with_detail("execution_id", current.execution_id)
            .with_detail("pid", current.pid));
    }
    
    // 3. 生成执行 ID 和日志文件路径
    let execution_id = format!("exec_{}", Uuid::new_v4().simple());
    let log_file = app.config.log_dir
        .join(&task_name)
        .join(format!("{}.log", execution_id));
    tokio::fs::create_dir_all(log_file.parent().unwrap()).await?;
    
    // 4. 构造执行记录
    let started_at = Utc::now();
    let execution = TaskExecutionRecord {
        execution_id: execution_id.clone(),
        task_name: task_name.clone(),
        started_at,
        finished_at: None,
        exit_code: None,
        status: ExecutionStatus::Running,
        trigger: TriggerType::Manual,
        pid: None,
        stdout_preview: String::new(),
        stderr_preview: String::new(),
        log_file: log_file.clone(),
    };
    
    // 5. 提交任务到调度引擎
    let task = ScheduledTask {
        name: task_name.clone(),
        trigger: Trigger::OneShot,
        execution: Execution::MiseTask {
            task_name: task_name.clone(),
            args: req.args,
        },
        state: TaskState::Pending,
        limits: None,
        timeout: if req.timeout > 0 { Some(Duration::from_secs(req.timeout)) } else { None },
        restart_policy: RestartPolicy::No,
    };
    
    let pid = app.scheduler.submit_task(task, execution.clone()).await?;
    
    // 6. 如果 wait=false，立即返回
    if !req.wait {
        return Ok(Json(ApiResponse::success(RunTaskResponse {
            execution_id,
            task_name,
            started_at,
            status: ExecutionStatus::Running,
            pid: Some(pid),
            log_file,
            finished_at: None,
            exit_code: None,
        })));
    }
    
    // 7. wait=true，轮询直到完成
    let timeout = Duration::from_secs(if req.timeout > 0 { req.timeout } else { 3600 });
    let final_execution = tokio::time::timeout(timeout, async {
        loop {
            match app.scheduler.get_execution_status(&execution_id).await? {
                Some(exec) if exec.status != ExecutionStatus::Running => {
                    return Ok(exec);
                }
                _ => tokio::time::sleep(Duration::from_millis(500)).await,
            }
        }
    }).await
        .map_err(|_| ApiError::timeout("EXECUTION_TIMEOUT", 
            "Task execution exceeded timeout"))?;
    
    Ok(Json(ApiResponse::success(RunTaskResponse {
        execution_id: final_execution.execution_id,
        task_name,
        started_at: final_execution.started_at,
        status: final_execution.status,
        pid: Some(pid),
        log_file,
        finished_at: final_execution.finished_at,
        exit_code: final_execution.exit_code,
    })))
}
```

### 创建定时任务 Handler

```rust
#[derive(Debug, Deserialize)]
pub struct CreateScheduledTaskRequest {
    pub name: String,
    pub description: Option<String>,
    pub execution: TaskExecution,
    pub schedule: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub timeout: u64,
    pub limits: Option<ResourceLimits>,
}

/// POST /api/v1/scheduled-tasks
pub async fn create_scheduled_task(
    State(app): State<Arc<AppState>>,
    Json(req): Json<CreateScheduledTaskRequest>,
) -> Result<Json<ApiResponse<ScheduledTask>>, ApiError> {
    // 1. 验证任务名称
    validate_task_name(&req.name)?;
    
    // 2. 检查名称冲突
    if app.config_manager.scheduled_task_exists(&req.name).await? {
        return Err(ApiError::conflict("SCHEDULED_TASK_ALREADY_EXISTS",
            format!("Scheduled task '{}' already exists", req.name)));
    }
    
    // 3. 验证 cron 表达式
    let schedule = cron::Schedule::from_str(&req.schedule)
        .map_err(|e| ApiError::bad_request("INVALID_CRON_EXPRESSION",
            format!("Invalid cron expression: {}", e)))?;
    
    // 4. 如果是 mise 任务，验证任务存在性
    if let TaskExecution::MiseTask { task, .. } = &req.execution {
        app.mise_adapter.get_task_definition(task).await
            .map_err(|_| ApiError::bad_request("MISE_TASK_NOT_FOUND",
                format!("mise task '{}' not found", task)))?;
    }
    
    // 5. 验证资源限制
    if let Some(limits) = &req.limits {
        validate_resource_limits(limits)?;
    }
    
    // 6. 计算下次运行时间
    let next_run = if req.enabled {
        schedule.upcoming(Utc).next().map(|dt| dt.into())
    } else {
        None
    };
    
    // 7. 构造配置对象
    let task = ScheduledTask {
        name: req.name.clone(),
        description: req.description,
        execution: req.execution,
        schedule: req.schedule.clone(),
        enabled: req.enabled,
        timeout: req.timeout,
        limits: req.limits,
        next_run,
    };
    
    // 8. 写入配置文件
    app.config_manager.add_scheduled_task(&task).await?;
    
    // 9. Git 自动暂存
    app.git_atom.stage_file(".config/mise/svcmgr/config.toml").await?;
    
    // 10. 在调度引擎注册
    if req.enabled {
        app.scheduler.register_cron_task(&task).await?;
    }
    
    Ok(Json(ApiResponse::success(task)))
}

/// 验证任务名称
fn validate_task_name(name: &str) -> Result<(), ApiError> {
    if name.is_empty() || name.len() > 64 {
        return Err(ApiError::bad_request("INVALID_TASK_NAME",
            "Task name must be 1-64 characters"));
    }
    
    if !name.chars().next().unwrap().is_ascii_alphabetic() {
        return Err(ApiError::bad_request("INVALID_TASK_NAME",
            "Task name must start with a letter"));
    }
    
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(ApiError::bad_request("INVALID_TASK_NAME",
            "Task name can only contain letters, numbers, and underscores"));
    }
    
    Ok(())
}

/// 验证资源限制
fn validate_resource_limits(limits: &ResourceLimits) -> Result<(), ApiError> {
    if let Some(memory) = limits.memory {
        if memory < 1_048_576 { // 1MB
            return Err(ApiError::bad_request("RESOURCE_LIMIT_EXCEEDED",
                "Memory limit must be at least 1MB"));
        }
    }
    
    if let Some(cpu_quota) = limits.cpu_quota {
        if cpu_quota < 1000 || cpu_quota > 100000 {
            return Err(ApiError::bad_request("RESOURCE_LIMIT_EXCEEDED",
                "CPU quota must be between 1000 (1%) and 100000 (100%)"));
        }
    }
    
    Ok(())
}
```

---

## 测试用例

### 1. 立即执行 mise 任务（成功）

```rust
#[tokio::test]
async fn test_run_task_success() {
    let app = setup_test_app().await;
    
    // 准备 mise 配置
    app.mise_adapter.add_task_definition(TaskDefinition {
        name: "test_task".to_string(),
        run: "echo hello".to_string(),
        description: None,
        env: HashMap::new(),
        dir: None,
        depends: vec![],
        alias: vec![],
        source: PathBuf::from("/test/config.toml"),
    }).await;
    
    // 执行任务
    let req = RunTaskRequest {
        args: vec![],
        env: HashMap::new(),
        timeout: 10,
        wait: true,
    };
    
    let res = run_task(
        State(app.clone()),
        Path("test_task".to_string()),
        Json(req),
    ).await.unwrap();
    
    // 验证
    assert_eq!(res.0.data.status, ExecutionStatus::Success);
    assert_eq!(res.0.data.exit_code, Some(0));
    assert!(res.0.data.stdout_preview.contains("hello"));
}
```

### 2. 创建定时任务（cron 表达式错误）

```rust
#[tokio::test]
async fn test_create_scheduled_task_invalid_cron() {
    let app = setup_test_app().await;
    
    let req = CreateScheduledTaskRequest {
        name: "invalid_task".to_string(),
        description: None,
        execution: TaskExecution::Command {
            command: "echo test".to_string(),
            env: HashMap::new(),
            dir: None,
        },
        schedule: "invalid cron".to_string(), // 错误的 cron 表达式
        enabled: true,
        timeout: 0,
        limits: None,
    };
    
    let res = create_scheduled_task(
        State(app.clone()),
        Json(req),
    ).await;
    
    // 验证错误
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert_eq!(err.code, "INVALID_CRON_EXPRESSION");
}
```

### 3. 取消正在运行的任务

```rust
#[tokio::test]
async fn test_cancel_running_task() {
    let app = setup_test_app().await;
    
    // 启动长时间运行任务
    app.mise_adapter.add_task_definition(TaskDefinition {
        name: "long_task".to_string(),
        run: "sleep 60".to_string(),
        description: None,
        env: HashMap::new(),
        dir: None,
        depends: vec![],
        alias: vec![],
        source: PathBuf::from("/test/config.toml"),
    }).await;
    
    let run_req = RunTaskRequest {
        args: vec![],
        env: HashMap::new(),
        timeout: 0,
        wait: false,
    };
    
    let run_res = run_task(
        State(app.clone()),
        Path("long_task".to_string()),
        Json(run_req),
    ).await.unwrap();
    
    let execution_id = run_res.0.data.execution_id;
    
    // 等待任务启动
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // 取消任务
    let cancel_req = CancelTaskRequest {
        signal: "SIGTERM".to_string(),
        timeout: 5,
    };
    
    let cancel_res = cancel_task(
        State(app.clone()),
        Path("long_task".to_string()),
        Json(cancel_req),
    ).await.unwrap();
    
    // 验证
    assert_eq!(cancel_res.0.data.status, ExecutionStatus::Cancelled);
}
```

---

## 配置文件交互

### mise 任务定义（只读）

```toml
# .config/mise/config.toml
[tasks.backup]
run = "pg_dump mydb > backup.sql"
description = "Database backup"
env = { PGPASSWORD = "secret" }
dir = "/data"

[tasks.deploy]
run = "./scripts/deploy.sh"
depends = ["test"]
alias = ["ship"]
```

**svcmgr 行为**：
- 解析此文件获取任务定义（通过 MiseConfigPort）
- **不修改** mise 配置文件（任务定义由用户维护）
- 通过 `POST /api/v1/tasks/{name}/run` 立即执行任务

### svcmgr 定时任务配置（读写）

```toml
# .config/mise/svcmgr/config.toml
[scheduled_tasks.nightly_backup]
type = "mise_task"
task = "backup"
args = ["--incremental"]
schedule = "0 3 * * *"
enabled = true
timeout = 3600

[scheduled_tasks.nightly_backup.limits]
memory = 2147483648
cpu_quota = 50000

[scheduled_tasks.weekly_cleanup]
type = "command"
command = "find /tmp -mtime +7 -delete"
schedule = "0 2 * * 0"
enabled = true
```

**svcmgr 行为**：
- 读取此文件加载定时任务（启动时）
- 通过 API 修改后写回此文件
- 自动 Git 暂存变更

---

## 与调度引擎集成

### 任务提交流程

```rust
// 调度引擎接口
pub trait SchedulerEngine {
    /// 提交一次性任务（OneShot 触发）
    async fn submit_task(
        &self,
        task: ScheduledTask,
        execution: TaskExecutionRecord,
    ) -> Result<u32>; // 返回 PID
    
    /// 注册定时任务（Cron 触发）
    async fn register_cron_task(&self, task: &ScheduledTask) -> Result<()>;
    
    /// 注销定时任务
    async fn unregister_cron_task(&self, name: &str) -> Result<()>;
    
    /// 取消正在运行的任务
    async fn cancel_task(&self, name: &str, signal: Signal, timeout: Duration) -> Result<()>;
    
    /// 查询任务运行状态
    async fn is_task_running(&self, name: &str) -> bool;
    
    /// 获取执行记录
    async fn get_execution_status(&self, execution_id: &str) -> Result<Option<TaskExecutionRecord>>;
}
```

### 状态变更事件

```rust
/// 任务状态变更事件（调度引擎发出）
pub enum TaskEvent {
    /// 任务启动
    Started {
        execution_id: String,
        task_name: String,
        pid: u32,
    },
    
    /// 任务完成
    Finished {
        execution_id: String,
        exit_code: i32,
    },
    
    /// 任务失败
    Failed {
        execution_id: String,
        error: String,
    },
    
    /// 任务取消
    Cancelled {
        execution_id: String,
    },
}

// API Handler 订阅事件更新执行记录
app.event_bus.subscribe("task_events", |event: TaskEvent| {
    match event {
        TaskEvent::Finished { execution_id, exit_code } => {
            // 更新执行记录状态
            app.execution_history.update_status(&execution_id, ExecutionStatus::Success).await?;
        }
        _ => {}
    }
});
```

---

## 与服务管理的区别

| 特性 | 任务 API (`/api/v1/tasks`) | 服务 API (`/api/v1/services`) |
|------|---------------------------|-------------------------------|
| **生命周期** | 一次性或定时执行 | 长期运行 |
| **启动方式** | `POST /tasks/{name}/run` | `POST /services/{name}/start` |
| **停止方式** | `POST /tasks/{name}/cancel` | `POST /services/{name}/stop` |
| **重启** | 不支持（任务完成即退出） | `POST /services/{name}/restart` |
| **日志** | 执行历史归档（`/tasks/{name}/history`） | 实时流式日志（`/services/{name}/logs?follow=true`） |
| **依赖** | 任务依赖（`depends` 字段） | 服务依赖（`dependencies` 字段） |
| **触发器** | OneShot / Cron / Event | OneShot (autostart) |
| **配置存储** | mise 配置（只读）+ svcmgr 配置（定时任务） | svcmgr 配置 |

---

## 相关规范

- **02-scheduler-engine.md** - 调度引擎设计（Trigger 类型、TaskState、ScheduledTask）
- **03-process-manager.md** - 子进程管理（资源限制、信号处理、日志管理）
- **10-api-overview.md** - API 设计总览（认证、版本管理、错误响应、幂等性）
- **11-api-services.md** - 服务管理 API（对比任务与服务的区别）
- **13-api-tools.md** - 工具管理 API（mise tools 安装/卸载，与任务定义关联）

---

## 未来扩展

1. **任务链**：支持任务 DAG（有向无环图），多个任务按依赖顺序执行
2. **条件触发**：基于系统事件（文件变化、网络连接、任务完成）触发任务
3. **任务模板**：预定义任务模板（备份、部署、测试），快速创建实例
4. **执行统计**：任务执行次数、平均耗时、成功率统计
5. **通知集成**：任务失败时通过 webhook/email 通知
6. **并发控制**：限制同名任务的并发执行数量（`max_concurrent`）
7. **重试策略**：任务失败后自动重试（`retry_count`, `retry_delay`）
