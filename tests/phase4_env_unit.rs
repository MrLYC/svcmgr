//! Phase 4.4: 环境变量管理 - 单元测试
//!
//! 测试核心功能模块:
//! - parse_env_file: .env 文件解析
//! - parse_scope: 作用域字符串解析
//! - scope_priority: 优先级计算
//! - get_source_file: 配置文件路径映射
//! - VariableExpander: 变量展开、循环检测、缓存
//! - MockMiseAdapter ConfigPort 方法

use std::collections::HashMap;
use std::path::PathBuf;
use svcmgr::adapters::mock::MockMiseAdapter;
use svcmgr::env::{EnvScope, VariableExpander};
use svcmgr::mocks::mise::MiseMock;
use svcmgr::mocks::mise::TaskDef;
use svcmgr::ports::mise_port::ConfigPort;
use svcmgr::ports::mise_port::MiseVersion;
use svcmgr::web::api::env_models::{parse_scope, scope_priority};

// ============================================================================
// parse_env_file 测试
// ============================================================================

/// 辅助函数：解析 .env 文件内容（从 env_handlers.rs 复制）
fn parse_env_file(content: &str) -> Result<Vec<(String, String)>, String> {
    let mut vars = Vec::new();

    for (line_num, line) in content.lines().enumerate() {
        let line = line.trim();

        // 跳过空行和注释
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // 解析 KEY=VALUE 格式
        let parts: Vec<_> = line.splitn(2, '=').collect();
        if parts.len() != 2 {
            return Err(format!("Invalid .env format at line {}", line_num + 1));
        }

        let key = parts[0].trim().to_string();
        let value = parts[1]
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_string();

        vars.push((key, value));
    }

    Ok(vars)
}

#[test]
fn test_parse_env_file_valid() {
    let content = r#"
# Comment line
KEY1=value1
KEY2="value with spaces"
KEY3='single quoted'

# Another comment
KEY4=value4
"#;

    let result = parse_env_file(content).unwrap();
    assert_eq!(result.len(), 4);
    assert_eq!(result[0], ("KEY1".to_string(), "value1".to_string()));
    assert_eq!(
        result[1],
        ("KEY2".to_string(), "value with spaces".to_string())
    );
    assert_eq!(result[2], ("KEY3".to_string(), "single quoted".to_string()));
    assert_eq!(result[3], ("KEY4".to_string(), "value4".to_string()));
}

#[test]
fn test_parse_env_file_empty() {
    let content = r#"
# Only comments
# No variables
"#;

    let result = parse_env_file(content).unwrap();
    assert_eq!(result.len(), 0);
}

#[test]
fn test_parse_env_file_invalid_format() {
    let content = "INVALID_LINE_WITHOUT_EQUALS";

    let result = parse_env_file(content);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .contains("Invalid .env format at line 1"));
}

#[test]
fn test_parse_env_file_with_equals_in_value() {
    let content = "KEY=value=with=equals";

    let result = parse_env_file(content).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(
        result[0],
        ("KEY".to_string(), "value=with=equals".to_string())
    );
}

// ============================================================================
// parse_scope 测试
// ============================================================================

#[test]
fn test_parse_scope_global() {
    let scope = parse_scope("global").unwrap();
    assert!(matches!(scope, EnvScope::Global));
}

#[test]
fn test_parse_scope_service() {
    let scope = parse_scope("service:nginx").unwrap();
    match scope {
        EnvScope::Service { name } => assert_eq!(name, "nginx"),
        _ => panic!("Expected Service scope"),
    }
}

#[test]
fn test_parse_scope_task() {
    let scope = parse_scope("task:deploy").unwrap();
    match scope {
        EnvScope::Task { name } => assert_eq!(name, "deploy"),
        _ => panic!("Expected Task scope"),
    }
}

#[test]
fn test_parse_scope_invalid() {
    assert!(parse_scope("invalid").is_err());
    assert!(parse_scope("service:").is_err());
    assert!(parse_scope("task:").is_err());
    assert!(parse_scope("").is_err());
}

// ============================================================================
// scope_priority 测试
// ============================================================================

#[test]
fn test_scope_priority_values() {
    let global = EnvScope::Global;
    let service = EnvScope::Service {
        name: "test".to_string(),
    };
    let task = EnvScope::Task {
        name: "test".to_string(),
    };

    assert_eq!(scope_priority(&global), 1);
    assert_eq!(scope_priority(&service), 2);
    assert_eq!(scope_priority(&task), 3);
}

#[test]
fn test_scope_priority_ordering() {
    let global = EnvScope::Global;
    let service = EnvScope::Service {
        name: "test".to_string(),
    };
    let task = EnvScope::Task {
        name: "test".to_string(),
    };

    // Task > Service > Global
    assert!(scope_priority(&task) > scope_priority(&service));
    assert!(scope_priority(&service) > scope_priority(&global));
}

// ============================================================================
// get_source_file 测试
// ============================================================================

/// 辅助函数：获取配置文件路径（从 env_handlers.rs 复制）
fn get_source_file(scope: &EnvScope) -> String {
    match scope {
        EnvScope::Global | EnvScope::Task { .. } => "~/.config/mise/config.toml".to_string(),
        EnvScope::Service { .. } => "~/.config/mise/svcmgr/config.toml".to_string(),
    }
}

#[test]
fn test_get_source_file_global() {
    let scope = EnvScope::Global;
    let path = get_source_file(&scope);
    assert_eq!(path, "~/.config/mise/config.toml");
}

#[test]
fn test_get_source_file_service() {
    let scope = EnvScope::Service {
        name: "nginx".to_string(),
    };
    let path = get_source_file(&scope);
    assert_eq!(path, "~/.config/mise/svcmgr/config.toml");
}

#[test]
fn test_get_source_file_task() {
    let scope = EnvScope::Task {
        name: "deploy".to_string(),
    };
    let path = get_source_file(&scope);
    assert_eq!(path, "~/.config/mise/config.toml");
}

// ============================================================================
// VariableExpander 测试
// ============================================================================

#[tokio::test]
async fn test_variable_expander_simple() {
    let adapter = MockMiseAdapter::new(
        MiseMock::new(PathBuf::from(".")),
        MiseVersion::new(2026, 2, 17),
    );

    // 设置全局环境变量
    let mut global_env = HashMap::new();
    global_env.insert("BASE_URL".to_string(), "https://example.com".to_string());
    global_env.insert("API_PATH".to_string(), "/api/v1".to_string());
    adapter.mock().lock().unwrap().env = global_env;

    let mut expander = VariableExpander::new(&adapter).await.unwrap();

    // 测试简单展开
    let result = expander
        .expand("${BASE_URL}${API_PATH}", &EnvScope::Global)
        .await
        .unwrap();
    assert_eq!(result, "https://example.com/api/v1");
}

#[tokio::test]
async fn test_variable_expander_no_references() {
    let adapter = MockMiseAdapter::new(
        MiseMock::new(PathBuf::from(".")),
        MiseVersion::new(2026, 2, 17),
    );
    let mut expander = VariableExpander::new(&adapter).await.unwrap();

    // 没有引用的字符串应该保持不变
    let result = expander
        .expand("plain text", &EnvScope::Global)
        .await
        .unwrap();
    assert_eq!(result, "plain text");
}

#[tokio::test]
async fn test_variable_expander_undefined_variable() {
    let adapter = MockMiseAdapter::new(
        MiseMock::new(PathBuf::from(".")),
        MiseVersion::new(2026, 2, 17),
    );
    let mut expander = VariableExpander::new(&adapter).await.unwrap();

    // 未定义的变量应该保持原样
    let result = expander
        .expand("${UNDEFINED_VAR}", &EnvScope::Global)
        .await
        .unwrap();
    assert_eq!(result, "${UNDEFINED_VAR}");
}

#[tokio::test]
async fn test_variable_expander_circular_detection() {
    let adapter = MockMiseAdapter::new(
        MiseMock::new(PathBuf::from(".")),
        MiseVersion::new(2026, 2, 17),
    );

    // 设置循环引用: A -> B -> C -> A
    let mut global_env = HashMap::new();
    global_env.insert("VAR_A".to_string(), "${VAR_B}".to_string());
    global_env.insert("VAR_B".to_string(), "${VAR_C}".to_string());
    global_env.insert("VAR_C".to_string(), "${VAR_A}".to_string());
    adapter.mock().lock().unwrap().env = global_env;

    let mut expander = VariableExpander::new(&adapter).await.unwrap();

    // 应该检测到循环引用并返回错误
    let result = expander.expand("${VAR_A}", &EnvScope::Global).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Circular"));
}

#[tokio::test]
async fn test_variable_expander_scope_priority() {
    let adapter = MockMiseAdapter::new(
        MiseMock::new(PathBuf::from(".")),
        MiseVersion::new(2026, 2, 17),
    );

    // 设置不同作用域的同名变量
    let mut global_env = HashMap::new();
    global_env.insert("ENV".to_string(), "global".to_string());
    adapter.mock().lock().unwrap().env = global_env;

    let mut service_envs = HashMap::new();
    let mut nginx_env = HashMap::new();
    nginx_env.insert("ENV".to_string(), "service".to_string());
    service_envs.insert("nginx".to_string(), nginx_env);
    adapter.mock().lock().unwrap().service_envs = service_envs;

    let mut tasks = HashMap::new();
    let mut deploy_env = HashMap::new();
    deploy_env.insert("ENV".to_string(), "task".to_string());
    tasks.insert(
        "deploy".to_string(),
        TaskDef {
            run: "".to_string(),
            env: deploy_env.clone(),
            depends: vec![],
            description: None,
        },
    );
    adapter.mock().lock().unwrap().tasks = tasks;

    let mut expander = VariableExpander::new(&adapter).await.unwrap();

    // Global scope - 应该使用全局值
    let result = expander.expand("${ENV}", &EnvScope::Global).await.unwrap();
    assert_eq!(result, "global");

    // Service scope - 应该使用服务值（优先级更高）
    let service_scope = EnvScope::Service {
        name: "nginx".to_string(),
    };
    let result = expander.expand("${ENV}", &service_scope).await.unwrap();
    assert_eq!(result, "service");

    // Task scope - 应该使用任务值（最高优先级）
    let task_scope = EnvScope::Task {
        name: "deploy".to_string(),
    };
    let result = expander.expand("${ENV}", &task_scope).await.unwrap();
    assert_eq!(result, "task");
}

// ============================================================================
// MockMiseAdapter ConfigPort 测试
// ============================================================================

#[tokio::test]
async fn test_mock_adapter_get_global_env() {
    let adapter = MockMiseAdapter::new(
        MiseMock::new(PathBuf::from(".")),
        MiseVersion::new(2026, 2, 17),
    );

    let mut global_env = HashMap::new();
    global_env.insert("KEY1".to_string(), "value1".to_string());
    global_env.insert("KEY2".to_string(), "value2".to_string());
    adapter.mock().lock().unwrap().env = global_env.clone();

    let result = adapter.get_global_env().await.unwrap();
    assert_eq!(result, global_env);
}

#[tokio::test]
async fn test_mock_adapter_get_service_envs() {
    let adapter = MockMiseAdapter::new(
        MiseMock::new(PathBuf::from(".")),
        MiseVersion::new(2026, 2, 17),
    );

    let mut service_envs = HashMap::new();
    let mut nginx_env = HashMap::new();
    nginx_env.insert("PORT".to_string(), "80".to_string());
    service_envs.insert("nginx".to_string(), nginx_env.clone());
    adapter.mock().lock().unwrap().service_envs = service_envs.clone();

    let result = adapter.get_service_envs().await.unwrap();
    assert_eq!(result, service_envs);
}

#[tokio::test]
async fn test_mock_adapter_get_task_envs() {
    let adapter = MockMiseAdapter::new(
        MiseMock::new(PathBuf::from(".")),
        MiseVersion::new(2026, 2, 17),
    );

    let mut tasks = HashMap::new();
    let mut deploy_env = HashMap::new();
    deploy_env.insert("STAGE".to_string(), "production".to_string());
    tasks.insert(
        "deploy".to_string(),
        TaskDef {
            run: "".to_string(),
            env: deploy_env.clone(),
            depends: vec![],
            description: None,
        },
    );
    adapter.mock().lock().unwrap().tasks = tasks;

    // get_task_envs should extract env from TaskDef
    let expected_task_envs: HashMap<String, HashMap<String, String>> =
        vec![("deploy".to_string(), deploy_env.clone())]
            .into_iter()
            .collect();
    let result = adapter.get_task_envs().await.unwrap();
    assert_eq!(result, expected_task_envs);
}

#[tokio::test]
async fn test_mock_adapter_set_env_var_global() {
    let adapter = MockMiseAdapter::new(
        MiseMock::new(PathBuf::from(".")),
        MiseVersion::new(2026, 2, 17),
    );

    let scope = EnvScope::Global;
    adapter
        .set_env_var("NEW_KEY", "new_value", &scope)
        .await
        .unwrap();

    let global_env = adapter.get_global_env().await.unwrap();
    assert_eq!(global_env.get("NEW_KEY"), Some(&"new_value".to_string()));
}

#[tokio::test]
async fn test_mock_adapter_set_env_var_service() {
    let adapter = MockMiseAdapter::new(
        MiseMock::new(PathBuf::from(".")),
        MiseVersion::new(2026, 2, 17),
    );

    let scope = EnvScope::Service {
        name: "redis".to_string(),
    };
    adapter.set_env_var("PORT", "6379", &scope).await.unwrap();

    let service_envs = adapter.get_service_envs().await.unwrap();
    let redis_env = service_envs.get("redis").unwrap();
    assert_eq!(redis_env.get("PORT"), Some(&"6379".to_string()));
}

#[tokio::test]
async fn test_mock_adapter_set_env_var_task() {
    let adapter = MockMiseAdapter::new(
        MiseMock::new(PathBuf::from(".")),
        MiseVersion::new(2026, 2, 17),
    );

    let scope = EnvScope::Task {
        name: "build".to_string(),
    };
    adapter
        .set_env_var("TARGET", "release", &scope)
        .await
        .unwrap();

    let task_envs = adapter.get_task_envs().await.unwrap();
    let build_env = task_envs.get("build").unwrap();
    assert_eq!(build_env.get("TARGET"), Some(&"release".to_string()));
}

#[tokio::test]
async fn test_mock_adapter_delete_env_var_global() {
    let adapter = MockMiseAdapter::new(
        MiseMock::new(PathBuf::from(".")),
        MiseVersion::new(2026, 2, 17),
    );

    // 先设置一个变量
    let mut global_env = HashMap::new();
    global_env.insert("TO_DELETE".to_string(), "value".to_string());
    adapter.mock().lock().unwrap().env = global_env;

    // 删除变量
    let scope = EnvScope::Global;
    adapter.delete_env_var("TO_DELETE", &scope).await.unwrap();

    let global_env = adapter.get_global_env().await.unwrap();
    assert!(!global_env.contains_key("TO_DELETE"));
}

#[tokio::test]
async fn test_mock_adapter_delete_env_var_service() {
    let adapter = MockMiseAdapter::new(
        MiseMock::new(PathBuf::from(".")),
        MiseVersion::new(2026, 2, 17),
    );

    // 先设置服务变量
    let mut service_envs = HashMap::new();
    let mut postgres_env = HashMap::new();
    postgres_env.insert("DB_NAME".to_string(), "mydb".to_string());
    service_envs.insert("postgres".to_string(), postgres_env);
    adapter.mock().lock().unwrap().service_envs = service_envs;

    // 删除变量
    let scope = EnvScope::Service {
        name: "postgres".to_string(),
    };
    adapter.delete_env_var("DB_NAME", &scope).await.unwrap();

    let service_envs = adapter.get_service_envs().await.unwrap();
    let postgres_env = service_envs.get("postgres").unwrap();
    assert!(!postgres_env.contains_key("DB_NAME"));
}

#[tokio::test]
async fn test_mock_adapter_delete_nonexistent_var() {
    let adapter = MockMiseAdapter::new(
        MiseMock::new(PathBuf::from(".")),
        MiseVersion::new(2026, 2, 17),
    );

    // 删除不存在的变量应该成功（幂等性）
    let scope = EnvScope::Global;
    let result = adapter.delete_env_var("NONEXISTENT", &scope).await;
    assert!(result.is_ok());
}
