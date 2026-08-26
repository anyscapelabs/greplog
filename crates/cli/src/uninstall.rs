//! `greplog uninstall`: one command that removes Greplog cleanly — storage,
//! WAL and the binary itself — instead of a hand-rolled `rm -rf`.

use std::fs;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use greplog_engine::config::EngineConfig;

use crate::ui;

/// How long to wait when probing a local port for a live instance.
const PORT_PROBE_TIMEOUT: Duration = Duration::from_millis(150);

/// One filesystem location the uninstaller removes.
struct Target {
    label: &'static str,
    path: PathBuf,
    bytes: u64,
    exists: bool,
}

impl Target {
    fn missing(label: &'static str, path: PathBuf) -> Self {
        Self {
            label,
            path,
            bytes: 0,
            exists: false,
        }
    }
}

/// Recursively sums file sizes under `path`; zero when it does not exist.
#[must_use]
fn directory_size(path: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| {
            let path = entry.path();
            if path.is_dir() {
                directory_size(&path)
            } else {
                fs::metadata(&path).map_or(0, |meta| meta.len())
            }
        })
        .sum()
}

/// Every location uninstall touches: Parquet storage, the WAL (sealed
/// segments live next to the active one), and the running binary.
#[must_use]
fn collect_targets(config: &EngineConfig) -> Vec<Target> {
    let mut targets = Vec::new();

    for (label, path) in [
        ("data", config.data_dir.as_path()),
        ("wal", config.wal_path.parent().unwrap_or(&config.wal_path)),
    ] {
        if path.exists() {
            targets.push(Target {
                label,
                path: path.to_path_buf(),
                bytes: directory_size(path),
                exists: true,
            });
        } else {
            targets.push(Target::missing(label, path.to_path_buf()));
        }
    }

    if let Ok(binary) = std::env::current_exe() {
        let bytes = fs::metadata(&binary).map_or(0, |meta| meta.len());
        if binary.is_file() {
            targets.push(Target {
                label: "binary",
                path: binary,
                bytes,
                exists: true,
            });
        } else {
            targets.push(Target::missing("binary", binary));
        }
    }

    targets
}

/// Returns the local port of a live Greplog instance, if one answers.
fn find_running_instance(config: &EngineConfig) -> Option<u16> {
    for port in [config.ingest_port, config.dashboard_port] {
        let Ok(address) = format!("127.0.0.1:{port}").parse() else {
            continue;
        };
        if TcpStream::connect_timeout(&address, PORT_PROBE_TIMEOUT).is_ok() {
            return Some(port);
        }
    }
    None
}

/// Reads a y/N answer from stdin; anything unconfirmed counts as no.
fn confirm(prompt: &str) -> std::io::Result<bool> {
    use std::io::Write;

    print!("{prompt} ");
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    Ok(matches!(answer.trim().to_lowercase().as_str(), "y" | "yes"))
}

/// Removes one target and reports the bytes it freed.
fn remove_target(target: &Target) -> std::io::Result<u64> {
    if target.path.is_dir() {
        fs::remove_dir_all(&target.path)?;
    } else {
        fs::remove_file(&target.path)?;
    }
    Ok(target.bytes)
}

/// Removes `data/` itself when the uninstall left it empty, so no stray
/// directories survive. Errors are ignored on purpose: a non-empty or
/// undeletable parent is not an uninstall failure.
fn remove_empty_parent(data_dir: &Path) {
    let Some(parent) = data_dir.parent() else {
        return;
    };
    let is_empty = fs::read_dir(parent).is_ok_and(|mut entries| entries.next().is_none());
    if is_empty {
        drop(fs::remove_dir(parent));
    }
}

/// Runs the interactive uninstall flow.
///
/// # Errors
/// Propagates filesystem failures after confirmation has been given.
pub fn run(assume_yes: bool) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::IsTerminal;

    let config = EngineConfig::default();
    let targets = collect_targets(&config);
    let found: Vec<&Target> = targets.iter().filter(|target| target.exists).collect();

    println!("{}", ui::bold("Uninstall Greplog"));
    if found.is_empty() {
        for target in &targets {
            println!(
                "  {:<8} {} {}",
                ui::dim(target.label),
                ui::dim("(not found)"),
                target.path.display()
            );
        }
        println!(
            "\n{} Nothing to remove — Greplog is already gone.",
            ui::ok_mark()
        );
        return Ok(());
    }

    for target in &targets {
        let size = if target.exists {
            ui::storage_bytes(target.bytes)
        } else {
            String::from("-")
        };
        println!(
            "  {:<8} {:>10}  {}",
            ui::dim(target.label),
            size,
            target.path.display()
        );
    }
    let total: u64 = found.iter().map(|target| target.bytes).sum();
    println!(
        "\n  {}",
        ui::dim(&format!("total {} to free", ui::storage_bytes(total)))
    );

    if let Some(port) = find_running_instance(&config) {
        println!(
            "\n  {} A Greplog instance looks like it is still serving on port {port}; \
             stop it first so nothing rewrites the removed paths.",
            ui::warn_mark()
        );
    }

    if !assume_yes {
        if !std::io::stdin().is_terminal() {
            return Err(
                "refusing to uninstall without a prompt in non-interactive use; pass --yes".into(),
            );
        }
        if !confirm("Proceed with uninstall? [y/N]")? {
            println!("{}", ui::dim("Aborted — nothing was removed."));
            return Ok(());
        }
    }

    let mut freed = 0;
    for target in &found {
        match remove_target(target) {
            Ok(bytes) => freed += bytes,
            Err(error) => {
                // The binary may be locked (e.g. Windows); leave it behind
                // with instructions rather than failing the whole run.
                if target.label == "binary" {
                    println!(
                        "  {} could not remove {}: {error}. Delete it manually.",
                        ui::warn_mark(),
                        target.path.display()
                    );
                    continue;
                }
                return Err(error.into());
            }
        }
    }
    remove_empty_parent(&config.data_dir);

    println!(
        "\n{} Uninstalled — freed {}. Logs stop being accepted immediately; \
         SDKs still pointed at this host will fail to connect.",
        ui::ok_mark(),
        ui::bold(&ui::storage_bytes(freed))
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_file(path: &Path, size: usize) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, vec![0u8; size]).unwrap();
    }

    #[test]
    fn directory_size_sums_files_recursively() {
        let root = tempfile::tempdir().unwrap();
        write_file(&root.path().join("a.parquet"), 100);
        write_file(&root.path().join("nested").join("b.parquet"), 30);
        assert_eq!(directory_size(root.path()), 130);
    }

    #[test]
    fn missing_paths_report_zero_bytes() {
        assert_eq!(directory_size(Path::new("/nonexistent/greplog")), 0);
    }

    #[test]
    fn collect_targets_lists_storage_even_when_absent() {
        let config = EngineConfig {
            data_dir: PathBuf::from("/nonexistent/greplog-data"),
            wal_path: PathBuf::from("/nonexistent/greplog-wal/current.wal"),
            ..EngineConfig::default()
        };

        let targets = collect_targets(&config);

        assert_eq!(targets.len(), 3, "data, wal and binary are always listed");
        assert!(targets.iter().take(2).all(|target| !target.exists));
    }

    #[test]
    fn remove_target_handles_directories_and_files() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("logs");
        let file = root.path().join("greplog");
        write_file(&dir.join("chunk.parquet"), 50);
        write_file(&file, 20);

        let dir_target = Target {
            label: "data",
            path: dir.clone(),
            bytes: directory_size(&dir),
            exists: true,
        };
        let file_target = Target {
            label: "binary",
            path: file.clone(),
            bytes: 20,
            exists: true,
        };

        assert_eq!(remove_target(&dir_target).unwrap(), 50);
        assert_eq!(remove_target(&file_target).unwrap(), 20);
        assert!(!dir.exists());
        assert!(!file.exists());
    }

    #[test]
    fn empty_parent_is_cleaned_but_populated_one_is_kept() {
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("data");
        let data_dir = parent.join("logs");
        write_file(&data_dir.join("chunk.parquet"), 10);

        remove_empty_parent(&data_dir);
        assert!(parent.exists(), "parent with siblings survives");

        fs::remove_dir_all(&parent).unwrap();
        fs::create_dir_all(&parent).unwrap();
        remove_empty_parent(&parent.join("logs"));
        assert!(!parent.exists(), "emptied parent is removed");
    }
}
