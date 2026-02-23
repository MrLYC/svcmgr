use svcmgr::web::{ApiError, HttpConfig, HttpServer};

/// 测试 HTTP 服务器基础功能
#[tokio::test]
async fn test_http_server_startup_and_health_check() {
    // 使用随机端口避免冲突
    let config = HttpConfig {
        bind: "127.0.0.1".to_string(),
        port: 0, // 0 表示由操作系统分配随机端口
    };

    // 启动服务器（在后台任务中）
    let server = HttpServer::new(config);

    // 注意：axum 的 serve 在绑定时会占用端口，所以我们需要手动获取监听器地址
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // 在后台启动服务器
    tokio::spawn(async move {
        axum::serve(listener, server.router).await.unwrap();
    });

    // 等待服务器启动
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // 测试健康检查端点
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{}/health", addr))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");
    assert!(body["timestamp"].is_string());
}

/// 测试 404 错误返回统一 JSON 格式
#[tokio::test]
async fn test_404_error_format() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let config = HttpConfig {
        bind: "127.0.0.1".to_string(),
        port: addr.port(),
    };

    let server = HttpServer::new(config);

    tokio::spawn(async move {
        axum::serve(listener, server.router).await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // 请求不存在的路由
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{}/nonexistent", addr))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 404);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["error"].is_object());
    assert_eq!(body["error"]["code"], "RESOURCE_NOT_FOUND");
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("/nonexistent")
    );
}

/// 测试 API 错误类型转换为正确的 HTTP 状态码
#[test]
fn test_api_error_status_codes() {
    use axum::response::IntoResponse;

    // 404 错误
    let error = ApiError::not_found("Service");
    let response = error.into_response();
    assert_eq!(response.status(), 404);

    // 400 错误
    let error = ApiError::bad_request("Invalid input");
    let response = error.into_response();
    assert_eq!(response.status(), 400);

    // 500 错误
    let error = ApiError::internal_error("Something went wrong");
    let response = error.into_response();
    assert_eq!(response.status(), 500);
}

/// 测试错误响应 JSON 结构
#[test]
fn test_error_response_serialization() {
    let error = ApiError::new("TEST_ERROR", "Test message")
        .with_details(serde_json::json!({"field": "value"}))
        .with_request_id("req_12345");

    let json = serde_json::to_value(&error).unwrap();

    assert_eq!(json["code"], "TEST_ERROR");
    assert_eq!(json["message"], "Test message");
    assert_eq!(json["details"]["field"], "value");
    assert_eq!(json["request_id"], "req_12345");
}
