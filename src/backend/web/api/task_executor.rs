/// 即时任务执行引擎
use super::task_models::{ImmediateTaskState, ImmediateTaskStatus};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// 任务条目（包含状态和取消令牌）
#[derive(Clone)]
pub struct TaskEntry {
    pub state: ImmediateTaskState,
    pub cancel_token: CancellationToken,
}

/// 全局任务管理器
#[derive(Clone)]
pub struct TaskExecutor {
    /// 任务存储（使用 RwLock 支持读多写少场景）
    tasks: Arc<RwLock<HashMap<String, TaskEntry>>>,
}

impl TaskExecutor {
    /// 创建新的任务执行器
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 创建并启动新任务
    pub async fn create_task(&self, command: String, args: Vec<String>) -> String {
        let task_id = Uuid::new_v4().to_string();
        let cancel_token = CancellationToken::new();

        // 创建初始状态
        let state = ImmediateTaskState {
            id: task_id.clone(),
            status: ImmediateTaskStatus::Pending,
            created_at: Utc::now(),
            started_at: None,
            finished_at: None,
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            error: None,
        };

        // 存储任务条目
        {
            let mut tasks = self.tasks.write().await;
            tasks.insert(
                task_id.clone(),
                TaskEntry {
                    state: state.clone(),
                    cancel_token: cancel_token.clone(),
                },
            );
        }

        // 在后台启动任务执行
        let executor = self.clone();
        tokio::spawn(async move {
            executor
                .execute_task(task_id, command, args, cancel_token)
                .await;
        });

        state.id
    }

    /// 执行任务（在后台运行）
    async fn execute_task(
        &self,
        task_id: String,
        command: String,
        args: Vec<String>,
        cancel_token: CancellationToken,
    ) {
        // 更新状态为 Running
        self.update_status(&task_id, |state| {
            state.status = ImmediateTaskStatus::Running;
            state.started_at = Some(Utc::now());
        })
        .await;

        // 创建命令
        let mut cmd = tokio::process::Command::new(&command);
        cmd.args(&args);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        // 执行命令并监听取消信号
        let result = tokio::select! {
            output = cmd.output() => {
                match output {
                    Ok(output) => Ok(output),
                    Err(e) => Err(format!("Failed to execute command: {}", e))
                }
            }
            _ = cancel_token.cancelled() => {
                Err("Task cancelled".to_string())
            }
        };

        // 更新最终状态
        self.update_status(&task_id, |state| {
            state.finished_at = Some(Utc::now());
            match result {
                Ok(output) => {
                    state.stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    state.stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    state.exit_code = output.status.code();
                    state.status = if output.status.success() {
                        ImmediateTaskStatus::Succeeded
                    } else {
                        ImmediateTaskStatus::Failed
                    };
                }
                Err(err) => {
                    state.status = ImmediateTaskStatus::Cancelled;
                    state.error = Some(err);
                }
            }
        })
        .await;
    }

    /// 获取任务状态
    pub async fn get_task(&self, task_id: &str) -> Option<ImmediateTaskState> {
        let tasks = self.tasks.read().await;
        tasks.get(task_id).map(|entry| entry.state.clone())
    }

    /// 取消任务
    pub async fn cancel_task(&self, task_id: &str) -> Result<(), String> {
        let tasks = self.tasks.read().await;
        if let Some(entry) = tasks.get(task_id) {
            entry.cancel_token.cancel();
            Ok(())
        } else {
            Err(format!("Task '{}' not found", task_id))
        }
    }

    /// 更新任务状态（内部辅助方法）
    async fn update_status<F>(&self, task_id: &str, update_fn: F)
    where
        F: FnOnce(&mut ImmediateTaskState),
    {
        let mut tasks = self.tasks.write().await;
        if let Some(entry) = tasks.get_mut(task_id) {
            update_fn(&mut entry.state);
        }
    }
}

impl Default for TaskExecutor {
    fn default() -> Self {
        Self::new()
    }
}
