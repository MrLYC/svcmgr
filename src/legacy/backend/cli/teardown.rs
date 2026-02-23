use crate::config::Config;
use crate::error::Result;
use std::io::{self, Write};
use tracing::{info, warn};

pub async fn run(force: bool) -> Result<()> {
    let config = Config::new()?;

    if !config.data_dir.exists() {
        warn!("svcmgr is not initialized. Nothing to teardown.");
        return Ok(());
    }

    if !force {
        print!(
            "This will remove all svcmgr data at {:?}. Continue? [y/N]: ",
            config.data_dir
        );
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            info!("Teardown cancelled");
            return Ok(());
        }
    }

    info!("Removing svcmgr data directory...");
    std::fs::remove_dir_all(&config.data_dir)?;

    info!("Teardown complete!");
    Ok(())
}
