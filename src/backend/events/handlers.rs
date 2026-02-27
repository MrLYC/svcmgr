//! Event handlers for processing emitted events

use anyhow::Result;
use async_trait::async_trait;

use crate::scheduler::trigger::EventType;

#[async_trait]
pub trait EventHandler: Send + Sync {
    async fn handle(&self, event: &EventType) -> Result<()>;
}

pub struct LoggingHandler;

#[async_trait]
impl EventHandler for LoggingHandler {
    async fn handle(&self, event: &EventType) -> Result<()> {
        match event {
            EventType::SystemInit => tracing::info!("System initialized"),
            EventType::SystemShutdown => tracing::info!("System shutting down"),
            EventType::TaskStart { task_name } => {
                tracing::info!("Task started: {}", task_name);
            }
            EventType::TaskExit {
                task_name,
                exit_code,
            } => {
                if let Some(code) = exit_code {
                    tracing::info!("Task {} exited with code {}", task_name, code);
                } else {
                    tracing::info!("Task {} exited (no exit code)", task_name);
                }
            }
            EventType::ConfigChanged { path } => {
                tracing::info!("Configuration changed: {}", path);
            }
            EventType::Custom { name } => {
                tracing::info!("Custom event: {}", name);
            }
            EventType::TaskUnhealthy {
                task_name,
                consecutive_failures,
            } => {
                tracing::warn!(
                    "Task {} unhealthy ({} consecutive failures)",
                    task_name,
                    consecutive_failures
                );
            }
            EventType::TaskHealthy { task_name } => {
                tracing::info!("Task {} recovered to healthy state", task_name);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_logging_handler_handles_all_events() {
        let handler = LoggingHandler;

        assert!(handler.handle(&EventType::SystemInit).await.is_ok());
        assert!(handler.handle(&EventType::SystemShutdown).await.is_ok());
        assert!(handler
            .handle(&EventType::TaskStart {
                task_name: "test".to_string()
            })
            .await
            .is_ok());
        assert!(handler
            .handle(&EventType::TaskExit {
                task_name: "test".to_string(),
                exit_code: Some(0)
            })
            .await
            .is_ok());
        assert!(handler
            .handle(&EventType::ConfigChanged {
                path: "/test/path".to_string()
            })
            .await
            .is_ok());
        assert!(handler
            .handle(&EventType::Custom {
                name: "custom_event".to_string()
            })
            .await
            .is_ok());
        assert!(handler
            .handle(&EventType::TaskUnhealthy {
                task_name: "test".to_string(),
                consecutive_failures: 3
            })
            .await
            .is_ok());
        assert!(handler
            .handle(&EventType::TaskHealthy {
                task_name: "test".to_string()
            })
            .await
            .is_ok());
    }
}
