use super::GitError;
use git2::{Commit, Oid, Repository};

use std::path::{Path, PathBuf};

/// Git 版本管理核心
///
/// 负责配置文件的 Git 版本控制:
/// - 仓库初始化
/// - 自动暂存变更
/// - 手动提交
/// - 差异查看
/// - 提交历史
pub struct GitVersioning {
    repo: Repository,
    config_dir: PathBuf,
}

impl GitVersioning {
    /// 初始化 Git 仓库
    ///
    /// 如果仓库已存在则打开,否则创建新仓库并设置初始配置
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

    /// 设置 Git 配置
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

    /// 创建初始提交
    fn create_initial_commit(repo: &Repository, config_dir: &Path) -> Result<(), GitError> {
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
            &[], // 没有父提交
        )?;

        Ok(())
    }

    /// 自动暂存所有配置文件的变更
    pub fn auto_stage(&mut self) -> Result<(), GitError> {
        let mut index = self.repo.index()?;

        // 添加所有修改的配置文件
        index.add_all(["."].iter(), git2::IndexAddOption::DEFAULT, None)?;

        index.write()?;

        tracing::debug!("Auto-staged configuration changes");
        Ok(())
    }

    /// 检查是否有暂存的变更
    pub fn has_staged_changes(&self) -> Result<bool, GitError> {
        let head = self.repo.head()?.peel_to_tree()?;
        let index = self.repo.index()?;
        let diff = self
            .repo
            .diff_tree_to_index(Some(&head), Some(&index), None)?;

        Ok(diff.deltas().len() > 0)
    }

    /// 提交暂存的变更
    ///
    /// # Arguments
    /// * `message` - 提交消息
    /// * `author` - 可选的作者信息 (格式: "Name <email>")
    pub fn commit(&self, message: &str, author: Option<&str>) -> Result<Oid, GitError> {
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
        let oid = self
            .repo
            .commit(Some("HEAD"), &sig, &sig, message, &tree, &[&parent_commit])?;

        tracing::info!("Committed changes: {} ({})", message, oid);
        Ok(oid)
    }

    /// 解析作者签名字符串
    fn parse_signature(&self, author_str: &str) -> Result<git2::Signature<'_>, GitError> {
        // 解析格式: "Name <email>"
        let parts: Vec<&str> = author_str.split('<').collect();
        if parts.len() != 2 {
            return Err(GitError::InvalidSignature(author_str.to_string()));
        }

        let name = parts[0].trim();
        let email = parts[1].trim_end_matches('>').trim();

        Ok(git2::Signature::now(name, email)?)
    }

    /// 查看暂存的变更差异
    pub fn diff_staged(&self) -> Result<String, GitError> {
        let head = self.repo.head()?.peel_to_tree()?;
        let index = self.repo.index()?;
        let diff = self
            .repo
            .diff_tree_to_index(Some(&head), Some(&index), None)?;

        let mut output = String::new();
        diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
            output.push_str(&String::from_utf8_lossy(line.content()));
            true
        })?;

        Ok(output)
    }

    /// 获取提交历史
    ///
    /// # Arguments
    /// * `limit` - 最多返回的提交数量
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

    /// 回滚到指定版本
    ///
    /// # Arguments
    /// * `target` - 回滚目标
    /// * `dry_run` - 是否只查看影响而不实际执行
    pub fn rollback(
        &mut self,
        target: RollbackTarget,
        dry_run: bool,
    ) -> Result<RollbackResult, GitError> {
        // 1. 解析目标提交
        let target_commit = match target {
            RollbackTarget::Commit(hash) => self.repo.find_commit(Oid::from_str(&hash)?)?,
            RollbackTarget::Previous => {
                let head = self.repo.head()?.peel_to_commit()?;
                self.repo.find_commit(head.parent_id(0)?)?
            }
            RollbackTarget::Time(timestamp) => self.find_commit_at_time(timestamp)?,
        };

        // 2. 计算变更
        let current_tree = self.repo.head()?.peel_to_tree()?;
        let target_tree = target_commit.tree()?;
        let diff = self
            .repo
            .diff_tree_to_tree(Some(&current_tree), Some(&target_tree), None)?;

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
            target_commit
                .message()
                .unwrap_or("")
                .lines()
                .next()
                .unwrap_or(""),
        );

        // 4. 创建回滚提交
        let head = self.repo.head()?.peel_to_commit()?;
        self.repo
            .commit(Some("HEAD"), &sig, &sig, &message, &target_tree, &[&head])?;

        tracing::info!("Rolled back to {}", target_commit.id());
        Ok(result)
    }

    /// 查找指定时间点的提交
    fn find_commit_at_time(&self, timestamp: i64) -> Result<Commit<'_>, GitError> {
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

    /// 格式化差异输出
    fn format_diff(diff: &git2::Diff) -> Result<String, GitError> {
        let mut output = String::new();
        diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
            output.push_str(&String::from_utf8_lossy(line.content()));
            true
        })?;
        Ok(output)
    }

    pub fn get_head_oid(&self) -> Result<Oid, GitError> {
        Ok(self.repo.head()?.peel_to_commit()?.id())
    }

    pub fn resolve_conflict(
        &mut self,
        _conflict: &super::conflict::Conflict,
        resolution: super::conflict::ConflictResolution,
    ) -> Result<(), GitError> {
        match resolution {
            super::conflict::ConflictResolution::Abort => {
                self.repo.reset(
                    &self.repo.head()?.peel(git2::ObjectType::Commit)?,
                    git2::ResetType::Hard,
                    None,
                )?;
                tracing::info!("Aborted conflicting changes");
            }
            super::conflict::ConflictResolution::Force => {
                self.commit("fix: force commit to resolve conflict", None)?;
                tracing::warn!("Force committed changes, may have overwritten other changes");
            }
            super::conflict::ConflictResolution::Merge => {
                return Err(GitError::Io(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "Merge resolution not yet implemented",
                )));
            }
            super::conflict::ConflictResolution::Manual => {
                return Err(GitError::Io(std::io::Error::other(
                    "Manual resolution required",
                )));
            }
        }

        Ok(())
    }

    pub fn get_changed_files_in_commit(&self, commit_id: &str) -> Result<Vec<String>, GitError> {
        let oid = Oid::from_str(commit_id).map_err(|e| {
            GitError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Invalid commit hash: {}", e),
            ))
        })?;

        let commit = self.repo.find_commit(oid)?;
        let tree = commit.tree()?;

        let parent = commit.parent(0).ok();
        let parent_tree = parent.and_then(|p| p.tree().ok());

        let diff = self
            .repo
            .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)?;

        let mut files = Vec::new();
        diff.foreach(
            &mut |delta, _| {
                if let Some(path) = delta.new_file().path() {
                    files.push(path.to_string_lossy().to_string());
                }
                true
            },
            None,
            None,
            None,
        )?;

        Ok(files)
    }

    pub fn get_repo(&self) -> &Repository {
        &self.repo
    }

    pub fn get_config_dir(&self) -> &PathBuf {
        &self.config_dir
    }
}

/// 提交信息
#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub id: String,
    pub author: String,
    pub email: String,
    pub message: String,
    pub time: i64,
}

/// 回滚目标
#[derive(Debug)]
pub enum RollbackTarget {
    Commit(String), // 指定提交哈希
    Previous,       // 上一个提交
    Time(i64),      // 指定时间戳
}

/// 回滚结果
#[derive(Debug, Clone)]
pub struct RollbackResult {
    pub target_commit: String,
    pub target_message: String,
    pub files_changed: usize,
    pub diff: String,
}
