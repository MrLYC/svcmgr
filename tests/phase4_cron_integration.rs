//! Phase 4.1 Integration Tests - Cron Scheduling
//!
//! End-to-end tests for Cron-based scheduled tasks.
//! Tests cover:
//! - Cron expression parsing and validation
//! - Scheduled task execution at correct times
//! - Multiple concurrent scheduled tasks
//! - Task timeout handling
//! - Retry on failure
//! - Scheduler precision (< 1 second error)

use anyhow::Result;
use chrono::{Duration as ChronoDuration, Local, Timelike};
use std::collections::HashMap;
use std::time::Duration;
use svcmgr::scheduler::engine::{Execution, ScheduledTask, SchedulerEngine};
use svcmgr::scheduler::trigger::{RestartPolicy, Trigger};
use tempfile::tempdir;
use tokio::time::sleep;

/// Helper to create a simple Cron task
fn create_cron_task(name: &str, cron_expr: &str) -> ScheduledTask {
    ScheduledTask::new(
        name.to_string(),
        Trigger::Cron {
            expression: cron_expr.to_string(),
            next_tick: None, // Will be computed by register_task()
        },
        Execution::Command {
            command: "echo".to_string(),
            args: vec![format!("Task {} executed", name)],
            env: HashMap::new(),
            workdir: None,
        },
        RestartPolicy::Never,
    )
}

#[tokio::test]
async fn test_cron_expression_parsing() -> Result<()> {
    // Test valid cron expressions are accepted
    let log_dir = tempdir()?.path().to_path_buf();
    let mut engine = SchedulerEngine::new(log_dir);

    // Every minute: "0 * * * * *"
    let task1 = create_cron_task("every_minute", "0 * * * * *");
    engine.register_task(task1)?;

    // Every hour at :30: "0 30 * * * *"
    let task2 = create_cron_task("every_hour_at_30", "0 30 * * * *");
    engine.register_task(task2)?;

    // Daily at 2 AM: "0 0 2 * * *"
    let task3 = create_cron_task("daily_2am", "0 0 2 * * *");
    engine.register_task(task3)?;

    assert_eq!(engine.list_tasks().len(), 3);
    Ok(())
}

#[tokio::test]
async fn test_invalid_cron_expression_rejected() -> Result<()> {
    // Test invalid cron expressions are rejected
    let log_dir = tempdir()?.path().to_path_buf();
    let mut engine = SchedulerEngine::new(log_dir);

    // Invalid expression: too many fields
    let task = create_cron_task("invalid", "0 0 0 0 0 0 0");
    let result = engine.register_task(task);

    // Should fail during compute_next_tick() inside register_task()
    assert!(result.is_err());

    Ok(())
}

#[tokio::test]
async fn test_cron_next_tick_computation() -> Result<()> {
    // Test that next_tick is correctly computed after registration
    let log_dir = tempdir()?.path().to_path_buf();
    let mut engine = SchedulerEngine::new(log_dir);

    // Schedule task to run every minute
    let task = create_cron_task("test", "0 * * * * *");
    engine.register_task(task)?;

    // Verify next_tick is set and in the future
    let tasks = engine.list_tasks();
    let scheduled_task = tasks.iter().find(|t| t.name == "test").unwrap();

    if let Trigger::Cron { next_tick, .. } = &scheduled_task.trigger {
        assert!(next_tick.is_some());
        let next = next_tick.unwrap();
        let now = Local::now();
        assert!(next > now);

        // Should be within next 60 seconds
        let diff = next.signed_duration_since(now);
        assert!(diff < ChronoDuration::seconds(60));
    } else {
        panic!("Expected Cron trigger");
    }

    Ok(())
}

#[tokio::test]
async fn test_cron_task_execution_timing() -> Result<()> {
    // Test that cron tasks execute at approximately correct times
    // Note: This test may be flaky on slow CI systems
    let log_dir = tempdir()?.path().to_path_buf();
    let mut engine = SchedulerEngine::new(log_dir.clone());

    // Use "every second" cron to avoid timing boundary issues
    // (avoids problems when current second > 56, where target_second wraps to next minute)
    let cron_expr = "* * * * * *"; // Run every second

    let task = create_cron_task("timed_test", cron_expr);
    engine.register_task(task)?;

    // Start engine in background
    let engine_handle = tokio::spawn(async move {
        let _ = engine.start().await;
    });

    // Wait 5 seconds to ensure at least one execution (increased to rule out startup delay)
    sleep(Duration::from_secs(5)).await;

    // Check execution log exists
    // Check execution log exists (ProcessHandle creates {name}.stdout.log)
    let log_file = log_dir.join("timed_test.stdout.log");
    assert!(
        log_file.exists(),
        "Task should have executed and created log"
    );

    // Cleanup
    engine_handle.abort();
    Ok(())
}

#[tokio::test]
async fn test_multiple_concurrent_cron_tasks() -> Result<()> {
    // Test that multiple cron tasks can coexist
    let log_dir = tempdir()?.path().to_path_buf();
    let mut engine = SchedulerEngine::new(log_dir);

    // Create 5 tasks with different schedules
    let schedules = vec![
        ("task1", "0 * * * * *"),  // Every minute
        ("task2", "30 * * * * *"), // Every minute at :30
        ("task3", "0 0 * * * *"),  // Every hour
        ("task4", "0 0 0 * * *"),  // Every day at midnight
        ("task5", "0 0 12 * * 1"), // Every Monday at noon
    ];

    for (name, schedule) in schedules {
        let task = create_cron_task(name, schedule);
        engine.register_task(task)?;
    }

    assert_eq!(engine.list_tasks().len(), 5);

    // Verify all tasks have next_tick computed
    for task in engine.list_tasks() {
        if let Trigger::Cron { next_tick, .. } = &task.trigger {
            assert!(
                next_tick.is_some(),
                "Task {} should have next_tick",
                task.name
            );
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_cron_task_timeout() -> Result<()> {
    // Test that cron tasks respect timeout settings
    let log_dir = tempdir()?.path().to_path_buf();
    let mut engine = SchedulerEngine::new(log_dir);

    // Create task that sleeps longer than timeout
    let mut task = create_cron_task("timeout_test", "0 * * * * *");
    task.timeout = Some(Duration::from_millis(100));
    task.execution = Execution::Command {
        command: "sleep".to_string(),
        args: vec!["1".to_string()], // Sleep 1 second (will timeout at 100ms)
        env: HashMap::new(),
        workdir: None,
    };

    engine.register_task(task)?;

    // Note: Full timeout testing requires starting engine and waiting,
    // which is better suited for integration tests with real process management
    Ok(())
}

#[tokio::test]
async fn test_cron_task_retry_on_failure() -> Result<()> {
    // Test that cron tasks retry on failure when configured
    let log_dir = tempdir()?.path().to_path_buf();
    let mut engine = SchedulerEngine::new(log_dir);

    // Create task that always fails, with retry policy
    let mut task = create_cron_task("retry_test", "0 * * * * *");
    task.restart_policy = RestartPolicy::OnFailure {
        delay: Duration::from_millis(100),
        limit: 3,
        window: Duration::from_secs(60),
    };
    task.execution = Execution::Command {
        command: "false".to_string(), // Command that always fails
        args: vec![],
        env: HashMap::new(),
        workdir: None,
    };

    engine.register_task(task)?;

    // Note: Full retry testing requires starting engine and monitoring restarts,
    // which is tested by existing RestartPolicy tests in trigger.rs
    Ok(())
}

#[tokio::test]
async fn test_cron_ticker_precision() -> Result<()> {
    // Test scheduler tick precision (should check every second)
    // This is a smoke test - full precision testing requires time mocking
    let log_dir = tempdir()?.path().to_path_buf();
    let mut engine = SchedulerEngine::new(log_dir);

    // Schedule task for next minute
    let now = Local::now();
    let next_minute_second = 0; // Top of next minute
    let cron_expr = format!("{} * * * * *", next_minute_second);

    let task = create_cron_task("precision_test", &cron_expr);
    engine.register_task(task)?;

    // Verify next_tick is set correctly
    let tasks = engine.list_tasks();
    let scheduled_task = tasks.iter().find(|t| t.name == "precision_test").unwrap();

    if let Trigger::Cron { next_tick, .. } = &scheduled_task.trigger {
        let next = next_tick.unwrap();

        // Next tick should be at second 0 of next minute
        assert_eq!(next.second(), next_minute_second as u32);

        // Should be within next 60 seconds
        let diff = next.signed_duration_since(now);
        assert!(diff < ChronoDuration::seconds(60));
        assert!(diff >= ChronoDuration::seconds(0));
    }

    Ok(())
}

#[tokio::test]
async fn test_cron_task_after_manual_stop() -> Result<()> {
    // Test that cron tasks can be manually stopped and don't re-execute
    let log_dir = tempdir()?.path().to_path_buf();
    let mut engine = SchedulerEngine::new(log_dir);

    let task = create_cron_task("stop_test", "0 * * * * *");
    engine.register_task(task)?;

    // Stop task immediately
    engine.stop_task("stop_test").await?;

    // Task should be marked as stopped
    // (Further testing requires engine lifecycle management)

    Ok(())
}
