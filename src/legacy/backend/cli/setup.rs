use crate::config::Config;
use crate::error::Result;
use tracing::{info, warn};

pub async fn run(force: bool) -> Result<()> {
    info!("Starting svcmgr setup...");

    let config = Config::new()?;

    if config.data_dir.exists() && !force {
        warn!("svcmgr is already initialized at {:?}", config.data_dir);
        warn!("Use --force to re-initialize");
        return Ok(());
    }

    info!("Creating data directory: {:?}", config.data_dir);
    std::fs::create_dir_all(&config.data_dir)?;

    info!("Creating web directory: {:?}", config.web_dir);
    std::fs::create_dir_all(&config.web_dir)?;

    info!("Creating nginx directory: {:?}", config.nginx_dir);
    std::fs::create_dir_all(&config.nginx_dir)?;

    info!("Setup complete!");
    info!("Data directory: {:?}", config.data_dir);
    info!("Next steps:");
    info!("  1. Run 'svcmgr run' to start the service");
    info!("  2. Access web UI at http://localhost:8080/svcmgr");

    Ok(())
}
