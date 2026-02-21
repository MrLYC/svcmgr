use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "svcmgr")]
#[command(version = "0.1.0")]
#[command(about = "Linux service management tool", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(about = "Initialize base environment (nginx, mise, cloudflare, etc.)")]
    Setup {
        #[arg(short, long, help = "Force re-initialization even if already setup")]
        force: bool,
    },

    #[command(about = "Start svcmgr service")]
    Run,

    #[command(about = "Uninstall base environment")]
    Teardown {
        #[arg(short, long, help = "Force teardown without confirmation")]
        force: bool,
    },
}

pub mod run;
pub mod setup;
pub mod teardown;
