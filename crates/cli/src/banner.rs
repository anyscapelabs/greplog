//! Terminal banner: the brand icon rendered to half-block art.
//!
//! `banner.txt` is generated from the logo SVG by
//! `scripts/render_banner.py` — rerun that script when the logo changes,
//! then commit the result. The runtime stays SVG-free and tints the art
//! with the brand color only when colors are enabled.

/// Rendered brand icon; 46 columns wide.
const ART: &str = include_str!("banner.txt");
const ART_WIDTH: usize = 46;

/// Prints the banner centered on the terminal width.
pub fn print_ascii_banner() {
    let term_width = crossterm::terminal::window_size().map_or(80, |size| usize::from(size.columns));
    let padding = term_width.saturating_sub(ART_WIDTH) / 2;
    let art = crate::ui::brand(ART);

    println!();
    for line in art.lines().filter(|line| !line.trim().is_empty()) {
        println!("{:width$}{}", "", line, width = padding);
    }
}
