mod atoms;
mod cli;
mod config;
mod error;
mod features;

use clap::Parser;
use cli::{Cli, Commands};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Setup { force } => cli::setup::run(force).await,
        Commands::Run => cli::run::run().await,
        Commands::Teardown { force } => cli::teardown::run(force).await,
        Commands::Service { action } => cli::service::handle_service_command(action).await,
        Commands::Cron { action } => cli::cron::handle_cron_command(action).await,
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
