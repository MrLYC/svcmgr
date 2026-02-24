use super::{GitError, versioning::GitVersioning};
use git2::Oid;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

pub struct ConflictDetector {
    git: Arc<Mutex<GitVersioning>>,
    last_known_head: Oid,
    last_checked: Instant,
}

impl ConflictDetector {
    pub async fn new(git: Arc<Mutex<GitVersioning>>) -> Result<Self, GitError> {
        let git_locked = git.lock().await;
        let last_known_head = git_locked.get_head_oid()?;
        drop(git_locked);

        Ok(Self {
            git,
            last_known_head,
            last_checked: Instant::now(),
        })
    }

    pub async fn check_conflicts(&mut self) -> Result<Option<Conflict>, GitError> {
        let git = self.git.lock().await;

        if !git.has_staged_changes()? {
            return Ok(None);
        }

        let head_now = git.get_head_oid()?;

        if self.last_known_head != head_now {
            return Ok(Some(Conflict {
                our_changes: git.diff_staged()?,
                their_commit: head_now.to_string(),
            }));
        }

        Ok(None)
    }

    pub fn update_head(&mut self, new_head: Oid) {
        self.last_known_head = new_head;
        self.last_checked = Instant::now();
    }
}

#[derive(Debug, Clone)]
pub struct Conflict {
    pub our_changes: String,
    pub their_commit: String,
}

#[derive(Debug, Clone)]
pub enum ConflictResolution {
    Abort,
    Force,
    Merge,
    Manual,
}
