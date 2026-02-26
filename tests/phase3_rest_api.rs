// Phase 3.2 REST API 集成测试
//
// 验证目标：
// 1. 所有 API 端点路由正确（34 个端点）
// 2. 响应格式符合规范（统一 ApiResponse/ErrorResponse）
// 3. 错误处理正确（404, 500, 400 等）
// 4. HTTP 状态码映射正确

use svcmgr::web::server::{HttpConfig, HttpServer};
use tokio::net::TcpListener;

/// 辅助函数：创建测试服务器并返回基础 URL
async fn spawn_test_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);

    let config = HttpConfig {
        bind: addr.ip().to_string(),
        port: addr.port(),
    };

    let server = HttpServer::new(config);
    let router = server.router;

    // 在后台启动服务器
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    // 等待服务器ready
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    base_url
}

// =============================================================================
// 服务管理 API 测试 (11 个端点)
// =============================================================================

#[tokio::test]
async fn test_services_list() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/api/v1/services", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("data").is_some());
    assert!(body["data"].is_array());
}

#[tokio::test]
async fn test_services_create_not_implemented() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let payload = serde_json::json!({
        "name": "test-service",
        "command": "node server.js",
        "autostart": false
    });

    let resp = client
        .post(format!("{}/api/v1/services", base_url))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // 当前实现返回 500 + NOT_IMPLEMENTED
    assert_eq!(resp.status(), 500);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["code"], "NOT_IMPLEMENTED");
}

#[tokio::test]
async fn test_services_get_not_found() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/api/v1/services/nonexistent", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 500); // Skeleton阶段: 无数据库,无法判断资源是否存在

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["code"], "NOT_IMPLEMENTED"); // Skeleton阶段返回 NOT_IMPLEMENTED
}

#[tokio::test]
async fn test_services_start_not_implemented() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/api/v1/services/test/start", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 500);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["code"], "NOT_IMPLEMENTED");
}

// =============================================================================
// 任务管理 API 测试 (13 个端点)
// =============================================================================

#[tokio::test]
async fn test_tasks_list() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/api/v1/tasks", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("data").is_some());
    assert!(body["data"].is_array());
}

#[tokio::test]
async fn test_tasks_get_not_found() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/api/v1/tasks/nonexistent", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 500); // Skeleton阶段: 无数据库,无法判断资源是否存在

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["code"], "NOT_IMPLEMENTED"); // Skeleton阶段返回 NOT_IMPLEMENTED
}

#[tokio::test]
async fn test_scheduled_tasks_list() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/api/v1/scheduled-tasks", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("data").is_some());
    assert!(body["data"].is_array());
}

#[tokio::test]
async fn test_scheduled_tasks_create_not_implemented() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let payload = serde_json::json!({
        "name": "backup",
        "command": "backup.sh",
        "schedule": "0 2 * * *",
        "enabled": true
    });

    let resp = client
        .post(format!("{}/api/v1/scheduled-tasks", base_url))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 500);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["code"], "NOT_IMPLEMENTED");
}

// =============================================================================
// 配置管理 API 测试 (10 个端点)
// =============================================================================

#[tokio::test]
/// Test: Config GET endpoint returns full config
/// Expected: 200 OK with complete config structure
async fn test_config_get_success() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/api/v1/config", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["data"].is_object());
    // Config should have expected sections
    assert!(body["data"]["tools"].is_object());
    assert!(body["data"]["env"].is_object());
}

#[tokio::test]
async fn test_config_section_invalid() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/api/v1/config/nonexistent", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400); // Invalid section name

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["code"], "INVALID_INPUT"); // Invalid section error
}

#[tokio::test]
async fn test_config_history() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/api/v1/config/history", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("data").is_some());
    assert!(body["data"].is_array());
}

// =============================================================================
// 统一响应格式测试
// =============================================================================

#[tokio::test]
async fn test_success_response_format() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/api/v1/services", base_url))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = resp.json().await.unwrap();

    // 验证成功响应格式
    assert!(
        body.get("data").is_some(),
        "Success response must have 'data' field"
    );
    assert!(
        body.get("error").is_none(),
        "Success response should not have 'error' field"
    );
}

#[tokio::test]
async fn test_error_response_format() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/api/v1/services/nonexistent", base_url))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = resp.json().await.unwrap();

    // 验证错误响应格式
    assert!(
        body.get("error").is_some(),
        "Error response must have 'error' field"
    );
    assert!(
        body["error"].get("code").is_some(),
        "Error must have 'code' field"
    );
    assert!(
        body["error"].get("message").is_some(),
        "Error must have 'message' field"
    );
    assert!(
        body.get("data").is_none(),
        "Error response should not have 'data' field"
    );
}

// =============================================================================
// 路由覆盖率测试（抽样验证）
// =============================================================================

#[tokio::test]
async fn test_all_service_endpoints_routable() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    // 测试所有服务管理端点是否可路由（不要求功能实现）
    let endpoints = vec![
        ("GET", "/api/v1/services"),
        ("POST", "/api/v1/services"),
        ("GET", "/api/v1/services/test"),
        ("PUT", "/api/v1/services/test"),
        ("DELETE", "/api/v1/services/test"),
        ("POST", "/api/v1/services/test/start"),
        ("POST", "/api/v1/services/test/stop"),
        ("POST", "/api/v1/services/test/restart"),
        ("GET", "/api/v1/services/test/logs"),
        ("GET", "/api/v1/services/test/health"),
        ("GET", "/api/v1/services/test/status"),
    ];

    for (method, path) in endpoints {
        let url = format!("{}{}", base_url, path);
        let resp = match method {
            "GET" => client.get(&url).send().await.unwrap(),
            "POST" => client
                .post(&url)
                .json(&serde_json::json!({}))
                .send()
                .await
                .unwrap(),
            "PUT" => client
                .put(&url)
                .json(&serde_json::json!({}))
                .send()
                .await
                .unwrap(),
            "DELETE" => client.delete(&url).send().await.unwrap(),
            _ => panic!("Unknown method: {}", method),
        };

        // 只要不是 404，就说明路由存在（可能返回 200, 500, 等）
        assert_ne!(
            resp.status(),
            404,
            "{} {} should be routable (got 404)",
            method,
            path
        );
    }
}

#[tokio::test]
async fn test_all_task_endpoints_routable() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let endpoints = vec![
        ("GET", "/api/v1/tasks"),
        ("GET", "/api/v1/tasks/test"),
        ("POST", "/api/v1/tasks/test/run"),
        ("POST", "/api/v1/tasks/test/cancel"),
        ("GET", "/api/v1/tasks/test/history"),
        ("GET", "/api/v1/scheduled-tasks"),
        ("POST", "/api/v1/scheduled-tasks"),
        ("GET", "/api/v1/scheduled-tasks/test"),
        ("PUT", "/api/v1/scheduled-tasks/test"),
        ("DELETE", "/api/v1/scheduled-tasks/test"),
        ("POST", "/api/v1/scheduled-tasks/test/enable"),
        ("POST", "/api/v1/scheduled-tasks/test/disable"),
        ("POST", "/api/v1/scheduled-tasks/test/run"),
    ];

    for (method, path) in endpoints {
        let url = format!("{}{}", base_url, path);
        let resp = match method {
            "GET" => client.get(&url).send().await.unwrap(),
            "POST" => client
                .post(&url)
                .json(&serde_json::json!({}))
                .send()
                .await
                .unwrap(),
            "PUT" => client
                .put(&url)
                .json(&serde_json::json!({}))
                .send()
                .await
                .unwrap(),
            "DELETE" => client.delete(&url).send().await.unwrap(),
            _ => panic!("Unknown method: {}", method),
        };

        assert_ne!(
            resp.status(),
            404,
            "{} {} should be routable (got 404)",
            method,
            path
        );
    }
}

#[tokio::test]
async fn test_all_config_endpoints_routable() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let endpoints = vec![
        ("GET", "/api/v1/config"),
        ("PUT", "/api/v1/config"),
        ("GET", "/api/v1/config/mise"),
        ("PATCH", "/api/v1/config/mise"),
        ("POST", "/api/v1/config/validate"),
        ("GET", "/api/v1/config/history"),
        ("POST", "/api/v1/config/rollback"),
        ("GET", "/api/v1/config/diff?from=HEAD~1&to=HEAD"),
        ("GET", "/api/v1/config/export"),
        ("POST", "/api/v1/config/import"),
    ];

    for (method, path) in endpoints {
        let url = format!("{}{}", base_url, path);
        let resp = match method {
            "GET" => client.get(&url).send().await.unwrap(),
            "POST" => client
                .post(&url)
                .json(&serde_json::json!({}))
                .send()
                .await
                .unwrap(),
            "PUT" => client
                .put(&url)
                .json(&serde_json::json!({}))
                .send()
                .await
                .unwrap(),
            "PATCH" => client
                .patch(&url)
                .json(&serde_json::json!({}))
                .send()
                .await
                .unwrap(),
            _ => panic!("Unknown method: {}", method),
        };

        assert_ne!(
            resp.status(),
            404,
            "{} {} should be routable (got 404)",
            method,
            path
        );
    }
}
