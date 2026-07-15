use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

pub async fn run(
    foreground: bool,
    ui_port: u16,
    tcp_port: u16,
    workspace: Option<PathBuf>,
) -> Result<()> {
    let workspace = resolve_workspace(workspace)?;

    println!(
        "  {}  {}\n",
        "▸".bright_green().bold(),
        "Starting Greplog agent...".bold()
    );

    let config = greplog_agent::Config {
        workspace: workspace.clone(),
        tcp_port,
        ui_port,
        flush_interval_secs: 2,
        socket_path: PathBuf::from(".greplog/greplog.sock"),
    };

    if foreground {
        print_ready(&workspace, ui_port, tcp_port);
        greplog_agent::run(config).await?;
    } else {
        let handle = tokio::spawn(async move {
            if let Err(e) = greplog_agent::run(config).await {
                eprintln!("  {}  Agent error: {}", "✗".bright_red().bold(), e);
            }
        });

        if wait_for_health(ui_port, 5).await {
            print_ready(&workspace, ui_port, tcp_port);
            println!(
                "  {}  Agent is running in the background.\n",
                "✓".bright_green().bold()
            );
            let _ = handle.await;
        } else {
            eprintln!(
                "  {}  Agent failed to start within 5s. Check logs.\n",
                "✗".bright_red().bold()
            );
            handle.abort();
            std::process::exit(1);
        }
    }

    Ok(())
}

fn print_ready(workspace: &std::path::Path, ui_port: u16, tcp_port: u16) {
    println!(
        "  {}  {}\n\n\
         {}  {}\n\
         {}  {}\n\
         {}  {}\n\
         {}  {}\n",
        "⚡".bold(),
        "Greplog is ready!".bright_green().bold(),
        "  Dashboard:".dimmed(),
        format!("http://localhost:{}", ui_port).bright_cyan().underline(),
        "  Workspace:".dimmed(),
        workspace.display().to_string().bright_white(),
        "  Socket:".dimmed(),
        workspace.join(".greplog/greplog.sock").display().to_string().dimmed(),
        "  TCP ingest:".dimmed(),
        format!("127.0.0.1:{}", tcp_port).dimmed(),
    );
}

async fn wait_for_health(port: u16, timeout_secs: u64) -> bool {
    let url = format!("http://127.0.0.1:{}/health", port);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .unwrap_or_default();

    let deadline =
        tokio::time::Instant::now() + tokio::time::Duration::from_secs(timeout_secs);

    loop {
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => return true,
            _ => {}
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    }
}

fn resolve_workspace(workspace: Option<PathBuf>) -> Result<PathBuf> {
    let ws = workspace.unwrap_or_else(|| std::env::current_dir().unwrap());
    Ok(ws.canonicalize().unwrap_or(ws))
}
