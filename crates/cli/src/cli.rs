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
    /// Start the system in development mode with defaults tuned for a dev machine.
    Dev {
        /// Custom dashboard port (overrides `EngineConfig::dashboard_port`).
        #[arg(short, long)]
        port: Option<u16>,
    },
    /// Start the system in production mode.
    Start {
        /// Custom dashboard port (overrides `EngineConfig::dashboard_port`).
        #[arg(short, long)]
        port: Option<u16>,
        /// Days of Parquet history to keep before automatic purge.
        #[arg(long)]
        retention_days: Option<u32>,
    },
    /// Report WAL and storage status without a running server.
    Status,
}