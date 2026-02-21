use std::fs;
use svcmgr::atoms::git::GitAtom;
use tempfile::TempDir;

#[tokio::test]
async fn test_init_repo_creates_git_directory() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_path_buf();

    let atom = GitAtom::new(repo_path.clone());
    let status = atom.init_repo().await.unwrap();

    assert!(status.initialized);
    assert!(repo_path.join(".git").exists());
}

#[tokio::test]
async fn test_commit_creates_commit() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_path_buf();

    fs::write(repo_path.join("test.txt"), "test content").unwrap();

    let atom = GitAtom::new(repo_path.clone());
    atom.init_repo().await.unwrap();

    let commit_id = atom.commit("test: initial commit", &[]).await.unwrap();
    assert!(!commit_id.is_empty());
}

#[tokio::test]
async fn test_log_returns_commits() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_path_buf();

    fs::write(repo_path.join("test.txt"), "test content").unwrap();

    let atom = GitAtom::new(repo_path.clone());
    atom.init_repo().await.unwrap();
    atom.commit("test: first commit", &[]).await.unwrap();

    let commits = atom.log(10, None).await.unwrap();
    assert_eq!(commits.len(), 1);
    assert_eq!(commits[0].message, "test: first commit");
}

#[tokio::test]
async fn test_diff_shows_changes() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_path_buf();

    fs::write(repo_path.join("test.txt"), "original").unwrap();

    let atom = GitAtom::new(repo_path.clone());
    atom.init_repo().await.unwrap();
    let first_commit = atom.commit("test: initial", &[]).await.unwrap();

    fs::write(repo_path.join("test.txt"), "modified").unwrap();
    atom.commit("test: second", &[]).await.unwrap();

    let diff = atom.diff(&first_commit, "HEAD", None).await.unwrap();
    assert!(diff.contains("original") || diff.contains("modified"));
}
