# Web TTY 功能

## Overview

提供基于 Web 的终端访问能力，允许通过浏览器访问远程终端会话。

## 技术原子组合

```
web_tty = template(mise-task-template)
        + mise-task(ttyd-runner)
        + systemd(systemd-run)
        + nginx(/tty/{name})
```

---

## ADDED Requirements

### Requirement: TTY 会话创建

系统 **MUST** 支持创建 Web TTY 会话。

#### Scenario: 创建默认 shell 会话

- **GIVEN** 会话名称
- **WHEN** 请求创建 TTY 会话
- **THEN** 系统必须：
  1. 通过 template 原子渲染 mise 任务配置
  2. 通过 mise-task 原子定义 ttyd 启动任务
  3. 通过 systemd 原子使用 systemd-run 启动临时任务
  4. 通过 nginx 原子配置 `/tty/{name}` 代理
- **AND** 返回访问 URL

#### Scenario: 创建自定义命令会话

- **GIVEN** 会话名称和自定义命令
- **WHEN** 请求创建 TTY 会话
- **THEN** ttyd 必须启动指定命令而非默认 shell

#### Scenario: 会话端口分配

- **WHEN** 创建 TTY 会话
- **THEN** 系统必须分配未使用的本地端口
- **AND** 端口范围：10000-20000

---

### Requirement: TTY 会话管理

系统 **MUST** 管理 TTY 会话生命周期。

#### Scenario: 列出活跃会话

- **WHEN** 请求 TTY 会话列表
- **THEN** 系统必须返回所有活跃的 TTY 会话
- **AND** 每项包含：名称、命令、端口、创建时间、访问 URL

#### Scenario: 终止会话

- **GIVEN** 会话名称
- **WHEN** 请求终止会话
- **THEN** 系统必须：
  1. 通过 systemd 原子停止临时任务
  2. 通过 nginx 原子移除代理配置
  3. 释放分配的端口

#### Scenario: 会话超时自动清理

- **GIVEN** 会话空闲超过配置的超时时间
- **WHEN** 系统执行清理检查
- **THEN** 系统应自动终止超时会话

---

### Requirement: TTY 访问控制

系统 **SHOULD** 提供 TTY 会话访问控制。

#### Scenario: 只读会话

- **GIVEN** readonly 参数为 true
- **WHEN** 创建 TTY 会话
- **THEN** ttyd 必须以只读模式启动
- **AND** 用户只能查看输出，不能输入

#### Scenario: 会话认证

- **GIVEN** 启用认证配置
- **WHEN** 访问 TTY 会话
- **THEN** 系统必须要求 HTTP Basic 认证
- **AND** 凭证由 nginx 层验证

---

### Requirement: TTY 模板

系统 **MUST** 提供 TTY 相关模板。

#### Scenario: mise 任务模板

- **WHEN** 创建 TTY 会话
- **THEN** 系统必须使用 `mise/ttyd-task.toml.j2` 模板
- **AND** 模板变量：name, port, command, readonly

#### Scenario: nginx 代理模板

- **WHEN** 配置 TTY 代理
- **THEN** 系统必须使用 `nginx/ttyd-proxy.conf.j2` 模板
- **AND** 包含 WebSocket 升级配置
