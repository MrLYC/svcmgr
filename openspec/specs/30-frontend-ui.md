# Spec 30: Frontend UI - Web 管理界面

## ADDED Requirements

### Requirement: 前端架构

系统 **MUST** 提供一个单页面应用（SPA）作为 Web 管理界面，用于可视化管理所有服务功能。

#### Scenario: 前端静态文件部署
- **WHEN** 用户执行 `svcmgr setup`
- **THEN** 系统应将前端静态文件（HTML/CSS/JS）部署到指定目录（如 `~/.local/share/svcmgr/web`）
- **AND** 系统应配置 nginx，将 `/svcmgr` 路径映射到该静态文件目录
- **AND** 访问 `http://localhost/svcmgr` 应返回前端应用的首页

#### Scenario: 前端技术栈
- **WHEN** 构建前端应用
- **THEN** 应使用现代前端框架（建议 Vue 3 + TypeScript）
- **AND** 应使用组件化设计，支持响应式布局
- **AND** 应使用 Tailwind CSS 或类似 utility-first CSS 框架
- **AND** 构建产物应为纯静态文件，无需 Node.js 运行时

#### Scenario: API 通信
- **WHEN** 前端需要与后端交互
- **THEN** 应通过 RESTful API 调用 `/svcmgr/api/*` 端点
- **AND** 所有请求应包含 CSRF token（如需要）
- **AND** 应处理 API 错误（401/403/404/500）并显示友好提示

---

### Requirement: 主导航布局

系统 **MUST** 提供侧边栏导航，组织所有功能模块。

#### Scenario: 侧边栏菜单结构
- **WHEN** 用户访问前端首页
- **THEN** 左侧应显示垂直侧边栏，包含以下菜单项：
  - **Dashboard**（仪表盘）
  - **Services**（服务管理）
    - Systemd Services
    - Crontab Tasks
    - Mise Tasks
  - **Proxy**（代理管理）
    - Nginx Proxies
    - Cloudflare Tunnels
  - **TTY**（终端管理）
  - **Config**（配置管理）
  - **Settings**（系统设置）
- **AND** 当前激活菜单项应高亮显示
- **AND** 点击菜单项应切换右侧内容区域（无页面刷新）

#### Scenario: 响应式导航
- **WHEN** 屏幕宽度小于 768px（移动设备）
- **THEN** 侧边栏应折叠为汉堡菜单按钮
- **AND** 点击按钮应展开/收起侧边栏
- **AND** 选择菜单项后，侧边栏应自动收起

---

### Requirement: Dashboard 仪表盘

系统 **MUST** 提供仪表盘页面，展示系统整体状态。

#### Scenario: 系统状态卡片
- **WHEN** 用户访问 Dashboard
- **THEN** 应显示以下状态卡片：
  - **Systemd Services**: 运行中/总数（如 "5 / 8 running"）
  - **Crontab Tasks**: 启用任务数
  - **Nginx Proxies**: 活跃代理数
  - **Cloudflare Tunnels**: 连接状态
- **AND** 每个卡片应显示图标、数值、状态颜色（绿色=正常，黄色=警告，红色=错误）
- **AND** 点击卡片应跳转到对应的详情页面

#### Scenario: 近期活动日志
- **WHEN** 在 Dashboard 页面
- **THEN** 应显示"最近活动"列表，包含：
  - 服务启动/停止事件
  - 配置变更记录
  - 错误/警告消息
- **AND** 每条记录应显示时间戳、类型图标、描述文本
- **AND** 最多显示最近 20 条记录，支持"查看全部"链接

#### Scenario: 快速操作面板
- **WHEN** 在 Dashboard 页面
- **THEN** 应提供快速操作按钮：
  - "新建 Systemd 服务"
  - "新建 Nginx 代理"
  - "新建 TTY 会话"
- **AND** 点击按钮应打开对应的创建对话框

---

### Requirement: Systemd Services 管理页面

系统 **MUST** 提供 Systemd 服务的可视化管理界面。

#### Scenario: 服务列表展示
- **WHEN** 用户访问"Systemd Services"页面
- **THEN** 应以表格形式显示所有用户级 systemd 服务：
  - **Name**（服务名称）
  - **Status**（状态：running/stopped/failed）
  - **Enabled**（是否自动启动）
  - **PID**（进程 ID）
  - **Memory**（内存占用）
  - **Uptime**（运行时长）
  - **Actions**（操作按钮）
- **AND** 状态列应使用彩色徽章（绿色=running，灰色=stopped，红色=failed）
- **AND** 表格应支持按名称/状态筛选和排序

#### Scenario: 服务操作按钮
- **WHEN** 在服务列表中
- **THEN** 每行的 Actions 列应包含：
  - **Start**（启动）按钮（当服务停止时）
  - **Stop**（停止）按钮（当服务运行时）
  - **Restart**（重启）按钮
  - **Enable/Disable**（启用/禁用自动启动）切换开关
  - **View Logs**（查看日志）按钮
  - **Edit**（编辑配置）按钮
  - **Delete**（删除）按钮
- **AND** 点击操作按钮应调用对应的 API
- **AND** 操作执行中应显示 loading 状态
- **AND** 操作完成后应显示成功/失败提示

#### Scenario: 创建新服务
- **WHEN** 用户点击"新建服务"按钮
- **THEN** 应打开模态对话框，包含表单字段：
  - **Name**（必填，服务名称）
  - **Description**（描述）
  - **ExecStart**（必填，启动命令）
  - **WorkingDirectory**（工作目录）
  - **Environment**（环境变量，支持多行）
  - **Restart Policy**（重启策略：no/on-failure/always）
  - **Template**（可选，选择预定义模板）
- **AND** 选择模板后应自动填充表单字段
- **AND** 应提供"预览配置文件"按钮，显示生成的 .service 文件内容
- **AND** 提交表单应调用 API 创建服务

#### Scenario: 查看服务日志
- **WHEN** 用户点击"View Logs"按钮
- **THEN** 应打开日志查看面板（可抽屉或全屏）
- **AND** 应显示 journalctl 日志输出（最近 100 行）
- **AND** 应支持实时刷新（类似 `tail -f`）
- **AND** 应支持筛选日志级别（ERROR/WARNING/INFO）
- **AND** 应提供"下载完整日志"按钮

#### Scenario: 编辑服务配置
- **WHEN** 用户点击"Edit"按钮
- **THEN** 应打开编辑对话框，加载当前服务的配置
- **AND** 应支持两种编辑模式：
  - **表单模式**（可视化编辑字段）
  - **文本模式**（直接编辑 .service 文件内容）
- **AND** 保存后应调用 API 更新服务配置并 reload systemd

#### Scenario: 查看进程树
- **WHEN** 用户点击服务行的"详情"链接
- **THEN** 应展开详情面板，显示进程树
- **AND** 应以树形结构显示主进程和子进程
- **AND** 每个进程应显示 PID、CPU%、内存占用

---

### Requirement: Crontab Tasks 管理页面

系统 **MUST** 提供 Crontab 任务的可视化管理界面。

#### Scenario: 任务列表展示
- **WHEN** 用户访问"Crontab Tasks"页面
- **THEN** 应以表格形式显示所有 crontab 任务：
  - **Schedule**（执行时间，如 "0 2 * * *"）
  - **Command**（执行命令）
  - **Description**（描述，从注释解析）
  - **Enabled**（是否启用）
  - **Last Run**（上次执行时间，需从日志推断）
  - **Actions**（操作按钮）
- **AND** Schedule 列应显示人类可读的描述（如 "Every day at 2:00 AM"）

#### Scenario: 创建新任务
- **WHEN** 用户点击"新建任务"按钮
- **THEN** 应打开对话框，包含表单：
  - **Template**（可选，选择日/周/月模板）
  - **Schedule**（Cron 表达式，提供可视化选择器）
  - **Command**（必填，执行命令）
  - **Description**（描述）
  - **Working Directory**（工作目录）
  - **Environment Variables**（环境变量）
- **AND** Cron 表达式选择器应包含：
  - **Minute/Hour/Day/Month/Weekday** 下拉选择
  - **预览**文本框显示生成的 cron 表达式
  - **人类可读描述**（如 "Every Monday at 9:00 AM"）
- **AND** 选择模板应自动填充 Schedule 和 Command

#### Scenario: 编辑和禁用任务
- **WHEN** 在任务列表中
- **THEN** 每行应包含：
  - **Toggle 开关**（启用/禁用，通过注释 cron 行实现）
  - **Edit**（编辑）按钮
  - **Delete**（删除）按钮
- **AND** 禁用任务应在 crontab 中添加 `#` 注释

---

### Requirement: Mise Tasks 管理页面

系统 **MUST** 提供 Mise 全局任务和依赖管理界面。

#### Scenario: 依赖版本展示
- **WHEN** 用户访问"Mise Tasks"页面的"Dependencies"标签
- **THEN** 应显示已安装工具列表：
  - **Tool**（工具名，如 "node", "python"）
  - **Current Version**（当前版本）
  - **Latest Version**（最新版本，可选）
  - **Source**（来源：.mise.toml 或全局配置）
  - **Actions**（更新/卸载按钮）

#### Scenario: 全局任务管理
- **WHEN** 用户访问"Mise Tasks"页面的"Tasks"标签
- **THEN** 应显示 mise 任务列表：
  - **Name**（任务名）
  - **Description**（描述）
  - **Command**（执行脚本）
  - **Actions**（运行/编辑/删除按钮）
- **AND** 点击"运行"按钮应：
  - 打开终端输出面板
  - 显示实时任务执行日志
  - 显示任务状态（running/success/failed）

#### Scenario: 创建 Mise 任务模板
- **WHEN** 用户点击"新建任务"按钮
- **THEN** 应打开对话框，包含：
  - **Name**（任务名）
  - **Description**（描述）
  - **Script**（多行文本框，支持 bash 语法高亮）
  - **Dependencies**（依赖的其他任务）
  - **Environment Variables**（环境变量）
- **AND** 应提供常用模板：
  - "备份数据库"
  - "清理日志"
  - "健康检查"

---

### Requirement: Nginx Proxies 管理页面

系统 **MUST** 提供 Nginx 代理配置的可视化管理界面。

#### Scenario: 代理列表展示
- **WHEN** 用户访问"Nginx Proxies"页面
- **THEN** 应显示代理配置列表：
  - **Path**（路径，如 "/app1"）
  - **Type**（类型：static/proxy_pass/tcp）
  - **Target**（目标：端口号或文件路径）
  - **Status**（状态：active/inactive）
  - **Actions**（操作按钮）
- **AND** 应支持按类型筛选（Static/HTTP Proxy/TCP Proxy）

#### Scenario: 创建静态文件代理
- **WHEN** 用户点击"新建代理"并选择"Static Files"模板
- **THEN** 应显示表单：
  - **Path**（URL 路径，如 "/myapp"）
  - **Root Directory**（文件系统路径，如 "/home/user/www"）
  - **Index File**（默认文件，如 "index.html"）
  - **Enable Directory Listing**（是否启用目录浏览）
- **AND** 保存后应生成 nginx 配置并 reload

#### Scenario: 创建 HTTP 反向代理
- **WHEN** 用户选择"HTTP Proxy"模板
- **THEN** 应显示表单：
  - **Path**（URL 路径，如 "/api"）
  - **Target Port**（目标端口，如 3000）
  - **Strip Path**（是否去除路径前缀）
  - **WebSocket Support**（是否支持 WebSocket）
  - **Custom Headers**（自定义请求头）

#### Scenario: 测试代理连通性
- **WHEN** 在代理列表或编辑对话框中
- **THEN** 应提供"Test"按钮
- **AND** 点击后应发送 HTTP 请求到代理路径
- **AND** 应显示响应状态码和响应时间
- **AND** 失败时应显示错误详情

---

### Requirement: Cloudflare Tunnels 管理页面

系统 **MUST** 提供 Cloudflare 隧道管理界面。

#### Scenario: 隧道列表展示
- **WHEN** 用户访问"Cloudflare Tunnels"页面
- **THEN** 应显示隧道列表：
  - **Name**（隧道名称）
  - **Hostname**（公网域名）
  - **Service**（本地服务地址，如 "http://localhost:3000"）
  - **Status**（连接状态：connected/disconnected）
  - **Actions**（操作按钮）

#### Scenario: 创建新隧道
- **WHEN** 用户点击"新建隧道"按钮
- **THEN** 应打开对话框，包含：
  - **Name**（隧道名称）
  - **Hostname**（公网域名，需在 Cloudflare 配置）
  - **Service**（本地服务 URL）
  - **Authentication**（认证方式：none/basic/oauth）
- **AND** 保存后应生成 cloudflared 配置并启动隧道服务

#### Scenario: 查看隧道状态
- **WHEN** 在隧道列表中
- **THEN** Status 列应显示：
  - **绿色圆点 + "Connected"**（隧道正常）
  - **红色圆点 + "Disconnected"**（隧道断开）
  - **黄色圆点 + "Connecting"**（连接中）
- **AND** 应显示连接时长（如 "Connected for 2h 15m"）

---

### Requirement: TTY 终端管理页面

系统 **MUST** 提供 Web TTY 会话管理界面。

#### Scenario: TTY 会话列表
- **WHEN** 用户访问"TTY"页面
- **THEN** 应显示已创建的 TTY 会话：
  - **Name**（会话名称）
  - **Command**（执行的命令）
  - **URL**（访问地址，如 "/tty/session1"）
  - **Status**（运行状态）
  - **Created At**（创建时间）
  - **Actions**（打开/停止/删除按钮）

#### Scenario: 创建新 TTY 会话
- **WHEN** 用户点击"新建 TTY"按钮
- **THEN** 应打开对话框，包含：
  - **Name**（会话名称，用于生成 URL）
  - **Command**（执行命令，如 "bash"）
  - **Template**（选择 mise 任务模板）
  - **Authentication**（是否需要密码）
- **AND** 保存后应：
  - 使用 systemd-run 启动 ttyd 临时服务
  - 配置 nginx 代理到 `/tty/{name}`
  - 返回访问 URL

#### Scenario: 打开 TTY 终端
- **WHEN** 用户点击"打开"按钮
- **THEN** 应在新标签页或嵌入式 iframe 中打开 TTY 终端
- **AND** 终端应支持全屏模式
- **AND** 应显示终端实时输出

---

### Requirement: Config 配置管理页面

系统 **MUST** 提供配置文件的 Git 版本管理界面。

#### Scenario: 配置目录状态
- **WHEN** 用户访问"Config"页面
- **THEN** 应显示配置目录信息：
  - **Path**（配置目录路径）
  - **Git Status**（是否为 Git 仓库）
  - **Current Branch**（当前分支）
  - **Uncommitted Changes**（未提交变更数量）
  - **Last Commit**（最后提交信息）

#### Scenario: 查看配置变更
- **WHEN** 配置目录有未提交变更
- **THEN** 应显示变更文件列表：
  - **File**（文件名）
  - **Status**（状态：Modified/Added/Deleted）
  - **Diff Preview**（差异预览，点击展开）
- **AND** 应提供"查看完整 Diff"按钮

#### Scenario: 提交配置变更
- **WHEN** 用户点击"提交变更"按钮
- **THEN** 应打开提交对话框：
  - **Commit Message**（必填，提交消息）
  - **Changed Files**（变更文件列表，支持选择性提交）
- **AND** 提交后应显示成功提示和 commit hash

#### Scenario: 查看提交历史
- **WHEN** 在 Config 页面点击"历史记录"标签
- **THEN** 应显示 Git 提交历史：
  - **Commit Hash**（短哈希）
  - **Message**（提交消息）
  - **Author**（作者）
  - **Date**（时间）
  - **Actions**（查看差异/回滚按钮）

#### Scenario: 回滚到历史版本
- **WHEN** 用户点击"回滚"按钮
- **THEN** 应显示确认对话框，警告可能的数据丢失
- **AND** 确认后应执行 `git revert` 或 `git reset` 操作
- **AND** 应重新加载受影响的服务配置

---

### Requirement: Settings 系统设置页面

系统 **MUST** 提供系统级配置管理界面。

#### Scenario: 基础设置
- **WHEN** 用户访问"Settings"页面
- **THEN** 应显示设置表单：
  - **Nginx Port**（Nginx 监听端口）
  - **Config Directory**（配置文件目录）
  - **Auto Commit**（是否自动提交配置变更）
  - **Log Level**（日志级别）
- **AND** 修改后应调用 API 更新配置

#### Scenario: 外部工具检测
- **WHEN** 在 Settings 页面的"Dependencies"标签
- **THEN** 应显示外部工具安装状态：
  - **systemd**（版本号或"未安装"）
  - **nginx**（版本号或"未安装"）
  - **cloudflared**（版本号或"未安装"）
  - **mise**（版本号或"未安装"）
  - **ttyd**（版本号或"未安装"）
- **AND** 未安装的工具应显示"安装"按钮
- **AND** 点击应执行 `svcmgr setup` 相关子步骤

#### Scenario: 系统重置
- **WHEN** 在 Settings 页面点击"危险操作"标签
- **THEN** 应显示"重置系统"按钮
- **AND** 点击后应显示确认对话框，要求输入 "RESET" 确认
- **AND** 确认后应调用 `svcmgr teardown` 清理所有配置

---

### Requirement: 通用 UI 组件

系统 **SHOULD** 提供一致的 UI 组件和交互模式。

#### Scenario: 状态徽章
- **WHEN** 需要显示状态信息
- **THEN** 应使用彩色徽章：
  - **绿色**（success）：运行中、成功、连接正常
  - **黄色**（warning）：警告、部分成功
  - **红色**（error）：失败、停止、错误
  - **灰色**（inactive）：未启用、停止
  - **蓝色**（info）：信息、进行中

#### Scenario: 确认对话框
- **WHEN** 用户执行危险操作（删除、停止、重置）
- **THEN** 应显示确认对话框，说明操作后果
- **AND** 高危操作（如删除服务）应要求输入服务名或"CONFIRM"确认

#### Scenario: 加载状态
- **WHEN** 执行 API 请求时
- **THEN** 应显示 loading 指示器（按钮 spinner 或全局加载条）
- **AND** 应禁用操作按钮防止重复提交

#### Scenario: 错误提示
- **WHEN** API 请求失败
- **THEN** 应显示 Toast 通知，包含：
  - **错误类型**（网络错误/权限错误/业务错误）
  - **错误消息**（友好的用户可读文本）
  - **错误详情**（可展开查看技术细节）
- **AND** 5秒后自动消失（可手动关闭）

#### Scenario: 命令输出显示
- **WHEN** 显示命令输出或日志
- **THEN** 应使用等宽字体（如 monospace）
- **AND** 应保留 ANSI 颜色代码（如有）
- **AND** 应支持自动滚动到底部

---

### Requirement: 响应式设计

系统 **MUST** 支持桌面和移动设备访问。

#### Scenario: 桌面布局（≥ 1024px）
- **WHEN** 屏幕宽度 ≥ 1024px
- **THEN** 应显示左侧固定侧边栏（宽度 240px）
- **AND** 右侧内容区域应自适应剩余宽度
- **AND** 表格应显示所有列

#### Scenario: 平板布局（768px - 1023px）
- **WHEN** 屏幕宽度在 768px - 1023px
- **THEN** 侧边栏应可折叠
- **AND** 表格应隐藏次要列（如 PID、Memory）

#### Scenario: 移动布局（< 768px）
- **WHEN** 屏幕宽度 < 768px
- **THEN** 表格应切换为卡片视图
- **AND** 每个服务/任务应显示为独立卡片
- **AND** 卡片应包含关键信息和快捷操作按钮

---

### Requirement: 权限和安全

系统 **SHOULD** 提供基础的身份验证和授权机制。

#### Scenario: HTTP Basic Auth（可选）
- **WHEN** 在 Settings 中启用"Require Authentication"
- **THEN** 访问 `/svcmgr` 应要求 HTTP Basic Auth
- **AND** 用户名密码应存储在配置文件中（加密）

#### Scenario: CSRF 保护
- **WHEN** 前端提交 POST/PUT/DELETE 请求
- **THEN** 应在请求头中包含 CSRF token
- **AND** 后端应验证 token 有效性

#### Scenario: 本地访问限制（推荐）
- **WHEN** 部署到生产环境
- **THEN** nginx 应配置为仅监听 `127.0.0.1`
- **AND** 外部访问应通过 Cloudflare Tunnel 或 SSH 隧道

---

## 依赖关系

- **依赖 Spec 20**: CLI Interface - 前端通过 API 调用后端子命令
- **依赖 Spec 04**: Systemd 原子 - 前端展示和操作 systemd 服务
- **依赖 Spec 05**: Crontab 原子 - 前端管理 crontab 任务
- **依赖 Spec 03**: Mise 原子 - 前端管理 mise 任务和依赖
- **依赖 Spec 07**: Nginx 原子 - 前端管理代理配置
- **依赖 Spec 06**: Cloudflare 原子 - 前端管理隧道
- **依赖 Spec 01**: Git 原子 - 前端展示配置变更历史

---

## 技术实现建议

### 推荐技术栈
```yaml
frontend:
  framework: Vue 3 + TypeScript
  build_tool: Vite
  ui_library: Element Plus / Naive UI / Ant Design Vue
  css: Tailwind CSS
  state_management: Pinia
  router: Vue Router
  http_client: Axios
  terminal: xterm.js (用于显示日志和 TTY)

deployment:
  build_output: dist/ (纯静态文件)
  install_path: ~/.local/share/svcmgr/web
  nginx_config: |
    location /svcmgr {
      alias ~/.local/share/svcmgr/web;
      try_files $uri $uri/ /svcmgr/index.html;
    }
```

### 目录结构建议
```
frontend/
├── src/
│   ├── components/          # 通用组件
│   │   ├── StatusBadge.vue
│   │   ├── ConfirmDialog.vue
│   │   └── Terminal.vue
│   ├── views/               # 页面组件
│   │   ├── Dashboard.vue
│   │   ├── SystemdServices.vue
│   │   ├── CrontabTasks.vue
│   │   ├── MiseTasks.vue
│   │   ├── NginxProxies.vue
│   │   ├── CloudflareTunnels.vue
│   │   ├── TTYSessions.vue
│   │   ├── ConfigManagement.vue
│   │   └── Settings.vue
│   ├── api/                 # API 客户端
│   │   ├── systemd.ts
│   │   ├── crontab.ts
│   │   ├── mise.ts
│   │   ├── nginx.ts
│   │   ├── tunnel.ts
│   │   └── config.ts
│   ├── stores/              # Pinia stores
│   │   └── system.ts
│   ├── router/              # 路由配置
│   │   └── index.ts
│   ├── App.vue
│   └── main.ts
├── public/                  # 静态资源
├── package.json
├── vite.config.ts
└── tsconfig.json
```

### API 接口约定
所有 API 端点应遵循 RESTful 风格：

```
GET    /svcmgr/api/systemd/services       # 列出服务
POST   /svcmgr/api/systemd/services       # 创建服务
GET    /svcmgr/api/systemd/services/:name # 获取服务详情
PUT    /svcmgr/api/systemd/services/:name # 更新服务
DELETE /svcmgr/api/systemd/services/:name # 删除服务
POST   /svcmgr/api/systemd/services/:name/start   # 启动服务
POST   /svcmgr/api/systemd/services/:name/stop    # 停止服务
GET    /svcmgr/api/systemd/services/:name/logs    # 获取日志

# 其他模块类似结构
```

响应格式统一为：
```json
{
  "success": true,
  "data": { ... },
  "message": "操作成功"
}
```

错误响应：
```json
{
  "success": false,
  "error": {
    "code": "SERVICE_NOT_FOUND",
    "message": "服务不存在",
    "details": "..."
  }
}
```

---

## 验收标准

1. ✅ 执行 `svcmgr setup` 后，访问 `http://localhost/svcmgr` 可打开 Web 界面
2. ✅ 所有功能页面（7个主要模块）均可正常访问和操作
3. ✅ 所有 CRUD 操作（创建/读取/更新/删除）均通过 API 实现
4. ✅ 实时状态更新（服务状态、日志输出）正常工作
5. ✅ 移动端访问时，界面自适应为卡片布局
6. ✅ 错误处理完善，所有 API 错误均有友好提示
7. ✅ 前端构建产物为纯静态文件，无需 Node.js 运行时
