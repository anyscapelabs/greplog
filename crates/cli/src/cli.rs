//! CLI definitions for `greplog`.

use clap::{Parser, Subcommand};

/// Greplog CLI.
#[derive(Debug, Parser)]
#[command(name = "greplog", version, about = "Greplog logging system")]
pub struct Cli {
    /// Subcommand to run.
    #[command(subcommand)]
    pub command: Commands,
}

/// Available subcommands.
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Start the system in development mode.
    Dev {
        /// Custom dashboard port (overrides `EngineConfig::dashboard_port`).
        #[arg(short, long)]
        port: Option<u16>,
    },
}
