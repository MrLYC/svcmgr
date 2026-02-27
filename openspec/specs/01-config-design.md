# 01 - 配置文件设计

> 版本：2.0.0-draft
> 状态：设计中

## 1. 配置文件层级

svcmgr 与 mise 配置物理分离，但存放在同一父目录下：

```
.config/mise/                           # mise 配置目录
├── config.toml                        # mise 全局配置
├── conf.d/                            # mise 场景配置（按字母序加载）
│   └── 00-base.toml
└── svcmgr/                            # svcmgr 配置（独立）
    ├── config.toml                    # svcmgr 核心配置
    └── conf.d/                        # svcmgr 场景配置
        ├── services.toml              # 服务定义
        └── local.toml                 # 本地覆盖
```

## 2. mise 配置格式

mise 配置遵循 [mise 官方文档](https://mise.jdx.dev/configuration.html)：

```toml
# .config/mise/config.toml

[tools]
node = "22"
python = "3.12"

[env]
NODE_ENV = "production"
DATABASE_URL = "postgres://localhost:5432/mydb"

[tasks.api-start]
description = "Start API server"
run = "node dist/server.js"
depends = ["api-build"]
env = { PORT = "3000" }

[tasks.api-build]
description = "Build API server"
run = "npm run build"
sources = ["src/**/*.ts"]
outputs = ["dist/**/*.js"]

[tasks.worker-run]
description = "Start background worker"
run = "python worker.py"

[tasks.cleanup]
description = "Cleanup old data"
run = "python scripts/cleanup.py"
```

**mise 任务形式**：
- 使用 file-tasks 形式定义任务（见 mise 文档）
- 任务可被外部直接调用：`mise run api-start`
- 当前工作目录下的环境变量和依赖用 `mise.toml` 管理

## 3. svcmgr 配置格式

### 3.1 服务定义 `[services.<name>]`

服务既可以是长期运行的后台进程，也可以是定时任务：

```toml
# .config/mise/svcmgr/config.toml

# 示例 1: mise 模式服务（默认，推荐）
[services.api]
run_mode = "mise"            # 运行模式: mise | script （默认 mise，可省略）
task = "api-start"           # mise 模式: 引用 mise 任务名
enable = true                # 开机自启
restart = "always"           # no | always | on-failure
restart_delay = "2s"         # 指数退避的初始值
restart_limit = 10           # 最大重启次数
restart_window = "60s"       # 统计窗口
stop_timeout = "10s"         # 优雅停止超时
workdir = "/app"             # 工作目录
timeout = "0"                # 0 = 无超时（长期服务）

# 端口管理（参考 pitchfork，但使用 services.api.ports）
# 格式: { 端口名 = 端口号 }
ports = { web = 8080, admin = 9000 }

# cgroups v2 资源限制（可选）
cpu_max_percent = 50         # 50% CPU
memory_max = "512m"          # 物理内存上限
pids_max = 100               # 最大进程数

# 示例 2: script 模式服务（直接执行命令）
[services.redis]
run_mode = "script"           # script 模式: 直接执行 command
command = "redis-server --port 6379 --daemonize no"  # 直接命令
enable = true
restart = "on-failure"
env = { REDIS_LOG_LEVEL = "notice" }  # script 模式的环境变量

[services.worker]
task = "worker-run"          # 省略 run_mode，默认为 mise 模式
enable = true
restart = "on-failure"
memory_max = "512m"
pids_max = 50

[services.cleanup]
task = "cleanup"

**关键设计**：
- **不支持 shell-hook**（与原始 pitchfork 的差异）
- **端口管理**：使用 `services.<name>.ports` 而非 `daemons.<name>.ports`
 **任务引用**：`task` 字段引用 mise 任务名，必须在 mise 配置中定义
 **运行模式**：支持 `mise` 和 `script` 两种模式
  - `mise` 模式（默认）：通过 `mise run <task>` 执行，继承 mise 环境变量和依赖管理
  - `script` 模式：直接执行 `command` 字段，不依赖 mise 任务，适用于特殊场景

### 3.2 配置目录管理 `[configurations.<name>]`

```toml
[configurations.app]
path = ".config/app"         # 受 Git 版本化管理的目录

[configurations.mise]
path = ".config/mise"        # mise 配置目录（可选纳入版本化）
```

### 3.3 凭据定义 `[credentials.<name>]`

凭据用于 HTTP 代理认证和外部服务访问，敏感信息通过 fnox 加密存储。

#### 3.3.1 Basic Authentication

```toml
[credentials.admin_basic]
type = "basic"
username_secret = "admin_username"  # 引用 fnox.toml 中的 secret
password_secret = "admin_password"
realm = "Admin Area"                # HTTP Basic Auth realm（可选，默认 "Restricted"）
```

#### 3.3.2 Bearer Token

```toml
[credentials.api_bearer]
type = "bearer"
token_secret = "api_token"          # 引用 fnox.toml 中的 secret
```

#### 3.3.3 API Key

```toml
# 通过 HTTP Header 传递
[credentials.external_api_header]
type = "api_key"
key_secret = "external_api_key"     # 引用 fnox.toml 中的 secret
header_name = "X-API-Key"           # HTTP 头名称

# 通过查询参数传递
[credentials.external_api_query]
type = "api_key"
key_secret = "external_api_key"
query_param = "api_key"             # 查询参数名称
```

#### 3.3.4 Custom Header

```toml
[credentials.custom_auth]
type = "custom"
header_name = "X-Custom-Token"      # 自定义头名称
value_secret = "custom_value"       # 引用 fnox.toml 中的 secret
```

**关键设计**：
- **敏感信息分离**：凭据定义只包含引用，实际的密码/token 存储在 fnox.toml 中
- **加密存储**：fnox 使用 age 或云 KMS 加密敏感信息
- **Git 友好**：加密后的凭据可以安全地提交到 Git
- **凭据引用**：在 `[[http.routes]]` 中通过 `auth` 字段引用凭据名称

**fnox 配置示例**：
```toml
# .config/mise/svcmgr/fnox.toml

[providers]
age = { type = "age", recipients = ["age1ql3z7hjy54pw3hyww5ayyfg7zqgvc7w3j2elw8zmrj2kg5sfn9aqmcac8p"] }

[secrets]
admin_username = { provider = "age", value = "age[...]encrypted_base64[...]" }
admin_password = { provider = "age", value = "age[...]encrypted_base64[...]" }
api_token = { provider = "age", value = "age[...]encrypted_base64[...]" }
```

**参考**：详见 [09-credential-management.md](./09-credential-management.md)

### 3.4 功能开关 `[features]`

```toml
[features]
web_ui = true
proxy = true
tunnel = false
scheduler = true
git_versioning = true
resource_limits = true
```

等价的环境变量控制：

```bash
SVCMGR_FEATURE_WEB_UI=1
SVCMGR_FEATURE_PROXY=1
SVCMGR_FEATURE_TUNNEL=0
```

## 4. 配置文件之间的引用关系

```
.config/mise/svcmgr/config.toml         .config/mise/config.toml
┌────────────────────┐                 ┌────────────────────┐
│ [services.api]     │ ──引用任务名──→  │ [tasks.api-start]  │
│ task = "api-start" │                 │ run = "node ..."   │
│ restart = "always" │                 │ env = { ... }      │
│ ports = { web=8080}│                 └────────────────────┘
└────────────────────┘
```

## 5. 配置解析流程

```
          ┌──────────────────────┐           ┌────────────────────────────┐
          │ mise 配置文件集        │           │ svcmgr 配置文件集            │
          │ (config.toml/conf.d)  │           │ (svcmgr/config.toml)        │
          └───────────┬──────────┘           └──────────────┬─────────────┘
                      │                                      │
               Mise Adapter/Port                       svcmgr TOML parser
                      │                                      │
                      └───────────────┬──────────────────────┘
                                      │
                              运行时配置对象
                         （任务命令 + 环境 + 服务/触发器）
```

svcmgr 启动时：
1. 读取 `.config/mise/svcmgr/config.toml` 获取自身配置
2. 解析 `.config/mise/config.toml` 和 `conf.d/*.toml` 获取 mise 任务/工具/环境变量定义
3. 将两者关联（svcmgr 服务引用 mise 任务名）后驱动调度引擎

## 6. 配置分离的优势

| 方面 | 独立配置文件 | 共享配置文件（x- 前缀）|
|------|-------------|----------------------|
| mise 未知段警告/报错风险 | **消除** | 存在（未来版本可能拒绝加载）|
| 配置格式独立演进 | **完全独立** | 受 mise 约束 |
| Git 版本化 | 同一目录，自然包含 | 同一目录，自然包含 |
| 配置职责 | **清晰分离** | 混合在一起 |
| 任务名一致性检查 | 启动时校验 | 启动时校验 |

## 7. 配置校验

启动时执行以下校验：

```rust
// 伪代码
fn validate_config(svcmgr_config: &SvcmgrConfig, mise_config: &MiseConfig) -> Result<()> {
    for (name, service) in &svcmgr_config.services {
        // 检查引用的任务是否存在
        if !mise_config.tasks.contains_key(&service.task) {
            return Err(format!(
                "Service '{}' references non-existent mise task '{}'",
                name, service.task
            ));
        }
        
        // 检查 cron 表达式有效性
        if let Some(cron) = &service.cron {
            cron::Schedule::from_str(cron)?;
        }
        
        // 检查资源限制值合法性
        if let Some(cpu) = service.cpu_max_percent {
            if cpu > 100 {
                return Err(format!("cpu_max_percent must be <= 100, got {}", cpu));
            }
        }
    }
    Ok(())
}
```

## 8. 配置更新与热重载

配置变更流程（通过 Git 版本化）：

```
1. 用户修改配置文件
   ↓
2. git add .config/mise/svcmgr/config.toml
   ↓
3. git commit -m "update: api service memory limit"
   ↓
4. 触发 ConfigChanged 事件
   ↓
5. 调度引擎重新加载配置
   ↓
6. 比较新旧配置 diff
   ↓
7. 对变更的服务执行操作：
   - 服务定义变化 → restart
   - 新增服务 + enable=true → start
   - 删除服务 → stop
```

## 9. 完整配置文件示例

### mise 配置

```toml
# ~/.config/mise/config.toml（或项目根 mise.toml）

[tools]
node = "22"
python = "3.12"

[env]
NODE_ENV = "production"
DATABASE_URL = "postgres://localhost:5432/mydb"

[tasks.api-start]
description = "Start API server"
run = "node dist/server.js"
depends = ["api-build"]
env = { PORT = "3000" }

[tasks.api-build]
description = "Build API server"
run = "npm run build"
sources = ["src/**/*.ts"]
outputs = ["dist/**/*.js"]

[tasks.worker-run]
description = "Start background worker"
run = "python worker.py"

[tasks.cleanup]
description = "Cleanup old data"
run = "python scripts/cleanup.py"
```

### svcmgr 配置

```toml
# ~/.config/mise/svcmgr/config.toml

[features]
web_ui = true
proxy = true
tunnel = false
scheduler = true
git_versioning = true
resource_limits = true

[services.api]
task = "api-start"
enable = true
restart = "always"
restart_delay = "2s"
restart_limit = 10
restart_window = "60s"
stop_timeout = "10s"
ports = { web = 3000 }
cpu_max_percent = 50
memory_max = "512m"
pids_max = 100

[services.worker]
task = "worker-run"
enable = true
restart = "on-failure"
restart_delay = "5s"
memory_max = "512m"
pids_max = 50

[services.cleanup]
task = "cleanup"
cron = "0 2 * * *"
timeout = "600s"

[services.health-check]
task = "api-health"
cron = "*/5 * * * *"
timeout = "30s"

[configurations.app]
path = ".config/app"
```

## 10. 配置迁移工具

提供 CLI 工具将旧格式转换为新格式：

```bash
# 迁移旧配置
svcmgr config migrate --from ./old-config --to .config/mise/svcmgr

# 验证配置
svcmgr config validate

# 查看配置引用关系
svcmgr config check
```

## 参考

- [00-architecture-overview.md](./00-architecture-overview.md) - 整体架构
- [04-git-versioning.md](./04-git-versioning.md) - Git 配置版本管理
- [mise 配置文档](https://mise.jdx.dev/configuration.html)
- [pitchfork 配置参考](https://pitchfork.jdx.dev)
