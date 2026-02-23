# svcmgr Specifications (基于 mise 重新设计)

> 版本：2.0.0-draft
> 基于：MISE_REDESIGN_RESEARCH_ZH.md

## 文档结构

### 00-09: 架构与核心组件

- **00-architecture-overview.md** - 整体架构概览
- **01-config-design.md** - 配置文件设计（svcmgr.toml + mise.toml）
- **02-scheduler-engine.md** - 多任务调度引擎设计
- **03-process-manager.md** - 子进程管理与资源限制
- **04-git-versioning.md** - Git 配置版本管理
- **05-web-service.md** - Web 服务与内置反向代理
- **06-feature-flags.md** - 功能开关机制
- **07-mise-integration.md** - mise 集成层（Port-Adapter 模式）

### 10-19: API 设计

- **10-api-overview.md** - API 设计总览
- **11-api-services.md** - 服务管理 API
- **12-api-tasks.md** - 任务管理 API
- **13-api-tools.md** - 工具管理 API（mise tools）
- **14-api-config.md** - 配置管理 API
- **15-api-env.md** - 环境变量 API

### 20-29: 实施与迁移

- **20-implementation-phases.md** - 实施路径（分阶段）
- **21-migration-guide.md** - 从旧架构迁移指南
- **22-breaking-changes.md** - 破坏性变更清单

## 核心设计原则

1. **配置文件驱动**：所有行为由 TOML 配置文件定义
2. **mise 作为基础设施**：依赖安装、环境变量、任务定义通过 mise 实现
3. **配置分离**：svcmgr 配置独立于 mise 配置，避免冲突
4. **Git 版本化**：配置变更通过 Git 暂存/提交/回滚管理
5. **事件驱动**：系统生命周期和任务状态通过事件总线通知
6. **功能开关**：核心功能可通过配置或环境变量开关控制

## 配置文件层级

```
.config/mise/                          # mise 配置目录
├── config.toml                        # mise 配置（tools, env, tasks）
├── conf.d/                            # mise 场景配置
│   └── *.toml
└── svcmgr/                            # svcmgr 配置（独立）
    ├── config.toml                    # svcmgr 核心配置
    └── conf.d/                        # svcmgr 场景配置
        └── *.toml
```

## 技术栈变更

| 组件 | 当前实现 | 新设计 |
|------|----------|--------|
| 依赖管理 | mise CLI 封装 | 配置文件驱动 + Port-Adapter |
| 任务定义 | mise CLI 封装 | 解析 mise [tasks] + 直接 spawn |
| 服务管理 | supervisor.rs | 调度引擎 + pitchfork 库内嵌 |
| 定时任务 | scheduler.rs | 调度引擎 + Cron 触发器 |
| 反向代理 | nginx 管理 | 内置 HTTP 代理（axum/hyper）|
| 资源限制 | 无 | cgroups v2（功能开关可关闭）|
| 配置管理 | 独立 TOML | Git 版本化 + svcmgr 独立配置 |

## 参考资料

- [MISE_REDESIGN_RESEARCH_ZH.md](../../MISE_REDESIGN_RESEARCH_ZH.md) - 完整设计文档
- [mise 官方文档](https://mise.jdx.dev)
- [pitchfork 参考](https://pitchfork.jdx.dev)
