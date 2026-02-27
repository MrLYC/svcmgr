# 14 - 配置管理 API

> 版本：2.0.0-draft  
> 状态：设计中

## 设计目标

配置管理 API 提供以下能力：

1. **配置读取**：获取完整配置或特定段落（mise.toml + svcmgr.toml）
2. **配置更新**：更新配置并自动触发 Git 暂存/提交
3. **配置验证**：语法和语义验证（检查依赖、循环引用等）
4. **配置回滚**：基于 Git 历史回滚到指定版本
5. **配置历史**：查看配置变更历史（Git commit log）
6. **配置导出/导入**：备份和恢复配置（JSON 格式）
7. **分段管理**：对 tools、env、services、scheduled_tasks 等段落独立操作

## 为什么需要配置管理 API？

### 配置文件分离策略

根据 **01-config-design.md** 和 **MISE_REDESIGN_RESEARCH_ZH.md**，svcmgr 使用两层配置：

```
.config/mise/
├── config.toml          # mise 配置（tools, env, tasks）
└── svcmgr/
    └── config.toml      # svcmgr 配置（services, scheduled_tasks, features, http）
```

**分离原因**：
- **避免冲突**：mise 未来可能引入与 svcmgr 冲突的新段落（如 `[services]`）
- **独立版本化**：svcmgr 配置可以有独立的 Git 仓库
- **清晰边界**：mise 管理依赖/环境/任务，svcmgr 管理服务/调度/功能

### 配置与 Git 版本化集成

根据 **04-git-versioning.md**，所有配置变更都通过 Git 管理：

```
配置更新 → 文件写入 → Git add → Git commit → ConfigChanged 事件
```

**自动化流程**：
- 每次 API 更新配置都自动暂存（`git add`）
- 可选择立即提交或延迟提交（批量变更）
- 提交后触发 `ConfigChanged` 事件，通知调度引擎重新加载
- 支持回滚到任意历史版本（`git reset --hard <commit>`）

### 配置验证策略

配置验证分为两层：

1. **语法验证**：TOML 语法、字段类型、必填项
2. **语义验证**：
   - 依赖检查：服务依赖的工具是否已安装
   - 循环依赖检查：服务 A 依赖 B，B 依赖 A
   - 端口冲突检查：两个服务绑定同一端口
   - 路径有效性：command、working_dir 是否存在

**验证时机**：
- 配置更新时自动验证（失败则拒绝更新）
- 可通过 `POST /api/v1/config/validate` 手动验证（dry-run）

## API 端点概览

| HTTP 方法 | 路径 | 用途 |
|-----------|------|------|
| **配置读取** |
| GET | `/api/v1/config` | 获取完整配置（mise + svcmgr） |
| GET | `/api/v1/config/{section}` | 获取特定段落（tools/env/services 等） |
| **配置更新** |
| PUT | `/api/v1/config` | 完整替换配置 |
| PATCH | `/api/v1/config/{section}` | 部分更新特定段落 |
| **配置验证** |
| POST | `/api/v1/config/validate` | 验证配置（不实际应用） |
| **配置历史与回滚** |
| GET | `/api/v1/config/history` | 获取配置变更历史 |
| POST | `/api/v1/config/rollback` | 回滚到指定版本 |
| GET | `/api/v1/config/diff` | 对比两个版本的差异 |
| **配置导出/导入** |
| GET | `/api/v1/config/export` | 导出配置为 JSON |
| POST | `/api/v1/config/import` | 导入配置并应用 |

## 数据模型

### Config (完整配置)

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 完整配置（mise.toml + svcmgr.toml）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// mise 配置段
    pub mise: MiseConfig,
    
    /// svcmgr 配置段
    pub svcmgr: SvcmgrConfig,
    
    /// 元数据
    pub metadata: ConfigMetadata,
}

/// mise 配置（来自 .config/mise/config.toml）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiseConfig {
    /// 工具版本（[tools] 段）
    #[serde(default)]
    pub tools: HashMap<String, String>,
    
    /// 环境变量（[env] 段）
    #[serde(default)]
    pub env: HashMap<String, String>,
    
    /// 任务定义（[tasks.*] 段）
    #[serde(default)]
    pub tasks: HashMap<String, TaskConfig>,
}

/// svcmgr 配置（来自 .config/mise/svcmgr/config.toml）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvcmgrConfig {
    /// 服务定义（[services.*] 段）
    #[serde(default)]
    pub services: HashMap<String, ServiceConfig>,
    
    /// 定时任务（[scheduled_tasks.*] 段）
    #[serde(default)]
    pub scheduled_tasks: HashMap<String, ScheduledTaskConfig>,
    
    /// 功能开关（[features] 段）
    #[serde(default)]
    pub features: FeaturesConfig,
    
    /// HTTP 路由（[[http.routes]] 段）
    #[serde(default)]
    pub http: HttpConfig,
}

/// 配置元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigMetadata {
    /// 当前 Git commit hash
    pub commit: String,
    
    /// 最后修改时间（Unix timestamp）
    pub last_modified: i64,
    
    /// 配置文件路径
    pub paths: ConfigPaths,
}

/// 配置文件路径
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigPaths {
    /// mise 配置文件路径
    pub mise_config: String,
    
    /// svcmgr 配置文件路径
    pub svcmgr_config: String,
}

/// 任务配置（mise [tasks.*]）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskConfig {
    /// 任务描述
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    
    /// 任务命令（单行或多行）
    #[serde(alias = "run")]
    pub command: TaskCommand,
    
    /// 依赖的其他任务
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends: Vec<String>,
    
    /// 环境变量
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub env: HashMap<String, String>,
}

/// 任务命令（支持字符串或字符串数组）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TaskCommand {
    /// 单行命令
    Single(String),
    
    /// 多行命令（脚本）
    Multiple(Vec<String>),
}

/// 功能开关配置（[features]）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeaturesConfig {
    /// cgroups v2 资源限制
    #[serde(default)]
    pub cgroups: FeatureMode,
    
    /// 内置 HTTP 代理
    #[serde(default)]
    pub http_proxy: FeatureMode,
    
    /// Git 自动提交
    #[serde(default)]
    pub git_auto_commit: FeatureMode,
}

/// 功能开关模式
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FeatureMode {
    /// 自动检测（默认）
    Auto,
    
    /// 强制启用
    Enabled,
    
    /// 完全禁用
    Disabled,
}

impl Default for FeatureMode {
    fn default() -> Self {
        Self::Auto
    }
}

/// HTTP 配置（[[http.routes]]）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    /// 监听地址
    #[serde(default = "default_listen_addr")]
    pub listen: String,
    
    /// 路由规则
    #[serde(default)]
    pub routes: Vec<HttpRoute>,
}

fn default_listen_addr() -> String {
    "127.0.0.1:3080".to_string()
}

/// HTTP 路由规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpRoute {
    /// 路由路径（如 /api/*）
    pub path: String,
    
    /// 目标服务名
    pub target: String,
    
    /// 目标端口名（services.*.ports 中的 key）
    pub port: String,
    
    /// 路径重写规则（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rewrite: Option<String>,
}
```

### ConfigHistory (配置历史)

```rust
/// 配置变更历史项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigHistory {
    /// Git commit hash
    pub commit: String,
    
    /// 提交信息
    pub message: String,
    
    /// 提交时间（Unix timestamp）
    pub timestamp: i64,
    
    /// 提交者
    pub author: String,
    
    /// 变更文件列表
    pub files: Vec<String>,
}
```

### ConfigDiff (配置差异)

```rust
/// 配置差异
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigDiff {
    /// 起始版本（commit hash）
    pub from: String,
    
    /// 目标版本（commit hash）
    pub to: String,
    
    /// 差异内容（unified diff 格式）
    pub diff: String,
    
    /// 变更统计
    pub stats: DiffStats,
}

/// 差异统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffStats {
    /// 变更文件数
    pub files_changed: usize,
    
    /// 新增行数
    pub insertions: usize,
    
    /// 删除行数
    pub deletions: usize,
}
```

### ValidationResult (验证结果)

```rust
/// 配置验证结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// 是否有效
    pub valid: bool,
    
    /// 错误列表（语法错误 + 语义错误）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<ValidationError>,
    
    /// 警告列表
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<ValidationWarning>,
}

/// 验证错误
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    /// 错误类型
    pub kind: ValidationErrorKind,
    
    /// 错误位置（段落.键名）
    pub path: String,
    
    /// 错误信息
    pub message: String,
}

/// 验证错误类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ValidationErrorKind {
    /// 语法错误（TOML 解析失败）
    Syntax,
    
    /// 类型错误（字段类型不匹配）
    Type,
    
    /// 缺失必填字段
    MissingField,
    
    /// 依赖缺失（服务依赖的工具不存在）
    MissingDependency,
    
    /// 循环依赖
    CircularDependency,
    
    /// 端口冲突
    PortConflict,
    
    /// 路径无效
    InvalidPath,
    
    /// 其他错误
    Other,
}

/// 验证警告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationWarning {
    /// 警告位置（段落.键名）
    pub path: String,
    
    /// 警告信息
    pub message: String,
}
```

## API 端点详细设计

### 1. 获取完整配置

获取 mise.toml 和 svcmgr.toml 的合并视图。

```http
GET /api/v1/config
```

**查询参数**：
- `format` (可选) - 响应格式：`json` (默认) 或 `toml`

**成功响应** (200 OK)：
```json
{
  "data": {
    "mise": {
      "tools": {
        "node": "20.11.0",
        "python": "3.12.1"
      },
      "env": {
        "NODE_ENV": "production",
        "LOG_LEVEL": "info"
      },
      "tasks": {
        "dev": {
          "description": "Start development server",
          "command": "npm run dev",
          "depends": [],
          "env": {}
        }
      }
    },
    "svcmgr": {
      "services": {
        "web": {
          "command": "mise run server",
          "working_dir": "/home/user/app",
          "ports": {
            "http": 8080
          },
          "env": {},
          "health_check": {
            "type": "http",
            "endpoint": "http://localhost:8080/health",
            "interval": 10,
            "timeout": 5,
            "retries": 3
          }
        }
      },
      "scheduled_tasks": {
        "backup": {
          "command": "mise run backup",
          "schedule": "0 2 * * *",
          "enabled": true
        }
      },
      "features": {
        "cgroups": "auto",
        "http_proxy": "enabled",
        "git_auto_commit": "enabled"
      },
      "http": {
        "listen": "127.0.0.1:3080",
        "routes": [
          {
            "path": "/api/*",
            "target": "web",
            "port": "http",
            "rewrite": null
          }
        ]
      }
    },
    "metadata": {
      "commit": "a1b2c3d4",
      "last_modified": 1708675200,
      "paths": {
        "mise_config": "/home/user/.config/mise/config.toml",
        "svcmgr_config": "/home/user/.config/mise/svcmgr/config.toml"
      }
    }
  }
}
```

**TOML 格式响应** (format=toml)：
```toml
# .config/mise/config.toml
[tools]
node = "20.11.0"
python = "3.12.1"

[env]
NODE_ENV = "production"
LOG_LEVEL = "info"

[tasks.dev]
description = "Start development server"
run = "npm run dev"

# .config/mise/svcmgr/config.toml
[services.web]
command = "mise run server"
working_dir = "/home/user/app"

[services.web.ports]
http = 8080

[services.web.health_check]
type = "http"
endpoint = "http://localhost:8080/health"
interval = 10
timeout = 5
retries = 3

[scheduled_tasks.backup]
command = "mise run backup"
schedule = "0 2 * * *"
enabled = true

[features]
cgroups = "auto"
http_proxy = "enabled"
git_auto_commit = "enabled"

[http]
listen = "127.0.0.1:3080"

[[http.routes]]
path = "/api/*"
target = "web"
port = "http"
```

**错误响应** (500 Internal Server Error)：
```json
{
  "error": {
    "code": "CONFIG_READ_ERROR",
    "message": "Failed to read configuration files",
    "details": {
      "reason": "TOML parse error: missing field `command` at line 5"
    },
    "request_id": "req_abc123"
  }
}
```

### 2. 获取特定配置段落

获取配置的特定段落（如 tools、env、services 等）。

```http
GET /api/v1/config/{section}
```

**路径参数**：
- `section` (必填) - 段落名称：
  - `tools` - mise 工具版本
  - `env` - mise 环境变量
  - `tasks` - mise 任务定义
  - `services` - svcmgr 服务定义
  - `scheduled_tasks` - svcmgr 定时任务
  - `features` - svcmgr 功能开关
  - `http` - svcmgr HTTP 配置

**成功响应** (200 OK) - 示例：GET /api/v1/config/services
```json
{
  "data": {
    "web": {
      "command": "mise run server",
      "working_dir": "/home/user/app",
      "ports": {
        "http": 8080
      },
      "env": {},
      "health_check": {
        "type": "http",
        "endpoint": "http://localhost:8080/health",
        "interval": 10,
        "timeout": 5,
        "retries": 3
      }
    },
    "worker": {
      "command": "mise run worker",
      "working_dir": "/home/user/app",
      "ports": {},
      "env": {
        "WORKER_THREADS": "4"
      }
    }
  }
}
```

**错误响应** (404 Not Found)：
```json
{
  "error": {
    "code": "CONFIG_SECTION_NOT_FOUND",
    "message": "Configuration section not found",
    "details": {
      "section": "invalid_section",
      "valid_sections": [
        "tools",
        "env",
        "tasks",
        "services",
        "scheduled_tasks",
        "features",
        "http"
      ]
    },
    "request_id": "req_abc123"
  }
}
```

### 3. 完整替换配置

完整替换 mise.toml 和 svcmgr.toml 的内容，并自动 Git 提交。

```http
PUT /api/v1/config
Content-Type: application/json
```

**请求体**：
```json
{
  "mise": {
    "tools": {
      "node": "22.0.0",
      "python": "3.12.2"
    },
    "env": {
      "NODE_ENV": "production"
    },
    "tasks": {}
  },
  "svcmgr": {
    "services": {},
    "scheduled_tasks": {},
    "features": {
      "cgroups": "enabled",
      "http_proxy": "enabled",
      "git_auto_commit": "enabled"
    },
    "http": {
      "listen": "127.0.0.1:3080",
      "routes": []
    }
  },
  "commit_message": "Update configuration: upgrade Node.js to v22"
}
```

**成功响应** (200 OK)：
```json
{
  "data": {
    "commit": "f5e6d7c8",
    "message": "Update configuration: upgrade Node.js to v22",
    "timestamp": 1708675300,
    "files_changed": [
      ".config/mise/config.toml",
      ".config/mise/svcmgr/config.toml"
    ]
  }
}
```

**验证失败响应** (400 Bad Request)：
```json
{
  "error": {
    "code": "CONFIG_VALIDATION_FAILED",
    "message": "Configuration validation failed",
    "details": {
      "errors": [
        {
          "kind": "missing_dependency",
          "path": "services.web",
          "message": "Service 'web' depends on tool 'node' version 22.0.0, but it is not installed"
        }
      ],
      "warnings": []
    },
    "request_id": "req_abc123"
  }
}
```

**副作用**：
1. 写入 `.config/mise/config.toml`（mise 配置）
2. 写入 `.config/mise/svcmgr/config.toml`（svcmgr 配置）
3. 执行 `git add .config/mise/config.toml .config/mise/svcmgr/config.toml`
4. 执行 `git commit -m "<commit_message>"`
5. 发布 `ConfigChanged` 事件到事件总线
6. 调度引擎重新加载配置并重启受影响的服务

### 4. 部分更新配置段落

部分更新特定配置段落，仅影响指定段落，其他段落保持不变。

```http
PATCH /api/v1/config/{section}
Content-Type: application/json
```

**路径参数**：
- `section` (必填) - 段落名称（同 GET）

**请求体** - 示例：PATCH /api/v1/config/tools
```json
{
  "data": {
    "node": "22.0.0",
    "go": "1.22.0"
  },
  "commit_message": "Add Go 1.22.0 and upgrade Node.js"
}
```

**成功响应** (200 OK)：
```json
{
  "data": {
    "commit": "a9b8c7d6",
    "message": "Add Go 1.22.0 and upgrade Node.js",
    "timestamp": 1708675400,
    "files_changed": [
      ".config/mise/config.toml"
    ]
  }
}
```

**更新行为**：
- **合并更新**：新字段与现有字段合并
- **覆盖更新**：同名字段值被覆盖
- **删除字段**：值为 `null` 时删除该字段

**示例：删除工具**
```json
{
  "data": {
    "python": null
  },
  "commit_message": "Remove Python"
}
```

**副作用**：
- 仅修改受影响的配置文件（mise.toml 或 svcmgr.toml）
- Git 提交仅包含变更的文件
- 发布 `ConfigChanged` 事件（携带变更段落信息）

### 5. 验证配置

验证配置的语法和语义，但不实际应用（dry-run）。

```http
POST /api/v1/config/validate
Content-Type: application/json
```

**请求体**：
```json
{
  "mise": {
    "tools": {
      "node": "999.0.0"
    }
  },
  "svcmgr": {
    "services": {
      "web": {
        "command": "nonexistent-command",
        "working_dir": "/invalid/path",
        "ports": {
          "http": 8080
        }
      },
      "api": {
        "command": "node api.js",
        "ports": {
          "http": 8080
        }
      }
    }
  }
}
```

**成功响应** (200 OK) - 验证失败：
```json
{
  "data": {
    "valid": false,
    "errors": [
      {
        "kind": "missing_dependency",
        "path": "tools.node",
        "message": "Tool version 'node@999.0.0' does not exist in mise registry"
      },
      {
        "kind": "invalid_path",
        "path": "services.web.working_dir",
        "message": "Directory '/invalid/path' does not exist"
      },
      {
        "kind": "port_conflict",
        "path": "services.api.ports.http",
        "message": "Port 8080 is already used by service 'web'"
      }
    ],
    "warnings": [
      {
        "path": "services.web.command",
        "message": "Command 'nonexistent-command' is not found in PATH"
      }
    ]
  }
}
```

**成功响应** (200 OK) - 验证通过：
```json
{
  "data": {
    "valid": true,
    "errors": [],
    "warnings": []
  }
}
```

**验证规则**：

**语法验证**：
- TOML 语法正确性
- 字段类型匹配（字符串、整数、布尔等）
- 必填字段存在

**语义验证**：
- **依赖检查**：服务依赖的工具版本是否存在于 mise registry
- **循环依赖检查**：服务 A 依赖 B，B 依赖 C，C 依赖 A
- **端口冲突检查**：多个服务绑定同一端口
- **路径有效性**：`working_dir` 是否存在
- **命令有效性**：`command` 是否存在于 PATH（警告级别）

### 6. 获取配置历史

获取配置变更的 Git commit 历史。

```http
GET /api/v1/config/history
```

**查询参数**：
- `limit` (可选) - 返回的历史记录数量（默认 50，最大 500）
- `offset` (可选) - 分页偏移量（默认 0）
- `since` (可选) - 起始时间（Unix timestamp 或 ISO 8601）
- `until` (可选) - 结束时间（Unix timestamp 或 ISO 8601）

**成功响应** (200 OK)：
```json
{
  "data": [
    {
      "commit": "a1b2c3d4",
      "message": "Update configuration: upgrade Node.js to v22",
      "timestamp": 1708675300,
      "author": "user <user@example.com>",
      "files": [
        ".config/mise/config.toml"
      ]
    },
    {
      "commit": "e5f6g7h8",
      "message": "Add new service: worker",
      "timestamp": 1708671700,
      "author": "user <user@example.com>",
      "files": [
        ".config/mise/svcmgr/config.toml"
      ]
    },
    {
      "commit": "i9j0k1l2",
      "message": "Initial configuration",
      "timestamp": 1708668100,
      "author": "user <user@example.com>",
      "files": [
        ".config/mise/config.toml",
        ".config/mise/svcmgr/config.toml"
      ]
    }
  ],
  "pagination": {
    "total": 3,
    "limit": 50,
    "offset": 0,
    "has_more": false
  }
}
```

### 7. 回滚配置

回滚配置到指定的 Git commit 版本。

```http
POST /api/v1/config/rollback
Content-Type: application/json
```

**请求体**：
```json
{
  "commit": "e5f6g7h8",
  "reason": "Revert to stable configuration before Node.js upgrade"
}
```

**成功响应** (200 OK)：
```json
{
  "data": {
    "commit": "e5f6g7h8",
    "message": "Add new service: worker",
    "timestamp": 1708671700,
    "rollback_commit": "m3n4o5p6",
    "rollback_message": "Rollback: Revert to stable configuration before Node.js upgrade",
    "files_restored": [
      ".config/mise/config.toml"
    ]
  }
}
```

**错误响应** (404 Not Found)：
```json
{
  "error": {
    "code": "COMMIT_NOT_FOUND",
    "message": "Commit not found in configuration history",
    "details": {
      "commit": "invalid_commit"
    },
    "request_id": "req_abc123"
  }
}
```

**回滚实现**：
```bash
# 1. 验证 commit 存在
git rev-parse --verify <commit>

# 2. 回滚文件（不改变 HEAD）
git checkout <commit> -- .config/mise/config.toml .config/mise/svcmgr/config.toml

# 3. 提交回滚变更
git commit -m "Rollback: <reason>"

# 4. 发布 ConfigRolledBack 事件
```

**副作用**：
1. 恢复指定 commit 时的配置文件内容
2. 创建新的 Git commit（回滚不改变历史）
3. 发布 `ConfigRolledBack` 事件
4. 调度引擎重新加载配置并重启所有服务

### 8. 对比配置差异

对比两个 Git commit 版本的配置差异。

```http
GET /api/v1/config/diff
```

**查询参数**：
- `from` (必填) - 起始版本（commit hash 或 `HEAD~N`）
- `to` (可选) - 目标版本（默认 `HEAD`）

**成功响应** (200 OK)：
```json
{
  "data": {
    "from": "e5f6g7h8",
    "to": "a1b2c3d4",
    "diff": "diff --git a/.config/mise/config.toml b/.config/mise/config.toml\nindex e5f6g7h..a1b2c3d 100644\n--- a/.config/mise/config.toml\n+++ b/.config/mise/config.toml\n@@ -1,5 +1,5 @@\n [tools]\n-node = \"20.11.0\"\n+node = \"22.0.0\"\n python = \"3.12.1\"\n",
    "stats": {
      "files_changed": 1,
      "insertions": 1,
      "deletions": 1
    }
  }
}
```

**错误响应** (400 Bad Request)：
```json
{
  "error": {
    "code": "INVALID_COMMIT_RANGE",
    "message": "Invalid commit range for diff",
    "details": {
      "from": "invalid_commit",
      "to": "HEAD"
    },
    "request_id": "req_abc123"
  }
}
```

### 9. 导出配置

导出完整配置为 JSON 格式（用于备份）。

```http
GET /api/v1/config/export
```

**查询参数**：
- `format` (可选) - 导出格式：`json` (默认) 或 `toml`

**成功响应** (200 OK)：
```json
{
  "data": {
    "version": "2.0.0",
    "exported_at": 1708675500,
    "commit": "a1b2c3d4",
    "config": {
      "mise": { /* 完整 mise 配置 */ },
      "svcmgr": { /* 完整 svcmgr 配置 */ }
    }
  }
}
```

**TOML 格式导出**：
返回合并后的 TOML 文件内容（Content-Type: application/toml）。

### 10. 导入配置

导入配置并应用（从备份恢复）。

```http
POST /api/v1/config/import
Content-Type: application/json
```

**请求体**：
```json
{
  "config": {
    "mise": { /* 完整 mise 配置 */ },
    "svcmgr": { /* 完整 svcmgr 配置 */ }
  },
  "commit_message": "Restore configuration from backup (2026-02-20)",
  "validate": true
}
```

**成功响应** (200 OK)：
```json
{
  "data": {
    "commit": "q7r8s9t0",
    "message": "Restore configuration from backup (2026-02-20)",
    "timestamp": 1708675600,
    "files_changed": [
      ".config/mise/config.toml",
      ".config/mise/svcmgr/config.toml"
    ]
  }
}
```

**验证失败响应** (400 Bad Request)：
```json
{
  "error": {
    "code": "CONFIG_VALIDATION_FAILED",
    "message": "Imported configuration is invalid",
    "details": {
      "errors": [ /* 验证错误列表 */ ]
    },
    "request_id": "req_abc123"
  }
}
```

**导入流程**：
1. 验证导入的配置（如果 `validate=true`）
2. 分离配置为 mise.toml 和 svcmgr.toml
3. 写入配置文件
4. Git add + commit
5. 发布 `ConfigChanged` 事件

## Handler 实现示例

### ConfigHandler (配置管理 Handler)

```rust
use crate::config::{Config, MiseConfig, SvcmgrConfig, ValidationResult};
use crate::git::GitPort;
use crate::events::EventBus;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// 配置管理 Handler
pub struct ConfigHandler {
    git: Arc<dyn GitPort>,
    event_bus: Arc<EventBus>,
    mise_config_path: String,
    svcmgr_config_path: String,
}

impl ConfigHandler {
    pub fn new(
        git: Arc<dyn GitPort>,
        event_bus: Arc<EventBus>,
        mise_config_path: String,
        svcmgr_config_path: String,
    ) -> Self {
        Self {
            git,
            event_bus,
            mise_config_path,
            svcmgr_config_path,
        }
    }

    /// GET /api/v1/config - 获取完整配置
    pub async fn get_config(
        &self,
        Query(params): Query<GetConfigParams>,
    ) -> Result<Json<Config>, ApiError> {
        // 1. 读取两个配置文件
        let mise_toml = tokio::fs::read_to_string(&self.mise_config_path).await?;
        let svcmgr_toml = tokio::fs::read_to_string(&self.svcmgr_config_path).await?;

        // 2. 解析 TOML
        let mise_config: MiseConfig = toml::from_str(&mise_toml)
            .map_err(|e| ApiError::ConfigParseError(format!("mise.toml: {}", e)))?;
        let svcmgr_config: SvcmgrConfig = toml::from_str(&svcmgr_toml)
            .map_err(|e| ApiError::ConfigParseError(format!("svcmgr.toml: {}", e)))?;

        // 3. 获取 Git 元数据
        let commit = self.git.get_current_commit().await?;
        let last_modified = tokio::fs::metadata(&self.mise_config_path)
            .await?
            .modified()?
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs() as i64;

        // 4. 构造响应
        let config = Config {
            mise: mise_config,
            svcmgr: svcmgr_config,
            metadata: ConfigMetadata {
                commit,
                last_modified,
                paths: ConfigPaths {
                    mise_config: self.mise_config_path.clone(),
                    svcmgr_config: self.svcmgr_config_path.clone(),
                },
            },
        };

        // 5. 根据格式返回
        if params.format == Some("toml".to_string()) {
            // 返回 TOML 格式（需自定义响应类型）
            Ok(Json(config)) // 简化示例，实际需返回 TOML 字符串
        } else {
            Ok(Json(config))
        }
    }

    /// GET /api/v1/config/{section} - 获取特定段落
    pub async fn get_config_section(
        &self,
        Path(section): Path<String>,
    ) -> Result<Json<serde_json::Value>, ApiError> {
        // 1. 读取完整配置
        let config = self.get_config(Query(GetConfigParams { format: None })).await?;

        // 2. 根据 section 提取段落
        let section_data = match section.as_str() {
            "tools" => serde_json::to_value(&config.0.mise.tools)?,
            "env" => serde_json::to_value(&config.0.mise.env)?,
            "tasks" => serde_json::to_value(&config.0.mise.tasks)?,
            "services" => serde_json::to_value(&config.0.svcmgr.services)?,
            "scheduled_tasks" => serde_json::to_value(&config.0.svcmgr.scheduled_tasks)?,
            "features" => serde_json::to_value(&config.0.svcmgr.features)?,
            "http" => serde_json::to_value(&config.0.svcmgr.http)?,
            _ => return Err(ApiError::ConfigSectionNotFound(section)),
        };

        Ok(Json(section_data))
    }

    /// PUT /api/v1/config - 完整替换配置
    pub async fn update_config(
        &self,
        Json(req): Json<UpdateConfigRequest>,
    ) -> Result<Json<ConfigUpdateResponse>, ApiError> {
        // 1. 验证配置
        let validation = self.validate_config_internal(&req.mise, &req.svcmgr).await?;
        if !validation.valid {
            return Err(ApiError::ConfigValidationFailed(validation.errors));
        }

        // 2. 序列化为 TOML
        let mise_toml = toml::to_string_pretty(&req.mise)?;
        let svcmgr_toml = toml::to_string_pretty(&req.svcmgr)?;

        // 3. 写入文件
        tokio::fs::write(&self.mise_config_path, mise_toml).await?;
        tokio::fs::write(&self.svcmgr_config_path, svcmgr_toml).await?;

        // 4. Git add + commit
        let files = vec![
            self.mise_config_path.clone(),
            self.svcmgr_config_path.clone(),
        ];
        self.git.stage(&files).await?;
        let commit = self.git.commit(&req.commit_message).await?;

        // 5. 发布事件
        self.event_bus.publish(Event::ConfigChanged {
            commit: commit.clone(),
            files: files.clone(),
        }).await?;

        // 6. 返回响应
        Ok(Json(ConfigUpdateResponse {
            commit,
            message: req.commit_message.clone(),
            timestamp: chrono::Utc::now().timestamp(),
            files_changed: files,
        }))
    }

    /// PATCH /api/v1/config/{section} - 部分更新段落
    pub async fn patch_config_section(
        &self,
        Path(section): Path<String>,
        Json(req): Json<PatchConfigSectionRequest>,
    ) -> Result<Json<ConfigUpdateResponse>, ApiError> {
        // 1. 读取当前配置
        let mut config = self.get_config(Query(GetConfigParams { format: None })).await?.0;

        // 2. 更新指定段落
        let affected_file = match section.as_str() {
            "tools" => {
                // 合并更新 tools
                for (key, value) in req.data.as_object().unwrap() {
                    if value.is_null() {
                        config.mise.tools.remove(key);
                    } else {
                        config.mise.tools.insert(key.clone(), value.as_str().unwrap().to_string());
                    }
                }
                self.mise_config_path.clone()
            }
            "env" => {
                // 合并更新 env
                for (key, value) in req.data.as_object().unwrap() {
                    if value.is_null() {
                        config.mise.env.remove(key);
                    } else {
                        config.mise.env.insert(key.clone(), value.as_str().unwrap().to_string());
                    }
                }
                self.mise_config_path.clone()
            }
            "services" => {
                // 合并更新 services（简化示例）
                self.svcmgr_config_path.clone()
            }
            _ => return Err(ApiError::ConfigSectionNotFound(section)),
        };

        // 3. 验证更新后的配置
        let validation = self.validate_config_internal(&config.mise, &config.svcmgr).await?;
        if !validation.valid {
            return Err(ApiError::ConfigValidationFailed(validation.errors));
        }

        // 4. 写入文件（仅写入受影响的文件）
        if affected_file == self.mise_config_path {
            let mise_toml = toml::to_string_pretty(&config.mise)?;
            tokio::fs::write(&self.mise_config_path, mise_toml).await?;
        } else {
            let svcmgr_toml = toml::to_string_pretty(&config.svcmgr)?;
            tokio::fs::write(&self.svcmgr_config_path, svcmgr_toml).await?;
        }

        // 5. Git add + commit
        self.git.stage(&[affected_file.clone()]).await?;
        let commit = self.git.commit(&req.commit_message).await?;

        // 6. 发布事件（携带变更段落信息）
        self.event_bus.publish(Event::ConfigChanged {
            commit: commit.clone(),
            files: vec![affected_file.clone()],
        }).await?;

        Ok(Json(ConfigUpdateResponse {
            commit,
            message: req.commit_message.clone(),
            timestamp: chrono::Utc::now().timestamp(),
            files_changed: vec![affected_file],
        }))
    }

    /// POST /api/v1/config/validate - 验证配置
    pub async fn validate_config(
        &self,
        Json(req): Json<ValidateConfigRequest>,
    ) -> Result<Json<ValidationResult>, ApiError> {
        let validation = self.validate_config_internal(&req.mise, &req.svcmgr).await?;
        Ok(Json(validation))
    }

    /// GET /api/v1/config/history - 获取配置历史
    pub async fn get_config_history(
        &self,
        Query(params): Query<ConfigHistoryParams>,
    ) -> Result<Json<Vec<ConfigHistory>>, ApiError> {
        let limit = params.limit.unwrap_or(50).min(500);
        let offset = params.offset.unwrap_or(0);

        // 1. 获取 Git log
        let commits = self.git.log(limit, offset).await?;

        // 2. 过滤配置文件相关的 commit
        let history: Vec<ConfigHistory> = commits
            .into_iter()
            .filter(|commit| {
                commit.files.iter().any(|f| {
                    f.contains("mise/config.toml") || f.contains("mise/svcmgr/config.toml")
                })
            })
            .map(|commit| ConfigHistory {
                commit: commit.hash,
                message: commit.message,
                timestamp: commit.timestamp,
                author: commit.author,
                files: commit.files,
            })
            .collect();

        Ok(Json(history))
    }

    /// POST /api/v1/config/rollback - 回滚配置
    pub async fn rollback_config(
        &self,
        Json(req): Json<RollbackConfigRequest>,
    ) -> Result<Json<RollbackConfigResponse>, ApiError> {
        // 1. 验证 commit 存在
        if !self.git.commit_exists(&req.commit).await? {
            return Err(ApiError::CommitNotFound(req.commit));
        }

        // 2. 回滚文件
        self.git.checkout_file(&req.commit, &self.mise_config_path).await?;
        self.git.checkout_file(&req.commit, &self.svcmgr_config_path).await?;

        // 3. 提交回滚变更
        let rollback_message = format!("Rollback: {}", req.reason);
        self.git.stage(&[
            self.mise_config_path.clone(),
            self.svcmgr_config_path.clone(),
        ]).await?;
        let rollback_commit = self.git.commit(&rollback_message).await?;

        // 4. 发布事件
        self.event_bus.publish(Event::ConfigRolledBack {
            original_commit: req.commit.clone(),
            rollback_commit: rollback_commit.clone(),
            reason: req.reason.clone(),
        }).await?;

        // 5. 获取目标 commit 信息
        let target_commit_info = self.git.get_commit_info(&req.commit).await?;

        Ok(Json(RollbackConfigResponse {
            commit: req.commit,
            message: target_commit_info.message,
            timestamp: target_commit_info.timestamp,
            rollback_commit,
            rollback_message,
            files_restored: vec![
                self.mise_config_path.clone(),
                self.svcmgr_config_path.clone(),
            ],
        }))
    }

    /// GET /api/v1/config/diff - 对比配置差异
    pub async fn diff_config(
        &self,
        Query(params): Query<ConfigDiffParams>,
    ) -> Result<Json<ConfigDiff>, ApiError> {
        let to = params.to.unwrap_or_else(|| "HEAD".to_string());

        // 1. 执行 git diff
        let diff_output = self.git.diff(&params.from, &to).await?;

        // 2. 解析 diff 统计信息（简化示例）
        let lines: Vec<&str> = diff_output.lines().collect();
        let insertions = lines.iter().filter(|l| l.starts_with('+')).count();
        let deletions = lines.iter().filter(|l| l.starts_with('-')).count();
        let files_changed = 1; // 简化：实际需解析 diff 头部

        Ok(Json(ConfigDiff {
            from: params.from,
            to,
            diff: diff_output,
            stats: DiffStats {
                files_changed,
                insertions,
                deletions,
            },
        }))
    }

    /// GET /api/v1/config/export - 导出配置
    pub async fn export_config(
        &self,
        Query(params): Query<ExportConfigParams>,
    ) -> Result<Json<ConfigExport>, ApiError> {
        let config = self.get_config(Query(GetConfigParams { format: None })).await?.0;

        let export = ConfigExport {
            version: "2.0.0".to_string(),
            exported_at: chrono::Utc::now().timestamp(),
            commit: config.metadata.commit.clone(),
            config,
        };

        Ok(Json(export))
    }

    /// POST /api/v1/config/import - 导入配置
    pub async fn import_config(
        &self,
        Json(req): Json<ImportConfigRequest>,
    ) -> Result<Json<ConfigUpdateResponse>, ApiError> {
        // 1. 验证配置（如果 validate=true）
        if req.validate.unwrap_or(true) {
            let validation = self.validate_config_internal(&req.config.mise, &req.config.svcmgr).await?;
            if !validation.valid {
                return Err(ApiError::ConfigValidationFailed(validation.errors));
            }
        }

        // 2. 写入文件并提交
        self.update_config(Json(UpdateConfigRequest {
            mise: req.config.mise,
            svcmgr: req.config.svcmgr,
            commit_message: req.commit_message,
        })).await
    }

    // ==================== Private Methods ====================

    /// 内部验证逻辑
    async fn validate_config_internal(
        &self,
        mise: &MiseConfig,
        svcmgr: &SvcmgrConfig,
    ) -> Result<ValidationResult, ApiError> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // 1. 语法验证（TOML 序列化测试）
        if let Err(e) = toml::to_string(mise) {
            errors.push(ValidationError {
                kind: ValidationErrorKind::Syntax,
                path: "mise".to_string(),
                message: format!("TOML syntax error: {}", e),
            });
        }
        if let Err(e) = toml::to_string(svcmgr) {
            errors.push(ValidationError {
                kind: ValidationErrorKind::Syntax,
                path: "svcmgr".to_string(),
                message: format!("TOML syntax error: {}", e),
            });
        }

        // 2. 依赖检查（服务依赖的工具是否存在）
        for (service_name, service_config) in &svcmgr.services {
            // 检查 command 中引用的工具是否在 mise.tools 中定义
            // 简化示例：实际需解析 command 字符串
            if service_config.command.contains("node") && !mise.tools.contains_key("node") {
                errors.push(ValidationError {
                    kind: ValidationErrorKind::MissingDependency,
                    path: format!("services.{}", service_name),
                    message: "Service depends on 'node' but it is not defined in [tools]".to_string(),
                });
            }
        }

        // 3. 端口冲突检查
        let mut port_usage: std::collections::HashMap<u16, Vec<String>> = std::collections::HashMap::new();
        for (service_name, service_config) in &svcmgr.services {
            for (_port_name, port_number) in &service_config.ports {
                port_usage.entry(*port_number).or_default().push(service_name.clone());
            }
        }
        for (port, services) in port_usage {
            if services.len() > 1 {
                errors.push(ValidationError {
                    kind: ValidationErrorKind::PortConflict,
                    path: format!("services.{}.ports", services[1]),
                    message: format!("Port {} is already used by service '{}'", port, services[0]),
                });
            }
        }

        // 4. 路径有效性检查
        for (service_name, service_config) in &svcmgr.services {
            if let Some(working_dir) = &service_config.working_dir {
                if !tokio::fs::metadata(working_dir).await.is_ok() {
                    errors.push(ValidationError {
                        kind: ValidationErrorKind::InvalidPath,
                        path: format!("services.{}.working_dir", service_name),
                        message: format!("Directory '{}' does not exist", working_dir),
                    });
                }
            }
        }

        // 5. 循环依赖检查（简化示例：实际需图遍历算法）
        // TODO: 实现循环依赖检测

        Ok(ValidationResult {
            valid: errors.is_empty(),
            errors,
            warnings,
        })
    }
}

// ==================== Request/Response Types ====================

#[derive(Debug, Deserialize)]
struct GetConfigParams {
    format: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateConfigRequest {
    mise: MiseConfig,
    svcmgr: SvcmgrConfig,
    commit_message: String,
}

#[derive(Debug, Serialize)]
struct ConfigUpdateResponse {
    commit: String,
    message: String,
    timestamp: i64,
    files_changed: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct PatchConfigSectionRequest {
    data: serde_json::Value,
    commit_message: String,
}

#[derive(Debug, Deserialize)]
struct ValidateConfigRequest {
    mise: MiseConfig,
    svcmgr: SvcmgrConfig,
}

#[derive(Debug, Deserialize)]
struct ConfigHistoryParams {
    limit: Option<usize>,
    offset: Option<usize>,
    since: Option<i64>,
    until: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct RollbackConfigRequest {
    commit: String,
    reason: String,
}

#[derive(Debug, Serialize)]
struct RollbackConfigResponse {
    commit: String,
    message: String,
    timestamp: i64,
    rollback_commit: String,
    rollback_message: String,
    files_restored: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ConfigDiffParams {
    from: String,
    to: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ExportConfigParams {
    format: Option<String>,
}

#[derive(Debug, Serialize)]
struct ConfigExport {
    version: String,
    exported_at: i64,
    commit: String,
    config: Config,
}

#[derive(Debug, Deserialize)]
struct ImportConfigRequest {
    config: Config,
    commit_message: String,
    validate: Option<bool>,
}
```

## 测试用例

### 1. 测试：获取完整配置

```rust
#[tokio::test]
async fn test_get_config_success() {
    let handler = setup_test_handler().await;

    // 准备测试配置文件
    write_test_config().await;

    let response = handler
        .get_config(Query(GetConfigParams { format: None }))
        .await
        .unwrap();

    let config = response.0;
    assert_eq!(config.mise.tools.get("node"), Some(&"20.11.0".to_string()));
    assert!(config.metadata.commit.len() > 0);
}
```

### 2. 测试：配置验证失败

```rust
#[tokio::test]
async fn test_validate_config_port_conflict() {
    let handler = setup_test_handler().await;

    let mut services = HashMap::new();
    services.insert("web".to_string(), ServiceConfig {
        command: "node server.js".to_string(),
        ports: {
            let mut ports = HashMap::new();
            ports.insert("http".to_string(), 8080);
            ports
        },
        ..Default::default()
    });
    services.insert("api".to_string(), ServiceConfig {
        command: "node api.js".to_string(),
        ports: {
            let mut ports = HashMap::new();
            ports.insert("http".to_string(), 8080); // 冲突
            ports
        },
        ..Default::default()
    });

    let request = ValidateConfigRequest {
        mise: MiseConfig::default(),
        svcmgr: SvcmgrConfig {
            services,
            ..Default::default()
        },
    };

    let response = handler.validate_config(Json(request)).await.unwrap();
    let validation = response.0;

    assert!(!validation.valid);
    assert!(validation.errors.iter().any(|e| e.kind == ValidationErrorKind::PortConflict));
}
```

### 3. 测试：配置回滚

```rust
#[tokio::test]
async fn test_rollback_config() {
    let handler = setup_test_handler().await;

    // 1. 创建初始配置
    let initial_commit = create_initial_config(&handler).await;

    // 2. 更新配置
    update_config(&handler, "node", "22.0.0").await;

    // 3. 回滚到初始版本
    let rollback_request = RollbackConfigRequest {
        commit: initial_commit.clone(),
        reason: "Revert Node.js upgrade".to_string(),
    };

    let response = handler.rollback_config(Json(rollback_request)).await.unwrap();
    let rollback = response.0;

    assert_eq!(rollback.commit, initial_commit);
    assert!(rollback.rollback_commit.len() > 0);

    // 4. 验证配置已回滚
    let config = handler.get_config(Query(GetConfigParams { format: None })).await.unwrap().0;
    assert_eq!(config.mise.tools.get("node"), Some(&"20.11.0".to_string()));
}
```

### 4. 测试：部分更新段落

```rust
#[tokio::test]
async fn test_patch_config_section() {
    let handler = setup_test_handler().await;

    // 1. 准备初始配置
    create_initial_config(&handler).await;

    // 2. 更新 tools 段落（添加 go）
    let patch_request = PatchConfigSectionRequest {
        data: serde_json::json!({
            "go": "1.22.0"
        }),
        commit_message: "Add Go 1.22.0".to_string(),
    };

    let response = handler
        .patch_config_section(Path("tools".to_string()), Json(patch_request))
        .await
        .unwrap();

    let update = response.0;
    assert!(update.files_changed.contains(&handler.mise_config_path));

    // 3. 验证更新成功
    let config = handler.get_config(Query(GetConfigParams { format: None })).await.unwrap().0;
    assert_eq!(config.mise.tools.get("go"), Some(&"1.22.0".to_string()));
    assert_eq!(config.mise.tools.get("node"), Some(&"20.11.0".to_string())); // 原有工具保持不变
}
```

### 5. 测试：配置导出/导入

```rust
#[tokio::test]
async fn test_export_import_config() {
    let handler = setup_test_handler().await;

    // 1. 创建初始配置
    create_initial_config(&handler).await;

    // 2. 导出配置
    let export_response = handler
        .export_config(Query(ExportConfigParams { format: None }))
        .await
        .unwrap();
    let export = export_response.0;

    // 3. 修改配置
    update_config(&handler, "node", "22.0.0").await;

    // 4. 导入配置（恢复）
    let import_request = ImportConfigRequest {
        config: export.config,
        commit_message: "Restore from backup".to_string(),
        validate: Some(true),
    };

    let import_response = handler.import_config(Json(import_request)).await.unwrap();
    assert!(import_response.0.files_changed.len() > 0);

    // 5. 验证配置已恢复
    let config = handler.get_config(Query(GetConfigParams { format: None })).await.unwrap().0;
    assert_eq!(config.mise.tools.get("node"), Some(&"20.11.0".to_string()));
}
```

## 事件集成

### ConfigChanged 事件

```rust
/// 配置变更事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigChanged {
    /// Git commit hash
    pub commit: String,
    
    /// 变更的文件列表
    pub files: Vec<String>,
    
    /// 变更的段落列表（可选）
    pub sections: Option<Vec<String>>,
}
```

**事件流**：
```
配置更新 → Git commit → ConfigChanged 事件 → 调度引擎监听 → 重新加载配置 → 重启受影响的服务
```

**调度引擎处理**：
```rust
impl SchedulerEngine {
    async fn handle_config_changed(&self, event: ConfigChanged) {
        // 1. 重新加载配置
        let new_config = self.load_config().await?;
        
        // 2. 对比新旧配置，找出变更的服务
        let changed_services = self.diff_services(&self.current_config, &new_config);
        
        // 3. 重启受影响的服务
        for service_name in changed_services {
            self.restart_service(&service_name).await?;
        }
        
        // 4. 更新当前配置
        self.current_config = new_config;
    }
}
```

### ConfigRolledBack 事件

```rust
/// 配置回滚事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigRolledBack {
    /// 回滚到的原始 commit
    pub original_commit: String,
    
    /// 回滚操作产生的新 commit
    pub rollback_commit: String,
    
    /// 回滚原因
    pub reason: String,
}
```

**事件流**：
```
配置回滚 → Git commit → ConfigRolledBack 事件 → 调度引擎监听 → 重新加载配置 → 重启所有服务
```

## 错误处理

### 配置管理错误

```rust
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Configuration parse error: {0}")]
    ConfigParseError(String),

    #[error("Configuration section not found: {0}")]
    ConfigSectionNotFound(String),

    #[error("Configuration validation failed")]
    ConfigValidationFailed(Vec<ValidationError>),

    #[error("Commit not found: {0}")]
    CommitNotFound(String),

    #[error("Git operation failed: {0}")]
    GitError(String),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("TOML serialization error: {0}")]
    TomlError(#[from] toml::ser::Error),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message, details) = match self {
            ApiError::ConfigParseError(msg) => (
                StatusCode::BAD_REQUEST,
                "CONFIG_PARSE_ERROR",
                msg,
                None,
            ),
            ApiError::ConfigSectionNotFound(section) => (
                StatusCode::NOT_FOUND,
                "CONFIG_SECTION_NOT_FOUND",
                format!("Configuration section '{}' not found", section),
                Some(serde_json::json!({
                    "section": section,
                    "valid_sections": ["tools", "env", "tasks", "services", "scheduled_tasks", "features", "http"]
                })),
            ),
            ApiError::ConfigValidationFailed(errors) => (
                StatusCode::BAD_REQUEST,
                "CONFIG_VALIDATION_FAILED",
                "Configuration validation failed".to_string(),
                Some(serde_json::json!({ "errors": errors })),
            ),
            ApiError::CommitNotFound(commit) => (
                StatusCode::NOT_FOUND,
                "COMMIT_NOT_FOUND",
                format!("Commit '{}' not found", commit),
                Some(serde_json::json!({ "commit": commit })),
            ),
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                self.to_string(),
                None,
            ),
        };

        let body = serde_json::json!({
            "error": {
                "code": code,
                "message": message,
                "details": details,
                "request_id": format!("req_{}", uuid::Uuid::new_v4())
            }
        });

        (status, Json(body)).into_response()
    }
}
```

## 安全性考虑

### 1. 配置访问控制

- **本地模式**：Unix Socket 连接，进程 UID 匹配验证
- **网络模式**：Bearer Token 认证，仅允许配置管理权限的 Token

### 2. 配置验证强制

- 所有配置更新必须通过验证（语法 + 语义）
- 验证失败时拒绝更新，返回详细错误信息
- 支持 dry-run 验证（不实际应用）

### 3. Git 历史保护

- 配置回滚不删除历史（使用 `git checkout` 而非 `git reset --hard`）
- 所有配置变更都有完整的 Git 历史记录
- 支持审计日志（通过 Git log 实现）

### 4. 并发控制

- 配置更新使用分布式锁（防止并发修改冲突）
- Git 提交失败时自动重试（处理并发提交冲突）

## 性能优化

### 1. 配置缓存

```rust
pub struct ConfigCache {
    cache: Arc<RwLock<Option<(Config, Instant)>>>,
    ttl: Duration,
}

impl ConfigCache {
    pub async fn get_or_load(&self) -> Result<Config, ApiError> {
        let cache = self.cache.read().await;
        if let Some((config, loaded_at)) = &*cache {
            if loaded_at.elapsed() < self.ttl {
                return Ok(config.clone());
            }
        }
        drop(cache);

        // 缓存过期，重新加载
        let config = self.load_config().await?;
        let mut cache = self.cache.write().await;
        *cache = Some((config.clone(), Instant::now()));
        Ok(config)
    }
}
```

### 2. 增量验证

- 部分更新时仅验证受影响的段落
- 避免重新验证整个配置文件

### 3. 异步 Git 操作

- 所有 Git 操作使用 `tokio::task::spawn_blocking`
- 避免阻塞异步运行时

## 相关规范

- **00-architecture-overview.md** - 整体架构
- **01-config-design.md** - 配置文件设计（mise.toml + svcmgr.toml 分离）
- **04-git-versioning.md** - Git 版本管理机制
- **07-mise-integration.md** - mise 集成层（ConfigPort）
- **10-api-overview.md** - API 设计总览
- **11-api-services.md** - 服务管理 API（依赖配置段 [services.*]）
- **12-api-tasks.md** - 任务管理 API（依赖配置段 [tasks.*]）
- **13-api-tools.md** - 工具管理 API（依赖配置段 [tools]）

## 未来扩展

### 1. 配置模板系统

支持配置模板和变量替换：

```toml
# .config/mise/svcmgr/config.toml
[services.web]
command = "mise run server"
env.DATABASE_URL = "{{ database.url }}"
env.API_KEY = "{{ secrets.api_key }}"
```

### 2. 配置继承

支持配置文件继承（通过 `extends` 字段）：

```toml
# .config/mise/svcmgr/conf.d/production.toml
extends = ["base.toml"]

[services.web.env]
NODE_ENV = "production"
```

### 3. 配置 Schema 验证

使用 JSON Schema 或 TOML Schema 进行更严格的配置验证。

### 4. 配置加密

支持敏感配置加密存储（如 API 密钥、数据库密码）：

```toml
[services.web.env]
DATABASE_PASSWORD = "enc:AES256:base64encodedvalue"
```
