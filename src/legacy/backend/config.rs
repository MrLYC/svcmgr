/// Global configuration for svcmgr
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Base data directory (~/.local/share/svcmgr)
    pub data_dir: PathBuf,

    /// Web UI static files directory
    pub web_dir: PathBuf,

    /// Nginx configuration directory
    pub nginx_dir: PathBuf,

    /// Configuration repository path
    pub config_repo: Option<PathBuf>,
}

impl Config {
    /// Create default configuration with XDG base directory
    pub fn new() -> crate::error::Result<Self> {
        let home = dirs::home_dir()
            .ok_or_else(|| crate::error::Error::Config("Cannot find home directory".into()))?;

        let data_dir = home.join(".local/share/svcmgr");

        Ok(Self {
            web_dir: data_dir.join("web"),
            nginx_dir: data_dir.join("nginx"),
            config_repo: None,
            data_dir,
        })
    }

    /// Load configuration from file
    #[allow(dead_code)]
    pub fn load() -> crate::error::Result<Self> {
        // TODO: Load from ~/.config/svcmgr/config.toml
        Self::new()
    }

    /// Save configuration to file
    #[allow(dead_code)]
    pub fn save(&self) -> crate::error::Result<()> {
        // TODO: Save to ~/.config/svcmgr/config.toml
        Ok(())
    }
}
