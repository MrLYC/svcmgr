# OpenSpec 规格文档索引

> svcmgr - Linux 服务管理工具

## 📋 规格文档清单

### 架构与总览
- [00-architecture-overview.md](./00-architecture-overview.md) - 架构总览、设计原则、技术原子与功能清单

### 技术原子规格（9个）

#### 基础能力
- [01-atom-git.md](./01-atom-git.md) - **T01: Git 版本管理** - 配置文件版本控制
- [02-atom-template.md](./02-atom-template.md) - **T02: 模板管理** - Jinja2 模板渲染

#### 核心工具集成
- [03-atom-mise.md](./03-atom-mise.md) - **T03-T05: Mise 原子** - 依赖管理、全局任务、环境变量
- [04-atom-systemd.md](./04-atom-systemd.md) - **T06: Systemd 服务管理** - 用户级服务、日志、临时任务
- [05-atom-crontab.md](./05-atom-crontab.md) - **T07: Crontab 周期任务** - 定时任务管理
- [06-atom-tunnel.md](./06-atom-tunnel.md) - **T08: Cloudflare 隧道** - 安全隧道管理
- [07-atom-proxy.md](./07-atom-proxy.md) - **T09: Nginx 代理** - HTTP/TCP 代理、静态文件

### 功能组合规格（7个）

#### 已完成
- [16-feature-webtty.md](./16-feature-webtty.md) - **F07: Web TTY** - 浏览器终端（组合 T02/T04/T06/T09）

#### 待完成
- `10-feature-systemd.md` - **F01: Systemd 服务管理** - 组合 T02/T06/T07
- `11-feature-crontab.md` - **F02: Crontab 任务管理** - 组合 T02/T07
- `12-feature-mise.md` - **F03: Mise 依赖管理** - 组合 T02/T03/T04/T05
- `13-feature-nginx.md` - **F04: Nginx 代理管理** - 组合 T02/T09
- `14-feature-tunnel.md` - **F05: Cloudflare 隧道管理** - 组合 T02/T08
- `15-feature-config.md` - **F06: 配置文件管理** - 组合 T01

### CLI 接口
- [20-cli-interface.md](./20-cli-interface.md) - **CLI 命令规格** - 完整命令行接口定义

### 前端界面
- [30-frontend-ui.md](./30-frontend-ui.md) - **Web UI 规格** - 前端管理界面设计与交互

---

## 🎯 实现优先级

### Phase 1: 核心基础设施
1. **T01: Git 版本管理** - 配置持久化基础
2. **T02: 模板管理** - 配置生成基础
3. **T06: Systemd 服务管理** - 服务运行基础
4. **T09: Nginx 代理** - 统一入口基础

### Phase 2: 扩展能力
5. **T03-T05: Mise 原子** - 工具和任务管理
6. **T07: Crontab 周期任务** - 定时任务支持
7. **F07: Web TTY** - 第一个完整功能验证

### Phase 3: 高级特性
8. **T08: Cloudflare 隧道** - 外部访问能力
9. **F01-F06: 其他功能组合** - 完整业务功能

### Phase 4: Web UI
10. **F08: Web 管理界面** - Vue 3 + TypeScript SPA

### Phase 5: 完善与优化
11. CLI 完善、错误处理、测试覆盖
12. 文档和部署工具

---

## 📐 设计原则验证

### ✅ 技术原子正交性
- 每个原子独立实现单一技术领域
- 原子之间无直接依赖
- 功能通过组合实现

### ✅ 配置即代码
- 所有配置文件 Git 版本控制
- 模板化配置生成
- 可追溯的变更历史

### ✅ 用户级部署
- 无需 root 权限
- 使用 `systemd --user`
- 非特权端口（>1024）
- XDG 目录规范

### ✅ 统一入口
- Nginx 统一 HTTP 入口
- 路径规则清晰一致
- WebSocket 支持

---

## 🔗 依赖关系图

```
┌─────────────────────────────────────────┐
│          CLI Interface (20)              │
└─────────────────────────────────────────┘
                    │
        ┌───────────┴───────────┐
        │                       │
┌───────▼────────┐    ┌─────────▼──────┐
│   Features     │    │   Atoms        │
│   (F01-F07)    │◄───┤   (T01-T09)    │
│   10-16        │    │   01-07        │
└────────────────┘    └────────────────┘

功能依赖示例：
F07 (Web TTY) → T02, T04, T06, T09
F01 (Systemd) → T02, T06, T07
F06 (Config)  → T01
```

---

## 📝 文档规范

所有规格文档遵循 **OpenSpec 中文版格式**：

1. **Delta 分区**：ADDED / MODIFIED / REMOVED Requirements
2. **Requirement 语句**：系统 MUST/SHALL/SHOULD + 功能描述
3. **Scenario 场景**：WHEN / THEN / AND 描述行为
4. **接口定义**：Rust trait/struct 定义
5. **配置示例**：TOML 格式配置

---

## 🚀 下一步行动

1. **完成剩余功能规格**（F01-F06）
2. **初始化 Rust 项目结构**
3. **实现 Phase 1 核心原子**
4. **编写单元测试和集成测试**
5. **实现 CLI 框架**

---

## 📞 联系与反馈

规格文档持续演进中，欢迎反馈和建议。

---

**生成时间**: 2026-02-21  
**版本**: 1.0.0  
**状态**: 进行中（技术原子和 F07 已完成，F01-F06 待完成）
