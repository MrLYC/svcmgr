// 任务管理 API 集成测试
//
// 测试覆盖范围:
// 1. 即时任务 API (5 个端点的完整流程)
// 2. 定时任务 API (8 个端点的完整流程)
// 3. 错误处理和边界情况

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
// 即时任务 API 测试 (5 个端点)
// =============================================================================

#[tokio::test]
async fn test_list_tasks_empty() {
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
async fn test_get_task_not_found() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/api/v1/tasks/nonexistent", base_url))
        .send()
        .await
        .unwrap();

    // MockMiseAdapter 返回 404
    assert_eq!(resp.status(), 404);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("error").is_some());
}

#[tokio::test]
async fn test_run_task_success() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let payload = serde_json::json!({
        "args": ["arg1", "arg2"]
    });

    let resp = client
        .post(format!("{}/api/v1/tasks/test_task/run", base_url))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // MockMiseAdapter 返回成功
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("data").is_some());
    assert!(body["data"]["execution_id"].is_string());
}

#[tokio::test]
async fn test_cancel_task_not_implemented() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .delete(format!("{}/api/v1/tasks/test_task/cancel", base_url))
        .send()
        .await
        .unwrap();

    // cancel_task 是 MVP 实现,返回成功但不执行实际取消
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_get_task_history_empty() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/api/v1/tasks/test_task/history", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("data").is_some());
    assert!(body["data"].is_array());
}

// =============================================================================
// 定时任务 API 测试 (8 个端点)
// =============================================================================

#[tokio::test]
async fn test_list_scheduled_tasks_empty() {
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
async fn test_create_scheduled_task_success() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let payload = serde_json::json!({
        "name": "backup_task",
        "execution": {
            "type": "command",
            "command": "tar -czf backup.tar.gz /data",
            "dir": "/tmp"
        },
        "schedule": "0 2 * * *",
        "enabled": true,
        "description": "Daily backup",
        "timeout": 3600
    });

    let resp = client
        .post(format!("{}/api/v1/scheduled-tasks", base_url))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // 成功创建
    assert_eq!(resp.status(), 201);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("data").is_some());
    assert_eq!(body["data"]["name"], "backup_task");
}

#[tokio::test]
async fn test_create_scheduled_task_invalid_cron() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let payload = serde_json::json!({
        "name": "invalid_task",
        "execution": {
            "type": "command",
            "command": "echo test",
            "dir": "/tmp"
        },
        "schedule": "invalid cron",
        "enabled": true,
        "timeout": 3600
    });

    let resp = client
        .post(format!("{}/api/v1/scheduled-tasks", base_url))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // 验证失败
    assert_eq!(resp.status(), 400);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("error").is_some());
}

#[tokio::test]
async fn test_get_scheduled_task_success() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    // 先创建一个任务
    let payload = serde_json::json!({
        "name": "test_task",
        "execution": {
            "type": "command",
            "command": "echo hello",
            "dir": "/tmp"
        },
        "schedule": "0 * * * *",
        "enabled": true,
        "timeout": 3600
    });

    client
        .post(format!("{}/api/v1/scheduled-tasks", base_url))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // 获取任务
    let resp = client
        .get(format!("{}/api/v1/scheduled-tasks/test_task", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("data").is_some());
    assert_eq!(body["data"]["name"], "test_task");
}

#[tokio::test]
async fn test_update_scheduled_task_success() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    // 先创建一个任务
    let create_payload = serde_json::json!({
        "name": "update_test",
        "execution": {
            "type": "command",
            "command": "echo old",
            "dir": "/tmp"
        },
        "schedule": "0 * * * *",
        "enabled": true,
        "timeout": 3600
    });

    client
        .post(format!("{}/api/v1/scheduled-tasks", base_url))
        .json(&create_payload)
        .send()
        .await
        .unwrap();

    // 更新任务
    let update_payload = serde_json::json!({
        "schedule": "0 2 * * *",
        "enabled": false,
        "timeout": 3600
    });

    let resp = client
        .put(format!("{}/api/v1/scheduled-tasks/update_test", base_url))
        .json(&update_payload)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("data").is_some());
    assert_eq!(body["data"]["enabled"], false);
}

#[tokio::test]
async fn test_delete_scheduled_task_success() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    // 先创建一个任务
    let payload = serde_json::json!({
        "name": "delete_test",
        "execution": {
            "type": "command",
            "command": "echo hello",
            "dir": "/tmp"
        },
        "schedule": "0 * * * *",
        "enabled": true,
        "timeout": 3600
    });

    client
        .post(format!("{}/api/v1/scheduled-tasks", base_url))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // 删除任务
    let resp = client
        .delete(format!("{}/api/v1/scheduled-tasks/delete_test", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 204);

    // 验证任务已删除
    let get_resp = client
        .get(format!("{}/api/v1/scheduled-tasks/delete_test", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(get_resp.status(), 404);
}

#[tokio::test]
async fn test_enable_scheduled_task_success() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    // 先创建一个禁用的任务
    let payload = serde_json::json!({
        "name": "enable_test",
        "execution": {
            "type": "command",
            "command": "echo hello",
            "dir": "/tmp"
        },
        "schedule": "0 * * * *",
        "enabled": false,
        "timeout": 3600
    });

    client
        .post(format!("{}/api/v1/scheduled-tasks", base_url))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // 启用任务
    let resp = client
        .post(format!(
            "{}/api/v1/scheduled-tasks/enable_test/enable",
            base_url
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("data").is_some());
    assert_eq!(body["data"]["enabled"], true);
}

#[tokio::test]
async fn test_disable_scheduled_task_success() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    // 先创建一个启用的任务
    let payload = serde_json::json!({
        "name": "disable_test",
        "execution": {
            "type": "command",
            "command": "echo hello",
            "dir": "/tmp"
        },
        "schedule": "0 * * * *",
        "enabled": true,
        "timeout": 3600
    });

    client
        .post(format!("{}/api/v1/scheduled-tasks", base_url))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // 禁用任务
    let resp = client
        .post(format!(
            "{}/api/v1/scheduled-tasks/disable_test/disable",
            base_url
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("data").is_some());
    assert_eq!(body["data"]["enabled"], false);
}

#[tokio::test]
async fn test_run_scheduled_task_now() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    // 先创建一个任务
    let payload = serde_json::json!({
        "name": "run_test",
        "execution": {
            "type": "command",
            "command": "echo hello",
            "dir": "/tmp"
        },
        "schedule": "0 * * * *",
        "enabled": true,
        "timeout": 3600
    });

    client
        .post(format!("{}/api/v1/scheduled-tasks", base_url))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // 立即运行任务
    let resp = client
        .post(format!("{}/api/v1/scheduled-tasks/run_test/run", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("data").is_some());
    assert!(body["data"]["execution_id"].is_string());
}
