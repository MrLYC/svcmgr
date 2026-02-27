use crate::atoms::TemplateContext;
use crate::cli::CronAction;
use crate::error::Result;
use crate::features::{CrontabTaskManager, TaskConfig};
use std::collections::HashMap;

pub async fn handle_cron_command(action: CronAction) -> Result<()> {
    let manager = CrontabTaskManager::default_config()?;

    match action {
        CronAction::List => list_tasks(&manager).await,
        CronAction::Add {
            id,
            expression,
            command,
            description,
            template,
            var,
        } => {
            add_task(
                &manager,
                id,
                expression,
                command,
                description,
                template,
                var,
            )
            .await
        }
        CronAction::Status { id } => show_status(&manager, id).await,
        CronAction::Update {
            id,
            expression,
            command,
            description,
        } => update_task(&manager, id, expression, command, description).await,
        CronAction::Remove { id, force } => remove_task(&manager, id, force).await,
        CronAction::Next { id, count } => show_next_runs(&manager, id, count).await,
        CronAction::Validate { expression } => validate_expression(&manager, expression).await,
        CronAction::SetEnv { key, value } => set_env(&manager, key, value).await,
        CronAction::GetEnv => get_env(&manager).await,
    }
}

async fn list_tasks(manager: &CrontabTaskManager) -> Result<()> {
    let tasks = manager.list_tasks()?;

    if tasks.is_empty() {
        println!("没有找到管理的 crontab 任务。");
        return Ok(());
    }

    println!(
        "{:<20} {:<15} {:<10} {:<30} 描述",
        "ID", "表达式", "状态", "命令"
    );
    println!("{}", "-".repeat(100));

    for task in tasks {
        let status = if task.enabled { "启用" } else { "禁用" };
        let next_run = task
            .next_run
            .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "N/A".to_string());

        println!(
            "{:<20} {:<15} {:<10} {:<30} {}",
            task.id,
            task.expression,
            status,
            truncate(&task.command, 30),
            task.description
        );
        println!("  下次运行: {}", next_run);
    }

    Ok(())
}

async fn add_task(
    manager: &CrontabTaskManager,
    id: String,
    expression: String,
    command: String,
    description: String,
    template: Option<String>,
    variables: Vec<(String, String)>,
) -> Result<()> {
    let mut context = TemplateContext::new();
    for (key, value) in variables {
        context.insert(&key, value);
    }

    let config = TaskConfig {
        id: Some(id.clone()),
        description,
        expression,
        command,
        env: HashMap::new(),
        enabled: true,
        template,
        variables: context,
    };

    let task_id = manager.create_task(&config)?;
    println!("Crontab 任务 {} 创建成功。", task_id);

    // 显示下次运行时间
    if let Ok(next_runs) = manager.get_next_runs(&task_id, 1) {
        if let Some(next) = next_runs.first() {
            println!("下次运行时间: {}", next.format("%Y-%m-%d %H:%M:%S"));
        }
    }

    Ok(())
}

async fn show_status(manager: &CrontabTaskManager, id: String) -> Result<()> {
    let task = manager.get_task(&id)?;

    println!("\n=== Crontab 任务详情: {} ===", id);
    println!("描述: {}", task.description);
    println!("表达式: {}", task.expression);
    println!("命令: {}", task.command);
    println!("状态: {}", if task.enabled { "启用" } else { "禁用" });

    if !task.env.is_empty() {
        println!("\n环境变量:");
        for (key, value) in &task.env {
            println!("  {}={}", key, value);
        }
    }

    if let Some(next_run) = task.next_run {
        println!("\n下次运行: {}", next_run.to_rfc3339());
    }

    // 显示未来 5 次运行时间
    if let Ok(next_runs) = manager.get_next_runs(&id, 5)
        && !next_runs.is_empty()
    {
        println!("\n未来 5 次运行时间:");
        for (i, time) in next_runs.iter().enumerate() {
            println!("  {}. {}", i + 1, time.format("%Y-%m-%d %H:%M:%S"));
        }
    }

    Ok(())
}

async fn update_task(
    manager: &CrontabTaskManager,
    id: String,
    expression: Option<String>,
    command: Option<String>,
    description: Option<String>,
) -> Result<()> {
    // 获取现有任务
    let existing = manager.get_task(&id)?;

    // 构建更新配置
    let config = TaskConfig {
        id: Some(id.clone()),
        description: description.unwrap_or(existing.description),
        expression: expression.unwrap_or(existing.expression),
        command: command.unwrap_or(existing.command),
        env: existing.env,
        enabled: existing.enabled,
        template: None,
        variables: TemplateContext::new(),
    };

    manager.update_task(&id, &config)?;
    println!("任务 {} 更新成功。", id);

    Ok(())
}

async fn remove_task(manager: &CrontabTaskManager, id: String, force: bool) -> Result<()> {
    if !force {
        println!("确认删除任务 '{}' ? [y/N]: ", id);
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("取消删除。");
            return Ok(());
        }
    }

    manager.delete_task(&id)?;
    println!("任务 {} 已删除。", id);

    Ok(())
}

async fn show_next_runs(manager: &CrontabTaskManager, id: String, count: usize) -> Result<()> {
    let next_runs = manager.get_next_runs(&id, count)?;

    if next_runs.is_empty() {
        println!("无法计算未来运行时间。");
        return Ok(());
    }

    println!("任务 {} 的未来 {} 次运行时间:", id, count);
    for (i, time) in next_runs.iter().enumerate() {
        println!("  {}. {}", i + 1, time.format("%Y-%m-%d %H:%M:%S %Z"));
    }

    Ok(())
}

async fn validate_expression(manager: &CrontabTaskManager, expression: String) -> Result<()> {
    match manager.validate_expression(&expression) {
        Ok(true) => {
            println!("✓ Cron 表达式有效: {}", expression);

            // 尝试显示下次运行时间
            let temp_config = TaskConfig {
                id: Some("_temp_".to_string()),
                description: "临时验证".to_string(),
                expression: expression.clone(),
                command: "echo test".to_string(),
                env: HashMap::new(),
                enabled: true,
                template: None,
                variables: TemplateContext::new(),
            };

            if let Ok(temp_id) = manager.create_task(&temp_config) {
                if let Ok(next_runs) = manager.get_next_runs(&temp_id, 3) {
                    println!("\n示例运行时间:");
                    for (i, time) in next_runs.iter().enumerate() {
                        println!("  {}. {}", i + 1, time.format("%Y-%m-%d %H:%M:%S"));
                    }
                }
                let _ = manager.delete_task(&temp_id);
            }
        }
        Ok(false) => {
            println!("✗ Cron 表达式无效: {}", expression);
        }
        Err(e) => {
            println!("✗ 验证失败: {}", e);
        }
    }

    Ok(())
}

async fn set_env(manager: &CrontabTaskManager, key: String, value: String) -> Result<()> {
    manager.set_env(&key, &value)?;
    println!("环境变量已设置: {}={}", key, value);
    Ok(())
}

async fn get_env(manager: &CrontabTaskManager) -> Result<()> {
    let env = manager.get_env()?;

    if env.is_empty() {
        println!("未设置环境变量。");
        return Ok(());
    }

    println!("当前环境变量:");
    for (key, value) in env {
        println!("  {}={}", key, value);
    }

    Ok(())
}

/// 截断字符串到指定长度
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
