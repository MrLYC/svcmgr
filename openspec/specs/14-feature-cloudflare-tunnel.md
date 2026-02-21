# Feature Spec: Cloudflare Tunnel Management

**版本**: 1.0.0  
**状态**: Draft  
**创建日期**: 2026-02-21

## ADDED Requirements

### Requirement: Tunnel Configuration Management
系统 MUST 管理 Cloudflare 隧道配置。

#### Scenario: Create Tunnel
- **WHEN** 用户创建新隧道,指定隧道名称
- **THEN** 系统应执行 `cloudflared tunnel create <name>`
- **AND** 系统应保存隧道 credentials 到配置目录
- **AND** 系统应生成隧道配置文件
- **AND** 系统应返回隧道 UUID 和 credentials 路径

#### Scenario: List Tunnels
- **WHEN** 用户查询隧道列表
- **THEN** 系统应执行 `cloudflared tunnel list`
- **AND** 系统应返回结构化列表,包含:隧道名、UUID、创建时间、连接状态

#### Scenario: Delete Tunnel
- **WHEN** 用户删除隧道
- **THEN** 系统应先检查隧道是否正在运行
- **AND** 如果隧道运行中,系统应要求先停止服务
- **AND** 系统应执行 `cloudflared tunnel delete <uuid>`
- **AND** 系统应删除本地 credentials 文件

### Requirement: Ingress Rule Management
系统 MUST 支持配置隧道的 ingress 规则。

#### Scenario: Add Ingress Rule
- **WHEN** 用户添加 ingress 规则
- **THEN** 系统应支持以下参数:
  - `hostname`: 域名(可选,默认为 catch-all)
  - `path`: 路径模式(可选)
  - `service`: 后端服务地址(http://、tcp://、unix://)
- **AND** 系统应更新隧道配置文件的 `ingress` 部分
- **AND** 系统应验证至少有一个 catch-all 规则(service: http_status:404)

#### Scenario: Ingress Rule Priority
- **WHEN** 用户添加多个 ingress 规则
- **THEN** 系统应按以下优先级排序:
  1. 完整域名 + 路径
  2. 完整域名
  3. 通配符域名 + 路径
  4. 通配符域名
  5. Catch-all 规则(必须是最后一条)

#### Scenario: Update Ingress Rule
- **WHEN** 用户更新 ingress 规则
- **THEN** 系统应找到匹配的规则并更新
- **AND** 系统应重新验证规则完整性
- **AND** 系统应重新加载隧道配置

#### Scenario: Delete Ingress Rule
- **WHEN** 用户删除 ingress 规则
- **THEN** 系统应从配置中移除规则
- **AND** 如果是 catch-all 规则,系统应警告并拒绝删除
- **AND** 系统应重新加载隧道配置

### Requirement: Tunnel Service Management
系统 MUST 将隧道作为 systemd 服务运行。

#### Scenario: Start Tunnel as Service
- **WHEN** 用户启动隧道服务
- **THEN** 系统应创建 systemd user service
- **AND** service 应执行 `cloudflared tunnel run <uuid>`
- **AND** 系统应配置 credentials 文件路径
- **AND** 系统应配置自动重启策略

#### Scenario: Stop Tunnel Service
- **WHEN** 用户停止隧道服务
- **THEN** 系统应执行 `systemctl --user stop cloudflared-<name>`
- **AND** 系统应等待隧道连接优雅关闭

#### Scenario: Auto-start on Boot
- **WHEN** 用户启用隧道自动启动
- **THEN** 系统应执行 `systemctl --user enable cloudflared-<name>`
- **AND** 系统应确保 user lingering 已启用

### Requirement: Tunnel Status Monitoring
系统 MUST 支持查询隧道状态。

#### Scenario: Query Tunnel Status
- **WHEN** 用户查询隧道状态
- **THEN** 系统应返回以下信息:
  - 服务运行状态(active/inactive/failed)
  - 连接状态(connected/disconnected)
  - Connectors 数量
  - 最近错误信息

#### Scenario: Check Connection Health
- **WHEN** 用户检查隧道健康状态
- **THEN** 系统应执行 `cloudflared tunnel info <uuid>`
- **AND** 系统应返回连接详情:协议、边缘节点位置、延迟

### Requirement: DNS Integration
系统 SHOULD 支持自动配置 DNS 记录。

#### Scenario: Create DNS Record for Tunnel
- **WHEN** 用户为隧道配置域名
- **THEN** 系统应执行 `cloudflared tunnel route dns <uuid> <hostname>`
- **AND** 系统应创建 CNAME 记录指向隧道
- **AND** 系统应返回 DNS 传播状态

#### Scenario: List DNS Routes
- **WHEN** 用户查询 DNS 路由
- **THEN** 系统应执行 `cloudflared tunnel route list`
- **AND** 系统应返回所有域名到隧道的映射

#### Scenario: Delete DNS Route
- **WHEN** 用户删除 DNS 路由
- **THEN** 系统应执行 `cloudflared tunnel route delete dns <uuid> <hostname>`
- **AND** 系统应等待 DNS 记录删除确认

### Requirement: Configuration Template
系统 MUST 提供隧道配置模板。

#### Scenario: Built-in Tunnel Templates
- **WHEN** 用户使用隧道模板
- **THEN** 系统应提供以下模板:
  - `web-app`: 标准 Web 应用(HTTP/HTTPS)
  - `api-service`: API 服务(带速率限制)
  - `ssh-tunnel`: SSH 隧道(TCP)
  - `multi-service`: 多服务路由(基于子域名/路径)

#### Scenario: Generate Tunnel Config from Template
- **WHEN** 用户使用模板创建隧道
- **THEN** 系统应渲染模板生成配置文件
- **AND** 系统应验证所有必需参数已提供
- **AND** 系统应保存配置到 `~/.config/cloudflared/config-<name>.yaml`

### Requirement: Integration with Nginx
系统 MUST 支持隧道与本地 nginx 的集成。

#### Scenario: Expose Nginx via Tunnel
- **WHEN** 用户暴露 nginx 服务到隧道
- **THEN** 系统应创建 ingress 规则指向 nginx 监听地址
- **AND** 系统应支持路径前缀映射
- **AND** 系统应配置 WebSocket 支持(如需要)

#### Scenario: Unified Service Routing
- **WHEN** 用户配置 svcmgr 的统一路由
- **THEN** 系统应创建隧道 ingress:
  - `{domain}/` → nginx → svcmgr 后端
  - `{domain}/tty/*` → nginx → ttyd 服务
  - `{domain}/port/*` → nginx → 动态端口代理

### Requirement: Integration with Config Management
系统 MUST 将隧道配置纳入 git 版本管理。

#### Scenario: Auto-commit Tunnel Configuration
- **WHEN** 用户修改隧道配置
- **THEN** 系统应自动提交配置文件到 git
- **AND** commit message 应描述具体变更(ingress规则/域名)
- **AND** 系统应排除 credentials 文件(敏感信息)

#### Scenario: Restore Tunnel Configuration
- **WHEN** 用户回滚隧道配置
- **THEN** 系统应从 git 历史恢复配置文件
- **AND** 系统应重新加载隧道服务

### Requirement: Security Best Practices
系统 MUST 遵循安全最佳实践。

#### Scenario: Protect Credentials
- **WHEN** 系统存储隧道 credentials
- **THEN** 系统应设置文件权限为 0600(仅所有者可读写)
- **AND** 系统应将 credentials 添加到 `.gitignore`

#### Scenario: Validate Service URLs
- **WHEN** 用户配置 ingress 服务地址
- **THEN** 系统应验证 URL 格式正确
- **AND** 系统应警告暴露敏感服务(如数据库)
- **AND** 系统应建议使用本地地址(127.0.0.1)而非公网地址

## Technical Notes

### Implementation Dependencies
- 技术原子: Template Management (02)
- 技术原子: Tunnel Management (06)
- 技术原子: Git Repository Management (01)
- 集成: Systemd Service Management (10)
- 集成: Nginx Proxy (13)

### Cloudflare Tunnel Directory Structure
```
~/.config/cloudflared/
├── config-{name}.yaml      # 隧道配置文件
└── {uuid}.json            # Credentials(git-ignored)

~/.local/share/cloudflared/
└── logs/
    └── {name}.log         # 隧道日志
```

### Tunnel Configuration Format
```yaml
# config-{name}.yaml
tunnel: {uuid}
credentials-file: /home/{user}/.config/cloudflared/{uuid}.json

ingress:
  - hostname: app.example.com
    service: http://localhost:8080
  - hostname: api.example.com
    path: /v1/*
    service: http://localhost:3000
  - hostname: ssh.example.com
    service: tcp://localhost:22
  # Catch-all 规则(必须)
  - service: http_status:404
```

### Cloudflared Command Reference
```bash
# 隧道管理
cloudflared tunnel create <name>
cloudflared tunnel list
cloudflared tunnel delete <uuid>
cloudflared tunnel info <uuid>

# 运行隧道
cloudflared tunnel run <uuid>
cloudflared tunnel run --config <path> <uuid>

# DNS 路由
cloudflared tunnel route dns <uuid> <hostname>
cloudflared tunnel route list
```

### Systemd Service Template
```ini
[Unit]
Description=Cloudflare Tunnel - {name}
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=/usr/local/bin/cloudflared tunnel run --config %h/.config/cloudflared/config-{name}.yaml {uuid}
Restart=on-failure
RestartSec=5s

[Install]
WantedBy=default.target
```

### Error Handling
- Credentials 文件缺失: 提示重新创建隧道
- Ingress 规则语法错误: 返回 cloudflared 验证错误
- 连接失败: 检查网络连接和 Cloudflare 账户状态
- DNS 记录冲突: 提示用户手动解决或使用其他域名
