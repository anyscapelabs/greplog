use clap::Parser;

mod commands;

#[derive(Parser)]
#[command(
    name = "greplog",
    version,
    about = "Local-first observability for developers"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Start the local workspace agent + dashboard
    Dev {
        #[arg(long)]
        foreground: bool,
        #[arg(long, default_value_t = 4317)]
        port: u16,
        #[arg(long, default_value_t = 4318)]
        tcp_port: u16,
        #[arg(long)]
        workspace: Option<std::path::PathBuf>,
        #[arg(long, default_value_t = 50_000)]
        max_buffer_events: usize,
        #[arg(long, default_value_t = 2)]
        flush_interval_secs: u64,
        #[arg(long, default_value_t = 300)]
        compaction_interval_secs: u64,
    },
    /// Detect frameworks and write greplog.config.json
    Init {
        #[arg(long)]
        workspace: Option<std::path::PathBuf>,
    },
    /// Show agent health, buffer size, disk usage
    Status {
        #[arg(long, default_value_t = 4317)]
        port: u16,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn".into()),
        )
        .init();

    commands::print_banner();

    let cli = Cli::parse();

    match cli.command {
        Commands::Dev {
            foreground,
            port,
            tcp_port,
            workspace,
            max_buffer_events,
            flush_interval_secs,
            compaction_interval_secs,
        } => commands::dev::run(foreground, port, tcp_port, workspace, max_buffer_events, flush_interval_secs, compaction_interval_secs).await,
        Commands::Init { workspace } => commands::init::run(workspace).await,
        Commands::Status { port } => commands::status::run(port).await,
    }
}
