use anyhow::Result;
use colored::Colorize;
use serde::Deserialize;

pub async fn run(port: u16) -> Result<()> {
    let base_url = format!("http://127.0.0.1:{}", port);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()?;

    println!(
        "\n  {}  {}\n",
        "▸".bright_green().bold(),
        "Checking Greplog agent status...".bold()
    );

    let _health = match client.get(format!("{}/health", base_url)).send().await {
        Ok(resp) if resp.status().is_success() => {
            println!("  {}  Agent is healthy", "✓".bright_green().bold());
            true
        }
        _ => {
            println!("  {}  Agent is not running", "✗".bright_red().bold());
            return Ok(());
        }
    };

    match client.get(format!("{}/resources", base_url)).send().await {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(resources) = resp.json::<SystemResources>().await {
                println!();
                println!("  {}  System Resources:", "📊".bold());
                println!(
                    "     CPU:    {} cores · {:.1}% usage",
                    resources.cpu.cores.to_string().bright_white(),
                    resources.cpu.global_usage_percent
                );
                println!(
                    "     Memory: {:.1} MB / {:.1} MB ({:.1}%)",
                    resources.memory.used_bytes as f64 / 1024.0 / 1024.0,
                    resources.memory.total_bytes as f64 / 1024.0 / 1024.0,
                    resources.memory.usage_percent
                );
            }
        }
        _ => {}
    }

    match client.get(format!("{}/detect", base_url)).send().await {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(detections) = resp.json::<Vec<DetectionInfo>>().await {
                if !detections.is_empty() {
                    println!();
                    println!("  {}  Detected Services:", "🔍".bold());
                    for det in &detections {
                        let name = det.service_name.as_deref().unwrap_or("(unnamed)");
                        println!("     {} · {}", name.bright_white().bold(), det.language.bright_cyan());
                    }
                }
            }
        }
        _ => {}
    }

    match client
        .post(format!("{}/query", base_url))
        .json(&serde_json::json!({
            "sql": "SELECT \
                (SELECT count(*) FROM logs) as log_count, \
                (SELECT count(*) FROM spans) as span_count, \
                (SELECT count(*) FROM metrics) as metric_count"
        }))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(result) = resp.json::<QueryResult>().await {
                if let Some(row) = result.rows.first() {
                    let logs = row.first().and_then(|v| v.as_i64()).unwrap_or(0);
                    let spans = row.get(1).and_then(|v| v.as_i64()).unwrap_or(0);
                    let metrics = row.get(2).and_then(|v| v.as_i64()).unwrap_or(0);

                    println!();
                    println!("  {}  Stored Events:", "📦".bold());
                    println!(
                        "     Logs: {}   Spans: {}   Metrics: {}",
                        logs.to_string().bright_white().bold(),
                        spans.to_string().bright_white().bold(),
                        metrics.to_string().bright_white().bold()
                    );
                }
            }
        }
        _ => {}
    }

    println!();
    Ok(())
}

#[derive(Deserialize)]
struct SystemResources {
    memory: MemoryInfo,
    cpu: CpuInfo,
}

#[derive(Deserialize)]
struct MemoryInfo {
    total_bytes: u64,
    used_bytes: u64,
    usage_percent: f64,
}

#[derive(Deserialize)]
struct CpuInfo {
    cores: usize,
    global_usage_percent: f64,
}

#[derive(Deserialize)]
struct DetectionInfo {
    service_name: Option<String>,
    language: String,
}

#[derive(Deserialize)]
struct QueryResult {
    rows: Vec<Vec<serde_json::Value>>,
}
