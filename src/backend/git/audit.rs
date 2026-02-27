use super::{versioning::GitVersioning, CommitInfo, GitError};
use std::sync::Arc;
use tokio::sync::Mutex;

/// 配置审计日志查询器
///
/// 提供配置变更历史的过滤查询功能
pub struct ConfigAudit {
    git: Arc<Mutex<GitVersioning>>,
}

impl ConfigAudit {
    pub fn new(git: Arc<Mutex<GitVersioning>>) -> Self {
        Self { git }
    }

    /// 查询审计日志
    ///
    /// 根据过滤条件查询配置变更历史
    pub async fn query(&self, filter: AuditFilter) -> Result<Vec<AuditEntry>, GitError> {
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

    /// 检查提交是否匹配过滤条件
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
        if let Some(ref message_pattern) = filter.message_pattern {
            if !commit.message.contains(message_pattern) {
                return false;
            }
        }

        true
    }
}

/// 审计日志过滤器
#[derive(Debug, Default)]
pub struct AuditFilter {
    /// 开始时间戳（Unix timestamp）
    pub after: Option<i64>,
    /// 结束时间戳（Unix timestamp）
    pub before: Option<i64>,
    /// 作者名称（部分匹配）
    pub author: Option<String>,
    /// 消息模式（部分匹配）
    pub message_pattern: Option<String>,
    /// 最大返回数量
    pub limit: Option<usize>,
}

/// 审计日志条目
#[derive(Debug, Clone)]
pub struct AuditEntry {
    pub commit_id: String,
    pub author: String,
    pub email: String,
    pub timestamp: i64,
    pub message: String,
    pub files_changed: Vec<String>,
}
