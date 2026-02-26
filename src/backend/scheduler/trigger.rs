//! Trigger types for task scheduling
//!
//! Phase 2.1: Unified scheduler engine core

use anyhow::{anyhow, Result};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Task trigger types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Trigger {
    /// One-shot trigger - execute immediately (equivalent to `mise run`)
    OneShot,

    /// Delayed trigger - execute after specified delay
    Delayed {
        #[serde(with = "humantime_serde")]
        delay: Duration,
    },

    /// Cron trigger - driven by cron expression
    Cron {
        /// Cron expression (e.g., "0 0 * * * *" for hourly)
        expression: String,
        /// Next execution time (computed from expression)
        #[serde(skip)]
        next_tick: Option<DateTime<Local>>,
    },

    /// Event trigger - driven by system/task events
    Event { event_type: EventType },
}

/// Event types for event-driven triggers
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventType {
    /// System initialization complete
    SystemInit,

    /// Before system shutdown
    SystemShutdown,

    /// Task exit (normal or abnormal)
    TaskExit {
        task_name: String,
        exit_code: Option<i32>,
    },

    /// Task start
    TaskStart { task_name: String },

    /// Configuration changed
    ConfigChanged { path: String },

    /// Custom event
    Custom { name: String },

    /// Task health check failed
    TaskUnhealthy {
        task_name: String,
        consecutive_failures: u32,
    },

    /// Task recovered from unhealthy state
    TaskHealthy { task_name: String },
}

/// Restart policy for services
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(tag = "policy", rename_all = "snake_case")]
pub enum RestartPolicy {
    /// Never restart
    #[default]
    Never,

    /// Always restart
    Always {
        #[serde(with = "humantime_serde")]
        delay: Duration,
        limit: u32,
        #[serde(with = "humantime_serde")]
        window: Duration,
    },

    /// Restart only on failure (non-zero exit code)
    OnFailure {
        #[serde(with = "humantime_serde")]
        delay: Duration,
        limit: u32,
        #[serde(with = "humantime_serde")]
        window: Duration,
    },
}

impl RestartPolicy {
    /// Check if this policy requires restart for given exit code
    pub fn should_restart(&self, exit_code: i32) -> bool {
        match self {
            RestartPolicy::Never => false,
            RestartPolicy::Always { .. } => true,
            RestartPolicy::OnFailure { .. } => exit_code != 0,
        }
    }

    /// Get restart delay for this policy
    pub fn delay(&self) -> Option<Duration> {
        match self {
            RestartPolicy::Never => None,
            RestartPolicy::Always { delay, .. } | RestartPolicy::OnFailure { delay, .. } => {
                Some(*delay)
            }
        }
    }
}

/// Exponential backoff for restart delays
#[derive(Debug, Clone)]
pub struct RestartBackoff {
    initial_delay: Duration,
    max_delay: Duration,
    current_delay: Duration,
    attempt: u32,
}

impl RestartBackoff {
    pub fn new(initial_delay: Duration, max_delay: Duration) -> Self {
        Self {
            initial_delay,
            max_delay,
            current_delay: initial_delay,
            attempt: 0,
        }
    }

    /// Get next delay (with exponential backoff)
    pub fn next_delay(&mut self) -> Duration {
        let delay = self.current_delay;
        self.current_delay = std::cmp::min(self.current_delay * 2, self.max_delay);
        self.attempt += 1;
        delay
    }

    /// Reset backoff state (e.g., after successful run)
    pub fn reset(&mut self) {
        self.current_delay = self.initial_delay;
        self.attempt = 0;
    }

    /// Get current attempt count
    pub fn attempts(&self) -> u32 {
        self.attempt
    }
}

/// Restart tracker with time window and limits
#[derive(Debug, Clone)]
pub struct RestartTracker {
    restart_limit: u32,
    restart_window: Duration,
    restart_history: VecDeque<Instant>,
}

impl RestartTracker {
    pub fn new(limit: u32, window: Duration) -> Self {
        Self {
            restart_limit: limit,
            restart_window: window,
            restart_history: VecDeque::new(),
        }
    }

    /// Check if restart is allowed (within limit)
    pub fn can_restart(&mut self) -> bool {
        let now = Instant::now();

        // Remove restart records outside the time window
        while let Some(&start_time) = self.restart_history.front() {
            if now.duration_since(start_time) > self.restart_window {
                self.restart_history.pop_front();
            } else {
                break;
            }
        }

        // Check if within limit
        (self.restart_history.len() as u32) < self.restart_limit
    }

    /// Record a restart attempt
    pub fn record_restart(&mut self) {
        self.restart_history.push_back(Instant::now());
    }

    /// Get current restart count within window
    pub fn restart_count(&self) -> u32 {
        self.restart_history.len() as u32
    }

    /// Reset tracker (e.g., manual start after fatal state)
    pub fn reset(&mut self) {
        self.restart_history.clear();
    }
}

impl Trigger {
    /// Check if this trigger should fire now
    pub fn should_fire(&self, now: DateTime<Local>) -> bool {
        match self {
            Trigger::OneShot => true,         // Always fires immediately
            Trigger::Delayed { .. } => false, // Handled by timer
            Trigger::Cron { next_tick, .. } => {
                if let Some(tick) = next_tick {
                    now >= *tick
                } else {
                    false
                }
            }
            Trigger::Event { .. } => false, // Handled by event bus
        }
    }

    /// Compute next execution time for Cron triggers
    pub fn compute_next_tick(&mut self) -> Result<Option<DateTime<Local>>> {
        if let Trigger::Cron {
            expression,
            next_tick,
        } = self
        {
            use cron::Schedule;
            use std::str::FromStr;

            let schedule = Schedule::from_str(expression)
                .map_err(|e| anyhow!("Invalid cron expression '{}': {}", expression, e))?;
            let now = Local::now();
            let next = schedule.after(&now).next();
            *next_tick = next;
            return Ok(next);
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_restart_policy_should_restart() {
        let never = RestartPolicy::Never;
        assert!(!never.should_restart(0));
        assert!(!never.should_restart(1));

        let always = RestartPolicy::Always {
            delay: Duration::from_secs(1),
            limit: 3,
            window: Duration::from_secs(10),
        };
        assert!(always.should_restart(0));
        assert!(always.should_restart(1));

        let on_failure = RestartPolicy::OnFailure {
            delay: Duration::from_secs(1),
            limit: 3,
            window: Duration::from_secs(10),
        };
        assert!(!on_failure.should_restart(0));
        assert!(on_failure.should_restart(1));
    }

    #[test]
    fn test_restart_backoff() {
        let mut backoff = RestartBackoff::new(Duration::from_secs(1), Duration::from_secs(10));

        assert_eq!(backoff.next_delay(), Duration::from_secs(1));
        assert_eq!(backoff.next_delay(), Duration::from_secs(2));
        assert_eq!(backoff.next_delay(), Duration::from_secs(4));
        assert_eq!(backoff.next_delay(), Duration::from_secs(8));
        assert_eq!(backoff.next_delay(), Duration::from_secs(10)); // capped

        backoff.reset();
        assert_eq!(backoff.next_delay(), Duration::from_secs(1));
    }

    #[test]
    fn test_restart_tracker() {
        let mut tracker = RestartTracker::new(3, Duration::from_millis(100));

        // First 3 restarts should be allowed
        assert!(tracker.can_restart());
        tracker.record_restart();
        assert!(tracker.can_restart());
        tracker.record_restart();
        assert!(tracker.can_restart());
        tracker.record_restart();

        // 4th restart should be denied (within window)
        assert!(!tracker.can_restart());

        // Wait for window to expire
        thread::sleep(Duration::from_millis(120));

        // Should be allowed again (old records expired)
        assert!(tracker.can_restart());
    }

    #[test]
    fn test_trigger_cron_parsing() {
        let mut trigger = Trigger::Cron {
            expression: "0 0 * * * *".to_string(), // Every hour
            next_tick: None,
        };

        let next = trigger.compute_next_tick().unwrap();
        assert!(next.is_some());

        // Next tick should be in the future
        let now = Local::now();
        assert!(next.unwrap() > now);
    }

    #[test]
    fn test_event_type_equality() {
        let event1 = EventType::SystemInit;
        let event2 = EventType::SystemInit;
        assert_eq!(event1, event2);

        let event3 = EventType::TaskExit {
            task_name: "test".to_string(),
            exit_code: Some(0),
        };
        let event4 = EventType::TaskExit {
            task_name: "test".to_string(),
            exit_code: Some(0),
        };
        assert_eq!(event3, event4);
    }
}
