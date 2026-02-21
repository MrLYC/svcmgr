# Feature: Dashboard Overview (F08)

**特性代号**: F08  
**原子依赖**: A04 (Systemd 服务管理), A05 (Crontab 任务管理), A07 (Nginx 代理), A06 (Cloudflare 隧道)  
**功能目标**: 提供系统概览仪表板，聚合展示各模块运行状态和活动日志

---

## 需求说明

### 概述

系统 **必须** 提供仪表板功能，在单个页面中聚合展示所有模块的关键指标和最近活动记录。仪表板 **不修改** 任何系统状态，仅提供只读视图用于快速监控。

### 核心能力

1. **统计数据聚合**: 展示各模块的关键指标（运行服务数、任务数、代理数、隧道数）
2. **活动日志查询**: 展示最近系统活动记录（按时间倒序）
3. **实时状态刷新**: 支持定期刷新获取最新状态

### 技术约束

- **API 基础路径**: `/svcmgr/api/dashboard` (统计数据), `/svcmgr/api/activity` (活动日志)
- **只读操作**: 仪表板不提供任何写操作，所有操作通过各功能模块的专属 API 完成
- **数据聚合**: 统计数据从各模块的数据源聚合计算
- **性能优化**: 统计查询应当优化为 O(1) 或 O(n) 时间复杂度（不触发复杂的递归查询）

---

## ADDED Requirements

### Requirement: 统计数据聚合
系统 **必须** 提供 REST API 端点用于查询各模块的关键统计指标。

#### Scenario: 查询仪表板统计数据
- **WHEN** 客户端发送 `GET /svcmgr/api/dashboard/stats` 请求
- **THEN** 系统 **应当** 聚合以下统计数据：
  - `systemd_running`: 正在运行的 systemd 服务数量
  - `systemd_total`: systemd 服务总数
  - `crontab_tasks`: crontab 任务总数
  - `nginx_proxies`: nginx 代理总数
  - `cloudflare_connected`: 已连接的 Cloudflare 隧道数量
  - `cloudflare_total`: Cloudflare 隧道总数
- **AND** HTTP 响应状态码 **应当** 为 `200 OK`

#### Scenario: 统计数据计算
- **WHEN** 系统计算统计数据时
- **THEN** 系统 **应当** 从各模块的数据源读取原始数据
- **AND** 系统 **应当** 执行简单的计数聚合（不执行复杂业务逻辑）
- **AND** 统计查询 **应当** 在 100ms 内完成

---

### Requirement: 活动日志查询
系统 **必须** 提供 REST API 端点用于查询最近的系统活动记录。

#### Scenario: 查询活动日志
- **WHEN** 客户端发送 `GET /svcmgr/api/activity` 请求
- **THEN** 系统 **应当** 返回最近 50 条活动记录的 JSON 数组（按时间倒序）
- **AND** 每个活动对象 **应当** 包含 `id, timestamp, type, action, description` 字段
- **AND** `type` 字段 **必须** 为 `"systemd" | "crontab" | "nginx" | "cloudflare" | "tty" | "config" | "system"` 之一
- **AND** HTTP 响应状态码 **应当** 为 `200 OK`

#### Scenario: 活动类型分类
- **WHEN** 系统记录活动时
- **THEN** 系统 **应当** 按模块分类活动类型：
  - `systemd`: systemd 服务操作（启动、停止、重启、启用、禁用）
  - `crontab`: crontab 任务操作（创建、更新、删除、切换启用状态）
  - `nginx`: nginx 代理操作（创建、更新、删除、测试）
  - `cloudflare`: Cloudflare 隧道操作（创建、更新、删除、连接、断开）
  - `tty`: TTY 会话操作（创建、启动、停止、删除）
  - `config`: 配置文件操作（提交、回滚）
  - `system`: 系统级操作（重置、设置更新）

#### Scenario: 空活动日志
- **WHEN** 系统中无任何活动记录
- **THEN** 系统 **应当** 返回空数组 `[]`
- **AND** HTTP 响应状态码 **应当** 为 `200 OK`

---

## REST API 接口规范

### 1. 获取仪表板统计数据

#### `GET /svcmgr/api/dashboard/stats`

**描述**: 获取各模块的关键统计指标

**请求参数**: 无

**响应** (200):
```json
{
  "systemd_running": 3,
  "systemd_total": 5,
  "crontab_tasks": 7,
  "nginx_proxies": 4,
  "cloudflare_connected": 1,
  "cloudflare_total": 2
}
```

**字段说明**:
- `systemd_running` (int): 正在运行的 systemd 服务数量
- `systemd_total` (int): systemd 服务总数
- `crontab_tasks` (int): crontab 任务总数（包括启用和禁用）
- `nginx_proxies` (int): nginx 代理总数
- `cloudflare_connected` (int): 状态为 `connected` 的 Cloudflare 隧道数量
- `cloudflare_total` (int): Cloudflare 隧道总数

**错误响应**:
- `500 INTERNAL_ERROR`: 系统内部错误

---

### 2. 获取活动日志

#### `GET /svcmgr/api/activity`

**描述**: 获取最近的系统活动记录（最多 50 条，按时间倒序）

**请求参数**: 无

**响应** (200):
```json
[
  {
    "id": "activity-001",
    "timestamp": "2026-02-21T10:55:00Z",
    "type": "systemd",
    "action": "restart",
    "description": "Restarted nginx.service"
  },
  {
    "id": "activity-002",
    "timestamp": "2026-02-21T10:30:00Z",
    "type": "crontab",
    "action": "run",
    "description": "Executed health-check.sh"
  },
  {
    "id": "activity-003",
    "timestamp": "2026-02-21T09:00:00Z",
    "type": "config",
    "action": "commit",
    "description": "Committed nginx config changes"
  },
  {
    "id": "activity-004",
    "timestamp": "2026-02-20T18:00:00Z",
    "type": "nginx",
    "action": "create",
    "description": "Created proxy /api -> localhost:8080"
  },
  {
    "id": "activity-005",
    "timestamp": "2026-02-20T15:30:00Z",
    "type": "cloudflare",
    "action": "connect",
    "description": "Tunnel main-tunnel connected"
  }
]
```

**字段说明**:
- `id` (string): 活动记录唯一标识符
- `timestamp` (string): 活动发生时间（ISO 8601 格式）
- `type` (string): 活动类型（模块分类）
- `action` (string): 操作类型（如 "create", "start", "stop", "commit"）
- `description` (string): 人类可读的活动描述

**错误响应**:
- `500 INTERNAL_ERROR`: 系统内部错误

---

## Rust 数据类型定义

### 仪表板统计数据

```rust
use serde::{Deserialize, Serialize};

/// 仪表板统计数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStats {
    /// 正在运行的 systemd 服务数量
    pub systemd_running: i32,
    
    /// systemd 服务总数
    pub systemd_total: i32,
    
    /// crontab 任务总数
    pub crontab_tasks: i32,
    
    /// nginx 代理总数
    pub nginx_proxies: i32,
    
    /// 已连接的 Cloudflare 隧道数量
    pub cloudflare_connected: i32,
    
    /// Cloudflare 隧道总数
    pub cloudflare_total: i32,
}
```

### 活动日志对象

```rust
/// 系统活动日志记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityLog {
    /// 活动记录唯一标识符
    pub id: String,
    
    /// 活动发生时间（ISO 8601 格式）
    pub timestamp: String,
    
    /// 活动类型（模块分类）
    #[serde(rename = "type")]
    pub activity_type: ActivityType,
    
    /// 操作类型
    pub action: String,
    
    /// 人类可读的活动描述
    pub description: String,
}

/// 活动类型（模块分类）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActivityType {
    /// Systemd 服务操作
    Systemd,
    
    /// Crontab 任务操作
    Crontab,
    
    /// Nginx 代理操作
    Nginx,
    
    /// Cloudflare 隧道操作
    Cloudflare,
    
    /// TTY 会话操作
    Tty,
    
    /// 配置文件操作
    Config,
    
    /// 系统级操作
    System,
}
```

---

## Rust Trait 接口定义

```rust
use async_trait::async_trait;
use crate::error::ApiError;

/// 仪表板功能特性
#[async_trait]
pub trait DashboardFeature {
    /// 获取仪表板统计数据
    async fn get_dashboard_stats(&self) -> Result<DashboardStats, ApiError>;
    
    /// 获取活动日志（最多 50 条，按时间倒序）
    async fn get_activity_logs(&self) -> Result<Vec<ActivityLog>, ApiError>;
}
```

### 实现说明

```rust
/// 仪表板功能实现
pub struct DashboardManager {
    systemd_feature: Arc<dyn SystemdFeature>,
    crontab_feature: Arc<dyn CrontabFeature>,
    nginx_feature: Arc<dyn NginxFeature>,
    cloudflare_feature: Arc<dyn CloudflareFeature>,
    activity_store: Arc<dyn ActivityStore>,
}

#[async_trait]
impl DashboardFeature for DashboardManager {
    async fn get_dashboard_stats(&self) -> Result<DashboardStats, ApiError> {
        // 1. 并发查询各模块数据（使用 tokio::join!）
        // 2. 聚合统计数据：
        //    - systemd_running: 过滤 status == "running" 的服务数量
        //    - systemd_total: 服务列表长度
        //    - crontab_tasks: 任务列表长度
        //    - nginx_proxies: 代理列表长度
        //    - cloudflare_connected: 过滤 status == "connected" 的隧道数量
        //    - cloudflare_total: 隧道列表长度
        // 3. 返回 DashboardStats 对象
        
        let (services, tasks, proxies, tunnels) = tokio::join!(
            self.systemd_feature.list_services(),
            self.crontab_feature.list_tasks(),
            self.nginx_feature.list_proxies(),
            self.cloudflare_feature.list_tunnels(),
        );
        
        let services = services?;
        let tasks = tasks?;
        let proxies = proxies?;
        let tunnels = tunnels?;
        
        Ok(DashboardStats {
            systemd_running: services.iter().filter(|s| s.status == "running").count() as i32,
            systemd_total: services.len() as i32,
            crontab_tasks: tasks.len() as i32,
            nginx_proxies: proxies.len() as i32,
            cloudflare_connected: tunnels.iter().filter(|t| t.status == TunnelStatus::Connected).count() as i32,
            cloudflare_total: tunnels.len() as i32,
        })
    }
    
    async fn get_activity_logs(&self) -> Result<Vec<ActivityLog>, ApiError> {
        // 1. 从持久化存储读取活动日志
        // 2. 按时间倒序排序
        // 3. 取前 50 条记录
        // 4. 返回 ActivityLog 列表
        
        let mut logs = self.activity_store.get_recent_activities(50).await?;
        logs.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(logs)
    }
}
```

### 活动日志存储接口

```rust
/// 活动日志存储特性
#[async_trait]
pub trait ActivityStore {
    /// 记录新活动
    async fn log_activity(&self, activity: ActivityLog) -> Result<(), ApiError>;
    
    /// 获取最近的活动记录
    async fn get_recent_activities(&self, limit: usize) -> Result<Vec<ActivityLog>, ApiError>;
}

/// 简单的内存活动日志存储（用于 MVP）
pub struct InMemoryActivityStore {
    logs: Arc<Mutex<VecDeque<ActivityLog>>>,
    max_size: usize,
}

impl InMemoryActivityStore {
    pub fn new(max_size: usize) -> Self {
        Self {
            logs: Arc::new(Mutex::new(VecDeque::with_capacity(max_size))),
            max_size,
        }
    }
}

#[async_trait]
impl ActivityStore for InMemoryActivityStore {
    async fn log_activity(&self, activity: ActivityLog) -> Result<(), ApiError> {
        let mut logs = self.logs.lock().await;
        
        // 如果超过最大容量，移除最旧的记录
        if logs.len() >= self.max_size {
            logs.pop_front();
        }
        
        logs.push_back(activity);
        Ok(())
    }
    
    async fn get_recent_activities(&self, limit: usize) -> Result<Vec<ActivityLog>, ApiError> {
        let logs = self.logs.lock().await;
        Ok(logs.iter()
            .rev()
            .take(limit)
            .cloned()
            .collect())
    }
}
```

---

## 活动日志集成

### 各模块触发活动日志的时机

每个功能模块在执行关键操作后，**应当** 通过 `ActivityStore` 记录活动日志。

#### Systemd 模块活动
```rust
// 启动服务时
activity_store.log_activity(ActivityLog {
    id: uuid::Uuid::new_v4().to_string(),
    timestamp: chrono::Utc::now().to_rfc3339(),
    activity_type: ActivityType::Systemd,
    action: "start".to_string(),
    description: format!("Started service: {}", service_name),
}).await?;

// 停止服务时
activity_store.log_activity(ActivityLog {
    id: uuid::Uuid::new_v4().to_string(),
    timestamp: chrono::Utc::now().to_rfc3339(),
    activity_type: ActivityType::Systemd,
    action: "stop".to_string(),
    description: format!("Stopped service: {}", service_name),
}).await?;

// 重启服务时
activity_store.log_activity(ActivityLog {
    id: uuid::Uuid::new_v4().to_string(),
    timestamp: chrono::Utc::now().to_rfc3339(),
    activity_type: ActivityType::Systemd,
    action: "restart".to_string(),
    description: format!("Restarted service: {}", service_name),
}).await?;
```

#### Crontab 模块活动
```rust
// 创建任务时
activity_store.log_activity(ActivityLog {
    id: uuid::Uuid::new_v4().to_string(),
    timestamp: chrono::Utc::now().to_rfc3339(),
    activity_type: ActivityType::Crontab,
    action: "create".to_string(),
    description: format!("Created crontab task: {}", task_name),
}).await?;

// 删除任务时
activity_store.log_activity(ActivityLog {
    id: uuid::Uuid::new_v4().to_string(),
    timestamp: chrono::Utc::now().to_rfc3339(),
    activity_type: ActivityType::Crontab,
    action: "delete".to_string(),
    description: format!("Deleted crontab task: {}", task_name),
}).await?;
```

#### Nginx 模块活动
```rust
// 创建代理时
activity_store.log_activity(ActivityLog {
    id: uuid::Uuid::new_v4().to_string(),
    timestamp: chrono::Utc::now().to_rfc3339(),
    activity_type: ActivityType::Nginx,
    action: "create".to_string(),
    description: format!("Created proxy: {} -> {}", proxy_path, target),
}).await?;
```

#### Cloudflare 模块活动
```rust
// 隧道连接时
activity_store.log_activity(ActivityLog {
    id: uuid::Uuid::new_v4().to_string(),
    timestamp: chrono::Utc::now().to_rfc3339(),
    activity_type: ActivityType::Cloudflare,
    action: "connect".to_string(),
    description: format!("Tunnel {} connected", tunnel_name),
}).await?;
```

#### Config 模块活动
```rust
// 提交配置时
activity_store.log_activity(ActivityLog {
    id: uuid::Uuid::new_v4().to_string(),
    timestamp: chrono::Utc::now().to_rfc3339(),
    activity_type: ActivityType::Config,
    action: "commit".to_string(),
    description: format!("Committed config: {}", commit_message),
}).await?;

// 回滚配置时
activity_store.log_activity(ActivityLog {
    id: uuid::Uuid::new_v4().to_string(),
    timestamp: chrono::Utc::now().to_rfc3339(),
    activity_type: ActivityType::Config,
    action: "rollback".to_string(),
    description: format!("Rolled back to commit: {}", commit_hash),
}).await?;
```

---

## 错误码定义

```rust
#[derive(Debug, Serialize)]
#[serde(tag = "error", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DashboardError {
    /// 内部错误（聚合查询失败）
    InternalError { message: String },
}
```

---

## 实施检查清单

### Phase 1: 基础仪表板功能
- [ ] 实现 `DashboardFeature` trait
- [ ] 实现统计数据聚合（并发查询各模块）
- [ ] 实现活动日志存储（内存存储）
- [ ] 实现活动日志查询（按时间倒序，限制 50 条）

### Phase 2: 活动日志集成
- [ ] 在 Systemd 模块集成活动日志记录
- [ ] 在 Crontab 模块集成活动日志记录
- [ ] 在 Nginx 模块集成活动日志记录
- [ ] 在 Cloudflare 模块集成活动日志记录
- [ ] 在 TTY 模块集成活动日志记录
- [ ] 在 Config 模块集成活动日志记录

### Phase 3: 性能优化
- [ ] 优化统计查询性能（使用缓存或预聚合）
- [ ] 优化活动日志存储（考虑持久化到文件或数据库）

### Phase 4: 测试
- [ ] 单元测试：统计数据聚合逻辑
- [ ] 单元测试：活动日志存储和查询
- [ ] 集成测试：各模块活动日志集成
- [ ] 性能测试：统计查询响应时间

---

## 相关文档

- [API 设计规范](./20-api-design.md)
- [Systemd 服务管理功能 (F01)](./21-feature-systemd.md)
- [Crontab 任务管理功能 (F02)](./22-feature-crontab.md)
- [Nginx 代理管理功能 (F04)](./24-feature-nginx.md)
- [Cloudflare 隧道管理功能 (F05)](./25-feature-tunnel.md)
- [TTY 会话管理功能 (F07)](./26-feature-tty.md)
- [配置文件管理功能 (F06)](./27-feature-config.md)
- [前端 UI 设计](./30-frontend-ui.md)
