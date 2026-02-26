//! Phase 3.3 反向代理集成测试
//!
//! 测试内容:
//! - 基于路径的路由
//! - 基于主机名的路由
//! - 路由优先级(host+path > host > path)
//! - 健康检查(不健康后端返回 503)
//! - 前缀去除功能
//! - 动态后端注册/注销
//! - X-Forwarded-* 头部转发

use axum::{body::Body, extract::Request, http::StatusCode, routing::any, Router};
use serde_json::json;
use std::net::SocketAddr;
use svcmgr::{config::models::RouteConfig, web::proxy::ProxyService};
use tokio::task::JoinHandle;

/// 清除系统 HTTP 代理环境变量，避免拦截本地测试请求
fn clear_http_proxy() {
    unsafe {
        std::env::remove_var("HTTP_PROXY");
        std::env::remove_var("HTTPS_PROXY");
        std::env::remove_var("http_proxy");
        std::env::remove_var("https_proxy");
    }
}

/// 启动一个简单的 mock 后端服务器
/// 返回 JSON: {"backend": "name:port", "path": "/...", "headers": {...}}
async fn spawn_mock_backend(name: &str, port: u16) -> JoinHandle<()> {
    let name = name.to_string();
    eprintln!(
        "[TEST] Starting mock backend '{}' on 127.0.0.1:{}",
        name, port
    );
    let name_for_log = name.clone();

    tokio::spawn(async move {
        let app = Router::new().route(
            "/*path",
            any(|req: Request| async move {
                let path = req.uri().path().to_string();
                let query = req.uri().query().map(|q| q.to_string());

                // 提取所有请求头
                let mut headers_map = serde_json::Map::new();
                for (key, value) in req.headers() {
                    if let Ok(v) = value.to_str() {
                        headers_map.insert(key.as_str().to_string(), json!(v));
                    }
                }

                let response = json!({
                    "backend": name.clone(),
                    "path": path,
                    "query": query,
                    "headers": headers_map,
                });

                axum::Json(response)
            }),
        );

        let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        eprintln!("[TEST] Mock backend '{}' READY on {}", name_for_log, addr);
        axum::serve(listener, app).await.unwrap();
    })
}

/// 辅助函数: 发送请求到代理并返回状态码和响应体
async fn make_proxy_request(
    proxy: &ProxyService,
    host: Option<&str>,
    path: &str,
) -> (StatusCode, String) {
    let mut builder = hyper::Request::builder().method("GET").uri(path);

    if let Some(h) = host {
        builder = builder.header("host", h);
    }

    let req = builder.body(Body::empty()).unwrap();
    let response = proxy.handle_request(req).await;

    let status = response.status();
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body_bytes.to_vec()).unwrap_or_default();

    (status, body)
}

// ============================================================================
// 测试 1: 基于路径的路由
// ============================================================================

#[tokio::test]
async fn test_path_based_routing() {
    // 启动两个 mock 后端
    let _backend1 = spawn_mock_backend("api-backend", 40001).await;
    let _backend2 = spawn_mock_backend("web-backend", 40002).await;

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // 配置路由
    let routes = vec![
        RouteConfig {
            name: "api-route".to_string(),
            host: None,
            path: Some("/api/*".to_string()),
            backend: Some("api:http".to_string()),
            serve_dir: None,
            index: None,
            strip_prefix: false,
            auth: None,
            websocket: false,
        },
        RouteConfig {
            name: "web-route".to_string(),
            host: None,
            path: Some("/web/*".to_string()),
            backend: Some("web:http".to_string()),
            serve_dir: None,
            index: None,
            strip_prefix: false,
            auth: None,
            websocket: false,
        },
    ];

    let proxy = ProxyService::new(routes);

    // 注册后端
    proxy
        .register_backend("api", "http", "127.0.0.1:40001".parse().unwrap())
        .await;
    proxy
        .register_backend("web", "http", "127.0.0.1:40002".parse().unwrap())
        .await;

    // 测试 /api/* 路由
    let (status, body) = make_proxy_request(&proxy, None, "/api/users").await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["backend"], "api-backend");
    assert_eq!(json["path"], "/api/users");

    // 测试 /web/* 路由
    let (status, body) = make_proxy_request(&proxy, None, "/web/index.html").await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["backend"], "web-backend");
    assert_eq!(json["path"], "/web/index.html");

    // 测试不匹配的路径 -> 404
    let (status, _body) = make_proxy_request(&proxy, None, "/unknown").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// 测试 2: 基于主机名的路由
// ============================================================================

#[tokio::test]
async fn test_host_based_routing() {
    let _backend1 = spawn_mock_backend("api-host", 40003).await;
    let _backend2 = spawn_mock_backend("admin-host", 40004).await;

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let routes = vec![
        RouteConfig {
            name: "api-host-route".to_string(),
            host: Some("api.example.com".to_string()),
            path: None,
            backend: Some("api:http".to_string()),
            serve_dir: None,
            index: None,
            strip_prefix: false,
            auth: None,
            websocket: false,
        },
        RouteConfig {
            name: "admin-host-route".to_string(),
            host: Some("admin.example.com".to_string()),
            path: None,
            backend: Some("admin:http".to_string()),
            serve_dir: None,
            index: None,
            strip_prefix: false,
            auth: None,
            websocket: false,
        },
    ];

    let proxy = ProxyService::new(routes);

    proxy
        .register_backend("api", "http", "127.0.0.1:40003".parse().unwrap())
        .await;
    proxy
        .register_backend("admin", "http", "127.0.0.1:40004".parse().unwrap())
        .await;

    // 测试 api.example.com
    let (status, body) = make_proxy_request(&proxy, Some("api.example.com"), "/any/path").await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["backend"], "api-host");

    // 测试 admin.example.com
    let (status, body) = make_proxy_request(&proxy, Some("admin.example.com"), "/dashboard").await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["backend"], "admin-host");

    // 测试不匹配的主机名 -> 404
    let (status, _body) = make_proxy_request(&proxy, Some("unknown.com"), "/").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ============================================================================
// 测试 3: 路由优先级 (host+path > host > path)
// ============================================================================

#[tokio::test]
async fn test_route_priority() {
    let _backend_specific = spawn_mock_backend("specific", 40005).await;
    let _backend_host = spawn_mock_backend("host-only", 40006).await;
    let _backend_path = spawn_mock_backend("path-only", 40007).await;

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let routes = vec![
        // 优先级 1: host + path
        RouteConfig {
            name: "host-path-route".to_string(),
            host: Some("api.example.com".to_string()),
            path: Some("/v1/*".to_string()),
            backend: Some("specific:http".to_string()),
            serve_dir: None,
            index: None,
            strip_prefix: false,
            auth: None,
            websocket: false,
        },
        // 优先级 2: host only
        RouteConfig {
            name: "host-only-route".to_string(),
            host: Some("api.example.com".to_string()),
            path: None,
            backend: Some("host:http".to_string()),
            serve_dir: None,
            index: None,
            strip_prefix: false,
            auth: None,
            websocket: false,
        },
        // 优先级 3: path only
        RouteConfig {
            name: "path-only-route".to_string(),
            host: None,
            path: Some("/v1/*".to_string()),
            backend: Some("path:http".to_string()),
            serve_dir: None,
            index: None,
            strip_prefix: false,
            auth: None,
            websocket: false,
        },
    ];

    let proxy = ProxyService::new(routes);

    proxy
        .register_backend("specific", "http", "127.0.0.1:40005".parse().unwrap())
        .await;
    proxy
        .register_backend("host", "http", "127.0.0.1:40006".parse().unwrap())
        .await;
    proxy
        .register_backend("path", "http", "127.0.0.1:40007".parse().unwrap())
        .await;

    // Case 1: host+path 匹配 -> 应该路由到 specific
    let (status, body) = make_proxy_request(&proxy, Some("api.example.com"), "/v1/users").await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        json["backend"], "specific",
        "host+path should have highest priority"
    );

    // Case 2: 只有 host 匹配 (path 不匹配 /v1/*) -> 应该路由到 host-only
    let (status, body) = make_proxy_request(&proxy, Some("api.example.com"), "/v2/users").await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        json["backend"], "host-only",
        "host-only should be second priority"
    );

    // Case 3: 只有 path 匹配 (没有 host 头) -> 应该路由到 path-only
    let (status, body) = make_proxy_request(&proxy, None, "/v1/users").await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        json["backend"], "path-only",
        "path-only should be third priority"
    );
}

// ============================================================================
// 测试 4: 最长前缀匹配 (path-only 路由)
// ============================================================================

#[tokio::test]
async fn test_longest_prefix_matching() {
    let _backend_api = spawn_mock_backend("api-backend", 40008).await;
    let _backend_users = spawn_mock_backend("users-backend", 40009).await;

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let routes = vec![
        // 更具体的路径
        RouteConfig {
            name: "users-route".to_string(),
            host: None,
            path: Some("/api/users/*".to_string()),
            backend: Some("users:http".to_string()),
            serve_dir: None,
            index: None,
            strip_prefix: false,
            auth: None,
            websocket: false,
        },
        // 更通用的路径
        RouteConfig {
            name: "api-route".to_string(),
            host: None,
            path: Some("/api/*".to_string()),
            backend: Some("api:http".to_string()),
            serve_dir: None,
            index: None,
            strip_prefix: false,
            auth: None,
            websocket: false,
        },
    ];

    let proxy = ProxyService::new(routes);

    proxy
        .register_backend("api", "http", "127.0.0.1:40008".parse().unwrap())
        .await;
    proxy
        .register_backend("users", "http", "127.0.0.1:40009".parse().unwrap())
        .await;

    // /api/users/* 应该匹配更具体的路由
    let (status, body) = make_proxy_request(&proxy, None, "/api/users/123").await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        json["backend"], "users-backend",
        "Should match longest prefix"
    );

    // /api/products 应该匹配通用路由
    let (status, body) = make_proxy_request(&proxy, None, "/api/products").await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        json["backend"], "api-backend",
        "Should match shorter prefix"
    );
}

// ============================================================================
// 测试 5: 后端健康检查
// ============================================================================

#[tokio::test]
async fn test_backend_health_check() {
    let _backend = spawn_mock_backend("health-test", 40010).await;

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let routes = vec![RouteConfig {
        name: "test-route".to_string(),
        host: None,
        path: Some("/api/*".to_string()),
        backend: Some("test:http".to_string()),
        serve_dir: None,
        index: None,
        strip_prefix: false,
        auth: None,
        websocket: false,
    }];

    let proxy = ProxyService::new(routes);

    // 注册健康后端
    proxy
        .register_backend("test", "http", "127.0.0.1:40010".parse().unwrap())
        .await;

    // 正常请求应该成功
    let (status, _body) = make_proxy_request(&proxy, None, "/api/test").await;
    assert_eq!(status, StatusCode::OK);

    // 标记后端为不健康
    proxy.update_backend_health("test", "http", false).await;

    // 不健康后端应该返回 503
    let (status, body) = make_proxy_request(&proxy, None, "/api/test").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(
        body.contains("unhealthy"),
        "Should indicate backend is unhealthy"
    );

    // 恢复健康状态
    proxy.update_backend_health("test", "http", true).await;

    // 应该恢复正常
    let (status, _body) = make_proxy_request(&proxy, None, "/api/test").await;
    assert_eq!(status, StatusCode::OK);
}

// ============================================================================
// 调试测试: 验证 mock backend 是否正常工作
// ============================================================================

#[tokio::test]
async fn test_mock_backend_direct() {
    let _backend = spawn_mock_backend("debug-backend", 40099).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // 直接用 reqwest 测试(不经过 ProxyService)
    let client = reqwest::Client::builder().no_proxy().build().unwrap();

    let resp = client
        .get("http://127.0.0.1:40099/test")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    println!("Direct mock backend response: {}", body);

    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["path"], "/test");
    assert_eq!(json["backend"], "debug-backend");
}

#[tokio::test]
async fn test_hyper_client_direct() {
    // 测试 hyper-util Client 是否能绕过系统代理连接本地端口
    let _backend = spawn_mock_backend("hyper-test", 40098).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // 使用与 ProxyService 相同的 hyper-util Client
    use hyper_util::{client::legacy::Client, rt::TokioExecutor};
    let client = Client::builder(TokioExecutor::new()).build_http();

    // 构造请求
    let req = hyper::Request::builder()
        .method("GET")
        .uri("http://127.0.0.1:40098/test")
        .body(Body::empty())
        .unwrap();

    // 发送请求
    let resp = client.request(req).await.unwrap();

    println!("Hyper client status: {:?}", resp.status());

    use http_body_util::BodyExt;
    let body_bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body = String::from_utf8(body_bytes.to_vec()).unwrap();
    println!("Hyper client body: {}", body);

    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["backend"], "hyper-test");
}

// ============================================================================
// 测试 6: 前缀去除 (strip_prefix)
// ============================================================================

#[tokio::test]
async fn test_strip_prefix() {
    // 修复: 禁用系统 HTTP 代理,避免拦截本地测试请求
    clear_http_proxy();

    let _backend = spawn_mock_backend("strip-backend", 40018).await;

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // 健康检查: 确认 mock backend 已启动
    let health_check = reqwest::Client::builder()
        .no_proxy()
        .build()
        .unwrap()
        .get("http://127.0.0.1:40018/health")
        .timeout(tokio::time::Duration::from_secs(2))
        .send()
        .await;

    if let Err(e) = &health_check {
        eprintln!("[ERROR] Mock backend on 40018 not responding: {:?}", e);
    }
    assert!(
        health_check.is_ok(),
        "Mock backend on 40018 not responding after 500ms"
    );
    eprintln!("[TEST] Mock backend health check passed");

    // 测试 strip_prefix = true
    let routes_strip = vec![RouteConfig {
        name: "api-strip".to_string(),
        host: None,
        path: Some("/api/*".to_string()),
        backend: Some("api:http".to_string()),
        serve_dir: None,
        index: None,
        strip_prefix: true, // 去除 /api 前缀
        auth: None,
        websocket: false,
    }];

    let proxy_strip = ProxyService::new(routes_strip);
    proxy_strip
        .register_backend("api", "http", "127.0.0.1:40018".parse().unwrap())
        .await;

    // /api/users 应该转发为 /users
    let (status, body) = make_proxy_request(&proxy_strip, None, "/api/users").await;
    println!("strip_prefix=true: status={:?}, body={:?}", status, body);
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["path"], "/users", "Prefix should be stripped");

    // 测试 strip_prefix = false
    let routes_no_strip = vec![RouteConfig {
        name: "api-no-strip".to_string(),
        host: None,
        path: Some("/api/*".to_string()),
        backend: Some("api:http".to_string()),
        serve_dir: None,
        index: None,
        strip_prefix: false, // 保留 /api 前缀
        auth: None,
        websocket: false,
    }];

    let proxy_no_strip = ProxyService::new(routes_no_strip);
    proxy_no_strip
        .register_backend("api", "http", "127.0.0.1:40018".parse().unwrap())
        .await;

    // /api/users 应该保持原样
    let (status, body) = make_proxy_request(&proxy_no_strip, None, "/api/users").await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["path"], "/api/users", "Prefix should be preserved");
}

// ============================================================================
// 测试 7: 查询字符串保留
// ============================================================================

#[tokio::test]
async fn test_query_string_preservation() {
    let _backend = spawn_mock_backend("query-backend", 40012).await;

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let routes = vec![RouteConfig {
        name: "query-route".to_string(),
        host: None,
        path: Some("/api/*".to_string()),
        backend: Some("api:http".to_string()),
        serve_dir: None,
        index: None,
        strip_prefix: false,
        auth: None,
        websocket: false,
    }];

    let proxy = ProxyService::new(routes);
    proxy
        .register_backend("api", "http", "127.0.0.1:40012".parse().unwrap())
        .await;

    // 测试查询字符串
    let (status, body) = make_proxy_request(&proxy, None, "/api/search?q=rust&limit=10").await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["path"], "/api/search");
    assert_eq!(
        json["query"], "q=rust&limit=10",
        "Query string should be preserved"
    );
}

// ============================================================================
// 测试 8: X-Forwarded-* 头部
// ============================================================================

#[tokio::test]
async fn test_forwarded_headers() {
    let _backend = spawn_mock_backend("headers-backend", 40013).await;

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let routes = vec![RouteConfig {
        name: "headers-route".to_string(),
        host: None,
        path: Some("/api/*".to_string()),
        backend: Some("api:http".to_string()),
        serve_dir: None,
        index: None,
        strip_prefix: false,
        auth: None,
        websocket: false,
    }];

    let proxy = ProxyService::new(routes);
    proxy
        .register_backend("api", "http", "127.0.0.1:40013".parse().unwrap())
        .await;

    // 发送请求
    let (status, body) = make_proxy_request(&proxy, Some("example.com"), "/api/test").await;
    assert_eq!(status, StatusCode::OK);

    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    let headers = json["headers"].as_object().unwrap();

    // 验证 X-Forwarded-Host
    assert_eq!(
        headers.get("x-forwarded-host").unwrap().as_str().unwrap(),
        "example.com",
        "X-Forwarded-Host should be set"
    );

    // 验证 X-Forwarded-Proto
    assert_eq!(
        headers.get("x-forwarded-proto").unwrap().as_str().unwrap(),
        "http",
        "X-Forwarded-Proto should be set"
    );
}

// ============================================================================
// 测试 9: 动态后端注册和注销
// ============================================================================

#[tokio::test]
async fn test_dynamic_backend_registration() {
    let _backend1 = spawn_mock_backend("dynamic-1", 40014).await;
    let _backend2 = spawn_mock_backend("dynamic-2", 40015).await;

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let routes = vec![RouteConfig {
        name: "dynamic-route".to_string(),
        host: None,
        path: Some("/api/*".to_string()),
        backend: Some("api:http".to_string()),
        serve_dir: None,
        index: None,
        strip_prefix: false,
        auth: None,
        websocket: false,
    }];

    let proxy = ProxyService::new(routes);

    // 初始状态: 没有后端 -> 404
    let (status, _body) = make_proxy_request(&proxy, None, "/api/test").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "No backend registered yet");

    // 注册第一个后端
    proxy
        .register_backend("api", "http", "127.0.0.1:40014".parse().unwrap())
        .await;

    let (status, body) = make_proxy_request(&proxy, None, "/api/test").await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["backend"], "dynamic-1");

    // 注销第一个后端
    proxy.unregister_backend("api", "http").await;

    let (status, _body) = make_proxy_request(&proxy, None, "/api/test").await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "Backend should be unregistered"
    );

    // 注册第二个后端
    proxy
        .register_backend("api", "http", "127.0.0.1:40015".parse().unwrap())
        .await;

    let (status, body) = make_proxy_request(&proxy, None, "/api/test").await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["backend"], "dynamic-2");
}

// ============================================================================
// 测试 10: 动态路由表更新
// ============================================================================

#[tokio::test]
async fn test_dynamic_route_updates() {
    let _backend1 = spawn_mock_backend("route-update-1", 40016).await;
    let _backend2 = spawn_mock_backend("route-update-2", 40017).await;

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // 初始路由: /v1/*
    let initial_routes = vec![RouteConfig {
        name: "v1-route".to_string(),
        host: None,
        path: Some("/v1/*".to_string()),
        backend: Some("api:http".to_string()),
        serve_dir: None,
        index: None,
        strip_prefix: false,
        auth: None,
        websocket: false,
    }];

    let proxy = ProxyService::new(initial_routes);
    proxy
        .register_backend("api", "http", "127.0.0.1:40016".parse().unwrap())
        .await;
    proxy
        .register_backend("web", "http", "127.0.0.1:40017".parse().unwrap())
        .await;

    // /v1/* 应该有效
    let (status, body) = make_proxy_request(&proxy, None, "/v1/test").await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["backend"], "route-update-1");

    // /v2/* 应该 404
    let (status, _body) = make_proxy_request(&proxy, None, "/v2/test").await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // 更新路由表: 替换为 /v2/*
    let new_routes = vec![RouteConfig {
        name: "v2-route".to_string(),
        host: None,
        path: Some("/v2/*".to_string()),
        backend: Some("web:http".to_string()),
        serve_dir: None,
        index: None,
        strip_prefix: false,
        auth: None,
        websocket: false,
    }];

    proxy.update_routes(new_routes).await;

    // /v1/* 现在应该 404
    let (status, _body) = make_proxy_request(&proxy, None, "/v1/test").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "Old route should be removed");

    // /v2/* 现在应该有效
    let (status, body) = make_proxy_request(&proxy, None, "/v2/test").await;
    assert_eq!(status, StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        json["backend"], "route-update-2",
        "New route should be active"
    );
}

// ============================================================================
// 测试 11: 错误处理 - 后端连接失败
// ============================================================================

#[tokio::test]
async fn test_backend_connection_failure() {
    // 不启动任何后端服务器

    let routes = vec![RouteConfig {
        name: "fail-route".to_string(),
        host: None,
        path: Some("/api/*".to_string()),
        backend: Some("api:http".to_string()),
        serve_dir: None,
        index: None,
        strip_prefix: false,
        auth: None,
        websocket: false,
    }];

    let proxy = ProxyService::new(routes);

    // 注册一个不存在的后端地址
    proxy
        .register_backend("api", "http", "127.0.0.1:19999".parse().unwrap())
        .await;

    // 应该返回 502 Bad Gateway
    let (status, body) = make_proxy_request(&proxy, None, "/api/test").await;
    assert_eq!(
        status,
        StatusCode::BAD_GATEWAY,
        "Should return 502 when backend is unreachable"
    );
    assert!(
        body.contains("backend") || body.contains("connect"),
        "Should indicate connection failure"
    );
}
