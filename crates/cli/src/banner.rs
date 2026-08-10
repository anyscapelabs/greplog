//! Terminal wordmark rendering: SVG → tiny-skia raster → half-block Unicode.
//!
//! [`print_ascii_banner`] parses the white wordmark SVG asset with `resvg` and
//! renders it into an in-memory [`Pixmap`](tiny_skia::Pixmap) at a width derived
//! from the live terminal. Each terminal character row samples *two* vertical
//! pixels of the bitmap, so the logo keeps its true aspect ratio on the cell
//! grid; the alpha channel is cut at a strict 50% threshold and printed with
//! Unicode half-blocks (`█▀▄`), producing a sharp binary rendering of the white
//! vector on a transparent background.
//!
//! Any failure along the way (missing asset, malformed SVG, zero-sized target,
//! unreadably sparse render) falls back to a stylized text banner, so the CLI
//! never panics on a missing or broken asset.

use std::path::Path;

use resvg::usvg::{Options, Tree};
use tiny_skia::{Pixmap, Transform};

/// Widest the banner may grow before it is assumed to clip the terminal.
const MAX_BANNER_COLUMNS: u16 = 60;

/// Alpha at which a pixel counts as "filled" (binary white/blank threshold).
const OPAQUE_ALPHA: u8 = 128;

/// Fewest opaque pixels a rendered banner must contain to be readable.
///
/// A lone icon or a font-less wordmark (e.g. a text logo whose family is not
/// installed) can render far below this even though `resvg` succeeded; such a
/// sparse result does not read as the brand, so it degrades to the text banner.
const MIN_READABLE_PIXELS: u32 = 100;

/// Renders `svg_path` to a centered banner, falling back on any failure.
pub fn print_ascii_banner(svg_path: &Path) {
    let term_width = crossterm::terminal::window_size().map_or(80, |size| size.columns);
    let target_width = u32::from(term_width.min(MAX_BANNER_COLUMNS));
    if target_width == 0 {
        print_fallback_banner(term_width);
        return;
    }

    let Some(tree) = parse_svg(svg_path) else {
        print_fallback_banner(term_width);
        return;
    };
    let svg_width = tree.size().width();
    let svg_height = tree.size().height();

    let Some(pixel_height) = pixel_height(target_width, svg_width, svg_height) else {
        print_fallback_banner(term_width);
        return;
    };

    let Some(mut pixmap) = Pixmap::new(target_width, pixel_height) else {
        print_fallback_banner(term_width);
        return;
    };

    let transform = Transform::from_scale(
        to_scale(target_width, svg_width),
        to_scale(pixel_height, svg_height),
    );
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    if opaque_pixels(&pixmap, target_width, pixel_height) < MIN_READABLE_PIXELS {
        print_fallback_banner(term_width);
        return;
    }

    print_half_blocks(&pixmap, target_width, pixel_height, term_width);
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

/// Offscreen canvas height in real pixels, preserving the asset's aspect ratio.
///
/// Every printed character row covers two vertical pixels (half-blocks), so the
/// canvas is built at the SVG's true aspect ratio rather than halved, and
/// rounded up to an even height: the bottom-half lookup in [`print_half_blocks`]
/// then always stays in bounds, and terminal cells' 2:1 height-to-width ratio
/// keeps the logo undistorted on screen.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss, clippy::cast_sign_loss)]
fn pixel_height(target_width: u32, svg_width: f32, svg_height: f32) -> Option<u32> {
    if svg_width <= 0.0 || svg_height <= 0.0 {
        return None;
    }
    let mut height = ((target_width as f32 * (svg_height / svg_width)) as u32).max(4);
    if !height.is_multiple_of(2) {
        height += 1;
    }
    Some(height)
}

/// Scale factor mapping SVG pixels onto `target` terminal units.
#[allow(clippy::cast_precision_loss)]
fn to_scale(target: u32, svg_extent: f32) -> f32 {
    target as f32 / svg_extent
}

/// Counts pixels whose alpha exceeds the binary threshold.
fn opaque_pixels(pixmap: &Pixmap, width: u32, height: u32) -> u32 {
    let mut total = 0u32;
    for y in 0..height {
        for x in 0..width {
            if pixmap
                .pixel(x, y)
                .is_some_and(|pixel| pixel.alpha() > OPAQUE_ALPHA)
            {
                total += 1;
            }
        }
    }
    total
}

/// Prints the pixmap as half-block Unicode, two vertical pixels per character.
///
/// Each character is a strict binary decision on the two sampled pixels: `█`
/// when both are filled, `▀` for the top half, `▄` for the bottom half, and
/// whitespace for an empty cell.
fn print_half_blocks(pixmap: &Pixmap, width: u32, height: u32, term_width: u16) {
    let width_u16 = u16::try_from(width).unwrap_or(0);
    let padding = usize::from(term_width.saturating_sub(width_u16) / 2);
    let capacity = usize::from(width_u16);

    println!();
    for y in (0..height).step_by(2) {
        let mut line = String::with_capacity(capacity);
        for x in 0..width {
            let top = pixmap.pixel(x, y).is_some_and(|p| p.alpha() > OPAQUE_ALPHA);
            let bottom = pixmap
                .pixel(x, y + 1)
                .is_some_and(|p| p.alpha() > OPAQUE_ALPHA);
            let character = match (top, bottom) {
                (true, true) => '█',
                (true, false) => '▀',
                (false, true) => '▄',
                (false, false) => ' ',
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