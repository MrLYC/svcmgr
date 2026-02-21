# AI 助手指南 - SvcMgr

## 项目理解要点

1. **技术正交性**: 永远不要为特定功能硬编码实现，而是组合现有技术原子
2. **用户级**: 所有操作都在用户权限下执行（systemd --user, ~/.config 等）
3. **模板驱动**: 配置生成优先使用模板，而非字符串拼接

## Spec 文件结构

```
openspec/specs/
├── atoms/               # 技术原子规范（基础能力）
│   ├── git.md
│   ├── template.md
│   ├── mise.md
│   ├── systemd.md
│   ├── crontab.md
│   ├── cloudflare.md
│   └── nginx.md
├── features/            # 功能规范（原子组合）
│   ├── web-tty.md
│   ├── service-management.md
│   ├── cron-management.md
│   ├── proxy-management.md
│   ├── tunnel-management.md
│   └── config-versioning.md
├── api/                 # API 规范
│   └── rest-api.md
└── cli/                 # CLI 规范
    └── commands.md
```

## 实施规范时的注意事项

1. 每个技术原子必须完全独立，不依赖其他原子的实现细节
2. 功能规范必须明确列出所使用的原子组合
3. 新功能必须优先考虑复用现有原子

## 三阶段工作流

### Phase 1: Creating Changes

1. 在 `changes/[change-id]/` 创建提案
2. 编写 proposal.md 说明变更原因
3. 编写 tasks.md 列出实施步骤
4. 编写增量 specs

### Phase 2: Implementing Changes

1. 按 tasks.md 顺序实施
2. 每完成一个任务更新状态

### Phase 3: Archiving Changes

1. 将 delta specs 合并到主 specs
2. 移动 change 到 archive/

## Rust 代码规范

```rust
// 模块组织
src/
├── atoms/           // 技术原子实现
│   ├── git.rs
│   ├── template.rs
│   ├── mise.rs
│   ├── systemd.rs
│   ├── crontab.rs
│   ├── cloudflare.rs
│   └── nginx.rs
├── features/        // 功能组合层
├── api/            // REST API 路由
├── cli/            // CLI 入口
└── main.rs
```
