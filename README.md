# svcmgr - Linux Service Management Tool

一个用于远程管理 Linux 服务环境的现代化工具，支持 systemd、crontab、mise、nginx 和 Cloudflare tunnels。

## 🚀 快速开始

```bash
# 编译项目
cargo build --release

# 安装（可选）
cargo install --path .

# 初始化环境
svcmgr setup

# 查看帮助
svcmgr --help
```

### 使用 Docker 🐳

```bash
# 拉取最新镜像
docker pull <your-dockerhub-username>/svcmgr:latest

# 运行容器
docker run -d \
  --name svcmgr \
  -p 8080:8080 \
  -v svcmgr-data:/home/svcmgr/.local/share/svcmgr \
  <your-dockerhub-username>/svcmgr:latest

# 查看日志
docker logs -f svcmgr

# 进入容器
docker exec -it svcmgr bash
```

**Docker 镜像标签**:
- `latest` - 最新的 main 分支构建
- `develop` - 开发分支构建
- `v1.0.0` - 语义化版本标签
- `main-<sha>` - 特定提交的构建

## 📋 功能特性

### Phase 1 (已完成) ✅
- ✅ **Git 原子模块**: 配置文件版本管理
- ✅ **CLI 框架**: setup/run/teardown 命令
- ✅ **配置管理**: XDG 标准路径

### Phase 2-7 (规划中)
- 🔜 **Systemd 管理**: 用户级服务管理
- 🔜 **Crontab 管理**: 周期任务管理
- 🔜 **Mise 集成**: 依赖和任务管理
- 🔜 **Nginx 代理**: HTTP/TCP 代理配置
- 🔜 **Cloudflare Tunnels**: 隧道管理
- 🔜 **Web TTY**: 基于 ttyd 的 Web 终端
- 🔜 **Frontend UI**: Vue 3 Web 管理界面

## 📖 文档

- [OpenSpec 规格文档](./openspec/README.md)
- [实施指南](./openspec/IMPLEMENTATION_GUIDE.md)
- [Phase 1 完成报告](./docs/PHASE1_COMPLETE.md)

## 🏗️ 项目结构

```
svcmgr/
├── src/
│   ├── atoms/          # 技术原子模块
│   │   └── git.rs      # Git 版本管理 ✅
│   ├── cli/            # CLI 命令
│   │   ├── setup.rs    # 初始化 ✅
│   │   ├── run.rs      # 启动服务 🔜
│   │   └── teardown.rs # 卸载 ✅
│   ├── config.rs       # 全局配置
│   ├── error.rs        # 错误类型
│   └── main.rs         # 入口
├── openspec/           # 规格文档
│   └── specs/          # 详细规格 (17 个文档)
└── tests/              # 集成测试
```

## 🧪 测试

```bash
# 运行所有测试
cargo test

# 运行特定测试
cargo test --test git_tests

# 查看测试输出
cargo test -- --nocapture
```

当前测试覆盖:
- ✅ Git 仓库初始化
- ✅ 文件提交和历史
- ✅ 版本回滚和恢复
- ✅ 版本差异比较

## 📦 CLI 命令

### `svcmgr setup`
初始化 svcmgr 环境：
- 创建配置目录 (`~/.local/share/svcmgr`)
- 初始化 Git 配置仓库
- 准备 nginx 和 web 目录

```bash
$ svcmgr setup
INFO Starting svcmgr setup...
INFO Created directory: ~/.local/share/svcmgr
INFO Initialized Git repository
INFO Setup completed successfully
```

### `svcmgr run`
启动 svcmgr 服务（Phase 2+ 实现）

### `svcmgr teardown`
卸载 svcmgr 并清理所有数据：

```bash
$ svcmgr teardown
WARN This will remove all svcmgr data. Continue? [y/N]: y
INFO Teardown completed successfully
```

## 🛠️ 技术栈

- **语言**: Rust (Edition 2021)
- **CLI**: clap 4 (derive)
- **异步**: tokio
- **Git**: git2 (libgit2 binding)
- **序列化**: serde + serde_json
- **日志**: tracing + tracing-subscriber
- **测试**: tempfile

## 🎯 设计原则

1. **技术原子正交**: 9 个独立技术原子可自由组合
2. **功能通过组合实现**: 业务功能由多个原子组合
3. **OpenSpec 驱动**: 严格遵循规格文档开发
4. **测试先行**: 每个模块都有完整测试

## 📊 进度

| Phase | 模块 | 状态 |
|-------|------|------|
| 1 | Git 原子 + CLI 框架 | ✅ 完成 |
| 2 | 模板引擎原子 | 🔜 待开始 |
| 3 | Systemd 原子 | 🔜 待开始 |
| 4 | Nginx 原子 | 🔜 待开始 |
| 5-7 | 其他原子和功能 | 🔜 待开始 |

## 🤝 贡献

项目处于早期开发阶段。欢迎：
- 🐛 Bug 报告
- 💡 功能建议
- 📝 文档改进
- 🧪 测试用例

## 📝 许可

MIT License

## 🔗 相关资源

- [OpenSpec 中文规范](./openspec/specs/README.md)
- [实施路线图](./openspec/IMPLEMENTATION_GUIDE.md)
- [前端设计](./openspec/specs/30-frontend-ui.md)

---

**当前版本**: v0.1.0 (Phase 1)  
**最后更新**: 2026-02-21
