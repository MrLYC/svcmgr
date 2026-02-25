// 配置管理 API 基础测试
//
// 测试覆盖范围:
// 1. 配置数据模型测试 (10 tests)
// 2. 配置验证测试 (8 tests)
// 3. TOML 序列化/反序列化测试 (6 tests)
// 4. 配置操作测试 (8 tests)

use chrono::Utc;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use svcmgr::web::api::config_models::*;

// ============================================================================
// 配置数据模型测试 (10 tests)
// ============================================================================

#[test]
fn test_config_creation() {
    let config = Config {
        tools: HashMap::new(),
        env: HashMap::new(),
        tasks: HashMap::new(),
        services: HashMap::new(),
        scheduled_tasks: HashMap::new(),
        features: Features::default(),
        http: None,
    };

    assert!(config.tools.is_empty());
    assert!(config.env.is_empty());
    assert!(config.tasks.is_empty());
    assert!(config.services.is_empty());
    assert!(config.scheduled_tasks.is_empty());
}

#[test]
fn test_config_with_tools() {
    let mut tools = HashMap::new();
    tools.insert("node".to_string(), "20.0.0".to_string());
    tools.insert("python".to_string(), "3.11".to_string());

    let config = Config {
        tools,
        env: HashMap::new(),
        tasks: HashMap::new(),
        services: HashMap::new(),
        scheduled_tasks: HashMap::new(),
        features: Features::default(),
        http: None,
    };

    assert_eq!(config.tools.len(), 2);
    assert_eq!(config.tools.get("node"), Some(&"20.0.0".to_string()));
    assert_eq!(config.tools.get("python"), Some(&"3.11".to_string()));
}

#[test]
fn test_config_with_env() {
    let mut env = HashMap::new();
    env.insert("NODE_ENV".to_string(), "production".to_string());
    env.insert("LOG_LEVEL".to_string(), "info".to_string());

    let config = Config {
        tools: HashMap::new(),
        env,
        tasks: HashMap::new(),
        services: HashMap::new(),
        scheduled_tasks: HashMap::new(),
        features: Features::default(),
        http: None,
    };

    assert_eq!(config.env.len(), 2);
    assert_eq!(config.env.get("NODE_ENV"), Some(&"production".to_string()));
    assert_eq!(config.env.get("LOG_LEVEL"), Some(&"info".to_string()));
}

#[test]
fn test_config_section_enum() {
    let sections = vec![
        ConfigSection::Tools,
        ConfigSection::Env,
        ConfigSection::Tasks,
        ConfigSection::Services,
        ConfigSection::ScheduledTasks,
        ConfigSection::Features,
        ConfigSection::Http,
    ];

    assert_eq!(sections.len(), 7);
    assert_eq!(ConfigSection::Tools.to_toml_key(), "tools");
    assert_eq!(ConfigSection::Env.to_toml_key(), "env");
    assert_eq!(ConfigSection::Tasks.to_toml_key(), "tasks");
    assert_eq!(ConfigSection::Services.to_toml_key(), "services");
    assert_eq!(
        ConfigSection::ScheduledTasks.to_toml_key(),
        "scheduled_tasks"
    );
    assert_eq!(ConfigSection::Features.to_toml_key(), "features");
    assert_eq!(ConfigSection::Http.to_toml_key(), "http");
}

#[test]
fn test_config_section_from_str() {
    assert_eq!(ConfigSection::parse("tools"), Some(ConfigSection::Tools));
    assert_eq!(ConfigSection::parse("env"), Some(ConfigSection::Env));
    assert_eq!(ConfigSection::parse("tasks"), Some(ConfigSection::Tasks));
    assert_eq!(
        ConfigSection::parse("services"),
        Some(ConfigSection::Services)
    );
    assert_eq!(
        ConfigSection::parse("scheduled_tasks"),
        Some(ConfigSection::ScheduledTasks)
    );
    assert_eq!(
        ConfigSection::parse("features"),
        Some(ConfigSection::Features)
    );
    assert_eq!(ConfigSection::parse("http"), Some(ConfigSection::Http));
    assert_eq!(ConfigSection::parse("invalid"), None);
}

#[test]
fn test_features_default() {
    let features = Features::default();

    assert_eq!(features.systemd, FeatureMode::Auto);
    assert_eq!(features.cgroups, FeatureMode::Auto);
    assert_eq!(features.http_proxy, FeatureMode::Auto);
    assert_eq!(features.git_auto_commit, FeatureMode::Enabled);
}

#[test]
fn test_features_custom() {
    let features = Features {
        systemd: FeatureMode::Enabled,
        cgroups: FeatureMode::Disabled,
        http_proxy: FeatureMode::Auto,
        git_auto_commit: FeatureMode::Enabled,
    };

    assert_eq!(features.systemd, FeatureMode::Enabled);
    assert_eq!(features.cgroups, FeatureMode::Disabled);
    assert_eq!(features.http_proxy, FeatureMode::Auto);
    assert_eq!(features.git_auto_commit, FeatureMode::Enabled);
}

#[test]
fn test_feature_mode_variants() {
    let mode1 = FeatureMode::Auto;
    let mode2 = FeatureMode::Enabled;
    let mode3 = FeatureMode::Disabled;

    assert_eq!(mode1, FeatureMode::Auto);
    assert_eq!(mode2, FeatureMode::Enabled);
    assert_eq!(mode3, FeatureMode::Disabled);
}

#[test]
fn test_http_config_default() {
    let http = HttpConfig {
        listen: "127.0.0.1:3080".to_string(),
        routes: vec![],
    };

    assert_eq!(http.listen, "127.0.0.1:3080");
    assert!(http.routes.is_empty());
}

#[test]
fn test_http_config_with_routes() {
    let route = HttpRoute {
        path: "/api".to_string(),
        target: "app-service".to_string(),
        port: "http".to_string(),
        rewrite: None,
    };

    let http = HttpConfig {
        listen: "0.0.0.0:8080".to_string(),
        routes: vec![route],
    };

    assert_eq!(http.listen, "0.0.0.0:8080");
    assert_eq!(http.routes.len(), 1);
    assert_eq!(http.routes[0].path, "/api");
    assert_eq!(http.routes[0].target, "app-service");
}

// ============================================================================
// 配置验证测试 (8 tests)
// ============================================================================

#[test]
fn test_validation_result_valid() {
    let result = ValidationResult {
        valid: true,
        errors: vec![],
        warnings: vec![],
    };

    assert!(result.valid);
    assert!(result.errors.is_empty());
    assert!(result.warnings.is_empty());
}

#[test]
fn test_validation_result_with_errors() {
    let error = ValidationError {
        kind: ValidationErrorKind::MissingField,
        path: "tools.node".to_string(),
        message: "Node.js version is required".to_string(),
    };

    let result = ValidationResult {
        valid: false,
        errors: vec![error],
        warnings: vec![],
    };

    assert!(!result.valid);
    assert_eq!(result.errors.len(), 1);
    assert_eq!(result.errors[0].kind, ValidationErrorKind::MissingField);
}

#[test]
fn test_validation_error_kinds() {
    let kinds = vec![
        ValidationErrorKind::Syntax,
        ValidationErrorKind::Type,
        ValidationErrorKind::MissingField,
        ValidationErrorKind::MissingDependency,
        ValidationErrorKind::CircularDependency,
        ValidationErrorKind::PortConflict,
        ValidationErrorKind::InvalidPath,
        ValidationErrorKind::Other,
    ];

    assert_eq!(kinds.len(), 8);
}

#[test]
fn test_validation_error_syntax() {
    let error = ValidationError {
        kind: ValidationErrorKind::Syntax,
        path: "config.toml".to_string(),
        message: "Invalid TOML syntax".to_string(),
    };

    assert_eq!(error.kind, ValidationErrorKind::Syntax);
    assert_eq!(error.path, "config.toml");
}

#[test]
fn test_validation_error_port_conflict() {
    let error = ValidationError {
        kind: ValidationErrorKind::PortConflict,
        path: "services.api.ports.http".to_string(),
        message: "Port 8080 is already in use".to_string(),
    };

    assert_eq!(error.kind, ValidationErrorKind::PortConflict);
    assert!(error.message.contains("8080"));
}

#[test]
fn test_validation_error_missing_dependency() {
    let error = ValidationError {
        kind: ValidationErrorKind::MissingDependency,
        path: "services.webapp".to_string(),
        message: "Service depends on missing tool 'node'".to_string(),
    };

    assert_eq!(error.kind, ValidationErrorKind::MissingDependency);
    assert!(error.message.contains("node"));
}

#[test]
fn test_validation_warning() {
    let warning = ValidationWarning {
        path: "tools.python".to_string(),
        message: "Python 3.8 is deprecated, consider upgrading".to_string(),
    };

    assert_eq!(warning.path, "tools.python");
    assert!(warning.message.contains("deprecated"));
}

#[test]
fn test_validate_config_request() {
    let config = Config {
        tools: HashMap::new(),
        env: HashMap::new(),
        tasks: HashMap::new(),
        services: HashMap::new(),
        scheduled_tasks: HashMap::new(),
        features: Features::default(),
        http: None,
    };

    let request = ValidateConfigRequest { config };

    assert!(request.config.tools.is_empty());
}

// ============================================================================
// TOML 序列化/反序列化测试 (6 tests)
// ============================================================================

#[test]
fn test_config_json_serialization() {
    let config = Config {
        tools: HashMap::new(),
        env: HashMap::new(),
        tasks: HashMap::new(),
        services: HashMap::new(),
        scheduled_tasks: HashMap::new(),
        features: Features::default(),
        http: None,
    };

    let json = serde_json::to_string(&config);
    assert!(json.is_ok());
}

#[test]
fn test_config_json_deserialization() {
    let json = r#"{
        "tools": {},
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
    }"#;

    let config: Result<Config, _> = serde_json::from_str(json);
    assert!(config.is_ok());
    let config = config.unwrap();
    assert_eq!(config.features.git_auto_commit, FeatureMode::Enabled);
}

#[test]
fn test_features_json_serialization() {
    let features = Features {
        systemd: FeatureMode::Enabled,
        cgroups: FeatureMode::Disabled,
        http_proxy: FeatureMode::Auto,
        git_auto_commit: FeatureMode::Enabled,
    };

    let json = serde_json::to_value(&features);
    assert!(json.is_ok());
    let json = json.unwrap();
    assert_eq!(json["systemd"], "enabled");
    assert_eq!(json["cgroups"], "disabled");
    assert_eq!(json["http_proxy"], "auto");
}

#[test]
fn test_http_config_json_serialization() {
    let http = HttpConfig {
        listen: "127.0.0.1:3080".to_string(),
        routes: vec![],
    };

    let json = serde_json::to_value(&http);
    assert!(json.is_ok());
    let json = json.unwrap();
    assert_eq!(json["listen"], "127.0.0.1:3080");
}

#[test]
fn test_patch_operation_variants() {
    let op1 = PatchOperation::Merge;
    let op2 = PatchOperation::Replace;
    let op3 = PatchOperation::Remove;

    assert_eq!(op1, PatchOperation::Merge);
    assert_eq!(op2, PatchOperation::Replace);
    assert_eq!(op3, PatchOperation::Remove);
}

#[test]
fn test_patch_config_section_request() {
    let mut data = serde_json::Map::new();
    data.insert("key".to_string(), JsonValue::String("value".to_string()));

    let request = PatchConfigSectionRequest {
        op: PatchOperation::Merge,
        data: JsonValue::Object(data),
    };

    assert_eq!(request.op, PatchOperation::Merge);
    assert!(request.data.is_object());
}

// ============================================================================
// 配置历史和差异测试 (8 tests)
// ============================================================================

#[test]
fn test_config_history_creation() {
    let history = ConfigHistory {
        commit: "abc123".to_string(),
        message: "feat: update config".to_string(),
        timestamp: Utc::now(),
        author: "user".to_string(),
        files: vec!["config.toml".to_string()],
    };

    assert_eq!(history.commit, "abc123");
    assert_eq!(history.message, "feat: update config");
    assert_eq!(history.author, "user");
    assert_eq!(history.files.len(), 1);
}

#[test]
fn test_config_diff_creation() {
    let diff = ConfigDiff {
        from: "HEAD~1".to_string(),
        to: "HEAD".to_string(),
        diff: "diff content".to_string(),
        stats: DiffStats {
            files_changed: 2,
            insertions: 10,
            deletions: 5,
        },
    };

    assert_eq!(diff.from, "HEAD~1");
    assert_eq!(diff.to, "HEAD");
    assert_eq!(diff.stats.files_changed, 2);
    assert_eq!(diff.stats.insertions, 10);
    assert_eq!(diff.stats.deletions, 5);
}

#[test]
fn test_diff_stats() {
    let stats = DiffStats {
        files_changed: 3,
        insertions: 20,
        deletions: 15,
    };

    assert_eq!(stats.files_changed, 3);
    assert_eq!(stats.insertions, 20);
    assert_eq!(stats.deletions, 15);
}

#[test]
fn test_config_history_query() {
    let query = ConfigHistoryQuery {
        limit: 50,
        offset: 0,
        file: Some("config.toml".to_string()),
    };

    assert_eq!(query.limit, 50);
    assert_eq!(query.offset, 0);
    assert!(query.file.is_some());
}

#[test]
fn test_config_diff_query() {
    let query = ConfigDiffQuery {
        from: Some("HEAD~1".to_string()),
        to: Some("HEAD".to_string()),
    };

    assert_eq!(query.from, Some("HEAD~1".to_string()));
    assert_eq!(query.to, Some("HEAD".to_string()));
}

#[test]
fn test_rollback_config_request() {
    let request = RollbackConfigRequest {
        commit: "abc123".to_string(),
    };

    assert_eq!(request.commit, "abc123");
    assert_eq!(request.commit, "abc123");
}

#[test]
fn test_export_format_variants() {
    let format1 = ExportFormat::Json;
    let format2 = ExportFormat::Toml;

    assert_eq!(format1, ExportFormat::Json);
    assert_eq!(format2, ExportFormat::Toml);
}

#[test]
fn test_import_config_request() {
    let request = ImportConfigRequest {
        config: "{}".to_string(),
        format: ExportFormat::Json,
        overwrite: false,
    };

    assert_eq!(request.config, "{}");
    assert_eq!(request.format, ExportFormat::Json);
    assert!(!request.overwrite);
}
