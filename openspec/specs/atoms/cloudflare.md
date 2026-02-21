# Cloudflare 隧道管理原子

## Overview

基于 cloudflared 提供安全隧道的创建和管理能力，支持将本地服务暴露到互联网。

---

## ADDED Requirements

### Requirement: Cloudflared 环境

系统 **MUST** 管理 cloudflared 的安装和认证。

#### Scenario: 检测 cloudflared 安装

- **WHEN** 系统启动或执行 setup 命令
- **THEN** 系统必须检测 cloudflared 是否已安装
- **AND** 可通过 mise 原子安装 cloudflared

#### Scenario: 认证状态检查

- **WHEN** 执行隧道操作前
- **THEN** 系统必须检查 cloudflared 认证状态
- **AND** 如未认证，提示执行 `cloudflared tunnel login`

#### Scenario: 存储认证凭证

- **WHEN** 完成 cloudflared 认证
- **THEN** 凭证文件存储于 `~/.cloudflared/`
- **AND** 系统不托管凭证文件（安全考虑）

---

### Requirement: 隧道生命周期管理

系统 **MUST** 管理隧道的创建、配置和删除。

#### Scenario: 创建新隧道

- **GIVEN** 隧道名称
- **WHEN** 请求创建隧道
- **THEN** 系统必须调用 `cloudflared tunnel create {name}`
- **AND** 保存隧道 ID 和凭证文件路径

#### Scenario: 列出隧道

- **WHEN** 请求隧道列表
- **THEN** 系统必须调用 `cloudflared tunnel list`
- **AND** 返回所有隧道的 ID、名称、创建时间

#### Scenario: 删除隧道

- **GIVEN** 隧道名称或 ID
- **WHEN** 请求删除隧道
- **THEN** 系统必须先停止隧道
- **AND** 调用 `cloudflared tunnel delete {name}`
- **AND** 清理相关配置文件

---

### Requirement: 隧道路由配置

系统 **MUST** 管理隧道的 ingress 规则。

#### Scenario: 添加 HTTP 路由

- **GIVEN** 隧道名称、域名、本地服务地址
- **WHEN** 请求添加路由
- **THEN** 系统必须更新隧道配置文件的 ingress 段
- **AND** 配置格式：`hostname: {domain} -> service: http://localhost:{port}`

#### Scenario: 添加 TCP 路由

- **GIVEN** 隧道名称、域名、本地 TCP 端口
- **WHEN** 请求添加 TCP 路由
- **THEN** 系统必须配置 `service: tcp://localhost:{port}`

#### Scenario: 添加通配符路由

- **GIVEN** 隧道名称和默认后端
- **WHEN** 请求添加 catch-all 路由
- **THEN** 系统必须在 ingress 末尾添加 `service: {backend}` 规则

#### Scenario: 移除路由

- **GIVEN** 隧道名称和域名
- **WHEN** 请求移除路由
- **THEN** 系统必须从配置中删除对应 ingress 规则

---

### Requirement: DNS 配置

系统 **MUST** 支持自动 DNS 配置。

#### Scenario: 创建 DNS 记录

- **GIVEN** 隧道名称和域名
- **WHEN** 请求配置 DNS
- **THEN** 系统必须调用 `cloudflared tunnel route dns {tunnel} {hostname}`
- **AND** 在 Cloudflare DNS 中创建 CNAME 记录

#### Scenario: 查询 DNS 路由

- **GIVEN** 隧道名称
- **WHEN** 请求 DNS 路由列表
- **THEN** 系统必须返回该隧道关联的所有 DNS 记录

---

### Requirement: 隧道运行管理

系统 **MUST** 管理隧道的运行状态。

#### Scenario: 启动隧道

- **GIVEN** 隧道名称
- **WHEN** 请求启动隧道
- **THEN** 系统必须通过 systemd 原子创建/启动隧道服务
- **AND** 服务执行 `cloudflared tunnel run {name}`

#### Scenario: 停止隧道

- **GIVEN** 隧道名称
- **WHEN** 请求停止隧道
- **THEN** 系统必须通过 systemd 原子停止隧道服务

#### Scenario: 查询隧道状态

- **GIVEN** 隧道名称
- **WHEN** 请求隧道状态
- **THEN** 系统必须返回：运行状态、连接数、最近错误

---

### Requirement: 隧道配置持久化

系统 **MUST** 持久化隧道配置。

#### Scenario: 保存隧道配置

- **WHEN** 隧道配置发生变更
- **THEN** 系统必须将配置写入托管目录
- **AND** 通过 git 原子记录变更

#### Scenario: 配置文件格式

- **WHEN** 生成隧道配置文件
- **THEN** 配置必须为 YAML 格式
- **AND** 路径为 `~/.config/svcmgr/managed/cloudflare/{tunnel-name}.yml`

---

### Requirement: Quick Tunnel 支持

系统 **SHOULD** 支持临时隧道（无需域名）。

#### Scenario: 创建临时隧道

- **GIVEN** 本地服务地址
- **WHEN** 请求创建 quick tunnel
- **THEN** 系统必须调用 `cloudflared tunnel --url {url}`
- **AND** 返回自动生成的 trycloudflare.com URL

#### Scenario: 临时隧道生命周期

- **WHEN** 临时隧道创建
- **THEN** 系统必须通过 systemd 原子的 systemd-run 运行
- **AND** 隧道停止后 URL 自动失效
