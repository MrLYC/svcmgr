# Feature Spec: Nginx-based Proxy Management

**版本**: 1.0.0  
**状态**: Draft  
**创建日期**: 2026-02-21

## ADDED Requirements

### Requirement: User-level Nginx Configuration
系统 MUST 管理用户级别的 nginx 代理配置。

#### Scenario: Initialize User Nginx
- **WHEN** 用户首次设置 nginx
- **THEN** 系统应在用户目录创建 nginx 配置结构
- **AND** 配置目录应为 `~/.config/nginx/`
- **AND** 系统应生成主配置文件 `nginx.conf`
- **AND** 系统应创建 `sites-available/` 和 `sites-enabled/` 目录
- **AND** 系统应配置 nginx 监听非特权端口(如 8080)

#### Scenario: Start User Nginx
- **WHEN** 用户启动 nginx 服务
- **THEN** 系统应创建 systemd user service 运行 nginx
- **AND** 系统应使用 `-c ~/.config/nginx/nginx.conf` 指定配置
- **AND** 系统应设置 `-p ~/.local/share/nginx/` 作为 prefix
- **AND** 系统应验证配置语法正确性(`nginx -t`)

#### Scenario: Reload Nginx Configuration
- **WHEN** 用户更新 nginx 配置
- **THEN** 系统应先验证配置语法
- **AND** 如果语法正确,系统应执行 `nginx -s reload`
- **AND** 如果 nginx 未运行,系统应提示用户启动服务

### Requirement: HTTP Proxy Configuration
系统 MUST 支持 HTTP 反向代理配置。

#### Scenario: Create HTTP Proxy
- **WHEN** 用户创建 HTTP 代理,指定路径和目标
- **THEN** 系统应生成 nginx location 配置
- **AND** 系统应支持以下参数:
  - `path`: 代理路径(如 `/api`)
  - `target`: 目标地址(如 `http://localhost:3000`)
  - `strip_prefix`: 是否移除路径前缀
  - `websocket`: 是否支持 WebSocket
- **AND** 系统应自动配置 proxy headers

#### Scenario: Proxy to Local Port
- **WHEN** 用户配置 `/port/{port}` 路径
- **THEN** 系统应动态代理到本地指定端口
- **AND** 系统应使用 nginx 正则表达式提取端口号
- **AND** 系统应配置为 `proxy_pass http://127.0.0.1:$1;`

#### Scenario: WebSocket Proxy Support
- **WHEN** 用户创建支持 WebSocket 的代理
- **THEN** 系统应添加以下 headers:
  ```nginx
  proxy_http_version 1.1;
  proxy_set_header Upgrade $http_upgrade;
  proxy_set_header Connection "upgrade";
  ```

### Requirement: Static File Serving
系统 MUST 支持静态文件服务配置。

#### Scenario: Serve Static Directory
- **WHEN** 用户配置静态文件服务
- **THEN** 系统应生成 location 配置指向指定目录
- **AND** 系统应支持以下参数:
  - `path`: URL 路径
  - `root`: 文件系统根目录
  - `index`: 默认索引文件列表
  - `autoindex`: 是否启用目录浏览
- **AND** 系统应配置合理的 MIME types

#### Scenario: Default Static File Template
- **WHEN** 用户使用静态文件模板
- **THEN** 系统应提供预配置选项:
  - SPA 应用(单页应用,try_files 回退到 index.html)
  - 文档站点(支持 .html 扩展名省略)
  - 文件服务器(启用 autoindex)

### Requirement: TCP Proxy Configuration
系统 MUST 支持 TCP 流代理(stream proxy)。

#### Scenario: Create TCP Proxy
- **WHEN** 用户创建 TCP 代理
- **THEN** 系统应在 nginx `stream` 块中配置
- **AND** 系统应支持以下参数:
  - `listen_port`: 监听端口
  - `target_host`: 目标主机
  - `target_port`: 目标端口
- **AND** 系统应配置合理的超时设置

#### Scenario: TCP Proxy for Database
- **WHEN** 用户使用数据库代理模板
- **THEN** 系统应使用适合数据库的超时配置
- **AND** 系统应支持 PostgreSQL、MySQL、Redis 等常见数据库

### Requirement: Built-in Proxy Templates
系统 MUST 提供常用代理配置模板。

#### Scenario: List Proxy Templates
- **WHEN** 用户请求代理模板列表
- **THEN** 系统应返回所有内置模板及其描述

#### Scenario: Built-in Templates
- **WHEN** 系统初始化
- **THEN** 系统应提供以下模板:
  - `static-spa`: 单页应用静态文件服务
  - `static-docs`: 文档站点(支持扩展名省略)
  - `reverse-proxy`: 标准 HTTP 反向代理
  - `websocket-proxy`: WebSocket 代理
  - `api-gateway`: API 网关(带速率限制)
  - `tcp-forward`: TCP 端口转发

### Requirement: Unified Routing Configuration
系统 MUST 提供统一的路由规则。

#### Scenario: Default Routing Rules
- **WHEN** 用户初始化 nginx
- **THEN** 系统应配置以下默认路由:
  - `/` → 重定向到 `/svcmgr`
  - `/svcmgr/*` → svcmgr 后端服务
  - `/tty/{name}` → 对应的 ttyd 服务
  - `/port/{port}` → 本地端口动态代理

#### Scenario: Add Custom Route
- **WHEN** 用户添加自定义路由
- **THEN** 系统应验证路径不与现有路由冲突
- **AND** 系统应按优先级排序路由规则
- **AND** 系统应生成对应的 location 配置

#### Scenario: Route Priority Management
- **WHEN** 多个路由匹配同一请求
- **THEN** 系统应按以下优先级处理:
  1. 精确匹配 `location = /path`
  2. 前缀匹配(最长优先) `location ^~ /path`
  3. 正则匹配 `location ~ pattern`
  4. 普通前缀 `location /path`

### Requirement: SSL/TLS Support
系统 SHOULD 支持 HTTPS 配置。

#### Scenario: Enable HTTPS
- **WHEN** 用户启用 HTTPS
- **THEN** 系统应配置 nginx 监听 443 端口(或其他指定端口)
- **AND** 系统应要求提供证书和私钥路径
- **AND** 系统应配置合理的 SSL 参数(协议、密码套件)

#### Scenario: Auto-redirect to HTTPS
- **WHEN** 用户启用 HTTPS 重定向
- **THEN** 系统应在 HTTP server 块配置 301 重定向
- **AND** 重定向应保留原始请求路径和查询参数

### Requirement: Integration with Config Management
系统 MUST 将 nginx 配置纳入 git 版本管理。

#### Scenario: Auto-commit Nginx Configuration
- **WHEN** 用户修改 nginx 配置
- **THEN** 系统应自动提交配置文件到 git
- **AND** commit message 应描述具体变更(路由/代理/静态文件)

#### Scenario: Restore Nginx Configuration
- **WHEN** 用户回滚 nginx 配置
- **THEN** 系统应从 git 历史恢复配置文件
- **AND** 系统应执行 `nginx -t` 验证配置
- **AND** 系统应重新加载 nginx

### Requirement: Access Log and Error Log
系统 MUST 配置访问日志和错误日志。

#### Scenario: Configure Logging
- **WHEN** 用户初始化 nginx
- **THEN** 系统应配置日志文件路径:
  - 访问日志: `~/.local/share/nginx/logs/access.log`
  - 错误日志: `~/.local/share/nginx/logs/error.log`
- **AND** 系统应配置合理的日志格式(包含响应时间)

#### Scenario: Query Access Logs
- **WHEN** 用户查询访问日志
- **THEN** 系统应解析日志文件并返回结构化数据
- **AND** 系统应支持过滤:时间范围、状态码、路径、IP

## Technical Notes

### Implementation Dependencies
- 技术原子: Template Management (02)
- 技术原子: Service Proxy (07)
- 技术原子: Git Repository Management (01)
- 集成: Systemd Service Management (10)

### User Nginx Directory Structure
```
~/.config/nginx/
├── nginx.conf              # 主配置文件
├── mime.types             # MIME 类型定义
├── sites-available/       # 可用站点配置
│   ├── default.conf
│   ├── svcmgr.conf
│   └── tty-*.conf
└── sites-enabled/         # 启用站点(符号链接)

~/.local/share/nginx/
├── logs/                  # 日志目录
│   ├── access.log
│   └── error.log
├── tmp/                   # 临时文件
└── cache/                 # 缓存目录
```

### Nginx Configuration Template
```nginx
# 主配置文件模板
user {username};
worker_processes auto;
error_log {home}/.local/share/nginx/logs/error.log;
pid {home}/.local/share/nginx/nginx.pid;

events {
    worker_connections 1024;
}

http {
    include mime.types;
    default_type application/octet-stream;
    
    log_format main '$remote_addr - $remote_user [$time_local] "$request" '
                    '$status $body_bytes_sent "$http_referer" '
                    '"$http_user_agent" $request_time';
    
    access_log {home}/.local/share/nginx/logs/access.log main;
    
    sendfile on;
    keepalive_timeout 65;
    
    include {home}/.config/nginx/sites-enabled/*.conf;
}

# TCP 代理块
stream {
    include {home}/.config/nginx/streams-enabled/*.conf;
}
```

### Port Selection Strategy
- HTTP 默认端口: 8080(用户可修改)
- HTTPS 默认端口: 8443(用户可修改)
- TCP 代理端口: 动态分配,避免冲突

### Error Handling
- 配置语法错误: 返回 `nginx -t` 的详细错误信息
- 端口占用: 提示用户选择其他端口
- 权限错误: 检查文件/目录权限并给出修复建议
