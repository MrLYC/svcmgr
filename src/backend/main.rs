//! svcmgr CLI entry point
//!
//! Phase 1.3: Basic CLI commands implementation
//! - init: Initialize configuration directory
//! - service: Service lifecycle management (start, stop, list)

use clap::{Parser, Subcommand};
use std::process;
use svcmgr::{cli, config::parser::ConfigParser};

#[derive(Parser)]
#[command(name = "svcmgr")]
#[command(version = "2.0.0-dev")]
#[command(about = "Service Manager - Process lifecycle management with mise integration")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize svcmgr configuration directory and Git repository
    Init,

    /// Service lifecycle management
    #[command(subcommand)]
    Service(ServiceCommands),
}

#[derive(Subcommand)]
enum ServiceCommands {
    /// Start a service
    Start {
        /// Service name
        name: String,
    },

    /// Stop a running service
    Stop {
        /// Service name
        name: String,
    },

    /// List all services and their status
    List,
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Init => cli::init::init().await,

        Commands::Service(service_cmd) => match service_cmd {
            ServiceCommands::Start { name } => {
                // Load configuration
                let config_path = dirs::config_dir()
                    .expect("Cannot determine config directory")
                    .join("mise/svcmgr/config.toml");

                if !config_path.exists() {
                    eprintln!(
                        "Error: Configuration file not found: {}",
                        config_path.display()
                    );
                    eprintln!("Run 'svcmgr init' first to initialize configuration.");
                    process::exit(1);
                }

                let parser = match ConfigParser::new() {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("Error creating config parser: {}", e);
                        process::exit(1);
                    }
                };
                let config = match parser.load() {
                    Ok(cfg) => cfg,
                    Err(e) => {
                        eprintln!("Error parsing configuration: {}", e);
                        process::exit(1);
                    }
                };

                cli::service::start(&name, &config).await
            }

            ServiceCommands::Stop { name } => cli::service::stop(&name).await,

            ServiceCommands::List => {
                // Load configuration for service list
                let config_path = dirs::config_dir()
                    .expect("Cannot determine config directory")
                    .join("mise/svcmgr/config.toml");

                if !config_path.exists() {
                    eprintln!(
                        "Error: Configuration file not found: {}",
                        config_path.display()
                    );
                    eprintln!("Run 'svcmgr init' first to initialize configuration.");
                    process::exit(1);
                }

                let parser = match ConfigParser::new() {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("Error creating config parser: {}", e);
                        process::exit(1);
                    }
                };
                let config = match parser.load() {
                    Ok(cfg) => cfg,
                    Err(e) => {
                        eprintln!("Error parsing configuration: {}", e);
                        process::exit(1);
                    }
                };

                cli::service::list(&config).await
            }
        },
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}
