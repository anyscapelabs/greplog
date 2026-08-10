//! Terminal wordmark rendering via image-to-ASCII rasterization.

use std::path::Path;

use image::GenericImageView;

/// Prints an ASCII art wordmark centered in the terminal.
///
/// The `wordmark-white.svg` asset is rasterized at runtime. For SVG assets,
/// this function falls back to a text banner if rasterization fails, ensuring
/// the CLI never panics on missing assets.
pub fn print_ascii_wordmark(asset_path: &Path) {
    // Attempt to open the asset as a raster image. SVG will fail gracefully
    // via the `if let Ok` guard and we fall back to a simple text logo.
    if let Ok(img) = image::open(asset_path) {
        let (width, height) = img.dimensions();
        let term_width = crossterm::terminal::window_size().map_or(80, |s| s.columns);

        // Scale down width to fit nicely in terminal (e.g., max 60 cols wide)
        let target_width = 60_u32;
        let scale = width / target_width;
        if scale == 0 {
            return;
        }

        let mut ascii_lines = Vec::new();
        for y in (0..height).step_by((scale * 2) as usize) {
            let mut line = String::new();
            for x in (0..width).step_by(scale as usize) {
                let pixel = img.get_pixel(x, y);
                let intensity = (u32::from(pixel[0]) + u32::from(pixel[1]) + u32::from(pixel[2])) / 3;
                let character = match intensity {
                    0..=50 => ' ',
                    51..=100 => '.',
                    101..=150 => ':',
                    151..=200 => '*',
                    _ => '@',
                };
                line.push(character);
            }
            ascii_lines.push(line);
        }

        // Print centered
        println!("\n");
        for line in ascii_lines {
            let Ok(line_len) = u16::try_from(line.len()) else {
                continue;
            };
            let padding = if term_width > line_len {
                (term_width - line_len) / 2
            } else {
                0
            };
            println!("{:width$}{}", "", line, width = usize::from(padding));
        }
        println!("\n");
    } else {
        // Fallback: centered text wordmark when SVG rasterization is unavailable.
        let term_width = crossterm::terminal::window_size().map_or(80, |s| s.columns);
        let logo = "GREPLOG";
        let Ok(logo_len) = u16::try_from(logo.len()) else {
            println!("\n{logo}\n");
            return;
        };
        let padding = if term_width > logo_len {
            (term_width - logo_len) / 2
        } else {
            0
        };
        println!("\n{:width$}{}\n", "", logo, width = usize::from(padding));
    }
}
