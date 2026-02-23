# svcmgr 规范文档（基于 mise 重新设计）

> 版本：2.0.0-draft
> 基于：MISE_REDESIGN_RESEARCH_ZH.md

## 文档结构（20个规范文档）

### 00-09: 架构与核心组件

- **00-architecture-overview.md** - 整体架构概览
- **01-config-design.md** - 配置文件设计（svcmgr.toml + mise.toml）
- **02-scheduler-engine.md** - 多任务调度引擎设计
- **03-process-manager.md** - 子进程管理与资源限制
- **04-git-versioning.md** - Git 配置版本管理
- **05-web-service.md** - Web 服务与内置反向代理
- **06-feature-flags.md** - 功能开关机制
- **07-mise-integration.md** - mise 集成层（Port-Adapter 模式）
 **08-config-validation.md** - 配置校验与 Doctor 机制
 **09-credential-management.md** - 凭据管理模块（基于 fnox）

### 10-19: API 设计

- **10-api-overview.md** - API 设计总览
- **11-api-services.md** - 服务管理 API
- **12-api-tasks.md** - 任务管理 API
- **13-api-tools.md** - 工具管理 API（mise tools）
- **14-api-config.md** - 配置管理 API
- **15-api-env.md** - 环境变量管理 API（分层作用域、变量展开、批量操作）

### 20-29: 实施与迁移

- **20-implementation-phases.md** - 实施路径（分阶段）
- **21-migration-guide.md** - 从旧架构迁移指南
- **22-breaking-changes.md** - 破坏性变更清单（v1.x → v2.0 不兼容变更）

## 规范文档覆盖范围

### 架构与核心（11/11 完成）

| 文档 | 状态 | 描述 |
|------|------|------|
| ✅ 00-architecture-overview.md | 完成 | 4层架构、原子设计、Port-Adapter 模式 |
| ✅ 01-config-design.md | 完成 | 配置分离策略、svcmgr.toml + mise.toml 格式 |
| ✅ 02-scheduler-engine.md | 完成 | 统一调度引擎、4种触发器（OneShot, Delayed, Cron, Event） |
| ✅ 03-process-manager.md | 完成 | 进程生命周期、cgroups v2 资源限制、健康检查 |
| ✅ 04-git-versioning.md | 完成 | 自动暂存、回滚、审计日志、备份/恢复 |
| ✅ 05-web-service.md | 完成 | 内置 HTTP 反向代理（基于 axum），替代 nginx |
| ✅ 06-feature-flags.md | 完成 | 功能开关机制、自动检测、优雅降级 |
| ✅ 07-mise-integration.md | 完成 | Port-Adapter 防腐层、版本兼容性处理 |
| ✅ 08-config-validation.md | 完成 | 配置校验与 Doctor 机制，三层校验，自动修复 |
| ✅ 09-credential-management.md | 完成 | 凭据管理模块，HTTP 认证，基于 fnox，支持多种加密/远程提供者 |

### API 规范（6/6 完成）

| 文档 | 状态 | 描述 |
|------|------|------|
| ✅ 10-api-overview.md | 完成 | API 设计原则、REST 语义、认证、错误处理 |
| ✅ 11-api-services.md | 完成 | 服务 CRUD、生命周期管理、日志查询、13个 API 端点 |
| ✅ 12-api-tasks.md | 完成 | 任务管理、定时任务、执行历史、13个 API 端点 |
| ✅ 13-api-tools.md | 完成 | 工具安装、版本管理、插件支持、11个 API 端点 |
| ✅ 14-api-config.md | 完成 | 配置管理、Git 版本控制、验证、10个 API 端点 |
| ✅ 15-api-env.md | 完成 | 环境变量管理、分层作用域、变量展开、7个 API 端点 |

### 实施指南（3/3 完成）

| 文档 | 状态 | 描述 |
|------|------|------|
| ✅ 20-implementation-phases.md | 完成 | 分阶段实施路线图（5阶段，11-16周） |
| ✅ 21-migration-guide.md | 完成 | 从 systemd+cron+nginx 迁移指南，自动化工具、灰度策略 |
| ✅ 22-breaking-changes.md | 完成 | 破坏性变更清单（v1.x → v2.0）、迁移时间线 |

## 推荐阅读顺序

### 快速入门（新用户）

1. **00-architecture-overview.md** - 了解整体架构
2. **01-config-design.md** - 掌握配置格式
3. **20-implementation-phases.md** - 了解开发路线图

### 详细设计（开发者）

1. **00-architecture-overview.md** - 架构总览
2. **07-mise-integration.md** - mise 集成层设计
3. **02-scheduler-engine.md** - 调度引擎核心
4. **03-process-manager.md** - 进程管理实现
5. **05-web-service.md** - 内置 HTTP 代理
6. **10-15-api-*.md** - 完整 API 规范

### 迁移指南（v1.x 用户）

1. **22-breaking-changes.md** - 了解不兼容变更
2. **21-migration-guide.md** - 详细迁移步骤
3. **01-config-design.md** - 学习新配置格式
4. **20-implementation-phases.md** - 规划迁移时间表

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

- [MISE_REDESIGN_RESEARCH_ZH.md](../../docs/MISE_REDESIGN_RESEARCH_ZH.md) - 完整设计文档
- [mise 官方文档](https://mise.jdx.dev)
- [pitchfork 参考](https://pitchfork.jdx.dev)
