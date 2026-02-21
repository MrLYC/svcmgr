# svcmgr 实施指南

本指南提供了基于 OpenSpec 规格文档的分步骤实施路线图。

## 📋 规格文档总览

已创建的规格文档位于 `openspec/specs/` 目录，包括：

### 架构与原子模块 (00-07)
- `00-architecture-overview.md` - 整体架构设计
- `01-atom-git.md` - Git 版本管理原子
- `02-atom-template.md` - Jinja2 模板管理原子
- `03-atom-mise.md` - mise 依赖/任务/环境变量管理原子
- `04-atom-systemd.md` - systemd 服务管理原子
- `05-atom-crontab.md` - crontab 周期任务原子
- `06-atom-tunnel.md` - Cloudflare 隧道管理原子
- `07-atom-proxy.md` - nginx 代理管理原子

### 业务功能 (10-16)
- `10-feature-systemd-service.md` - systemd 服务管理功能
- `11-feature-crontab.md` - crontab 任务管理功能
- `12-feature-mise.md` - mise 集成功能
- `13-feature-nginx-proxy.md` - nginx 代理配置功能
- `14-feature-cloudflare-tunnel.md` - Cloudflare 隧道功能
- `15-feature-config-management.md` - 配置文件管理功能
- `16-feature-webtty.md` - Web TTY 功能

### CLI 接口 (20)
- `20-cli-interface.md` - 命令行接口规格

### 前端界面 (30)
- `30-frontend-ui.md` - Web 管理界面规格

## 🚀 推荐实施顺序

### Phase 1: 项目基础设施 (1-2 天)

#### Step 1.1: 项目初始化
```bash
# 创建 Rust 项目
cargo init --name svcmgr

# 添加基础依赖
cargo add clap --features derive
cargo add serde --features derive
cargo add serde_json
cargo add tokio --features full
cargo add anyhow
cargo add tracing
cargo add tracing-subscriber
```

**对应规格**: `20-cli-interface.md`

**验收标准**:
- [ ] 项目结构创建完成
- [ ] 基础依赖配置正确
- [ ] `svcmgr --version` 可运行

#### Step 1.2: CLI 框架搭建
实现基础命令结构：
- `svcmgr setup`
- `svcmgr run`
- `svcmgr teardown`
- `svcmgr config`

**对应规格**: `20-cli-interface.md`

**验收标准**:
- [ ] 所有子命令可识别
- [ ] 帮助信息完整
- [ ] 错误处理基本框架

---

### Phase 2: 核心技术原子 (3-5 天)

#### Step 2.1: 模板管理原子
```bash
cargo add tera  # Jinja2-like template engine for Rust
```

**对应规格**: `02-atom-template.md`

**实施任务**:
1. 创建 `src/atoms/template.rs`
2. 实现 `TemplateManager` 结构
3. 实现模板加载、渲染功能
4. 支持内置模板和用户模板
5. 编写单元测试

**验收标准**:
- [ ] 可从目录加载模板
- [ ] 支持 Jinja2 语法渲染
- [ ] 内置模板目录结构完整
- [ ] 单元测试覆盖率 > 80%

#### Step 2.2: Git 管理原子
```bash
cargo add git2
```

**对应规格**: `01-atom-git.md`

**实施任务**:
1. 创建 `src/atoms/git.rs`
2. 实现 `GitManager` 结构
3. 实现 init、add、commit、status、log 功能
4. 实现自动提交逻辑
5. 编写单元测试

**验收标准**:
- [ ] 可初始化 Git 仓库
- [ ] 可执行基本 Git 操作
- [ ] 自动提交功能正常
- [ ] 错误处理完善

#### Step 2.3: Systemd 管理原子
```bash
cargo add zbus --features tokio
cargo add systemd  # If available
```

**对应规格**: `04-atom-systemd.md`

**实施任务**:
1. 创建 `src/atoms/systemd.rs`
2. 实现 `SystemdManager` 结构
3. 实现服务 CRUD 操作
4. 实现服务状态查询
5. 实现日志查询功能
6. 实现 systemd-run 临时服务
7. 编写集成测试

**验收标准**:
- [ ] 可管理用户级 systemd 服务
- [ ] 服务状态查询准确
- [ ] 日志查询正常
- [ ] systemd-run 功能可用

#### Step 2.4: Crontab 管理原子
```bash
cargo add cron_parser
```

**对应规格**: `05-atom-crontab.md`

**实施任务**:
1. 创建 `src/atoms/crontab.rs`
2. 实现 `CrontabManager` 结构
3. 实现 crontab 条目解析
4. 实现 CRUD 操作
5. 实现时间表达式验证
6. 编写单元测试

**验收标准**:
- [ ] 可解析现有 crontab
- [ ] 可添加/修改/删除条目
- [ ] 时间表达式验证正确
- [ ] 不破坏现有 crontab

---

### Phase 3: 外部工具集成 (3-4 天)

#### Step 3.1: mise 集成原子
```bash
cargo add which  # For checking mise installation
```

**对应规格**: `03-atom-mise.md`

**实施任务**:
1. 创建 `src/atoms/mise.rs`
2. 实现 `MiseManager` 结构
3. 实现 .mise.toml 配置管理
4. 实现任务定义和执行
5. 实现环境变量管理
6. 实现依赖版本管理
7. 编写集成测试

**验收标准**:
- [ ] 可读写 .mise.toml
- [ ] 可定义和执行任务
- [ ] 环境变量管理正常
- [ ] 依赖查询功能可用

#### Step 3.2: nginx 代理原子
```bash
cargo add nginx-config  # If available, or use custom parser
```

**对应规格**: `07-atom-proxy.md`

**实施任务**:
1. 创建 `src/atoms/proxy.rs`
2. 实现 `ProxyManager` 结构
3. 实现 nginx 配置生成
4. 实现配置验证（nginx -t）
4. 实现配置重载
5. 实现内置代理模板（静态文件、端口转发、TCP 代理）
6. 编写集成测试

**验收标准**:
- [ ] 可生成 nginx 配置
- [ ] 配置验证功能正常
- [ ] 重载不中断服务
- [ ] 内置模板可用

#### Step 3.3: Cloudflare 隧道原子
```bash
cargo add cloudflare  # Cloudflare API client
```

**对应规格**: `06-atom-tunnel.md`

**实施任务**:
1. 创建 `src/atoms/tunnel.rs`
2. 实现 `TunnelManager` 结构
3. 实现 cloudflared 配置管理
4. 实现隧道 CRUD 操作
5. 实现 ingress 规则管理
6. 编写集成测试

**验收标准**:
- [ ] 可管理 cloudflared 配置
- [ ] 可创建/删除隧道
- [ ] ingress 规则管理正常
- [ ] 状态查询准确

---

### Phase 4: 业务功能组合 (4-6 天)

#### Step 4.1: systemd 服务管理功能
**对应规格**: `10-feature-systemd-service.md`

**实施任务**:
1. 创建 `src/features/systemd_service.rs`
2. 组合 `SystemdManager` + `TemplateManager`
3. 实现 CLI 子命令：
   - `svcmgr service list`
   - `svcmgr service add <name>`
   - `svcmgr service edit <name>`
   - `svcmgr service remove <name>`
   - `svcmgr service status <name>`
   - `svcmgr service start/stop/restart <name>`
   - `svcmgr service logs <name>`
   - `svcmgr service run <command>`
4. 实现服务模板库
5. 编写 E2E 测试

**验收标准**:
- [ ] 所有 CLI 命令可用
- [ ] 服务模板正常工作
- [ ] systemd-run 临时服务可用

#### Step 4.2: crontab 管理功能
**对应规格**: `11-feature-crontab.md`

**实施任务**:
1. 创建 `src/features/crontab.rs`
2. 组合 `CrontabManager` + `TemplateManager`
3. 实现 CLI 子命令：
   - `svcmgr cron list`
   - `svcmgr cron add <name>`
   - `svcmgr cron edit <name>`
   - `svcmgr cron remove <name>`
4. 实现任务模板（日/周/月）
5. 编写 E2E 测试

**验收标准**:
- [ ] 所有 CLI 命令可用
- [ ] 任务模板正常工作
- [ ] 时间表达式验证准确

#### Step 4.3: mise 管理功能
**对应规格**: `12-feature-mise.md`

**实施任务**:
1. 创建 `src/features/mise_mgmt.rs`
2. 组合 `MiseManager` + `TemplateManager`
3. 实现 CLI 子命令：
   - `svcmgr mise deps list`
   - `svcmgr mise deps add/remove <tool>`
   - `svcmgr mise task list`
   - `svcmgr mise task add/remove <name>`
   - `svcmgr mise env list`
   - `svcmgr mise env set/unset <key>`
4. 实现任务模板
5. 编写 E2E 测试

**验收标准**:
- [ ] 所有 CLI 命令可用
- [ ] mise 配置管理正常
- [ ] 任务模板可用

#### Step 4.4: nginx 代理管理功能
**对应规格**: `13-feature-nginx-proxy.md`

**实施任务**:
1. 创建 `src/features/proxy.rs`
2. 组合 `ProxyManager` + `TemplateManager`
3. 实现 CLI 子命令：
   - `svcmgr proxy list`
   - `svcmgr proxy add <name>`
   - `svcmgr proxy edit <name>`
   - `svcmgr proxy remove <name>`
   - `svcmgr proxy reload`
4. 实现内置代理模板
5. 编写 E2E 测试

**验收标准**:
- [ ] 所有 CLI 命令可用
- [ ] 代理模板正常工作
- [ ] 配置重载不中断服务

#### Step 4.5: Cloudflare 隧道管理功能
**对应规格**: `14-feature-cloudflare-tunnel.md`

**实施任务**:
1. 创建 `src/features/tunnel.rs`
2. 组合 `TunnelManager` + `TemplateManager`
3. 实现 CLI 子命令：
   - `svcmgr tunnel list`
   - `svcmgr tunnel add <name>`
   - `svcmgr tunnel edit <name>`
   - `svcmgr tunnel remove <name>`
   - `svcmgr tunnel status <name>`
4. 编写 E2E 测试

**验收标准**:
- [ ] 所有 CLI 命令可用
- [ ] 隧道管理正常
- [ ] ingress 规则可配置

#### Step 4.6: 配置文件管理功能
**对应规格**: `15-feature-config-management.md`

**实施任务**:
1. 创建 `src/features/config_mgmt.rs`
2. 组合 `GitManager`
3. 实现 CLI 子命令：
   - `svcmgr config init <dir>`
   - `svcmgr config add <dir>`
   - `svcmgr config list`
   - `svcmgr config remove <dir>`
   - `svcmgr config status <dir>`
   - `svcmgr config history <dir>`
   - `svcmgr config diff <dir>`
4. 实现自动提交机制
5. 编写 E2E 测试

**验收标准**:
- [ ] 所有 CLI 命令可用
- [ ] 自动提交正常工作
- [ ] 历史查询准确

#### Step 4.7: Web TTY 功能
**对应规格**: `16-feature-webtty.md`

**实施任务**:
1. 创建 `src/features/webtty.rs`
2. 组合 `MiseManager` + `SystemdManager` + `ProxyManager` + `TemplateManager`
3. 实现 CLI 子命令：
   - `svcmgr tty create <name> <command>`
   - `svcmgr tty list`
   - `svcmgr tty remove <name>`
4. 实现 mise 任务模板 + systemd-run 启动
5. 实现 nginx 路径 `/tty/{name}` 代理配置
6. 编写 E2E 测试

**验收标准**:
- [ ] 所有 CLI 命令可用
- [ ] TTY 服务可通过浏览器访问
- [ ] nginx 代理正常转发
- [ ] 服务停止后自动清理

---

### Phase 5: 环境管理命令 (2-3 天)

#### Step 5.1: setup 命令
**对应规格**: `20-cli-interface.md`

**实施任务**:
1. 创建 `src/commands/setup.rs`
2. 实现环境检查逻辑
3. 实现依赖安装（mise、nginx、cloudflared、ttyd）
4. 实现配置初始化
5. 实现内置模板部署
6. 编写集成测试

**验收标准**:
- [ ] 环境检查准确
- [ ] 依赖自动安装
- [ ] 配置文件正确生成
- [ ] 幂等性保证

#### Step 5.2: run 命令
**对应规格**: `20-cli-interface.md`

**实施任务**:
1. 创建 `src/commands/run.rs`
2. 实现 Web API 服务器（使用 axum 或 actix-web）
3. 实现认证中间件
4. 实现 RESTful API 端点
5. 实现前端静态文件服务
6. 编写 API 测试

**验收标准**:
- [ ] API 服务器可启动
- [ ] 认证功能正常
- [ ] 所有 API 端点可用
- [ ] 静态文件正确服务

#### Step 5.3: teardown 命令
**对应规格**: `20-cli-interface.md`

**实施任务**:
1. 创建 `src/commands/teardown.rs`
2. 实现服务停止逻辑
3. 实现配置清理（可选保留）
4. 实现卸载确认机制
5. 编写集成测试

**验收标准**:
- [ ] 所有服务正确停止
- [ ] 配置清理选项可用
- [ ] 卸载确认机制有效

---

### Phase 6: Web 界面 (可选，3-5 天)

#### Step 6.1: 前端项目初始化
```bash
cd frontend
npm init vite@latest . -- --template react-ts
npm install
```

#### Step 6.2: 功能页面开发
1. Dashboard（总览）
2. Service 管理页面
3. Crontab 管理页面
4. mise 管理页面
5. Proxy 管理页面
6. Tunnel 管理页面
7. Config 管理页面
8. TTY 列表页面

#### Step 6.3: 前后端集成
1. API 客户端封装
2. 状态管理（zustand/redux）
3. 路由配置
4. 认证流程
5. E2E 测试（Playwright）

---

## 🧪 测试策略

### 单元测试
每个技术原子都应有独立的单元测试，覆盖率 > 80%

```bash
cargo test --lib
```

### 集成测试
每个功能组合都应有集成测试，验证多个原子的协同工作

```bash
cargo test --test '*'
```

### E2E 测试
使用真实环境测试完整功能流程

```bash
# 创建测试环境
./scripts/setup_test_env.sh

# 运行 E2E 测试
cargo test --test e2e
```

---

## 📦 依赖管理

### Rust 依赖（Cargo.toml）

```toml
[dependencies]
# CLI
clap = { version = "4", features = ["derive"] }
anyhow = "1"
thiserror = "1"

# Async runtime
tokio = { version = "1", features = ["full"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"

# Template engine
tera = "1"

# Git integration
git2 = "0.18"

# systemd integration
zbus = { version = "4", features = ["tokio"] }

# Crontab parsing
cron_parser = "0.8"

# Process management
which = "6"

# Web server (for `svcmgr run`)
axum = "0.7"
tower-http = { version = "0.5", features = ["fs", "cors"] }

# Logging
tracing = "0.1"
tracing-subscriber = "0.3"

# Testing
[dev-dependencies]
tempfile = "3"
assert_cmd = "2"
predicates = "3"
```

### 系统依赖

```yaml
required:
  - mise (>= 2024.0.0)
  - systemd (user mode enabled)
  - nginx (>= 1.18)
  - git (>= 2.25)

optional:
  - cloudflared (for tunnel feature)
  - ttyd (for web tty feature)
```

---

## 🔧 开发环境设置

```bash
# 1. 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 2. 安装系统依赖
# Ubuntu/Debian
sudo apt install -y nginx git mise ttyd

# Arch Linux
sudo pacman -S nginx git mise ttyd

# 3. 启用用户级 systemd
systemctl --user daemon-reload

# 4. Clone 项目
git clone <repo-url> svcmgr
cd svcmgr

# 5. 构建项目
cargo build

# 6. 运行测试
cargo test

# 7. 本地运行
cargo run -- setup
cargo run -- run
```

---

## 📝 提交规范

使用 Conventional Commits 格式：

```
feat(systemd): add service management API
fix(template): handle empty template directory
docs(spec): update systemd atom specification
test(crontab): add integration tests for cron parser
refactor(proxy): simplify nginx config generation
```

---

## 🚢 发布流程

### 版本规划
- v0.1.0: Phase 1-3 完成（技术原子）
- v0.2.0: Phase 4-5 完成（业务功能）
- v0.3.0: Phase 6 完成（环境管理）
- v0.8.0: Phase 7 完成（Web 前端）
- v1.0.0: 稳定性验证 + 生产级优化

### 构建发布

```bash
# 构建 release 版本
cargo build --release

# 打包二进制
tar -czf svcmgr-${VERSION}-linux-x86_64.tar.gz \
  -C target/release svcmgr \
  -C ../../ templates/ \
  -C ../../ README.md

# 发布到 GitHub Releases
gh release create v${VERSION} \
  svcmgr-${VERSION}-linux-x86_64.tar.gz \
  --title "v${VERSION}" \
  --notes-file CHANGELOG.md
```

---

## 🎯 里程碑检查清单

### Milestone 1: 技术原子完成
- [ ] 所有 7 个技术原子模块实现完成
- [ ] 单元测试覆盖率 > 80%
- [ ] 集成测试全部通过
- [ ] 文档完善

### Milestone 2: 业务功能完成
- [ ] 所有 7 个业务功能实现完成
- [ ] E2E 测试全部通过
- [ ] CLI 命令完整
- [ ] 用户手册完成

### Milestone 3: 生产就绪
- [ ] 环境管理命令完成
- [ ] 性能测试通过
- [ ] 安全审计通过
- [ ] 发布流程验证

### Milestone 4: v1.0 发布
- [ ] Web 界面完成
- [ ] 完整的 E2E 测试套件
- [ ] 用户反馈收集和修复
- [ ] 文档和教程完善

---

## 📚 相关资源

### 规格文档
- [架构总览](specs/00-architecture-overview.md)
- [技术原子规格](specs/01-atom-git.md) - [07-atom-proxy.md]
- [业务功能规格](specs/10-feature-systemd-service.md) - [16-feature-webtty.md]
- [CLI 接口规格](specs/20-cli-interface.md)

### 外部文档
- [systemd 用户服务](https://www.freedesktop.org/software/systemd/man/systemd.service.html)
- [mise 文档](https://mise.jdx.dev/)
- [nginx 配置](https://nginx.org/en/docs/)
- [Cloudflare Tunnel](https://developers.cloudflare.com/cloudflare-one/connections/connect-apps/)
- [ttyd](https://github.com/tsl0922/ttyd)

---

## 💡 最佳实践建议

1. **遵循 OpenSpec 规范**: 所有实现必须严格遵循对应的规格文档
2. **测试驱动开发**: 先写测试，再写实现
3. **增量开发**: 按照推荐顺序逐步实现，避免跳跃式开发
4. **频繁集成**: 每个 Phase 完成后进行集成测试
5. **文档同步**: 代码变更时同步更新规格文档
6. **代码审查**: 每个 feature 完成后进行 code review
7. **性能优先**: 关注性能和资源消耗，特别是 systemd 和 nginx 操作
8. **安全第一**: 所有外部输入必须验证，避免命令注入

---

**注意**: 本指南基于 OpenSpec 规格文档生成，如规格有更新，请相应调整实施计划。
