use super::versioning::GitVersioning;
use super::GitError;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

const DEBOUNCE_DURATION: Duration = Duration::from_secs(2);

pub struct ConfigWatcher {
    git: Arc<Mutex<GitVersioning>>,
    watch_paths: Vec<PathBuf>,
    last_trigger: Instant,
}

impl ConfigWatcher {
    pub fn new(git: Arc<Mutex<GitVersioning>>, watch_paths: Vec<PathBuf>) -> Self {
        Self {
            git,
            watch_paths,
            last_trigger: Instant::now()
                .checked_sub(DEBOUNCE_DURATION)
                .unwrap_or_else(Instant::now),
        }
    }

    pub async fn start(&mut self) -> Result<(), GitError> {
        let (tx, mut rx) = tokio::sync::mpsc::channel(100);

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    let _ = tx.blocking_send(event);
                }
            },
            Config::default(),
        )
        .map_err(|e| GitError::Io(std::io::Error::other(e)))?;

        for path in &self.watch_paths {
            watcher
                .watch(path, RecursiveMode::NonRecursive)
                .map_err(|e| GitError::Io(std::io::Error::other(e)))?;
        }

        while let Some(event) = rx.recv().await {
            if Self::should_process(&event) && self.should_trigger() {
                self.on_file_change().await?;
            }
        }

        Ok(())
    }

    fn should_process(event: &Event) -> bool {
        matches!(
            event.kind,
            EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
        )
    }

    fn should_trigger(&mut self) -> bool {
        let now = Instant::now();
        if now.duration_since(self.last_trigger) >= DEBOUNCE_DURATION {
            self.last_trigger = now;
            true
        } else {
            false
        }
    }

    async fn on_file_change(&mut self) -> Result<(), GitError> {
        let mut git = self.git.lock().await;

        git.auto_stage()?;

        if git.has_staged_changes()? {
            let oid = git.commit("chore: auto-save configuration changes", None)?;
            tracing::info!("Auto-committed changes: {}", oid);
        }

        Ok(())
    }
}

pub fn watch_config_directory(config_dir: &Path, git: Arc<Mutex<GitVersioning>>) -> ConfigWatcher {
    let config_toml = config_dir.join("config.toml");
    ConfigWatcher::new(git, vec![config_toml])
}
