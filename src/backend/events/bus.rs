//! Event bus for publish-subscribe event system
//!
//! This module provides a broadcast-based event bus that allows components to:
//! - Emit events (fire-and-forget)
//! - Subscribe to all events (receive via channel)
//! - Register typed event handlers (async callbacks)

use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};

use crate::scheduler::trigger::EventType;

use super::handlers::EventHandler;

/// Handler registry type alias to reduce complexity
type HandlerRegistry = Arc<RwLock<HashMap<String, Vec<Arc<dyn EventHandler>>>>>;

/// Event bus for publish-subscribe pattern
///
/// Uses tokio broadcast channels for efficient event distribution.
/// Supports both raw subscriptions (receive channel) and typed handlers (callbacks).
// Manual Clone implementation below (Receiver doesn't impl Clone)
pub struct EventBus {
    /// Broadcast sender for events
    tx: broadcast::Sender<EventType>,
    /// Keep-alive receiver (prevents channel from closing)
    #[allow(dead_code)]
    _keep_alive: broadcast::Receiver<EventType>,
    /// Registered event handlers (keyed by event discriminant)
    handlers: HandlerRegistry,
}
impl Clone for EventBus {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            _keep_alive: self.tx.subscribe(), // Create new receiver from sender
            handlers: self.handlers.clone(),
        }
    }
}

impl EventBus {
    /// Create a new event bus with default capacity (1024 events)
    pub fn new() -> Self {
        let (tx, rx) = broadcast::channel(1024);
        Self {
            tx,
            _keep_alive: rx,
            handlers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new event bus with custom capacity
    pub fn with_capacity(capacity: usize) -> Self {
        let (tx, rx) = broadcast::channel(capacity);
        Self {
            tx,
            _keep_alive: rx,
            handlers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Emit an event (fire-and-forget)
    ///
    /// This will:
    /// 1. Broadcast event to all raw subscribers
    /// 2. Trigger registered handlers asynchronously
    pub fn emit(&self, event: EventType) -> Result<()> {
        // Broadcast to raw subscribers
        self.tx
            .send(event.clone())
            .map_err(|e| anyhow!("Failed to emit event: {}", e))?;

        // Trigger handlers asynchronously (don't block)
        let handlers = self.handlers.clone();
        tokio::spawn(async move {
            let key = event_key(&event);
            let handlers_map = handlers.read().await;

            if let Some(handler_list) = handlers_map.get(&key) {
                for handler in handler_list {
                    if let Err(e) = handler.handle(&event).await {
                        tracing::error!("Event handler failed for {}: {}", key, e);
                    }
                }
            }
        });

        Ok(())
    }

    /// Subscribe to all events (returns raw receiver)
    ///
    /// Useful for monitoring or logging all events.
    pub fn subscribe(&self) -> broadcast::Receiver<EventType> {
        self.tx.subscribe()
    }

    /// Register an event handler for a specific event type
    ///
    /// # Example
    /// ```ignore
    /// let bus = EventBus::new();
    /// bus.register_handler("SystemInit", Arc::new(MyHandler)).await;
    /// ```
    pub async fn register_handler(&self, event_key: &str, handler: Arc<dyn EventHandler>) {
        let mut handlers = self.handlers.write().await;
        handlers
            .entry(event_key.to_string())
            .or_insert_with(Vec::new)
            .push(handler);
    }

    /// Unregister all handlers for a specific event type
    pub async fn unregister_handlers(&self, event_key: &str) {
        let mut handlers = self.handlers.write().await;
        handlers.remove(event_key);
    }

    /// Get current number of active subscribers
    pub fn receiver_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate a key for event type (used for handler registration)
///
/// Maps EventType variants to string keys:
/// - SystemInit -> "SystemInit"
/// - TaskExit { .. } -> "TaskExit"
/// - Custom { name: "foo" } -> "Custom"
fn event_key(event: &EventType) -> String {
    match event {
        EventType::SystemInit => "SystemInit".to_string(),
        EventType::SystemShutdown => "SystemShutdown".to_string(),
        EventType::TaskExit { .. } => "TaskExit".to_string(),
        EventType::TaskStart { .. } => "TaskStart".to_string(),
        EventType::ConfigChanged { .. } => "ConfigChanged".to_string(),
        EventType::Custom { .. } => "Custom".to_string(),
        EventType::TaskUnhealthy { .. } => "TaskUnhealthy".to_string(),
        EventType::TaskHealthy { .. } => "TaskHealthy".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::trigger::EventType;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingHandler {
        count: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl EventHandler for CountingHandler {
        async fn handle(&self, _event: &EventType) -> Result<()> {
            self.count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_event_bus_emit_and_subscribe() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();

        bus.emit(EventType::SystemInit).unwrap();

        let event = rx.recv().await.unwrap();
        assert_eq!(event, EventType::SystemInit);
    }

    #[tokio::test]
    async fn test_event_bus_multiple_subscribers() {
        let bus = EventBus::new();
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        bus.emit(EventType::SystemInit).unwrap();

        assert_eq!(rx1.recv().await.unwrap(), EventType::SystemInit);
        assert_eq!(rx2.recv().await.unwrap(), EventType::SystemInit);
    }

    #[tokio::test]
    async fn test_event_bus_handlers() {
        let bus = EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let handler = Arc::new(CountingHandler {
            count: count.clone(),
        });

        bus.register_handler("SystemInit", handler).await;
        bus.emit(EventType::SystemInit).unwrap();

        // Wait for async handler execution
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_event_key_generation() {
        assert_eq!(event_key(&EventType::SystemInit), "SystemInit");
        assert_eq!(
            event_key(&EventType::TaskExit {
                task_name: "test".to_string(),
                exit_code: Some(0)
            }),
            "TaskExit"
        );
        assert_eq!(
            event_key(&EventType::Custom {
                name: "foo".to_string()
            }),
            "Custom"
        );
    }
}
