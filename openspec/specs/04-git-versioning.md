# 04 - Git 配置版本管理

> **规格编号**: 04  
> **依赖**: [00-架构总览](./00-architecture-overview.md), [01-配置设计](./01-config-design.md)  
> **相关**: [14-配置管理 API](./14-api-config.md)

---

## 1. 概述

### 1.1 目标

Git 配置版本管理模块负责:
- **配置版本化**:所有配置变更通过 Git 跟踪
- **变更追溯**:查看配置历史和变更原因
- **配置回滚**:快速回滚到历史版本
- **冲突解决**:处理并发配置变更冲突
- **审计日志**:完整的配置变更审计链

### 1.2 设计原则

- **Git 为核心**:利用 Git 成熟的版本管理能力
- **透明操作**:用户可直接使用 git 命令查看历史
- **最小侵入**:不强制用户使用特定工作流
- **自动化优先**:常见操作自动执行,减少手动操作
- **安全第一**:变更前自动备份,支持快速恢复

---

## 2. Git 工作流

### 2.1 配置文件位置

```
.config/mise/
├── config.toml              # mise 配置（用户手动编辑）
└── svcmgr/
    ├── config.toml          # svcmgr 配置（用户手动编辑）
    └── .git/                # Git 仓库（svcmgr 管理）
```

**注意事项**:
- `.config/mise/svcmgr/` 是独立的 Git 仓库
- `.config/mise/config.toml` **不**在 Git 管理范围内（用户可选择性加入父仓库 Git）
- svcmgr 只管理 `.config/mise/svcmgr/` 下的文件

### 2.2 初始化

#### 首次运行自动初始化

```rust
pub struct GitVersioning {
    repo: Repository,  // libgit2 Repository
    config_dir: PathBuf,
}

impl GitVersioning {
    pub fn init(config_dir: &Path) -> Result<Self, GitError> {
        let git_dir = config_dir.join(".git");

        let repo = if git_dir.exists() {
            // 已存在仓库,打开它
            Repository::open(config_dir)?
        } else {
            // 创建新仓库
            let repo = Repository::init(config_dir)?;
            
            // 初始配置
            Self::setup_git_config(&repo)?;
            
            // 创建初始提交
            Self::create_initial_commit(&repo, config_dir)?;
            
            repo
        };

        Ok(Self {
            repo,
            config_dir: config_dir.to_path_buf(),
        })
    }

    fn setup_git_config(repo: &Repository) -> Result<(), GitError> {
        let mut config = repo.config()?;
        
        // 设置默认用户信息（如果未设置）
        if config.get_string("user.name").is_err() {
            config.set_str("user.name", "svcmgr")?;
        }
        if config.get_string("user.email").is_err() {
            config.set_str("user.email", "svcmgr@localhost")?;
        }

        Ok(())
    }

    fn create_initial_commit(
        repo: &Repository,
        config_dir: &Path,
    ) -> Result<(), GitError> {
        // 如果没有配置文件,创建空的配置文件
        let config_path = config_dir.join("config.toml");
        if !config_path.exists() {
            std::fs::write(&config_path, "# svcmgr configuration\n\n")?;
        }

        // 暂存所有文件
        let mut index = repo.index()?;
        index.add_path(Path::new("config.toml"))?;
        index.write()?;

        // 创建提交
        let tree_id = index.write_tree()?;
        let tree = repo.find_tree(tree_id)?;
        let sig = repo.signature()?;

        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "chore: initialize svcmgr configuration",
            &tree,
            &[],  // 没有父提交
        )?;

        Ok(())
    }
}
```

---

## 3. 配置变更管理

### 3.1 自动暂存变更

#### 监控配置文件变化

```rust
use notify::{Watcher, RecursiveMode, Event};

pub struct ConfigWatcher {
    watcher: RecommendedWatcher,
    git: Arc<Mutex<GitVersioning>>,
}

impl ConfigWatcher {
    pub fn new(
        config_dir: &Path,
        git: Arc<Mutex<GitVersioning>>,
    ) -> Result<Self, WatcherError> {
        let (tx, rx) = mpsc::channel();
        
        let mut watcher = notify::recommended_watcher(move |res: Result<Event, _>| {
            if let Ok(event) = res {
                tx.send(event).unwrap();
            }
        })?;

        watcher.watch(config_dir, RecursiveMode::Recursive)?;

        // 启动后台任务处理文件变化
        let git_clone = git.clone();
        tokio::spawn(async move {
            Self::handle_events(rx, git_clone).await;
        });

        Ok(Self { watcher, git })
    }

    async fn handle_events(
        rx: mpsc::Receiver<Event>,
        git: Arc<Mutex<GitVersioning>>,
    ) {
        let mut debouncer = Debouncer::new(Duration::from_secs(2));

        for event in rx {
            if Self::should_handle(&event) {
                debouncer.trigger(async {
                    if let Ok(mut git) = git.lock() {
                        if let Err(e) = git.auto_stage().await {
                            tracing::error!("Failed to auto-stage changes: {}", e);
                        }
                    }
                });
            }
        }
    }

    fn should_handle(event: &Event) -> bool {
        // 只处理配置文件的修改事件
        matches!(event.kind, EventKind::Modify(_)) &&
        event.paths.iter().any(|p| {
            p.extension().map_or(false, |ext| ext == "toml")
        })
    }
}
```

#### 自动暂存

```rust
impl GitVersioning {
    pub async fn auto_stage(&mut self) -> Result<(), GitError> {
        let mut index = self.repo.index()?;
        
        // 添加所有修改的配置文件
        index.add_all(
            ["."].iter(),
            git2::IndexAddOption::DEFAULT,
            None,
        )?;
        
        index.write()?;
        
        tracing::debug!("Auto-staged configuration changes");
        Ok(())
    }

    pub fn has_staged_changes(&self) -> Result<bool, GitError> {
        let head = self.repo.head()?.peel_to_tree()?;
        let index = self.repo.index()?;
        let diff = self.repo.diff_tree_to_index(
            Some(&head),
            Some(&index),
            None,
        )?;

        Ok(diff.deltas().len() > 0)
    }
}
```

### 3.2 手动提交

#### CLI 命令

```bash
# 查看暂存的变更
svcmgr config diff

# 提交变更
svcmgr config commit -m "feat: add new service configuration"

# 查看提交历史
svcmgr config log

# 查看某个提交的详细信息
svcmgr config show <commit-hash>
```

#### 实现

```rust
impl GitVersioning {
    pub fn commit(
        &self,
        message: &str,
        author: Option<&str>,
    ) -> Result<Oid, GitError> {
        // 检查是否有暂存的变更
        if !self.has_staged_changes()? {
            return Err(GitError::NoStagedChanges);
        }

        let mut index = self.repo.index()?;
        let tree_id = index.write_tree()?;
        let tree = self.repo.find_tree(tree_id)?;

        // 获取 HEAD 提交作为父提交
        let parent_commit = self.repo.head()?.peel_to_commit()?;

        // 创建签名
        let sig = if let Some(author_str) = author {
            self.parse_signature(author_str)?
        } else {
            self.repo.signature()?
        };

        // 提交
        let oid = self.repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            message,
            &tree,
            &[&parent_commit],
        )?;

        tracing::info!("Committed changes: {} ({})", message, oid);
        Ok(oid)
    }

    pub fn diff_staged(&self) -> Result<String, GitError> {
        let head = self.repo.head()?.peel_to_tree()?;
        let index = self.repo.index()?;
        let diff = self.repo.diff_tree_to_index(
            Some(&head),
            Some(&index),
            None,
        )?;

        let mut output = String::new();
        diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
            output.push_str(&String::from_utf8_lossy(line.content()));
            true
        })?;

        Ok(output)
    }

    pub fn log(&self, limit: usize) -> Result<Vec<CommitInfo>, GitError> {
        let mut revwalk = self.repo.revwalk()?;
        revwalk.push_head()?;
        revwalk.set_sorting(git2::Sort::TIME)?;

        let mut commits = Vec::new();
        for oid in revwalk.take(limit) {
            let oid = oid?;
            let commit = self.repo.find_commit(oid)?;
            
            commits.push(CommitInfo {
                id: oid.to_string(),
                author: commit.author().name().unwrap_or("").to_string(),
                email: commit.author().email().unwrap_or("").to_string(),
                message: commit.message().unwrap_or("").to_string(),
                time: commit.time().seconds(),
            });
        }

        Ok(commits)
    }
}

#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub id: String,
    pub author: String,
    pub email: String,
    pub message: String,
    pub time: i64,
}
```

---

## 4. 配置回滚

### 4.1 回滚到历史版本

#### CLI 命令

```bash
# 回滚到上一个版本
svcmgr config rollback

# 回滚到指定版本
svcmgr config rollback <commit-hash>

# 回滚到指定时间点
svcmgr config rollback --time "2024-01-15 10:30:00"

# 查看回滚的影响（不实际执行）
svcmgr config rollback <commit-hash> --dry-run
```

#### 实现

```rust
impl GitVersioning {
    pub fn rollback(
        &mut self,
        target: RollbackTarget,
        dry_run: bool,
    ) -> Result<RollbackResult, GitError> {
        // 1. 解析目标提交
        let target_commit = match target {
            RollbackTarget::Commit(hash) => {
                self.repo.find_commit(Oid::from_str(&hash)?)?
            }
            RollbackTarget::Previous => {
                let head = self.repo.head()?.peel_to_commit()?;
                self.repo.find_commit(head.parent_id(0)?)?
            }
            RollbackTarget::Time(timestamp) => {
                self.find_commit_at_time(timestamp)?
            }
        };

        // 2. 计算变更
        let current_tree = self.repo.head()?.peel_to_tree()?;
        let target_tree = target_commit.tree()?;
        let diff = self.repo.diff_tree_to_tree(
            Some(&current_tree),
            Some(&target_tree),
            None,
        )?;

        let result = RollbackResult {
            target_commit: target_commit.id().to_string(),
            target_message: target_commit.message().unwrap_or("").to_string(),
            files_changed: diff.deltas().len(),
            diff: Self::format_diff(&diff)?,
        };

        if dry_run {
            return Ok(result);
        }

        // 3. 执行回滚（创建新提交,不是 reset）
        let sig = self.repo.signature()?;
        let message = format!(
            "revert: rollback to {}\n\nOriginal commit: {}\nReason: Manual rollback",
            &target_commit.id().to_string()[..8],
            target_commit.message().unwrap_or("").lines().next().unwrap_or(""),
        );

        // 4. 创建回滚提交
        let head = self.repo.head()?.peel_to_commit()?;
        self.repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            &message,
            &target_tree,
            &[&head],
        )?;

        // 5. 触发配置重载
        self.notify_config_change().await?;

        tracing::info!("Rolled back to {}", target_commit.id());
        Ok(result)
    }

    fn find_commit_at_time(
        &self,
        timestamp: i64,
    ) -> Result<Commit, GitError> {
        let mut revwalk = self.repo.revwalk()?;
        revwalk.push_head()?;

        for oid in revwalk {
            let oid = oid?;
            let commit = self.repo.find_commit(oid)?;
            
            if commit.time().seconds() <= timestamp {
                return Ok(commit);
            }
        }

        Err(GitError::NoCommitAtTime(timestamp))
    }
}

#[derive(Debug)]
pub enum RollbackTarget {
    Commit(String),      // 指定提交哈希
    Previous,            // 上一个提交
    Time(i64),           // 指定时间戳
}

#[derive(Debug, Clone)]
pub struct RollbackResult {
    pub target_commit: String,
    pub target_message: String,
    pub files_changed: usize,
    pub diff: String,
}
```

### 4.2 配置热重载

#### 重载通知

```rust
impl GitVersioning {
    async fn notify_config_change(&self) -> Result<(), GitError> {
        // 发送配置变更事件
        self.event_bus.publish(Event::ConfigChanged {
            source: ChangeSource::Rollback,
            affected_files: self.get_changed_files()?,
        }).await?;

        Ok(())
    }
}

// 在主应用中监听配置变更事件
pub async fn handle_config_change(event: Event) -> Result<(), AppError> {
    match event {
        Event::ConfigChanged { source, affected_files } => {
            tracing::info!(
                "Configuration changed (source: {:?}), reloading...",
                source
            );

            // 1. 重新解析配置
            let new_config = Config::load().await?;

            // 2. 计算配置差异
            let diff = config_manager.diff(&new_config)?;

            // 3. 应用差异
            for change in diff.changes {
                match change {
                    ConfigChange::ServiceAdded(name) => {
                        scheduler.add_service(&new_config.services[&name]).await?;
                    }
                    ConfigChange::ServiceRemoved(name) => {
                        scheduler.remove_service(&name).await?;
                    }
                    ConfigChange::ServiceModified(name) => {
                        scheduler.restart_service(&name).await?;
                    }
                    // ... 其他变更类型
                }
            }

            tracing::info!("Configuration reloaded successfully");
        }
        _ => {}
    }

    Ok(())
}
```

---

## 5. 冲突解决

### 5.1 并发变更检测

```rust
pub struct ConflictDetector {
    git: Arc<Mutex<GitVersioning>>,
    last_checked: Instant,
}

impl ConflictDetector {
    pub async fn check_conflicts(&mut self) -> Result<Option<Conflict>, GitError> {
        let git = self.git.lock().await;

        // 检查是否有未提交的变更
        if !git.has_staged_changes()? {
            return Ok(None);
        }

        // 检查是否有其他进程的提交
        let head_before = self.last_known_head;
        let head_now = git.repo.head()?.peel_to_commit()?.id();

        if head_before != head_now {
            // HEAD 已经移动,可能有冲突
            return Ok(Some(Conflict {
                our_changes: git.diff_staged()?,
                their_commit: head_now.to_string(),
            }));
        }

        Ok(None)
    }
}

#[derive(Debug)]
pub struct Conflict {
    pub our_changes: String,
    pub their_commit: String,
}
```

### 5.2 冲突解决策略

```rust
#[derive(Debug, Clone)]
pub enum ConflictResolution {
    Abort,              // 放弃我们的变更
    Force,              // 强制提交我们的变更
    Merge,              // 尝试自动合并
    Manual,             // 手动解决
}

impl GitVersioning {
    pub fn resolve_conflict(
        &mut self,
        conflict: &Conflict,
        resolution: ConflictResolution,
    ) -> Result<(), GitError> {
        match resolution {
            ConflictResolution::Abort => {
                // 丢弃暂存的变更
                self.repo.reset(
                    &self.repo.head()?.peel_to_object()?,
                    git2::ResetType::Hard,
                    None,
                )?;
                tracing::info!("Aborted conflicting changes");
            }
            ConflictResolution::Force => {
                // 强制提交（可能覆盖他人变更）
                self.commit(
                    "fix: force commit to resolve conflict",
                    None,
                )?;
                tracing::warn!("Force committed changes, may have overwritten other changes");
            }
            ConflictResolution::Merge => {
                // 尝试三方合并
                self.try_merge(conflict)?;
            }
            ConflictResolution::Manual => {
                // 输出冲突信息,让用户手动处理
                return Err(GitError::ManualResolutionRequired(
                    conflict.clone()
                ));
            }
        }

        Ok(())
    }

    fn try_merge(&mut self, conflict: &Conflict) -> Result<(), GitError> {
        // 使用 libgit2 的合并能力
        let their_commit = self.repo.find_commit(
            Oid::from_str(&conflict.their_commit)?
        )?;
        
        let mut index = self.repo.index()?;
        let annotated = self.repo.find_annotated_commit(their_commit.id())?;
        
        self.repo.merge(&[&annotated], None, None)?;

        if index.has_conflicts() {
            // 有冲突,回退
            self.repo.reset(
                &self.repo.head()?.peel_to_object()?,
                git2::ResetType::Hard,
                None,
            )?;
            return Err(GitError::MergeConflict);
        }

        // 合并成功,提交
        let tree_id = index.write_tree()?;
        let tree = self.repo.find_tree(tree_id)?;
        let head = self.repo.head()?.peel_to_commit()?;
        let sig = self.repo.signature()?;

        self.repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "merge: auto-merge concurrent configuration changes",
            &tree,
            &[&head, &their_commit],
        )?;

        Ok(())
    }
}
```

---

## 6. 审计日志

### 6.1 查询配置历史

```rust
pub struct ConfigAudit {
    git: Arc<Mutex<GitVersioning>>,
}

impl ConfigAudit {
    pub async fn query(
        &self,
        filter: AuditFilter,
    ) -> Result<Vec<AuditEntry>, GitError> {
        let git = self.git.lock().await;
        let commits = git.log(filter.limit.unwrap_or(100))?;

        let mut entries = Vec::new();
        for commit in commits {
            // 应用过滤器
            if !Self::matches_filter(&commit, &filter) {
                continue;
            }

            // 获取变更的文件
            let files = git.get_changed_files_in_commit(&commit.id)?;

            entries.push(AuditEntry {
                commit_id: commit.id,
                author: commit.author,
                email: commit.email,
                timestamp: commit.time,
                message: commit.message,
                files_changed: files,
            });
        }

        Ok(entries)
    }

    fn matches_filter(commit: &CommitInfo, filter: &AuditFilter) -> bool {
        // 时间范围过滤
        if let Some(after) = filter.after {
            if commit.time < after {
                return false;
            }
        }
        if let Some(before) = filter.before {
            if commit.time > before {
                return false;
            }
        }

        // 作者过滤
        if let Some(ref author) = filter.author {
            if !commit.author.contains(author) {
                return false;
            }
        }

        // 消息过滤
        if let Some(ref message) = filter.message_pattern {
            if !commit.message.contains(message) {
                return false;
            }
        }

        true
    }
}

#[derive(Debug, Default)]
pub struct AuditFilter {
    pub after: Option<i64>,       // 开始时间戳
    pub before: Option<i64>,      // 结束时间戳
    pub author: Option<String>,   // 作者名称
    pub message_pattern: Option<String>, // 消息模式
    pub limit: Option<usize>,     // 最大返回数量
}

#[derive(Debug, Clone)]
pub struct AuditEntry {
    pub commit_id: String,
    pub author: String,
    pub email: String,
    pub timestamp: i64,
    pub message: String,
    pub files_changed: Vec<String>,
}
```

#### CLI 命令

```bash
# 查询最近 20 次配置变更
svcmgr config audit --limit 20

# 查询指定时间范围的变更
svcmgr config audit --after "2024-01-01" --before "2024-01-31"

# 查询指定作者的变更
svcmgr config audit --author "john@example.com"

# 查询包含特定关键词的变更
svcmgr config audit --message "nginx"

# 导出审计日志为 JSON
svcmgr config audit --format json > audit.json
```

---

## 7. 配置备份与恢复

### 7.1 自动备份

```rust
pub struct ConfigBackup {
    git: Arc<Mutex<GitVersioning>>,
    backup_dir: PathBuf,
}

impl ConfigBackup {
    pub async fn create_backup(
        &self,
        tag: Option<&str>,
    ) -> Result<String, BackupError> {
        let git = self.git.lock().await;

        // 创建 Git tag
        let head = git.repo.head()?.peel_to_commit()?;
        let tag_name = tag.unwrap_or_else(|| {
            format!("backup-{}", chrono::Utc::now().format("%Y%m%d-%H%M%S"))
        });

        let sig = git.repo.signature()?;
        git.repo.tag(
            &tag_name,
            &head.into_object(),
            &sig,
            &format!("Backup created at {}", chrono::Utc::now()),
            false,  // 不强制覆盖
        )?;

        // 可选:导出为 tar.gz 归档
        if let Some(ref backup_dir) = self.backup_dir {
            let archive_path = backup_dir.join(format!("{}.tar.gz", tag_name));
            Self::export_archive(&git.config_dir, &archive_path)?;
            tracing::info!("Backup archived to: {}", archive_path.display());
        }

        Ok(tag_name)
    }

    pub async fn restore_backup(
        &self,
        tag: &str,
    ) -> Result<(), BackupError> {
        let mut git = self.git.lock().await;

        // 查找 tag
        let tag_ref = git.repo.find_reference(&format!("refs/tags/{}", tag))?;
        let target = tag_ref.peel_to_commit()?;

        // 重置到 tag 指向的提交
        git.repo.reset(
            &target.into_object(),
            git2::ResetType::Hard,
            None,
        )?;

        // 触发配置重载
        git.notify_config_change().await?;

        tracing::info!("Restored backup: {}", tag);
        Ok(())
    }

    pub fn list_backups(&self) -> Result<Vec<BackupInfo>, BackupError> {
        let git = self.git.lock().unwrap();
        let tags = git.repo.tag_names(Some("backup-*"))?;

        let mut backups = Vec::new();
        for tag in tags.iter().flatten() {
            let tag_ref = git.repo.find_reference(&format!("refs/tags/{}", tag))?;
            let commit = tag_ref.peel_to_commit()?;

            backups.push(BackupInfo {
                name: tag.to_string(),
                commit_id: commit.id().to_string(),
                timestamp: commit.time().seconds(),
                message: commit.message().unwrap_or("").to_string(),
            });
        }

        // 按时间排序
        backups.sort_by_key(|b| -b.timestamp);

        Ok(backups)
    }
}

#[derive(Debug, Clone)]
pub struct BackupInfo {
    pub name: String,
    pub commit_id: String,
    pub timestamp: i64,
    pub message: String,
}
```

#### CLI 命令

```bash
# 创建备份
svcmgr config backup

# 创建带标签的备份
svcmgr config backup --tag "before-upgrade"

# 列出所有备份
svcmgr config backup list

# 恢复备份
svcmgr config backup restore <tag-name>

# 删除备份
svcmgr config backup delete <tag-name>
```

---

## 8. 配置导入导出

### 8.1 导出配置

```rust
impl GitVersioning {
    pub fn export_config(
        &self,
        output: &Path,
        format: ExportFormat,
    ) -> Result<(), GitError> {
        match format {
            ExportFormat::Tar => {
                Self::export_archive(&self.config_dir, output)?;
            }
            ExportFormat::Git => {
                Self::export_git_bundle(&self.repo, output)?;
            }
            ExportFormat::Files => {
                Self::export_files(&self.config_dir, output)?;
            }
        }

        Ok(())
    }

    fn export_git_bundle(
        repo: &Repository,
        output: &Path,
    ) -> Result<(), GitError> {
        // 导出为 Git bundle（包含完整历史）
        let mut revwalk = repo.revwalk()?;
        revwalk.push_head()?;

        let file = std::fs::File::create(output)?;
        // 使用 git bundle create
        std::process::Command::new("git")
            .args(&["bundle", "create", output.to_str().unwrap(), "--all"])
            .current_dir(repo.path().parent().unwrap())
            .output()?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum ExportFormat {
    Tar,       // tar.gz 归档（只包含当前文件）
    Git,       // Git bundle（包含完整历史）
    Files,     // 直接复制文件
}
```

### 8.2 导入配置

```rust
impl GitVersioning {
    pub fn import_config(
        &mut self,
        source: &Path,
        format: ExportFormat,
        merge_strategy: MergeStrategy,
    ) -> Result<(), GitError> {
        match format {
            ExportFormat::Tar => {
                Self::import_archive(source, &self.config_dir)?;
            }
            ExportFormat::Git => {
                Self::import_git_bundle(source, &self.repo)?;
            }
            ExportFormat::Files => {
                Self::import_files(source, &self.config_dir)?;
            }
        }

        // 根据合并策略处理
        match merge_strategy {
            MergeStrategy::Replace => {
                // 直接替换,创建新提交
                self.auto_stage().await?;
                self.commit("chore: import configuration", None)?;
            }
            MergeStrategy::Merge => {
                // 尝试合并
                self.auto_stage().await?;
                // ... 合并逻辑
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum MergeStrategy {
    Replace,    // 直接替换
    Merge,      // 尝试合并
}
```

---

## 9. 错误处理

```rust
#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("Git error: {0}")]
    Git(#[from] git2::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("No staged changes to commit")]
    NoStagedChanges,

    #[error("No commit found at time {0}")]
    NoCommitAtTime(i64),

    #[error("Merge conflict detected")]
    MergeConflict,

    #[error("Manual resolution required: {0:?}")]
    ManualResolutionRequired(Conflict),

    #[error("Invalid commit hash: {0}")]
    InvalidCommitHash(String),
}
```

---

## 10. 性能优化

### 10.1 缓存策略

```rust
pub struct GitCache {
    commit_cache: LruCache<String, Arc<CommitInfo>>,
    diff_cache: LruCache<String, Arc<String>>,
}

impl GitCache {
    pub fn new() -> Self {
        Self {
            commit_cache: LruCache::new(NonZeroUsize::new(100).unwrap()),
            diff_cache: LruCache::new(NonZeroUsize::new(50).unwrap()),
        }
    }

    pub fn get_commit(&mut self, id: &str) -> Option<Arc<CommitInfo>> {
        self.commit_cache.get(id).cloned()
    }

    pub fn cache_commit(&mut self, id: String, info: CommitInfo) {
        self.commit_cache.put(id, Arc::new(info));
    }
}
```

### 10.2 后台操作

- **异步提交**:提交操作在后台线程执行,不阻塞主线程
- **延迟暂存**:文件变更防抖(debounce)2 秒后才暂存
- **批量操作**:合并多个小提交为一个大提交

---

## 11. 测试策略

### 11.1 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_git_repo() {
        let temp_dir = tempfile::tempdir().unwrap();
        let git = GitVersioning::init(temp_dir.path()).unwrap();
        
        assert!(git.config_dir.join(".git").exists());
        assert_eq!(git.log(1).unwrap().len(), 1); // 初始提交
    }

    #[test]
    fn test_commit_changes() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut git = GitVersioning::init(temp_dir.path()).unwrap();

        // 修改配置文件
        std::fs::write(
            temp_dir.path().join("config.toml"),
            "# modified\n"
        ).unwrap();

        git.auto_stage().unwrap();
        let oid = git.commit("test: modify config", None).unwrap();
        
        assert_ne!(oid.to_string(), "");
        assert_eq!(git.log(2).unwrap().len(), 2);
    }

    #[test]
    fn test_rollback() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut git = GitVersioning::init(temp_dir.path()).unwrap();

        // 创建第二个提交
        std::fs::write(
            temp_dir.path().join("config.toml"),
            "# version 2\n"
        ).unwrap();
        git.auto_stage().unwrap();
        git.commit("test: version 2", None).unwrap();

        // 回滚
        git.rollback(RollbackTarget::Previous, false).unwrap();

        // 验证内容已回滚
        let content = std::fs::read_to_string(
            temp_dir.path().join("config.toml")
        ).unwrap();
        assert!(content.contains("# svcmgr configuration"));
    }
}
```

---

## 12. 安全考虑

### 12.1 权限检查

- **文件权限**:配置文件只允许所有者读写(0600)
- **Git 目录权限**:.git 目录只允许所有者访问(0700)

### 12.2 敏感信息处理

```rust
impl GitVersioning {
    fn should_ignore(&self, path: &Path) -> bool {
        let filename = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        // 忽略敏感文件
        matches!(filename,
            ".env" | "*.key" | "*.pem" | "*secret*" | "*password*"
        )
    }
}
```

---

## 13. 相关规格

- [00-架构总览](./00-architecture-overview.md) - 整体架构设计
- [01-配置设计](./01-config-design.md) - 配置文件格式
- [14-配置管理 API](./14-api-config.md) - 配置管理接口
