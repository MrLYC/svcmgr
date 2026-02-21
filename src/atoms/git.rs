use crate::error::{Error, Result};
use git2::{DiffOptions, IndexAddOption, ObjectType, Repository, Signature};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoStatus {
    pub initialized: bool,
    pub path: PathBuf,
    pub head: Option<String>,
    pub clean: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitInfo {
    pub id: String,
    pub message: String,
    pub author: String,
    pub timestamp: i64,
    pub files: Vec<String>,
}

pub type CommitId = String;

pub struct GitAtom {
    repo_path: PathBuf,
}

impl GitAtom {
    pub fn new(repo_path: PathBuf) -> Self {
        Self { repo_path }
    }

    pub async fn init_repo(&self) -> Result<RepoStatus> {
        let path = &self.repo_path;

        if path.join(".git").exists() {
            let repo = Repository::open(path)?;
            let head = repo
                .head()
                .ok()
                .and_then(|h| h.shorthand().map(String::from));

            let statuses = repo.statuses(None)?;
            let clean = statuses.is_empty();

            return Ok(RepoStatus {
                initialized: true,
                path: path.to_path_buf(),
                head,
                clean,
            });
        }

        std::fs::create_dir_all(path)?;

        let _repo = Repository::init(path)?;

        let gitignore_path = path.join(".gitignore");
        if !gitignore_path.exists() {
            let gitignore_content =
                "# Runtime files\n*.log\n*.pid\n*.sock\n\n# Temporary files\n*.tmp\n*.swp\n*~\n";
            std::fs::write(&gitignore_path, gitignore_content)?;
        }

        Ok(RepoStatus {
            initialized: true,
            path: path.to_path_buf(),
            head: None,
            clean: true,
        })
    }

    pub async fn commit(&self, message: &str, files: &[PathBuf]) -> Result<CommitId> {
        let repo = Repository::open(&self.repo_path)?;

        let mut index = repo.index()?;

        if files.is_empty() {
            index.add_all(["."].iter(), IndexAddOption::DEFAULT, None)?;
        } else {
            for file in files {
                let relative_path = file
                    .strip_prefix(&self.repo_path)
                    .map_err(|_| Error::InvalidArgument("File not in repository".into()))?;
                index.add_path(relative_path)?;
            }
        }

        index.write()?;
        let tree_id = index.write_tree()?;
        let tree = repo.find_tree(tree_id)?;

        let signature = Signature::now("svcmgr", "svcmgr@localhost")?;

        let parent_commit = match repo.head() {
            Ok(head) => {
                let oid = head
                    .target()
                    .ok_or_else(|| Error::Git("HEAD has no target".into()))?;
                Some(repo.find_commit(oid)?)
            }
            Err(_) => None,
        };

        let parents: Vec<_> = parent_commit.iter().collect();

        let oid = repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &parents,
        )?;

        Ok(oid.to_string())
    }

    pub async fn log(&self, limit: usize, path: Option<&Path>) -> Result<Vec<CommitInfo>> {
        let repo = Repository::open(&self.repo_path)?;

        let mut revwalk = repo.revwalk()?;
        revwalk.push_head()?;
        revwalk.set_sorting(git2::Sort::TIME)?;

        let mut commits = Vec::new();

        for (idx, oid) in revwalk.enumerate() {
            if idx >= limit {
                break;
            }

            let oid = oid?;
            let commit = repo.find_commit(oid)?;

            let mut files = Vec::new();
            if let Ok(tree) = commit.tree()
                && let Some(parent) = commit.parent(0).ok()
                && let Ok(parent_tree) = parent.tree()
            {
                let diff = repo.diff_tree_to_tree(Some(&parent_tree), Some(&tree), None)?;

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
            }

            commits.push(CommitInfo {
                id: oid.to_string(),
                message: commit.message().unwrap_or("").to_string(),
                author: commit.author().name().unwrap_or("Unknown").to_string(),
                timestamp: commit.time().seconds(),
                files,
            });
        }

        if let Some(filter_path) = path {
            let filter_str = filter_path.to_string_lossy();
            commits.retain(|c| c.files.iter().any(|f| f.contains(filter_str.as_ref())));
        }

        Ok(commits)
    }

    pub async fn diff(&self, from: &str, to: &str, path: Option<&Path>) -> Result<String> {
        let repo = Repository::open(&self.repo_path)?;

        let from_obj = repo.revparse_single(from)?;
        let to_obj = repo.revparse_single(to)?;

        let from_tree = from_obj.peel_to_tree()?;
        let to_tree = to_obj.peel_to_tree()?;

        let mut diff_opts = DiffOptions::new();
        if let Some(p) = path {
            diff_opts.pathspec(p);
        }

        let diff =
            repo.diff_tree_to_tree(Some(&from_tree), Some(&to_tree), Some(&mut diff_opts))?;

        let mut result = String::new();
        diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
            result.push_str(&format!("{}", String::from_utf8_lossy(line.content())));
            true
        })?;

        Ok(result)
    }

    pub async fn checkout_file(&self, commit: &str, file: &Path) -> Result<()> {
        let repo = Repository::open(&self.repo_path)?;

        let obj = repo.revparse_single(commit)?;
        let commit = obj.peel_to_commit()?;
        let tree = commit.tree()?;

        let relative_path = file
            .strip_prefix(&self.repo_path)
            .map_err(|_| Error::InvalidArgument("File not in repository".into()))?;

        let entry = tree.get_path(relative_path)?;
        let object = entry.to_object(&repo)?;

        if object.kind() != Some(ObjectType::Blob) {
            return Err(Error::InvalidArgument("Not a file".into()));
        }

        let blob = object
            .as_blob()
            .ok_or_else(|| Error::Git("Failed to get blob".into()))?;

        std::fs::write(file, blob.content())?;

        self.commit(
            &format!(
                "[git] revert: {} to {}",
                relative_path.display(),
                &commit.id().to_string()[..7]
            ),
            &[file.to_path_buf()],
        )
        .await?;

        Ok(())
    }

    pub async fn revert(&self, commit_id: &str) -> Result<CommitId> {
        let repo = Repository::open(&self.repo_path)?;

        let obj = repo.revparse_single(commit_id)?;
        let commit = obj.peel_to_commit()?;

        let mut revert_opts = git2::RevertOptions::new();
        repo.revert(&commit, Some(&mut revert_opts))?;

        let head = repo.head()?;
        let oid = head
            .target()
            .ok_or_else(|| Error::Git("HEAD has no target".into()))?;

        Ok(oid.to_string())
    }

    pub async fn push(&self, remote: &str, branch: &str) -> Result<()> {
        let repo = Repository::open(&self.repo_path)?;

        let mut remote = repo.find_remote(remote)?;

        let refspec = format!("refs/heads/{}:refs/heads/{}", branch, branch);
        remote.push(&[&refspec], None)?;

        Ok(())
    }

    pub async fn pull(&self, remote_name: &str, branch: &str) -> Result<()> {
        let repo = Repository::open(&self.repo_path)?;

        let mut remote = repo.find_remote(remote_name)?;
        remote.fetch(&[branch], None, None)?;

        let fetch_head = repo.find_reference("FETCH_HEAD")?;
        let fetch_commit = repo.reference_to_annotated_commit(&fetch_head)?;

        let analysis = repo.merge_analysis(&[&fetch_commit])?;

        if analysis.0.is_up_to_date() {
            return Ok(());
        }

        if analysis.0.is_fast_forward() {
            let refname = format!("refs/heads/{}", branch);
            let mut reference = repo.find_reference(&refname)?;
            reference.set_target(fetch_commit.id(), "Fast-forward")?;
            repo.set_head(&refname)?;
            repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;
            return Ok(());
        }

        Err(Error::NotSupported(
            "Merge conflicts require manual resolution".into(),
        ))
    }
}
