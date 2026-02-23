use crate::atoms::git::{CommitId, CommitInfo, GitAtom, RepoStatus};
use crate::error::{Error, Result};
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigInfo {
    pub repo_path: PathBuf,
    pub status: RepoStatus,
    pub last_commit: Option<CommitInfo>,
}

#[derive(Debug, Clone)]
pub struct BackupInfo {
    pub name: String,
    pub timestamp: String,
    pub commit_id: CommitId,
}

pub struct ConfigManager {
    git: GitAtom,
    config_dir: PathBuf,
}

impl ConfigManager {
    pub fn new(config_dir: PathBuf) -> Self {
        let git = GitAtom::new(config_dir.clone());
        Self { git, config_dir }
    }

    pub fn default_config() -> Result<Self> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| Error::Other("Config directory not found".into()))?
            .join("svcmgr")
            .join("managed");

        Ok(Self::new(config_dir))
    }

    /// Initialize configuration repository
    pub async fn init(&self) -> Result<ConfigInfo> {
        let status = self.git.init_repo().await?;

        // Create subdirectories
        for subdir in &["supervisor", "nginx", "cloudflared", "mise", "templates"] {
            let dir = self.config_dir.join(subdir);
            std::fs::create_dir_all(&dir)?;

            // Create .gitkeep to track empty directories
            let gitkeep = dir.join(".gitkeep");
            if !gitkeep.exists() {
                std::fs::write(&gitkeep, "")?;
            }
        }

        // Initial commit if no commits yet
        let last_commit = if status.head.is_none() {
            let _commit_id = self
                .git
                .commit("[svcmgr] Initialize configuration repository", &[])
                .await?;

            let commits = self.git.log(1, None).await?;
            commits.into_iter().next()
        } else {
            let commits = self.git.log(1, None).await?;
            commits.into_iter().next()
        };

        Ok(ConfigInfo {
            repo_path: self.config_dir.clone(),
            status,
            last_commit,
        })
    }

    /// Set configuration directory (for testing or custom paths)
    pub fn set_config_dir(&mut self, dir: PathBuf) {
        self.config_dir = dir.clone();
        self.git = GitAtom::new(dir);
    }

    /// Auto-commit configuration changes
    pub async fn auto_commit(
        &self,
        module: &str,
        resource: &str,
        action: &str,
        name: &str,
    ) -> Result<CommitId> {
        let message = format!("{}({}): {} {}", module, resource, action, name);
        self.git.commit(&message, &[]).await
    }

    /// Get configuration history
    pub async fn log(&self, limit: usize, path: Option<&Path>) -> Result<Vec<CommitInfo>> {
        self.git.log(limit, path).await
    }

    /// Show specific commit
    pub async fn show(&self, commit_id: &str) -> Result<CommitInfo> {
        let commits = self.git.log(1000, None).await?;
        commits
            .into_iter()
            .find(|c| c.id.starts_with(commit_id))
            .ok_or_else(|| Error::Other(format!("Commit {} not found", commit_id)))
    }

    /// Show diff between commits
    pub async fn diff(&self, from: &str, to: &str, path: Option<&Path>) -> Result<String> {
        self.git.diff(from, to, path).await
    }

    /// Rollback file to specific commit
    pub async fn rollback_file(&self, file: &Path, commit: &str) -> Result<()> {
        self.git.checkout_file(commit, file).await
    }

    /// Rollback entire commit
    pub async fn rollback_commit(&self, commit_id: &str) -> Result<CommitId> {
        self.git.revert(commit_id).await
    }

    /// Create backup with tag
    pub async fn backup(&self, name: Option<&str>) -> Result<BackupInfo> {
        let timestamp = Local::now().format("%Y%m%d-%H%M%S").to_string();
        let backup_name = if let Some(n) = name {
            format!("backup-{}-{}", timestamp, n)
        } else {
            format!("backup-{}", timestamp)
        };

        // Get current HEAD commit
        let commits = self.git.log(1, None).await?;
        let commit = commits
            .first()
            .ok_or_else(|| Error::Other("No commits to backup".into()))?;

        // Create tag using git2
        let repo = git2::Repository::open(&self.config_dir)?;
        let obj = repo.revparse_single("HEAD")?;
        let signature = git2::Signature::now("svcmgr", "svcmgr@localhost")?;
        repo.tag(&backup_name, &obj, &signature, &backup_name, false)?;

        Ok(BackupInfo {
            name: backup_name,
            timestamp,
            commit_id: commit.id.clone(),
        })
    }

    /// List all backups
    pub async fn list_backups(&self) -> Result<Vec<BackupInfo>> {
        let repo = git2::Repository::open(&self.config_dir)?;
        let tags = repo.tag_names(Some("backup-*"))?;

        let mut backups = Vec::new();
        for tag in tags.iter().flatten() {
            let reference = repo.find_reference(&format!("refs/tags/{}", tag))?;
            let obj = reference.peel_to_commit()?;
            let commit_id = obj.id().to_string();

            // Extract timestamp from tag name
            let parts: Vec<&str> = tag.split('-').collect();
            let timestamp = if parts.len() >= 3 {
                format!("{}-{}", parts[1], parts[2])
            } else {
                "unknown".to_string()
            };

            backups.push(BackupInfo {
                name: tag.to_string(),
                timestamp,
                commit_id,
            });
        }

        Ok(backups)
    }

    /// Restore from backup
    pub async fn restore(&self, backup_name: &str) -> Result<CommitId> {
        let repo = git2::Repository::open(&self.config_dir)?;

        // Find tag
        let reference = repo.find_reference(&format!("refs/tags/{}", backup_name))?;
        let commit = reference.peel_to_commit()?;

        // Reset to backup commit
        repo.reset(commit.as_object(), git2::ResetType::Hard, None)?;

        Ok(commit.id().to_string())
    }

    /// Add remote repository
    pub async fn add_remote(&self, name: &str, url: &str) -> Result<()> {
        let repo = git2::Repository::open(&self.config_dir)?;
        repo.remote(name, url)?;
        Ok(())
    }

    /// Push to remote
    pub async fn push(&self, remote: &str, branch: &str) -> Result<()> {
        self.git.push(remote, branch).await
    }

    /// Pull from remote
    pub async fn pull(&self, remote: &str, branch: &str) -> Result<()> {
        self.git.pull(remote, branch).await
    }

    /// Get repository status
    pub async fn status(&self) -> Result<RepoStatus> {
        self.git.init_repo().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_manager() -> (ConfigManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let manager = ConfigManager::new(temp_dir.path().to_path_buf());
        (manager, temp_dir)
    }

    #[tokio::test]
    async fn test_init() {
        let (manager, _temp) = setup_test_manager();
        let info = manager.init().await.unwrap();

        assert!(info.status.initialized);
        assert!(info.last_commit.is_some());
    }

    #[tokio::test]
    async fn test_auto_commit() {
        let (manager, _temp) = setup_test_manager();
        manager.init().await.unwrap();

        let commit_id = manager
            .auto_commit("nginx", "proxy", "add", "myapp")
            .await
            .unwrap();

        assert!(!commit_id.is_empty());

        let commits = manager.log(1, None).await.unwrap();
        assert_eq!(commits[0].message, "nginx(proxy): add myapp");
    }

    #[tokio::test]
    async fn test_log() {
        let (manager, _temp) = setup_test_manager();
        manager.init().await.unwrap();

        manager
            .auto_commit("supervisor", "service", "create", "test")
            .await
            .unwrap();
        manager
            .auto_commit("nginx", "proxy", "add", "web")
            .await
            .unwrap();

        let commits = manager.log(10, None).await.unwrap();
        assert_eq!(commits.len(), 3); // init + 2 auto-commits
    }

    #[tokio::test]
    async fn test_backup_and_restore() {
        let (manager, _temp) = setup_test_manager();
        manager.init().await.unwrap();

        manager
            .auto_commit("test", "resource", "add", "item1")
            .await
            .unwrap();

        let backup = manager.backup(Some("test")).await.unwrap();
        assert!(backup.name.contains("backup-"));
        assert!(backup.name.contains("test"));

        let backups = manager.list_backups().await.unwrap();
        assert_eq!(backups.len(), 1);

        manager
            .auto_commit("test", "resource", "add", "item2")
            .await
            .unwrap();

        let restored_id = manager.restore(&backup.name).await.unwrap();
        assert_eq!(restored_id, backup.commit_id);
    }
}
