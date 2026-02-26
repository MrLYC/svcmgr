// 配置管理 API 处理器
//
// 实现 OpenSpec 14-api-config.md 定义的 10 个端点:
// - GET    /api/v1/config                - 获取完整配置
// - GET    /api/v1/config/{section}      - 获取特定段落
// - PUT    /api/v1/config                - 完整替换配置
// - PATCH  /api/v1/config/{section}      - 部分更新特定段落
// - POST   /api/v1/config/validate       - 验证配置(不实际应用)
// - GET    /api/v1/config/history        - 获取配置变更历史
// - POST   /api/v1/config/rollback       - 回滚到指定版本
// - GET    /api/v1/config/diff           - 对比两个版本的差异
// - GET    /api/v1/config/export         - 导出配置为 JSON
// - POST   /api/v1/config/import         - 导入配置并应用

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

use super::config_models::*;
use crate::git::versioning::{GitVersioning, RollbackTarget};
use crate::web::server::{ApiError, ApiResponse, AppState};

// ============================================================================
// 辅助函数
// ============================================================================

/// 获取 mise.toml 路径
fn get_mise_config_path(state: &AppState) -> std::path::PathBuf {
    state.config_dir.join(".config/mise/config.toml")
}

/// 获取 svcmgr.toml 路径
fn get_svcmgr_config_path(state: &AppState) -> std::path::PathBuf {
    state.config_dir.join(".config/mise/svcmgr/config.toml")
}

/// 从文件系统读取并合并配置
async fn read_merged_config(state: &AppState) -> Result<Config, ApiError> {
    let mise_path = get_mise_config_path(state);
    let svcmgr_path = get_svcmgr_config_path(state);

    // 读取 mise 配置
    let mise_content = tokio::fs::read_to_string(&mise_path)
        .await
        .unwrap_or_else(|_| String::new());

    // 读取 svcmgr 配置
    let svcmgr_content = tokio::fs::read_to_string(&svcmgr_path)
        .await
        .unwrap_or_else(|_| String::new());

    // 解析 mise 配置
    let mise_table: toml::Value =
        toml::from_str(&mise_content).unwrap_or(toml::Value::Table(toml::map::Map::new()));
    let svcmgr_table: toml::Value =
        toml::from_str(&svcmgr_content).unwrap_or(toml::Value::Table(toml::map::Map::new()));

    // 提取各个段落
    let tools = extract_section_as_map(&mise_table, "tools")?;
    let env = extract_section_as_map(&mise_table, "env")?;
    let tasks = extract_section_as_jsonvalue(&mise_table, "tasks")?;
    let services = extract_section_as_jsonvalue(&svcmgr_table, "services")?;
    let scheduled_tasks = extract_section_as_jsonvalue(&svcmgr_table, "scheduled_tasks")?;
    let features = extract_features(&svcmgr_table)?;
    let http = extract_http_config(&svcmgr_table)?;

    Ok(Config {
        tools,
        env,
        tasks,
        services,
        scheduled_tasks,
        features,
        http,
    })
}

/// 提取 TOML 段落为 HashMap<String, String>
fn extract_section_as_map(
    table: &toml::Value,
    section: &str,
) -> Result<HashMap<String, String>, ApiError> {
    let mut result = HashMap::new();
    if let Some(sec) = table.get(section).and_then(|v| v.as_table()) {
        for (k, v) in sec {
            if let Some(s) = v.as_str() {
                result.insert(k.clone(), s.to_string());
            } else {
                result.insert(k.clone(), v.to_string());
            }
        }
    }
    Ok(result)
}

/// 提取 TOML 段落为 HashMap<String, JsonValue>
fn extract_section_as_jsonvalue(
    table: &toml::Value,
    section: &str,
) -> Result<HashMap<String, JsonValue>, ApiError> {
    let mut result = HashMap::new();
    if let Some(sec) = table.get(section).and_then(|v| v.as_table()) {
        for (k, v) in sec {
            let json_value = toml_to_json(v);
            result.insert(k.clone(), json_value);
        }
    }
    Ok(result)
}

/// 转换 TOML Value 为 JSON Value
fn toml_to_json(toml_val: &toml::Value) -> JsonValue {
    match toml_val {
        toml::Value::String(s) => JsonValue::String(s.clone()),
        toml::Value::Integer(i) => JsonValue::Number((*i).into()),
        toml::Value::Float(f) => {
            if let Some(n) = serde_json::Number::from_f64(*f) {
                JsonValue::Number(n)
            } else {
                JsonValue::Null
            }
        }
        toml::Value::Boolean(b) => JsonValue::Bool(*b),
        toml::Value::Datetime(dt) => JsonValue::String(dt.to_string()),
        toml::Value::Array(arr) => {
            let json_arr: Vec<JsonValue> = arr.iter().map(toml_to_json).collect();
            JsonValue::Array(json_arr)
        }
        toml::Value::Table(table) => {
            let mut json_obj = serde_json::Map::new();
            for (k, v) in table {
                json_obj.insert(k.clone(), toml_to_json(v));
            }
            JsonValue::Object(json_obj)
        }
    }
}

/// 提取 Features 配置
fn extract_features(table: &toml::Value) -> Result<Features, ApiError> {
    if let Some(features_table) = table.get("features").and_then(|v| v.as_table()) {
        let json = toml_to_json(&toml::Value::Table(features_table.clone()));
        serde_json::from_value(json).map_err(|e| {
            ApiError::new(
                "CONFIG_PARSE_ERROR",
                format!("Failed to parse features: {}", e),
            )
        })
    } else {
        Ok(Features::default())
    }
}

/// 提取 HTTP 配置
fn extract_http_config(table: &toml::Value) -> Result<Option<HttpConfig>, ApiError> {
    if let Some(http_table) = table.get("http").and_then(|v| v.as_table()) {
        let json = toml_to_json(&toml::Value::Table(http_table.clone()));
        let http_config = serde_json::from_value(json).map_err(|e| {
            ApiError::new(
                "CONFIG_PARSE_ERROR",
                format!("Failed to parse http config: {}", e),
            )
        })?;
        Ok(Some(http_config))
    } else {
        Ok(None)
    }
}

/// 写入配置到文件系统
async fn write_config_files(state: &AppState, config: &Config) -> Result<(), ApiError> {
    let mise_path = get_mise_config_path(state);
    let svcmgr_path = get_svcmgr_config_path(state);

    // 构建 mise.toml 内容
    let mut mise_table = toml::map::Map::new();
    if !config.tools.is_empty() {
        let tools_table: toml::map::Map<String, toml::Value> = config
            .tools
            .iter()
            .map(|(k, v): (&String, &String)| (k.clone(), toml::Value::String(v.clone())))
            .collect();
        mise_table.insert("tools".to_string(), toml::Value::Table(tools_table));
    }
    if !config.env.is_empty() {
        let env_table: toml::map::Map<String, toml::Value> = config
            .env
            .iter()
            .map(|(k, v): (&String, &String)| (k.clone(), toml::Value::String(v.clone())))
            .collect();
        mise_table.insert("env".to_string(), toml::Value::Table(env_table));
    }
    if !config.tasks.is_empty() {
        let tasks_table = json_to_toml_table(&config.tasks)?;
        mise_table.insert("tasks".to_string(), toml::Value::Table(tasks_table));
    }

    // 构建 svcmgr.toml 内容
    let mut svcmgr_table = toml::map::Map::new();
    if !config.services.is_empty() {
        let services_table = json_to_toml_table(&config.services)?;
        svcmgr_table.insert("services".to_string(), toml::Value::Table(services_table));
    }
    if !config.scheduled_tasks.is_empty() {
        let scheduled_tasks_table = json_to_toml_table(&config.scheduled_tasks)?;
        svcmgr_table.insert(
            "scheduled_tasks".to_string(),
            toml::Value::Table(scheduled_tasks_table),
        );
    }

    // Features 配置
    let features_json = serde_json::to_value(&config.features).map_err(|e| {
        ApiError::new(
            "CONFIG_SERIALIZE_ERROR",
            format!("Failed to serialize features: {}", e),
        )
    })?;
    let features_table = json_to_toml_value(&features_json)?;
    if let toml::Value::Table(t) = features_table {
        svcmgr_table.insert("features".to_string(), toml::Value::Table(t));
    }

    // HTTP 配置
    if let Some(http) = &config.http {
        let http_json = serde_json::to_value(http).map_err(|e| {
            ApiError::new(
                "CONFIG_SERIALIZE_ERROR",
                format!("Failed to serialize http: {}", e),
            )
        })?;
        let http_toml = json_to_toml_value(&http_json)?;
        if let toml::Value::Table(t) = http_toml {
            svcmgr_table.insert("http".to_string(), toml::Value::Table(t));
        }
    }

    // 序列化为 TOML 字符串
    let mise_content = toml::to_string_pretty(&toml::Value::Table(mise_table)).map_err(|e| {
        ApiError::new(
            "CONFIG_SERIALIZE_ERROR",
            format!("Failed to serialize mise config: {}", e),
        )
    })?;
    let svcmgr_content =
        toml::to_string_pretty(&toml::Value::Table(svcmgr_table)).map_err(|e| {
            ApiError::new(
                "CONFIG_SERIALIZE_ERROR",
                format!("Failed to serialize svcmgr config: {}", e),
            )
        })?;

    // 确保目录存在
    if let Some(parent) = mise_path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| {
            ApiError::new(
                "IO_ERROR",
                format!("Failed to create mise config dir: {}", e),
            )
        })?;
    }
    if let Some(parent) = svcmgr_path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| {
            ApiError::new(
                "IO_ERROR",
                format!("Failed to create svcmgr config dir: {}", e),
            )
        })?;
    }

    // 写入文件
    tokio::fs::write(&mise_path, mise_content)
        .await
        .map_err(|e| ApiError::new("IO_ERROR", format!("Failed to write mise config: {}", e)))?;
    tokio::fs::write(&svcmgr_path, svcmgr_content)
        .await
        .map_err(|e| ApiError::new("IO_ERROR", format!("Failed to write svcmgr config: {}", e)))?;

    Ok(())
}

/// 转换 JSON HashMap 为 TOML Table
fn json_to_toml_table(
    map: &HashMap<String, JsonValue>,
) -> Result<toml::map::Map<String, toml::Value>, ApiError> {
    let mut table = toml::map::Map::new();
    for (k, v) in map {
        let toml_val = json_to_toml_value(v)?;
        table.insert(k.clone(), toml_val);
    }
    Ok(table)
}

/// 转换 JSON Value 为 TOML Value
fn json_to_toml_value(json: &JsonValue) -> Result<toml::Value, ApiError> {
    match json {
        JsonValue::Null => Ok(toml::Value::String("".to_string())),
        JsonValue::Bool(b) => Ok(toml::Value::Boolean(*b)),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(toml::Value::Integer(i))
            } else if let Some(f) = n.as_f64() {
                Ok(toml::Value::Float(f))
            } else {
                Err(ApiError::new("CONVERSION_ERROR", "Invalid number"))
            }
        }
        JsonValue::String(s) => Ok(toml::Value::String(s.clone())),
        JsonValue::Array(arr) => {
            let toml_arr: Result<Vec<toml::Value>, ApiError> =
                arr.iter().map(json_to_toml_value).collect();
            Ok(toml::Value::Array(toml_arr?))
        }
        JsonValue::Object(obj) => {
            let mut table = toml::map::Map::new();
            for (k, v) in obj {
                table.insert(k.clone(), json_to_toml_value(v)?);
            }
            Ok(toml::Value::Table(table))
        }
    }
}

/// Git 提交配置变更
async fn commit_config_change(git: &mut GitVersioning, message: &str) -> Result<(), ApiError> {
    git.auto_stage()
        .map_err(|e| ApiError::internal_error(format!("Git stage failed: {}", e)))?;

    if git
        .has_staged_changes()
        .map_err(|e| ApiError::internal_error(format!("Git check failed: {}", e)))?
    {
        git.commit(message, None)
            .map_err(|e| ApiError::internal_error(format!("Git commit failed: {}", e)))?;
    }

    Ok(())
}

// ============================================================================
// 处理器函数
// ============================================================================

/// GET /api/v1/config - 获取完整配置
async fn get_config(State(state): State<AppState>) -> Result<Json<ApiResponse<Config>>, ApiError> {
    let config = read_merged_config(&state).await?;

    Ok(Json(ApiResponse {
        data: config,
        pagination: None,
    }))
}

/// GET /api/v1/config/{section} - 获取特定段落
async fn get_config_section(
    State(state): State<AppState>,
    Path(section_str): Path<String>,
) -> Result<Json<ApiResponse<JsonValue>>, ApiError> {
    let section = ConfigSection::parse(&section_str).ok_or_else(|| {
        ApiError::new(
            "INVALID_INPUT",
            format!("Invalid config section: {}", section_str),
        )
    })?;

    let config = read_merged_config(&state).await?;

    let data = match section {
        ConfigSection::Tools => serde_json::to_value(&config.tools),
        ConfigSection::Env => serde_json::to_value(&config.env),
        ConfigSection::Tasks => serde_json::to_value(&config.tasks),
        ConfigSection::Services => serde_json::to_value(&config.services),
        ConfigSection::ScheduledTasks => serde_json::to_value(&config.scheduled_tasks),
        ConfigSection::Features => serde_json::to_value(&config.features),
        ConfigSection::Http => serde_json::to_value(&config.http),
    }
    .map_err(|e| {
        ApiError::new(
            "SERIALIZATION_ERROR",
            format!("Failed to serialize section: {}", e),
        )
    })?;

    Ok(Json(ApiResponse {
        data,
        pagination: None,
    }))
}

/// PUT /api/v1/config - 完整替换配置
async fn update_config(
    State(state): State<AppState>,
    Json(config): Json<Config>,
) -> Result<Json<ApiResponse<Config>>, ApiError> {
    // 写入配置文件
    write_config_files(&state, &config).await?;

    // Git 提交
    let mut git = state.git_versioning.lock().await;
    commit_config_change(&mut git, "feat: update full configuration").await?;

    Ok(Json(ApiResponse {
        data: config,
        pagination: None,
    }))
}

/// PATCH /api/v1/config/{section} - 部分更新特定段落
async fn patch_config_section(
    State(state): State<AppState>,
    Path(section_str): Path<String>,
    Json(request): Json<PatchConfigSectionRequest>,
) -> Result<Json<ApiResponse<JsonValue>>, ApiError> {
    let section = ConfigSection::parse(&section_str).ok_or_else(|| {
        ApiError::new(
            "INVALID_INPUT",
            format!("Invalid config section: {}", section_str),
        )
    })?;

    // 读取当前配置
    let mut config = read_merged_config(&state).await?;

    // 应用 PATCH 操作
    match (section, request.op) {
        (ConfigSection::Tools, PatchOperation::Merge) => {
            if let JsonValue::Object(obj) = request.data {
                for (k, v) in obj {
                    if let JsonValue::String(s) = v {
                        config.tools.insert(k, s);
                    }
                }
            }
        }
        (ConfigSection::Tools, PatchOperation::Replace) => {
            if let JsonValue::Object(obj) = request.data {
                config.tools.clear();
                for (k, v) in obj {
                    if let JsonValue::String(s) = v {
                        config.tools.insert(k, s);
                    }
                }
            }
        }
        (ConfigSection::Tools, PatchOperation::Remove) => {
            if let JsonValue::Array(keys) = request.data {
                for key in keys {
                    if let JsonValue::String(k) = key {
                        config.tools.remove(&k);
                    }
                }
            }
        }
        (ConfigSection::Env, PatchOperation::Merge) => {
            if let JsonValue::Object(obj) = request.data {
                for (k, v) in obj {
                    if let JsonValue::String(s) = v {
                        config.env.insert(k, s);
                    }
                }
            }
        }
        (ConfigSection::Env, PatchOperation::Replace) => {
            if let JsonValue::Object(obj) = request.data {
                config.env.clear();
                for (k, v) in obj {
                    if let JsonValue::String(s) = v {
                        config.env.insert(k, s);
                    }
                }
            }
        }
        (ConfigSection::Env, PatchOperation::Remove) => {
            if let JsonValue::Array(keys) = request.data {
                for key in keys {
                    if let JsonValue::String(k) = key {
                        config.env.remove(&k);
                    }
                }
            }
        }
        (ConfigSection::Tasks, op) => {
            apply_patch_to_jsonvalue_map(&mut config.tasks, &request.data, op)?;
        }
        (ConfigSection::Services, op) => {
            apply_patch_to_jsonvalue_map(&mut config.services, &request.data, op)?;
        }
        (ConfigSection::ScheduledTasks, op) => {
            apply_patch_to_jsonvalue_map(&mut config.scheduled_tasks, &request.data, op)?;
        }
        (ConfigSection::Features, PatchOperation::Merge | PatchOperation::Replace) => {
            config.features = serde_json::from_value(request.data).map_err(|e| {
                ApiError::new("INVALID_DATA", format!("Invalid features data: {}", e))
            })?;
        }
        (ConfigSection::Http, PatchOperation::Merge | PatchOperation::Replace) => {
            config.http = serde_json::from_value(request.data)
                .map_err(|e| ApiError::new("INVALID_DATA", format!("Invalid http data: {}", e)))?;
        }
        _ => {
            return Err(ApiError::new(
                "UNSUPPORTED_OPERATION",
                format!(
                    "Operation {:?} not supported for section {:?}",
                    request.op, section
                ),
            ));
        }
    }

    // 写入配置文件
    write_config_files(&state, &config).await?;

    // Git 提交
    let mut git = state.git_versioning.lock().await;
    commit_config_change(
        &mut git,
        &format!("feat: patch config section {}", section_str),
    )
    .await?;

    // 返回更新后的段落
    let data = match section {
        ConfigSection::Tools => serde_json::to_value(&config.tools),
        ConfigSection::Env => serde_json::to_value(&config.env),
        ConfigSection::Tasks => serde_json::to_value(&config.tasks),
        ConfigSection::Services => serde_json::to_value(&config.services),
        ConfigSection::ScheduledTasks => serde_json::to_value(&config.scheduled_tasks),
        ConfigSection::Features => serde_json::to_value(&config.features),
        ConfigSection::Http => serde_json::to_value(&config.http),
    }
    .map_err(|e| {
        ApiError::new(
            "SERIALIZATION_ERROR",
            format!("Failed to serialize section: {}", e),
        )
    })?;

    Ok(Json(ApiResponse {
        data,
        pagination: None,
    }))
}

/// 应用 PATCH 操作到 JsonValue HashMap
fn apply_patch_to_jsonvalue_map(
    map: &mut HashMap<String, JsonValue>,
    data: &JsonValue,
    op: PatchOperation,
) -> Result<(), ApiError> {
    match op {
        PatchOperation::Merge => {
            if let JsonValue::Object(obj) = data {
                for (k, v) in obj {
                    map.insert(k.clone(), v.clone());
                }
            } else {
                return Err(ApiError::new(
                    "INVALID_DATA",
                    "PATCH merge requires object data",
                ));
            }
        }
        PatchOperation::Replace => {
            if let JsonValue::Object(obj) = data {
                map.clear();
                for (k, v) in obj {
                    map.insert(k.clone(), v.clone());
                }
            } else {
                return Err(ApiError::new(
                    "INVALID_DATA",
                    "PATCH replace requires object data",
                ));
            }
        }
        PatchOperation::Remove => {
            if let JsonValue::Array(keys) = data {
                for key in keys {
                    if let JsonValue::String(k) = key {
                        map.remove(k);
                    }
                }
            } else {
                return Err(ApiError::new(
                    "INVALID_DATA",
                    "PATCH remove requires array of keys",
                ));
            }
        }
    }
    Ok(())
}

/// POST /api/v1/config/validate - 验证配置(不实际应用)
async fn validate_config(
    Json(request): Json<ValidateConfigRequest>,
) -> Result<Json<ApiResponse<ValidationResult>>, ApiError> {
    let mut errors = Vec::new();
    let warnings = Vec::new();

    // 基础验证: 检查工具定义是否有效
    for (tool, version) in &request.config.tools {
        if version.is_empty() {
            errors.push(ValidationError {
                kind: ValidationErrorKind::MissingField,
                path: format!("tools.{}", tool),
                message: "Tool version cannot be empty".to_string(),
            });
        }
    }

    // 验证服务端口冲突
    let mut used_ports = std::collections::HashSet::new();
    for (service_name, service_def) in &request.config.services {
        if let Some(ports_obj) = service_def.get("ports").and_then(|v| v.as_object()) {
            for (port_name, port_value) in ports_obj {
                if let Some(port_num) = port_value.as_u64() {
                    if used_ports.contains(&port_num) {
                        errors.push(ValidationError {
                            kind: ValidationErrorKind::PortConflict,
                            path: format!("services.{}.ports.{}", service_name, port_name),
                            message: format!("Port {} is already in use", port_num),
                        });
                    } else {
                        used_ports.insert(port_num);
                    }
                }
            }
        }
    }

    let valid = errors.is_empty();

    Ok(Json(ApiResponse {
        data: ValidationResult {
            valid,
            errors,
            warnings,
        },
        pagination: None,
    }))
}

/// GET /api/v1/config/history - 获取配置变更历史
async fn get_config_history(
    State(state): State<AppState>,
    Query(params): Query<ConfigHistoryQuery>,
) -> Result<Json<ApiResponse<Vec<ConfigHistory>>>, ApiError> {
    let git = state.git_versioning.lock().await;

    let limit = params.limit.min(200); // 最多返回 200 条
    let commits = git
        .log(limit + params.offset)
        .map_err(|e| ApiError::internal_error(format!("Failed to read git log: {}", e)))?;

    // 跳过 offset
    let commits: Vec<_> = commits
        .into_iter()
        .skip(params.offset)
        .take(limit)
        .collect();

    let mut history = Vec::new();
    for commit_info in commits {
        let files = git
            .get_changed_files_in_commit(&commit_info.id)
            .unwrap_or_default();

        // 过滤: 如果指定了 file,只返回涉及该文件的提交
        if let Some(file_filter) = &params.file
            && !files.iter().any(|f| f.contains(file_filter))
        {
            continue;
        }

        history.push(ConfigHistory {
            commit: commit_info.id,
            message: commit_info.message,
            timestamp: DateTime::<Utc>::from_timestamp(commit_info.time, 0)
                .unwrap_or_else(Utc::now),
            author: commit_info.author,
            files,
        });
    }

    Ok(Json(ApiResponse {
        data: history,
        pagination: None,
    }))
}

/// POST /api/v1/config/rollback - 回滚到指定版本
async fn rollback_config(
    State(state): State<AppState>,
    Json(request): Json<RollbackConfigRequest>,
) -> Result<Json<ApiResponse<Config>>, ApiError> {
    let mut git = state.git_versioning.lock().await;

    let target = RollbackTarget::Commit(request.commit);
    git.rollback(target, false)
        .map_err(|e| ApiError::internal_error(format!("Rollback failed: {}", e)))?;

    // 释放 Git 锁
    drop(git);

    // 重新读取配置
    let config = read_merged_config(&state).await?;

    Ok(Json(ApiResponse {
        data: config,
        pagination: None,
    }))
}

/// GET /api/v1/config/diff - 对比两个版本的差异
async fn get_config_diff(
    State(state): State<AppState>,
    Query(params): Query<ConfigDiffQuery>,
) -> Result<Json<ApiResponse<ConfigDiff>>, ApiError> {
    let git = state.git_versioning.lock().await;

    let from = params.from.unwrap_or_else(|| "HEAD~1".to_string());
    let to = params.to.unwrap_or_else(|| "HEAD".to_string());

    // 获取两个 commit 的 diff (这里简化处理,实际需要调用 git diff 命令)
    let diff_output = git
        .diff_staged()
        .map_err(|e| ApiError::internal_error(format!("Failed to get diff: {}", e)))?;

    // 统计差异
    let stats = parse_diff_stats(&diff_output);

    Ok(Json(ApiResponse {
        data: ConfigDiff {
            from,
            to,
            diff: diff_output,
            stats,
        },
        pagination: None,
    }))
}

/// 解析 diff 统计信息
fn parse_diff_stats(diff: &str) -> DiffStats {
    let mut files_changed = 0;
    let mut insertions = 0;
    let mut deletions = 0;

    for line in diff.lines() {
        if line.starts_with("diff --git") {
            files_changed += 1;
        } else if line.starts_with('+') && !line.starts_with("+++") {
            insertions += 1;
        } else if line.starts_with('-') && !line.starts_with("---") {
            deletions += 1;
        }
    }

    DiffStats {
        files_changed,
        insertions,
        deletions,
    }
}

/// GET /api/v1/config/export - 导出配置为 JSON
async fn export_config(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Config>>, ApiError> {
    let config = read_merged_config(&state).await?;

    Ok(Json(ApiResponse {
        data: config,
        pagination: None,
    }))
}

/// POST /api/v1/config/import - 导入配置并应用
async fn import_config(
    State(state): State<AppState>,
    Json(request): Json<ImportConfigRequest>,
) -> Result<Json<ApiResponse<Config>>, ApiError> {
    // 解析导入的配置
    let imported_config: Config = match request.format {
        ExportFormat::Json => serde_json::from_str(&request.config)
            .map_err(|e| ApiError::new("INVALID_FORMAT", format!("Invalid JSON: {}", e)))?,
        ExportFormat::Toml => toml::from_str(&request.config)
            .map_err(|e| ApiError::new("INVALID_FORMAT", format!("Invalid TOML: {}", e)))?,
    };

    // 如果是合并模式,先读取现有配置
    let final_config = if request.overwrite {
        imported_config
    } else {
        let mut current = read_merged_config(&state).await?;
        // 合并配置
        current.tools.extend(imported_config.tools);
        current.env.extend(imported_config.env);
        current.tasks.extend(imported_config.tasks);
        current.services.extend(imported_config.services);
        current
            .scheduled_tasks
            .extend(imported_config.scheduled_tasks);
        if imported_config.http.is_some() {
            current.http = imported_config.http;
        }
        current
    };

    // 写入配置文件
    write_config_files(&state, &final_config).await?;

    // Git 提交
    let mut git = state.git_versioning.lock().await;
    let message = if request.overwrite {
        "feat: import configuration (overwrite)"
    } else {
        "feat: import configuration (merge)"
    };
    commit_config_change(&mut git, message).await?;

    Ok(Json(ApiResponse {
        data: final_config,
        pagination: None,
    }))
}

// ============================================================================
// 路由注册
// ============================================================================

/// 创建配置管理路由
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(get_config).put(update_config))
        .route("/validate", post(validate_config))
        .route("/history", get(get_config_history))
        .route("/rollback", post(rollback_config))
        .route("/diff", get(get_config_diff))
        .route("/export", get(export_config))
        .route("/import", post(import_config))
        .route(
            "/:section",
            get(get_config_section).patch(patch_config_section),
        )
}
