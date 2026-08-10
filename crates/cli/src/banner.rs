//! Terminal wordmark banner.
//!
//! [`print_ascii_banner`] prints a pre-formatted ASCII block logo and centers
//! it on the active terminal window width, falling back to 80 columns when the
//! terminal size cannot be queried. No SVG rasterization is involved, so the
//! logo renders identically in every terminal window.

/// Prints the Greplog ASCII block banner, centered on the terminal width.
pub fn print_ascii_banner() {
    const ART_WIDTH: usize = 59;
    const ART: [&str; 6] = [
        " ██████╗ ██████╗ ███████╗██████╗ ██╗      ██████╗  ██████╗ ",
        "██╔════╝ ██╔══██╗██╔════╝██╔══██╗██║     ██╔═══██╗██╔════╝ ",
        "██║  ███╗██████╔╝█████╗  ██████╔╝██║     ██║   ██║██║  ███╗",
        "██║   ██║██╔══██╗██╔══╝  ██╔═══╝ ██║     ██║   ██║██║   ██║",
        "╚██████╔╝██║  ██║███████╗██║     ███████╗╚██████╔╝╚██████╔╝",
        " ╚═════╝ ╚═╝  ╚═╝╚══════╝╚═╝     ╚══════╝ ╚═════╝  ╚═════╝ ",
    ];

    let term_width = crossterm::terminal::window_size().map_or(80, |size| usize::from(size.columns));
    let padding = term_width.saturating_sub(ART_WIDTH) / 2;

    println!();
    for line in ART {
        println!("{:width$}{}", "", line, width = padding);
    }
    println!();
}