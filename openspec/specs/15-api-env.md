# 15. Environment Variable Management API

## Design Goal

提供完整的环境变量管理 API，支持分层作用域（全局/服务/任务）、变量展开、批量操作、导入导出，并与 Git 版本控制深度集成。环境变量的每次修改都会自动暂存、提交到相应的配置文件（`mise.toml` 或 `svcmgr.toml`），确保配置变更可追溯、可回滚。

---

## Why This Design?

### 问题分析

1. **作用域混乱**：传统方案中环境变量往往只有全局作用域，导致服务或任务的特定配置需要通过前缀、命名约定等 workaround 实现，容易出错且难以维护
2. **变更追踪困难**：环境变量修改通常没有版本控制，无法回滚到之前的配置状态，故障排查困难
3. **批量操作缺失**：需要一次性修改多个相关变量时，缺乏原子性保证，可能导致配置不一致
4. **变量引用受限**：缺少变量展开机制，导致重复定义、配置冗余
5. **配置迁移困难**：.env 文件与配置系统之间的互操作性差，迁移现有配置费时费力

### 解决方案

1. **分层作用域系统**：
   - **全局作用域**：`.config/mise/config.toml` 的 `[env]` 段，适用于所有服务和任务
   - **服务作用域**：`.config/mise/svcmgr/config.toml` 的 `[services.<name>.env]` 段，覆盖全局配置
   - **任务作用域**：`.config/mise/config.toml` 的 `[tasks.<name>.env]` 段，覆盖全局配置
   - 优先级：任务 > 服务 > 全局

2. **Git 自动版本控制**：
   - 所有环境变量修改自动暂存到对应的配置文件
   - 每次修改触发 Git 提交，提交信息包含变量名、作用域、操作类型
   - 支持通过 Git 历史回滚到任意配置状态

3. **批量操作原子性**：
   - 批量设置/删除多个变量时，要么全部成功，要么全部失败
   - 失败时自动回滚文件系统和 Git 状态

4. **变量展开和验证**：
   - 支持 `${VAR_NAME}` 语法引用其他变量
   - 自动检测循环引用（A → B → A）
   - 展开时按作用域优先级解析引用

5. **导入导出互操作性**：
   - 从 .env 文件导入变量到指定作用域
   - 导出指定作用域的变量为 .env 格式
   - 支持过滤、合并、冲突检测

---

## Data Models

### EnvVar

```rust
/// 环境变量实体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVar {
    /// 变量名
    pub key: String,
    
    /// 变量值（原始值，未展开）
    pub value: String,
    
    /// 作用域
    pub scope: EnvScope,
    
    /// 源配置文件路径
    pub source_file: PathBuf,
    
    /// 是否包含变量引用（${...}）
    pub has_references: bool,
    
    /// 展开后的值（如果 has_references 为 true）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expanded_value: Option<String>,
    
    /// 创建时间
    pub created_at: DateTime<Utc>,
    
    /// 最后修改时间
    pub updated_at: DateTime<Utc>,
}
```

### EnvScope

```rust
/// 环境变量作用域
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum EnvScope {
    /// 全局作用域（mise.toml [env]）
    Global,
    
    /// 服务作用域（svcmgr.toml [services.<name>.env]）
    Service { 
        name: String 
    },
    
    /// 任务作用域（mise.toml [tasks.<name>.env]）
    Task { 
        name: String 
    },
}

impl EnvScope {
    /// 作用域优先级（数字越大优先级越高）
    pub fn priority(&self) -> u8 {
        match self {
            EnvScope::Global => 1,
            EnvScope::Service { .. } => 2,
            EnvScope::Task { .. } => 3,
        }
    }
    
    /// 获取配置文件路径
    pub fn config_file(&self, config_dir: &Path) -> PathBuf {
        match self {
            EnvScope::Global | EnvScope::Task { .. } => {
                config_dir.join("mise/config.toml")
            }
            EnvScope::Service { .. } => {
                config_dir.join("mise/svcmgr/config.toml")
            }
        }
    }
}
```

### EnvVarRequest

```rust
/// 设置环境变量请求
#[derive(Debug, Deserialize, Validate)]
pub struct EnvVarRequest {
    /// 变量值
    #[validate(length(max = 10240))]
    pub value: String,
    
    /// 作用域（默认为 Global）
    #[serde(default)]
    pub scope: EnvScope,
}
```

### EnvBatchRequest

```rust
/// 批量操作请求
#[derive(Debug, Deserialize, Validate)]
pub struct EnvBatchRequest {
    /// 要设置的变量列表
    #[serde(default)]
    #[validate(length(max = 100))]
    pub set: Vec<EnvBatchSetItem>,
    
    /// 要删除的变量列表
    #[serde(default)]
    #[validate(length(max = 100))]
    pub delete: Vec<EnvBatchDeleteItem>,
}

#[derive(Debug, Deserialize)]
pub struct EnvBatchSetItem {
    pub key: String,
    pub value: String,
    pub scope: EnvScope,
}

#[derive(Debug, Deserialize)]
pub struct EnvBatchDeleteItem {
    pub key: String,
    pub scope: EnvScope,
}
```

### EnvImportRequest

```rust
/// 导入 .env 文件请求
#[derive(Debug, Deserialize, Validate)]
pub struct EnvImportRequest {
    /// .env 文件内容（Base64 编码）
    #[validate(length(max = 1048576))] // 1MB
    pub content: String,
    
    /// 目标作用域
    pub scope: EnvScope,
    
    /// 冲突策略
    #[serde(default)]
    pub conflict_strategy: ConflictStrategy,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConflictStrategy {
    /// 跳过已存在的变量
    #[default]
    Skip,
    
    /// 覆盖已存在的变量
    Overwrite,
    
    /// 遇到冲突时中止
    Abort,
}
```

### EnvExportOptions

```rust
/// 导出选项（查询参数）
#[derive(Debug, Deserialize)]
pub struct EnvExportOptions {
    /// 作用域过滤器（可选，多个）
    #[serde(default)]
    pub scopes: Vec<EnvScope>,
    
    /// 是否包含注释（说明变量来源）
    #[serde(default = "default_true")]
    pub include_comments: bool,
    
    /// 是否展开变量引用
    #[serde(default)]
    pub expand: bool,
}

fn default_true() -> bool { true }
```

---

## API Endpoints

### 1. List Environment Variables

列出所有环境变量，支持按作用域、前缀过滤。

**Endpoint**: `GET /api/v1/env`

**Query Parameters**:
- `scope` (optional, repeatable): 作用域过滤器，支持 `global`, `service:<name>`, `task:<name>`
- `prefix` (optional): 变量名前缀过滤器
- `search` (optional): 搜索关键词（匹配变量名或值）
- `expand` (optional, boolean, default=false): 是否展开变量引用
- `page` (optional, default=1): 页码
- `per_page` (optional, default=50, max=200): 每页数量

**Response** (200 OK):
```json
{
  "data": [
    {
      "key": "DATABASE_URL",
      "value": "postgresql://localhost:5432/mydb",
      "scope": {
        "type": "global"
      },
      "source_file": "/home/user/.config/mise/config.toml",
      "has_references": false,
      "created_at": "2026-01-15T10:00:00Z",
      "updated_at": "2026-01-20T14:30:00Z"
    },
    {
      "key": "REDIS_URL",
      "value": "redis://${REDIS_HOST}:6379",
      "scope": {
        "type": "service",
        "name": "api"
      },
      "source_file": "/home/user/.config/mise/svcmgr/config.toml",
      "has_references": true,
      "expanded_value": "redis://localhost:6379",
      "created_at": "2026-01-16T11:00:00Z",
      "updated_at": "2026-01-16T11:00:00Z"
    }
  ],
  "pagination": {
    "page": 1,
    "per_page": 50,
    "total": 127,
    "total_pages": 3
  }
}
```

**Errors**:
- `400 BAD_REQUEST`: 无效的作用域格式
- `500 INTERNAL_ERROR`: 配置文件读取失败

**Handler Implementation**:
```rust
pub async fn list_env_vars(
    State(state): State<AppState>,
    Query(params): Query<ListEnvVarsParams>,
) -> Result<Json<ListResponse<EnvVar>>> {
    // 1. 从 mise.toml 和 svcmgr.toml 读取所有环境变量
    let global_vars = state.config_port.get_global_env().await?;
    let service_vars = state.config_port.get_service_envs().await?;
    let task_vars = state.config_port.get_task_envs().await?;
    
    // 2. 合并所有变量，标记作用域
    let mut all_vars = Vec::new();
    for (key, value) in global_vars {
        all_vars.push(EnvVar {
            key,
            value: value.clone(),
            scope: EnvScope::Global,
            source_file: state.config_dir.join("mise/config.toml"),
            has_references: value.contains("${"),
            expanded_value: None,
            created_at: Utc::now(), // 实际应从 Git 历史读取
            updated_at: Utc::now(),
        });
    }
    // ... 添加服务和任务作用域变量 ...
    
    // 3. 应用过滤器
    if let Some(scopes) = &params.scopes {
        all_vars.retain(|v| scopes.contains(&v.scope));
    }
    if let Some(prefix) = &params.prefix {
        all_vars.retain(|v| v.key.starts_with(prefix));
    }
    if let Some(search) = &params.search {
        let search_lower = search.to_lowercase();
        all_vars.retain(|v| {
            v.key.to_lowercase().contains(&search_lower) 
            || v.value.to_lowercase().contains(&search_lower)
        });
    }
    
    // 4. 如果需要展开变量引用
    if params.expand {
        let expander = VariableExpander::new(&all_vars);
        for var in &mut all_vars {
            if var.has_references {
                var.expanded_value = Some(expander.expand(&var.value, &var.scope)?);
            }
        }
    }
    
    // 5. 分页
    let total = all_vars.len();
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(50).min(200);
    let start = ((page - 1) * per_page) as usize;
    let end = (start + per_page as usize).min(total);
    let items = all_vars[start..end].to_vec();
    
    Ok(Json(ListResponse {
        data: items,
        pagination: Pagination {
            page,
            per_page,
            total: total as u64,
            total_pages: ((total as f64) / (per_page as f64)).ceil() as u64,
        },
    }))
}
```

---

### 2. Get Single Environment Variable

获取单个环境变量的详细信息，包括所有作用域的定义（如果存在）和优先级解析结果。

**Endpoint**: `GET /api/v1/env/{key}`

**Query Parameters**:
- `expand` (optional, boolean, default=false): 是否展开变量引用

**Response** (200 OK):
```json
{
  "data": {
    "key": "DATABASE_URL",
    "effective_value": "postgresql://localhost:5432/mydb",
    "effective_scope": {
      "type": "global"
    },
    "definitions": [
      {
        "scope": {
          "type": "global"
        },
        "value": "postgresql://localhost:5432/mydb",
        "source_file": "/home/user/.config/mise/config.toml",
        "priority": 1
      }
    ],
    "has_references": false,
    "created_at": "2026-01-15T10:00:00Z",
    "updated_at": "2026-01-20T14:30:00Z"
  }
}
```

**Example with Multiple Scopes**:
```json
{
  "data": {
    "key": "LOG_LEVEL",
    "effective_value": "debug",
    "effective_scope": {
      "type": "service",
      "name": "api"
    },
    "definitions": [
      {
        "scope": {
          "type": "global"
        },
        "value": "info",
        "source_file": "/home/user/.config/mise/config.toml",
        "priority": 1
      },
      {
        "scope": {
          "type": "service",
          "name": "api"
        },
        "value": "debug",
        "source_file": "/home/user/.config/mise/svcmgr/config.toml",
        "priority": 2
      }
    ],
    "has_references": false,
    "created_at": "2026-01-15T10:00:00Z",
    "updated_at": "2026-01-22T16:45:00Z"
  }
}
```

**Errors**:
- `404 NOT_FOUND`: 变量不存在
- `500 INTERNAL_ERROR`: 配置文件读取失败

**Handler Implementation**:
```rust
pub async fn get_env_var(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Query(params): Query<GetEnvVarParams>,
) -> Result<Json<SingleResponse<EnvVarDetail>>> {
    // 1. 从所有作用域收集该变量的定义
    let mut definitions = Vec::new();
    
    // 全局作用域
    if let Some(value) = state.config_port.get_global_env_var(&key).await? {
        definitions.push(EnvVarDefinition {
            scope: EnvScope::Global,
            value: value.clone(),
            source_file: state.config_dir.join("mise/config.toml"),
            priority: EnvScope::Global.priority(),
        });
    }
    
    // 服务作用域
    for service_name in state.service_manager.list_services().await? {
        if let Some(value) = state.config_port
            .get_service_env_var(&service_name, &key).await? 
        {
            definitions.push(EnvVarDefinition {
                scope: EnvScope::Service { name: service_name },
                value: value.clone(),
                source_file: state.config_dir.join("mise/svcmgr/config.toml"),
                priority: EnvScope::Service { name: String::new() }.priority(),
            });
        }
    }
    
    // 任务作用域
    for task_name in state.task_manager.list_tasks().await? {
        if let Some(value) = state.config_port
            .get_task_env_var(&task_name, &key).await? 
        {
            definitions.push(EnvVarDefinition {
                scope: EnvScope::Task { name: task_name },
                value: value.clone(),
                source_file: state.config_dir.join("mise/config.toml"),
                priority: EnvScope::Task { name: String::new() }.priority(),
            });
        }
    }
    
    // 2. 如果没有任何定义，返回 404
    if definitions.is_empty() {
        return Err(Error::NotFound {
            resource: "env_var",
            id: key,
        });
    }
    
    // 3. 按优先级排序，最高优先级作为有效值
    definitions.sort_by_key(|d| std::cmp::Reverse(d.priority));
    let effective_def = definitions.first().unwrap();
    
    // 4. 展开变量引用（如果需要）
    let effective_value = if params.expand && effective_def.value.contains("${") {
        let expander = VariableExpander::new(&state.config_port).await?;
        expander.expand(&effective_def.value, &effective_def.scope)?
    } else {
        effective_def.value.clone()
    };
    
    Ok(Json(SingleResponse {
        data: EnvVarDetail {
            key,
            effective_value,
            effective_scope: effective_def.scope.clone(),
            definitions,
            has_references: effective_def.value.contains("${"),
            created_at: Utc::now(), // 从 Git 历史读取
            updated_at: Utc::now(),
        },
    }))
}
```

---

### 3. Set Environment Variable

设置或更新单个环境变量。

**Endpoint**: `PUT /api/v1/env/{key}`

**Request Headers**:
- `Idempotency-Key` (optional): 幂等性令牌（24小时有效）

**Request Body**:
```json
{
  "value": "postgresql://db.example.com:5432/prod",
  "scope": {
    "type": "global"
  }
}
```

**Example - Service Scope**:
```json
{
  "value": "debug",
  "scope": {
    "type": "service",
    "name": "api"
  }
}
```

**Response** (200 OK):
```json
{
  "data": {
    "key": "DATABASE_URL",
    "value": "postgresql://db.example.com:5432/prod",
    "scope": {
      "type": "global"
    },
    "source_file": "/home/user/.config/mise/config.toml",
    "has_references": false,
    "created_at": "2026-01-15T10:00:00Z",
    "updated_at": "2026-02-23T11:04:45Z"
  }
}
```

**Errors**:
- `400 BAD_REQUEST`: 
  - 变量名包含非法字符
  - 变量值超过最大长度（10KB）
  - 作用域引用的服务/任务不存在
  - 变量引用存在循环依赖
- `409 CONFLICT`: Git 提交冲突（需要先拉取远程变更）
- `500 INTERNAL_ERROR`: 配置文件写入失败

**Side Effects**:
- 更新对应的配置文件（`mise.toml` 或 `svcmgr.toml`）
- 自动 `git add` 配置文件
- 自动 `git commit` 变更，提交信息格式：`env: set ${key} in ${scope}`
- 发布 `EnvChanged` 事件
- 如果变量被运行中的服务使用，记录警告日志（需要重启服务生效）

**Handler Implementation**:
```rust
pub async fn set_env_var(
    State(state): State<AppState>,
    Path(key): Path<String>,
    TypedHeader(idempotency_key): TypedHeader<IdempotencyKey>,
    Json(request): Json<EnvVarRequest>,
) -> Result<Json<SingleResponse<EnvVar>>> {
    // 1. 幂等性检查
    if let Some(cached) = state.idempotency_cache
        .get(&idempotency_key).await? 
    {
        return Ok(Json(cached));
    }
    
    // 2. 验证变量名格式（字母、数字、下划线）
    if !key.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(Error::BadRequest {
            message: "Invalid environment variable name".into(),
        });
    }
    
    // 3. 验证作用域引用的服务/任务是否存在
    match &request.scope {
        EnvScope::Service { name } => {
            if !state.service_manager.service_exists(name).await? {
                return Err(Error::BadRequest {
                    message: format!("Service '{}' does not exist", name),
                });
            }
        }
        EnvScope::Task { name } => {
            if !state.task_manager.task_exists(name).await? {
                return Err(Error::BadRequest {
                    message: format!("Task '{}' does not exist", name),
                });
            }
        }
        EnvScope::Global => {}
    }
    
    // 4. 检测循环引用（如果值包含 ${...}）
    if request.value.contains("${") {
        let expander = VariableExpander::new(&state.config_port).await?;
        expander.check_circular_reference(&key, &request.value, &request.scope)?;
    }
    
    // 5. 更新配置文件
    let config_file = request.scope.config_file(&state.config_dir);
    state.config_port.set_env_var(&key, &request.value, &request.scope).await?;
    
    // 6. Git 版本控制
    state.git_service.stage_file(&config_file).await?;
    let commit_msg = format!(
        "env: set {} in {}",
        key,
        match &request.scope {
            EnvScope::Global => "global".to_string(),
            EnvScope::Service { name } => format!("service:{}", name),
            EnvScope::Task { name } => format!("task:{}", name),
        }
    );
    state.git_service.commit(&commit_msg).await?;
    
    // 7. 发布事件
    state.event_bus.publish(Event::EnvChanged {
        key: key.clone(),
        old_value: None, // 从 Git 历史读取旧值
        new_value: request.value.clone(),
        scope: request.scope.clone(),
        timestamp: Utc::now(),
    }).await?;
    
    // 8. 构造响应
    let result = EnvVar {
        key,
        value: request.value,
        scope: request.scope,
        source_file: config_file,
        has_references: request.value.contains("${"),
        expanded_value: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    
    // 9. 缓存幂等性结果
    let response = SingleResponse { data: result.clone() };
    state.idempotency_cache.set(&idempotency_key, &response, Duration::hours(24)).await?;
    
    Ok(Json(response))
}
```

---

### 4. Delete Environment Variable

删除指定作用域的环境变量。

**Endpoint**: `DELETE /api/v1/env/{key}`

**Query Parameters**:
- `scope` (required): 要删除的作用域，格式：`global`, `service:<name>`, `task:<name>`

**Response** (204 NO CONTENT)

**Errors**:
- `400 BAD_REQUEST`: 作用域格式无效或缺失
- `404 NOT_FOUND`: 变量在指定作用域中不存在
- `409 CONFLICT`: Git 提交冲突
- `500 INTERNAL_ERROR`: 配置文件写入失败

**Side Effects**:
- 从对应配置文件中删除变量定义
- 自动 `git add` 配置文件
- 自动 `git commit` 变更，提交信息格式：`env: delete ${key} from ${scope}`
- 发布 `EnvChanged` 事件（new_value 为 None）

**Handler Implementation**:
```rust
pub async fn delete_env_var(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Query(params): Query<DeleteEnvVarParams>,
) -> Result<StatusCode> {
    // 1. 解析作用域参数
    let scope = parse_scope_param(&params.scope)?;
    
    // 2. 检查变量是否存在
    let old_value = match &scope {
        EnvScope::Global => {
            state.config_port.get_global_env_var(&key).await?
        }
        EnvScope::Service { name } => {
            state.config_port.get_service_env_var(name, &key).await?
        }
        EnvScope::Task { name } => {
            state.config_port.get_task_env_var(name, &key).await?
        }
    };
    
    if old_value.is_none() {
        return Err(Error::NotFound {
            resource: "env_var",
            id: format!("{}@{}", key, params.scope),
        });
    }
    
    // 3. 删除变量
    let config_file = scope.config_file(&state.config_dir);
    state.config_port.delete_env_var(&key, &scope).await?;
    
    // 4. Git 版本控制
    state.git_service.stage_file(&config_file).await?;
    let commit_msg = format!(
        "env: delete {} from {}",
        key,
        params.scope
    );
    state.git_service.commit(&commit_msg).await?;
    
    // 5. 发布事件
    state.event_bus.publish(Event::EnvChanged {
        key: key.clone(),
        old_value,
        new_value: None,
        scope: scope.clone(),
        timestamp: Utc::now(),
    }).await?;
    
    Ok(StatusCode::NO_CONTENT)
}

fn parse_scope_param(scope_str: &str) -> Result<EnvScope> {
    if scope_str == "global" {
        return Ok(EnvScope::Global);
    }
    
    if let Some(name) = scope_str.strip_prefix("service:") {
        return Ok(EnvScope::Service { name: name.to_string() });
    }
    
    if let Some(name) = scope_str.strip_prefix("task:") {
        return Ok(EnvScope::Task { name: name.to_string() });
    }
    
    Err(Error::BadRequest {
        message: format!("Invalid scope format: '{}'", scope_str),
    })
}
```

---

### 5. Batch Operations

批量设置/删除多个环境变量，保证原子性（全部成功或全部失败）。

**Endpoint**: `POST /api/v1/env/batch`

**Request Headers**:
- `Idempotency-Key` (optional): 幂等性令牌

**Request Body**:
```json
{
  "set": [
    {
      "key": "DATABASE_URL",
      "value": "postgresql://localhost:5432/prod",
      "scope": {
        "type": "global"
      }
    },
    {
      "key": "REDIS_URL",
      "value": "redis://localhost:6379",
      "scope": {
        "type": "service",
        "name": "api"
      }
    }
  ],
  "delete": [
    {
      "key": "DEPRECATED_VAR",
      "scope": {
        "type": "global"
      }
    }
  ]
}
```

**Response** (200 OK):
```json
{
  "data": {
    "set_count": 2,
    "delete_count": 1,
    "affected_files": [
      "/home/user/.config/mise/config.toml",
      "/home/user/.config/mise/svcmgr/config.toml"
    ],
    "commit_sha": "a1b2c3d4e5f6"
  }
}
```

**Errors**:
- `400 BAD_REQUEST`: 
  - 请求体为空（set 和 delete 都为空）
  - 操作数量超过限制（单次最多 100 个）
  - 同一个变量在 set 和 delete 中都出现
  - 存在循环引用
- `409 CONFLICT`: Git 提交冲突
- `500 INTERNAL_ERROR`: 部分操作失败，已回滚所有变更

**Side Effects**:
- 更新涉及的所有配置文件
- 单次 Git 提交包含所有变更
- 批量发布 `EnvChanged` 事件

**Handler Implementation**:
```rust
pub async fn batch_env_operations(
    State(state): State<AppState>,
    TypedHeader(idempotency_key): TypedHeader<IdempotencyKey>,
    Json(request): Json<EnvBatchRequest>,
) -> Result<Json<SingleResponse<EnvBatchResult>>> {
    // 1. 幂等性检查
    if let Some(cached) = state.idempotency_cache
        .get(&idempotency_key).await? 
    {
        return Ok(Json(cached));
    }
    
    // 2. 验证请求
    if request.set.is_empty() && request.delete.is_empty() {
        return Err(Error::BadRequest {
            message: "Empty batch request".into(),
        });
    }
    
    // 检查冲突（同一变量同时 set 和 delete）
    let set_keys: HashSet<_> = request.set.iter()
        .map(|item| (&item.key, &item.scope))
        .collect();
    let delete_keys: HashSet<_> = request.delete.iter()
        .map(|item| (&item.key, &item.scope))
        .collect();
    let conflicts: Vec<_> = set_keys.intersection(&delete_keys).collect();
    if !conflicts.is_empty() {
        return Err(Error::BadRequest {
            message: format!("Conflicting operations: {:?}", conflicts),
        });
    }
    
    // 3. 创建事务（通过 Git stash 实现回滚能力）
    state.git_service.stash_push("batch_env_operations").await?;
    
    let result = async {
        let mut affected_files = HashSet::new();
        
        // 4. 执行所有 set 操作
        for item in &request.set {
            // 验证和写入（复用 set_env_var 的逻辑）
            validate_env_var(&item.key, &item.value, &item.scope, &state).await?;
            
            let config_file = item.scope.config_file(&state.config_dir);
            state.config_port.set_env_var(&item.key, &item.value, &item.scope).await?;
            affected_files.insert(config_file);
        }
        
        // 5. 执行所有 delete 操作
        for item in &request.delete {
            let config_file = item.scope.config_file(&state.config_dir);
            state.config_port.delete_env_var(&item.key, &item.scope).await?;
            affected_files.insert(config_file);
        }
        
        // 6. Git 提交（单次提交包含所有变更）
        for file in &affected_files {
            state.git_service.stage_file(file).await?;
        }
        let commit_msg = format!(
            "env: batch operation (set={}, delete={})",
            request.set.len(),
            request.delete.len()
        );
        let commit_sha = state.git_service.commit(&commit_msg).await?;
        
        // 7. 发布事件
        for item in &request.set {
            state.event_bus.publish(Event::EnvChanged {
                key: item.key.clone(),
                old_value: None,
                new_value: Some(item.value.clone()),
                scope: item.scope.clone(),
                timestamp: Utc::now(),
            }).await?;
        }
        for item in &request.delete {
            state.event_bus.publish(Event::EnvChanged {
                key: item.key.clone(),
                old_value: None,
                new_value: None,
                scope: item.scope.clone(),
                timestamp: Utc::now(),
            }).await?;
        }
        
        Ok(EnvBatchResult {
            set_count: request.set.len(),
            delete_count: request.delete.len(),
            affected_files: affected_files.into_iter().collect(),
            commit_sha,
        })
    }.await;
    
    // 8. 失败时回滚
    match result {
        Ok(res) => {
            state.git_service.stash_drop().await?;
            let response = SingleResponse { data: res };
            state.idempotency_cache.set(&idempotency_key, &response, Duration::hours(24)).await?;
            Ok(Json(response))
        }
        Err(err) => {
            state.git_service.stash_pop().await?;
            Err(err)
        }
    }
}
```

---

### 6. Import from .env File

从 .env 文件导入环境变量到指定作用域。

**Endpoint**: `POST /api/v1/env/import`

**Request Body**:
```json
{
  "content": "REFUQUJBU0VfVVJMPXBvc3RncmVzcWw6Ly9sb2NhbGhvc3Q6NTQzMi9kYgpSRURJU19VUkw9cmVkaXM6Ly9sb2NhbGhvc3Q6NjM3OQ==",
  "scope": {
    "type": "global"
  },
  "conflict_strategy": "skip"
}
```

**content field**: Base64 编码的 .env 文件内容，解码后示例：
```env
DATABASE_URL=postgresql://localhost:5432/db
REDIS_URL=redis://localhost:6379
LOG_LEVEL=info
# 注释会被忽略
```

**Response** (200 OK):
```json
{
  "data": {
    "imported_count": 3,
    "skipped_count": 0,
    "failed_count": 0,
    "details": [
      {
        "key": "DATABASE_URL",
        "status": "imported"
      },
      {
        "key": "REDIS_URL",
        "status": "imported"
      },
      {
        "key": "LOG_LEVEL",
        "status": "imported"
      }
    ],
    "commit_sha": "b2c3d4e5f6g7"
  }
}
```

**Errors**:
- `400 BAD_REQUEST`: 
  - content 字段不是有效的 Base64
  - 解码后内容超过 1MB
  - .env 文件格式错误
  - conflict_strategy=abort 且存在冲突
- `409 CONFLICT`: Git 提交冲突
- `500 INTERNAL_ERROR`: 导入过程中出错

**Side Effects**:
- 批量写入环境变量到指定作用域
- 单次 Git 提交包含所有导入的变量
- 批量发布 `EnvChanged` 事件

**Handler Implementation**:
```rust
pub async fn import_env_file(
    State(state): State<AppState>,
    Json(request): Json<EnvImportRequest>,
) -> Result<Json<SingleResponse<EnvImportResult>>> {
    // 1. 解码 Base64 内容
    let content = base64::decode(&request.content)
        .map_err(|_| Error::BadRequest {
            message: "Invalid Base64 encoding".into(),
        })?;
    let content_str = String::from_utf8(content)
        .map_err(|_| Error::BadRequest {
            message: "Invalid UTF-8 encoding".into(),
        })?;
    
    // 2. 解析 .env 文件
    let vars = parse_env_file(&content_str)?;
    
    // 3. 检查冲突
    let mut import_items = Vec::new();
    let mut details = Vec::new();
    
    for (key, value) in vars {
        let exists = match &request.scope {
            EnvScope::Global => {
                state.config_port.get_global_env_var(&key).await?.is_some()
            }
            EnvScope::Service { name } => {
                state.config_port.get_service_env_var(name, &key).await?.is_some()
            }
            EnvScope::Task { name } => {
                state.config_port.get_task_env_var(name, &key).await?.is_some()
            }
        };
        
        if exists {
            match request.conflict_strategy {
                ConflictStrategy::Skip => {
                    details.push(EnvImportItemResult {
                        key,
                        status: EnvImportStatus::Skipped,
                    });
                    continue;
                }
                ConflictStrategy::Abort => {
                    return Err(Error::BadRequest {
                        message: format!("Variable '{}' already exists", key),
                    });
                }
                ConflictStrategy::Overwrite => {
                    // 继续处理
                }
            }
        }
        
        import_items.push((key.clone(), value));
        details.push(EnvImportItemResult {
            key,
            status: EnvImportStatus::Imported,
        });
    }
    
    // 4. 批量导入（复用 batch 逻辑）
    if !import_items.is_empty() {
        let batch_request = EnvBatchRequest {
            set: import_items.into_iter().map(|(key, value)| {
                EnvBatchSetItem {
                    key,
                    value,
                    scope: request.scope.clone(),
                }
            }).collect(),
            delete: vec![],
        };
        
        let batch_result = batch_env_operations(
            State(state.clone()),
            TypedHeader(IdempotencyKey::generate()),
            Json(batch_request),
        ).await?.0.data;
        
        Ok(Json(SingleResponse {
            data: EnvImportResult {
                imported_count: details.iter().filter(|d| matches!(d.status, EnvImportStatus::Imported)).count(),
                skipped_count: details.iter().filter(|d| matches!(d.status, EnvImportStatus::Skipped)).count(),
                failed_count: 0,
                details,
                commit_sha: batch_result.commit_sha,
            },
        }))
    } else {
        Ok(Json(SingleResponse {
            data: EnvImportResult {
                imported_count: 0,
                skipped_count: details.len(),
                failed_count: 0,
                details,
                commit_sha: None,
            },
        }))
    }
}

fn parse_env_file(content: &str) -> Result<Vec<(String, String)>> {
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
            return Err(Error::BadRequest {
                message: format!("Invalid .env format at line {}: '{}'", line_num + 1, line),
            });
        }
        
        let key = parts[0].trim();
        let value = parts[1].trim()
            .trim_matches('"')  // 移除可选的双引号
            .trim_matches('\''); // 移除可选的单引号
        
        vars.push((key.to_string(), value.to_string()));
    }
    Ok(vars)
}
```

---

### 7. Export to .env File

导出环境变量为 .env 文件格式。

**Endpoint**: `GET /api/v1/env/export`

**Query Parameters**:
- `scopes` (optional, repeatable): 要导出的作用域（默认导出所有）
- `include_comments` (optional, boolean, default=true): 是否包含注释说明变量来源
- `expand` (optional, boolean, default=false): 是否展开变量引用

**Response** (200 OK):
```
Content-Type: text/plain
Content-Disposition: attachment; filename="svcmgr.env"

# Global variables (from /home/user/.config/mise/config.toml)
DATABASE_URL=postgresql://localhost:5432/db
LOG_LEVEL=info

# Service: api (from /home/user/.config/mise/svcmgr/config.toml)
REDIS_URL=redis://localhost:6379
API_PORT=3000
```

**Example with expand=true**:
```
# Global variables (expanded)
DATABASE_URL=postgresql://localhost:5432/db
LOG_LEVEL=info

# Service: api (expanded)
REDIS_URL=redis://localhost:6379
API_PORT=3000
FULL_API_URL=http://localhost:3000
```

**Errors**:
- `400 BAD_REQUEST`: 无效的作用域格式
- `500 INTERNAL_ERROR`: 配置文件读取失败

**Handler Implementation**:
```rust
pub async fn export_env_file(
    State(state): State<AppState>,
    Query(options): Query<EnvExportOptions>,
) -> Result<(TypedHeader<ContentType>, TypedHeader<ContentDisposition>, String)> {
    // 1. 获取所有环境变量
    let all_vars = list_all_env_vars(&state, &options.scopes).await?;
    
    // 2. 按作用域分组
    let mut vars_by_scope: HashMap<String, Vec<EnvVar>> = HashMap::new();
    for var in all_vars {
        let scope_key = match &var.scope {
            EnvScope::Global => "Global".to_string(),
            EnvScope::Service { name } => format!("Service: {}", name),
            EnvScope::Task { name } => format!("Task: {}", name),
        };
        vars_by_scope.entry(scope_key).or_default().push(var);
    }
    
    // 3. 生成 .env 格式内容
    let mut output = String::new();
    
    // 全局作用域优先
    if let Some(global_vars) = vars_by_scope.remove("Global") {
        if options.include_comments {
            writeln!(output, "# Global variables (from {})", 
                state.config_dir.join("mise/config.toml").display())?;
        }
        append_env_vars(&mut output, &global_vars, &state, options.expand).await?;
        output.push('\n');
    }
    
    // 其他作用域按字母顺序
    let mut other_scopes: Vec<_> = vars_by_scope.into_iter().collect();
    other_scopes.sort_by(|a, b| a.0.cmp(&b.0));
    
    for (scope_name, vars) in other_scopes {
        if options.include_comments {
            let source_file = vars.first().map(|v| &v.source_file)
                .unwrap_or(&PathBuf::from("unknown"));
            writeln!(output, "# {} (from {})", scope_name, source_file.display())?;
        }
        append_env_vars(&mut output, &vars, &state, options.expand).await?;
        output.push('\n');
    }
    
    Ok((
        TypedHeader(ContentType::from(mime::TEXT_PLAIN)),
        TypedHeader(ContentDisposition::attachment("svcmgr.env")),
        output,
    ))
}

async fn append_env_vars(
    output: &mut String,
    vars: &[EnvVar],
    state: &AppState,
    expand: bool,
) -> Result<()> {
    for var in vars {
        let value = if expand && var.has_references {
            let expander = VariableExpander::new(&state.config_port).await?;
            expander.expand(&var.value, &var.scope)?
        } else {
            var.value.clone()
        };
        
        // 如果值包含空格或特殊字符，用双引号包裹
        let quoted_value = if value.contains(' ') || value.contains('#') {
            format!("\"{}\"", value)
        } else {
            value
        };
        
        writeln!(output, "{}={}", var.key, quoted_value)?;
    }
    Ok(())
}
```

---

## Variable Expansion

### Expansion Syntax

支持 `${VAR_NAME}` 语法引用其他环境变量：

```toml
# .config/mise/config.toml
[env]
HOST = "localhost"
PORT = "3000"
BASE_URL = "http://${HOST}:${PORT}"  # 展开为 "http://localhost:3000"
```

### Expansion Rules

1. **作用域优先级**：引用解析时遵循作用域优先级（任务 > 服务 > 全局）
2. **递归展开**：支持多层引用（A → B → C），最大深度 10 层
3. **循环检测**：自动检测循环引用（A → B → A），返回错误
4. **未定义引用**：引用不存在的变量时保持原样（`${UNDEFINED}` 不变）
5. **转义**：使用 `\${VAR}` 转义，展开为 `${VAR}` 字面量

### VariableExpander Implementation

```rust
pub struct VariableExpander<'a> {
    config_port: &'a dyn ConfigPort,
    cache: HashMap<(String, EnvScope), String>,
}

impl<'a> VariableExpander<'a> {
    pub async fn new(config_port: &'a dyn ConfigPort) -> Result<Self> {
        Ok(Self {
            config_port,
            cache: HashMap::new(),
        })
    }
    
    /// 展开变量引用
    pub async fn expand(
        &mut self,
        value: &str,
        scope: &EnvScope,
    ) -> Result<String> {
        self.expand_with_depth(value, scope, 0, &mut HashSet::new()).await
    }
    
    fn expand_with_depth(
        &mut self,
        value: &str,
        scope: &EnvScope,
        depth: usize,
        visiting: &mut HashSet<String>,
    ) -> Result<String> {
        // 防止无限递归
        if depth > 10 {
            return Err(Error::BadRequest {
                message: "Variable expansion depth exceeded (max 10)".into(),
            });
        }
        
        let mut result = String::new();
        let mut chars = value.chars().peekable();
        
        while let Some(ch) = chars.next() {
            if ch == '\\' && chars.peek() == Some(&'$') {
                // 转义：\${VAR} → ${VAR}
                chars.next(); // 跳过 $
                result.push('$');
                continue;
            }
            
            if ch == '$' && chars.peek() == Some(&'{') {
                chars.next(); // 跳过 {
                
                // 提取变量名（直到 }）
                let mut var_name = String::new();
                while let Some(&ch) = chars.peek() {
                    if ch == '}' {
                        chars.next(); // 跳过 }
                        break;
                    }
                    var_name.push(chars.next().unwrap());
                }
                
                // 检测循环引用
                if visiting.contains(&var_name) {
                    return Err(Error::BadRequest {
                        message: format!("Circular reference detected: {}", var_name),
                    });
                }
                visiting.insert(var_name.clone());
                
                // 解析引用（按作用域优先级）
                if let Some(ref_value) = self.resolve_var(&var_name, scope).await? {
                    // 递归展开引用的值
                    let expanded = self.expand_with_depth(
                        &ref_value,
                        scope,
                        depth + 1,
                        visiting,
                    ).await?;
                    result.push_str(&expanded);
                } else {
                    // 未定义的引用，保持原样
                    result.push_str(&format!("${{{}}}", var_name));
                }
                
                visiting.remove(&var_name);
            } else {
                result.push(ch);
            }
        }
        
        Ok(result)
    }
    
    /// 按作用域优先级解析变量
    async fn resolve_var(
        &mut self,
        key: &str,
        scope: &EnvScope,
    ) -> Result<Option<String>> {
        let cache_key = (key.to_string(), scope.clone());
        if let Some(cached) = self.cache.get(&cache_key) {
            return Ok(Some(cached.clone()));
        }
        
        // 优先级：当前作用域 → 全局作用域
        let value = match scope {
            EnvScope::Task { name } => {
                self.config_port.get_task_env_var(name, key).await?
                    .or_else(|| self.config_port.get_global_env_var(key).await.ok().flatten())
            }
            EnvScope::Service { name } => {
                self.config_port.get_service_env_var(name, key).await?
                    .or_else(|| self.config_port.get_global_env_var(key).await.ok().flatten())
            }
            EnvScope::Global => {
                self.config_port.get_global_env_var(key).await?
            }
        };
        
        if let Some(ref v) = value {
            self.cache.insert(cache_key, v.clone());
        }
        
        Ok(value)
    }
    
    /// 检测循环引用（不实际展开）
    pub fn check_circular_reference(
        &self,
        key: &str,
        value: &str,
        scope: &EnvScope,
    ) -> Result<()> {
        let mut visiting = HashSet::new();
        visiting.insert(key.to_string());
        
        self.check_circular_in_value(value, scope, &mut visiting)?;
        Ok(())
    }
    
    fn check_circular_in_value(
        &self,
        value: &str,
        scope: &EnvScope,
        visiting: &mut HashSet<String>,
    ) -> Result<()> {
        // 提取所有 ${VAR_NAME} 引用
        let re = regex::Regex::new(r"\$\{([^}]+)\}").unwrap();
        for cap in re.captures_iter(value) {
            let ref_var = &cap[1];
            
            if visiting.contains(ref_var) {
                return Err(Error::BadRequest {
                    message: format!(
                        "Circular reference detected: {} references {}",
                        visiting.iter().next().unwrap(),
                        ref_var
                    ),
                });
            }
            
            visiting.insert(ref_var.to_string());
            
            // 递归检查引用的变量
            if let Some(ref_value) = self.resolve_var(ref_var, scope).await? {
                self.check_circular_in_value(&ref_value, scope, visiting)?;
            }
            
            visiting.remove(ref_var);
        }
        
        Ok(())
    }
}
```

---

## Events

### EnvChanged Event

环境变量发生变更时发布：

```rust
#[derive(Debug, Clone, Serialize)]
pub struct EnvChangedEvent {
    /// 变量名
    pub key: String,
    
    /// 旧值（删除时为 Some，新增时为 None）
    pub old_value: Option<String>,
    
    /// 新值（删除时为 None，新增/更新时为 Some）
    pub new_value: Option<String>,
    
    /// 作用域
    pub scope: EnvScope,
    
    /// 事件时间
    pub timestamp: DateTime<Utc>,
}
```

**订阅者示例**：
- **Service Manager**：检测服务使用的环境变量是否变化，记录警告日志
- **Task Scheduler**：触发使用该变量的任务重新加载配置
- **Audit Logger**：记录环境变量变更审计日志

---

## Git Integration

### Auto-Commit Format

环境变量修改自动生成 Git 提交，提交信息格式：

```
env: <action> <key> [in <scope>]

<action>:
  - set: 设置或更新变量
  - delete: 删除变量
  - batch operation: 批量操作

<scope>:
  - global
  - service:<name>
  - task:<name>
```

**示例**：
```bash
git log --oneline
a1b2c3d env: set DATABASE_URL in global
b2c3d4e env: delete DEPRECATED_VAR in service:api
c3d4e5f env: batch operation (set=3, delete=1)
```

### Rollback Support

通过 Git 历史回滚环境变量配置：

```bash
# 通过配置管理 API 回滚
POST /api/v1/config/rollback
{
  "commit_sha": "a1b2c3d",
  "files": [".config/mise/config.toml"]
}
```

详见 **14-api-config.md** 中的回滚 API 规范。

---

## Validation

### Variable Name Validation

- **字符限制**：仅允许字母、数字、下划线（`[A-Za-z0-9_]+`）
- **长度限制**：最大 256 字符
- **保留前缀**：禁止使用 `SVCMGR_` 前缀（系统保留）

### Variable Value Validation

- **长度限制**：最大 10KB（10240 字节）
- **编码**：必须是有效的 UTF-8
- **引用深度**：变量展开最大递归深度 10 层
- **循环检测**：自动检测并拒绝循环引用

### Scope Validation

- **服务存在性**：`EnvScope::Service { name }` 必须引用已存在的服务
- **任务存在性**：`EnvScope::Task { name }` 必须引用已存在的任务
- **作用域冲突**：同一变量可以在多个作用域定义（按优先级生效）

---

## Error Handling

### Common Error Codes

| Code | HTTP Status | Description | Example |
|------|-------------|-------------|---------|
| `INVALID_ENV_NAME` | 400 | 变量名格式无效 | `MY-VAR` 包含非法字符 `-` |
| `ENV_VALUE_TOO_LARGE` | 400 | 变量值超过最大长度 | 值大于 10KB |
| `CIRCULAR_REFERENCE` | 400 | 检测到循环引用 | A → B → A |
| `SCOPE_NOT_FOUND` | 400 | 引用的服务/任务不存在 | `service:nonexistent` |
| `ENV_NOT_FOUND` | 404 | 变量不存在 | 在 global 作用域未找到 `FOO` |
| `GIT_CONFLICT` | 409 | Git 提交冲突 | 远程配置文件有新提交 |
| `CONFIG_WRITE_FAILED` | 500 | 配置文件写入失败 | 磁盘空间不足 |

### Error Response Example

```json
{
  "error": {
    "code": "CIRCULAR_REFERENCE",
    "message": "Circular reference detected: A references B, B references A",
    "details": {
      "variable": "A",
      "chain": ["A", "B", "A"]
    },
    "request_id": "req_8x9y0z"
  }
}
```

---

## Testing

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_set_global_env_var() {
        let state = setup_test_state().await;
        
        let request = EnvVarRequest {
            value: "test_value".to_string(),
            scope: EnvScope::Global,
        };
        
        let response = set_env_var(
            State(state.clone()),
            Path("TEST_VAR".to_string()),
            TypedHeader(IdempotencyKey::generate()),
            Json(request),
        ).await.unwrap();
        
        assert_eq!(response.0.data.key, "TEST_VAR");
        assert_eq!(response.0.data.value, "test_value");
        
        // 验证 Git 提交
        let commits = state.git_service.log(1).await.unwrap();
        assert_eq!(commits[0].message, "env: set TEST_VAR in global");
    }
    
    #[tokio::test]
    async fn test_variable_expansion() {
        let state = setup_test_state().await;
        
        // 设置基础变量
        set_var(&state, "HOST", "localhost", EnvScope::Global).await;
        set_var(&state, "PORT", "3000", EnvScope::Global).await;
        
        // 设置引用变量
        set_var(&state, "BASE_URL", "http://${HOST}:${PORT}", EnvScope::Global).await;
        
        // 获取并展开
        let response = get_env_var(
            State(state.clone()),
            Path("BASE_URL".to_string()),
            Query(GetEnvVarParams { expand: true }),
        ).await.unwrap();
        
        assert_eq!(
            response.0.data.effective_value,
            "http://localhost:3000"
        );
    }
    
    #[tokio::test]
    async fn test_circular_reference_detection() {
        let state = setup_test_state().await;
        
        // A → B
        set_var(&state, "A", "${B}", EnvScope::Global).await;
        
        // B → A（循环）
        let request = EnvVarRequest {
            value: "${A}".to_string(),
            scope: EnvScope::Global,
        };
        
        let result = set_env_var(
            State(state.clone()),
            Path("B".to_string()),
            TypedHeader(IdempotencyKey::generate()),
            Json(request),
        ).await;
        
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), "CIRCULAR_REFERENCE");
    }
    
    #[tokio::test]
    async fn test_batch_operations_atomicity() {
        let state = setup_test_state().await;
        
        let request = EnvBatchRequest {
            set: vec![
                EnvBatchSetItem {
                    key: "VAR1".to_string(),
                    value: "value1".to_string(),
                    scope: EnvScope::Global,
                },
                EnvBatchSetItem {
                    key: "VAR2".to_string(),
                    value: "${NONEXISTENT}".to_string(), // 无效引用
                    scope: EnvScope::Global,
                },
            ],
            delete: vec![],
        };
        
        let result = batch_env_operations(
            State(state.clone()),
            TypedHeader(IdempotencyKey::generate()),
            Json(request),
        ).await;
        
        // 验证失败时所有变更都被回滚
        assert!(result.is_err());
        assert!(state.config_port.get_global_env_var("VAR1").await.unwrap().is_none());
    }
    
    #[tokio::test]
    async fn test_import_env_file() {
        let state = setup_test_state().await;
        
        let env_content = "VAR1=value1\nVAR2=value2\n# Comment\nVAR3=value3";
        let encoded = base64::encode(env_content);
        
        let request = EnvImportRequest {
            content: encoded,
            scope: EnvScope::Global,
            conflict_strategy: ConflictStrategy::Skip,
        };
        
        let response = import_env_file(
            State(state.clone()),
            Json(request),
        ).await.unwrap();
        
        assert_eq!(response.0.data.imported_count, 3);
        
        // 验证变量已导入
        assert_eq!(
            state.config_port.get_global_env_var("VAR1").await.unwrap(),
            Some("value1".to_string())
        );
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_env_lifecycle() {
    let app = setup_test_app().await;
    
    // 1. 设置变量
    let response = app
        .put("/api/v1/env/MY_VAR")
        .json(&json!({
            "value": "my_value",
            "scope": { "type": "global" }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    // 2. 获取变量
    let response = app
        .get("/api/v1/env/MY_VAR")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let data: EnvVarDetail = response.json().await.unwrap().data;
    assert_eq!(data.effective_value, "my_value");
    
    // 3. 列出变量
    let response = app
        .get("/api/v1/env?scope=global")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let data: ListResponse<EnvVar> = response.json().await.unwrap();
    assert!(data.data.iter().any(|v| v.key == "MY_VAR"));
    
    // 4. 删除变量
    let response = app
        .delete("/api/v1/env/MY_VAR?scope=global")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    
    // 5. 验证已删除
    let response = app
        .get("/api/v1/env/MY_VAR")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
```

---

## Related Specifications

- **01-config-design.md** - 配置文件分离策略和 TOML 格式定义
- **04-git-versioning.md** - Git 自动版本控制和回滚机制
- **07-mise-integration.md** - mise Port-Adapter 模式和 EnvPort 接口
- **10-api-overview.md** - REST API 设计原则、认证、错误处理
- **11-api-services.md** - 服务管理 API（服务作用域环境变量）
- **12-api-tasks.md** - 任务管理 API（任务作用域环境变量）
- **14-api-config.md** - 配置管理 API（配置文件回滚）

---

## Open Questions

1. **跨作用域引用**：是否允许服务作用域的变量引用任务作用域的变量？（当前设计：不允许，只能引用同作用域或全局）
2. **环境变量加密**：是否需要支持敏感变量加密存储？（如数据库密码）
3. **变量生命周期**：是否需要支持临时变量（会话级别，不持久化到配置文件）？
4. **变量模板**：是否支持预定义变量模板（如 `production.env`, `development.env`）？
5. **变量继承**：服务是否可以显式继承其他服务的环境变量？

---

**文档状态**: Draft  
**最后更新**: 2026-02-23  
**作者**: svcmgr Redesign Team
