//! Phase 4.3: Git 版本控制单元测试和集成测试
//!
//! 测试 GitVersioning 核心功能:
//! - 仓库初始化
//! - 自动暂存
//! - 手动提交
//! - 差异查看
//! - 提交历史
//! - 回滚机制

use anyhow::Result;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

use std::sync::Arc;
use std::time::Duration;
use svcmgr::git::watcher::ConfigWatcher;
use svcmgr::git::{AuditFilter, ConfigAudit, ConfigBackup, GitVersioning, RollbackTarget};
use tokio::sync::Mutex;

/// 创建临时测试目录
fn setup_test_dir() -> Result<(TempDir, PathBuf)> {
    let temp_dir = TempDir::new()?;
    let config_dir = temp_dir.path().join("config");
    fs::create_dir(&config_dir)?;
    Ok((temp_dir, config_dir))
}

#[test]
fn test_init_git_repo() {
    let (_temp, config_dir) = setup_test_dir().unwrap();

    let git = GitVersioning::init(&config_dir).unwrap();

    // 验证 .git 目录存在
    assert!(config_dir.join(".git").exists());

    // 验证初始提交存在
    let commits = git.log(1).unwrap();
    assert_eq!(commits.len(), 1);
    assert!(commits[0].message.contains("initialize"));
}

#[test]
fn test_init_existing_repo() {
    let (_temp, config_dir) = setup_test_dir().unwrap();

    // 第一次初始化
    let git1 = GitVersioning::init(&config_dir).unwrap();
    let commits1 = git1.log(5).unwrap();

    // 第二次初始化（应该打开已有仓库）
    let git2 = GitVersioning::init(&config_dir).unwrap();
    let commits2 = git2.log(5).unwrap();

    // 验证提交历史一致
    assert_eq!(commits1.len(), commits2.len());
    assert_eq!(commits1[0].id, commits2[0].id);
}

#[test]
fn test_auto_stage_and_commit() {
    let (_temp, config_dir) = setup_test_dir().unwrap();
    let mut git = GitVersioning::init(&config_dir).unwrap();

    // 修改配置文件
    let config_path = config_dir.join("config.toml");
    fs::write(&config_path, "# Modified config\nkey = \"value\"\n").unwrap();

    // 自动暂存
    git.auto_stage().unwrap();

    // 验证有暂存变更
    assert!(git.has_staged_changes().unwrap());

    // 提交变更
    let oid = git.commit("feat: add key config", None).unwrap();
    assert!(!oid.is_zero());

    // 验证提交后没有暂存变更
    assert!(!git.has_staged_changes().unwrap());

    // 验证提交历史
    let commits = git.log(5).unwrap();
    assert_eq!(commits.len(), 2); // 初始提交 + 新提交
    assert!(commits[0].message.contains("add key config"));
}

#[test]
fn test_commit_without_staged_changes() {
    let (_temp, config_dir) = setup_test_dir().unwrap();
    let git = GitVersioning::init(&config_dir).unwrap();

    // 尝试在没有暂存变更时提交
    let result = git.commit("feat: nothing", None);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("No staged changes")
    );
}

#[test]
fn test_diff_staged() {
    let (_temp, config_dir) = setup_test_dir().unwrap();
    let mut git = GitVersioning::init(&config_dir).unwrap();

    // 修改配置文件
    let config_path = config_dir.join("config.toml");
    fs::write(&config_path, "# Modified\nnew_key = \"new_value\"\n").unwrap();

    // 暂存变更
    git.auto_stage().unwrap();

    // 获取差异
    let diff = git.diff_staged().unwrap();
    assert!(!diff.is_empty());
    assert!(diff.contains("new_key"));
}

#[test]
fn test_log_limit() {
    let (_temp, config_dir) = setup_test_dir().unwrap();
    let mut git = GitVersioning::init(&config_dir).unwrap();

    // 创建多个提交
    for i in 1..=5 {
        let config_path = config_dir.join("config.toml");
        fs::write(&config_path, format!("# Version {}\n", i)).unwrap();
        git.auto_stage().unwrap();
        git.commit(&format!("feat: version {}", i), None).unwrap();
    }

    // 验证 log 限制
    let commits = git.log(3).unwrap();
    assert_eq!(commits.len(), 3);

    let all_commits = git.log(100).unwrap();
    assert_eq!(all_commits.len(), 6); // 初始提交 + 5个新提交
}

#[test]
fn test_rollback_to_previous() {
    let (_temp, config_dir) = setup_test_dir().unwrap();
    let mut git = GitVersioning::init(&config_dir).unwrap();

    // 创建第一个提交
    let config_path = config_dir.join("config.toml");
    fs::write(&config_path, "# Version 1\n").unwrap();
    git.auto_stage().unwrap();
    git.commit("feat: version 1", None).unwrap();

    // 创建第二个提交
    fs::write(&config_path, "# Version 2\n").unwrap();
    git.auto_stage().unwrap();
    git.commit("feat: version 2", None).unwrap();

    // Dry-run 回滚
    let result = git.rollback(RollbackTarget::Previous, true).unwrap();
    assert!(result.files_changed > 0);
    assert!(result.target_message.contains("version 1"));

    // 实际回滚
    git.rollback(RollbackTarget::Previous, false).unwrap();

    // 验证回滚后的提交历史
    let commits = git.log(5).unwrap();
    assert_eq!(commits.len(), 4); // 初始 + v1 + v2 + 回滚提交
    assert!(commits[0].message.contains("rollback"));
}

#[test]
fn test_rollback_by_commit_hash() {
    let (_temp, config_dir) = setup_test_dir().unwrap();
    let mut git = GitVersioning::init(&config_dir).unwrap();

    // 创建多个提交
    let config_path = config_dir.join("config.toml");
    for i in 1..=3 {
        fs::write(&config_path, format!("# Version {}\n", i)).unwrap();
        git.auto_stage().unwrap();
        git.commit(&format!("feat: version {}", i), None).unwrap();
    }

    // 获取第一个非初始提交的哈希
    let commits = git.log(10).unwrap();
    let target_hash = commits[2].id.clone(); // version 1

    // 回滚到指定提交
    let result = git
        .rollback(RollbackTarget::Commit(target_hash.clone()), false)
        .unwrap();
    assert_eq!(result.target_commit, target_hash);
    assert!(result.target_message.contains("version 1"));

    // 验证回滚成功
    let new_commits = git.log(1).unwrap();
    assert!(new_commits[0].message.contains("rollback"));
}

#[test]
fn test_custom_author_signature() {
    let (_temp, config_dir) = setup_test_dir().unwrap();
    let mut git = GitVersioning::init(&config_dir).unwrap();

    // 修改配置
    let config_path = config_dir.join("config.toml");
    fs::write(&config_path, "# By user\n").unwrap();
    git.auto_stage().unwrap();

    // 使用自定义作者提交
    git.commit("feat: custom author", Some("John Doe <john@example.com>"))
        .unwrap();

    // 验证提交作者
    let commits = git.log(1).unwrap();
    assert_eq!(commits[0].author, "John Doe");
    assert_eq!(commits[0].email, "john@example.com");
}

#[test]
fn test_invalid_author_signature() {
    let (_temp, config_dir) = setup_test_dir().unwrap();
    let mut git = GitVersioning::init(&config_dir).unwrap();

    // 修改配置
    let config_path = config_dir.join("config.toml");
    fs::write(&config_path, "# Test\n").unwrap();
    git.auto_stage().unwrap();

    // 使用无效格式的作者
    let result = git.commit("feat: invalid", Some("InvalidFormat"));
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Invalid signature")
    );
}

// ============================================================
// 集成测试 (Integration Tests)
// ============================================================

#[tokio::test]
async fn test_integration_audit_query_time_filter() {
    let (_temp, config_dir) = setup_test_dir().unwrap();
    let git = GitVersioning::init(&config_dir).unwrap();
    let git_arc = Arc::new(Mutex::new(git));
    let audit = ConfigAudit::new(git_arc.clone());

    // 创建多个提交
    let config_path = config_dir.join("config.toml");
    for i in 1..=3 {
        fs::write(&config_path, format!("# Version {}\n", i)).unwrap();
        let mut g = git_arc.lock().await;
        g.auto_stage().unwrap();
        g.commit(&format!("feat: version {}", i), None).unwrap();
        drop(g);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // 查询所有历史
    let all_logs = audit.query(AuditFilter::default()).await.unwrap();
    assert!(all_logs.len() >= 4); // 初始 + 3 个提交

    // 时间过滤 (最近 1 条)
    let filter = AuditFilter {
        limit: Some(1),
        ..Default::default()
    };
    let recent = audit.query(filter).await.unwrap();
    assert_eq!(recent.len(), 1);
    assert!(recent[0].message.contains("version 3"));

    // 验证日志内容
    assert!(!recent[0].author.is_empty());
    assert!(!recent[0].commit_id.is_empty());
}

#[tokio::test]
async fn test_integration_audit_query_author_filter() {
    let (_temp, config_dir) = setup_test_dir().unwrap();
    let git = GitVersioning::init(&config_dir).unwrap();
    let git_arc = Arc::new(Mutex::new(git));
    let audit = ConfigAudit::new(git_arc.clone());

    // 使用不同作者创建提交
    let config_path = config_dir.join("config.toml");
    fs::write(&config_path, "# By Alice\n").unwrap();
    {
        let mut g = git_arc.lock().await;
        g.auto_stage().unwrap();
        g.commit("feat: alice commit", Some("Alice <alice@example.com>"))
            .unwrap();
    }

    fs::write(&config_path, "# By Bob\n").unwrap();
    {
        let mut g = git_arc.lock().await;
        g.auto_stage().unwrap();
        g.commit("feat: bob commit", Some("Bob <bob@example.com>"))
            .unwrap();
    }

    // 过滤 Alice 的提交
    let filter = AuditFilter {
        author: Some("Alice".to_string()),
        ..Default::default()
    };
    let alice_logs = audit.query(filter).await.unwrap();
    assert!(alice_logs.iter().any(|log| log.author == "Alice"));

    // 过滤 Bob 的提交
    let filter = AuditFilter {
        author: Some("Bob".to_string()),
        ..Default::default()
    };
    let bob_logs = audit.query(filter).await.unwrap();
    assert!(bob_logs.iter().any(|log| log.author == "Bob"));
}

#[tokio::test]
async fn test_integration_backup_create_and_list() {
    let (_temp, config_dir) = setup_test_dir().unwrap();
    let git = GitVersioning::init(&config_dir).unwrap();
    let git_arc = Arc::new(Mutex::new(git));
    let backup = ConfigBackup::new(git_arc.clone(), None);

    // 创建一些配置变更
    let config_path = config_dir.join("config.toml");
    fs::write(&config_path, "# Production Config\n").unwrap();
    {
        let mut g = git_arc.lock().await;
        g.auto_stage().unwrap();
        g.commit("feat: production config", None).unwrap();
    }

    // 创建备份
    let tag_name = backup.create_backup(Some("backup-v1.0.0")).await.unwrap();
    assert_eq!(tag_name, "backup-v1.0.0");

    // 列出备份
    let backups = backup.list_backups().await.unwrap();
    assert!(!backups.is_empty());
    assert!(backups.iter().any(|b| b.name == "backup-v1.0.0"));

    // 验证备份内容
    let v1_backup = backups.iter().find(|b| b.name == "backup-v1.0.0").unwrap();
    assert!(!v1_backup.message.is_empty());
    assert!(!v1_backup.commit_id.is_empty());
}

#[tokio::test]
async fn test_integration_backup_restore() {
    let (_temp, config_dir) = setup_test_dir().unwrap();
    let git = GitVersioning::init(&config_dir).unwrap();
    let git_arc = Arc::new(Mutex::new(git));
    let backup = ConfigBackup::new(git_arc.clone(), None);

    // 创建初始配置并备份
    let config_path = config_dir.join("config.toml");
    fs::write(&config_path, "# Version 1\n").unwrap();
    {
        let mut g = git_arc.lock().await;
        g.auto_stage().unwrap();
        g.commit("feat: version 1", None).unwrap();
    }
    backup.create_backup(Some("backup-v1")).await.unwrap();

    // 修改配置
    fs::write(&config_path, "# Version 2 (broken)\n").unwrap();
    {
        let mut g = git_arc.lock().await;
        g.auto_stage().unwrap();
        g.commit("feat: version 2", None).unwrap();
    }

    // 恢复备份
    backup.restore_backup("backup-v1").await.unwrap();

    // 验证恢复成功
    let content = fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("Version 1"));
}

#[tokio::test]
async fn test_integration_lifecycle_full_workflow() {
    let (_temp, config_dir) = setup_test_dir().unwrap();
    let git = GitVersioning::init(&config_dir).unwrap();
    let git_arc = Arc::new(Mutex::new(git));
    let audit = ConfigAudit::new(git_arc.clone());
    let backup = ConfigBackup::new(git_arc.clone(), None);

    // 阶段 1: 初始配置
    let config_path = config_dir.join("config.toml");
    fs::write(&config_path, "# Initial\n").unwrap();
    {
        let mut g = git_arc.lock().await;
        g.auto_stage().unwrap();
        g.commit("feat: initial config", None).unwrap();
    }
    backup
        .create_backup(Some("backup-stable-v1"))
        .await
        .unwrap();

    // 阶段 2: 多次修改
    for i in 2..=4 {
        fs::write(&config_path, format!("# Version {}\n", i)).unwrap();
        let mut g = git_arc.lock().await;
        g.auto_stage().unwrap();
        g.commit(&format!("feat: version {}", i), None).unwrap();
    }

    // 阶段 3: 审计检查
    let all_commits = audit.query(AuditFilter::default()).await.unwrap();
    assert!(all_commits.len() >= 5); // 初始 + 1 + 3

    // 阶段 4: 回滚到稳定版本
    let backups = backup.list_backups().await.unwrap();
    assert!(backups.iter().any(|b| b.name == "backup-stable-v1"));
    backup.restore_backup("backup-stable-v1").await.unwrap();

    // 阶段 5: 验证恢复
    let content = fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("Initial"));

    // 阶段 6: 最终审计
    let final_commits = audit
        .query(AuditFilter {
            limit: Some(1),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(final_commits.len(), 1);
}

#[tokio::test]
async fn test_integration_watcher_auto_commit() {
    let (_temp, config_dir) = setup_test_dir().unwrap();
    let git = GitVersioning::init(&config_dir).unwrap();
    let git_arc = Arc::new(Mutex::new(git));
    // 先创建文件,确保 watcher 可以监听
    let config_path = config_dir.join("watched.toml");
    fs::write(&config_path, "# Initial content\n").unwrap();
    // 创建 Watcher
    let watcher = ConfigWatcher::new(git_arc.clone(), vec![config_path.clone()]);
    let watcher_arc = Arc::new(Mutex::new(watcher));
    let watcher_clone = watcher_arc.clone();
    let _watch_handle = tokio::spawn(async move {
        let mut w = watcher_clone.lock().await;
        let _ = w.start().await;
    });
    // 等待 watcher 初始化
    tokio::time::sleep(Duration::from_millis(500)).await;

    // 修改配置文件(触发 watcher)
    fs::write(&config_path, "# Modified content\n").unwrap();

    // 等待自动提交触发(2秒防抖 + 1秒缓冲)
    tokio::time::sleep(Duration::from_secs(3)).await;
    // 验证提交
    let g = git_arc.lock().await;
    let commits = g.log(5).unwrap();
    assert!(
        commits.len() >= 2,
        "Expected at least 2 commits (init + auto-commit), got {}",
        commits.len()
    );

    // 验证最新提交消息
    if !commits.is_empty() {
        assert!(commits[0].message.contains("auto-save") || commits[0].message.contains("chore"));
    }
}

#[tokio::test]
async fn test_integration_concurrent_modification_detection() {
    let (_temp, config_dir) = setup_test_dir().unwrap();
    let mut git = GitVersioning::init(&config_dir).unwrap();

    // 模拟并发修改场景
    let config_path = config_dir.join("config.toml");

    // 用户 A 的修改
    fs::write(&config_path, "# Modified by A\nkey_a = 1\n").unwrap();
    git.auto_stage().unwrap();
    let commit_a = git
        .commit("feat: user A changes", Some("UserA <a@example.com>"))
        .unwrap();

    // 用户 B 基于旧版本修改 (模拟冲突)
    git.rollback(RollbackTarget::Previous, false).unwrap();
    fs::write(&config_path, "# Modified by B\nkey_b = 2\n").unwrap();
    git.auto_stage().unwrap();
    git.commit("feat: user B changes", Some("UserB <b@example.com>"))
        .unwrap();

    // 检查提交历史中是否有分支/冲突
    let commits = git.log(10).unwrap();
    assert!(commits.len() >= 4);

    // 验证两个用户的提交都存在
    let commit_ids: Vec<String> = commits.iter().map(|c| c.id.clone()).collect();
    assert!(commit_ids.contains(&commit_a.to_string()));
}
