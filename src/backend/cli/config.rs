use crate::cli::ConfigAction;
use crate::error::Result;
use crate::features::ConfigManager;

pub async fn handle_config_command(action: ConfigAction) -> Result<()> {
    let manager = ConfigManager::default_config()?;

    match action {
        ConfigAction::Init => init_config(&manager).await,
        ConfigAction::Log { limit } => show_log(&manager, limit).await,
        ConfigAction::Show { commit } => show_commit(&manager, &commit).await,
        ConfigAction::Diff { from, to } => show_diff(&manager, &from, &to).await,
        ConfigAction::Rollback { commit } => rollback_to(&manager, &commit).await,
        ConfigAction::Backup => create_backup(&manager).await,
        ConfigAction::Restore { name } => restore_backup(&manager, &name).await,
    }
}

async fn init_config(manager: &ConfigManager) -> Result<()> {
    let info = manager.init().await?;
    println!("✅ 配置目录已初始化: {:?}", info.repo_path);
    if let Some(commit) = info.last_commit {
        println!("📝 初始提交: {} ({})", &commit.id[..8], commit.message);
    }
    Ok(())
}

async fn show_log(manager: &ConfigManager, limit: usize) -> Result<()> {
    let commits = manager.log(limit, None).await?;

    if commits.is_empty() {
        println!("ℹ️  暂无提交记录");
        return Ok(());
    }

    println!("{:<10} {:<50} {:<20} 时间", "提交", "消息", "作者");
    println!("{}", "-".repeat(100));

    for commit in commits {
        let short_hash = &commit.id[..8];
        let timestamp = chrono::DateTime::from_timestamp(commit.timestamp, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| commit.timestamp.to_string());
        println!(
            "{:<10} {:<50} {:<20} {}",
            short_hash, commit.message, commit.author, timestamp
        );
    }

    Ok(())
}

async fn show_commit(manager: &ConfigManager, commit: &str) -> Result<()> {
    let info = manager.show(commit).await?;

    let timestamp = chrono::DateTime::from_timestamp(info.timestamp, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| info.timestamp.to_string());

    println!("提交: {}", info.id);
    println!("作者: {}", info.author);
    println!("时间: {}", timestamp);
    println!("消息: {}", info.message);
    println!("\n变更文件:");
    for file in &info.files {
        println!("  - {}", file);
    }

    // Show diff
    let diff = manager.diff(&format!("{}^", commit), commit, None).await?;
    if !diff.is_empty() {
        println!("\n差异内容:");
        println!("{}", diff);
    }

    Ok(())
}

async fn show_diff(manager: &ConfigManager, from: &str, to: &str) -> Result<()> {
    let diff = manager.diff(from, to, None).await?;

    if diff.is_empty() {
        println!("ℹ️  两个提交之间无差异");
    } else {
        println!("{}", diff);
    }

    Ok(())
}

async fn rollback_to(manager: &ConfigManager, commit: &str) -> Result<()> {
    let new_commit = manager.rollback_commit(commit).await?;
    println!("✅ 已回滚到提交 {}", commit);
    println!("📝 创建新提交: {}", &new_commit[..8]);
    Ok(())
}

async fn create_backup(manager: &ConfigManager) -> Result<()> {
    let backup = manager.backup(None).await?;
    println!("✅ 备份已创建");
    println!("标签名: {}", backup.name);
    println!("时间戳: {}", backup.timestamp);
    println!("提交ID: {}", &backup.commit_id[..8]);
    Ok(())
}

async fn restore_backup(manager: &ConfigManager, name: &str) -> Result<()> {
    let commit_id = manager.restore(name).await?;
    println!("✅ 已从备份恢复: {}", name);
    println!("📝 当前提交: {}", &commit_id[..8]);
    Ok(())
}
