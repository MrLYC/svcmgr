# SvcMgr - Linux Service Manager

## Overview

SvcMgr 是一个用户级 Linux 服务管理工具，通过统一的 Web 界面远程管理 Linux 服务环境。核心设计理念是**技术与功能正交**：功能通过组合多个技术原子实现，避免重复实现相似能力。

## Architecture Principles

### 技术原子（Atomic Capabilities）

系统由以下不可再分的技术原子组成，每个原子只负责单一职责：

| 原子 ID      | 名称     | 底层技术       | 职责                       |
| ------------ | -------- | -------------- | -------------------------- |
| `git`        | 版本管理 | git            | 配置文件版本控制、变更追踪 |
| `template`   | 模板渲染 | Jinja2         | 配置模板管理和渲染         |
| `mise-dep`   | 依赖管理 | mise           | 用户级依赖安装和版本管理   |
| `mise-task`  | 全局任务 | mise           | 可复用的任务定义和执行     |
| `mise-env`   | 环境变量 | mise           | 环境变量定义和注入         |
| `systemd`    | 服务管理 | systemd --user | 用户级服务生命周期管理     |
| `crontab`    | 周期任务 | crontab        | 定时任务调度               |
| `cloudflare` | 隧道管理 | cloudflared    | 安全隧道创建和管理         |
| `nginx`      | 服务代理 | nginx          | HTTP/TCP 反向代理          |

### 功能组合模式

功能 = Σ(技术原子组合)

示例 - Web TTY 功能：

```
web_tty = template(mise_task_template)
        + mise_task(run_ttyd)
        + systemd(systemd-run)
        + nginx(/tty/{name})
```

## Technical Stack

- **语言**: Rust (Edition 2024)
- **Web 框架**: Axum
- **模板引擎**: minijinja (Jinja2 语法)
- **配置格式**: TOML (主配置), YAML (模板)
- **CLI 框架**: clap

## URL Routing Convention

nginx 统一代理路由：

```
/                    → 301 → /svcmgr
/svcmgr/{path}       → svcmgr 后端服务
/tty/{name}          → ttyd websocket 代理
/port/{port}         → 本地端口透传代理
```

## Directory Structure

```
~/.config/svcmgr/
├── config.toml              # 主配置
├── templates/               # 用户自定义模板
├── managed/                 # 托管配置目录（git 仓库）
│   ├── systemd/            # systemd unit 文件
│   ├── crontab/            # crontab 配置片段
│   ├── nginx/              # nginx 配置片段
│   └── cloudflare/         # cloudflare 隧道配置
└── state/                   # 运行时状态
```

## CLI Commands

```bash
svcmgr setup      # 初始化基础环境
svcmgr run        # 启动服务
svcmgr teardown   # 卸载基础环境
```

## Team Conventions

- 代码风格遵循 rustfmt 默认配置
- 错误处理使用 `thiserror` + `anyhow`
- 日志使用 `tracing`
- 异步运行时使用 tokio
