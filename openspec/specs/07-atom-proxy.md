# T09: Nginx 服务代理原子

> 版本：1.0.0
> 技术基础：nginx (用户级)

## 概述

提供用户级别的 nginx 反向代理管理能力，支持 HTTP/HTTPS 和 TCP/UDP 代理。

---

## ADDED Requirements

### Requirement: 用户级 Nginx 部署
系统 **MUST** 支持在用户空间运行 nginx。

#### Scenario: 初始化 Nginx
- **WHEN** 首次设置 svcmgr
- **THEN** 系统 **SHALL** 创建用户级 nginx 配置
- **AND** 配置 nginx 监听非特权端口（>1024）
- **AND** 日志和运行时文件存放在用户目录

#### Scenario: Nginx 配置结构
- **WHEN** 初始化 nginx 配置
- **THEN** 系统 **SHALL** 创建以下结构：
```
~/.config/svcmgr/nginx/
├── nginx.conf          # 主配置
├── conf.d/             # 站点配置
│   ├── svcmgr.conf     # svcmgr API
│   ├── tty.conf        # TTY 代理
│   └── ports.conf      # 端口转发
└── includes/           # 可复用片段
    ├── proxy.conf
    └── websocket.conf

~/.local/share/svcmgr/nginx/
├── logs/
│   ├── access.log
│   └── error.log
├── run/
│   └── nginx.pid
└── cache/
```

---

### Requirement: 代理配置管理
系统 **MUST** 支持代理配置的增删改查。

#### Scenario: 添加 HTTP 代理
- **WHEN** 用户配置 HTTP 反向代理
- **THEN** 系统 **SHALL** 生成 location 配置块
- **AND** 包含必要的代理头设置

#### Scenario: 添加 WebSocket 代理
- **WHEN** 用户配置 WebSocket 代理
- **THEN** 系统 **SHALL** 添加 WebSocket 升级头
- **AND** 配置适当的超时时间

#### Scenario: 添加 TCP 代理
- **WHEN** 用户配置 TCP 流代理
- **THEN** 系统 **SHALL** 在 stream 块中添加配置
- **AND** 支持端口范围映射

#### Scenario: 删除代理
- **WHEN** 用户删除代理配置
- **THEN** 系统 **SHALL** 移除对应配置
- **AND** 重载 nginx 配置

---

### Requirement: 统一路径路由
系统 **MUST** 实现统一的路径路由规则。

#### Scenario: 默认路由
- **WHEN** 请求到达 nginx
- **THEN** 系统 **SHALL** 按以下规则路由：

| 路径 | 目标 | 配置文件 |
|------|------|----------|
| `/` | 重定向到 `/svcmgr` | svcmgr.conf |
| `/svcmgr/*` | svcmgr API (127.0.0.1:port) | svcmgr.conf |
| `/tty/{name}` | ttyd 实例 | tty.conf |
| `/port/{port}` | localhost:{port} | ports.conf |
| `/static/*` | 静态文件目录 | static.conf |

#### Scenario: TTY 路由
- **WHEN** 请求路径匹配 `/tty/{name}`
- **THEN** 系统 **SHALL** 代理到对应 ttyd 实例
- **AND** 支持 WebSocket 升级

#### Scenario: 端口转发路由
- **WHEN** 请求路径匹配 `/port/{port}`
- **THEN** 系统 **SHALL** 代理到 `localhost:{port}`
- **AND** 验证端口号有效性（1-65535）

---

### Requirement: 静态文件服务
系统 **MUST** 支持静态文件服务。

#### Scenario: 配置静态目录
- **WHEN** 用户配置静态文件服务
- **THEN** 系统 **SHALL** 添加 root 或 alias 指令
- **AND** 配置适当的 MIME 类型

#### Scenario: 目录列表
- **WHEN** 用户启用目录浏览
- **THEN** 系统 **SHALL** 添加 `autoindex on`

---

### Requirement: Nginx 生命周期
系统 **MUST** 支持 nginx 的启动、停止、重载。

#### Scenario: 启动 Nginx
- **WHEN** 用户启动 svcmgr
- **THEN** 系统 **SHALL** 启动用户级 nginx
- **AND** 可选通过 systemd --user 管理

#### Scenario: 配置重载
- **WHEN** 代理配置变更
- **THEN** 系统 **SHALL** 执行 `nginx -s reload`
- **AND** 验证配置语法后再重载

#### Scenario: 配置测试
- **WHEN** 重载配置前
- **THEN** 系统 **MUST** 执行 `nginx -t` 验证
- **AND** 验证失败时拒绝应用变更

---

## 接口定义

```rust
pub trait ProxyAtom {
    // Nginx 生命周期
    async fn start(&self) -> Result<()>;
    async fn stop(&self) -> Result<()>;
    async fn reload(&self) -> Result<()>;
    async fn status(&self) -> Result<NginxStatus>;
    async fn test_config(&self) -> Result<bool>;
    
    // HTTP 代理管理
    fn add_http_proxy(&self, config: &HttpProxyConfig) -> Result<()>;
    fn remove_http_proxy(&self, location: &str) -> Result<()>;
    fn list_http_proxies(&self) -> Result<Vec<HttpProxyConfig>>;
    
    // TCP 代理管理
    fn add_tcp_proxy(&self, config: &TcpProxyConfig) -> Result<()>;
    fn remove_tcp_proxy(&self, listen_port: u16) -> Result<()>;
    fn list_tcp_proxies(&self) -> Result<Vec<TcpProxyConfig>>;
    
    // 静态文件服务
    fn add_static_site(&self, config: &StaticSiteConfig) -> Result<()>;
    fn remove_static_site(&self, location: &str) -> Result<()>;
    fn list_static_sites(&self) -> Result<Vec<StaticSiteConfig>>;
    
    // TTY 路由（动态）
    fn add_tty_route(&self, name: &str, port: u16) -> Result<()>;
    fn remove_tty_route(&self, name: &str) -> Result<()>;
    fn list_tty_routes(&self) -> Result<Vec<TtyRoute>>;
}

pub struct HttpProxyConfig {
    pub location: String,           // 路径前缀
    pub upstream: String,           // 目标地址
    pub websocket: bool,            // 是否启用 WebSocket
    pub proxy_headers: HashMap<String, String>,
}

pub struct TcpProxyConfig {
    pub listen_port: u16,
    pub upstream: String,           // host:port
}

pub struct StaticSiteConfig {
    pub location: String,
    pub root: PathBuf,
    pub autoindex: bool,
    pub index: Vec<String>,
}

pub struct TtyRoute {
    pub name: String,
    pub port: u16,
}

pub struct NginxStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub worker_processes: u32,
    pub connections: u32,
}
```

---

## 配置项

```toml
[nginx]
# nginx 可执行文件路径（使用 mise 安装）
binary = "nginx"

# 监听端口
listen_port = 8080

# 配置目录
config_dir = "~/.config/svcmgr/nginx"

# 数据目录
data_dir = "~/.local/share/svcmgr/nginx"

# Worker 进程数
worker_processes = "auto"

# 日志级别
error_log_level = "warn"
```

---

## 内置模板

### http-proxy.conf.j2
```jinja2
# {{ description | default("HTTP Proxy for " ~ location) }}
location {{ location }} {
    proxy_pass {{ upstream }};
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
    {% if websocket %}
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
    proxy_read_timeout 86400;
    {% endif %}
    {% for key, value in headers.items() %}
    proxy_set_header {{ key }} "{{ value }}";
    {% endfor %}
}
```

### websocket-proxy.conf.j2
```jinja2
# WebSocket proxy for {{ name }}
location /tty/{{ name }}/ {
    proxy_pass http://127.0.0.1:{{ port }}/;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
    proxy_set_header Host $host;
    proxy_read_timeout 86400;
}
```
