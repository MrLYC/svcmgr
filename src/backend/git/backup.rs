use super::{versioning::GitVersioning, GitError};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::Mutex;

pub struct ConfigBackup {
    git: Arc<Mutex<GitVersioning>>,
    backup_dir: Option<PathBuf>,
}

impl ConfigBackup {
    pub fn new(git: Arc<Mutex<GitVersioning>>, backup_dir: Option<PathBuf>) -> Self {
        Self { git, backup_dir }
    }

    pub async fn create_backup(&self, tag: Option<&str>) -> Result<String, GitError> {
        let git = self.git.lock().await;

        let head = git.get_repo().head()?.peel_to_commit()?;
        let tag_name = tag
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("backup-{}", chrono::Utc::now().format("%Y%m%d-%H%M%S")));

        let sig = git.get_repo().signature()?;
        git.get_repo().tag(
            &tag_name,
            head.as_object(),
            &sig,
            &format!("Backup created at {}", chrono::Utc::now()),
            false,
        )?;

        if let Some(ref backup_dir) = self.backup_dir {
            let archive_path = backup_dir.join(format!("{}.tar.gz", tag_name));
            Self::export_archive(git.get_config_dir(), &archive_path)?;
            tracing::info!("Backup archived to: {}", archive_path.display());
        }

        Ok(tag_name.to_string())
    }

    pub async fn restore_backup(&self, tag: &str) -> Result<(), GitError> {
        let git = self.git.lock().await;

        let tag_ref = git
            .get_repo()
            .find_reference(&format!("refs/tags/{}", tag))?;
        let target = tag_ref.peel_to_commit()?;

        git.get_repo()
            .reset(target.as_object(), git2::ResetType::Hard, None)?;

        tracing::info!("Restored backup: {}", tag);
        Ok(())
    }

    pub async fn list_backups(&self) -> Result<Vec<BackupInfo>, GitError> {
        let git = self.git.lock().await;
        let tags = git.get_repo().tag_names(Some("backup-*"))?;

        let mut backups = Vec::new();
        for tag in tags.iter().flatten() {
            let tag_ref = git
                .get_repo()
                .find_reference(&format!("refs/tags/{}", tag))?;
            let commit = tag_ref.peel_to_commit()?;

            backups.push(BackupInfo {
                name: tag.to_string(),
                commit_id: commit.id().to_string(),
                timestamp: commit.time().seconds(),
                message: commit.message().unwrap_or("").to_string(),
            });
        }

        backups.sort_by_key(|b| -b.timestamp);

        Ok(backups)
    }

    fn export_archive(source: &Path, dest: &Path) -> Result<(), GitError> {
        use std::process::Command;

        let output = Command::new("tar")
            .arg("-czf")
            .arg(dest)
            .arg("-C")
            .arg(source.parent().unwrap_or(source))
            .arg(source.file_name().unwrap_or_default())
            .output()?;

        if !output.status.success() {
            return Err(GitError::Io(std::io::Error::other(format!(
                "tar command failed: {:?}",
                output.stderr
            ))));
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct BackupInfo {
    pub name: String,
    pub commit_id: String,
    pub timestamp: i64,
    pub message: String,
}
