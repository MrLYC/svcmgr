# 13 - 工具管理 API

> 版本：2.0.0-draft  
> 状态：设计中

## 设计目标

工具管理 API 提供以下能力：

1. **工具安装/卸载**：管理 mise 工具（如 Node.js、Python、Go）的生命周期
2. **版本管理**：列出可用版本、安装特定版本、切换当前版本
3. **工具查询**：列出已安装工具、查看工具详情、检查更新
4. **插件管理**：添加/删除 mise 插件（扩展工具支持）

## 为什么需要工具管理 API？

### mise 作为依赖管理器

mise 是一个统一的开发工具版本管理器，替代 nvm、pyenv、rbenv、gvm 等工具。它管理：

- **语言运行时**：Node.js、Python、Ruby、Go、Rust、Java 等
- **CLI 工具**：terraform、kubectl、helm、awscli、gh 等
- **自定义工具**：通过插件系统扩展

**配置驱动示例**：

```toml
# .config/mise/config.toml
[tools]
node = "20.11.0"
python = "3.12.1"
terraform = "1.7.0"

[env]
NODE_ENV = "production"
```

### 与任务/服务的关系

- **任务依赖工具**：`[tasks.deploy]` 可能需要 `node` 和 `terraform`
- **服务依赖工具**：`services.web.command = "mise run server"` 需要 `node` 已安装
- **环境隔离**：不同项目可使用不同工具版本，mise 自动切换

## API 端点概览

| HTTP 方法 | 路径 | 用途 |
|-----------|------|------|
| **工具管理** |
| GET | `/api/v1/tools` | 列出所有已安装工具 |
| GET | `/api/v1/tools/{name}` | 获取工具详情（当前版本、可用版本） |
| POST | `/api/v1/tools` | 安装工具（指定版本） |
| DELETE | `/api/v1/tools/{name}` | 卸载工具（所有版本或指定版本） |
| PUT | `/api/v1/tools/{name}` | 更新工具到最新版本或切换版本 |
| GET | `/api/v1/tools/{name}/versions` | 列出工具的所有可用版本 |
| POST | `/api/v1/tools/{name}/use` | 设置当前目录使用的版本 |
| **插件管理** |
| GET | `/api/v1/plugins` | 列出所有已安装插件 |
| POST | `/api/v1/plugins` | 添加插件 |
| DELETE | `/api/v1/plugins/{name}` | 删除插件 |
| PUT | `/api/v1/plugins/{name}` | 更新插件 |

---

## 数据模型

### Tool（工具信息）

```rust
/// 工具信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    /// 工具名称（如 "node", "python"）
    pub name: String,
    
    /// 工具类型
    pub tool_type: ToolType,
    
    /// 当前激活的版本（在当前工作目录）
    pub active_version: Option<String>,
    
    /// 已安装的版本列表
    pub installed_versions: Vec<InstalledVersion>,
    
    /// 配置来源（哪个配置文件定义了此工具）
    pub source: Option<PathBuf>,
    
    /// 工具描述（来自插件元数据）
    pub description: Option<String>,
    
    /// 插件名称（如果是通过插件安装）
    pub plugin: Option<String>,
}

/// 工具类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ToolType {
    /// 核心工具（mise 内置支持）
    Core,
    /// 插件工具（通过插件安装）
    Plugin,
}

/// 已安装版本
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledVersion {
    /// 版本号
    pub version: String,
    
    /// 安装路径
    pub install_path: PathBuf,
    
    /// 是否是当前激活版本
    pub active: bool,
    
    /// 安装时间
    pub installed_at: DateTime<Utc>,
    
    /// 安装大小（字节）
    pub size: Option<u64>,
}
```

### AvailableVersion（可用版本）

```rust
/// 可用版本（远程可安装）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableVersion {
    /// 版本号
    pub version: String,
    
    /// 版本类型
    pub version_type: VersionType,
    
    /// 是否已安装
    pub installed: bool,
    
    /// 发布日期
    pub released_at: Option<DateTime<Utc>>,
    
    /// 版本描述/变更日志
    pub description: Option<String>,
}

/// 版本类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VersionType {
    /// LTS（长期支持版本）
    Lts,
    /// Stable（稳定版本）
    Stable,
    /// Latest（最新版本）
    Latest,
    /// Beta（测试版本）
    Beta,
    /// Dev（开发版本）
    Dev,
}
```

### Plugin（插件信息）

```rust
/// 插件信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plugin {
    /// 插件名称（如 "nodejs", "terraform"）
    pub name: String,
    
    /// 插件 URL（Git 仓库）
    pub url: String,
    
    /// 插件版本（Git ref）
    pub version: Option<String>,
    
    /// 是否启用
    pub enabled: bool,
    
    /// 插件描述
    pub description: Option<String>,
    
    /// 支持的工具列表
    pub tools: Vec<String>,
    
    /// 最后更新时间
    pub updated_at: Option<DateTime<Utc>>,
}
```

---

## API 详细设计

### 1. 列出所有已安装工具

**请求**:
```http
GET /api/v1/tools?type=all HTTP/1.1
```

**查询参数**:
- `type` (可选): 工具类型过滤
  - `all` (默认): 所有工具
  - `core`: 仅核心工具
  - `plugin`: 仅插件工具

**响应**:
```json
{
  "data": {
    "tools": [
      {
        "name": "node",
        "tool_type": "core",
        "active_version": "20.11.0",
        "installed_versions": [
          {
            "version": "20.11.0",
            "install_path": "/home/user/.local/share/mise/installs/node/20.11.0",
            "active": true,
            "installed_at": "2026-02-20T10:30:00Z",
            "size": 52428800
          },
          {
            "version": "18.19.0",
            "install_path": "/home/user/.local/share/mise/installs/node/18.19.0",
            "active": false,
            "installed_at": "2026-01-15T08:20:00Z",
            "size": 48234496
          }
        ],
        "source": "/home/user/.config/mise/config.toml",
        "description": "Node.js runtime",
        "plugin": null
      },
      {
        "name": "python",
        "tool_type": "core",
        "active_version": "3.12.1",
        "installed_versions": [
          {
            "version": "3.12.1",
            "install_path": "/home/user/.local/share/mise/installs/python/3.12.1",
            "active": true,
            "installed_at": "2026-02-18T14:15:00Z",
            "size": 67108864
          }
        ],
        "source": "/home/user/.config/mise/config.toml",
        "description": "Python runtime",
        "plugin": null
      },
      {
        "name": "terraform",
        "tool_type": "plugin",
        "active_version": "1.7.0",
        "installed_versions": [
          {
            "version": "1.7.0",
            "install_path": "/home/user/.local/share/mise/installs/terraform/1.7.0",
            "active": true,
            "installed_at": "2026-02-22T09:00:00Z",
            "size": 104857600
          }
        ],
        "source": "/home/user/.config/mise/config.toml",
        "description": "Infrastructure as Code tool",
        "plugin": "hashicorp/terraform"
      }
    ]
  },
  "pagination": {
    "total": 3,
    "page": 1,
    "per_page": 20
  }
}
```

**说明**:
- `active_version`: 当前工作目录生效的版本（根据 `.config/mise/config.toml`）
- `installed_versions`: 所有已安装版本（可能有多个）
- `source`: 工具版本定义在哪个配置文件

---

### 2. 获取工具详情

**请求**:
```http
GET /api/v1/tools/node HTTP/1.1
```

**响应**:
```json
{
  "data": {
    "name": "node",
    "tool_type": "core",
    "active_version": "20.11.0",
    "installed_versions": [
      {
        "version": "20.11.0",
        "install_path": "/home/user/.local/share/mise/installs/node/20.11.0",
        "active": true,
        "installed_at": "2026-02-20T10:30:00Z",
        "size": 52428800
      },
      {
        "version": "18.19.0",
        "install_path": "/home/user/.local/share/mise/installs/node/18.19.0",
        "active": false,
        "installed_at": "2026-01-15T08:20:00Z",
        "size": 48234496
      }
    ],
    "source": "/home/user/.config/mise/config.toml",
    "description": "Node.js runtime",
    "plugin": null,
    "latest_available": "20.11.1",
    "update_available": true,
    "used_by_services": ["web", "api"],
    "used_by_tasks": ["build", "test", "deploy"]
  }
}
```

**说明**:
- `latest_available`: 最新可用版本（从远程查询）
- `update_available`: 是否有更新版本
- `used_by_services`: 哪些服务依赖此工具
- `used_by_tasks`: 哪些任务依赖此工具

**错误响应**:
```json
{
  "error": {
    "code": "TOOL_NOT_FOUND",
    "message": "Tool 'golang' not found",
    "details": {
      "tool_name": "golang",
      "suggestion": "Use 'go' instead, or install plugin first"
    },
    "request_id": "req_abc123"
  }
}
```

---

### 3. 安装工具

**请求**:
```http
POST /api/v1/tools HTTP/1.1
Content-Type: application/json
Idempotency-Key: idempotency_abc123

{
  "name": "node",
  "version": "20.11.0",
  "set_active": true
}
```

**请求体**:
- `name` (必填): 工具名称
- `version` (必填): 版本号（支持通配符 `latest`, `lts`, `20.x`）
- `set_active` (可选): 是否设置为当前激活版本（默认 true）

**响应（异步安装）**:
```json
{
  "data": {
    "operation_id": "op_install_node_20110",
    "tool_name": "node",
    "version": "20.11.0",
    "status": "installing",
    "started_at": "2026-02-23T11:00:00Z",
    "estimated_duration": 120
  }
}
```

**说明**:
- 工具安装是长时间运行操作，立即返回 `operation_id`
- 客户端通过 `GET /api/v1/operations/{operation_id}` 轮询状态
- 安装完成后，自动更新 `.config/mise/config.toml` 的 `[tools]` 段

**错误响应**:
```json
{
  "error": {
    "code": "TOOL_ALREADY_INSTALLED",
    "message": "Tool 'node@20.11.0' is already installed",
    "details": {
      "tool_name": "node",
      "version": "20.11.0",
      "install_path": "/home/user/.local/share/mise/installs/node/20.11.0"
    },
    "request_id": "req_abc124"
  }
}
```

```json
{
  "error": {
    "code": "INVALID_VERSION",
    "message": "Version '999.0.0' not available for tool 'node'",
    "details": {
      "tool_name": "node",
      "requested_version": "999.0.0",
      "available_versions": ["20.11.1", "20.11.0", "18.19.0"]
    },
    "request_id": "req_abc125"
  }
}
```

**副作用**:
1. 调用 `mise install {name}@{version}`（通过 DependencyPort）
2. 如果 `set_active=true`，更新 `.config/mise/config.toml` 的 `[tools.{name}]`
3. Git 自动暂存配置变更
4. 触发 `tool_installed` 事件

---

### 4. 卸载工具

**请求（卸载特定版本）**:
```http
DELETE /api/v1/tools/node?version=18.19.0 HTTP/1.1
```

**请求（卸载所有版本）**:
```http
DELETE /api/v1/tools/node?all=true HTTP/1.1
```

**查询参数**:
- `version` (可选): 指定版本（默认卸载所有版本）
- `all` (可选): 明确卸载所有版本（`true` 或 `false`）

**响应**:
```http
HTTP/1.1 204 No Content
```

**错误响应**:
```json
{
  "error": {
    "code": "TOOL_IN_USE",
    "message": "Cannot uninstall 'node@20.11.0', it is used by active services",
    "details": {
      "tool_name": "node",
      "version": "20.11.0",
      "used_by_services": ["web", "api"],
      "used_by_tasks": ["build", "test"]
    },
    "request_id": "req_abc126"
  }
}
```

**副作用**:
1. 调用 `mise uninstall {name}@{version}` 或 `mise uninstall {name}`
2. 如果卸载的是 `active_version`，从 `.config/mise/config.toml` 删除 `[tools.{name}]`
3. Git 自动暂存配置变更
4. 触发 `tool_uninstalled` 事件

---

### 5. 更新工具

**请求（更新到最新版本）**:
```http
PUT /api/v1/tools/node HTTP/1.1
Content-Type: application/json

{
  "version": "latest"
}
```

**请求（切换到已安装版本）**:
```http
PUT /api/v1/tools/node HTTP/1.1
Content-Type: application/json

{
  "version": "18.19.0"
}
```

**请求体**:
- `version` (必填): 目标版本号
  - `latest`: 更新到最新稳定版
  - `lts`: 更新到 LTS 版本（如果支持）
  - 具体版本号: 切换到已安装版本，或安装新版本

**响应（如果需要安装新版本）**:
```json
{
  "data": {
    "operation_id": "op_update_node_20111",
    "tool_name": "node",
    "from_version": "20.11.0",
    "to_version": "20.11.1",
    "status": "installing",
    "started_at": "2026-02-23T11:10:00Z"
  }
}
```

**响应（如果只是切换版本）**:
```json
{
  "data": {
    "tool_name": "node",
    "from_version": "20.11.0",
    "to_version": "18.19.0",
    "status": "switched"
  }
}
```

**副作用**:
1. 如果目标版本未安装，先安装（同 `POST /api/v1/tools`）
2. 更新 `.config/mise/config.toml` 的 `[tools.{name}]` 为新版本
3. Git 自动暂存配置变更
4. 触发 `tool_updated` 事件

---

### 6. 列出工具的所有可用版本

**请求**:
```http
GET /api/v1/tools/node/versions?limit=20&filter=stable HTTP/1.1
```

**查询参数**:
- `limit` (可选): 返回版本数量（默认 50，最大 200）
- `filter` (可选): 版本类型过滤
  - `all` (默认): 所有版本
  - `stable`: 仅稳定版
  - `lts`: 仅 LTS 版本
  - `latest`: 仅最新版

**响应**:
```json
{
  "data": {
    "tool_name": "node",
    "versions": [
      {
        "version": "20.11.1",
        "version_type": "latest",
        "installed": false,
        "released_at": "2026-02-10T00:00:00Z",
        "description": "Latest stable release"
      },
      {
        "version": "20.11.0",
        "version_type": "lts",
        "installed": true,
        "released_at": "2026-01-25T00:00:00Z",
        "description": "Long-term support release"
      },
      {
        "version": "18.19.0",
        "version_type": "lts",
        "installed": true,
        "released_at": "2025-11-15T00:00:00Z",
        "description": "Maintenance LTS"
      },
      {
        "version": "21.0.0-beta.1",
        "version_type": "beta",
        "installed": false,
        "released_at": "2026-02-01T00:00:00Z",
        "description": "Beta release"
      }
    ]
  },
  "pagination": {
    "total": 157,
    "limit": 20,
    "offset": 0
  }
}
```

**说明**:
- 版本列表从 mise 插件获取（调用 `mise ls-remote {name}`）
- `installed` 字段标识本地是否已安装
- 版本按发布时间降序排列

---

### 7. 设置当前目录使用的版本

**请求**:
```http
POST /api/v1/tools/node/use HTTP/1.1
Content-Type: application/json

{
  "version": "18.19.0",
  "scope": "local"
}
```

**请求体**:
- `version` (必填): 目标版本号（必须已安装）
- `scope` (可选): 作用范围
  - `local` (默认): 当前工作目录（写入 `.config/mise/config.toml`）
  - `global`: 全局默认（写入 `~/.config/mise/config.toml`）

**响应**:
```json
{
  "data": {
    "tool_name": "node",
    "version": "18.19.0",
    "scope": "local",
    "config_file": "/home/user/.config/mise/config.toml"
  }
}
```

**错误响应**:
```json
{
  "error": {
    "code": "VERSION_NOT_INSTALLED",
    "message": "Version '16.20.0' is not installed for tool 'node'",
    "details": {
      "tool_name": "node",
      "requested_version": "16.20.0",
      "installed_versions": ["20.11.0", "18.19.0"]
    },
    "request_id": "req_abc127"
  }
}
```

**副作用**:
1. 更新配置文件 `[tools.{name}] = "{version}"`
2. Git 自动暂存配置变更
3. 触发 `tool_activated` 事件

---

### 8. 列出所有已安装插件

**请求**:
```http
GET /api/v1/plugins HTTP/1.1
```

**响应**:
```json
{
  "data": {
    "plugins": [
      {
        "name": "hashicorp/terraform",
        "url": "https://github.com/asdf-community/asdf-hashicorp.git",
        "version": "v1.2.0",
        "enabled": true,
        "description": "HashiCorp tools (Terraform, Vault, Consul)",
        "tools": ["terraform", "vault", "consul"],
        "updated_at": "2026-02-20T12:00:00Z"
      },
      {
        "name": "awscli",
        "url": "https://github.com/MetalBlueberry/asdf-awscli.git",
        "version": "main",
        "enabled": true,
        "description": "AWS Command Line Interface",
        "tools": ["awscli"],
        "updated_at": "2026-01-10T08:30:00Z"
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
- 插件信息从 `mise plugins ls` 获取
- `version`: Git ref（tag、branch、commit）

---

### 9. 添加插件

**请求**:
```http
POST /api/v1/plugins HTTP/1.1
Content-Type: application/json

{
  "name": "hashicorp/terraform",
  "url": "https://github.com/asdf-community/asdf-hashicorp.git",
  "version": "v1.2.0"
}
```

**请求体**:
- `name` (必填): 插件名称（短名称或完整名称）
- `url` (可选): 插件 Git 仓库 URL（如果是官方插件可省略）
- `version` (可选): Git ref（默认 `main`）

**响应**:
```json
{
  "data": {
    "name": "hashicorp/terraform",
    "url": "https://github.com/asdf-community/asdf-hashicorp.git",
    "version": "v1.2.0",
    "enabled": true,
    "description": "HashiCorp tools (Terraform, Vault, Consul)",
    "tools": ["terraform", "vault", "consul"],
    "updated_at": "2026-02-23T11:20:00Z"
  }
}
```

**错误响应**:
```json
{
  "error": {
    "code": "PLUGIN_ALREADY_INSTALLED",
    "message": "Plugin 'hashicorp/terraform' is already installed",
    "details": {
      "plugin_name": "hashicorp/terraform",
      "current_version": "v1.2.0"
    },
    "request_id": "req_abc128"
  }
}
```

**副作用**:
1. 调用 `mise plugins install {name} {url}`
2. 触发 `plugin_installed` 事件

---

### 10. 删除插件

**请求**:
```http
DELETE /api/v1/plugins/hashicorp%2Fterraform HTTP/1.1
```

**响应**:
```http
HTTP/1.1 204 No Content
```

**错误响应**:
```json
{
  "error": {
    "code": "PLUGIN_IN_USE",
    "message": "Cannot remove plugin 'hashicorp/terraform', tools are still installed",
    "details": {
      "plugin_name": "hashicorp/terraform",
      "installed_tools": ["terraform@1.7.0", "vault@1.15.0"]
    },
    "request_id": "req_abc129"
  }
}
```

**副作用**:
1. 检查是否有工具依赖此插件（如果有，拒绝删除）
2. 调用 `mise plugins uninstall {name}`
3. 触发 `plugin_uninstalled` 事件

---

### 11. 更新插件

**请求**:
```http
PUT /api/v1/plugins/hashicorp%2Fterraform HTTP/1.1
Content-Type: application/json

{
  "version": "v1.3.0"
}
```

**请求体**:
- `version` (可选): 目标 Git ref（默认更新到最新）

**响应**:
```json
{
  "data": {
    "name": "hashicorp/terraform",
    "from_version": "v1.2.0",
    "to_version": "v1.3.0",
    "updated_at": "2026-02-23T11:30:00Z"
  }
}
```

**副作用**:
1. 调用 `mise plugins update {name}`
2. 触发 `plugin_updated` 事件

---

## 批量操作

### 12. 批量安装工具

**请求**:
```http
POST /api/v1/tools/batch HTTP/1.1
Content-Type: application/json

{
  "tools": [
    {"name": "node", "version": "20.11.0"},
    {"name": "python", "version": "3.12.1"},
    {"name": "go", "version": "1.21.6"}
  ]
}
```

**响应**:
```json
{
  "data": {
    "operation_id": "op_batch_install_abc",
    "total": 3,
    "status": "installing",
    "started_at": "2026-02-23T11:40:00Z"
  }
}
```

---

### 13. 批量更新工具

**请求**:
```http
POST /api/v1/tools/update-all HTTP/1.1
Content-Type: application/json

{
  "filter": "outdated"
}
```

**请求体**:
- `filter` (可选): 更新范围
  - `all`: 所有工具
  - `outdated` (默认): 仅有更新的工具

**响应**:
```json
{
  "data": {
    "operation_id": "op_update_all_def",
    "tools_to_update": [
      {"name": "node", "from": "20.11.0", "to": "20.11.1"},
      {"name": "terraform", "from": "1.7.0", "to": "1.7.1"}
    ],
    "status": "installing",
    "started_at": "2026-02-23T11:50:00Z"
  }
}
```

---

## 错误码清单

| 错误码 | HTTP 状态 | 说明 |
|--------|-----------|------|
| `TOOL_NOT_FOUND` | 404 | 工具不存在 |
| `TOOL_ALREADY_INSTALLED` | 409 | 工具已安装 |
| `TOOL_IN_USE` | 409 | 工具正在被使用（无法卸载） |
| `VERSION_NOT_INSTALLED` | 400 | 版本未安装（无法切换） |
| `INVALID_VERSION` | 400 | 版本号不存在或无效 |
| `PLUGIN_NOT_FOUND` | 404 | 插件不存在 |
| `PLUGIN_ALREADY_INSTALLED` | 409 | 插件已安装 |
| `PLUGIN_IN_USE` | 409 | 插件正在被使用（无法删除） |
| `INSTALLATION_FAILED` | 500 | 安装失败（mise 错误） |
| `UNINSTALLATION_FAILED` | 500 | 卸载失败（mise 错误） |

---

## Handler 实现示例

### 安装工具 Handler

```rust
use axum::{extract::{Path, State}, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct InstallToolRequest {
    pub name: String,
    pub version: String,
    #[serde(default = "default_true")]
    pub set_active: bool,
}

#[derive(Debug, Serialize)]
pub struct InstallToolResponse {
    pub operation_id: String,
    pub tool_name: String,
    pub version: String,
    pub status: String,
    pub started_at: DateTime<Utc>,
    pub estimated_duration: u64,
}

/// POST /api/v1/tools
pub async fn install_tool(
    State(app): State<Arc<AppState>>,
    Json(req): Json<InstallToolRequest>,
) -> Result<Json<ApiResponse<InstallToolResponse>>, ApiError> {
    // 1. 验证工具名称
    validate_tool_name(&req.name)?;
    
    // 2. 解析版本号（支持 latest/lts）
    let resolved_version = app.mise_adapter.resolve_version(&req.name, &req.version).await
        .map_err(|_| ApiError::bad_request("INVALID_VERSION",
            format!("Version '{}' not available for tool '{}'", req.version, req.name)))?;
    
    // 3. 检查是否已安装
    if app.mise_adapter.is_tool_installed(&req.name, &resolved_version).await? {
        return Err(ApiError::conflict("TOOL_ALREADY_INSTALLED",
            format!("Tool '{}@{}' is already installed", req.name, resolved_version)));
    }
    
    // 4. 生成操作 ID
    let operation_id = format!("op_install_{}_{}", 
        req.name.replace("/", "_"), 
        Uuid::new_v4().simple().to_string().chars().take(8).collect::<String>());
    
    // 5. 创建后台安装任务
    let app_clone = app.clone();
    let tool_name = req.name.clone();
    let version = resolved_version.clone();
    let set_active = req.set_active;
    
    tokio::spawn(async move {
        // 调用 mise install
        let result = app_clone.mise_adapter
            .install(&tool_name, &version)
            .await;
        
        match result {
            Ok(_) => {
                // 如果 set_active=true，更新配置文件
                if set_active {
                    let _ = app_clone.config_manager
                        .set_tool_version(&tool_name, &version)
                        .await;
                    
                    // Git 自动暂存
                    let _ = app_clone.git_atom
                        .stage_file(".config/mise/config.toml")
                        .await;
                }
                
                // 触发事件
                app_clone.event_bus.publish(ToolEvent::Installed {
                    tool_name: tool_name.clone(),
                    version: version.clone(),
                });
                
                // 更新操作状态
                app_clone.operation_tracker.complete(&operation_id).await;
            }
            Err(e) => {
                app_clone.operation_tracker.fail(&operation_id, e.to_string()).await;
            }
        }
    });
    
    // 6. 立即返回操作 ID
    Ok(Json(ApiResponse::success(InstallToolResponse {
        operation_id,
        tool_name: req.name,
        version: resolved_version,
        status: "installing".to_string(),
        started_at: Utc::now(),
        estimated_duration: 120, // 估计 2 分钟
    })))
}

/// 验证工具名称
fn validate_tool_name(name: &str) -> Result<(), ApiError> {
    if name.is_empty() || name.len() > 64 {
        return Err(ApiError::bad_request("INVALID_TOOL_NAME",
            "Tool name must be 1-64 characters"));
    }
    
    // 允许字母、数字、下划线、斜杠（插件名）
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '/' || c == '-') {
        return Err(ApiError::bad_request("INVALID_TOOL_NAME",
            "Tool name can only contain letters, numbers, underscores, hyphens, and slashes"));
    }
    
    Ok(())
}

fn default_true() -> bool { true }
```

### 卸载工具 Handler

```rust
#[derive(Debug, Deserialize)]
pub struct UninstallToolQuery {
    pub version: Option<String>,
    #[serde(default)]
    pub all: bool,
}

/// DELETE /api/v1/tools/{name}
pub async fn uninstall_tool(
    State(app): State<Arc<AppState>>,
    Path(tool_name): Path<String>,
    Query(query): Query<UninstallToolQuery>,
) -> Result<StatusCode, ApiError> {
    // 1. 检查工具是否存在
    let tool = app.mise_adapter.get_tool_info(&tool_name).await
        .map_err(|_| ApiError::not_found("TOOL_NOT_FOUND",
            format!("Tool '{}' not found", tool_name)))?;
    
    // 2. 确定要卸载的版本
    let versions_to_uninstall = if query.all {
        tool.installed_versions.iter().map(|v| v.version.clone()).collect::<Vec<_>>()
    } else if let Some(version) = &query.version {
        vec![version.clone()]
    } else {
        // 默认卸载所有版本
        tool.installed_versions.iter().map(|v| v.version.clone()).collect::<Vec<_>>()
    };
    
    // 3. 检查工具是否正在使用
    for version in &versions_to_uninstall {
        let usage = app.dependency_tracker.get_tool_usage(&tool_name, version).await?;
        
        if !usage.services.is_empty() || !usage.tasks.is_empty() {
            return Err(ApiError::conflict("TOOL_IN_USE",
                format!("Cannot uninstall '{}@{}', it is used by active services or tasks", 
                    tool_name, version))
                .with_detail("used_by_services", usage.services)
                .with_detail("used_by_tasks", usage.tasks));
        }
    }
    
    // 4. 卸载工具
    for version in &versions_to_uninstall {
        app.mise_adapter.uninstall(&tool_name, version).await
            .map_err(|e| ApiError::internal("UNINSTALLATION_FAILED",
                format!("Failed to uninstall '{}@{}': {}", tool_name, version, e)))?;
    }
    
    // 5. 如果卸载的是激活版本，从配置文件删除
    if let Some(active_version) = &tool.active_version {
        if versions_to_uninstall.contains(active_version) {
            app.config_manager.remove_tool(&tool_name).await?;
            app.git_atom.stage_file(".config/mise/config.toml").await?;
        }
    }
    
    // 6. 触发事件
    for version in &versions_to_uninstall {
        app.event_bus.publish(ToolEvent::Uninstalled {
            tool_name: tool_name.clone(),
            version: version.clone(),
        });
    }
    
    Ok(StatusCode::NO_CONTENT)
}
```

### 列出可用版本 Handler

```rust
#[derive(Debug, Deserialize)]
pub struct ListVersionsQuery {
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub filter: VersionFilter,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VersionFilter {
    All,
    Stable,
    Lts,
    Latest,
}

impl Default for VersionFilter {
    fn default() -> Self {
        Self::All
    }
}

fn default_limit() -> usize { 50 }

/// GET /api/v1/tools/{name}/versions
pub async fn list_tool_versions(
    State(app): State<Arc<AppState>>,
    Path(tool_name): Path<String>,
    Query(query): Query<ListVersionsQuery>,
) -> Result<Json<ApiResponse<ListVersionsResponse>>, ApiError> {
    // 1. 调用 mise ls-remote
    let all_versions = app.mise_adapter.list_remote_versions(&tool_name).await
        .map_err(|_| ApiError::not_found("TOOL_NOT_FOUND",
            format!("Tool '{}' not found or has no available versions", tool_name)))?;
    
    // 2. 过滤版本
    let filtered_versions: Vec<_> = all_versions.into_iter()
        .filter(|v| match query.filter {
            VersionFilter::All => true,
            VersionFilter::Stable => v.version_type == VersionType::Stable || v.version_type == VersionType::Lts,
            VersionFilter::Lts => v.version_type == VersionType::Lts,
            VersionFilter::Latest => v.version_type == VersionType::Latest,
        })
        .take(query.limit)
        .collect();
    
    // 3. 标记已安装版本
    let installed_versions = app.mise_adapter.list_installed_versions(&tool_name).await?;
    let installed_set: HashSet<_> = installed_versions.iter()
        .map(|v| v.version.clone())
        .collect();
    
    let versions_with_status: Vec<_> = filtered_versions.into_iter()
        .map(|mut v| {
            v.installed = installed_set.contains(&v.version);
            v
        })
        .collect();
    
    Ok(Json(ApiResponse::success(ListVersionsResponse {
        tool_name,
        versions: versions_with_status,
    })))
}

#[derive(Debug, Serialize)]
pub struct ListVersionsResponse {
    pub tool_name: String,
    pub versions: Vec<AvailableVersion>,
}
```

---

## 测试用例

### 1. 安装工具（成功）

```rust
#[tokio::test]
async fn test_install_tool_success() {
    let app = setup_test_app().await;
    
    let req = InstallToolRequest {
        name: "node".to_string(),
        version: "20.11.0".to_string(),
        set_active: true,
    };
    
    let res = install_tool(
        State(app.clone()),
        Json(req),
    ).await.unwrap();
    
    // 验证返回操作 ID
    assert!(res.0.data.operation_id.starts_with("op_install_node_"));
    assert_eq!(res.0.data.status, "installing");
    
    // 等待安装完成
    let operation_id = res.0.data.operation_id;
    tokio::time::timeout(
        Duration::from_secs(180),
        app.operation_tracker.wait_completion(&operation_id)
    ).await.unwrap().unwrap();
    
    // 验证工具已安装
    let tool = app.mise_adapter.get_tool_info("node").await.unwrap();
    assert!(tool.installed_versions.iter().any(|v| v.version == "20.11.0"));
    
    // 验证配置文件已更新
    let config = app.config_manager.load_mise_config().await.unwrap();
    assert_eq!(config.tools.get("node"), Some(&"20.11.0".to_string()));
}
```

### 2. 卸载正在使用的工具（失败）

```rust
#[tokio::test]
async fn test_uninstall_tool_in_use() {
    let app = setup_test_app().await;
    
    // 安装工具
    app.mise_adapter.install("node", "20.11.0").await.unwrap();
    
    // 创建使用此工具的服务
    app.config_manager.add_service(&ServiceDefinition {
        name: "web".to_string(),
        command: "node server.js".to_string(),
        ..Default::default()
    }).await.unwrap();
    
    // 尝试卸载
    let res = uninstall_tool(
        State(app.clone()),
        Path("node".to_string()),
        Query(UninstallToolQuery {
            version: Some("20.11.0".to_string()),
            all: false,
        }),
    ).await;
    
    // 验证错误
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert_eq!(err.code, "TOOL_IN_USE");
}
```

### 3. 列出可用版本（LTS 过滤）

```rust
#[tokio::test]
async fn test_list_tool_versions_lts() {
    let app = setup_test_app().await;
    
    let res = list_tool_versions(
        State(app.clone()),
        Path("node".to_string()),
        Query(ListVersionsQuery {
            limit: 10,
            filter: VersionFilter::Lts,
        }),
    ).await.unwrap();
    
    // 验证所有返回版本都是 LTS
    for version in &res.0.data.versions {
        assert_eq!(version.version_type, VersionType::Lts);
    }
    
    // 验证版本数量不超过 limit
    assert!(res.0.data.versions.len() <= 10);
}
```

---

## 配置文件交互

### mise 配置文件（读写）

```toml
# .config/mise/config.toml
[tools]
node = "20.11.0"
python = "3.12.1"
terraform = "1.7.0"

[env]
NODE_ENV = "production"
PYTHON_PATH = "/usr/local/lib/python3.12"

[tasks.build]
run = "npm run build"
env = { NODE_ENV = "production" }
```

**svcmgr 行为**：
- 读取 `[tools]` 段获取已配置工具
- 通过 API 安装/卸载/更新工具时，修改 `[tools]` 段
- 自动 Git 暂存变更

---

## 与 mise 集成

### DependencyPort 接口

```rust
/// 依赖管理端口（mise 工具管理）
#[async_trait]
pub trait DependencyPort: Send + Sync {
    /// 安装工具
    async fn install(&self, tool: &str, version: &str) -> Result<()>;
    
    /// 卸载工具
    async fn uninstall(&self, tool: &str, version: &str) -> Result<()>;
    
    /// 列出已安装工具
    async fn list_installed(&self) -> Result<Vec<Tool>>;
    
    /// 获取工具信息
    async fn get_tool_info(&self, name: &str) -> Result<Tool>;
    
    /// 列出远程可用版本
    async fn list_remote_versions(&self, tool: &str) -> Result<Vec<AvailableVersion>>;
    
    /// 解析版本号（latest/lts → 具体版本）
    async fn resolve_version(&self, tool: &str, version: &str) -> Result<String>;
    
    /// 检查工具是否已安装
    async fn is_tool_installed(&self, tool: &str, version: &str) -> Result<bool>;
    
    /// 设置当前目录使用的版本
    async fn use_tool(&self, tool: &str, version: &str) -> Result<()>;
    
    /// 获取 mise 版本
    fn mise_version(&self) -> &MiseVersion;
}
```

### 事件系统

```rust
/// 工具事件
pub enum ToolEvent {
    /// 工具已安装
    Installed {
        tool_name: String,
        version: String,
    },
    
    /// 工具已卸载
    Uninstalled {
        tool_name: String,
        version: String,
    },
    
    /// 工具已更新
    Updated {
        tool_name: String,
        from_version: String,
        to_version: String,
    },
    
    /// 工具已激活（设置为当前版本）
    Activated {
        tool_name: String,
        version: String,
    },
}

// 订阅工具事件
app.event_bus.subscribe("tool_events", |event: ToolEvent| {
    match event {
        ToolEvent::Installed { tool_name, version } => {
            tracing::info!("Tool installed: {}@{}", tool_name, version);
        }
        _ => {}
    }
});
```

---

## 长时间运行操作

工具安装/更新是长时间运行操作，采用 202 Accepted + operation_id 模式：

### 操作状态查询

```http
GET /api/v1/operations/op_install_node_abc123 HTTP/1.1
```

**响应（运行中）**:
```json
{
  "data": {
    "operation_id": "op_install_node_abc123",
    "type": "tool_install",
    "status": "running",
    "progress": 45,
    "started_at": "2026-02-23T11:00:00Z",
    "estimated_completion": "2026-02-23T11:02:00Z",
    "details": {
      "tool_name": "node",
      "version": "20.11.0",
      "current_step": "Downloading binaries"
    }
  }
}
```

**响应（完成）**:
```json
{
  "data": {
    "operation_id": "op_install_node_abc123",
    "type": "tool_install",
    "status": "completed",
    "progress": 100,
    "started_at": "2026-02-23T11:00:00Z",
    "completed_at": "2026-02-23T11:01:45Z",
    "details": {
      "tool_name": "node",
      "version": "20.11.0",
      "install_path": "/home/user/.local/share/mise/installs/node/20.11.0"
    }
  }
}
```

---

## 相关规范

- **07-mise-integration.md** - mise 集成层（Port-Adapter 模式、DependencyPort 接口）
- **10-api-overview.md** - API 设计总览（认证、版本管理、长时间运行操作）
- **11-api-services.md** - 服务管理 API（服务如何依赖工具）
- **12-api-tasks.md** - 任务管理 API（任务如何依赖工具）
- **14-api-config.md** - 配置管理 API（mise 配置文件读写）

---

## 未来扩展

1. **自动依赖安装**：创建服务/任务时自动检测并安装依赖工具
2. **版本锁定**：支持 `.tool-versions` 文件，锁定项目工具版本
3. **工具别名**：支持工具别名（如 `nodejs` → `node`）
4. **离线安装**：支持从本地缓存安装工具（无网络环境）
5. **工具健康检查**：定期检查工具是否损坏，自动修复
6. **工具使用统计**：记录工具安装次数、使用频率
7. **自定义工具源**：支持私有插件仓库、镜像源
