pub mod audit;
pub mod backup;
pub mod conflict;
pub mod versioning;
pub mod watcher;

pub use audit::{AuditEntry, AuditFilter, ConfigAudit};
pub use backup::{BackupInfo, ConfigBackup};
pub use conflict::{Conflict, ConflictDetector, ConflictResolution};
pub use versioning::{CommitInfo, GitVersioning, RollbackResult, RollbackTarget};

use std::fmt;

#[derive(Debug)]
pub enum GitError {
    Io(std::io::Error),
    Git(git2::Error),
    NoStagedChanges,
    NoCommitAtTime(i64),
    InvalidSignature(String),
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GitError::Io(e) => write!(f, "IO error: {}", e),
            GitError::Git(e) => write!(f, "Git error: {}", e),
            GitError::NoStagedChanges => write!(f, "No staged changes to commit"),
            GitError::NoCommitAtTime(ts) => write!(f, "No commit found at timestamp {}", ts),
            GitError::InvalidSignature(s) => write!(f, "Invalid signature: {}", s),
        }
    }
}

impl std::error::Error for GitError {}

impl From<std::io::Error> for GitError {
    fn from(e: std::io::Error) -> Self {
        GitError::Io(e)
    }
}

impl From<git2::Error> for GitError {
    fn from(e: git2::Error) -> Self {
        GitError::Git(e)
    }
}
