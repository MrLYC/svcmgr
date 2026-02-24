use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose};

use crate::web::server::{ApiError, ApiResponse, AppState, Pagination};

use super::env_models::*;
use crate::env::EnvScope;

pub fn routes(app_state: AppState) -> Router {
    Router::new()
        .route("/batch", post(batch_env_operations))
        .route("/import", post(import_env_file))
        .route("/export", get(export_env_file))
        .route("/", get(list_env_vars))
        .route(
            "/:key",
            get(get_env_var).put(set_env_var).delete(delete_env_var),
        )
        .with_state(app_state)
}

async fn list_env_vars(
    State(state): State<AppState>,
    Query(params): Query<ListEnvVarsParams>,
) -> Result<Json<ApiResponse<Vec<EnvVar>>>, ApiError> {
    let config_port = &state.config_port;

    // 1. 获取所有作用域的环境变量
    let global_env = config_port
        .get_global_env()
        .await
        .map_err(|e| ApiError::new("CONFIG_ERROR", format!("Failed to get global env: {}", e)))?;

    let service_envs = config_port
        .get_service_envs()
        .await
        .map_err(|e| ApiError::new("CONFIG_ERROR", format!("Failed to get service envs: {}", e)))?;

    let task_envs = config_port
        .get_task_envs()
        .await
        .map_err(|e| ApiError::new("CONFIG_ERROR", format!("Failed to get task envs: {}", e)))?;

    // 2. 收集所有环境变量到统一结构
    let mut all_vars = Vec::new();
    let now = chrono::Utc::now();

    // Global scope
    for (key, value) in global_env {
        all_vars.push((key, value, EnvScope::Global));
    }

    // Service scopes
    for (service_name, env) in service_envs {
        for (key, value) in env {
            all_vars.push((
                key,
                value,
                EnvScope::Service {
                    name: service_name.clone(),
                },
            ));
        }
    }

    // Task scopes
    for (task_name, env) in task_envs {
        for (key, value) in env {
            all_vars.push((
                key,
                value,
                EnvScope::Task {
                    name: task_name.clone(),
                },
            ));
        }
    }

    // 3. 按 scopes 参数过滤
    if !params.scopes.is_empty() {
        let scope_filters: Result<Vec<EnvScope>, String> =
            params.scopes.iter().map(|s| parse_scope(s)).collect();

        let scope_filters = scope_filters.map_err(|e| ApiError::new("INVALID_SCOPE", &e))?;

        all_vars.retain(|(_, _, scope)| {
            scope_filters.iter().any(|filter| match (filter, scope) {
                (EnvScope::Global, EnvScope::Global) => true,
                (EnvScope::Service { name: f_name }, EnvScope::Service { name: s_name }) => {
                    f_name == s_name
                }
                (EnvScope::Task { name: f_name }, EnvScope::Task { name: s_name }) => {
                    f_name == s_name
                }
                _ => false,
            })
        });
    }

    // 4. 按 prefix 过滤
    if let Some(ref prefix) = params.prefix {
        all_vars.retain(|(key, _, _)| key.starts_with(prefix));
    }

    // 5. 按 search 过滤(值包含搜索字符串)
    if let Some(ref search) = params.search {
        all_vars.retain(|(_, value, _)| value.contains(search));
    }

    // 6. 计算分页
    let total = all_vars.len() as u64;
    let start = ((params.page - 1) * params.per_page) as usize;
    let end = (start + params.per_page as usize).min(all_vars.len());

    // 7. 分页切片
    let page_vars = &all_vars[start..end];

    // 8. 可选展开变量引用
    let mut result_vars = Vec::new();
    if params.expand {
        use crate::env::VariableExpander;
        let mut expander = VariableExpander::new(config_port.as_ref())
            .await
            .map_err(|e| {
                ApiError::new(
                    "EXPANDER_ERROR",
                    format!("Failed to create expander: {}", e),
                )
            })?;

        for (key, value, scope) in page_vars {
            let has_refs = value.contains("${");
            let expanded = if has_refs {
                expander.expand(value, scope).await.ok()
            } else {
                None
            };

            result_vars.push(EnvVar {
                key: key.clone(),
                value: value.clone(),
                scope: scope.clone(),
                source_file: get_source_file(scope),
                has_references: has_refs,
                expanded_value: expanded,
                created_at: now,
                updated_at: now,
            });
        }
    } else {
        for (key, value, scope) in page_vars {
            result_vars.push(EnvVar {
                key: key.clone(),
                value: value.clone(),
                scope: scope.clone(),
                source_file: get_source_file(scope),
                has_references: value.contains("${"),
                expanded_value: None,
                created_at: now,
                updated_at: now,
            });
        }
    }

    // 9. 构造响应
    let response = ApiResponse {
        data: result_vars,
        pagination: Some(Pagination {
            page: params.page,
            per_page: params.per_page,
            total,
            total_pages: total.div_ceil(params.per_page as u64) as u32,
        }),
    };

    Ok(Json(response))
}

async fn get_env_var(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Query(params): Query<GetEnvVarParams>,
) -> Result<Json<EnvVarDetail>, ApiError> {
    let config_port = &state.config_port;

    // 1. 从所有作用域获取环境变量
    let global_env = config_port
        .get_global_env()
        .await
        .map_err(|e| ApiError::new("CONFIG_ERROR", format!("Failed to get global env: {}", e)))?;

    let service_envs = config_port
        .get_service_envs()
        .await
        .map_err(|e| ApiError::new("CONFIG_ERROR", format!("Failed to get service envs: {}", e)))?;

    let task_envs = config_port
        .get_task_envs()
        .await
        .map_err(|e| ApiError::new("CONFIG_ERROR", format!("Failed to get task envs: {}", e)))?;

    // 2. 收集该变量的所有定义
    let mut definitions = Vec::new();

    // Global scope
    if let Some(value) = global_env.get(&key) {
        definitions.push(ScopeDefinition {
            scope: EnvScope::Global,
            value: value.clone(),
            source_file: get_source_file(&EnvScope::Global),
            priority: scope_priority(&EnvScope::Global),
        });
    }

    // Service scopes
    for (service_name, env) in service_envs {
        if let Some(value) = env.get(&key) {
            let scope = EnvScope::Service {
                name: service_name.clone(),
            };
            definitions.push(ScopeDefinition {
                scope: scope.clone(),
                value: value.clone(),
                source_file: get_source_file(&scope),
                priority: scope_priority(&scope),
            });
        }
    }

    // Task scopes
    for (task_name, env) in task_envs {
        if let Some(value) = env.get(&key) {
            let scope = EnvScope::Task {
                name: task_name.clone(),
            };
            definitions.push(ScopeDefinition {
                scope: scope.clone(),
                value: value.clone(),
                source_file: get_source_file(&scope),
                priority: scope_priority(&scope),
            });
        }
    }

    // 3. 检查是否找到变量
    if definitions.is_empty() {
        return Err(ApiError::new(
            "NOT_FOUND",
            format!("Environment variable '{}' not found in any scope", key),
        ));
    }

    // 4. 按优先级排序，找出生效值
    definitions.sort_by(|a, b| b.priority.cmp(&a.priority));
    let effective_def = definitions.first().unwrap();
    // 克隆所需字段,避免借用冲突
    let effective_scope = effective_def.scope.clone();
    let effective_value_raw = effective_def.value.clone();
    // 5. 可选展开变量引用
    let effective_value = if params.expand && effective_value_raw.contains("${") {
        use crate::env::VariableExpander;
        let mut expander = VariableExpander::new(config_port.as_ref())
            .await
            .map_err(|e| {
                ApiError::new(
                    "EXPANDER_ERROR",
                    format!("Failed to create expander: {}", e),
                )
            })?;
        expander
            .expand(&effective_value_raw, &effective_scope)
            .await
            .ok()
            .unwrap_or_else(|| effective_value_raw.clone())
    } else {
        effective_value_raw.clone()
    };

    // 6. 构造响应
    let now = chrono::Utc::now();
    let detail = EnvVarDetail {
        key: key.clone(),
        effective_value,
        effective_scope,
        definitions,
        has_references: effective_value_raw.contains("${"),
        created_at: now,
        updated_at: now,
    };

    Ok(Json(detail))
}

async fn set_env_var(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(req): Json<SetEnvVarRequest>,
) -> Result<Json<EnvVar>, ApiError> {
    let key = key.trim();
    if key.is_empty() {
        return Err(ApiError::bad_request("Variable key cannot be empty"));
    }

    let scope = parse_scope(&req.scope).map_err(|e| ApiError::bad_request(&e))?;
    state
        .config_port
        .set_env_var(key, &req.value, &scope)
        .await
        .map_err(|e| ApiError::internal_error(format!("Failed to set env var: {}", e)))?;
    let mut git = state.git_versioning.lock().await;
    git.auto_stage()
        .map_err(|e| ApiError::internal_error(format!("Git stage failed: {}", e)))?;
    let commit_msg = format!("env: set {} in {}", key, req.scope);
    git.commit(&commit_msg, None)
        .map_err(|e| ApiError::internal_error(format!("Git commit failed: {}", e)))?;
    // 4. 构建响应 (使用 EnvScope 枚举)
    let env_var = EnvVar {
        key: key.to_string(),
        value: req.value.clone(),
        scope: scope.clone(), // 使用 EnvScope 枚举而不是 String
        source_file: format!("{}/config.toml", state.config_dir.display()),
        has_references: req.value.contains("${"),
        expanded_value: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    Ok(Json(env_var))
}

async fn delete_env_var(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Query(query): Query<DeleteEnvVarQuery>,
) -> Result<StatusCode, ApiError> {
    // 1. 解析 scope
    let scope = parse_scope(&query.scope).map_err(|e| ApiError::bad_request(&e))?;

    // 2. 通过 ConfigPort 删除变量
    state
        .config_port
        .delete_env_var(&key, &scope)
        .await
        .map_err(|e| ApiError::internal_error(format!("Failed to delete env var: {}", e)))?;

    // 3. Git 自动提交
    let mut git = state.git_versioning.lock().await;
    git.auto_stage()
        .map_err(|e| ApiError::internal_error(format!("Git stage failed: {}", e)))?;
    let commit_msg = format!("env: delete {} in {}", key, query.scope);
    git.commit(&commit_msg, None)
        .map_err(|e| ApiError::internal_error(format!("Git commit failed: {}", e)))?;

    Ok(StatusCode::NO_CONTENT)
}

// ============================================================================
// 辅助函数
// ============================================================================

/// 根据作用域获取配置文件路径
fn get_source_file(scope: &EnvScope) -> String {
    match scope {
        EnvScope::Global | EnvScope::Task { .. } => "~/.config/mise/config.toml".to_string(),
        EnvScope::Service { .. } => "~/.config/mise/svcmgr/config.toml".to_string(),
    }
}

/// 解析 .env 文件内容
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

// ============================================================================
// Batch 操作 Handlers
// ============================================================================

/// POST /api/v1/env/batch - 批量设置/删除环境变量
async fn batch_env_operations(
    State(state): State<AppState>,
    Json(req): Json<EnvBatchRequest>,
) -> Result<Json<ApiResponse<EnvBatchResult>>, ApiError> {
    // 1. 验证请求不为空
    if req.set.is_empty() && req.delete.is_empty() {
        return Err(ApiError::new(
            "BAD_REQUEST",
            "Empty batch request: must provide at least one set or delete operation",
        ));
    }
    // 2. 检查冲突(同一变量同时 set 和 delete)
    for set_item in &req.set {
        for del_item in &req.delete {
            if set_item.key == del_item.key && set_item.scope == del_item.scope {
                return Err(ApiError::new(
                    "CONFLICT",
                    format!(
                        "Variable '{}' in scope '{}' appears in both set and delete operations",
                        set_item.key, set_item.scope
                    ),
                ));
            }
        }
    }
    // 3. 执行所有 set 操作
    let mut set_success_count = 0;
    let mut delete_success_count = 0;

    for set_item in &req.set {
        let scope = match parse_scope(&set_item.scope) {
            Ok(s) => s,
            Err(e) => {
                // 失败时直接返回错误(原子性)
                return Err(ApiError::bad_request(format!(
                    "Invalid scope '{}': {}",
                    set_item.scope, e
                )));
            }
        };

        state
            .config_port
            .set_env_var(&set_item.key, &set_item.value, &scope)
            .await
            .map_err(|e| {
                ApiError::internal_error(format!(
                    "Failed to set variable '{}': {}",
                    set_item.key, e
                ))
            })?;

        set_success_count += 1;
    }
    // 4. 执行所有 delete 操作
    for del_item in &req.delete {
        let scope = match parse_scope(&del_item.scope) {
            Ok(s) => s,
            Err(e) => {
                return Err(ApiError::bad_request(format!(
                    "Invalid scope '{}': {}",
                    del_item.scope, e
                )));
            }
        };

        state
            .config_port
            .delete_env_var(&del_item.key, &scope)
            .await
            .map_err(|e| {
                ApiError::internal_error(format!(
                    "Failed to delete variable '{}': {}",
                    del_item.key, e
                ))
            })?;

        delete_success_count += 1;
    }
    // 5. 如果有任何成功操作,进行 Git 提交
    let commit_sha = if set_success_count + delete_success_count > 0 {
        let mut git = state.git_versioning.lock().await;
        git.auto_stage()
            .map_err(|e| ApiError::internal_error(format!("Git stage failed: {}", e)))?;
        let commit_msg = format!(
            "env: batch operation ({} set, {} delete)",
            set_success_count, delete_success_count
        );
        let oid = git
            .commit(&commit_msg, None)
            .map_err(|e| ApiError::internal_error(format!("Git commit failed: {}", e)))?;
        Some(oid.to_string())
    } else {
        None
    };
    // 6. 返回结果
    let result = EnvBatchResult {
        set_count: set_success_count,
        delete_count: delete_success_count,
        affected_files: vec!["config.toml".to_string()],
        commit_sha,
    };

    Ok(Json(ApiResponse::new(result)))
}

// ============================================================================
// Import 操作 Handler
// ============================================================================

/// POST /api/v1/env/import - 从 .env 文件导入环境变量
async fn import_env_file(
    State(state): State<AppState>,
    Json(req): Json<EnvImportRequest>,
) -> Result<Json<ApiResponse<EnvImportResult>>, ApiError> {
    // 1. 解码 Base64
    let content_bytes = general_purpose::STANDARD
        .decode(&req.content)
        .map_err(|e| ApiError::new("BAD_REQUEST", format!("Invalid Base64 content: {}", e)))?;
    let content = String::from_utf8(content_bytes)
        .map_err(|e| ApiError::new("BAD_REQUEST", format!("Invalid UTF-8 content: {}", e)))?;
    // 2. 解析 .env 文件
    let vars = parse_env_file(&content).map_err(|e| ApiError::new("BAD_REQUEST", &e))?;

    if vars.is_empty() {
        return Err(ApiError::new(
            "BAD_REQUEST",
            "No valid environment variables found in .env file",
        ));
    }
    // 3. 解析目标 scope
    let scope = parse_scope(&req.scope).map_err(|e| ApiError::bad_request(&e))?;
    let mut imported_count = 0;
    let mut skipped_count = 0;
    let mut details: Vec<EnvImportItemResult> = Vec::new();
    for (key, value) in vars {
        // TODO: 处理 conflict_strategy (Skip/Overwrite/Abort)
        // 当前实现: 总是覆盖
        match state.config_port.set_env_var(&key, &value, &scope).await {
            Ok(_) => {
                imported_count += 1;
                details.push(EnvImportItemResult {
                    key: key.clone(),
                    status: EnvImportStatus::Imported,
                });
            }
            Err(_e) => {
                skipped_count += 1;
                details.push(EnvImportItemResult {
                    key: key.clone(),
                    status: EnvImportStatus::Failed,
                });
            }
        }
    }
    // 5. 如果有变量导入成功,进行 Git 提交
    let commit_sha = if imported_count > 0 {
        let mut git = state.git_versioning.lock().await;
        git.auto_stage()
            .map_err(|e| ApiError::internal_error(format!("Git stage failed: {}", e)))?;
        let commit_msg = format!("env: import {} variables to {}", imported_count, req.scope);
        let oid = git
            .commit(&commit_msg, None)
            .map_err(|e| ApiError::internal_error(format!("Git commit failed: {}", e)))?;
        Some(oid.to_string())
    } else {
        None
    };
    // 6. 返回结果
    let result = EnvImportResult {
        imported_count,
        skipped_count,
        failed_count: skipped_count, // 当前 skipped == failed
        details,
        commit_sha,
    };

    Ok(Json(ApiResponse::new(result)))
}

// ============================================================================
// Export 操作 Handler
// ============================================================================

/// GET /api/v1/env/export - 导出环境变量为 .env 文件
async fn export_env_file(
    State(state): State<AppState>,
    Query(params): Query<EnvExportParams>,
) -> Result<([(axum::http::HeaderName, &'static str); 2], String), ApiError> {
    let config_port = &state.config_port;

    // 1. 获取所有作用域的环境变量
    let global_env = config_port
        .get_global_env()
        .await
        .map_err(|e| ApiError::new("CONFIG_ERROR", format!("Failed to get global env: {}", e)))?;

    let service_envs = config_port
        .get_service_envs()
        .await
        .map_err(|e| ApiError::new("CONFIG_ERROR", format!("Failed to get service envs: {}", e)))?;

    let task_envs = config_port
        .get_task_envs()
        .await
        .map_err(|e| ApiError::new("CONFIG_ERROR", format!("Failed to get task envs: {}", e)))?;

    // 2. 收集所有环境变量到统一结构
    let mut all_vars = Vec::new();

    // Global scope
    for (key, value) in global_env {
        all_vars.push((key, value, EnvScope::Global));
    }

    // Service scopes
    for (service_name, env) in service_envs {
        for (key, value) in env {
            all_vars.push((
                key,
                value,
                EnvScope::Service {
                    name: service_name.clone(),
                },
            ));
        }
    }

    // Task scopes
    for (task_name, env) in task_envs {
        for (key, value) in env {
            all_vars.push((
                key,
                value,
                EnvScope::Task {
                    name: task_name.clone(),
                },
            ));
        }
    }

    // 3. 按 scopes 参数过滤
    if !params.scopes.is_empty() {
        let scope_filters: Result<Vec<EnvScope>, String> =
            params.scopes.iter().map(|s| parse_scope(s)).collect();

        let scope_filters = scope_filters.map_err(|e| ApiError::new("INVALID_SCOPE", &e))?;

        all_vars.retain(|(_, _, scope)| {
            scope_filters.iter().any(|filter| match (filter, scope) {
                (EnvScope::Global, EnvScope::Global) => true,
                (EnvScope::Service { name: f_name }, EnvScope::Service { name: s_name }) => {
                    f_name == s_name
                }
                (EnvScope::Task { name: f_name }, EnvScope::Task { name: s_name }) => {
                    f_name == s_name
                }
                _ => false,
            })
        });
    }

    // 4. 生成 .env 格式
    let mut output = String::new();

    // 添加文件头注释
    if params.include_comments {
        output.push_str("# Exported environment variables\n");
        output.push_str(&format!(
            "# Generated at: {}\n\n",
            chrono::Utc::now().to_rfc3339()
        ));
    }

    // 生成变量定义
    for (key, value, scope) in &all_vars {
        // 添加 scope 注释
        if params.include_comments {
            let scope_str = match scope {
                EnvScope::Global => "global".to_string(),
                EnvScope::Service { name } => format!("service:{}", name),
                EnvScope::Task { name } => format!("task:{}", name),
            };
            output.push_str(&format!("# Scope: {}\n", scope_str));
        }

        // 可选展开变量引用
        let final_value = if params.expand && value.contains("${") {
            use crate::env::VariableExpander;
            let mut expander = VariableExpander::new(config_port.as_ref())
                .await
                .map_err(|e| {
                    ApiError::new(
                        "EXPANDER_ERROR",
                        format!("Failed to create expander: {}", e),
                    )
                })?;
            expander
                .expand(value, scope)
                .await
                .unwrap_or_else(|_| value.clone())
        } else {
            value.clone()
        };

        output.push_str(&format!("{}={}\n", key, final_value));

        // 在变量之间添加空行(仅当包含注释时)
        if params.include_comments {
            output.push('\n');
        }
    }

    // 5. 返回响应
    use axum::http::HeaderName;
    Ok((
        [
            (HeaderName::from_static("content-type"), "text/plain"),
            (
                HeaderName::from_static("content-disposition"),
                "attachment; filename=\"env.txt\"",
            ),
        ],
        output,
    ))
}
