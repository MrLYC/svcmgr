// API 模块 - REST API 路由和处理器
//
// 此模块提供 svcmgr 的 REST API 实现,包括:
// - 服务管理 API (11个端点)
// - 任务管理 API (13个端点)
// - 配置管理 API (10个端点)

mod config;
mod services;
mod tasks;

use axum::Router;

/// 创建完整的 API 路由树
///
/// 返回 `/api/v1` 命名空间下的所有子路由
pub fn api_routes() -> Router {
    Router::new()
        .nest("/services", services::routes())
        .nest("/tasks", tasks::routes())
        .nest("/scheduled-tasks", tasks::scheduled_routes())
        .nest("/config", config::routes())
}
