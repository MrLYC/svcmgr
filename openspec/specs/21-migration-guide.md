# 21 - 从旧架构迁移指南

> 版本：2.0.0-draft
> 状态：设计中

## 目录

1. [迁移概览](#1-迁移概览)
2. [前置条件](#2-前置条件)
3. [配置迁移](#3-配置迁移)
4. [数据迁移](#4-数据迁移)
5. [服务迁移策略](#5-服务迁移策略)
6. [自动化迁移工具](#6-自动化迁移工具)
7. [回滚计划](#7-回滚计划)
8. [验证清单](#8-验证清单)
9. [测试策略](#9-测试策略)
10. [常见问题](#10-常见问题)

---

## 1. 迁移概览

### 1.1 设计目标

本指南提供从现有 systemd+cron 架构到新 mise 统一架构的完整迁移路径，确保：

- **零数据丢失**：所有服务配置、日志、状态数据完整保留
- **最小停机时间**：支持灰度迁移，服务可逐个迁移并验证
- **可回滚**：任何阶段都可快速回退到旧架构
- **平滑过渡**：迁移期间新旧系统可并行运行

### 1.2 迁移范围

**旧架构组件**（将被替换）：
- systemd 用户服务单元（`~/.config/systemd/user/*.service`）
- crontab 定时任务（`crontab -l`）
- nginx 反向代理配置（如果使用）
- 独立的环境变量文件（`.env` 或 systemd EnvironmentFile）

**新架构组件**（迁移目标）：
- `.config/mise/config.toml` - mise 管理的工具、环境变量、任务定义
- `.config/mise/svcmgr/config.toml` - svcmgr 服务和定时任务配置
- 统一调度引擎 - 替代 systemd + cron
- 内置 HTTP 代理 - 替代 nginx（如果使用）

### 1.3 迁移时间估算

| 服务数量 | 预计时间 | 停机时间 | 风险等级 |
|---------|---------|---------|---------|
| 1-3 个服务 | 1-2 小时 | 0-5 分钟（灰度） | 低 |
| 4-10 个服务 | 2-4 小时 | 5-15 分钟（灰度） | 中 |
| 10+ 个服务 | 4-8 小时 | 15-30 分钟（灰度） | 中-高 |

**建议**：优先选择灰度迁移策略，避免一次性迁移所有服务。

---

## 2. 前置条件

### 2.1 系统要求

| 项目 | 最低要求 | 推荐版本 | 验证命令 |
|-----|---------|---------|---------|
| 操作系统 | Linux（任何发行版） | Ubuntu 22.04+ / Fedora 38+ | `uname -a` |
| mise 版本 | 2024.1.0+ | 最新稳定版 | `mise --version` |
| Rust 工具链 | 1.75.0+ | 1.76.0+ | `rustc --version` |
| Git | 2.30.0+ | 2.40.0+ | `git --version` |
| cgroups | v2（可选） | v2 | `mount | grep cgroup` |
| 磁盘空间 | 500 MB | 1 GB | `df -h ~` |
| 内存 | 512 MB | 1 GB | `free -h` |

### 2.2 权限检查

```bash
# 检查用户级 systemd 访问权限
systemctl --user status >/dev/null 2>&1 && echo "✓ systemd 用户服务可用" || echo "✗ systemd 用户服务不可用"

# 检查 crontab 访问权限
crontab -l >/dev/null 2>&1 && echo "✓ crontab 可用" || echo "✗ crontab 不可用"

# 检查 cgroups v2（可选，资源限制功能需要）
if grep -q 'cgroup2' /proc/mounts; then
  echo "✓ cgroups v2 已挂载"
  # 检查无特权用户 cgroup 委派
  if [ -d "/sys/fs/cgroup/user.slice/user-$(id -u).slice" ]; then
    echo "✓ 用户 cgroup 委派已启用"
  else
    echo "⚠ 用户 cgroup 委派未启用（资源限制功能不可用）"
  fi
else
  echo "⚠ cgroups v2 未挂载（资源限制功能不可用）"
fi
```

### 2.3 mise 安装与配置

如果尚未安装 mise：

```bash
# 方法 1：使用官方安装脚本
curl https://mise.run | sh

# 方法 2：使用包管理器（推荐）
# Ubuntu/Debian
sudo apt install mise

# Fedora
sudo dnf install mise

# macOS
brew install mise

# 验证安装
mise --version

# 激活 mise（添加到 shell 配置）
echo 'eval "$(mise activate bash)"' >> ~/.bashrc   # Bash
echo 'eval "$(mise activate zsh)"' >> ~/.zshrc    # Zsh
source ~/.bashrc  # 或 source ~/.zshrc
```

### 2.4 备份现有配置

**关键：在迁移前务必备份所有配置和数据！**

```bash
# 创建备份目录
mkdir -p ~/svcmgr-migration-backup/$(date +%Y%m%d-%H%M%S)
BACKUP_DIR=~/svcmgr-migration-backup/$(date +%Y%m%d-%H%M%S)

# 备份 systemd 服务单元
mkdir -p "$BACKUP_DIR/systemd"
cp -r ~/.config/systemd/user/*.service "$BACKUP_DIR/systemd/" 2>/dev/null || echo "无 systemd 服务"

# 备份 crontab
crontab -l > "$BACKUP_DIR/crontab.txt" 2>/dev/null || echo "无 crontab 任务"

# 备份 nginx 配置（如果使用）
if [ -d ~/.config/nginx ]; then
  cp -r ~/.config/nginx "$BACKUP_DIR/nginx"
fi

# 备份环境变量文件
find ~ -maxdepth 2 -name ".env*" -exec cp {} "$BACKUP_DIR/" \;

# 备份旧 svcmgr 配置（如果存在）
if [ -d ~/.local/share/svcmgr ]; then
  cp -r ~/.local/share/svcmgr "$BACKUP_DIR/svcmgr"
fi

# 创建备份清单
cat > "$BACKUP_DIR/backup-manifest.txt" <<EOF
备份时间: $(date)
主机名: $(hostname)
用户: $(whoami)
备份内容:
- systemd 服务单元: $(ls -1 "$BACKUP_DIR/systemd" 2>/dev/null | wc -l) 个文件
- crontab 任务: $(grep -c '^[^#]' "$BACKUP_DIR/crontab.txt" 2>/dev/null || echo 0) 个任务
- nginx 配置: $([ -d "$BACKUP_DIR/nginx" ] && echo "已备份" || echo "无")
- 环境变量文件: $(ls -1 "$BACKUP_DIR"/.env* 2>/dev/null | wc -l) 个文件
EOF

echo "✓ 备份完成: $BACKUP_DIR"
cat "$BACKUP_DIR/backup-manifest.txt"
```

---

## 3. 配置迁移

### 3.1 systemd 服务 → svcmgr 服务

#### 3.1.1 字段映射表

| systemd 字段 | svcmgr 等效配置 | 说明 |
|-------------|----------------|------|
| `ExecStart=` | `[tasks.<name>] run =` + `[services.<name>] task =` | 命令在 mise 任务中定义，服务引用任务 |
| `WorkingDirectory=` | `[tasks.<name>] dir =` | 工作目录在任务中定义 |
| `Environment=` | `[tasks.<name>] env =` 或 `[env]` | 环境变量可在任务、服务或全局定义 |
| `EnvironmentFile=` | `[env] _.file =` | mise 支持从文件加载环境变量 |
| `Restart=always` | `[services.<name>] restart = "always"` | 重启策略 |
| `RestartSec=5` | `[services.<name>] restart_delay = "5s"` | 重启延迟（指数退避初始值） |
| `StandardOutput=` | `[services.<name>] stdout =` | 日志输出路径 |
| `StandardError=` | `[services.<name>] stderr =` | 错误输出路径 |
| `User=` | N/A | svcmgr 仅支持用户级服务，不涉及用户切换 |
| `WantedBy=default.target` | `[services.<name>] enable = true` | 开机自启 |

#### 3.1.2 迁移示例：Web API 服务

**旧配置**（systemd 服务单元 `~/.config/systemd/user/api.service`）：

```ini
[Unit]
Description=API Server
After=network.target

[Service]
Type=simple
ExecStart=/usr/bin/node /home/user/app/server.js
WorkingDirectory=/home/user/app
Environment="NODE_ENV=production"
Environment="PORT=3000"
EnvironmentFile=/home/user/app/.env
Restart=always
RestartSec=5
StandardOutput=append:/home/user/.local/share/api/logs/stdout.log
StandardError=append:/home/user/.local/share/api/logs/stderr.log

[Install]
WantedBy=default.target
```

**新配置**（mise 任务 + svcmgr 服务）：

```toml
# .config/mise/config.toml（mise 配置）

[tools]
node = "22"

[env]
NODE_ENV = "production"
_.file = "/home/user/app/.env"  # 从文件加载环境变量

[tasks.api-start]
description = "Start API server"
run = "node server.js"
dir = "/home/user/app"
env = { PORT = "3000" }
```

```toml
# .config/mise/svcmgr/config.toml（svcmgr 配置）

[services.api]
task = "api-start"           # 引用 mise 任务
enable = true                # 开机自启
restart = "always"           # 总是重启
restart_delay = "5s"         # 重启延迟
restart_limit = 10           # 1分钟内最多重启10次
restart_window = "60s"
stop_timeout = "10s"         # 优雅停止超时
stdout = "/home/user/.local/share/api/logs/stdout.log"
stderr = "/home/user/.local/share/api/logs/stderr.log"

[services.api.health]
type = "tcp"
address = "127.0.0.1:3000"
interval = "10s"
timeout = "5s"
retries = 3
```

#### 3.1.3 迁移命令

```bash
# 1. 停止旧服务（迁移时）
systemctl --user stop api.service
systemctl --user disable api.service

# 2. 创建 mise 配置
cat > ~/.config/mise/config.toml <<'EOF'
[tools]
node = "22"

[env]
NODE_ENV = "production"
_.file = "/home/user/app/.env"

[tasks.api-start]
description = "Start API server"
run = "node server.js"
dir = "/home/user/app"
env = { PORT = "3000" }
EOF

# 3. 创建 svcmgr 配置
mkdir -p ~/.config/mise/svcmgr
cat > ~/.config/mise/svcmgr/config.toml <<'EOF'
[services.api]
task = "api-start"
enable = true
restart = "always"
restart_delay = "5s"
restart_limit = 10
restart_window = "60s"
stop_timeout = "10s"
stdout = "/home/user/.local/share/api/logs/stdout.log"
stderr = "/home/user/.local/share/api/logs/stderr.log"

[services.api.health]
type = "tcp"
address = "127.0.0.1:3000"
interval = "10s"
timeout = "5s"
retries = 3
EOF

# 4. 安装依赖
mise install

# 5. 测试任务（前台运行）
mise run api-start

# 6. 启动 svcmgr（后续步骤，见第 5 节）
```

---

### 3.2 crontab → scheduled_tasks

#### 3.2.1 字段映射表

| crontab 字段 | svcmgr 等效配置 | 说明 |
|-------------|----------------|------|
| Cron 表达式 | `[scheduled_tasks.<name>] schedule =` | 使用标准 cron 语法 |
| 命令 | `[tasks.<name>] run =` + `[scheduled_tasks.<name>] task =` | 命令在 mise 任务中定义 |
| 工作目录 | `[tasks.<name>] dir =` | 在任务中定义 |
| 环境变量 | `[tasks.<name>] env =` | 在任务中定义 |
| 标准输出重定向 | `[scheduled_tasks.<name>] stdout =` | 日志路径 |

#### 3.2.2 迁移示例：数据清理任务

**旧配置**（crontab）：

```cron
# 每天凌晨 2 点执行数据清理
0 2 * * * cd /home/user/app && /usr/bin/python scripts/cleanup.py >> /home/user/.local/share/cleanup/logs/cleanup.log 2>&1
```

**新配置**：

```toml
# .config/mise/config.toml

[tools]
python = "3.12"

[tasks.cleanup]
description = "Cleanup old data"
run = "python scripts/cleanup.py"
dir = "/home/user/app"
env = { LOG_LEVEL = "INFO" }
```

```toml
# .config/mise/svcmgr/config.toml

[scheduled_tasks.cleanup]
task = "cleanup"
schedule = "0 2 * * *"  # 每天凌晨 2 点
enable = true
stdout = "/home/user/.local/share/cleanup/logs/cleanup.log"
stderr = "/home/user/.local/share/cleanup/logs/cleanup.log"
timeout = "1h"          # 任务超时时间
max_history = 30        # 保留最近 30 次执行记录
```

#### 3.2.3 迁移命令

```bash
# 1. 导出现有 crontab
crontab -l > ~/crontab-backup.txt

# 2. 分析 crontab 条目
# 对于每个条目，提取：
# - Cron 表达式
# - 命令和参数
# - 工作目录（如果有 cd 命令）
# - 输出重定向路径

# 3. 创建 mise 任务（见上文示例）

# 4. 创建 svcmgr 定时任务（见上文示例）

# 5. 移除旧 crontab 条目（验证后）
crontab -r  # 删除所有 crontab 任务
# 或者逐个注释掉
crontab -e  # 编辑并注释掉已迁移的任务
```

---

### 3.3 nginx → 内置 HTTP 代理

#### 3.3.1 字段映射表

| nginx 配置 | svcmgr 等效配置 | 说明 |
|-----------|----------------|------|
| `location /api { proxy_pass http://127.0.0.1:3000; }` | `[[http.routes]]` | 路径路由 |
| `server_name api.example.com` | `[[http.routes]] host =` | 主机路由 |
| `proxy_set_header` | `[[http.routes]] headers =` | 请求头设置 |
| `upstream` | 不需要 | svcmgr 自动管理后端服务 |
| `ssl_certificate` | `[http.tls]` | TLS 配置 |
| `root /var/www` | `[http.static]` | 静态文件服务 |

#### 3.3.2 迁移示例：API 反向代理

**旧配置**（nginx `~/.config/nginx/nginx.conf`）：

```nginx
server {
    listen 8080;
    server_name api.example.com;

    location /api {
        proxy_pass http://127.0.0.1:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }

    location /static {
        root /home/user/app/public;
        try_files $uri $uri/ =404;
    }
}
```

**新配置**：

```toml
# .config/mise/svcmgr/config.toml

[http]
listen = "0.0.0.0:8080"

[[http.routes]]
path = "/api"
service = "api"         # 引用服务名
strip_prefix = false    # 不移除路径前缀
headers = [
  { name = "X-Forwarded-For", value = "$remote_addr" },
  { name = "X-Forwarded-Proto", value = "$scheme" }
]

[[http.routes]]
path = "/static"
type = "static"
root = "/home/user/app/public"
index = ["index.html"]
```

**说明**：
- `service = "api"` 自动路由到 `[services.api]` 定义的服务
- svcmgr 自动管理服务启停时的路由更新，无需重启代理
- 内置代理支持 WebSocket、静态文件、TLS 等功能

#### 3.3.3 迁移命令

```bash
# 1. 停止 nginx（如果运行中）
systemctl --user stop nginx.service
systemctl --user disable nginx.service

# 2. 分析现有 nginx 配置
# 提取：
# - 监听端口
# - 路由规则（location 块）
# - 代理目标（proxy_pass）
# - 静态文件路径（root）
# - TLS 配置（ssl_certificate）

# 3. 创建 svcmgr HTTP 配置（见上文示例）

# 4. 测试配置
svcmgr config validate

# 5. 启动 svcmgr（包含内置代理）
svcmgr start
```

---

### 3.4 环境变量迁移

#### 3.4.1 迁移策略

旧架构中的环境变量可能分散在多个位置：
- systemd `Environment=` 字段
- systemd `EnvironmentFile=` 引用的文件
- Shell 配置文件（`.bashrc`, `.zshrc`）
- 独立的 `.env` 文件

新架构支持 3 种作用域：
1. **全局作用域**：`[env]` - 所有任务和服务共享
2. **任务作用域**：`[tasks.<name>] env =` - 特定任务
3. **服务作用域**：`[services.<name>] env =` - 特定服务（优先级最高）

#### 3.4.2 迁移示例

**旧配置**（多种来源）：

```bash
# systemd 服务单元
Environment="NODE_ENV=production"
Environment="DATABASE_URL=postgres://localhost/mydb"
EnvironmentFile=/home/user/app/.env

# .env 文件
API_KEY=secret123
LOG_LEVEL=info
```

**新配置**：

```toml
# .config/mise/config.toml

[env]
NODE_ENV = "production"
DATABASE_URL = "postgres://localhost/mydb"
_.file = "/home/user/app/.env"  # 从 .env 文件加载

[tasks.api-start]
run = "node server.js"
env = { PORT = "3000" }  # 任务级环境变量

[tasks.worker-run]
run = "python worker.py"
env = { WORKERS = "4" }
```

```toml
# .config/mise/svcmgr/config.toml

[services.api]
task = "api-start"
env = { DEBUG = "false" }  # 服务级环境变量（最高优先级）
```

**环境变量优先级**（从高到低）：
1. `[services.<name>] env =` - 服务级
2. `[tasks.<name>] env =` - 任务级
3. `[env]` - 全局级
4. 系统环境变量

#### 3.4.3 敏感信息处理

**不要将敏感信息直接写入配置文件！** 使用以下方法：

1. **从文件加载**（推荐）：

```toml
[env]
_.file = "/home/user/app/.env.secret"  # 不提交到 Git
```

2. **从命令输出加载**：

```toml
[env]
_.source = "pass show api/database-url"  # 使用密码管理器
```

3. **使用模板变量**：

```toml
[env]
DATABASE_URL = "postgres://{{env.DB_USER}}:{{env.DB_PASSWORD}}@localhost/mydb"
```

然后在 `.env.secret` 中定义 `DB_USER` 和 `DB_PASSWORD`。

---

## 4. 数据迁移

### 4.1 服务日志迁移

#### 4.1.1 日志路径变更

| 旧架构 | 新架构 | 说明 |
|-------|-------|------|
| journalctl 日志 | 文件日志 | svcmgr 使用文件日志，不依赖 systemd |
| 系统日志目录 | 用户日志目录 | 默认：`~/.local/share/svcmgr/logs/` |

#### 4.1.2 迁移步骤

```bash
# 1. 导出现有 systemd 日志
mkdir -p ~/.local/share/svcmgr/logs-migration

for service in api worker; do
  echo "导出 $service 服务日志..."
  journalctl --user -u "$service.service" --no-pager > \
    ~/.local/share/svcmgr/logs-migration/"$service-$(date +%Y%m%d).log"
done

# 2. 在 svcmgr 配置中指定日志路径
# （见 3.1.2 节示例）

# 3. 验证新日志文件正常写入
tail -f ~/.local/share/svcmgr/logs/api-stdout.log
```

### 4.2 PID 文件清理

systemd 管理的服务可能在 `/run/user/$(id -u)/` 创建 PID 文件。迁移后这些文件无用，可安全删除：

```bash
# 列出 PID 文件
find /run/user/$(id -u) -name "*.pid" -type f

# 删除（确保服务已停止）
find /run/user/$(id -u) -name "*.pid" -type f -delete
```

### 4.3 状态数据迁移

如果旧服务使用特定目录存储状态数据（数据库文件、缓存等），确保：

1. **工作目录正确**：在 `[tasks.<name>] dir =` 中指定
2. **文件权限正确**：确保 svcmgr 进程有读写权限
3. **路径一致**：如果服务硬编码了路径，可能需要符号链接

```bash
# 示例：迁移 SQLite 数据库
OLD_PATH="/home/user/.local/share/api/data.db"
NEW_PATH="/home/user/app/data/data.db"

# 如果服务硬编码了旧路径，创建符号链接
ln -s "$NEW_PATH" "$OLD_PATH"
```

### 4.4 配置文件 Git 历史保留

如果旧架构已使用 Git 管理配置，可保留历史：

```bash
# 1. 初始化新配置仓库（如果未初始化）
cd ~/.config/mise/svcmgr
git init

# 2. 导入旧仓库历史（可选）
cd ~/.config/mise/svcmgr
git remote add old-svcmgr /path/to/old/svcmgr/config/repo
git fetch old-svcmgr
git merge --allow-unrelated-histories old-svcmgr/main

# 3. 创建迁移提交
git add config.toml conf.d/
git commit -m "migrate: convert from systemd+cron to mise-based architecture

- Migrate 3 systemd services to [services.*]
- Migrate 2 crontab tasks to [scheduled_tasks.*]
- Centralize environment variables in mise [env]
- Replace nginx with built-in HTTP proxy"
```

---

## 5. 服务迁移策略

### 5.1 灰度迁移（推荐）

逐个服务迁移并验证，旧服务保持运行作为回退方案。

#### 5.1.1 迁移流程

```
阶段 1: 准备
├── 备份所有配置
├── 安装 mise 和依赖
└── 创建新配置（不启动）

阶段 2: 迁移第一个服务（非关键服务）
├── 停止旧服务（systemd）
├── 启动新服务（svcmgr）
├── 验证功能（健康检查、日志）
├── 监控 1-2 小时
└── 如有问题，立即回滚

阶段 3: 迁移剩余服务（逐个）
├── 对每个服务重复阶段 2
├── 间隔 30 分钟到 1 小时
└── 验证服务间通信正常

阶段 4: 清理
├── 停用所有旧服务
├── 移除 systemd 单元文件
├── 移除 crontab 任务
└── 卸载 nginx（如果不再需要）
```

#### 5.1.2 迁移命令示例

```bash
#!/bin/bash
# 灰度迁移脚本示例

SERVICE_NAME="api"

echo "=== 灰度迁移：$SERVICE_NAME ==="

# 1. 停止旧服务
echo "1. 停止旧服务..."
systemctl --user stop "$SERVICE_NAME.service"
systemctl --user disable "$SERVICE_NAME.service"

# 2. 启动新服务
echo "2. 启动新服务..."
svcmgr service start "$SERVICE_NAME"

# 3. 验证启动
echo "3. 验证服务状态..."
sleep 5
if svcmgr service status "$SERVICE_NAME" | grep -q "running"; then
  echo "✓ 服务启动成功"
else
  echo "✗ 服务启动失败，回滚..."
  svcmgr service stop "$SERVICE_NAME"
  systemctl --user start "$SERVICE_NAME.service"
  exit 1
fi

# 4. 健康检查
echo "4. 执行健康检查..."
if curl -f http://localhost:3000/health > /dev/null 2>&1; then
  echo "✓ 健康检查通过"
else
  echo "✗ 健康检查失败，回滚..."
  svcmgr service stop "$SERVICE_NAME"
  systemctl --user start "$SERVICE_NAME.service"
  exit 1
fi

# 5. 监控日志
echo "5. 监控日志（Ctrl+C 退出）..."
svcmgr service logs "$SERVICE_NAME" --follow
```

### 5.2 一次性迁移（高风险）

适用于开发/测试环境或可接受短暂停机的场景。

#### 5.2.1 迁移流程

```bash
#!/bin/bash
# 一次性迁移脚本示例

echo "=== 一次性迁移所有服务 ==="
echo "⚠ 警告：此操作将停止所有服务"
read -p "继续？[y/N] " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
  exit 1
fi

# 1. 停止所有旧服务
echo "1. 停止所有 systemd 服务..."
systemctl --user stop api.service worker.service
systemctl --user disable api.service worker.service

# 2. 移除 crontab 任务
echo "2. 移除 crontab 任务..."
crontab -r

# 3. 启动 svcmgr
echo "3. 启动 svcmgr..."
svcmgr start

# 4. 验证所有服务
echo "4. 验证服务状态..."
sleep 10
svcmgr service list

# 5. 检查健康状态
echo "5. 检查健康状态..."
for service in api worker; do
  if svcmgr service status "$service" | grep -q "running"; then
    echo "✓ $service: 运行中"
  else
    echo "✗ $service: 未运行"
  fi
done

echo "=== 迁移完成 ==="
echo "请监控日志确保服务正常运行"
```

### 5.3 并行运行注意事项

灰度迁移期间，新旧系统会并行运行，需注意：

#### 5.3.1 端口冲突

**问题**：systemd 服务和 svcmgr 服务可能绑定相同端口。

**解决方案**：
1. 迁移前停止旧服务（推荐）
2. 临时更改新服务端口，验证后切换

```toml
# 临时使用不同端口
[tasks.api-start]
run = "node server.js"
env = { PORT = "3001" }  # 临时端口

# 验证后改回
env = { PORT = "3000" }
```

#### 5.3.2 资源竞争

**问题**：新旧服务可能竞争 CPU、内存、磁盘 I/O。

**解决方案**：
- 监控系统资源使用率：`htop`, `iostat`
- 逐个迁移服务，避免同时运行过多服务
- 使用 cgroups 资源限制（新服务）

```toml
[services.api]
task = "api-start"

[services.api.resources]
cpu_quota = "50%"        # 限制 CPU 使用
memory_limit = "512MB"   # 限制内存使用
```

#### 5.3.3 日志路径冲突

**问题**：新旧服务可能写入相同日志文件。

**解决方案**：使用不同的日志路径

```toml
[services.api]
stdout = "/home/user/.local/share/svcmgr/logs/api-stdout.log"  # 新路径
stderr = "/home/user/.local/share/svcmgr/logs/api-stderr.log"
```

#### 5.3.4 数据文件冲突

**问题**：新旧服务可能同时访问数据库或共享文件。

**解决方案**：
- 确保服务互斥运行（不能同时启动）
- 使用文件锁机制
- 数据库连接使用唯一标识符

---

## 6. 自动化迁移工具

### 6.1 迁移工具概览

svcmgr 提供自动化迁移工具，简化迁移流程：

```bash
svcmgr migrate <subcommand>
```

| 子命令 | 功能 | 说明 |
|-------|------|------|
| `init` | 初始化迁移环境 | 创建配置目录、备份旧配置 |
| `analyze` | 分析现有配置 | 扫描 systemd/crontab/nginx，生成迁移报告 |
| `convert` | 转换配置 | 自动生成 mise 和 svcmgr 配置文件 |
| `verify` | 验证配置 | 检查配置文件语法和依赖 |
| `apply` | 应用迁移 | 停止旧服务，启动新服务 |
| `rollback` | 回滚迁移 | 恢复到旧架构 |

### 6.2 使用示例

#### 6.2.1 完整迁移流程

```bash
# 1. 初始化迁移环境
svcmgr migrate init
# 输出：
# ✓ 创建备份目录: ~/.local/share/svcmgr/migration/backup-20260223-110000
# ✓ 备份 systemd 服务: 3 个文件
# ✓ 备份 crontab 任务: 2 个任务
# ✓ 创建配置目录: ~/.config/mise/svcmgr

# 2. 分析现有配置
svcmgr migrate analyze
# 输出：
# === 迁移分析报告 ===
# systemd 服务:
#   - api.service → [services.api]
#   - worker.service → [services.worker]
#   - nginx.service → [http] (内置代理)
# crontab 任务:
#   - 0 2 * * * cleanup → [scheduled_tasks.cleanup]
#   - */15 * * * * healthcheck → [scheduled_tasks.healthcheck]
# 环境变量来源:
#   - api.service Environment: 2 个变量
#   - /home/user/app/.env: 5 个变量
# 建议操作:
#   - 创建 3 个 mise 任务
#   - 创建 2 个 svcmgr 服务
#   - 创建 2 个定时任务
#   - 配置内置 HTTP 代理

# 3. 转换配置（生成新配置文件）
svcmgr migrate convert
# 输出：
# ✓ 生成 ~/.config/mise/config.toml
# ✓ 生成 ~/.config/mise/svcmgr/config.toml
# ⚠ 请人工审查生成的配置文件

# 4. 审查生成的配置
cat ~/.config/mise/config.toml
cat ~/.config/mise/svcmgr/config.toml
# 根据需要手工调整

# 5. 验证配置
svcmgr migrate verify
# 输出：
# ✓ mise 配置语法正确
# ✓ svcmgr 配置语法正确
# ✓ 所有引用的任务存在
# ✓ 依赖工具已安装
# ✗ 警告：端口 3000 当前被占用（api.service）
# 建议：先停止 api.service 再应用迁移

# 6. 应用迁移（灰度模式：逐个服务）
svcmgr migrate apply --service api --mode gradual
# 输出：
# 1. 停止 api.service...
# 2. 启动 svcmgr 服务 api...
# 3. 验证健康检查...
# ✓ 服务迁移成功
# ⚠ 建议监控 10 分钟后再迁移下一个服务

# 等待 10 分钟，监控日志
svcmgr service logs api --follow

# 继续迁移其他服务
svcmgr migrate apply --service worker --mode gradual

# 7. 最终验证
svcmgr migrate verify --post-migration
# 输出：
# ✓ 所有服务运行正常
# ✓ 所有定时任务已调度
# ✓ HTTP 代理正常工作
# ✓ 健康检查全部通过

# 8. 清理旧配置（可选）
svcmgr migrate cleanup
# 输出：
# ⚠ 此操作将删除所有旧配置（systemd 单元、crontab）
# 请确认已完全验证新系统正常运行
# 继续？[y/N] y
# ✓ 移除 systemd 单元文件
# ✓ 清空 crontab
# ✓ 备份已保存到: ~/.local/share/svcmgr/migration/backup-20260223-110000
```

#### 6.2.2 回滚示例

```bash
# 如果迁移后发现问题，立即回滚
svcmgr migrate rollback

# 输出：
# === 回滚迁移 ===
# 1. 停止所有 svcmgr 服务...
# 2. 恢复 systemd 服务单元...
# 3. 恢复 crontab 任务...
# 4. 启动旧服务...
# ✓ 回滚完成
# 旧服务状态:
#   - api.service: active (running)
#   - worker.service: active (running)
```

### 6.3 迁移工具配置

迁移工具行为可通过配置文件或命令行参数定制：

```toml
# ~/.config/svcmgr/migration.toml

[migration]
backup_dir = "~/.local/share/svcmgr/migration/backups"
dry_run = false          # true: 仅生成配置，不实际应用
mode = "gradual"         # gradual | immediate
interval = "10m"         # 灰度迁移间隔

[conversion]
preserve_comments = true # 保留原配置注释
auto_detect_ports = true # 自动检测端口配置
generate_health_checks = true  # 自动生成健康检查

[verification]
check_dependencies = true      # 检查依赖工具
check_port_conflicts = true    # 检查端口冲突
check_file_permissions = true  # 检查文件权限
```

---

## 7. ttyd 迁移指南

### 7.1 迁移概览

**变更**:ttyd (Web 终端) 从独立功能模块迁移到基于 mise 任务的配置驱动模式

**迁移原则**:
- mise 安装 ttyd 工具
- mise 任务定义启动命令和参数
- svcmgr 服务管理进程生命周期

**代码影响**:
- ❌ 移除独立模块:`features/webtty.rs` (300行)
- ❌ 移除 CLI 命令:`svcmgr tty create/start/stop/delete`
- ✅ 新管理方式:通过 `svcmgr service *` 统一管理

---

### 7.2 配置迁移示例

#### 7.2.1 旧配置方式(v1.x - 独立模块)

**通过 CLI 命令创建**:
```bash
svcmgr tty create --name bash --port 9001 --command bash
svcmgr tty start bash
```

**生成的独立配置段**(内部格式):
```toml
[tty.bash]
port = 9001
command = "bash"
title = "Bash Terminal"
```

#### 7.2.2 新配置方式(v2.0 - mise 任务 + svcmgr 服务)

**Step 1: 在 mise 配置中定义工具和任务**

编辑 `~/.config/mise/config.toml`:
```toml
# 1. 安装 ttyd 工具
[tools]
ttyd = "1.7.7"

# 2. 定义 Web 终端任务
[tasks.tty-bash]
run = "ttyd -p 9001 -t titleFixed='Bash' bash"

[tasks.tty-python]
run = "ttyd -p 9002 -t titleFixed='Python REPL' python3"
```

**Step 2: 在 svcmgr 配置中定义服务**

编辑 `~/.config/mise/svcmgr/config.toml`:
```toml
# 1. 定义服务(引用 mise 任务)
[services.tty-bash]
task = "tty-bash"              # 引用 mise 任务
enable = true                   # 开机自启
restart = "always"             # 自动重启策略
ports = { terminal = 9001 }     # 端口定义

[services.tty-python]
task = "tty-python"
enable = true
ports = { terminal = 9002 }

# 2. 配置 HTTP 路由(访问 Web 终端)
[[http.routes]]
path = "/tty/bash"
target = "service:tty-bash:terminal"  # 代理到服务端口
websocket = true                       # 启用 WebSocket 支持

[[http.routes]]
path = "/tty/python"
target = "service:tty-python:terminal"
websocket = true
```

---

### 7.3 迁移步骤

#### Step 1: 安装 ttyd 工具

```bash
# 方法 1: 通过 mise 配置安装
echo 'ttyd = "1.7.7"' >> ~/.config/mise/config.toml
mise install ttyd@1.7.7

# 方法 2: 直接安装
mise use ttyd@1.7.7

# 验证安装
which ttyd
# 输出: /home/user/.local/share/mise/installs/ttyd/1.7.7/bin/ttyd
```

#### Step 2: 创建 mise 任务定义

编辑 `~/.config/mise/config.toml`,添加任务:
```toml
[tasks.tty-bash]
run = "ttyd -p 9001 -t titleFixed='Bash' bash"
```

**验证任务定义**:
```bash
mise tasks
# 输出应包含: tty-bash

# 测试任务执行
mise run tty-bash
# 输出: [INFO] ttyd 1.7.7 listening on port 9001
# Ctrl+C 停止测试
```

#### Step 3: 创建 svcmgr 服务配置

编辑 `~/.config/mise/svcmgr/config.toml`,添加服务和路由:
```toml
[services.tty-bash]
task = "tty-bash"
enable = true
ports = { terminal = 9001 }

[[http.routes]]
path = "/tty/bash"
target = "service:tty-bash:terminal"
websocket = true
```

#### Step 4: 启动服务

```bash
# 启动服务
svcmgr service start tty-bash

# 输出:
# ✓ Service 'tty-bash' started
# ✓ Listening on port 9001
# ✓ HTTP route '/tty/bash' configured
```

#### Step 5: 验证服务

```bash
# 检查服务状态
svcmgr service status tty-bash

# 测试 HTTP 路由
curl -I http://localhost/tty/bash
# 预期输出: HTTP/1.1 101 Switching Protocols(WebSocket 握手)

# 浏览器访问
# 打开 http://localhost/tty/bash,应看到 Bash 终端界面
```

---

### 7.4 破坏性变更

#### 7.4.1 移除的 CLI 命令

以下命令在 v2.0 中**完全移除**:

| 旧命令(v1.x) | 新命令(v2.0) | 说明 |
|---------------|---------------|------|
| `svcmgr tty create <name>` | 配置 `.config/mise/config.toml` 添加任务 + 服务 | 创建 Web 终端 |
| `svcmgr tty start <name>` | `svcmgr service start <name>` | 启动 Web 终端 |
| `svcmgr tty stop <name>` | `svcmgr service stop <name>` | 停止 Web 终端 |
| `svcmgr tty delete <name>` | 从配置文件删除任务和服务定义 | 删除 Web 终端 |

#### 7.4.2 配置位置变更

| 项目 | 旧位置(v1.x) | 新位置(v2.0) |
|-----|---------------|---------------|
| 工具安装 | 手动安装或系统包管理器 | `~/.config/mise/config.toml` [tools] |
| 启动命令 | CLI 参数或独立配置 | `~/.config/mise/config.toml` [tasks] |
| 服务配置 | 内部配置段 `[tty.*]` | `~/.config/mise/svcmgr/config.toml` [services] |
| HTTP 路由 | 独立路由配置 | `~/.config/mise/svcmgr/config.toml` [[http.routes]] |

#### 7.4.3 管理方式变更

- ✅ **保留功能**:所有 Web 终端功能完全保留(ttyd, WebSocket, HTTP 代理)
- ❌ **移除独立模块**:`features/webtty.rs` (300行) 完全移除
- 🔄 **管理方式变更**:从独立 CLI 命令迁移到统一服务管理
- 🔄 **配置方式变更**:从 CLI 参数迁移到配置文件驱动

**迁移建议**:
- 更新所有使用 `svcmgr tty *` 命令的脚本和文档
- 使用 Git 版本控制管理新配置文件
- 测试 WebSocket 连接是否正常工作

**相关文档**:
- **22-breaking-changes.md** §4.1 - CLI 命令移除清单
- **22-breaking-changes.md** §3.4 - 配置格式变更
- `docs/DESIGN_TTY_CLOUDFLARED.md` - 详细设计决策

---

## 8. cloudflared 迁移指南

### 8.1 迁移概览

**变更**:cloudflared (Cloudflare 隧道) 从独立原子模块迁移到基于 mise 任务的配置驱动模式

**迁移原则**:
- mise 安装 cloudflared 工具
- mise 任务定义隧道操作(create, route-dns, run)
- svcmgr 服务管理隧道进程生命周期

**代码影响**:
- ❌ 移除独立模块:`atoms/tunnel.rs` (865行)
- 🔄 可选便捷命令:`cli/tunnel.rs` (约 200行) - 如需保留简化的 CLI 体验
- ✅ 代码减少:**83%**(1165行 → 200行)

---

### 8.2 配置迁移示例

#### 8.2.1 旧配置方式(v1.x - 独立原子模块)

**独立配置段**:
```toml
# ~/.config/svcmgr/config.toml
[tunnel]
id = "abc123"
name = "my-tunnel"
credentials_file = "/home/user/.cloudflared/abc123.json"

[[tunnel.ingress]]
hostname = "app.example.com"
service = "http://localhost:3000"

[[tunnel.ingress]]
hostname = "api.example.com"
service = "http://localhost:8000"

[[tunnel.ingress]]
service = "http_status:404"  # 默认规则
```

**管理方式**:
```bash
svcmgr tunnel create my-tunnel
svcmgr tunnel route-dns abc123 app.example.com
svcmgr tunnel start
```

#### 8.2.2 新配置方式(v2.0 - mise 任务 + svcmgr 服务)

**Step 1: 在 mise 配置中定义工具和任务**

编辑 `~/.config/mise/config.toml`:
```toml
# 1. 安装 cloudflared 工具
[tools]
cloudflared = "latest"

# 2. 定义环境变量
[env]
TUNNEL_ID = "abc123"
TUNNEL_NAME = "my-tunnel"
TUNNEL_CREDENTIALS = "~/.cloudflared/abc123.json"
TUNNEL_CONFIG = "~/.cloudflared/config.yml"

# 3. 定义隧道操作任务
[tasks.tunnel-create]
run = "cloudflared tunnel create ${TUNNEL_NAME}"

[tasks.tunnel-route-dns]
run = "cloudflared tunnel route dns ${TUNNEL_ID} ${HOSTNAME}"

[tasks.tunnel-run]
run = "cloudflared tunnel run --config ${TUNNEL_CONFIG} ${TUNNEL_ID}"
```

**Step 2: 创建 cloudflared 配置文件**

创建 `~/.cloudflared/config.yml`:
```yaml
tunnel: abc123
credentials-file: /home/user/.cloudflared/abc123.json

ingress:
  - hostname: app.example.com
    service: http://localhost:3000
  - hostname: api.example.com
    service: http://localhost:8000
  - service: http_status:404  # 默认规则
```

**Step 3: 在 svcmgr 配置中定义服务**

编辑 `~/.config/mise/svcmgr/config.toml`:
```toml
[services.tunnel]
task = "tunnel-run"            # 引用 mise 任务
enable = true                   # 开机自启
restart = "always"             # 自动重启策略

[services.tunnel.health]
type = "http"
url = "https://app.example.com"
interval = "30s"
timeout = "5s"
```

---

### 8.3 迁移步骤

#### Step 1: 安装 cloudflared 工具

```bash
# 方法 1: 通过 mise 配置安装
echo 'cloudflared = "latest"' >> ~/.config/mise/config.toml
mise install cloudflared@latest

# 方法 2: 直接安装
mise use cloudflared@latest

# 验证安装
which cloudflared
cloudflared --version
```

#### Step 2: 创建隧道

```bash
# 通过 mise 任务创建隧道
mise run tunnel-create

# 输出:
# Tunnel credentials written to /home/user/.cloudflared/abc123.json
# Created tunnel 'my-tunnel' with ID: abc123
```

#### Step 3: 配置 DNS 路由

```bash
# 配置域名路由
HOSTNAME=app.example.com mise run tunnel-route-dns
HOSTNAME=api.example.com mise run tunnel-route-dns

# 输出:
# Added CNAME record for app.example.com
# Added CNAME record for api.example.com
```

#### Step 4: 创建 cloudflared 配置文件

创建 `~/.cloudflared/config.yml`(参考 §8.2.2 Step 2)

#### Step 5: 创建 svcmgr 服务配置

编辑 `~/.config/mise/svcmgr/config.toml`(参考 §8.2.2 Step 3)

#### Step 6: 启动隧道服务

```bash
# 启动服务
svcmgr service start tunnel

# 输出:
# ✓ Service 'tunnel' started
# ✓ Connected to Cloudflare network
# ✓ Tunnel 'my-tunnel' is active
```

#### Step 7: 验证隧道

```bash
# 检查服务状态
svcmgr service status tunnel

# 测试隧道连接
curl https://app.example.com
# 预期: 返回应用响应

# 查看隧道日志
svcmgr service logs tunnel --tail 50
```

---

### 8.4 可选便捷命令

如果希望保留简化的 CLI 体验(类似 v1.x),可以实现可选的便捷命令(约 200行代码):

**实现示例** (`src/cli/tunnel.rs`):
```rust
// 便捷命令:内部调用 mise 任务
use crate::mise::run_mise_task;

pub async fn tunnel_create(name: String) -> Result<()> {
    run_mise_task("tunnel-create", &[("TUNNEL_NAME", &name)]).await
}

pub async fn tunnel_route_dns(tunnel_id: String, hostname: String) -> Result<()> {
    run_mise_task("tunnel-route-dns", &[
        ("TUNNEL_ID", &tunnel_id),
        ("HOSTNAME", &hostname)
    ]).await
}

pub async fn tunnel_start() -> Result<()> {
    // 实际调用: svcmgr service start tunnel
    crate::service::start("tunnel").await
}
```

**CLI 定义**:
```rust
#[derive(Parser)]
pub enum TunnelCommand {
    Create { name: String },
    RouteDns { tunnel_id: String, hostname: String },
    Start,
    Stop,
}
```

**使用方式**:
```bash
# 便捷命令(如果实现)
svcmgr tunnel create my-tunnel
svcmgr tunnel route-dns abc123 app.example.com
svcmgr tunnel start

# 底层实际执行:
# - mise run tunnel-create
# - mise run tunnel-route-dns
# - svcmgr service start tunnel
```

**实现决策**:
- ✅ **推荐**:实现便捷命令,降低用户学习成本(约 200行代码)
- ❌ **不推荐**:强制用户直接使用 `mise run` 和 `svcmgr service`(学习曲线陡峭)

---

### 8.5 破坏性变更

#### 8.5.1 移除的独立模块

- ❌ **移除模块**:`atoms/tunnel.rs` (865行) 完全移除
- ❌ **移除配置段**:`[tunnel]` 和 `[[tunnel.ingress]]` 配置段不再支持

#### 8.5.2 配置格式变更

| 项目 | 旧位置(v1.x) | 新位置(v2.0) |
|-----|---------------|---------------|
| 工具安装 | 手动安装或系统包管理器 | `~/.config/mise/config.toml` [tools] |
| 隧道操作 | `svcmgr tunnel *` CLI 命令 | `~/.config/mise/config.toml` [tasks] |
| 隧道配置 | `~/.config/svcmgr/config.toml` [tunnel] | `~/.cloudflared/config.yml` |
| 服务配置 | 内部配置段 | `~/.config/mise/svcmgr/config.toml` [services] |

#### 8.5.3 CLI 命令行为变更

如果保留 `svcmgr tunnel *` 命令(可选便捷命令):

| 命令 | 旧行为(v1.x) | 新行为(v2.0) | 兼容性 |
|-----|---------------|---------------|--------|
| `svcmgr tunnel create <name>` | 直接调用 cloudflared API | 调用 `mise run tunnel-create` | 参数可能不兼容 |
| `svcmgr tunnel route-dns <id> <host>` | 直接调用 cloudflared API | 调用 `mise run tunnel-route-dns` | 参数可能不兼容 |
| `svcmgr tunnel start` | 启动独立守护进程 | 调用 `svcmgr service start tunnel` | 行为变更 |
| `svcmgr tunnel stop` | 停止独立守护进程 | 调用 `svcmgr service stop tunnel` | 行为变更 |

**迁移建议**:
- 测试所有 `svcmgr tunnel *` 命令的脚本,参数可能需要调整
- 验证隧道配置文件格式(从 TOML 迁移到 YAML)
- 测试 DNS 路由是否正常工作

---

### 8.6 代码减少统计

| 模块 | 旧代码行数(v1.x) | 新代码行数(v2.0) | 减少比例 |
|------|-------------------|-------------------|---------|
| `features/webtty.rs` | 300 | 0 | **100%** |
| `atoms/tunnel.rs` | 865 | 0 | **100%** |
| `cli/tunnel.rs`(可选) | 0 | ~200 | - |
| **总计** | **1165** | **200** | **83%** |

**收益**:
- ✅ 代码维护成本降低 83%
- ✅ 配置文件驱动,易于测试和调试
- ✅ 与 mise 生态无缝集成
- ✅ 自动依赖管理(mise 安装 cloudflared)

**相关文档**:
- **22-breaking-changes.md** §4.3 - CLI 命令行为变更
- **22-breaking-changes.md** §3.4 - 配置格式变更
- `docs/DESIGN_TTY_CLOUDFLARED.md` - 详细设计决策

---
## 9. 回滚计划

### 9.1 回滚触发条件

以下情况应立即回滚：

| 问题类型 | 严重程度 | 回滚决策 |
|---------|---------|---------|
| 服务启动失败 | 高 | 立即回滚 |
| 健康检查持续失败 | 高 | 立即回滚 |
| 数据丢失或损坏 | 高 | 立即回滚 |
| 性能严重下降（>50%） | 中-高 | 评估后回滚 |
| 日志异常增多 | 中 | 监控并准备回滚 |
| 部分功能异常 | 中 | 评估影响范围 |
| 配置错误（不影响服务） | 低 | 修复配置，不回滚 |

### 9.2 手动回滚步骤

如果自动回滚工具不可用，手动回滚步骤：

```bash
#!/bin/bash
# 手动回滚脚本

echo "=== 手动回滚到旧架构 ==="

# 1. 停止 svcmgr
echo "1. 停止 svcmgr..."
svcmgr stop || killall svcmgr

# 2. 恢复 systemd 服务单元（从备份）
echo "2. 恢复 systemd 服务单元..."
BACKUP_DIR=$(ls -td ~/svcmgr-migration-backup/* | head -1)
cp -r "$BACKUP_DIR/systemd/"*.service ~/.config/systemd/user/
systemctl --user daemon-reload

# 3. 启动旧服务
echo "3. 启动旧服务..."
for service in api worker; do
  systemctl --user enable "$service.service"
  systemctl --user start "$service.service"
  echo "  - $service.service: $(systemctl --user is-active $service.service)"
done

# 4. 恢复 crontab
echo "4. 恢复 crontab..."
crontab "$BACKUP_DIR/crontab.txt"
echo "  - crontab 任务数: $(crontab -l | grep -c '^[^#]')"

# 5. 恢复 nginx（如果使用）
if [ -d "$BACKUP_DIR/nginx" ]; then
  echo "5. 恢复 nginx..."
  cp -r "$BACKUP_DIR/nginx/"* ~/.config/nginx/
  systemctl --user restart nginx.service
  echo "  - nginx.service: $(systemctl --user is-active nginx.service)"
fi

# 6. 验证旧服务
echo "6. 验证服务状态..."
systemctl --user status api.service worker.service

echo "=== 回滚完成 ==="
```

### 9.3 数据恢复

如果数据在迁移过程中损坏：

```bash
# 1. 停止所有服务（新旧）
systemctl --user stop api.service worker.service
svcmgr stop

# 2. 恢复数据文件（从备份）
BACKUP_DIR=$(ls -td ~/svcmgr-migration-backup/* | head -1)
cp -r "$BACKUP_DIR/svcmgr/data/"* ~/.local/share/svcmgr/data/

# 3. 验证数据完整性
# （根据具体应用执行）

# 4. 重启服务
systemctl --user start api.service worker.service
```

### 9.4 故障排查清单

回滚前检查以下项目，可能问题可修复而无需回滚：

| 检查项 | 命令 | 预期结果 |
|-------|------|---------|
| mise 已安装 | `mise --version` | 显示版本号 |
| 配置文件语法 | `svcmgr config validate` | 无错误 |
| 依赖工具已安装 | `mise install` | 所有工具已安装 |
| 端口未被占用 | `lsof -i :3000` | 无输出或仅 svcmgr |
| 日志目录可写 | `touch ~/.local/share/svcmgr/logs/test.log` | 成功创建 |
| 工作目录存在 | `ls /home/user/app` | 目录存在 |
| 环境变量文件存在 | `cat /home/user/app/.env` | 文件存在 |
| cgroups 权限（可选） | `ls /sys/fs/cgroup/user.slice/user-$(id -u).slice` | 目录存在 |

---

## 10. 验证清单

### 10.1 迁移前验证

- [ ] **备份完成**：所有配置和数据已备份到安全位置
- [ ] **mise 已安装**：版本 >= 2024.1.0
- [ ] **依赖工具已安装**：node, python 等（通过 `mise install`）
- [ ] **配置文件已创建**：`~/.config/mise/config.toml` 和 `~/.config/mise/svcmgr/config.toml`
- [ ] **配置语法正确**：`svcmgr config validate` 无错误
- [ ] **端口冲突已解决**：新服务使用的端口未被占用
- [ ] **文件权限正确**：日志目录、工作目录可写
- [ ] **测试环境验证**：在开发环境完整测试迁移流程

### 8.2 迁移后验证

#### 8.2.1 服务验证

- [ ] **所有服务已启动**：`svcmgr service list` 显示所有服务 `running`
- [ ] **健康检查通过**：`svcmgr service status <name>` 显示 `healthy`
- [ ] **日志正常输出**：`svcmgr service logs <name>` 无错误
- [ ] **进程正常运行**：`ps aux | grep <process>` 显示进程存在
- [ ] **端口正常监听**：`lsof -i :<port>` 或 `netstat -tuln | grep <port>`

#### 8.2.2 功能验证

- [ ] **API 端点响应**：`curl http://localhost:3000/health` 返回 200
- [ ] **数据库连接**：应用能正常连接数据库
- [ ] **文件读写**：应用能正常读写文件
- [ ] **定时任务调度**：`svcmgr task list` 显示下次执行时间
- [ ] **日志轮转**：日志文件按配置轮转
- [ ] **重启策略**：手动停止服务后自动重启（如果配置 `restart = "always"`）

#### 8.2.3 性能验证

- [ ] **响应时间正常**：API 响应时间与旧架构相当
- [ ] **CPU 使用率**：`htop` 或 `top` 显示 CPU 使用率正常
- [ ] **内存使用率**：`free -h` 显示内存使用率正常
- [ ] **磁盘 I/O**：`iostat` 显示磁盘 I/O 正常
- [ ] **网络流量**：`iftop` 或 `nethogs` 显示网络流量正常

#### 8.2.4 可观测性验证

- [ ] **日志可访问**：`svcmgr service logs <name>` 能查看日志
- [ ] **指标可收集**：Prometheus/Grafana 能抓取指标（如果配置）
- [ ] **告警正常**：告警规则正常触发（如果配置）
- [ ] **追踪可用**：分布式追踪正常工作（如果配置）

### 8.3 长期监控

迁移后持续监控 **24-48 小时**：

- **每 1 小时检查**：
  - 服务状态（`svcmgr service list`）
  - 错误日志（`svcmgr service logs <name> --since 1h | grep ERROR`）
  - 系统资源（`htop`, `df -h`）

- **每 6 小时检查**：
  - 定时任务执行记录（`svcmgr task history <name>`）
  - 重启次数（`svcmgr service status <name> | grep restarts`）
  - 磁盘使用率（`du -sh ~/.local/share/svcmgr/logs/`）

- **每 24 小时检查**：
  - 完整系统健康检查（`svcmgr health check --all`）
  - 备份验证（定时备份是否正常执行）
  - 性能基准对比（与旧架构对比）

---

## 9. 测试策略

### 9.1 迁移前测试

#### 9.1.1 在开发环境完整测试

```bash
# 1. 克隆生产环境配置到开发环境
scp -r production:~/.config/systemd/user dev:~/.config/systemd/user
scp production:/tmp/crontab.txt dev:/tmp/crontab.txt

# 2. 在开发环境执行迁移
ssh dev "svcmgr migrate init && svcmgr migrate analyze && svcmgr migrate convert"

# 3. 审查生成的配置
scp dev:~/.config/mise/config.toml /tmp/
scp dev:~/.config/mise/svcmgr/config.toml /tmp/
cat /tmp/config.toml /tmp/svcmgr/config.toml

# 4. 应用迁移并验证
ssh dev "svcmgr migrate apply --mode immediate"
ssh dev "svcmgr service list"

# 5. 功能测试
curl http://dev:3000/health
```

#### 9.1.2 回滚演练

```bash
# 在开发环境测试回滚流程
ssh dev "svcmgr migrate rollback"
ssh dev "systemctl --user status api.service"
```

### 9.2 迁移中测试

每迁移一个服务后立即测试：

```bash
# 1. 服务状态
svcmgr service status api

# 2. 健康检查
curl -f http://localhost:3000/health || echo "健康检查失败"

# 3. 功能测试（根据具体应用）
curl http://localhost:3000/api/users | jq .

# 4. 日志检查
svcmgr service logs api --tail 50 | grep -i error

# 5. 性能测试
ab -n 1000 -c 10 http://localhost:3000/api/users
```

### 9.3 迁移后测试

#### 9.3.1 功能回归测试

创建测试脚本覆盖所有关键功能：

```bash
#!/bin/bash
# 功能回归测试脚本

echo "=== 功能回归测试 ==="

# 测试 1: API 健康检查
echo "测试 1: API 健康检查..."
if curl -f http://localhost:3000/health > /dev/null 2>&1; then
  echo "✓ 通过"
else
  echo "✗ 失败"
  exit 1
fi

# 测试 2: 用户列表接口
echo "测试 2: 用户列表接口..."
USERS=$(curl -s http://localhost:3000/api/users | jq '. | length')
if [ "$USERS" -gt 0 ]; then
  echo "✓ 通过（返回 $USERS 个用户）"
else
  echo "✗ 失败（返回 0 个用户）"
  exit 1
fi

# 测试 3: 数据库连接
echo "测试 3: 数据库连接..."
if curl -s http://localhost:3000/api/db-check | jq -e '.connected == true' > /dev/null; then
  echo "✓ 通过"
else
  echo "✗ 失败"
  exit 1
fi

# 测试 4: 定时任务执行
echo "测试 4: 定时任务执行..."
LAST_RUN=$(svcmgr task history cleanup --limit 1 --json | jq -r '.[-1].finished_at')
if [ "$LAST_RUN" != "null" ]; then
  echo "✓ 通过（最后执行: $LAST_RUN）"
else
  echo "✗ 失败（定时任务未执行）"
  exit 1
fi

echo "=== 所有测试通过 ==="
```

#### 9.3.2 性能基准测试

使用 `ab`（Apache Bench）或 `wrk` 进行压力测试：

```bash
# 旧架构性能基准
ab -n 10000 -c 100 http://localhost:3000/api/users > old-benchmark.txt

# 迁移后性能基准
ab -n 10000 -c 100 http://localhost:3000/api/users > new-benchmark.txt

# 对比结果
echo "=== 性能对比 ==="
echo "旧架构 QPS: $(grep 'Requests per second' old-benchmark.txt | awk '{print $4}')"
echo "新架构 QPS: $(grep 'Requests per second' new-benchmark.txt | awk '{print $4}')"
```

#### 9.3.3 故障恢复测试

测试服务崩溃后的自动恢复：

```bash
# 1. 模拟服务崩溃
SERVICE_PID=$(svcmgr service status api --json | jq -r '.pid')
kill -9 "$SERVICE_PID"

# 2. 等待自动重启
sleep 5

# 3. 验证服务已恢复
if svcmgr service status api | grep -q "running"; then
  echo "✓ 服务自动恢复"
else
  echo "✗ 服务未恢复"
fi
```

---

## 10. 常见问题

### 10.1 mise 相关问题

#### Q1: mise 未安装或版本过低

**问题**：
```
$ svcmgr start
Error: mise not found in PATH
```

**解决方案**：
```bash
# 安装 mise
curl https://mise.run | sh

# 或使用包管理器
sudo apt install mise  # Ubuntu/Debian
sudo dnf install mise  # Fedora

# 验证版本
mise --version
# 要求 >= 2024.1.0
```

#### Q2: mise 任务未找到

**问题**：
```
$ svcmgr service start api
Error: mise task 'api-start' not found
```

**解决方案**：
```bash
# 检查 mise 配置
mise task list

# 确保任务定义在 mise 配置中
cat ~/.config/mise/config.toml | grep -A 5 'tasks.api-start'

# 如果任务不存在，添加到配置
cat >> ~/.config/mise/config.toml <<'EOF'
[tasks.api-start]
run = "node server.js"
dir = "/home/user/app"
EOF

# 验证任务可执行
mise run api-start
```

#### Q3: mise 依赖工具未安装

**问题**：
```
$ mise run api-start
Error: node not found
```

**解决方案**：
```bash
# 安装所有依赖工具
mise install

# 或安装特定工具
mise install node@22

# 验证安装
mise list
```

---

### 10.2 服务启动问题

#### Q4: 服务启动失败

**问题**：
```
$ svcmgr service start api
Error: service 'api' failed to start
```

**解决方案**：
```bash
# 1. 查看详细日志
svcmgr service logs api --tail 100

# 2. 检查配置
svcmgr config validate

# 3. 手动运行任务（前台调试）
mise run api-start

# 4. 检查端口占用
lsof -i :3000

# 5. 检查工作目录
ls -ld /home/user/app

# 6. 检查环境变量
mise env --json | jq .
```

#### Q5: 服务频繁重启

**问题**：
```
$ svcmgr service status api
Status: restarting (restarts: 10)
```

**解决方案**：
```bash
# 1. 查看崩溃原因
svcmgr service logs api --tail 200 | grep -i error

# 2. 调整重启策略
# 编辑配置，增加重启延迟
[services.api]
restart_delay = "10s"  # 从 2s 增加到 10s
restart_limit = 5      # 降低重启次数限制

# 3. 检查资源限制（如果启用 cgroups）
svcmgr service status api --json | jq '.resources'

# 4. 临时禁用自动重启进行调试
svcmgr service update api --restart no
mise run api-start  # 前台运行调试
```

---

### 10.3 端口和网络问题

#### Q6: 端口被占用

**问题**：
```
Error: address already in use (port 3000)
```

**解决方案**：
```bash
# 1. 查找占用进程
lsof -i :3000
# 或
netstat -tuln | grep 3000

# 2. 如果是旧服务，停止它
systemctl --user stop api.service

# 3. 如果是其他进程，修改端口
# 编辑 mise 任务，更改端口
[tasks.api-start]
env = { PORT = "3001" }  # 使用不同端口
```

#### Q7: 内置代理无法访问

**问题**：
```
$ curl http://localhost:8080/api/users
curl: (7) Failed to connect to localhost port 8080
```

**解决方案**：
```bash
# 1. 检查 svcmgr 主进程是否运行
ps aux | grep svcmgr

# 2. 检查 HTTP 代理配置
cat ~/.config/mise/svcmgr/config.toml | grep -A 10 '^\[http\]'

# 3. 检查路由配置
svcmgr http routes list

# 4. 检查后端服务是否运行
svcmgr service status api

# 5. 测试后端服务直接访问
curl http://localhost:3000/health
```

---

### 10.4 权限问题

#### Q8: cgroups 权限不足

**问题**：
```
Error: failed to create cgroup: permission denied
```

**解决方案 1**（启用用户 cgroup 委派）：
```bash
# 检查 systemd 版本（需要 >= 244）
systemd --version

# 启用用户 cgroup 委派
sudo mkdir -p /etc/systemd/system/user@.service.d
sudo tee /etc/systemd/system/user@.service.d/delegate.conf > /dev/null <<EOF
[Service]
Delegate=yes
EOF

# 重启 systemd
sudo systemctl daemon-reload
sudo systemctl restart user@$(id -u).service

# 验证
ls /sys/fs/cgroup/user.slice/user-$(id -u).slice
```

**解决方案 2**（禁用 cgroups 资源限制）：
```toml
# .config/mise/svcmgr/config.toml
[features]
cgroups = "disabled"  # 完全禁用
```

#### Q9: 日志目录不可写

**问题**：
```
Error: failed to open log file: permission denied
```

**解决方案**：
```bash
# 1. 检查日志目录权限
ls -ld ~/.local/share/svcmgr/logs/

# 2. 创建目录并设置权限
mkdir -p ~/.local/share/svcmgr/logs
chmod 755 ~/.local/share/svcmgr/logs

# 3. 如果使用自定义路径，确保目录存在
mkdir -p /home/user/app/logs
```

---

### 10.5 配置问题

#### Q10: 配置语法错误

**问题**：
```
$ svcmgr config validate
Error: invalid TOML syntax at line 42
```

**解决方案**：
```bash
# 1. 查看详细错误信息
svcmgr config validate --verbose

# 2. 使用 TOML 检查工具
# 安装 taplo (TOML 格式化工具)
cargo install taplo-cli

# 检查语法
taplo check ~/.config/mise/svcmgr/config.toml

# 格式化配置
taplo format ~/.config/mise/svcmgr/config.toml
```

#### Q11: 环境变量未生效

**问题**：应用无法读取环境变量。

**解决方案**：
```bash
# 1. 检查环境变量作用域
# mise 全局环境变量
cat ~/.config/mise/config.toml | grep -A 10 '^\[env\]'

# 任务级环境变量
cat ~/.config/mise/config.toml | grep -A 5 'tasks.api-start'

# 服务级环境变量
cat ~/.config/mise/svcmgr/config.toml | grep -A 5 'services.api'

# 2. 验证环境变量展开
mise env --json | jq '.'

# 3. 测试任务环境变量
mise run api-start -- env | grep NODE_ENV

# 4. 检查变量优先级
# 服务级 > 任务级 > 全局级 > 系统级
```

---

### 10.6 数据迁移问题

#### Q12: 日志丢失

**问题**：迁移后找不到旧日志。

**解决方案**：
```bash
# 1. 导出旧 systemd 日志
journalctl --user -u api.service --no-pager > ~/api-old-logs.txt

# 2. 合并到新日志目录
cat ~/api-old-logs.txt >> ~/.local/share/svcmgr/logs/api-stdout.log
```

#### Q13: 状态数据丢失

**问题**：服务启动后状态异常（如用户会话丢失）。

**解决方案**：
```bash
# 1. 确认工作目录正确
svcmgr service status api --json | jq -r '.workdir'

# 2. 检查数据文件路径
ls -la /home/user/app/data/

# 3. 创建符号链接（如果路径变更）
ln -s /new/path/data.db /old/path/data.db
```

---

### 10.7 性能问题

#### Q14: 性能下降

**问题**：迁移后响应时间明显增加。

**解决方案**：
```bash
# 1. 检查资源限制是否过严
svcmgr service status api --json | jq '.resources'

# 2. 调整或移除资源限制
[services.api.resources]
cpu_quota = "200%"      # 允许使用 2 个 CPU 核心
memory_limit = "2GB"    # 增加内存限制

# 3. 检查是否启用了 cgroups v2（可能有性能开销）
[features]
cgroups = "disabled"    # 临时禁用测试

# 4. 对比进程数和资源使用
ps aux | grep node      # 旧架构
ps aux | grep svcmgr    # 新架构
```

#### Q15: 日志过多导致磁盘满

**问题**：日志文件快速增长。

**解决方案**：
```toml
# .config/mise/svcmgr/config.toml

[services.api]
stdout = "/home/user/.local/share/svcmgr/logs/api-stdout.log"
stderr = "/home/user/.local/share/svcmgr/logs/api-stderr.log"

# 启用日志轮转
[services.api.log_rotation]
max_size = "100MB"      # 单个文件最大 100MB
max_files = 5           # 保留最近 5 个文件
compress = true         # 压缩旧日志
```

---

## 相关规范文档

- **00-architecture-overview.md** - 新架构概览
- **01-config-design.md** - 配置文件设计
- **02-scheduler-engine.md** - 调度引擎设计
- **03-process-manager.md** - 进程管理与资源限制
- **04-git-versioning.md** - Git 版本管理
- **05-web-service.md** - 内置 HTTP 代理
- **20-implementation-phases.md** - 实施路线图
- **22-breaking-changes.md** - 破坏性变更清单

---

## 附录

### A. 完整迁移检查清单

打印此清单，逐项检查：

```
迁移前准备
□ 备份所有配置和数据
□ mise 已安装（>= 2024.1.0）
□ 依赖工具已安装（mise install）
□ 配置文件已创建并验证
□ 在开发环境测试完整流程

迁移执行
□ 停止旧服务（systemd/cron）
□ 启动新服务（svcmgr）
□ 验证服务启动
□ 健康检查通过
□ 功能回归测试通过

迁移后验证
□ 所有服务运行正常
□ 日志正常输出
□ 定时任务正常调度
□ HTTP 代理正常工作
□ 性能符合预期
□ 监控 24-48 小时

清理
□ 移除 systemd 单元文件
□ 清空 crontab
□ 卸载 nginx（如果不再需要）
□ 归档备份文件
```

### B. 紧急联系和支持

- **文档**：https://svcmgr.example.com/docs/migration
- **社区论坛**：https://discuss.svcmgr.example.com
- **GitHub Issues**：https://github.com/example/svcmgr/issues
- **Discord**：https://discord.gg/svcmgr

### C. 术语对照表

| 旧术语 | 新术语 | 说明 |
|-------|-------|------|
| systemd 服务单元 | mise 任务 + svcmgr 服务 | 服务定义分离 |
| crontab 任务 | scheduled_tasks | 定时任务 |
| nginx 反向代理 | 内置 HTTP 代理 | 集成到 svcmgr |
| EnvironmentFile | mise [env] | 环境变量管理 |
| ExecStart | [tasks.<name>] run | 命令定义 |

---

**文档版本**：2.0.0-draft
**最后更新**：2026-02-23
**维护者**：svcmgr 开发团队
