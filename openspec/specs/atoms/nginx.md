# Nginx 服务代理原子

## Overview

基于 nginx 提供用户级 HTTP/TCP 反向代理能力，统一管理所有对外服务的访问入口。

---

## ADDED Requirements

### Requirement: Nginx 用户级配置

系统 **MUST** 以用户权限运行和管理 nginx。

#### Scenario: 检测 nginx 安装

- **WHEN** 系统启动或执行 setup 命令
- **THEN** 系统必须检测 nginx 是否已安装
- **AND** 可通过 mise 原子安装 nginx

#### Scenario: 用户级 nginx 配置目录

- **WHEN** 初始化 nginx 环境
- **THEN** 系统必须使用用户目录存储配置
- **AND** 主配置路径：`~/.config/svcmgr/nginx/nginx.conf`
- **AND** 配置片段路径：`~/.config/svcmgr/managed/nginx/`

#### Scenario: 自定义监听端口

- **WHEN** 配置 nginx
- **THEN** 系统必须使用非特权端口（>1024）
- **AND** 默认 HTTP 端口为 8080

---

### Requirement: Nginx 进程管理

系统 **MUST** 管理 nginx 进程生命周期。

#### Scenario: 启动 nginx

- **WHEN** 请求启动 nginx
- **THEN** 系统必须通过 systemd 原子创建/启动 nginx 服务
- **AND** 使用用户级配置文件

#### Scenario: 重载配置

- **WHEN** 代理配置发生变更
- **THEN** 系统必须执行 `nginx -s reload`
- **AND** 验证配置语法正确后再重载

#### Scenario: 停止 nginx

- **WHEN** 请求停止 nginx
- **THEN** 系统必须通过 systemd 原子停止 nginx 服务

#### Scenario: 配置测试

- **WHEN** 修改配置前
- **THEN** 系统必须执行 `nginx -t` 验证配置
- **AND** 验证失败时拒绝应用并返回错误

---

### Requirement: HTTP 反向代理

系统 **MUST** 提供 HTTP 反向代理配置。

#### Scenario: 添加路径代理

- **GIVEN** 路径前缀和后端地址
- **WHEN** 请求添加代理规则
- **THEN** 系统必须调用 template 原子生成 nginx 配置
- **AND** 写入托管目录并 include 到主配置

#### Scenario: 标准代理路由

系统必须预配置以下路由：

- `/` → 301 重定向到 `/svcmgr`
- `/svcmgr/{path}` → svcmgr 后端服务
- `/tty/{name}` → ttyd websocket 代理
- `/port/{port}` → 本地端口透传

#### Scenario: WebSocket 代理

- **GIVEN** 后端服务使用 WebSocket
- **WHEN** 配置代理规则
- **THEN** 系统必须包含 WebSocket 升级头配置：
  - `proxy_http_version 1.1`
  - `proxy_set_header Upgrade $http_upgrade`
  - `proxy_set_header Connection "upgrade"`

#### Scenario: 删除代理规则

- **GIVEN** 路径前缀
- **WHEN** 请求删除代理
- **THEN** 系统必须移除对应配置文件
- **AND** 重载 nginx 配置

---

### Requirement: TCP 代理（Stream）

系统 **SHOULD** 支持 TCP 层代理。

#### Scenario: 添加 TCP 代理

- **GIVEN** 监听端口和后端地址
- **WHEN** 请求添加 TCP 代理
- **THEN** 系统必须在 nginx stream 模块配置代理
- **AND** 验证端口未被占用

#### Scenario: 删除 TCP 代理

- **GIVEN** 监听端口
- **WHEN** 请求删除 TCP 代理
- **THEN** 系统必须移除 stream 配置并释放端口

---

### Requirement: 代理规则查询

系统 **MUST** 提供代理规则查询。

#### Scenario: 列出所有代理规则

- **WHEN** 请求代理规则列表
- **THEN** 系统必须返回所有配置的代理规则
- **AND** 每项包含：路径/端口、后端地址、类型（HTTP/TCP/WebSocket）

#### Scenario: 查询规则详情

- **GIVEN** 路径或端口
- **WHEN** 请求规则详情
- **THEN** 系统必须返回完整的代理配置

---

### Requirement: 内置代理模板

系统 **MUST** 提供常用代理模板。

#### Scenario: 静态文件代理模板

- **WHEN** 使用 `static` 模板
- **THEN** 生成的配置必须包含：
  - root 指令指向文件目录
  - index 和 try_files 配置
  - 缓存头配置

#### Scenario: 反向代理模板

- **WHEN** 使用 `reverse-proxy` 模板
- **THEN** 生成的配置必须包含：
  - proxy_pass 指令
  - 标准代理头设置
  - 超时配置

#### Scenario: WebSocket 代理模板

- **WHEN** 使用 `websocket` 模板
- **THEN** 生成的配置必须包含：
  - WebSocket 升级头
  - 适当的超时设置
  - proxy_pass 指令

#### Scenario: 端口透传模板

- **WHEN** 使用 `port-forward` 模板
- **GIVEN** 目标端口号
- **THEN** 生成 `/port/{port}` 路径的代理配置
- **AND** 支持动态端口匹配

---

### Requirement: 访问日志

系统 **SHOULD** 提供代理访问日志。

#### Scenario: 配置访问日志

- **WHEN** 初始化 nginx
- **THEN** 系统必须配置访问日志路径
- **AND** 路径为 `~/.config/svcmgr/logs/nginx/access.log`

#### Scenario: 查询访问日志

- **WHEN** 请求访问日志
- **THEN** 系统必须返回最近的访问记录
- **AND** 支持按路径、时间过滤

---

### Requirement: SSL/TLS 支持

系统 **SHOULD** 支持 HTTPS 配置。

#### Scenario: 配置自签名证书

- **WHEN** 请求启用 HTTPS
- **AND** 未提供证书
- **THEN** 系统应生成自签名证书
- **AND** 配置 nginx 使用该证书

#### Scenario: 使用自定义证书

- **GIVEN** 证书和私钥路径
- **WHEN** 请求配置 HTTPS
- **THEN** 系统必须配置 nginx 使用指定证书
