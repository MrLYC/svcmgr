//! Service API Basic Tests
//!
//! 简化版测试 - 验证核心功能可用性

use std::path::PathBuf;
use svcmgr::adapters::mock::MockMiseAdapter;
use svcmgr::mocks::mise::MiseMock;
use svcmgr::ports::{MiseVersion, mise_port::ConfigPort};
use svcmgr::web::api::service_models::*;
use tempfile::TempDir;

// ============================================================================
// Data Model Tests (5 tests)
// ============================================================================

#[test]
fn test_service_definition_creation() {
    let def = ServiceDefinition {
        name: "test-service".to_string(),
        command: "echo hello".to_string(),
        working_dir: None,
        env: None,
        ports: None,
        health_check: None,
        resources: None,
        restart_policy: None,
        autostart: false,
        depends_on: None,
    };

    assert_eq!(def.name, "test-service");
    assert_eq!(def.command, "echo hello");
    assert!(!def.autostart);
}

#[test]
fn test_port_mapping_creation() {
    let port = PortMapping {
        host: 8080,
        container: 80,
        protocol: "tcp".to_string(),
    };

    assert_eq!(port.host, 8080);
    assert_eq!(port.container, 80);
    assert_eq!(port.protocol, "tcp");
}

#[test]
fn test_health_check_http() {
    let hc = HealthCheckConfig::Http {
        url: "http://localhost/health".to_string(),
        expected_status: 200,
        timeout: 5,
        interval: 10,
    };

    match hc {
        HealthCheckConfig::Http { url, .. } => {
            assert_eq!(url, "http://localhost/health");
        }
        _ => panic!("Wrong variant"),
    }
}

#[test]
fn test_resource_limits_creation() {
    let res = ResourceLimits {
        cpu: Some(2.0),
        memory: Some(1024 * 1024 * 1024),
        memory_str: Some("1G".to_string()),
    };

    assert_eq!(res.cpu, Some(2.0));
    assert!(res.memory.is_some());
}

#[test]
fn test_restart_policy_variants() {
    let policy1 = RestartPolicy::No;
    let policy2 = RestartPolicy::OnFailure;
    let policy3 = RestartPolicy::Always;

    assert_eq!(policy1, RestartPolicy::No);
    assert_eq!(policy2, RestartPolicy::OnFailure);
    assert_eq!(policy3, RestartPolicy::Always);
    assert_eq!(RestartPolicy::default(), RestartPolicy::OnFailure);
}

// ============================================================================
// CRUD Operations with MockMiseAdapter (10 tests)
// ============================================================================

#[tokio::test]
async fn test_create_service() {
    let adapter = MockMiseAdapter::new(
        MiseMock::new(std::path::PathBuf::from("/tmp")),
        MiseVersion::new(2024, 1, 0),
    );

    let service_def = ServiceDefinition {
        name: "test-service".to_string(),
        command: "echo hello".to_string(),
        working_dir: None,
        env: None,
        ports: None,
        health_check: None,
        resources: None,
        restart_policy: None,
        autostart: false,
        depends_on: None,
    };

    let result = adapter.create_service(&service_def).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_get_service() {
    let adapter = MockMiseAdapter::new(
        MiseMock::new(std::path::PathBuf::from("/tmp")),
        MiseVersion::new(2024, 1, 0),
    );

    let service_def = ServiceDefinition {
        name: "test-service".to_string(),
        command: "echo hello".to_string(),
        working_dir: None,
        env: None,
        ports: None,
        health_check: None,
        resources: None,
        restart_policy: None,
        autostart: false,
        depends_on: None,
    };

    adapter.create_service(&service_def).await.unwrap();

    let result = adapter.get_service("test-service").await;
    assert!(result.is_ok());
    let result = adapter.get_service("test-service").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_list_services() {
    let adapter = MockMiseAdapter::new(
        MiseMock::new(std::path::PathBuf::from("/tmp")),
        MiseVersion::new(2024, 1, 0),
    );

    let result = adapter.list_services().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_update_service() {
    let adapter = MockMiseAdapter::new(
        MiseMock::new(std::path::PathBuf::from("/tmp")),
        MiseVersion::new(2024, 1, 0),
    );

    let service_def = ServiceDefinition {
        name: "test-service".to_string(),
        command: "echo hello".to_string(),
        working_dir: None,
        env: None,
        ports: None,
        health_check: None,
        resources: None,
        restart_policy: None,
        autostart: false,
        depends_on: None,
    };

    adapter.create_service(&service_def).await.unwrap();

    let updated_def = ServiceDefinition {
        name: "test-service".to_string(),
        command: "echo world".to_string(),
        working_dir: None,
        env: None,
        ports: None,
        health_check: None,
        resources: None,
        restart_policy: None,
        autostart: false,
        depends_on: None,
    };

    let result = adapter.update_service("test-service", &updated_def).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_patch_service() {
    let adapter = MockMiseAdapter::new(
        MiseMock::new(std::path::PathBuf::from("/tmp")),
        MiseVersion::new(2024, 1, 0),
    );

    let service_def = ServiceDefinition {
        name: "test-service".to_string(),
        command: "echo hello".to_string(),
        working_dir: None,
        env: None,
        ports: None,
        health_check: None,
        resources: None,
        restart_policy: None,
        autostart: false,
        depends_on: None,
    };

    adapter.create_service(&service_def).await.unwrap();

    let patch = ServicePatchRequest {
        command: Some("echo patched".to_string()),
        working_dir: None,
        env: None,
        ports: None,
        health_check: None,
        resources: None,
        restart_policy: None,
        autostart: Some(true),
        depends_on: None,
    };

    let patch_json = serde_json::to_value(&patch).unwrap();
    let result = adapter.patch_service("test-service", &patch_json).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_delete_service() {
    let adapter = MockMiseAdapter::new(
        MiseMock::new(std::path::PathBuf::from("/tmp")),
        MiseVersion::new(2024, 1, 0),
    );

    let service_def = ServiceDefinition {
        name: "test-service".to_string(),
        command: "echo hello".to_string(),
        working_dir: None,
        env: None,
        ports: None,
        health_check: None,
        resources: None,
        restart_policy: None,
        autostart: false,
        depends_on: None,
    };

    adapter.create_service(&service_def).await.unwrap();

    let result = adapter.delete_service("test-service").await;
    assert!(result.is_ok());

    let get_result = adapter.get_service("test-service").await;
    assert!(get_result.is_err());
}

#[tokio::test]
async fn test_create_duplicate_service_fails() {
    let adapter = MockMiseAdapter::new(
        MiseMock::new(std::path::PathBuf::from("/tmp")),
        MiseVersion::new(2024, 1, 0),
    );

    let service_def = ServiceDefinition {
        name: "test-service".to_string(),
        command: "echo hello".to_string(),
        working_dir: None,
        env: None,
        ports: None,
        health_check: None,
        resources: None,
        restart_policy: None,
        autostart: false,
        depends_on: None,
    };

    adapter.create_service(&service_def).await.unwrap();

    let result = adapter.create_service(&service_def).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_get_nonexistent_service() {
    let adapter = MockMiseAdapter::new(
        MiseMock::new(std::path::PathBuf::from("/tmp")),
        MiseVersion::new(2024, 1, 0),
    );

    let result = adapter.get_service("nonexistent").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_update_nonexistent_service_fails() {
    let adapter = MockMiseAdapter::new(
        MiseMock::new(std::path::PathBuf::from("/tmp")),
        MiseVersion::new(2024, 1, 0),
    );

    let service_def = ServiceDefinition {
        name: "test-service".to_string(),
        command: "echo hello".to_string(),
        working_dir: None,
        env: None,
        ports: None,
        health_check: None,
        resources: None,
        restart_policy: None,
        autostart: false,
        depends_on: None,
    };

    let result = adapter.update_service("nonexistent", &service_def).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_delete_nonexistent_service_fails() {
    let adapter = MockMiseAdapter::new(
        MiseMock::new(std::path::PathBuf::from("/tmp")),
        MiseVersion::new(2024, 1, 0),
    );

    let result = adapter.delete_service("nonexistent").await;
    assert!(result.is_err());
}

// ============================================================================
// Service Definition Validation (5 tests)
// ============================================================================

#[test]
fn test_service_definition_validate_valid() {
    let def = ServiceDefinition {
        name: "valid-service".to_string(),
        command: "echo test".to_string(),
        working_dir: None,
        env: None,
        ports: None,
        health_check: None,
        resources: None,
        restart_policy: None,
        autostart: false,
        depends_on: None,
    };

    assert!(def.validate().is_ok());
}

#[test]
fn test_service_definition_validate_invalid_name() {
    let def = ServiceDefinition {
        name: "invalid name!".to_string(),
        command: "echo test".to_string(),
        working_dir: None,
        env: None,
        ports: None,
        health_check: None,
        resources: None,
        restart_policy: None,
        autostart: false,
        depends_on: None,
    };

    assert!(def.validate().is_err());
}

#[test]
fn test_service_definition_validate_empty_command() {
    let def = ServiceDefinition {
        name: "test-service".to_string(),
        command: "   ".to_string(),
        working_dir: None,
        env: None,
        ports: None,
        health_check: None,
        resources: None,
        restart_policy: None,
        autostart: false,
        depends_on: None,
    };

    assert!(def.validate().is_err());
}

#[test]
fn test_service_definition_validate_invalid_port() {
    let def = ServiceDefinition {
        name: "test-service".to_string(),
        command: "echo test".to_string(),
        working_dir: None,
        env: None,
        ports: Some(vec![PortMapping {
            host: 0, // Invalid port
            container: 8080,
            protocol: "tcp".to_string(),
        }]),
        health_check: None,
        resources: None,
        restart_policy: None,
        autostart: false,
        depends_on: None,
    };

    assert!(def.validate().is_err());
}

#[test]
fn test_service_definition_validate_invalid_protocol() {
    let def = ServiceDefinition {
        name: "test-service".to_string(),
        command: "echo test".to_string(),
        working_dir: None,
        env: None,
        ports: Some(vec![PortMapping {
            host: 8080,
            container: 80,
            protocol: "invalid".to_string(),
        }]),
        health_check: None,
        resources: None,
        restart_policy: None,
        autostart: false,
        depends_on: None,
    };

    assert!(def.validate().is_err());
}
