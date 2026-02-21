# F07: Web TTY 功能

> 版本：1.0.0
> 依赖原子：T02 (模板), T04 (任务), T06 (systemd), T09 (代理)

## 概述

Web TTY 提供基于浏览器的终端访问能力，通过组合 mise 任务、ttyd、systemd 和 nginx 实现。

---

## ADDED Requirements

### Requirement: TTY 实例管理
系统 **MUST** 支持创建和管理多个独立的 Web 终端实例。

#### Scenario: 创建 TTY 实例
- **WHEN** 用户请求创建 Web 终端
- **THEN** 系统 **SHALL** 执行以下步骤：
  1. 使用 **T02** 渲染 mise 任务模板
  2. 通过 **T04** 在后台运行 ttyd 任务（使用 systemd-run）
  3. 使用 **T09** 添加 nginx 代理规则到 `/tty/{name}`

#### Scenario: TTY 配置选项
- **WHEN** 创建 TTY 实例时
- **THEN** 系统 **MUST** 支持以下配置：
  - `name`: 实例名称（用于路径）
  - `command`: 启动的 shell 或命令（默认 bash）
  - `port`: ttyd 监听端口（自动分配或指定）
  - `readonly`: 是否只读模式
  - `credential`: 认证凭证（用户名:密码）
  - `writable`: 是否允许写入

#### Scenario: 列出 TTY 实例
- **WHEN** 用户请求列出 TTY 实例
- **THEN** 系统 **SHALL** 返回所有运行中的 TTY 实例
- **AND** 包含：名称、命令、URL、状态、创建时间

---

### Requirement: TTY 生命周期
系统 **MUST** 支持 TTY 实例的启动、停止、重启。

#### Scenario: 停止 TTY
- **WHEN** 用户停止 TTY 实例
- **THEN** 系统 **SHALL**：
  1. 通过 **T06** 停止 systemd transient unit
  2. 通过 **T09** 移除 nginx 代理规则

#### Scenario: 持久化 TTY
- **WHEN** 用户请求将临时 TTY 转为持久服务
- **THEN** 系统 **SHALL**：
  1. 使用 **T02** 渲染 systemd service 模板
  2. 通过 **T06** 创建持久服务 unit
  3. 启用服务开机自启

---

### Requirement: 预定义 TTY 模板
系统 **MUST** 提供常用终端模板。

#### Scenario: Shell 模板
- **WHEN** 用户选择 shell 模板
- **THEN** 系统 **SHALL** 创建交互式 shell TTY
- **AND** 支持：bash, zsh, fish

#### Scenario: 特定命令模板
- **WHEN** 用户选择命令模板
- **THEN** 系统 **SHALL** 支持预定义命令：
  - `htop`: 系统监控
  - `logs`: 实时日志查看
  - `mise run {task}`: 运行特定 mise 任务

---

### Requirement: 安全性
系统 **SHOULD** 提供 TTY 访问控制。

#### Scenario: 认证
- **WHEN** 配置了认证凭证
- **THEN** ttyd **SHALL** 要求用户名密码认证

#### Scenario: 只读模式
- **WHEN** 启用只读模式
- **THEN** Web 终端 **SHALL** 禁止输入，仅显示输出

---

## 接口定义

```rust
pub struct WebTtyFeature {
    template: Arc<dyn TemplateAtom>,
    task: Arc<dyn TaskAtom>,
    systemd: Arc<dyn SystemdAtom>,
    proxy: Arc<dyn ProxyAtom>,
}

impl WebTtyFeature {
    /// 创建临时 TTY
    pub async fn create_transient(&self, config: &TtyConfig) -> Result<TtyInstance> {
        // 1. 分配端口
        let port = self.allocate_port()?;
        
        // 2. 使用 systemd-run 启动 ttyd
        let unit = self.systemd.run_transient(&TransientOptions {
            name: format!("tty-{}", config.name),
            command: vec![
                "ttyd".to_string(),
                "-p".to_string(), port.to_string(),
                "-W".to_string(),  // WebSocket
                config.command.clone(),
            ],
            // ... 其他选项
        }).await?;
        
        // 3. 添加 nginx 代理
        self.proxy.add_tty_route(&config.name, port).await?;
        
        Ok(TtyInstance {
            name: config.name.clone(),
            port,
            unit_name: unit.unit_name,
            url: format!("/tty/{}/", config.name),
        })
    }
    
    /// 转为持久服务
    pub async fn make_persistent(&self, name: &str) -> Result<()> {
        // 1. 获取当前 TTY 配置
        let instance = self.get(name)?;
        
        // 2. 渲染 systemd service 模板
        let mut ctx = Context::new();
        ctx.insert("name", &instance.name);
        ctx.insert("port", &instance.port);
        ctx.insert("command", &instance.command);
        let unit_content = self.template.render("ttyd-service", &ctx)?;
        
        // 3. 创建持久服务
        self.systemd.create_unit(&format!("tty-{}.service", name), &unit_content).await?;
        
        // 4. 停止临时单元
        self.systemd.stop_transient(&instance.unit_name).await?;
        
        // 5. 启动持久服务
        self.systemd.start(&format!("tty-{}.service", name)).await?;
        
        Ok(())
    }
    
    /// 停止并清理 TTY
    pub async fn remove(&self, name: &str) -> Result<()> {
        let instance = self.get(name)?;
        
        // 停止服务
        if instance.persistent {
            self.systemd.stop(&format!("tty-{}.service", name)).await?;
            self.systemd.delete_unit(&format!("tty-{}.service", name)).await?;
        } else {
            self.systemd.stop_transient(&instance.unit_name).await?;
        }
        
        // 移除 nginx 代理
        self.proxy.remove_tty_route(name)?;
        
        Ok(())
    }
    
    /// 列出所有 TTY 实例
    pub fn list(&self) -> Result<Vec<TtyInstance>> {
        // 查询 nginx 配置和 systemd 状态
        let routes = self.proxy.list_tty_routes()?;
        let mut instances = Vec::new();
        
        for route in routes {
            instances.push(self.get(&route.name)?);
        }
        
        Ok(instances)
    }
}

pub struct TtyConfig {
    pub name: String,
    pub command: String,
    pub port: Option<u16>,
    pub readonly: bool,
    pub credential: Option<String>,
}

pub struct TtyInstance {
    pub name: String,
    pub command: String,
    pub port: u16,
    pub url: String,
    pub unit_name: String,
    pub persistent: bool,
    pub status: TtyStatus,
}

pub enum TtyStatus {
    Running,
    Stopped,
    Failed,
}
```

---

## 内置模板

### ttyd-task.mise.j2
```toml
[tasks.tty-{{ name }}]
run = "ttyd -p {{ port }} -W {% if readonly %}--readonly {% endif %}{% if credential %}--credential {{ credential }} {% endif %}{{ command }}"
description = "Web TTY: {{ name }}"
```

### ttyd-service.service.j2
```jinja2
[Unit]
Description=Web TTY {{ name }}
After=network.target

[Service]
Type=simple
ExecStart=/usr/bin/ttyd -p {{ port }} -W {% if readonly %}--readonly {% endif %}{% if credential %}--credential {{ credential }} {% endif %}{{ command }}
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
```

---

## 配置项

```toml
[webtty]
# 端口范围
port_range_start = 9000
port_range_end = 9100

# 默认 shell
default_shell = "bash"

# 默认 ttyd 选项
[webtty.defaults]
readonly = false
writable = true
check_origin = false
max_clients = 10

# 预定义模板
[[webtty.templates]]
name = "bash"
command = "bash"
description = "Bash shell"

[[webtty.templates]]
name = "htop"
command = "htop"
description = "System monitor"
readonly = true

[[webtty.templates]]
name = "logs"
command = "journalctl -f --user"
description = "Live logs"
readonly = true
```

---

## API 端点

```
POST   /api/tty              # 创建 TTY
GET    /api/tty              # 列出所有 TTY
GET    /api/tty/{name}       # 获取 TTY 详情
DELETE /api/tty/{name}       # 删除 TTY
POST   /api/tty/{name}/persist  # 转为持久服务

# 访问 TTY
GET    /tty/{name}/          # Web 终端页面（由 nginx 代理到 ttyd）
```
