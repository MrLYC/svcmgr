# 即时任务 API 实现

**状态**: 待实现  
**优先级**: 低  
**来源**: Phase 7 完成后识别的功能缺失  
**相关提交**: 7c83b08 (引入测试), 8ef2c52 (未涉及此部分)

## 问题描述

task_api_integration 测试套件中有 2 个即时任务 (Immediate Task) API 测试失败,相关功能未实现:

1. `test_get_task_not_found` - GET /api/v1/tasks/:id 返回 500 而非 404
2. `test_cancel_task_not_implemented` - POST /api/v1/tasks/:id/cancel 返回 405 而非 200

### 当前状态
- **测试结果**: task_api_integration (immediate tasks) 3/5 通过 (60%)
- **影响范围**: 即时任务 API (非定时任务)
- **性质**: 功能未完成 (commit 7c83b08 引入测试但未实现)

### 背景说明

**即时任务 (Immediate Task) vs 定时任务 (Scheduled Task)**:
- **定时任务**: 按 cron 表达式定期执行 (已完全实现,9/9 测试通过)
- **即时任务**: 手动触发立即执行,返回 task_id 用于跟踪执行状态

即时任务 API 路径:
- `POST /api/v1/tasks` - 创建并立即执行任务
- `GET /api/v1/tasks/:id` - 查询任务执行状态
- `POST /api/v1/tasks/:id/cancel` - 取消正在执行的任务
- `GET /api/v1/tasks` - 列出即时任务历史

## 根本原因分析

### 1. GET /api/v1/tasks/:id 错误处理

**当前行为**:
- 请求: `GET /api/v1/tasks/nonexistent`
- 期望: 404 NOT_FOUND (任务不存在)
- 实际: 500 INTERNAL_SERVER_ERROR

**疑似原因**:
- handler 中没有正确处理"任务不存在"的情况
- 可能直接 panic 或返回 unwrap() 错误
- 需要添加错误处理,返回 `ApiError::new("TASK_NOT_FOUND", ...)`

### 2. POST /api/v1/tasks/:id/cancel 路由缺失

**当前行为**:
- 请求: `POST /api/v1/tasks/test/cancel`
- 期望: 200 OK (测试名称误导,应该是功能性测试)
- 实际: 405 METHOD_NOT_ALLOWED (路由未注册)

**确认原因**:
- src/backend/web/api/tasks.rs 中没有 `cancel_task` handler
- src/backend/web/server.rs 中没有注册该路由

## API 规范定义

根据 openspec/specs/12-api-tasks.md,即时任务 API 应该包括:

### POST /api/v1/tasks (创建并执行)
```json
Request:
{
  "execution": {
    "type": "command",
    "command": "backup.sh"
  },
  "timeout": 3600,
  "limits": { ... }
}

Response (201 Created):
{
  "data": {
    "id": "task_123abc",
    "status": "running",
    "started_at": "2026-02-26T10:00:00Z",
    "execution": { ... }
  }
}
```

### GET /api/v1/tasks/:id (查询状态)
```json
Response (200 OK):
{
  "data": {
    "id": "task_123abc",
    "status": "completed",  // running | completed | failed | cancelled
    "started_at": "2026-02-26T10:00:00Z",
    "completed_at": "2026-02-26T10:05:30Z",
    "exit_code": 0,
    "output": "...",
    "error": null
  }
}

Response (404 NOT_FOUND):
{
  "error": {
    "code": "TASK_NOT_FOUND",
    "message": "Task 'task_123abc' not found"
  }
}
```

### POST /api/v1/tasks/:id/cancel (取消任务)
```json
Response (200 OK):
{
  "data": {
    "id": "task_123abc",
    "status": "cancelled",
    "cancelled_at": "2026-02-26T10:02:00Z"
  }
}

Response (404 NOT_FOUND):
{
  "error": {
    "code": "TASK_NOT_FOUND",
    "message": "Task 'task_123abc' not found"
  }
}
```

## 实现方案

### 阶段 1: 修复 GET /api/v1/tasks/:id 错误处理 (Quick Fix)

**目标**: 让测试通过,返回正确的 404 错误

**步骤**:
1. 定位 `get_task` handler (src/backend/web/api/tasks.rs)
2. 添加错误处理:
   ```rust
   async fn get_task(
       State(state): State<AppState>,
       Path(task_id): Path<String>,
   ) -> Result<Json<ApiResponse<TaskStatus>>, ApiError> {
       let task = state.task_store
           .get(&task_id)
           .ok_or_else(|| ApiError::new(
               "TASK_NOT_FOUND",
               format!("Task '{}' not found", task_id)
           ))?;
       
       Ok(Json(ApiResponse {
           data: task,
           pagination: None,
       }))
   }
   ```

3. 验证:
   ```bash
   cargo test --test task_api_integration test_get_task_not_found --jobs=1
   ```

### 阶段 2: 实现 POST /api/v1/tasks/:id/cancel (完整功能)

**前置条件**:
- 需要设计即时任务执行和状态管理机制
- 可能需要引入后台任务队列 (tokio task spawn)
- 需要线程安全的任务状态存储 (Arc<Mutex<HashMap<String, TaskState>>>)

**实现步骤**:

1. **设计数据结构**:
   ```rust
   // src/backend/web/api/task_models.rs
   pub struct TaskStatus {
       pub id: String,
       pub status: TaskState,
       pub started_at: DateTime<Utc>,
       pub completed_at: Option<DateTime<Utc>>,
       pub exit_code: Option<i32>,
       pub output: Option<String>,
       pub error: Option<String>,
   }
   
   pub enum TaskState {
       Running,
       Completed,
       Failed,
       Cancelled,
   }
   ```

2. **添加任务存储**:
   ```rust
   // src/backend/web/server.rs
   pub struct AppState {
       // ... existing fields
       pub task_store: Arc<Mutex<HashMap<String, TaskStatus>>>,
       pub cancel_tokens: Arc<Mutex<HashMap<String, tokio_util::sync::CancellationToken>>>,
   }
   ```

3. **实现 cancel_task handler**:
   ```rust
   // src/backend/web/api/tasks.rs
   async fn cancel_task(
       State(state): State<AppState>,
       Path(task_id): Path<String>,
   ) -> Result<Json<ApiResponse<TaskStatus>>, ApiError> {
       // Check task exists
       let mut tasks = state.task_store.lock().await;
       let task = tasks.get_mut(&task_id)
           .ok_or_else(|| ApiError::new("TASK_NOT_FOUND", ...))?;
       
       // Cancel the task
       if let Some(token) = state.cancel_tokens.lock().await.get(&task_id) {
           token.cancel();
       }
       
       task.status = TaskState::Cancelled;
       task.completed_at = Some(Utc::now());
       
       Ok(Json(ApiResponse {
           data: task.clone(),
           pagination: None,
       }))
   }
   ```

4. **注册路由**:
   ```rust
   // src/backend/web/server.rs
   let task_routes = Router::new()
       .route("/tasks", post(create_immediate_task).get(list_tasks))
       .route("/tasks/:id", get(get_task))
       .route("/tasks/:id/cancel", post(cancel_task));
   ```

5. **实现任务执行引擎**:
   ```rust
   async fn execute_task(
       task_id: String,
       execution: TaskExecution,
       state: AppState,
       cancel_token: CancellationToken,
   ) {
       tokio::spawn(async move {
           // Update status: Running
           // Execute command/mise task
           // Handle cancellation
           // Update status: Completed/Failed/Cancelled
           // Store output/error
       });
   }
   ```

### 阶段 3: 验证和测试

1. 运行即时任务 API 测试:
   ```bash
   cargo test --test task_api_integration --jobs=1
   ```
   期望: 5/5 通过

2. 端到端测试:
   ```bash
   # 启动服务
   cargo run
   
   # 创建任务
   curl -X POST http://localhost:3000/api/v1/tasks \
     -H "Content-Type: application/json" \
     -d '{"execution": {"type": "command", "command": "sleep 10"}}'
   
   # 查询状态
   curl http://localhost:3000/api/v1/tasks/{task_id}
   
   # 取消任务
   curl -X POST http://localhost:3000/api/v1/tasks/{task_id}/cancel
   ```

## 设计决策

### 任务 ID 生成策略
- **UUID v4**: 随机唯一 ID (推荐)
- **Timestamp + Random**: `task_20260226_abc123`
- **Sequential**: 需要全局计数器

### 任务存储策略
- **内存存储** (当前方案): 重启丢失,适合原型
- **数据库存储**: 持久化,适合生产环境
- **混合方案**: 内存 + 异步持久化

### 取消机制
- **CancellationToken**: Tokio 标准方案 (推荐)
- **Signal channel**: 自定义实现
- **Process kill**: 直接 kill 进程 (粗暴)

## 验收标准

**阶段 1 (Quick Fix)**:
- [ ] test_get_task_not_found 通过
- [ ] GET /api/v1/tasks/:id 返回 404 for nonexistent tasks

**阶段 2 (完整实现)**:
- [ ] test_cancel_task_not_implemented 通过 (重命名为 test_cancel_task_success)
- [ ] POST /api/v1/tasks 创建并执行任务
- [ ] POST /api/v1/tasks/:id/cancel 取消正在执行的任务
- [ ] task_api_integration 测试套件 5/5 通过 (100%)

**完整验收**:
- [ ] 所有即时任务 API 端点实现
- [ ] 任务状态正确跟踪
- [ ] 取消机制正常工作
- [ ] 无测试回归

## 技术约束

1. **OpenSpec 优先**: API 设计必须符合 openspec/specs/12-api-tasks.md
2. **线程安全**: 任务状态存储必须线程安全 (Arc<Mutex<...>>)
3. **错误处理**: 所有错误必须返回正确的 HTTP 状态码和 error code

## 风险评估

**中等风险**:
- 需要引入任务执行引擎和状态管理
- 取消机制需要仔细设计 (避免 race condition)
- 可能影响服务器架构 (AppState 扩展)

**建议**:
- 先实现阶段 1 (Quick Fix) 让测试通过
- 阶段 2 作为独立 feature 开发,充分测试后再合并

## 相关文件

```
src/backend/web/api/tasks.rs          - Task handlers
src/backend/web/api/task_models.rs    - 数据结构定义
src/backend/web/server.rs             - 路由注册和 AppState
tests/task_api_integration.rs         - 集成测试
openspec/specs/12-api-tasks.md        - API 规范定义
```

## 依赖关系

- **依赖**: tokio, tokio-util (CancellationToken)
- **可选**: uuid (任务 ID 生成)
- **可选**: serde_json (输出序列化)

## 参考资料

- [Tokio CancellationToken](https://docs.rs/tokio-util/latest/tokio_util/sync/struct.CancellationToken.html)
- [Axum State Management](https://docs.rs/axum/latest/axum/extract/struct.State.html)
- Phase 7 修复: commit 8ef2c52 (定时任务 API 完整实现)
