# 02 - 多任务调度引擎设计

> 版本：2.0.0-draft
> 状态：设计中

## 1. 概述

调度引擎是新架构的核心组件，负责管理所有任务的生命周期。它是一个纯内存运行时，配置来自解析后的 TOML 配置文件。

## 2. 触发器类型

```rust
/// 任务触发器类型
enum Trigger {
    /// 一次性触发 — 立即执行（等价于 `mise run`）
    OneShot,
    
    /// 延迟触发 — 延迟指定时间后执行
    Delayed { delay: Duration },
    
    /// 定时触发 — cron 表达式驱动
    Cron { 
        expression: String,
        schedule: cron::Schedule,
    },
    
    /// 事件触发 — 系统/任务事件驱动
    Event { event_type: EventType },
}

/// 事件类型
enum EventType {
    /// 系统初始化完成
    SystemInit,
    /// 系统关闭前
    SystemShutdown,
    /// 任务退出（正常或异常）
    TaskExit { 
        task_name: String,
        exit_code: Option<i32>,
    },
    /// 任务启动
    TaskStart { task_name: String },
    /// 配置变更
    ConfigChanged { path: String },
    /// 自定义事件
    Custom { name: String },
}
```

## 3. 任务定义

```rust
/// 调度任务
struct ScheduledTask {
    /// 任务名称（唯一标识）
    name: String,
    
    /// 触发器
    trigger: Trigger,
    
    /// 执行方式
    execution: Execution,
    
    /// 运行状态
    state: TaskState,
    
    /// 资源限制（可选）
    limits: Option<ResourceLimits>,
    
    /// 超时（0 = 无超时）
    timeout: Option<Duration>,
    
    /// 重启策略（仅服务类任务）
    restart_policy: RestartPolicy,
}

/// 执行方式
enum Execution {
    /// 通过 mise 任务执行（从 mise 配置获取命令）
    MiseTask { 
        task_name: String,
        args: Vec<String>,
    },
    
    /// 直接执行命令
    Command { 
        command: String,
        args: Vec<String>,
        env: HashMap<String, String>,
    },
}

/// 重启策略
enum RestartPolicy {
    /// 不自动重启
    No,
    
    /// 始终重启
    Always {
        delay: Duration,
        limit: u32,
        window: Duration,
    },
    
    /// 仅失败时重启
    OnFailure {
        delay: Duration,
        limit: u32,
        window: Duration,
    },
}

/// 任务状态
enum TaskState {
    /// 待调度
    Pending,
    
    /// 运行中
    Running { 
        pid: u32,
        started_at: Instant,
    },
    
    /// 已完成
    Completed { 
        exit_code: i32,
        finished_at: Instant,
    },
    
    /// 已失败
    Failed { 
        error: String,
        failed_at: Instant,
    },
    
    /// 致命错误（超过重启次数限制）
    Fatal { 
        last_error: String,
        restart_count: u32,
    },
}
```

## 4. 调度引擎核心循环

```rust
/// 调度引擎主循环（简化伪代码）
async fn engine_loop(engine: &SchedulerEngine) {
    loop {
        tokio::select! {
            // 1. 检查 cron 定时任务
            _ = engine.next_cron_tick() => {
                for task in engine.due_cron_tasks() {
                    engine.spawn_task(task).await;
                }
            }
            
            // 2. 处理事件队列
            event = engine.event_rx.recv() => {
                for task in engine.tasks_for_event(&event) {
                    engine.spawn_task(task).await;
                }
            }
            
            // 3. 处理延迟任务到期
            _ = engine.next_delayed_tick() => {
                for task in engine.due_delayed_tasks() {
                    engine.spawn_task(task).await;
                }
            }
            
            // 4. 监控子进程退出
            exit = engine.wait_any_child() => {
                engine.handle_child_exit(exit).await;
                // 触发 TaskExit 事件 → 可能触发自动重启或事件监听任务
            }
            
            // 5. 外部命令（来自 API/CLI）
            cmd = engine.command_rx.recv() => {
                engine.handle_command(cmd).await;
            }
        }
    }
}
```

## 5. 与 mise 的集成

### 配置示例

**mise 配置**：

```toml
# mise.toml
[tasks.api-start]
run = "node server.js"
env = { PORT = "3000" }
```

**svcmgr 配置**：

```toml
# svcmgr/config.toml
[services.api]
task = "api-start"
enable = true
restart = "always"
ports = { web = 3000 }
```

### 工作流

```
1. 解析 svcmgr [services.api]
   ↓
2. 发现 task="api-start" + enable=true
   ↓
3. 注册 Event(SystemInit) 触发器
   ↓
4. 系统初始化 → 触发 SystemInit 事件
   ↓
5. 引擎从 mise 配置读取 [tasks.api-start].run
   ↓
6. 直接 spawn "node server.js"（注入 PORT=3000）
   ↓
7. 进程管理器接管（日志捕获、资源限制）
   ↓
8. 进程退出 → 触发 TaskExit 事件
   ↓
9. 根据 restart="always" 自动重启
```

## 6. 事件风暴预防

### 6.1 指数退避（Exponential Backoff）

```rust
struct RestartBackoff {
    initial_delay: Duration,
    max_delay: Duration,
    current_delay: Duration,
    attempt: u32,
}

impl RestartBackoff {
    fn next_delay(&mut self) -> Duration {
        let delay = self.current_delay;
        self.current_delay = std::cmp::min(
            self.current_delay * 2,
            self.max_delay,
        );
        self.attempt += 1;
        delay
    }
    
    fn reset(&mut self) {
        self.current_delay = self.initial_delay;
        self.attempt = 0;
    }
}
```

### 6.2 重启次数限制 + 时间窗口

```rust
struct RestartTracker {
    restart_count: u32,
    restart_limit: u32,
    restart_window: Duration,
    restart_history: VecDeque<Instant>,
}

impl RestartTracker {
    fn can_restart(&mut self) -> bool {
        let now = Instant::now();
        
        // 移除窗口外的重启记录
        while let Some(&start_time) = self.restart_history.front() {
            if now.duration_since(start_time) > self.restart_window {
                self.restart_history.pop_front();
            } else {
                break;
            }
        }
        
        // 检查是否超过限制
        self.restart_history.len() < self.restart_limit as usize
    }
    
    fn record_restart(&mut self) {
        self.restart_history.push_back(Instant::now());
    }
}
```

### 6.3 致命状态（Fatal State）

当任务超过重启次数限制时，进入 `Fatal` 状态，需要人工 `start` 才能恢复：

```rust
async fn handle_task_exit(&mut self, task_name: &str, exit_code: i32) {
    let task = self.tasks.get_mut(task_name).unwrap();
    
    // 检查重启策略
    match &task.restart_policy {
        RestartPolicy::Always { .. } | RestartPolicy::OnFailure { .. } => {
            if task.restart_tracker.can_restart() {
                task.restart_tracker.record_restart();
                task.backoff.next_delay();
                
                // 延迟后重启
                tokio::time::sleep(task.backoff.current_delay).await;
                self.spawn_task(task).await;
            } else {
                // 超过重启次数限制 → Fatal 状态
                task.state = TaskState::Fatal {
                    last_error: format!("Exceeded restart limit"),
                    restart_count: task.restart_tracker.restart_count,
                };
                tracing::error!("Task '{}' entered FATAL state", task_name);
            }
        }
        RestartPolicy::No => {
            task.state = TaskState::Completed { exit_code, .. };
        }
    }
}
```

## 7. 调度引擎 API

```rust
pub struct SchedulerEngine {
    tasks: HashMap<String, ScheduledTask>,
    event_bus: EventBus,
    process_manager: ProcessManager,
}

impl SchedulerEngine {
    /// 启动引擎
    pub async fn start(&mut self) -> Result<()>;
    
    /// 停止引擎
    pub async fn stop(&mut self) -> Result<()>;
    
    /// 注册任务
    pub fn register_task(&mut self, task: ScheduledTask) -> Result<()>;
    
    /// 取消注册任务
    pub fn unregister_task(&mut self, name: &str) -> Result<()>;
    
    /// 手动启动任务
    pub async fn start_task(&mut self, name: &str) -> Result<()>;
    
    /// 手动停止任务
    pub async fn stop_task(&mut self, name: &str) -> Result<()>;
    
    /// 重启任务
    pub async fn restart_task(&mut self, name: &str) -> Result<()>;
    
    /// 获取任务状态
    pub fn get_task_state(&self, name: &str) -> Option<&TaskState>;
    
    /// 获取所有任务
    pub fn list_tasks(&self) -> Vec<&ScheduledTask>;
    
    /// 发送事件
    pub async fn emit_event(&mut self, event: EventType) -> Result<()>;
}
```

## 8. 与当前实现的对比

| 维度 | 当前（supervisor.rs） | 新设计（调度引擎） |
|------|----------------------|-------------------|
| 服务管理 | SupervisorAtom trait + SupervisorManager | SchedulerEngine + 事件触发器 |
| 定时任务 | SchedulerAtom trait（CRUD，不执行） | SchedulerEngine + Cron 触发器 |
| 任务执行 | 直接 `tokio::process::Command` | 通过 mise 任务或直接 Command |
| 配置来源 | 独立 TOML 文件（每个服务一个） | svcmgr 独立配置 + mise 配置关联 |
| 事件系统 | 无 | 内置事件总线 |
| 统一性 | 服务和定时任务分离管理 | 统一调度，通过触发器区分 |

## 参考

- [00-architecture-overview.md](./00-architecture-overview.md) - 整体架构
- [03-process-manager.md](./03-process-manager.md) - 子进程管理
- [pitchfork supervisor](https://github.com/jdx/pitchfork/tree/main/src/supervisor) - 参考实现
