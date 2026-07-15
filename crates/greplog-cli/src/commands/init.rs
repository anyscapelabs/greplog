use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

pub async fn run(workspace: Option<PathBuf>) -> Result<()> {
    let workspace = workspace.unwrap_or_else(|| std::env::current_dir().unwrap());
    let workspace = workspace.canonicalize().unwrap_or(workspace);

    println!(
        "\n  {}  {}  {}\n",
        "▸".bright_green().bold(),
        "Scanning workspace:".bold(),
        workspace.display().to_string().bright_white()
    );

    let detections = greplog_agent::detect::detect_workspace(&workspace)?;

    if detections.is_empty() {
        println!(
            "  {}  No supported frameworks detected.\n",
            "⚠".bright_yellow().bold(),
        );
        return Ok(());
    }

    println!("  {}  Detected {} service(s):\n", "✓".bright_green().bold(), detections.len());

    for (i, det) in detections.iter().enumerate() {
        let name = det.service_name.as_deref().unwrap_or("(unnamed)");
        let fw = det.framework.as_deref().unwrap_or("none");

        println!(
            "  {}  {}",
            format!("{}.", i + 1).dimmed(),
            format!(
                "{} · {} · framework: {} · ({})",
                name.bright_white().bold(),
                det.language.bright_cyan(),
                fw.bright_yellow(),
                det.project_file.dimmed()
            )
        );
    }

    let config_path = workspace.join("greplog.config.json");
    let config = GreplogConfig {
        version: 1,
        workspace: workspace.display().to_string(),
        services: detections
            .iter()
            .map(|d| ServiceConfig {
                name: d.service_name.clone().unwrap_or_default(),
                language: d.language.clone(),
                framework: d.framework.clone(),
                project_file: d.project_file.clone(),
            })
            .collect(),
    };

    let json = serde_json::to_string_pretty(&config)?;
    tokio::fs::write(&config_path, json).await?;

    println!(
        "\n  {}  Config written to {}\n",
        "✓".bright_green().bold(),
        config_path.display().to_string().bright_cyan().underline()
    );

    Ok(())
}

#[derive(serde::Serialize)]
struct GreplogConfig {
    version: u32,
    workspace: String,
    services: Vec<ServiceConfig>,
}

#[derive(serde::Serialize)]
struct ServiceConfig {
    name: String,
    language: String,
    framework: Option<String>,
    project_file: String,
}
