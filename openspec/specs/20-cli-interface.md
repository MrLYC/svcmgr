# CLI 命令规格

> 版本：1.0.0

## 概述

定义 svcmgr 命令行接口的完整规格。

---

## ADDED Requirements

### Requirement: 三阶段生命周期命令
系统 **MUST** 提供 setup/run/teardown 三个生命周期命令。

#### Scenario: svcmgr setup
- **WHEN** 用户执行 `svcmgr setup`
- **THEN** 系统 **SHALL** 初始化以下组件：
  1. 创建配置目录结构
  2. 初始化 Git 仓库（T01）
  3. 安装必要工具通过 mise（nginx, ttyd, cloudflared）
  4. 生成默认配置文件
  5. 初始化 nginx 配置
  6. 创建必要的 systemd unit 文件

#### Scenario: svcmgr run
- **WHEN** 用户执行 `svcmgr run`
- **THEN** 系统 **SHALL**：
  1. 验证配置完整性
  2. 启动 nginx（如未运行）
  3. 启动 svcmgr API 服务
  4. 输出访问 URL

#### Scenario: svcmgr teardown
- **WHEN** 用户执行 `svcmgr teardown`
- **THEN** 系统 **SHALL**：
  1. 停止所有 svcmgr 管理的服务
  2. 询问是否保留配置文件
  3. 可选清理安装的工具

---

### Requirement: 子命令结构
系统 **MUST** 提供以下子命令分组。

#### Scenario: systemd 管理
```bash
svcmgr systemd list                    # 列出服务
svcmgr systemd create <name> [opts]    # 创建服务
svcmgr systemd start <name>            # 启动服务
svcmgr systemd stop <name>             # 停止服务
svcmgr systemd restart <name>          # 重启服务
svcmgr systemd status <name>           # 查看状态
svcmgr systemd logs <name> [opts]      # 查看日志
svcmgr systemd enable <name>           # 开机自启
svcmgr systemd disable <name>          # 禁用自启
svcmgr systemd remove <name>           # 删除服务
svcmgr systemd run <cmd>               # 运行临时命令
```

#### Scenario: crontab 管理
```bash
svcmgr cron list                       # 列出定时任务
svcmgr cron add <name> [opts]          # 添加任务
svcmgr cron update <name> [opts]       # 更新任务
svcmgr cron remove <name>              # 删除任务
svcmgr cron show <name>                # 查看任务详情
svcmgr cron next <name>                # 查看下次执行时间
```

#### Scenario: mise 管理
```bash
svcmgr mise install <tool>[@version]   # 安装工具
svcmgr mise uninstall <tool>           # 卸载工具
svcmgr mise list                       # 列出已安装
svcmgr mise task add <name> [opts]     # 添加任务
svcmgr mise task run <name> [args]     # 运行任务
svcmgr mise task list                  # 列出任务
svcmgr mise env set <key> <value>      # 设置环境变量
svcmgr mise env unset <key>            # 删除环境变量
svcmgr mise env list                   # 列出环境变量
```

#### Scenario: nginx 管理
```bash
svcmgr nginx start                     # 启动 nginx
svcmgr nginx stop                      # 停止 nginx
svcmgr nginx reload                    # 重载配置
svcmgr nginx status                    # 查看状态
svcmgr nginx proxy add <name> [opts]   # 添加代理
svcmgr nginx proxy remove <name>       # 删除代理
svcmgr nginx proxy list                # 列出代理
svcmgr nginx static add <name> [opts]  # 添加静态站点
svcmgr nginx static remove <name>      # 删除静态站点
```

#### Scenario: tunnel 管理
```bash
svcmgr tunnel login                    # Cloudflare 认证
svcmgr tunnel create <name>            # 创建隧道
svcmgr tunnel delete <name>            # 删除隧道
svcmgr tunnel list                     # 列出隧道
svcmgr tunnel config <name> [opts]     # 配置隧道
svcmgr tunnel start <name>             # 启动隧道
svcmgr tunnel stop <name>              # 停止隧道
svcmgr tunnel status <name>            # 查看状态
svcmgr tunnel dns <name> <hostname>    # 配置 DNS
```

#### Scenario: config 管理
```bash
svcmgr config init                     # 初始化配置仓库
svcmgr config status                   # 查看仓库状态
svcmgr config log [opts]               # 查看变更历史
svcmgr config diff [commit]            # 查看差异
svcmgr config revert <commit> [file]   # 回滚版本
svcmgr config push                     # 推送到远程
svcmgr config pull                     # 从远程拉取
svcmgr config remote <url>             # 配置远程仓库
```

#### Scenario: tty 管理
```bash
svcmgr tty create <name> [opts]        # 创建 Web 终端
svcmgr tty list                        # 列出终端
svcmgr tty remove <name>               # 删除终端
svcmgr tty persist <name>              # 转为持久服务
svcmgr tty url <name>                  # 获取访问 URL
```

#### Scenario: template 管理
```bash
svcmgr template list [category]        # 列出模板
svcmgr template show <name>            # 查看模板内容
svcmgr template add <name> <file>      # 添加用户模板
svcmgr template remove <name>          # 删除用户模板
```

---

### Requirement: 全局选项
系统 **MUST** 支持以下全局选项。

#### Scenario: 全局选项
```bash
svcmgr [global-opts] <subcommand>

Global Options:
  -c, --config <file>     配置文件路径 (默认: ~/.config/svcmgr/config.toml)
  -v, --verbose           详细输出
  -q, --quiet             静默模式
  --no-color              禁用颜色输出
  -h, --help              显示帮助
  -V, --version           显示版本
```

---

### Requirement: 输出格式
系统 **SHOULD** 支持多种输出格式。

#### Scenario: 格式选项
```bash
svcmgr <cmd> --format <format>

Formats:
  - table (默认)          # 表格格式
  - json                  # JSON 格式
  - yaml                  # YAML 格式
  - plain                 # 纯文本
```

---

## CLI 实现

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "svcmgr")]
#[command(about = "Linux service management tool", long_about = None)]
#[command(version)]
struct Cli {
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,
    
    #[arg(short, long)]
    verbose: bool,
    
    #[arg(short, long)]
    quiet: bool,
    
    #[arg(long)]
    no_color: bool,
    
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize svcmgr environment
    Setup {
        #[arg(long)]
        skip_tools: bool,
    },
    
    /// Run svcmgr service
    Run {
        #[arg(short, long, default_value = "8080")]
        port: u16,
        
        #[arg(long)]
        daemon: bool,
    },
    
    /// Teardown svcmgr environment
    Teardown {
        #[arg(long)]
        keep_config: bool,
    },
    
    /// Systemd service management
    #[command(alias = "svc")]
    Systemd {
        #[command(subcommand)]
        command: SystemdCommands,
    },
    
    /// Crontab management
    #[command(alias = "cron")]
    Crontab {
        #[command(subcommand)]
        command: CrontabCommands,
    },
    
    /// Mise management
    Mise {
        #[command(subcommand)]
        command: MiseCommands,
    },
    
    /// Nginx proxy management
    Nginx {
        #[command(subcommand)]
        command: NginxCommands,
    },
    
    /// Cloudflare tunnel management
    #[command(alias = "tunnel")]
    Tunnel {
        #[command(subcommand)]
        command: TunnelCommands,
    },
    
    /// Configuration repository management
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
    
    /// Web TTY management
    Tty {
        #[command(subcommand)]
        command: TtyCommands,
    },
    
    /// Template management
    Template {
        #[command(subcommand)]
        command: TemplateCommands,
    },
}

// SystemdCommands, CrontabCommands 等子命令定义...
```

---

## 交互式向导

### Requirement: 交互式创建
系统 **SHOULD** 提供交互式向导简化配置。

#### Scenario: 服务创建向导
```bash
svcmgr systemd create --interactive

# 系统提示：
? Service name: my-app
? Description: My Application
? Select template:
  > simple-service
    forking-service
    oneshot-service
    timer-service
? ExecStart command: /usr/bin/python app.py
? Working directory: /home/user/my-app
? Auto-start on boot? (y/N): y
✓ Service created: my-app.service
```

---

## 配置文件

### ~/.config/svcmgr/config.toml
```toml
[general]
# 管理配置目录
managed_dir = "~/.config/svcmgr/managed"

# 数据目录
data_dir = "~/.local/share/svcmgr"

# 日志级别
log_level = "info"

# 详细模式见各 atom 配置
[git]
auto_commit = true

[nginx]
listen_port = 8080

[webtty]
port_range_start = 9000
port_range_end = 9100

# ... 其他配置
```
