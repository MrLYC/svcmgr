# svcmgr 架构总览规格

> 版本：1.0.0
> 最后更新：2026-02-21

## 概述

svcmgr 是一个用于远程管理 Linux 服务环境的工具，采用**技术原子与功能正交**的架构设计。

## 设计原则

### Requirement: 技术原子正交性
系统 **MUST** 将技术实现分解为独立的原子模块，每个原子模块只负责单一技术领域。

#### Scenario: 功能组合
- **WHEN** 实现一个业务功能（如 Web TTY）
- **THEN** 系统 **MUST** 通过组合多个技术原子来实现
- **AND** 不得在业务功能中重复实现已有原子的能力

#### Scenario: 原子独立性
- **WHEN** 修改某个技术原子的实现
- **THEN** 不应影响其他技术原子
- **AND** 只需更新依赖该原子的功能组合的配置

---

## 技术原子清单

| 编号 | 原子名称 | 技术基础 | 规格文档 |
|------|----------|----------|----------|
| T01 | Git 版本管理 | git CLI | `01-atom-git.md` |
| T02 | 模板管理 | Jinja2 | `02-atom-template.md` |
| T03 | 依赖管理 | mise | `03-atom-dependency.md` |
| T04 | 全局任务 | mise tasks | `03-atom-dependency.md` |
| T05 | 环境变量 | mise env | `03-atom-dependency.md` |
| T06 | 服务管理 | systemd --user | `04-atom-systemd.md` |
| T07 | 周期任务 | crontab | `05-atom-crontab.md` |
| T08 | 隧道管理 | cloudflared | `06-atom-tunnel.md` |
| T09 | 服务代理 | nginx | `07-atom-proxy.md` |

---

## 功能模块清单

| 编号 | 功能名称 | 依赖原子 | 规格文档 |
|------|----------|----------|----------|
| F01 | Systemd 服务管理 | T02, T06, T07 | `10-feature-systemd.md` |
| F02 | Crontab 任务管理 | T02, T07 | `11-feature-crontab.md` |
| F03 | Mise 依赖管理 | T02, T03, T04, T05 | `12-feature-mise.md` |
| F04 | Nginx 代理管理 | T02, T09 | `13-feature-nginx.md` |
| F05 | Cloudflare 隧道管理 | T02, T08 | `14-feature-tunnel.md` |
| F06 | 配置文件管理 | T01 | `15-feature-config.md` |
| F07 | Web TTY | T02, T04, T06, T09 | `16-feature-webtty.md` |

---

## 统一代理路径规范

### Requirement: Nginx 统一入口
系统 **MUST** 通过 nginx 提供统一的 HTTP 入口，所有服务通过路径区分。

#### Scenario: 路径路由
- **WHEN** 外部请求到达 nginx
- **THEN** 系统 **MUST** 根据以下规则路由：

| 路径模式 | 目标 | 说明 |
|----------|------|------|
| `/` | 重定向到 `/svcmgr` | 默认入口 |
| `/svcmgr/*` | svcmgr API/UI | 管理服务 |
| `/tty/{name}` | ttyd 实例 | Web 终端 |
| `/port/{port}` | localhost:{port} | 端口转发 |
| `/static/*` | 静态文件目录 | 文件服务 |

---

## CLI 命令结构

### Requirement: 三阶段生命周期
系统 **MUST** 提供 setup/run/teardown 三阶段命令。

#### Scenario: 初始化
- **WHEN** 用户执行 `svcmgr setup`
- **THEN** 系统 **MUST** 初始化所有外部依赖（nginx、mise、cloudflared）
- **AND** 创建必要的目录结构和配置文件

#### Scenario: 运行
- **WHEN** 用户执行 `svcmgr run`
- **THEN** 系统 **MUST** 启动 svcmgr 服务
- **AND** 提供 Web API 和 UI

#### Scenario: 卸载
- **WHEN** 用户执行 `svcmgr teardown`
- **THEN** 系统 **MUST** 停止所有服务
- **AND** 可选择性清理配置

---

## 目录结构

```
~/.config/svcmgr/           # XDG 配置目录
├── config.toml             # 主配置文件
├── templates/              # 用户自定义模板
│   ├── systemd/
│   ├── crontab/
│   ├── nginx/
│   └── mise/
└── managed/                # 托管配置（Git 仓库）
    ├── .git/
    ├── systemd/            # systemd unit 文件
    ├── crontab/            # crontab 配置
    ├── nginx/              # nginx 配置片段
    └── mise/               # mise 任务定义

~/.local/share/svcmgr/      # XDG 数据目录
├── nginx/                  # nginx 运行时数据
├── logs/                   # 日志
└── state/                  # 状态文件
```
