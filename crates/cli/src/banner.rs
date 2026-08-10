//! Terminal wordmark rendering: SVG → tiny-skia raster → ASCII art.
//!
//! [`print_ascii_banner`] parses the white wordmark SVG asset with `resvg`,
//! renders it into an in-memory [`Pixmap`](tiny_skia::Pixmap) at a width derived
//! from the live terminal, then maps each pixel's alpha to an ASCII character.
//! Any failure along the way (missing asset, malformed SVG, oversized render,
//! zero-sized target) falls back to a stylized text banner, so the CLI never
//! panics on a missing or broken asset.

use std::path::Path;

use resvg::usvg::{Options, Tree};
use tiny_skia::{Pixmap, Transform};

/// Widest the banner may grow before it is assumed to clip the terminal.
const MAX_BANNER_COLUMNS: u16 = 65;

/// Fewest opaque pixels a rendered banner must contain to be readable.
///
/// A lone icon or a font-less wordmark (e.g. a text logo whose family is not
/// installed) can render far below this even though `resvg` succeeded; such a
/// sparse result does not read as the brand, so it degrades to the text banner.
const MIN_READABLE_PIXELS: u32 = 100;

/// Renders `svg_path` to a centered ASCII banner, falling back on any failure.
pub fn print_ascii_banner(svg_path: &Path) {
    let term_width = crossterm::terminal::window_size().map_or(80, |size| size.columns);
    let target_width = u32::from(term_width.min(MAX_BANNER_COLUMNS));

    let Some(tree) = parse_svg(svg_path) else {
        print_fallback_banner(term_width);
        return;
    };
    let svg_width = tree.size().width();
    let svg_height = tree.size().height();
    if svg_width <= 0.0 || svg_height <= 0.0 {
        print_fallback_banner(term_width);
        return;
    }

    let target_height = ascii_height(target_width, svg_height / svg_width);
    if target_width == 0 || target_height == 0 {
        print_fallback_banner(term_width);
        return;
    }

    let Some(mut pixmap) = Pixmap::new(target_width, target_height) else {
        print_fallback_banner(term_width);
        return;
    };

    let transform = Transform::from_scale(
        to_scale(target_width, svg_width),
        to_scale(target_height, svg_height),
    );
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    if opaque_pixels(&pixmap, target_width, target_height) < MIN_READABLE_PIXELS {
        print_fallback_banner(term_width);
        return;
    }

    print_pixels(&pixmap, target_width, target_height, term_width);
}

/// Parses `svg_path` into an SVG [`Tree`], or `None` on any read/parse failure.
///
/// System fonts are loaded so text-based assets render when their family is
/// installed. `Options::default()` ships an empty font database, so without
/// this step *no* text element can ever draw.
fn parse_svg(svg_path: &Path) -> Option<Tree> {
    let data = std::fs::read(svg_path).ok()?;
    let mut options = Options::default();
    options.fontdb_mut().load_system_fonts();
    Tree::from_data(&data, &options).ok()
}

/// Counts pixels whose alpha exceeds the blank threshold.
fn opaque_pixels(pixmap: &Pixmap, width: u32, height: u32) -> u32 {
    let mut total = 0u32;
    for y in 0..height {
        for x in 0..width {
            if pixmap
                .pixel(x, y)
                .is_some_and(|pixel| pixel.alpha() > 30)
            {
                total += 1;
            }
        }
    }
    total
}

/// Banner height in rows for a `target_width`-column canvas.
///
/// The `0.5` factor accounts for terminal cells being roughly twice as tall as
/// they are wide, so the rendered logo keeps its proportions on screen.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss, clippy::cast_sign_loss)]
fn ascii_height(target_width: u32, aspect_ratio: f32) -> u32 {
    (target_width as f32 * aspect_ratio * 0.5) as u32
}

/// Scale factor mapping SVG pixels onto `target` terminal units.
#[allow(clippy::cast_precision_loss)]
fn to_scale(target: u32, svg_extent: f32) -> f32 {
    target as f32 / svg_extent
}

/// Prints the rasterized pixels as ASCII lines centered in the terminal.
fn print_pixels(pixmap: &Pixmap, width: u32, height: u32, term_width: u16) {
    let width_u16 = u16::try_from(width).unwrap_or(0);
    let padding = usize::from(term_width.saturating_sub(width_u16) / 2);
    let capacity = usize::from(width_u16);

    println!();
    for y in 0..height {
        let mut line = String::with_capacity(capacity);
        for x in 0..width {
            let alpha = pixmap
                .pixel(x, y)
                .map_or(0, tiny_skia::PremultipliedColorU8::alpha);
            let character = match alpha {
                0..=30 => ' ',
                31..=90 => '.',
                91..=160 => ':',
                161..=220 => '*',
                _ => '#',
            };
            line.push(character);
        }
        println!("{:width$}{}", "", line, width = padding);
    }
    println!();
}

/// Prints a stylized "Greplog" text banner when SVG rendering cannot proceed.
fn print_fallback_banner(term_width: u16) {
    const ART_WIDTH: u16 = 59;
    const ART: [&str; 6] = [
        " ██████╗ ██████╗ ███████╗██████╗ ██╗      ██████╗  ██████╗ ",
        "██╔════╝ ██╔══██╗██╔════╝██╔══██╗██║     ██╔═══██╗██╔════╝ ",
        "██║  ███╗██████╔╝█████╗  ██████╔╝██║     ██║   ██║██║  ███╗",
        "██║   ██║██╔══██╗██╔══╝  ██╔═══╝ ██║     ██║   ██║██║   ██║",
        "╚██████╔╝██║  ██║███████╗██║     ███████╗╚██████╔╝╚██████╔╝",
        " ╚═════╝ ╚═╝  ╚═╝╚══════╝╚═╝     ╚══════╝ ╚═════╝  ╚═════╝ ",
    ];
    let padding = usize::from(term_width.saturating_sub(ART_WIDTH) / 2);

    println!();
    for line in ART {
        println!("{:width$}{}", "", line, width = padding);
    }
    println!();
}