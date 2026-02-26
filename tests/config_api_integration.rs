// 配置管理 API 集成测试
//
// 测试覆盖范围:
// 1. 配置读取 API (2 个端点)
// 2. 配置更新 API (2 个端点)
// 3. 配置验证 API (1 个端点)
// 4. 配置历史 API (3 个端点)
// 5. 配置导入/导出 API (2 个端点)

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
// 配置读取 API 测试
// =============================================================================

#[tokio::test]
async fn test_get_config_success() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/api/v1/config", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("data").is_some());
    assert!(body["data"]["tools"].is_object());
    assert!(body["data"]["env"].is_object());
    assert!(body["data"]["features"].is_object());
}

#[tokio::test]
async fn test_get_config_section_tools() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/api/v1/config/tools", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("data").is_some());
}

#[tokio::test]
async fn test_get_config_section_env() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/api/v1/config/env", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("data").is_some());
}

#[tokio::test]
async fn test_get_config_section_features() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/api/v1/config/features", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("data").is_some());
    assert!(body["data"]["systemd"].is_string());
}

#[tokio::test]
async fn test_get_config_section_invalid() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/api/v1/config/invalid_section", base_url))
        .send()
        .await
        .unwrap();

    // 无效的段落名称应该返回错误
    assert_eq!(resp.status(), 400);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("error").is_some());
}

// =============================================================================
// 配置更新 API 测试
// =============================================================================

#[tokio::test]
async fn test_update_config_success() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let payload = serde_json::json!({
        "tools": {
            "node": "20.0.0",
            "python": "3.11"
        },
        "env": {
            "NODE_ENV": "production"
        },
        "tasks": {},
        "services": {},
        "scheduled_tasks": {},
        "features": {
            "systemd": "auto",
            "cgroups": "auto",
            "http_proxy": "auto",
            "git_auto_commit": "enabled"
        },
        "http": null
    });

    let resp = client
        .put(format!("{}/api/v1/config", base_url))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("data").is_some());
    assert_eq!(body["data"]["tools"]["node"], "20.0.0");
}

#[tokio::test]
async fn test_patch_config_section_merge() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let payload = serde_json::json!({
        "op": "merge",
        "data": {
            "go": "1.21"
        }
    });

    let resp = client
        .patch(format!("{}/api/v1/config/tools", base_url))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("data").is_some());
    assert!(body["data"]["go"].is_string());
}

#[tokio::test]
async fn test_patch_config_section_replace() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let payload = serde_json::json!({
        "op": "replace",
        "data": {
            "NEW_VAR": "new_value"
        }
    });

    let resp = client
        .patch(format!("{}/api/v1/config/env", base_url))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("data").is_some());
    // replace 操作会清空原有值并替换为新值
    assert_eq!(body["data"]["NEW_VAR"], "new_value");
}

#[tokio::test]
async fn test_patch_config_section_remove() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    // 先添加一些工具
    let add_payload = serde_json::json!({
        "op": "merge",
        "data": {
            "node": "20.0.0",
            "python": "3.11"
        }
    });

    client
        .patch(format!("{}/api/v1/config/tools", base_url))
        .json(&add_payload)
        .send()
        .await
        .unwrap();

    // 移除 node
    let remove_payload = serde_json::json!({
        "op": "remove",
        "data": ["node"]
    });

    let resp = client
        .patch(format!("{}/api/v1/config/tools", base_url))
        .json(&remove_payload)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("data").is_some());
    // node 应该被移除
    assert!(body["data"].get("node").is_none() || body["data"]["node"].is_null());
}

// =============================================================================
// 配置验证 API 测试
// =============================================================================

#[tokio::test]
async fn test_validate_config_success() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let payload = serde_json::json!({
        "config": {
            "tools": {
                "node": "20.0.0"
            },
            "env": {},
            "tasks": {},
            "services": {},
            "scheduled_tasks": {},
            "features": {
                "systemd": "auto",
                "cgroups": "auto",
                "http_proxy": "auto",
                "git_auto_commit": "enabled"
            },
            "http": null
        }
    });

    let resp = client
        .post(format!("{}/api/v1/config/validate", base_url))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("data").is_some());
    assert!(body["data"]["valid"].is_boolean());
}

#[tokio::test]
async fn test_validate_config_with_errors() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let payload = serde_json::json!({
        "config": {
            "tools": {
                "node": ""  // 空版本应该触发验证错误
            },
            "env": {},
            "tasks": {},
            "services": {},
            "scheduled_tasks": {},
            "features": {
                "systemd": "auto",
                "cgroups": "auto",
                "http_proxy": "auto",
                "git_auto_commit": "enabled"
            },
            "http": null
        }
    });

    let resp = client
        .post(format!("{}/api/v1/config/validate", base_url))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("data").is_some());
    assert_eq!(body["data"]["valid"], false);
    assert!(body["data"]["errors"].is_array());
}

#[tokio::test]
async fn test_validate_config_port_conflict() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let payload = serde_json::json!({
        "config": {
            "tools": {},
            "env": {},
            "tasks": {},
            "services": {
                "api": {
                    "ports": {
                        "http": 8080
                    }
                },
                "web": {
                    "ports": {
                        "http": 8080  // 端口冲突
                    }
                }
            },
            "scheduled_tasks": {},
            "features": {
                "systemd": "auto",
                "cgroups": "auto",
                "http_proxy": "auto",
                "git_auto_commit": "enabled"
            },
            "http": null
        }
    });

    let resp = client
        .post(format!("{}/api/v1/config/validate", base_url))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("data").is_some());
    assert_eq!(body["data"]["valid"], false);
    assert!(body["data"]["errors"].is_array());
    // 应该有端口冲突错误
    let errors = body["data"]["errors"].as_array().unwrap();
    assert!(!errors.is_empty());
}

// =============================================================================
// 配置历史 API 测试
// =============================================================================

#[tokio::test]
async fn test_get_config_history() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!(
            "{}/api/v1/config/history?limit=10&offset=0",
            base_url
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("data").is_some());
    assert!(body["data"].is_array());
}

#[tokio::test]
async fn test_get_config_diff() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!(
            "{}/api/v1/config/diff?from=HEAD~1&to=HEAD",
            base_url
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("data").is_some());
    assert!(body["data"]["from"].is_string());
    assert!(body["data"]["to"].is_string());
}

#[tokio::test]
async fn test_rollback_config() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    // 这个测试需要先有提交记录,实际测试中可能需要先创建一些配置变更

    let payload = serde_json::json!({
        "commit": "HEAD~1",
        "message": "Rollback to previous version"
    });

    let resp = client
        .post(format!("{}/api/v1/config/rollback", base_url))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // 如果没有足够的提交历史,可能会失败,但应该返回适当的错误
    assert!(resp.status().is_success() || resp.status().is_client_error());
}

// =============================================================================
// 配置导入/导出 API 测试
// =============================================================================

#[tokio::test]
async fn test_export_config() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/api/v1/config/export", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("data").is_some());
    assert!(body["data"]["tools"].is_object());
    assert!(body["data"]["features"].is_object());
}

#[tokio::test]
async fn test_import_config_json() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let config_json = serde_json::json!({
        "tools": {
            "rust": "1.75"
        },
        "env": {},
        "tasks": {},
        "services": {},
        "scheduled_tasks": {},
        "features": {
            "systemd": "auto",
            "cgroups": "auto",
            "http_proxy": "auto",
            "git_auto_commit": "enabled"
        },
        "http": null
    });

    let payload = serde_json::json!({
        "config": serde_json::to_string(&config_json).unwrap(),
        "format": "json",
        "overwrite": false
    });

    let resp = client
        .post(format!("{}/api/v1/config/import", base_url))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("data").is_some());
}

#[tokio::test]
async fn test_import_config_overwrite() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let config_json = serde_json::json!({
        "tools": {
            "new-tool": "1.0"
        },
        "env": {},
        "tasks": {},
        "services": {},
        "scheduled_tasks": {},
        "features": {
            "systemd": "enabled",
            "cgroups": "disabled",
            "http_proxy": "auto",
            "git_auto_commit": "enabled"
        },
        "http": null
    });

    let payload = serde_json::json!({
        "config": serde_json::to_string(&config_json).unwrap(),
        "format": "json",
        "overwrite": true
    });

    let resp = client
        .post(format!("{}/api/v1/config/import", base_url))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("data").is_some());
    assert_eq!(body["data"]["tools"]["new-tool"], "1.0");
}

#[tokio::test]
async fn test_import_config_invalid_format() {
    let base_url = spawn_test_server().await;
    let client = reqwest::Client::new();

    let payload = serde_json::json!({
        "config": "invalid json {",
        "format": "json",
        "overwrite": false
    });

    let resp = client
        .post(format!("{}/api/v1/config/import", base_url))
        .json(&payload)
        .send()
        .await
        .unwrap();

    // 应该返回错误
    assert_eq!(resp.status(), 400);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("error").is_some());
}
