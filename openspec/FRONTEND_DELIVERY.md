# 前端规格交付摘要

## 📋 新增规格文档

### 📄 `30-frontend-ui.md` - 前端 Web UI 规格

**位置**: `openspec/specs/30-frontend-ui.md`

**规模统计**:
- ✅ 50+ 个需求 (Requirements)
- ✅ 80+ 个验收场景 (Scenarios)
- ✅ 9 个管理页面详细设计
- ✅ 完整的技术栈和架构设计

---

## 🎨 前端技术栈

### 核心框架
- **Vue 3** - 渐进式框架，Composition API
- **TypeScript** - 类型安全
- **Vite** - 快速构建工具
- **Tailwind CSS** - 实用优先的 CSS 框架
- **Pinia** - Vue 3 官方状态管理

### UI 组件库建议
- **Headless UI** - 无样式组件（推荐，配合 Tailwind）
- **Element Plus** - 成熟的 Vue 3 组件库
- **Ant Design Vue** - 企业级组件库

### 关键依赖
- **axios** - HTTP 客户端
- **vue-router** - 路由管理
- **xterm.js** - 终端渲染（TTY 功能）
- **monaco-editor** - 代码编辑器（配置文件编辑）
- **cronstrue** - Cron 表达式可读化

---

## 🏗️ 页面架构

### 9 个核心管理页面

#### 1️⃣ **Dashboard** (`/svcmgr`)
- 系统状态概览（CPU/内存/磁盘）
- 服务运行统计
- 近期活动日志
- 快速操作面板

#### 2️⃣ **Systemd Services** (`/svcmgr/systemd`)
- 服务列表（状态/PID/内存/启动时间）
- 服务增删改查
- 实时日志查看（journalctl 集成）
- 进程树可视化
- 临时任务（systemd-run）

#### 3️⃣ **Crontab Tasks** (`/svcmgr/crontab`)
- 任务列表（表达式/命令/下次执行时间）
- 可视化 Cron 表达式编辑器
- 预设模板（每日/每周/每月）
- 执行历史记录

#### 4️⃣ **Mise Management** (`/svcmgr/mise`)
- 工具版本管理（列表/安装/卸载）
- 全局任务定义和执行
- 环境变量配置
- 实时任务输出

#### 5️⃣ **Nginx Proxies** (`/svcmgr/nginx`)
- 代理规则列表（类型/路径/目标）
- 三种代理类型配置:
  - 静态文件托管
  - HTTP 反向代理
  - TCP 流代理
- 连通性测试
- 日志查看

#### 6️⃣ **Cloudflare Tunnels** (`/svcmgr/tunnels`)
- 隧道列表（UUID/域名/状态）
- Ingress 规则配置
- 连接状态监控
- 隧道日志

#### 7️⃣ **TTY Sessions** (`/svcmgr/tty`)
- Web 终端会话管理
- 基于模板创建终端
- xterm.js 渲染
- 会话列表（状态/启动时间）

#### 8️⃣ **Config Management** (`/svcmgr/config`)
- Git 仓库状态
- 变更 diff 查看
- 提交历史浏览
- 版本回滚
- Commit/Push/Pull 操作

#### 9️⃣ **Settings** (`/svcmgr/settings`)
- 系统配置
- 外部工具检测（mise/nginx/cloudflared/ttyd）
- 危险操作（teardown）
- 版本信息

---

## 🔌 API 设计规范

### RESTful 路由结构

```
GET    /svcmgr/api/systemd/services        # 列表
POST   /svcmgr/api/systemd/services        # 创建
GET    /svcmgr/api/systemd/services/:name  # 详情
PUT    /svcmgr/api/systemd/services/:name  # 更新
DELETE /svcmgr/api/systemd/services/:name  # 删除

POST   /svcmgr/api/systemd/services/:name/start    # 启动
POST   /svcmgr/api/systemd/services/:name/stop     # 停止
POST   /svcmgr/api/systemd/services/:name/restart  # 重启
GET    /svcmgr/api/systemd/services/:name/logs     # 日志
GET    /svcmgr/api/systemd/services/:name/tree     # 进程树
```

### 统一响应格式

```json
{
  "success": true,
  "data": { ... },
  "message": "操作成功"
}
```

```json
{
  "success": false,
  "error": "SERVICE_NOT_FOUND",
  "message": "服务不存在"
}
```

---

## 🎯 关键交互特性

### 实时更新
- **WebSocket** - 实时日志流（systemd logs, mise task output）
- **轮询** - 服务状态自动刷新（可配置间隔）

### 状态可视化
- **状态徽章**: 
  - 🟢 Running (绿色)
  - 🔴 Failed (红色)
  - 🟡 Starting (黄色)
  - ⚫ Stopped (灰色)

### 表单增强
- **配置预览** - 显示生成的 `.service` 文件、crontab 行
- **模板系统** - Jinja2 模板渲染预览
- **验证反馈** - 实时字段验证

### 危险操作确认
- **双重确认** - 删除服务、停止隧道
- **输入确认** - 输入服务名称才能删除

### 响应式设计
- **桌面优先** - 表格、多列布局
- **平板适配** - 侧边栏折叠
- **移动友好** - 卡片列表、底部导航

---

## 📂 前端项目结构

```
frontend/
├── package.json
├── vite.config.ts
├── tsconfig.json
├── tailwind.config.js
└── src/
    ├── main.ts
    ├── App.vue
    ├── router/
    │   └── index.ts              # 路由定义
    ├── stores/
    │   ├── system.ts             # 系统状态
    │   ├── systemd.ts            # Systemd 状态
    │   ├── crontab.ts            # Crontab 状态
    │   ├── mise.ts               # Mise 状态
    │   ├── nginx.ts              # Nginx 状态
    │   ├── tunnel.ts             # Tunnel 状态
    │   └── config.ts             # Git Config 状态
    ├── api/
    │   ├── client.ts             # Axios 客户端
    │   ├── systemd.ts            # Systemd API
    │   ├── crontab.ts            # Crontab API
    │   ├── mise.ts               # Mise API
    │   ├── nginx.ts              # Nginx API
    │   ├── tunnel.ts             # Tunnel API
    │   └── config.ts             # Config API
    ├── components/
    │   ├── common/
    │   │   ├── StatusBadge.vue   # 状态徽章
    │   │   ├── ConfirmDialog.vue # 确认对话框
    │   │   ├── LogViewer.vue     # 日志查看器
    │   │   └── Terminal.vue      # xterm.js 终端
    │   ├── forms/
    │   │   ├── SystemdForm.vue
    │   │   ├── CrontabForm.vue
    │   │   ├── NginxForm.vue
    │   │   └── TunnelForm.vue
    │   └── layout/
    │       ├── Sidebar.vue       # 侧边栏
    │       └── Header.vue        # 顶部导航
    └── views/
        ├── Dashboard.vue
        ├── SystemdPage.vue
        ├── CrontabPage.vue
        ├── MisePage.vue
        ├── NginxPage.vue
        ├── TunnelPage.vue
        ├── TTYPage.vue
        ├── ConfigPage.vue
        └── SettingsPage.vue
```

---

## 🚀 部署流程

### 1. 构建静态文件

```bash
cd frontend
npm install
npm run build  # 输出到 dist/
```

### 2. svcmgr setup 集成

**Rust 端实现**:
```rust
// 将编译好的前端嵌入二进制
include_bytes!("../frontend/dist/index.html")

// setup 时解压到目标目录
fn setup() {
    let web_dir = PathBuf::from("~/.local/share/svcmgr/web");
    fs::create_dir_all(&web_dir)?;
    
    // 解压嵌入的静态文件
    extract_embedded_files(&web_dir)?;
    
    // 配置 nginx
    configure_nginx_static_files(&web_dir)?;
}
```

### 3. Nginx 配置注入

```nginx
server {
    listen 8080;
    server_name _;
    
    # 前端静态文件
    location /svcmgr {
        alias ~/.local/share/svcmgr/web;
        try_files $uri $uri/ /svcmgr/index.html;
    }
    
    # API 代理
    location /svcmgr/api/ {
        proxy_pass http://127.0.0.1:3000/api/;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }
    
    # WebSocket (日志流)
    location /svcmgr/ws/ {
        proxy_pass http://127.0.0.1:3000/ws/;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }
}
```

---

## ✅ 验收标准

### 功能完整性
- [ ] 9 个页面全部可访问
- [ ] 所有 CRUD 操作正常工作
- [ ] 实时日志流正常显示
- [ ] 表单验证覆盖所有必填字段

### 用户体验
- [ ] 响应式设计在移动端可用
- [ ] 危险操作有确认对话框
- [ ] 加载状态有 loading 指示器
- [ ] 错误信息友好可读

### 性能
- [ ] 首屏加载 < 2s
- [ ] 列表渲染 > 100 条数据无卡顿
- [ ] 日志流不阻塞 UI

### 兼容性
- [ ] Chrome/Edge 最新版本
- [ ] Firefox 最新版本
- [ ] Safari 14+

---

## 📖 后续工作建议

### Phase 1: MVP（最小可用产品）
1. 实现 Dashboard + Systemd Services 页面
2. 建立基础 API 通信
3. 完成一个完整的 CRUD 流程

### Phase 2: 核心功能
4. Nginx Proxies 页面
5. Crontab Tasks 页面
6. 实时日志流功能

### Phase 3: 高级功能
7. Mise Management 页面
8. Cloudflare Tunnels 页面
9. Config Management 页面

### Phase 4: 增强特性
10. TTY Sessions + xterm.js 集成
11. Settings 页面 + 工具检测
12. 响应式优化 + 移动端适配

---

## 🔗 相关规格文档

- `00-architecture-overview.md` - 整体架构
- `07-atom-proxy.md` - Nginx 代理原子
- `20-cli-interface.md` - CLI 命令
- **`30-frontend-ui.md`** - **前端详细规格（新增）**

---

## 📞 技术决策说明

### 为什么选择 Vue 3？
- 渐进式框架，易于集成
- Composition API 适合逻辑复用
- 生态成熟，TypeScript 支持良好

### 为什么选择 Tailwind CSS？
- 无需维护大量自定义 CSS
- 快速原型开发
- 配合 Headless UI 灵活性强

### 为什么嵌入静态文件？
- 单一二进制部署，无需额外依赖
- 简化 `svcmgr setup` 流程
- 版本一致性保证

### API 通信方式选择
- **REST API**: 标准 CRUD 操作
- **WebSocket**: 实时日志流（长连接必要场景）
- **轮询**: 状态监控（简单场景，避免过度设计）

---

生成时间: 2026-02-21
