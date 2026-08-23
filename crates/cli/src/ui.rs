//! Terminal styling: color where it helps, plain output everywhere else.
//!
//! Every helper degrades to unstyled text when stdout is not a TTY or
//! `NO_COLOR` is set, matching the convention of ripgrep and friends.

use std::io::IsTerminal;

use crossterm::style::{Attribute, Color, Stylize};

fn color_enabled() -> bool {
    std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

/// Dim secondary text: labels, separators, hints.
pub fn dim(text: &str) -> String {
    if color_enabled() {
        text.with(Color::DarkGrey).to_string()
    } else {
        text.to_string()
    }
}

/// Emphasized primary text: section titles, key values.
pub fn bold(text: &str) -> String {
    if color_enabled() {
        text.attribute(Attribute::Bold).to_string()
    } else {
        text.to_string()
    }
}

/// Links and actionable endpoints.
pub fn link(text: &str) -> String {
    if color_enabled() {
        text.with(Color::Blue).to_string()
    } else {
        text.to_string()
    }
}

/// Success confirmations.
pub fn ok_mark() -> String {
    if color_enabled() {
        "✓".with(Color::Green).to_string()
    } else {
        "ok".to_string()
    }
}

/// Human-readable byte sizes: one decimal below 10 units, integers above.
pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    #[expect(
        clippy::cast_precision_loss,
        reason = "sizes above 2^52 bytes are not a terminal-rendering concern"
    )]
    let mut size = bytes as f64;
    let mut unit = 0usize;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 || size >= 10.0 {
        format!("{size:.0} {}", UNITS[unit])
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::human_bytes;

    #[test]
    fn human_bytes_scales_and_rounds() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(393_602), "384 KB");
        assert_eq!(human_bytes(9 * 1024 * 1024), "9.0 MB");
        assert_eq!(human_bytes(40 * 1024 * 1024 * 1024), "40 GB");
    }
}
