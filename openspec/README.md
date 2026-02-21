# svcmgr OpenSpec 规格文档

本目录包含 svcmgr (Linux 服务管理工具) 的完整 OpenSpec 规格文档。

## 📚 文档结构

```
openspec/
├── README.md                      # 本文件 - 总览
├── IMPLEMENTATION_GUIDE.md        # 完整实施指南（推荐按阶段实施）
├── QUICK_START.md                 # MVP 快速开始指南（3-5天验证核心概念）
├── specs/                         # 详细规格文档
│   ├── README.md                  # 规格文档索引
│   ├── 00-architecture-overview.md      # 架构总览
│   ├── 01-atom-git.md                   # Git 版本管理原子
│   ├── 02-atom-template.md              # Jinja2 模板管理原子
│   ├── 03-atom-mise.md                  # mise 依赖/任务/环境变量原子
│   ├── 04-atom-systemd.md               # systemd 服务管理原子
│   ├── 05-atom-crontab.md               # crontab 周期任务原子
│   ├── 06-atom-tunnel.md                # Cloudflare 隧道管理原子
│   ├── 07-atom-proxy.md                 # nginx 代理管理原子
│   ├── 10-feature-systemd-service.md    # systemd 服务管理功能
│   ├── 11-feature-crontab.md            # crontab 任务管理功能
│   ├── 12-feature-mise.md               # mise 集成功能
│   ├── 13-feature-nginx-proxy.md        # nginx 代理配置功能
│   ├── 14-feature-cloudflare-tunnel.md  # Cloudflare 隧道功能
│   ├── 15-feature-config-management.md  # 配置文件管理功能
│   ├── 16-feature-webtty.md             # Web TTY 功能
│   └── 20-cli-interface.md              # CLI 接口规格
└── changes/                       # 提案和变更目录（待实施）
