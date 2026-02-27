//! 反向代理核心模块
//!
//! 实现动态路由匹配、HTTP 请求转发、WebSocket 支持
//! 与调度引擎集成,实现服务启停时的路由表自动更新

use crate::config::models::RouteConfig;
use axum::{
    body::Body,
    extract::Request,
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
};
use http_body_util::BodyExt;
use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Instant};
use tokio::sync::RwLock;

/// 反向代理服务
#[derive(Clone)]
pub struct ProxyService {
    /// 路由配置(动态更新)
    routes: Arc<RwLock<Vec<RouteConfig>>>,
    /// 后端服务注册表
    backends: Arc<RwLock<BackendRegistry>>,
    /// HTTP 客户端(用于转发请求)
    client: reqwest::Client,
}

/// 后端服务注册表
#[derive(Default)]
struct BackendRegistry {
    /// "service_name:port_name" -> Backend
    services: HashMap<String, Backend>,
}

/// 后端服务信息
#[allow(dead_code)]
#[derive(Clone)]
struct Backend {
    /// 服务名
    name: String,
    /// 端口名
    port_name: String,
    /// 实际监听地址
    address: SocketAddr,
    /// 健康状态
    healthy: bool,
    /// 最后健康检查时间
    last_check: Instant,
}

/// 路由匹配结果
struct RouteMatch<'a> {
    route: &'a RouteConfig,
    backend: Backend,
}

impl ProxyService {
    /// 创建新的代理服务
    pub fn new(routes: Vec<RouteConfig>) -> Self {
        // 使用 reqwest Client 并禁用系统代理
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            routes: Arc::new(RwLock::new(routes)),
            backends: Arc::new(RwLock::new(BackendRegistry::default())),
            client,
        }
    }

    /// 更新路由表(替换全部路由)
    pub async fn update_routes(&self, routes: Vec<RouteConfig>) {
        let mut route_table = self.routes.write().await;
        *route_table = routes;
    }

    /// 注册后端服务(服务启动时调用)
    pub async fn register_backend(&self, service: &str, port: &str, addr: SocketAddr) {
        let mut registry = self.backends.write().await;
        let key = format!("{}:{}", service, port);

        registry.services.insert(
            key,
            Backend {
                name: service.to_string(),
                port_name: port.to_string(),
                address: addr,
                healthy: true,
                last_check: Instant::now(),
            },
        );
    }

    /// 注销后端服务(服务停止时调用)
    pub async fn unregister_backend(&self, service: &str, port: &str) {
        let mut registry = self.backends.write().await;
        let key = format!("{}:{}", service, port);
        registry.services.remove(&key);
    }

    /// 更新后端健康状态
    pub async fn update_backend_health(&self, service: &str, port: &str, healthy: bool) {
        let mut registry = self.backends.write().await;
        let key = format!("{}:{}", service, port);

        if let Some(backend) = registry.services.get_mut(&key) {
            backend.healthy = healthy;
            backend.last_check = Instant::now();
        }
    }

    /// 匹配路由
    ///
    /// 优先级: host+path > host > path (path 使用最长前缀匹配)
    async fn match_route<'a>(
        &self,
        host: Option<&str>,
        path: &str,
        routes: &'a [RouteConfig],
    ) -> Option<RouteMatch<'a>> {
        let registry = self.backends.read().await;

        // 1. 尝试匹配 host + path
        if let Some(host) = host {
            for route in routes {
                if let (Some(route_host), Some(route_path)) = (&route.host, &route.path) {
                    if route_host == host && Self::path_matches(path, route_path) {
                        if let Some(backend_ref) = &route.backend {
                            if let Some(backend) = registry.services.get(backend_ref) {
                                return Some(RouteMatch {
                                    route,
                                    backend: backend.clone(),
                                });
                            }
                        }
                    }
                }
            }
        }

        // 2. 尝试匹配 host only
        if let Some(host) = host {
            for route in routes {
                if route.host.as_deref() == Some(host) && route.path.is_none() {
                    if let Some(backend_ref) = &route.backend {
                        if let Some(backend) = registry.services.get(backend_ref) {
                            return Some(RouteMatch {
                                route,
                                backend: backend.clone(),
                            });
                        }
                    }
                }
            }
        }

        // 3. 尝试匹配 path only (最长前缀匹配)
        let mut best_match: Option<(usize, &RouteConfig, Backend)> = None;

        for route in routes {
            if route.host.is_none() {
                if let Some(route_path) = &route.path {
                    if Self::path_matches(path, route_path) {
                        if let Some(backend_ref) = &route.backend {
                            if let Some(backend) = registry.services.get(backend_ref) {
                                let match_len = route_path.trim_end_matches('*').len();
                                if best_match
                                    .as_ref()
                                    .is_none_or(|(len, _, _)| match_len > *len)
                                {
                                    best_match = Some((match_len, route, backend.clone()));
                                }
                            }
                        }
                    }
                }
            }
        }

        best_match.map(|(_, route, backend)| RouteMatch { route, backend })
    }

    /// 检查路径是否匹配路由模式
    fn path_matches(path: &str, pattern: &str) -> bool {
        if pattern.ends_with('*') {
            // 通配符匹配: "/api/*" 匹配 "/api/users"
            let prefix = pattern.trim_end_matches('*');
            path.starts_with(prefix)
        } else {
            // 精确匹配
            path == pattern
        }
    }

    /// 处理代理请求(主入口)
    pub async fn handle_request(&self, req: Request) -> Response {
        // 1. 提取 Host 头和路径(克隆为 owned 值,避免借用冲突)
        let host = req
            .headers()
            .get("host")
            .and_then(|h| h.to_str().ok())
            .map(|h| h.split(':').next().unwrap_or(h).to_string()); // 克隆为 String
        let path = req.uri().path().to_string();
        let method = req.method().clone();

        // 2. 匹配路由
        let routes = self.routes.read().await;
        let route_match = self.match_route(host.as_deref(), &path, &routes).await;

        let route_match = match route_match {
            Some(m) => m,
            None => {
                return (StatusCode::NOT_FOUND, "No matching route").into_response();
            }
        };

        // 3. 检查后端健康状态
        if !route_match.backend.healthy {
            return (StatusCode::SERVICE_UNAVAILABLE, "Backend service unhealthy").into_response();
        }

        // 4. 构建后端 URL
        let backend_url = self.build_backend_url(
            &route_match.backend,
            req.uri(),
            route_match.route.strip_prefix,
            route_match.route.path.as_deref(),
        );

        // 5. 构建 reqwest 请求
        let mut reqwest_req = self.client.request(method, &backend_url);

        // 添加 X-Forwarded-* 头
        if let Some(original_host) = host.clone() {
            reqwest_req = reqwest_req.header("x-forwarded-host", original_host);
        }
        reqwest_req = reqwest_req.header("x-forwarded-proto", "http");

        // 复制原始请求的其他头 (除了 Host, 它会自动设置)
        for (name, value) in req.headers() {
            if name != "host" {
                if let Ok(value_str) = value.to_str() {
                    reqwest_req = reqwest_req.header(name.as_str(), value_str);
                }
            }
        }

        // 转换 body (axum Body -> reqwest Body)

        let (_parts, body) = req.into_parts();
        let body_bytes = match body.collect().await {
            Ok(collected) => collected.to_bytes(),
            Err(_) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to read request body",
                )
                    .into_response();
            }
        };

        reqwest_req = reqwest_req.body(body_bytes.to_vec());

        // 6. 发送请求到后端
        match reqwest_req.send().await {
            Ok(backend_resp) => {
                // 转换 reqwest::Response -> axum::Response
                let status = backend_resp.status();
                let mut response_builder = axum::http::Response::builder().status(status);

                // 复制响应头
                for (name, value) in backend_resp.headers() {
                    response_builder = response_builder.header(name, value);
                }

                // 转换 body
                let body_bytes = match backend_resp.bytes().await {
                    Ok(bytes) => bytes,
                    Err(_) => {
                        return (StatusCode::BAD_GATEWAY, "Failed to read backend response")
                            .into_response();
                    }
                };

                response_builder
                    .body(Body::from(body_bytes))
                    .unwrap_or_else(|_| {
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Failed to build response",
                        )
                            .into_response()
                    })
            }
            Err(err) => {
                eprintln!("[ERROR] Backend request failed: {:?}", err);
                (StatusCode::BAD_GATEWAY, "Failed to connect to backend").into_response()
            }
        }
    }

    /// 构建后端 URL
    fn build_backend_url(
        &self,
        backend: &Backend,
        uri: &Uri,
        strip_prefix: bool,
        route_path: Option<&str>,
    ) -> String {
        let mut path = uri.path().to_string();

        // 如果需要去除前缀
        if strip_prefix {
            if let Some(prefix) = route_path {
                let prefix = prefix.trim_end_matches('*');
                if path.starts_with(prefix) {
                    path = path[prefix.len()..].to_string();
                    // 确保路径始终以 / 开头
                    if !path.starts_with('/') {
                        path = format!("/{}", path);
                    }
                    if path.is_empty() {
                        path = "/".to_string();
                    }
                }
            }
        }

        // 保留查询字符串
        let query = uri.query().map(|q| format!("?{}", q)).unwrap_or_default();

        format!("http://{}{}{}", backend.address, path, query)
    }
}

/// Axum 路由处理器
pub async fn proxy_handler(_req: Request) -> Response {
    // 从请求扩展中提取 ProxyService (需要在路由层注入)
    // 这里先返回未实现错误,实际使用时需要通过 State 传入
    (StatusCode::NOT_IMPLEMENTED, "Proxy handler not configured").into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_matching() {
        // 精确匹配
        assert!(ProxyService::path_matches("/api", "/api"));
        assert!(!ProxyService::path_matches("/api/users", "/api"));

        // 通配符匹配
        assert!(ProxyService::path_matches("/api/users", "/api/*"));
        assert!(ProxyService::path_matches("/api/", "/api/*"));
        assert!(ProxyService::path_matches("/api", "/api*"));
        assert!(!ProxyService::path_matches("/users", "/api/*"));
    }

    #[tokio::test]
    async fn test_backend_registration() {
        let proxy = ProxyService::new(vec![]);
        let addr: SocketAddr = "127.0.0.1:3000".parse().unwrap();

        // 注册后端
        proxy.register_backend("api", "http", addr).await;

        // 验证注册
        let registry = proxy.backends.read().await;
        assert!(registry.services.contains_key("api:http"));
        let backend = registry.services.get("api:http").unwrap();
        assert_eq!(backend.name, "api");
        assert_eq!(backend.port_name, "http");
        assert_eq!(backend.address, addr);
        assert!(backend.healthy);
    }

    #[tokio::test]
    async fn test_route_updates() {
        use crate::config::models::RouteConfig;

        let proxy = ProxyService::new(vec![]);

        // 更新路由表
        let new_routes = vec![RouteConfig {
            name: "api".to_string(),
            host: None,
            path: Some("/api/*".to_string()),
            backend: Some("api:http".to_string()),
            serve_dir: None,
            index: None,
            strip_prefix: false,
            auth: None,
            websocket: false,
        }];

        proxy.update_routes(new_routes.clone()).await;

        // 验证更新
        let routes = proxy.routes.read().await;
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].name, "api");
    }
}
