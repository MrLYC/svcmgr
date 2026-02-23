# 22 - 破坏性变更清单

> 版本：2.0.0-draft
> 状态：设计中

## 目录

1. [破坏性变更概览](#1-破坏性变更概览)
2. [架构层面变更](#2-架构层面变更)
3. [配置文件格式变更](#3-配置文件格式变更)
4. [CLI 命令变更](#4-cli-命令变更)
5. [API 端点变更](#5-api-端点变更)
6. [行为变更](#6-行为变更)
7. [依赖变更](#7-依赖变更)
8. [兼容性矩阵](#8-兼容性矩阵)
9. [迁移时间线](#9-迁移时间线)
10. [不兼容的功能](#10-不兼容的功能)
11. [替代方案](#11-替代方案)
12. [风险评估](#12-风险评估)

---

## 1. 破坏性变更概览

### 1.1 设计目标

本文档列出从 svcmgr v1.x（基于 systemd + cron + nginx）到 v2.0（基于 mise 统一架构）的所有不兼容变更，帮助用户：

- **评估迁移影响**：理解哪些变更会影响现有工作流
- **制定迁移计划**：识别需要重写的配置和脚本
- **准备回滚策略**：了解不可逆变更和回滚风险

### 1.2 目标受众

- **现有 svcmgr v1.x 用户**：需要升级到 v2.0 的用户
- **系统管理员**：管理多个 svcmgr 实例的管理员
- **自动化脚本维护者**：使用 svcmgr CLI/API 的自动化脚本作者

### 1.3 使用场景

- **迁移前评估**：在开始迁移前评估工作量和风险
- **迁移计划制定**：根据变更清单制定详细迁移步骤
- **问题排查**：迁移后遇到问题时查找原因
- **文档更新**：更新依赖 svcmgr 的项目文档

### 1.4 变更严重程度分类

- **Critical（致命）**：必须立即处理，否则系统无法运行
- **Major（重大）**：影响核心功能，需要重写配置或代码
- **Minor（次要）**：影响边缘功能或可通过简单调整解决

---

## 2. 架构层面变更

### 2.1 服务管理架构变更

**变更**：从 systemd 用户服务切换到 mise 任务 + svcmgr 统一调度引擎

**严重程度**：**Critical**

**详细说明**：

| 旧架构（v1.x） | 新架构（v2.0） | 影响 |
|---------------|---------------|------|
| systemd 用户服务单元（`~/.config/systemd/user/*.service`） | mise 任务定义（`.config/mise/config.toml` `[tasks.*]`） + svcmgr 服务配置（`.config/mise/svcmgr/config.toml` `[services.*]`） | 所有服务配置需完全重写 |
| `systemctl --user start/stop/restart` | `svcmgr service start/stop/restart` | 所有管理命令需更改 |
| systemd 依赖顺序（`Before=`, `After=`） | svcmgr 任务依赖（mise `depends`） | 依赖声明方式完全不同 |
| journalctl 日志查看 | `svcmgr service logs` 或文件日志 | 日志访问方式变更 |

**示例对比**：

```ini
# 旧配置（v1.x）：~/.config/systemd/user/api.service
[Unit]
Description=API Server
After=network.target

[Service]
Type=simple
ExecStart=/usr/bin/node /app/server.js
Restart=always
Environment="PORT=3000"
Environment="NODE_ENV=production"

[Install]
WantedBy=default.target
```

```toml
# 新配置（v2.0）：.config/mise/config.toml
[tasks.api-start]
description = "Start API server"
run = "node /app/server.js"
env = { PORT = "3000", NODE_ENV = "production" }

# .config/mise/svcmgr/config.toml
[services.api]
task = "api-start"
enable = true
restart = "always"
stop_timeout = "10s"
```

**迁移建议**：
- 使用 `svcmgr migrate systemd` 自动转换服务单元（见 **21-migration-guide.md** §6.1）
- 手动验证转换后的配置，特别是环境变量和依赖关系
- 逐个服务迁移，避免一次性迁移所有服务

**相关文档**：
- **00-architecture-overview.md** - 新架构设计
- **02-scheduler-engine.md** - 统一调度引擎
- **21-migration-guide.md** §3.1 - 服务配置迁移

---

### 2.2 定时任务架构变更

**变更**：从 crontab 切换到 svcmgr 统一调度引擎

**严重程度**：**Critical**

**详细说明**：

| 旧架构（v1.x） | 新架构（v2.0） | 影响 |
|---------------|---------------|------|
| crontab 条目（`crontab -e`） | svcmgr scheduled_tasks（`.config/mise/svcmgr/config.toml` `[scheduled_tasks.*]`） | 所有定时任务需重写 |
| cron 语法（`0 2 * * *`） | 保持相同的 cron 语法（`schedule = "0 2 * * *"`） | 语法兼容，但配置位置变更 |
| cron 环境变量继承 | mise 环境变量显式定义 | 环境变量需显式声明 |
| `MAILTO` 通知 | 日志输出 + 可选的通知插件 | 通知机制变更 |

**示例对比**：

```bash
# 旧配置（v1.x）：crontab -l
0 2 * * * /usr/bin/python /app/scripts/cleanup.py
30 3 * * 0 /usr/bin/node /app/scripts/backup.js
```

```toml
# 新配置（v2.0）：.config/mise/config.toml
[tasks.cleanup]
run = "python /app/scripts/cleanup.py"

[tasks.backup]
run = "node /app/scripts/backup.js"

# .config/mise/svcmgr/config.toml
[scheduled_tasks.cleanup]
task = "cleanup"
schedule = "0 2 * * *"
timeout = "600s"

[scheduled_tasks.backup]
task = "backup"
schedule = "30 3 * * 0"
timeout = "1800s"
```

**迁移建议**：
- 使用 `svcmgr migrate crontab` 自动转换 crontab 条目（见 **21-migration-guide.md** §6.2）
- 手动添加可能缺失的环境变量（crontab 继承的环境变量需显式声明）
- 测试定时任务的触发和执行

**相关文档**：
- **02-scheduler-engine.md** - 调度引擎与 Cron 触发器
- **21-migration-guide.md** §3.2 - 定时任务迁移

---

### 2.3 反向代理架构变更

**变更**：从 nginx 切换到内置 HTTP 代理（基于 axum/hyper）

**严重程度**：**Major**

**详细说明**：

| 旧架构（v1.x） | 新架构（v2.0） | 影响 |
|---------------|---------------|------|
| nginx 配置文件（`nginx.conf`） | svcmgr HTTP 路由配置（`.config/mise/svcmgr/config.toml` `[[http.routes]]`） | nginx 配置需完全重写 |
| `nginx -s reload` 重载配置 | 自动热更新（无需手动操作） | 配置变更立即生效 |
| nginx 第三方模块 | 不支持（需评估替代方案） | 部分功能可能无法迁移 |
| 复杂的 Lua 脚本 | 需重写为 Rust 中间件 | 高度定制化逻辑需重新实现 |

**示例对比**：

```nginx
# 旧配置（v1.x）：nginx.conf
server {
    listen 80;
    server_name api.example.com;

    location /api {
        proxy_pass http://localhost:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }

    location /static {
        root /var/www/static;
    }
}
```

```toml
# 新配置（v2.0）：.config/mise/svcmgr/config.toml
[[http.routes]]
path = "/api"
service = "api"
strip_prefix = false

[[http.routes]]
path = "/static"
type = "static"
root = "/var/www/static"

[services.api]
task = "api-start"
ports = { web = 3000 }
```

**迁移建议**：
- 使用 `svcmgr migrate nginx` 半自动转换 nginx 配置（见 **21-migration-guide.md** §6.3）
- 手动处理复杂的 location 块和自定义指令
- 评估第三方模块的替代方案（可能需要保留 nginx 作为前端代理）
- 如果必须使用 nginx 特性，可继续使用 nginx 作为前端代理，svcmgr 作为后端服务管理

**相关文档**：
- **05-web-service.md** - 内置 HTTP 代理设计
- **21-migration-guide.md** §3.3 - nginx 配置迁移

---

### 2.4 进程管理架构变更

**变更**：从独立进程管理实现切换到内嵌 pitchfork 库

**严重程度**：**Minor**

**详细说明**：

| 旧架构（v1.x） | 新架构（v2.0） | 影响 |
|---------------|---------------|------|
| 独立的 supervisor 实现（`atoms/supervisor.rs`） | 内嵌 pitchfork 库（`supervisor` + `daemon` + `procs` 模块） | 进程管理实现细节变更，用户感知较小 |
| 无资源限制 | 可选的 cgroups v2 资源限制 | 新增资源限制功能（可关闭） |
| 进程重启策略有限 | 丰富的重启策略（always / on-failure / no） | 重启行为更可控 |

**迁移建议**：
- 此变更对用户透明，无需手动迁移
- 如需资源限制功能，在服务配置中添加 `cpu_max_percent`、`memory_max`、`pids_max` 字段

**相关文档**：
- **03-process-manager.md** - 进程管理器设计
- **06-feature-flags.md** - 资源限制功能开关

---

## 3. 配置文件格式变更

### 3.1 systemd 单元文件 → TOML 配置

**变更**：所有 systemd 单元文件需转换为 TOML 格式

**严重程度**：**Critical**

**字段映射表**：

| systemd 单元字段 | mise 任务字段 | svcmgr 服务字段 | 说明 |
|-----------------|--------------|----------------|------|
| `ExecStart` | `run` | - | 启动命令 |
| `WorkingDirectory` | `dir` | `workdir` | 工作目录 |
| `Environment` | `env` | - | 环境变量 |
| `EnvironmentFile` | - | - | 不支持，需手动展开环境变量 |
| `Restart` | - | `restart` | 重启策略（值不同：`always` 保持，`on-failure` 保持，`no` 保持） |
| `RestartSec` | - | `restart_delay` | 重启延迟（格式变更：`5` → `"5s"`） |
| `TimeoutStopSec` | - | `stop_timeout` | 停止超时（格式变更：`10` → `"10s"`） |
| `User` | - | - | 不支持（仅用户级服务） |
| `Group` | - | - | 不支持（仅用户级服务） |
| `Before=` / `After=` | `depends` | - | 依赖顺序（语义变更：systemd 是启动顺序，mise 是执行依赖） |
| `Wants=` / `Requires=` | - | - | 不支持（需手动管理服务启动顺序） |

**详细示例**：

```ini
# 旧配置（v1.x）：~/.config/systemd/user/worker.service
[Unit]
Description=Background Worker
After=api.service

[Service]
Type=simple
WorkingDirectory=/app
ExecStart=/usr/bin/python worker.py
Restart=on-failure
RestartSec=5
Environment="DATABASE_URL=postgres://localhost:5432/mydb"
EnvironmentFile=/app/.env

[Install]
WantedBy=default.target
```

```toml
# 新配置（v2.0）：.config/mise/config.toml
[env]
DATABASE_URL = "postgres://localhost:5432/mydb"
# 注意：EnvironmentFile 需手动展开到此处

[tasks.worker-run]
description = "Background Worker"
run = "python worker.py"
dir = "/app"
depends = ["api-start"]  # 依赖 api-start 任务

# .config/mise/svcmgr/config.toml
[services.worker]
task = "worker-run"
enable = true
restart = "on-failure"
restart_delay = "5s"
```

**不支持的字段及替代方案**：

| systemd 字段 | 替代方案 |
|-------------|---------|
| `EnvironmentFile` | 手动展开环境变量到 `[env]` 或 `[tasks.*.env]` |
| `User` / `Group` | svcmgr v2.0 仅支持用户级服务，无需指定 |
| `Wants=` / `Requires=` | 使用 mise 任务依赖 `depends` 或手动管理启动顺序 |
| `ConditionPathExists` | 启动前检查脚本手动实现 |
| `SuccessExitStatus` | 不支持，所有非零退出码视为失败 |

**迁移建议**：
- 使用 `svcmgr migrate systemd` 自动转换（覆盖 80% 常见字段）
- 手动处理 `EnvironmentFile`（读取文件内容并展开到 `[env]`）
- 重新评估 `Before=` / `After=` 依赖（systemd 是启动顺序，mise `depends` 是执行依赖）
- 测试所有服务的环境变量和依赖关系

**相关文档**：
- **01-config-design.md** §3.1 - 服务定义格式
- **21-migration-guide.md** §3.1 - systemd 配置迁移

---

### 3.2 crontab 语法 → TOML scheduled_tasks

**变更**：crontab 条目需转换为 TOML 格式

**严重程度**：**Major**

**字段映射表**：

| crontab 字段 | mise 任务字段 | svcmgr scheduled_tasks 字段 | 说明 |
|-------------|--------------|----------------------------|------|
| cron 表达式（`0 2 * * *`） | - | `schedule` | cron 语法保持兼容 |
| 命令 | `run` | - | 执行命令 |
| 环境变量（cron 继承） | `env` | - | 需显式声明 |
| `MAILTO` | - | - | 不支持，使用日志输出 |

**详细示例**：

```bash
# 旧配置（v1.x）：crontab -l
SHELL=/bin/bash
PATH=/usr/local/bin:/usr/bin:/bin
DATABASE_URL=postgres://localhost:5432/mydb

0 2 * * * /usr/bin/python /app/scripts/cleanup.py
30 3 * * 0 /app/scripts/backup.sh
```

```toml
# 新配置（v2.0）：.config/mise/config.toml
[env]
SHELL = "/bin/bash"
PATH = "/usr/local/bin:/usr/bin:/bin"
DATABASE_URL = "postgres://localhost:5432/mydb"

[tasks.cleanup]
run = "python /app/scripts/cleanup.py"

[tasks.backup]
run = "/app/scripts/backup.sh"

# .config/mise/svcmgr/config.toml
[scheduled_tasks.cleanup]
task = "cleanup"
schedule = "0 2 * * *"
timeout = "600s"

[scheduled_tasks.backup]
task = "backup"
schedule = "30 3 * * 0"
timeout = "1800s"
```

**不支持的功能及替代方案**：

| crontab 功能 | 替代方案 |
|-------------|---------|
| 环境变量继承（cron 自动继承用户环境） | 显式声明到 `[env]` 或 `[tasks.*.env]` |
| `MAILTO`（邮件通知） | 使用日志输出 + 外部通知工具（如监控告警） |
| 特殊字符串（`@daily`, `@hourly`） | 转换为标准 cron 表达式（`0 0 * * *`, `0 * * * *`） |

**迁移建议**：
- 使用 `svcmgr migrate crontab` 自动转换（覆盖 90% 常见场景）
- 手动添加 crontab 中定义的环境变量（`SHELL`, `PATH`, 自定义变量）
- 移除 `MAILTO` 并配置外部监控告警（如 Prometheus + Alertmanager）
- 测试定时任务的触发时间和环境变量

**相关文档**：
- **02-scheduler-engine.md** §3.3 - Cron 触发器
- **21-migration-guide.md** §3.2 - crontab 迁移

---

### 3.3 nginx 配置 → HTTP routes TOML

**变更**：nginx 配置文件需转换为 TOML 路由规则

**严重程度**：**Major**

**字段映射表**：

| nginx 配置 | svcmgr http.routes 字段 | 说明 |
|-----------|------------------------|------|
| `location /path` | `path` | 路径匹配 |
| `proxy_pass http://...` | `service` | 引用服务名（而非 URL） |
| `root /var/www` | `root` | 静态文件根目录 |
| `rewrite` | - | 不支持（部分功能可用 `strip_prefix`） |
| `proxy_set_header` | - | 自动设置常见头（`X-Real-IP`, `X-Forwarded-For`） |
| `add_header` | - | 不支持 |
| `return` / `redirect` | - | 不支持 |

**详细示例**：

```nginx
# 旧配置（v1.x）：nginx.conf
server {
    listen 80;
    server_name api.example.com;

    location /api/v1 {
        proxy_pass http://localhost:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }

    location /static {
        root /var/www;
        autoindex on;
    }

    location /admin {
        proxy_pass http://localhost:9000;
        auth_basic "Admin Area";
        auth_basic_user_file /etc/nginx/.htpasswd;
    }
}
```

```toml
# 新配置（v2.0）：.config/mise/svcmgr/config.toml
[[http.routes]]
path = "/api/v1"
service = "api"
strip_prefix = false

[[http.routes]]
path = "/static"
type = "static"
root = "/var/www/static"

[[http.routes]]
path = "/admin"
service = "admin"
# 注意：auth_basic 不支持，需在服务内部实现认证

[services.api]
task = "api-start"
ports = { web = 3000 }

[services.admin]
task = "admin-start"
ports = { web = 9000 }
```

**不支持的功能及替代方案**：

| nginx 功能 | 替代方案 |
|-----------|---------|
| `rewrite` / `redirect` | 在应用层实现重定向 |
| `auth_basic` | 在应用层实现 HTTP Basic Auth 或使用 JWT |
| `add_header` | 在应用层添加响应头 |
| Lua 脚本 | 重写为 Rust 中间件（需代码开发） |
| 第三方模块（ngx_cache_purge, ngx_lua_waf） | 评估是否必需，考虑保留 nginx 作为前端代理 |
| SSL/TLS 终止 | 使用 nginx/Caddy 作为前端代理，或在应用层实现 |

**复杂场景建议**：
- **简单代理**：直接迁移到 svcmgr 内置代理
- **中等复杂度**（使用 rewrite、认证、自定义头）：在应用层实现这些逻辑
- **高度复杂**（Lua 脚本、第三方模块、SSL 终止）：保留 nginx 作为前端代理，svcmgr 仅管理后端服务

**迁移建议**：
- 使用 `svcmgr migrate nginx` 半自动转换简单配置
- 手动处理复杂的 location 块（rewrite、认证、自定义头）
- 评估是否需要保留 nginx（如果依赖高级特性）
- 测试所有路由规则和静态文件服务

**相关文档**：
- **05-web-service.md** §3 - HTTP 路由规则
- **21-migration-guide.md** §3.3 - nginx 配置迁移

---

## 4. CLI 命令变更

### 4.1 移除的命令

**严重程度**：**Critical**

以下命令在 v2.0 中完全移除，需使用新命令替代：

| 旧命令（v1.x） | 新命令（v2.0） | 说明 |
|---------------|---------------|------|
| `systemctl --user start <service>` | `svcmgr service start <name>` | 启动服务 |
| `systemctl --user stop <service>` | `svcmgr service stop <name>` | 停止服务 |
| `systemctl --user restart <service>` | `svcmgr service restart <name>` | 重启服务 |
| `systemctl --user status <service>` | `svcmgr service status <name>` | 查看服务状态 |
| `systemctl --user enable <service>` | 编辑 `.config/mise/svcmgr/config.toml` 设置 `enable = true` | 启用开机自启 |
| `systemctl --user disable <service>` | 编辑 `.config/mise/svcmgr/config.toml` 设置 `enable = false` | 禁用开机自启 |
| `journalctl --user -u <service>` | `svcmgr service logs <name>` | 查看服务日志 |
| `crontab -e` | 编辑 `.config/mise/svcmgr/config.toml` | 编辑定时任务 |
| `crontab -l` | `svcmgr task list` | 列出定时任务 |
| `crontab -r` | 删除 `.config/mise/svcmgr/config.toml` 中的 `[scheduled_tasks.*]` 段 | 删除所有定时任务 |
| `nginx -s reload` | 无需操作（配置自动热更新） | 重载代理配置 |
| `nginx -t` | `svcmgr config validate` | 验证配置 |

**迁移建议**：
- 更新所有脚本和文档中的命令引用
- 搜索代码库中的 `systemctl --user` 和 `crontab` 调用
- 自动化脚本需全面重写（见 **21-migration-guide.md** §5）

---

### 4.2 新增的命令

**严重程度**：**Minor**

v2.0 新增以下管理命令：

| 命令 | 功能 | 示例 |
|-----|------|------|
| `svcmgr service start <name>` | 启动服务 | `svcmgr service start api` |
| `svcmgr service stop <name>` | 停止服务 | `svcmgr service stop api` |
| `svcmgr service restart <name>` | 重启服务 | `svcmgr service restart api` |
| `svcmgr service status [<name>]` | 查看服务状态 | `svcmgr service status` / `svcmgr service status api` |
| `svcmgr service logs <name>` | 查看服务日志 | `svcmgr service logs api --tail 100 --follow` |
| `svcmgr task list` | 列出所有任务 | `svcmgr task list` |
| `svcmgr task run <name>` | 手动运行任务 | `svcmgr task run cleanup` |
| `svcmgr task history <name>` | 查看任务执行历史 | `svcmgr task history cleanup --limit 10` |
| `svcmgr config validate` | 验证配置文件 | `svcmgr config validate` |
| `svcmgr config show` | 显示当前配置 | `svcmgr config show` |
| `svcmgr config rollback <commit>` | 回滚配置到指定版本 | `svcmgr config rollback HEAD~1` |
| `svcmgr migrate systemd` | 迁移 systemd 服务配置 | `svcmgr migrate systemd` |
| `svcmgr migrate crontab` | 迁移 crontab 定时任务 | `svcmgr migrate crontab` |
| `svcmgr migrate nginx` | 迁移 nginx 配置 | `svcmgr migrate nginx` |

**相关文档**：
- **11-api-services.md** - 服务管理 API（CLI 命令映射到 API）
- **12-api-tasks.md** - 任务管理 API
- **14-api-config.md** - 配置管理 API

---

### 4.3 变更的命令

**严重程度**：**Major**

以下命令行为发生变化：

| 命令 | 旧行为（v1.x） | 新行为（v2.0） | 影响 |
|-----|---------------|---------------|------|
| 日志查看 | `journalctl --user -u <service>` | `svcmgr service logs <name>` | 输出格式变更（journalctl → 文件日志） |
| 状态查询 | `systemctl --user status <service>` | `svcmgr service status <name>` | 输出格式变更（systemd 格式 → JSON） |
| 配置重载 | `nginx -s reload` | 无需操作（自动热更新） | nginx 配置变更立即生效 |

**输出格式对比**：

```bash
# 旧命令（v1.x）：systemctl --user status api.service
● api.service - API Server
   Loaded: loaded (/home/user/.config/systemd/user/api.service; enabled; vendor preset: enabled)
   Active: active (running) since Mon 2026-02-23 10:00:00 CST; 2h 30min ago
 Main PID: 12345 (node)
   CGroup: /user.slice/user-1000.slice/user@1000.service/api.service
           └─12345 /usr/bin/node /app/server.js

# 新命令（v2.0）：svcmgr service status api
{
  "name": "api",
  "status": "running",
  "pid": 12345,
  "uptime": "2h 30min",
  "restarts": 0,
  "memory_usage": "128 MB",
  "cpu_usage": "2.5%"
}
```

**迁移建议**：
- 更新依赖 CLI 输出的脚本（特别是解析 systemctl/journalctl 输出的脚本）
- 使用 `--format json` 选项获取结构化输出（如果可用）
- 测试所有自动化脚本

---

## 5. API 端点变更

### 5.1 API 端点完全重新设计

**变更**：如果 v1.x 提供了 REST API，v2.0 的 API 端点完全不兼容

**严重程度**：**Critical**（如果使用了 v1.x API）

**注意**：根据项目历史，v1.x 可能未提供公开 API。如果 v1.x 未提供 API，此部分可跳过。

**v2.0 新 API 端点**（假设 v1.x 未提供 API）：

| 端点 | 方法 | 功能 |
|-----|------|------|
| `/api/v1/services` | GET | 列出所有服务 |
| `/api/v1/services/{name}` | GET | 获取服务详情 |
| `/api/v1/services/{name}/start` | POST | 启动服务 |
| `/api/v1/services/{name}/stop` | POST | 停止服务 |
| `/api/v1/services/{name}/restart` | POST | 重启服务 |
| `/api/v1/services/{name}/logs` | GET | 获取服务日志 |
| `/api/v1/tasks` | GET | 列出所有任务 |
| `/api/v1/tasks/{name}/run` | POST | 手动运行任务 |
| `/api/v1/tasks/{name}/history` | GET | 获取任务执行历史 |
| `/api/v1/config` | GET | 获取当前配置 |
| `/api/v1/config` | PUT | 更新配置 |
| `/api/v1/config/validate` | POST | 验证配置 |

**如果 v1.x 有 API**，需要：
- 重写所有 API 客户端代码
- 更新 API 文档
- 提供 v1 → v2 API 迁移指南（需根据实际 v1.x API 设计补充）

**相关文档**：
- **10-api-overview.md** - API 设计总览
- **11-api-services.md** - 服务管理 API
- **12-api-tasks.md** - 任务管理 API
- **14-api-config.md** - 配置管理 API

---

## 6. 行为变更

### 6.1 服务重启策略默认值变更

**变更**：服务重启策略默认值从 `no` 改为 `on-failure`

**严重程度**：**Major**

**详细说明**：

| 旧行为（v1.x） | 新行为（v2.0） | 影响 |
|---------------|---------------|------|
| systemd 默认 `Restart=no`（不自动重启） | svcmgr 默认 `restart = "on-failure"`（失败时自动重启） | 服务失败后会自动重启（除非显式设置 `restart = "no"`） |

**示例**：

```toml
# v1.x：systemd 单元未指定 Restart，服务崩溃后不会重启
[Service]
ExecStart=/usr/bin/node server.js
# Restart 未指定，默认为 no

# v2.0：svcmgr 服务未指定 restart，服务崩溃后会自动重启
[services.api]
task = "api-start"
# restart 未指定，默认为 "on-failure"
```

**迁移建议**：
- 如果希望保持 v1.x 行为（不自动重启），显式设置 `restart = "no"`
- 检查所有服务配置，确认重启策略符合预期

**相关文档**：
- **03-process-manager.md** §2.3 - 重启策略

---

### 6.2 日志输出格式变更

**变更**：从 journalctl 格式切换到文件日志（可选结构化 JSON）

**严重程度**：**Major**

**详细说明**：

| 旧行为（v1.x） | 新行为（v2.0） | 影响 |
|---------------|---------------|------|
| journalctl 格式（systemd 日志） | 文件日志（`~/.local/share/svcmgr/logs/<service>.log`） | 日志查看方式和工具变更 |
| 日志自动轮转（journald） | 需手动配置日志轮转（logrotate 或 svcmgr 内置轮转） | 需配置日志轮转防止磁盘占满 |
| 结构化日志字段（journalctl -o json） | 可选 JSON 格式日志（配置 `log_format = "json"`） | 日志解析方式变更 |

**示例**：

```bash
# 旧方式（v1.x）：journalctl --user -u api.service
Feb 23 10:00:00 hostname node[12345]: Server listening on port 3000
Feb 23 10:00:01 hostname node[12345]: Database connected

# 新方式（v2.0）：cat ~/.local/share/svcmgr/logs/api.log
2026-02-23T10:00:00.000Z [INFO] Server listening on port 3000
2026-02-23T10:00:01.000Z [INFO] Database connected

# 或使用 CLI：svcmgr service logs api
```

**迁移建议**：
- 更新日志收集工具（如 Prometheus Loki、Elasticsearch）的配置
- 配置日志轮转（使用 logrotate 或 svcmgr 内置轮转功能）
- 如果需要结构化日志，配置 `log_format = "json"`

**相关文档**：
- **03-process-manager.md** §4 - 日志管理
- **11-api-services.md** - 日志查询 API（获取服务日志）

---

### 6.3 端口绑定行为变更

**变更**：从隐式端口绑定改为显式配置 + 自动服务发现

**严重程度**：**Minor**

**详细说明**：

| 旧行为（v1.x） | 新行为（v2.0） | 影响 |
|---------------|---------------|------|
| 服务端口在应用代码中硬编码或通过环境变量传递 | 服务端口在 `services.<name>.ports` 中显式声明 | 端口管理更清晰，支持反向代理自动路由 |
| nginx 配置中硬编码 `proxy_pass http://localhost:3000` | HTTP 路由通过 `service = "api"` 引用服务，端口自动解析 | 端口变更无需修改代理配置 |

**示例**：

```toml
# v2.0：显式声明服务端口
[services.api]
task = "api-start"
ports = { web = 3000, admin = 9000 }  # 显式声明端口

[[http.routes]]
path = "/api"
service = "api"  # 自动解析为 api.ports.web (3000)

[[http.routes]]
path = "/admin"
service = "api"
port_name = "admin"  # 显式指定端口名，解析为 api.ports.admin (9000)
```

**迁移建议**：
- 在服务配置中添加 `ports` 字段
- 更新 HTTP 路由配置，使用 `service` 引用而非硬编码 URL

**相关文档**：
- **01-config-design.md** §3.1 - 端口管理
- **05-web-service.md** §3.2 - 服务路由

---

### 6.4 环境变量继承方式变更

**变更**：从 systemd 继承用户环境改为 mise 显式定义

**严重程度**：**Major**

**详细说明**：

| 旧行为（v1.x） | 新行为（v2.0） | 影响 |
|---------------|---------------|------|
| systemd 服务继承用户 shell 环境变量（`~/.bashrc`, `~/.profile`） | mise 环境变量需显式定义在 `[env]` 或 `[tasks.*.env]` | 隐式依赖的环境变量需显式声明 |
| crontab 继承部分 cron 环境变量（`SHELL`, `PATH`, `HOME`） | 所有环境变量需显式定义 | crontab 任务的环境变量需手动迁移 |

**示例**：

```bash
# 旧方式（v1.x）：systemd 服务自动继承用户环境
# ~/.bashrc
export DATABASE_URL="postgres://localhost:5432/mydb"
export SECRET_KEY="abc123"

# ~/.config/systemd/user/api.service
[Service]
ExecStart=/usr/bin/node server.js
# 自动继承 DATABASE_URL 和 SECRET_KEY
```

```toml
# 新方式（v2.0）：需显式声明环境变量
# .config/mise/config.toml
[env]
DATABASE_URL = "postgres://localhost:5432/mydb"
SECRET_KEY = "abc123"  # 敏感信息建议使用 mise 的 _.file 或密钥管理工具

[tasks.api-start]
run = "node server.js"
# 继承 [env] 中的环境变量
```

**迁移建议**：
- 列出所有服务依赖的环境变量（从 `~/.bashrc`, `~/.profile`, `~/.zshrc` 等文件中提取）
- 将环境变量迁移到 `.config/mise/config.toml` 的 `[env]` 段
- 敏感信息使用 mise 的 `_.file` 功能加载（如 `.env` 文件）或使用密钥管理工具
- 测试所有服务的环境变量是否正确加载

**相关文档**：
- **01-config-design.md** §2 - mise 环境变量管理
- **15-api-env.md** - 环境变量 API

---

### 6.5 工作目录默认值变更

**变更**：从 systemd 默认 `/` 改为任务定义的 `dir` 或当前目录

**严重程度**：**Minor**

**详细说明**：

| 旧行为（v1.x） | 新行为（v2.0） | 影响 |
|---------------|---------------|------|
| systemd 服务未指定 `WorkingDirectory` 时默认为 `/` | mise 任务未指定 `dir` 时默认为 mise 配置文件所在目录（通常是项目根目录） | 相对路径引用行为可能变化 |

**示例**：

```ini
# 旧方式（v1.x）：systemd 服务
[Service]
ExecStart=/usr/bin/node server.js
# WorkingDirectory 未指定，默认为 /
# 如果 server.js 中使用相对路径（如 require('./config.json')），会从 / 查找
```

```toml
# 新方式（v2.0）：mise 任务
[tasks.api-start]
run = "node server.js"
dir = "/app"  # 显式指定工作目录
# 如果未指定 dir，默认为 mise 配置文件所在目录（如 /home/user/project）
```

**迁移建议**：
- 检查所有使用相对路径的服务（如 `require('./file')`, `open('data.txt')`）
- 在 mise 任务中显式指定 `dir`，确保工作目录正确

**相关文档**：
- **01-config-design.md** §2 - mise 任务定义

---

## 7. 依赖变更

### 7.1 移除的依赖

**严重程度**：**Major**

以下外部依赖在 v2.0 中不再必需（或变为可选）：

| 依赖 | v1.x 状态 | v2.0 状态 | 说明 |
|-----|----------|----------|------|
| systemd | 必需（用户级服务管理） | 可选（不再作为服务管理器） | 如果系统使用 systemd，svcmgr 不会干扰系统级 systemd |
| cron / cronie | 必需（定时任务调度） | 不需要（完全替代） | 使用 svcmgr 内置调度引擎 |
| nginx | 可选（反向代理） | 可选（内置代理替代） | 简单场景使用内置代理，复杂场景可保留 nginx |

**迁移建议**：
- 评估是否可以完全移除 cron（通常可以）
- 评估是否可以移除 nginx（取决于使用场景）
- 如果保留 nginx，确保 nginx 和 svcmgr 内置代理不冲突（监听不同端口）

---

### 7.2 新增的依赖

**严重程度**：**Critical**

v2.0 引入以下新依赖：

| 依赖 | 最低版本 | 推荐版本 | 说明 | 安装方式 |
|-----|---------|---------|------|---------|
| mise | 2024.1.0 | 最新稳定版 | 依赖管理、环境变量、任务定义 | `curl https://mise.run \| sh` |
| Rust 工具链 | 1.75.0 | 1.76.0+ | 编译 svcmgr（如果从源码安装） | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| Git | 2.30.0+ | 2.40.0+ | 配置版本化管理 | 系统包管理器 |

**可选依赖**（功能特性相关）：

| 依赖 | 最低版本 | 功能 | 如何启用 |
|-----|---------|------|---------|
| cgroups v2 | Linux 4.15+ | 资源限制（CPU、内存、进程数） | 默认启用（如果可用），配置 `features.resource_limits = false` 禁用 |

**安装步骤**：

```bash
# 1. 安装 mise（必需）
curl https://mise.run | sh
echo 'eval "$(mise activate bash)"' >> ~/.bashrc
source ~/.bashrc

# 2. 验证 mise 版本
mise --version  # 应 >= 2024.1.0

# 3. 安装 svcmgr v2.0（假设发布了预编译二进制）
# 方法 A：使用预编译二进制
curl -fsSL https://github.com/username/svcmgr/releases/download/v2.0.0/svcmgr-linux-x86_64 -o ~/.local/bin/svcmgr
chmod +x ~/.local/bin/svcmgr

# 方法 B：从源码编译
git clone https://github.com/username/svcmgr.git
cd svcmgr
cargo build --release
cp target/release/svcmgr ~/.local/bin/

# 4. 验证 svcmgr 版本
svcmgr --version  # 应为 2.0.0
```

**相关文档**：
- **21-migration-guide.md** §2.3 - mise 安装与配置

---

### 7.3 版本要求

**严重程度**：**Critical**

| 软件 | v1.x 要求 | v2.0 要求 | 兼容性 |
|-----|----------|----------|--------|
| Linux 内核 | 任意版本 | >= 4.15（cgroups v2 需要，可选） | 向后兼容（cgroups 可禁用） |
| systemd | 任意版本 | 不需要 | 不兼容（不再使用） |
| cron | 任意版本 | 不需要 | 不兼容（不再使用） |
| mise | 不需要 | >= 2024.1.0 | 不兼容（新依赖） |
| Rust | 不需要（预编译二进制） | >= 1.75.0（从源码编译） | 不兼容（新依赖） |
| Git | 2.0+ | 2.30.0+ | 部分兼容（版本要求提高） |

**不兼容的系统**：

| 系统 | v1.x | v2.0 | 说明 |
|-----|------|------|------|
| 无 systemd 的 Linux（如 Alpine + OpenRC） | 不支持 | 完全支持 | v2.0 不依赖 systemd |
| macOS | 部分支持 | 部分支持（无 cgroups） | 资源限制功能不可用 |
| Windows | 不支持 | 不支持 | 无计划支持 |

---

## 8. 兼容性矩阵

### 8.1 操作系统支持

| 操作系统 | v1.x 支持 | v2.0 支持 | 功能差异 |
|---------|----------|----------|---------|
| **Ubuntu 22.04+** | 完全支持 | 完全支持 | 所有功能可用（包括 cgroups v2） |
| **Ubuntu 20.04** | 完全支持 | 部分支持 | cgroups v2 需手动启用（默认为 v1） |
| **Debian 12+** | 完全支持 | 完全支持 | 所有功能可用 |
| **Fedora 38+** | 完全支持 | 完全支持 | 所有功能可用 |
| **CentOS 7 / RHEL 7** | 完全支持 | 不支持 | mise 不支持 |
| **Arch Linux** | 完全支持 | 完全支持 | 所有功能可用 |
| **Alpine Linux** | 不支持（无 systemd） | 完全支持 | v2.0 不依赖 systemd，Alpine 可用 |
| **macOS** | 部分支持 | 部分支持 | 无 cgroups，无 systemd 替代 |
| **Windows** | 不支持 | 不支持 | 无计划支持 |

---

### 8.2 mise 版本兼容性

| mise 版本 | v2.0 兼容性 | 说明 |
|----------|-----------|------|
| >= 2024.1.0 | 完全支持 | 推荐版本 |
| 2023.x | 不支持 | 缺少必需特性（如 file-tasks） |
| < 2023.x | 不支持 | 配置格式不兼容 |

**检查 mise 版本**：

```bash
mise --version
# 输出示例：2024.2.17 macos-arm64 (2024-02-17)
```

**升级 mise**：

```bash
mise self-update
```

---

### 8.3 向后兼容性保证

| 类型 | v1.x → v2.0 兼容性 | 说明 |
|-----|-------------------|------|
| **配置文件** | 不兼容 | 需完全重写（systemd 单元 → TOML，crontab → TOML，nginx → TOML） |
| **数据文件** | 兼容 | 日志、状态数据可迁移（需手动复制） |
| **API** | 不兼容（如果 v1.x 有 API） | API 端点完全重新设计 |
| **CLI 命令** | 不兼容 | 命令名和参数完全变更 |
| **插件/扩展** | 不适用 | v1.x 无插件系统 |

**数据迁移路径**：

```bash
# 迁移日志数据（示例）
cp -r ~/.local/share/svcmgr-v1/logs ~/.local/share/svcmgr/logs

# 迁移状态数据（如果有）
cp ~/.local/share/svcmgr-v1/state.json ~/.local/share/svcmgr/state.json
```

---

## 9. 迁移时间线

### 9.1 版本发布计划

| 里程碑 | 预计日期 | 说明 |
|-------|---------|------|
| v2.0.0-alpha.1 | TBD | 内部测试版本 |
| v2.0.0-beta.1 | TBD | 公开测试版本，功能冻结 |
| v2.0.0-rc.1 | TBD | 发布候选版本 |
| v2.0.0（正式版） | TBD | 正式发布 |

---

### 9.2 旧版本支持策略

| 版本 | 维护期 | 安全更新期 | 说明 |
|-----|-------|-----------|------|
| v1.x | v2.0.0 发布后 6 个月 | v2.0.0 发布后 12 个月 | 维护期内修复关键 bug，安全更新期内修复安全漏洞 |

**时间线示例**（假设 v2.0.0 于 2026-06-01 发布）：

```
2026-06-01: v2.0.0 正式发布
2026-12-01: v1.x 维护期结束（不再修复 bug）
2027-06-01: v1.x 安全更新期结束（不再修复安全漏洞）
```

**建议**：
- 在维护期结束前（2026-12-01）完成迁移
- 如果无法及时迁移,至少在安全更新期结束前（2027-06-01）完成迁移

---

### 9.3 过渡期策略

**灰度迁移（推荐）**：

```
Phase 1: 迁移非关键服务（1 周）
  - 选择 1-2 个低流量服务
  - 完整迁移并验证
  - 积累迁移经验

Phase 2: 迁移关键服务（2-4 周）
  - 逐个迁移生产服务
  - 每个服务迁移后观察 24-48 小时
  - 保留旧配置作为备份

Phase 3: 清理旧配置（1 周）
  - 禁用所有 systemd 服务
  - 清空 crontab
  - 停用 nginx（如果使用内置代理）
```

**并行运行**：

在迁移期间，v1.x 和 v2.0 可以并行运行：

```bash
# v1.x 服务继续运行
systemctl --user status api-v1.service

# v2.0 服务在不同端口运行（测试阶段）
svcmgr service start api-v2

# 验证 v2.0 服务正常后，停止 v1.x 服务
systemctl --user stop api-v1.service
systemctl --user disable api-v1.service
```

**相关文档**：
- **21-migration-guide.md** §5 - 灰度迁移策略

---

## 10. 不兼容的功能

### 10.1 systemd 特性不再支持

**严重程度**：**Major**

以下 systemd 特性在 v2.0 中无法实现：

| systemd 特性 | v2.0 状态 | 替代方案 |
|-------------|----------|---------|
| **Socket Activation** | 不支持 | 服务显式监听端口（无按需激活） |
| **D-Bus Activation** | 不支持 | 手动启动服务 |
| **systemd 依赖顺序**（`Before=`, `After=`） | 不支持 | 使用 mise 任务依赖 `depends`（语义不同：执行依赖而非启动顺序） |
| **systemd Wants/Requires** | 不支持 | 手动管理服务启动顺序 |
| **systemd Timer**（systemd 原生定时任务） | 不支持 | 使用 svcmgr scheduled_tasks |
| **systemd ConditionPathExists** | 不支持 | 在任务脚本中手动检查 |
| **systemd SuccessExitStatus** | 不支持 | 所有非零退出码视为失败 |

**Socket Activation 示例**：

```ini
# v1.x：systemd socket activation
# api.socket
[Socket]
ListenStream=3000

# api.service
[Service]
ExecStart=/usr/bin/node server.js
# systemd 会在端口 3000 收到连接时启动服务
```

```toml
# v2.0：无 socket activation，服务启动时立即监听端口
[services.api]
task = "api-start"
enable = true  # 开机自启，而非按需启动
```

**迁移建议**：
- Socket Activation：如果服务很少使用且启动慢，考虑保留 systemd socket 激活（v2.0 作为常驻服务可能更简单）
- D-Bus Activation：手动启动依赖的 D-Bus 服务
- 依赖顺序：重新评估服务依赖关系，使用 mise `depends` 或手动管理启动顺序

---

### 10.2 cron 特性不再支持

**严重程度**：**Minor**

以下 cron 特性在 v2.0 中无法实现：

| cron 特性 | v2.0 状态 | 替代方案 |
|----------|----------|---------|
| **环境变量继承**（cron 继承部分用户环境） | 不支持 | 显式声明到 `[env]` 或 `[tasks.*.env]` |
| **MAILTO**（邮件通知） | 不支持 | 使用日志输出 + 外部监控告警（如 Prometheus Alertmanager） |
| **特殊字符串**（`@daily`, `@hourly`, `@reboot`） | 部分支持 | `@daily` → `0 0 * * *`, `@hourly` → `0 * * * *`, `@reboot` 不支持 |

**MAILTO 替代方案**：

```bash
# 旧方式（v1.x）：crontab
MAILTO=admin@example.com
0 2 * * * /app/scripts/cleanup.py
# 如果脚本失败，cron 会发送邮件给 admin@example.com
```

```toml
# 新方式（v2.0）：使用外部监控告警
[scheduled_tasks.cleanup]
task = "cleanup"
schedule = "0 2 * * *"
# 配合 Prometheus + Alertmanager 或 svcmgr 的通知插件（如果有）
```

**迁移建议**：
- 移除 `MAILTO` 并配置外部监控告警
- 将特殊字符串转换为标准 cron 表达式
- 显式声明所有环境变量

---

### 10.3 nginx 特性不再支持

**严重程度**：**Major**（取决于使用场景）

以下 nginx 特性在 svcmgr 内置代理中无法实现：

| nginx 特性 | v2.0 状态 | 替代方案 |
|-----------|----------|---------|
| **rewrite / redirect** | 不支持 | 在应用层实现重定向 |
| **auth_basic** | 不支持 | 在应用层实现 HTTP Basic Auth 或使用 JWT |
| **add_header** | 不支持 | 在应用层添加响应头 |
| **Lua 脚本** | 不支持 | 重写为 Rust 中间件（需代码开发） |
| **第三方模块**（ngx_cache_purge, ngx_lua_waf） | 不支持 | 评估是否必需，考虑保留 nginx 作为前端代理 |
| **SSL/TLS 终止** | 不支持 | 使用 nginx/Caddy 作为前端代理，或在应用层实现 |
| **负载均衡** | 不支持 | 使用外部负载均衡器（nginx/HAProxy） |
| **缓存** | 不支持 | 使用外部缓存（Varnish/Redis） |

**复杂场景建议**：

| 场景 | 建议 |
|-----|------|
| **简单反向代理**（无 rewrite、认证、SSL） | 使用 svcmgr 内置代理 |
| **中等复杂度**（使用 rewrite、认证、自定义头） | 在应用层实现这些逻辑 |
| **高度复杂**（Lua 脚本、第三方模块、SSL 终止） | 保留 nginx 作为前端代理，svcmgr 仅管理后端服务 |

**保留 nginx 架构示例**：

```
客户端
  ↓
nginx（前端代理，处理 SSL、认证、Lua 脚本）
  ↓
svcmgr 内置代理（后端路由，可选）
  ↓
应用服务（由 svcmgr 管理）
```

**迁移建议**：
- 评估 nginx 配置的复杂度
- 简单场景：完全迁移到 svcmgr 内置代理
- 复杂场景：保留 nginx，仅迁移服务管理部分

**相关文档**：
- **05-web-service.md** - 内置代理功能和限制
- **21-migration-guide.md** §3.3 - nginx 迁移策略

---

## 11. 替代方案

### 11.1 功能缺失的替代方案

| 缺失功能 | v1.x 实现 | v2.0 替代方案 |
|---------|----------|-------------|
| **Socket Activation** | systemd socket 单元 | 服务常驻运行（或保留 systemd socket） |
| **D-Bus Activation** | systemd D-Bus 单元 | 手动启动依赖服务 |
| **MAILTO** | cron 邮件通知 | Prometheus Alertmanager / PagerDuty / Slack 通知 |
| **nginx rewrite** | nginx rewrite 指令 | 应用层 HTTP 重定向（如 Express/Flask 路由） |
| **nginx auth_basic** | nginx auth_basic 指令 | 应用层认证（如 Passport.js/Flask-Login） |
| **SSL/TLS 终止** | nginx ssl 指令 | Caddy（自动 HTTPS）或 Let's Encrypt + nginx 前端代理 |
| **负载均衡** | nginx upstream | 外部负载均衡器（nginx/HAProxy/Traefik） |

**Alertmanager 配置示例**（替代 MAILTO）：

```yaml
# alertmanager.yml
route:
  receiver: email

receivers:
  - name: email
    email_configs:
      - to: admin@example.com
        from: alertmanager@example.com
        smarthost: smtp.example.com:587
        auth_username: alertmanager@example.com
        auth_password: password

# Prometheus 规则（检测任务失败）
groups:
  - name: svcmgr
    rules:
      - alert: TaskFailed
        expr: svcmgr_task_last_exit_code != 0
        for: 1m
        annotations:
          summary: "Task {{ $labels.task }} failed"
```

---

### 11.2 保留旧工具的混合架构

如果无法完全迁移到 v2.0，可以采用混合架构：

**场景 1：保留 nginx 作为前端代理**

```
客户端
  ↓
nginx（SSL、认证、复杂路由）
  ↓
svcmgr 内置代理（可选）
  ↓
应用服务（由 svcmgr v2.0 管理）
```

**场景 2：保留部分 systemd 服务**

```
systemd 服务（关键系统服务，如数据库）
  ↓
svcmgr v2.0（应用服务）
```

**注意事项**：
- 混合架构增加管理复杂度
- 需明确划分职责边界（哪些服务由 systemd 管理，哪些由 svcmgr 管理）
- 长期目标应是完全迁移到 v2.0

---

## 12. 风险评估

### 12.1 迁移风险等级

| 风险类别 | 等级 | 说明 | 缓解措施 |
|---------|-----|------|---------|
| **配置丢失** | 高 | 错误的迁移可能导致配置丢失 | 备份所有配置文件（见 **21-migration-guide.md** §2.4） |
| **服务停机** | 中 | 迁移期间服务可能短暂不可用 | 采用灰度迁移策略（见 **21-migration-guide.md** §5） |
| **数据丢失** | 低 | 日志和状态数据可能丢失 | 迁移前复制数据目录 |
| **依赖问题** | 中 | mise 安装或版本不兼容 | 提前测试 mise 安装（见 §7.2） |
| **功能缺失** | 中-高 | v2.0 不支持某些 systemd/nginx 特性 | 提前评估功能需求（见 §10） |

---

### 12.2 潜在问题和缓解措施

| 潜在问题 | 影响 | 缓解措施 |
|---------|-----|---------|
| **mise 版本不兼容** | 无法启动 svcmgr | 验证 mise 版本 >= 2024.1.0（见 §8.2） |
| **环境变量缺失** | 服务启动失败 | 迁移时列出所有环境变量（见 §6.4） |
| **端口冲突** | 服务无法绑定端口 | 检查端口占用（`lsof -i :3000`） |
| **cgroups 不可用** | 资源限制功能失效 | 禁用资源限制功能（`features.resource_limits = false`） |
| **日志轮转未配置** | 磁盘占满 | 配置 logrotate 或 svcmgr 内置轮转（见 **03-process-manager.md** §4） |
| **回滚失败** | 无法恢复旧配置 | 使用 Git 管理配置变更（见 §12.3） |

---

### 12.3 回滚计划

**迁移前准备**：

```bash
# 1. 备份所有 systemd 服务单元
cp -r ~/.config/systemd/user ~/.config/systemd/user.backup

# 2. 备份 crontab
crontab -l > ~/crontab.backup

# 3. 备份 nginx 配置（如果使用）
cp -r /etc/nginx ~/nginx.backup

# 4. 备份数据目录
cp -r ~/.local/share/svcmgr ~/.local/share/svcmgr.backup
```

**回滚步骤**：

```bash
# 1. 停止 svcmgr v2.0
svcmgr stop  # 或 killall svcmgr

# 2. 恢复 systemd 服务
cp -r ~/.config/systemd/user.backup/* ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user start api.service worker.service

# 3. 恢复 crontab
crontab ~/crontab.backup

# 4. 恢复 nginx（如果使用）
sudo cp -r ~/nginx.backup/* /etc/nginx/
sudo nginx -s reload

# 5. 验证旧服务正常运行
systemctl --user status api.service
crontab -l
```

**相关文档**：
- **21-migration-guide.md** §7 - 回滚计划

---

## 相关文档

- **00-architecture-overview.md** - v2.0 架构概览
- **01-config-design.md** - v2.0 配置文件格式
- **20-implementation-phases.md** - 实施路线图
- **21-migration-guide.md** - 详细迁移步骤和策略
