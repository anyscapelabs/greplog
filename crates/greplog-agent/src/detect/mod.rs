use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

pub mod system;

#[derive(Debug, Serialize, Deserialize)]
pub struct Detection {
    pub service_name: Option<String>,
    pub language: String,
    pub framework: Option<String>,
    pub project_file: String,
}

pub fn detect_workspace(workspace: &Path) -> Result<Vec<Detection>> {
    let mut detections = Vec::new();

    if let Ok(content) = std::fs::read_to_string(workspace.join("package.json")) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            let name = json["name"].as_str().map(|s| s.to_string());
            let mut framework = None;

            if let Some(deps) = json["dependencies"].as_object() {
                if deps.contains_key("next") {
                    framework = Some("Next.js".into());
                } else if deps.contains_key("express") {
                    framework = Some("Express".into());
                }
            }

            detections.push(Detection {
                service_name: name,
                language: "Node.js".into(),
                framework,
                project_file: "package.json".into(),
            });
            info!("Detected Node.js project");
        }
    }

    if let Ok(content) = std::fs::read_to_string(workspace.join("Cargo.toml")) {
        let name = content
            .lines()
            .find(|l| l.starts_with("name ="))
            .and_then(|l| l.split('=').nth(1))
            .map(|s| s.trim().trim_matches('"').to_string());

        let mut framework = None;
        if content.contains("axum") {
            framework = Some("Axum".into());
        } else if content.contains("actix-web") {
            framework = Some("Actix Web".into());
        }

        detections.push(Detection {
            service_name: name,
            language: "Rust".into(),
            framework,
            project_file: "Cargo.toml".into(),
        });
        info!("Detected Rust project");
    }

    if let Ok(content) = std::fs::read_to_string(workspace.join("go.mod")) {
        let name = content
            .lines()
            .find(|l| l.starts_with("module "))
            .map(|l| l.replace("module ", "").trim().to_string());

        let mut framework = None;
        if content.contains("github.com/gin-gonic/gin") {
            framework = Some("Gin".into());
        } else if content.contains("github.com/labstack/echo") {
            framework = Some("Echo".into());
        } else if content.contains("github.com/gofiber/fiber") {
            framework = Some("Fiber".into());
        } else if content.contains("github.com/go-chi/chi") {
            framework = Some("Chi".into());
        } else if content.contains("github.com/gorilla/mux") {
            framework = Some("Gorilla Mux".into());
        }

        detections.push(Detection {
            service_name: name,
            language: "Go".into(),
            framework,
            project_file: "go.mod".into(),
        });
        info!("Detected Go project");
    }

    Ok(detections)
}
