//! Terminal banner: the brand icon rendered to half-block art.
//!
//! `banner.txt` is generated from the logo SVG by
//! `scripts/render_banner.py` — rerun that script when the logo changes,
//! then commit the result. The runtime stays SVG-free and tints the art
//! with the brand color only when colors are enabled.

/// Rendered brand icon; 46 columns wide.
const ART: &str = include_str!("banner.txt");

/// Width of the rendered art, for callers composing side-by-side layouts.
pub const ART_WIDTH: usize = 46;

/// The rendered icon lines, brand-tinted when colors are enabled.
#[must_use]
pub fn art_lines() -> Vec<String> {
    crate::ui::brand(ART)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_string)
        .collect()
}
