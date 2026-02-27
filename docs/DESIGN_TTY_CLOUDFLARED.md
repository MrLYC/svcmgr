# ttyd 和 cloudflared 特殊场景设计

> 版本：1.0.0
> 日期：2026-02-23
> 相关文档：MISE_REDESIGN_RESEARCH_ZH.md

## 背景

### 旧架构设计

在原有 svcmgr 设计中：

| 组件 | 实现方式 | 代码量 | 问题 |
|------|---------|-------|------|
| **ttyd** | 独立功能模块 `features/webtty.rs` | ~300行 | 特殊处理，与其他服务不统一 |
| **cloudflared** | 独立原子模块 `atoms/tunnel.rs` | ~865行 | 封装 CLI，复杂度高 |

**核心问题**：
1. **特殊化处理**：ttyd 和 cloudflared 作为独立模块，与普通服务管理逻辑不统一
2. **代码重复**：进程生命周期管理逻辑与 supervisor 重复
3. **扩展困难**：添加新的类似服务（如 gotty、frp）需要再写独立模块

### 新架构理念

**核心思路**：ttyd 和 cloudflared **本质上就是普通服务**，应该通过 mise 任务定义 + svcmgr 统一服务管理机制处理。

---

## 设计原则

### 1. 统一抽象

**所有长驻进程都是服务，无论是什么软件**：

```
服务 = mise 任务 + svcmgr 生命周期管理 + (可选)端口代理
```

| 服务类型 | mise 任务 | svcmgr 管理 | 端口代理 |
|---------|----------|------------|---------|
| Node.js API | `node server.js` | ✅ | ✅ (HTTP) |
| Python Worker | `python worker.py` | ✅ | ❌ |
| **ttyd 终端** | `ttyd -p 9001 bash` | ✅ | ✅ (WebSocket) |
| **cloudflared 隧道** | `cloudflared tunnel run` | ✅ | ❌ |
| Redis | `redis-server` | ✅ | ❌ |

**优势**：
- ✅ 代码统一：移除 `features/webtty.rs` (300行) 和 `atoms/tunnel.rs` (865行)
- ✅ 扩展简单：添加新服务只需配置文件，无需写代码
- ✅ 用户友好：用户可自定义 ttyd/cloudflared 启动参数

### 2. mise 负责依赖

**mise 管理工具安装，svcmgr 只负责运行**：

```toml
# .config/mise/config.toml
[tools]
ttyd = "1.7.7"           # mise 安装 ttyd（如果有插件）
cloudflared = "latest"   # mise 安装 cloudflared

# 如果 mise 没有对应插件，用户手动安装到 PATH
```

### 3. 便捷命令可选

**复杂操作提供便捷命令，但不强制**：

| 操作 | 方式 1（配置文件） | 方式 2（便捷命令） |
|------|------------------|------------------|
| 启动 ttyd | `svcmgr service start tty-bash` | N/A（配置即可） |
| 创建 cloudflared 隧道 | `mise run tunnel-create name=my-tunnel` | `svcmgr tunnel create my-tunnel` |
| 配置 DNS 路由 | `mise run tunnel-route-dns tunnel=... hostname=...` | `svcmgr tunnel route-dns ...` |
| 启动隧道 | `svcmgr service start tunnel` | N/A（配置即可） |

---

## 配置设计

### ttyd 服务配置

#### 方案 1：单个 ttyd 实例

```toml
# .config/mise/config.toml（mise 配置）
[tools]
ttyd = "1.7.7"  # 如果 mise 有 ttyd 插件

[tasks.tty-bash]
description = "Start bash web terminal"
run = "ttyd -p {{env.TTY_PORT}} -t titleFixed='Bash Terminal' bash"
env = { TTY_PORT = "9001" }

[tasks.tty-python]
run = "ttyd -p {{env.TTY_PORT}} -t titleFixed='Python REPL' python"
env = { TTY_PORT = "9002" }

# .config/mise/svcmgr/config.toml（svcmgr 配置）
[services.tty-bash]
task = "tty-bash"
enable = true
restart = "always"
ports = { terminal = 9001 }

[services.tty-python]
task = "tty-python"
enable = true
restart = "always"
ports = { terminal = 9002 }

# HTTP 代理（可选，提供友好路径）
[[http.routes]]
path = "/tty/bash"
target = "service:tty-bash:terminal"
websocket = true
strip_prefix = true

[[http.routes]]
path = "/tty/python"
target = "service:tty-python:terminal"
websocket = true
strip_prefix = true
```

**用户操作**：

```bash
# 启动服务
svcmgr service start tty-bash
svcmgr service start tty-python

# 访问终端
curl http://localhost:8000/tty/bash     # 自动代理到 localhost:9001
curl http://localhost:8000/tty/python   # 自动代理到 localhost:9002

# 查看状态
svcmgr service status tty-bash

# 停止服务
svcmgr service stop tty-bash
```

#### 方案 2：动态 ttyd 实例（高级）

**如果需要动态创建多个 ttyd 实例**，可以使用模板：

```toml
# .config/mise/config.toml
[tasks.tty-template]
run = """
ttyd -p {{arg(name="port")}} \
     -t titleFixed='{{arg(name="title")}}' \
     {{arg(name="command")}}
"""

# 用户动态创建
# mise run tty-template port=9003 title="Node REPL" command="node"
```

**svcmgr 端**（未来扩展）：

```bash
# 动态创建服务（生成配置到 conf.d/）
svcmgr service create tty-node \
    --task "tty-template port=9003 title='Node REPL' command=node" \
    --restart always \
    --port terminal=9003

# 等价于手动写配置文件
```

---

### cloudflared 服务配置

#### 基础配置

```toml
# .config/mise/config.toml（mise 配置）
[tools]
cloudflared = "latest"

[tasks.tunnel-run]
description = "Run cloudflared tunnel"
run = """
cloudflared tunnel run \
    --config {{env.TUNNEL_CONFIG}} \
    {{env.TUNNEL_ID}}
"""
env = {
    TUNNEL_CONFIG = "~/.cloudflared/config.yml",
    TUNNEL_ID = "my-tunnel-uuid"
}

# .config/mise/svcmgr/config.toml（svcmgr 配置）
[services.tunnel]
task = "tunnel-run"
enable = true
restart = "always"
# cloudflared 不需要端口代理（直接连接 Cloudflare）
```

#### cloudflared 复杂操作

**方式 1：通过 mise 任务封装**

```toml
# .config/mise/config.toml
[tasks.tunnel-login]
description = "Login to Cloudflare"
run = "cloudflared tunnel login"

[tasks.tunnel-create]
description = "Create a new tunnel"
run = """
cloudflared tunnel create {{arg(name="name")}} && \
echo "Tunnel created. Update TUNNEL_ID in config.toml"
"""

[tasks.tunnel-delete]
run = "cloudflared tunnel delete {{arg(name="id")}}"

[tasks.tunnel-list]
run = "cloudflared tunnel list"

[tasks.tunnel-route-dns]
description = "Route DNS to tunnel"
run = """
cloudflared tunnel route dns \
    {{arg(name="tunnel")}} \
    {{arg(name="hostname")}}
"""

[tasks.tunnel-config]
description = "Generate ingress config"
run = """
cat > ~/.cloudflared/config.yml <<EOF
tunnel: {{arg(name="tunnel_id")}}
credentials-file: ~/.cloudflared/{{arg(name="tunnel_id")}}.json

ingress:
  - hostname: {{arg(name="hostname")}}
    service: {{arg(name="service")}}
  - service: http_status:404
EOF
"""
```

**用户操作**：

```bash
# 1. 登录（一次性）
mise run tunnel-login

# 2. 创建隧道
mise run tunnel-create name=my-tunnel
# 输出：Created tunnel my-tunnel with id a1b2c3d4-...

# 3. 配置 ingress 规则
mise run tunnel-config \
    tunnel_id=a1b2c3d4 \
    hostname=example.com \
    service=http://localhost:3000

# 4. 配置 DNS 路由
mise run tunnel-route-dns \
    tunnel=a1b2c3d4 \
    hostname=example.com

# 5. 更新 svcmgr 配置（启用隧道服务）
# 编辑 .config/mise/svcmgr/config.toml
[services.tunnel]
task = "tunnel-run"
enable = true
env = { TUNNEL_ID = "a1b2c3d4" }

# 6. 启动隧道
svcmgr service start tunnel
```

**方式 2：保留便捷命令（可选）**

如果觉得 mise 任务太复杂，可以保留 `svcmgr tunnel` 子命令：

```bash
# 便捷命令（内部调用 cloudflared CLI）
svcmgr tunnel login
svcmgr tunnel create my-tunnel
svcmgr tunnel route-dns my-tunnel example.com

# 但进程管理统一走 svcmgr service
svcmgr service start tunnel
svcmgr service status tunnel
svcmgr service stop tunnel
```

**实现**：

```rust
// src/cli/tunnel.rs（可选，约 200 行）
pub async fn handle_tunnel_command(action: TunnelAction) -> Result<()> {
    match action {
        TunnelAction::Login => {
            // 直接调用 cloudflared CLI
            Command::new("cloudflared")
                .args(&["tunnel", "login"])
                .status()?;
        }
        TunnelAction::Create { name } => {
            let output = Command::new("cloudflared")
                .args(&["tunnel", "create", &name])
                .output()?;
            // 解析输出，提取 tunnel ID
            let tunnel_id = parse_tunnel_id(&output.stdout)?;
            println!("Created tunnel: {} (ID: {})", name, tunnel_id);
            println!("Update TUNNEL_ID in .config/mise/svcmgr/config.toml");
        }
        // ... 其他命令
    }
    Ok(())
}
```

**代码量对比**：

| 实现方式 | 代码量 | 维护成本 | 用户体验 |
|---------|-------|---------|---------|
| 纯 mise 任务 | 0 行（配置文件） | 最低 | 需要记住 mise 任务名 |
| 保留便捷命令 | ~200 行 | 中等 | 最友好 |
| 旧架构（独立原子） | ~865 行 | 高 | 友好，但代码复杂 |

**推荐**：**保留便捷命令**（折中方案），代码量减少 75%，用户体验不变。

---

## 实现差异对比

### 旧设计 vs 新设计

| 维度 | 旧设计 | 新设计 |
|------|-------|-------|
| **ttyd 进程管理** | `features/webtty.rs` (300行) | `Scheduler` 统一管理 |
| **ttyd 路由** | `NginxManager.add_tty_route()` | `http.routes` 配置 |
| **cloudflared 进程管理** | `atoms/tunnel.rs` + `supervisor` | `Scheduler` 统一管理 |
| **cloudflared CLI 封装** | `TunnelManager` (865行) | 可选：`cli/tunnel.rs` (200行) |
| **配置方式** | 硬编码或独立配置 | 统一 TOML 配置 |
| **代码总量** | ~1165 行 | ~200 行（如果保留便捷命令） |
| **扩展性** | 低（新服务需要新模块） | 高（配置文件即可） |

### 代码减少 83%

- **移除**: `features/webtty.rs` (300行)
- **移除**: `atoms/tunnel.rs` (865行)
- **新增**: `cli/tunnel.rs` (200行，可选)
- **净减少**: 965 行 → 83% 减少

---

## 迁移指南

### ttyd 迁移

#### 旧方式

```bash
# 旧命令
svcmgr tty create dev-shell --command bash --port 9001
svcmgr tty start dev-shell
svcmgr tty stop dev-shell
svcmgr tty delete dev-shell
```

#### 新方式

```toml
# .config/mise/config.toml
[tasks.tty-dev]
run = "ttyd -p 9001 bash"

# .config/mise/svcmgr/config.toml
[services.tty-dev]
task = "tty-dev"
enable = true
restart = "always"
ports = { terminal = 9001 }

[[http.routes]]
path = "/tty/dev"
target = "service:tty-dev:terminal"
websocket = true
```

```bash
# 新命令（统一接口）
svcmgr service start tty-dev
svcmgr service stop tty-dev
# 删除 = 从配置文件移除
```

### cloudflared 迁移

#### 旧方式

```bash
# 旧命令
svcmgr tunnel login
svcmgr tunnel create my-tunnel
svcmgr tunnel add-ingress my-tunnel example.com http://localhost:3000
svcmgr tunnel route-dns my-tunnel example.com
svcmgr tunnel start my-tunnel
svcmgr tunnel stop my-tunnel
```

#### 新方式

```bash
# 便捷命令保留（可选）
svcmgr tunnel login
svcmgr tunnel create my-tunnel
svcmgr tunnel route-dns my-tunnel example.com

# 配置 ingress（手动编辑 ~/.cloudflared/config.yml）
# 或使用 mise 任务生成

# 启动隧道（统一接口）
svcmgr service start tunnel
svcmgr service status tunnel
svcmgr service stop tunnel
```

---

## 未来扩展

### 支持更多类似服务（零代码）

**示例：添加 frp 内网穿透**

```toml
# .config/mise/config.toml
[tools]
frp = "0.51.0"

[tasks.frpc]
run = "frpc -c {{env.FRP_CONFIG}}"
env = { FRP_CONFIG = "~/.config/frp/frpc.ini" }

# .config/mise/svcmgr/config.toml
[services.frpc]
task = "frpc"
enable = true
restart = "always"
```

**无需修改 svcmgr 代码**。

---

## 总结

### 推荐方案

| 组件 | 依赖管理 | 任务定义 | 进程管理 | 便捷命令 | 代码量 |
|------|---------|---------|---------|---------|-------|
| **ttyd** | mise tools | mise tasks | svcmgr service | ❌ 不需要 | 0 行 |
| **cloudflared** | mise tools | mise tasks | svcmgr service | ✅ 保留（可选） | ~200 行 |

### 关键优势

1. ✅ **代码减少 83%**：从 1165 行 → 200 行（可选）
2. ✅ **统一抽象**：ttyd 和 cloudflared 就是普通服务
3. ✅ **配置驱动**：添加新服务无需写代码
4. ✅ **灵活性高**：用户可自定义启动参数
5. ✅ **扩展简单**：支持任意长驻进程（frp、gotty、nginx 等）

### 折中选择

**cloudflared 保留便捷命令的理由**：

1. **复杂操作多**：创建隧道、配置 ingress、DNS 路由等
2. **用户体验**：`svcmgr tunnel create` 比 `mise run tunnel-create` 更直观
3. **代码量可接受**：~200 行封装 vs 865 行完整实现

**ttyd 不需要便捷命令的理由**：

1. **操作简单**：只需配置文件即可
2. **无复杂逻辑**：启动 ttyd 就是一行命令
3. **配置清晰**：TOML 配置比 CLI 参数更易维护

### 实施路径

#### Phase 1: 配置设计（1天）

- [ ] 设计 ttyd 服务配置模板
- [ ] 设计 cloudflared 服务配置模板
- [ ] 设计 HTTP 路由配置（WebSocket 支持）

#### Phase 2: 迁移文档（1天）

- [ ] 编写 ttyd 迁移指南
- [ ] 编写 cloudflared 迁移指南
- [ ] 创建配置示例

#### Phase 3: 实现（2-3天）

- [ ] 移除 `features/webtty.rs`
- [ ] 移除 `atoms/tunnel.rs`（865行）
- [ ] 实现 `cli/tunnel.rs`（200行，可选）
- [ ] 更新 HTTP 代理支持 WebSocket

#### Phase 4: 测试（1-2天）

- [ ] ttyd 服务生命周期测试
- [ ] cloudflared 服务生命周期测试
- [ ] WebSocket 代理测试
- [ ] 迁移验证测试

---

## 配置示例完整版

```toml
# ============================================================================
# .config/mise/config.toml（mise 配置）
# ============================================================================

[tools]
node = "22"
python = "3.12"
ttyd = "1.7.7"
cloudflared = "latest"

[env]
PROJECT_ROOT = "{{cwd}}"

# ---- ttyd 任务定义 ----
[tasks.tty-bash]
run = "ttyd -p 9001 -t titleFixed='Bash' bash"

[tasks.tty-python]
run = "ttyd -p 9002 -t titleFixed='Python' python"

[tasks.tty-node]
run = "ttyd -p 9003 -t titleFixed='Node REPL' node"

# ---- cloudflared 任务定义 ----
[tasks.tunnel-run]
run = "cloudflared tunnel run --config {{env.TUNNEL_CONFIG}} {{env.TUNNEL_ID}}"
env = {
    TUNNEL_CONFIG = "~/.cloudflared/config.yml",
    TUNNEL_ID = "a1b2c3d4-1234-5678-90ab-cdef12345678"
}

# ---- cloudflared 便捷任务 ----
[tasks.tunnel-create]
run = "cloudflared tunnel create {{arg(name='name')}}"

[tasks.tunnel-list]
run = "cloudflared tunnel list"

[tasks.tunnel-route-dns]
run = "cloudflared tunnel route dns {{arg(name='tunnel')}} {{arg(name='hostname')}}"

# ============================================================================
# .config/mise/svcmgr/config.toml（svcmgr 配置）
# ============================================================================

[features]
systemd = true
cgroups_v2 = true

# ---- ttyd 服务 ----
[services.tty-bash]
task = "tty-bash"
enable = true
restart = "always"
ports = { terminal = 9001 }

[services.tty-python]
task = "tty-python"
enable = false  # 默认禁用
restart = "always"
ports = { terminal = 9002 }

[services.tty-node]
task = "tty-node"
enable = false
restart = "always"
ports = { terminal = 9003 }

# ---- cloudflared 服务 ----
[services.tunnel]
task = "tunnel-run"
enable = true
restart = "always"

# ---- HTTP 路由配置 ----
[[http.routes]]
path = "/tty/bash"
target = "service:tty-bash:terminal"
websocket = true
strip_prefix = true

[[http.routes]]
path = "/tty/python"
target = "service:tty-python:terminal"
websocket = true
strip_prefix = true

[[http.routes]]
path = "/api"
target = "service:api:web"
strip_prefix = false
```

**用户操作**：

```bash
# 启动所有启用的服务
svcmgr service start --all

# 启动特定服务
svcmgr service start tty-bash
svcmgr service start tunnel

# 查看状态
svcmgr service status tty-bash

# 访问终端
curl http://localhost:8000/tty/bash  # 自动代理到 localhost:9001

# 停止服务
svcmgr service stop tty-bash
```
