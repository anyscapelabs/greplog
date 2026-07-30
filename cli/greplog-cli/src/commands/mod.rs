pub mod dev;
pub mod init;
pub mod status;

use colored::Colorize;

pub fn print_banner() {
    if let Ok(font) = figlet_rs::FIGlet::standard() {
        if let Some(figure) = font.convert("Greplog") {
            let banner = figure.to_string();
            for line in banner.lines() {
                println!("{}", line.bright_cyan().bold());
            }
        }
    }
    println!(
        "  {} {}\n",
        "v".dimmed(),
        env!("CARGO_PKG_VERSION").dimmed()
    );
}
