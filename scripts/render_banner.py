#!/usr/bin/env python3
"""Renders the brand icon SVG into the terminal banner art.

Output is committed (crates/cli/src/banner.txt) so the runtime binary needs
no SVG stack: rerun this script only when the logo changes.

    python3 scripts/render_banner.py [width]

Half-block cells (▀) double vertical resolution; shape-only output — the
runtime tints it with the brand color when colors are enabled.
"""

import sys

import cairosvg
from PIL import Image

SVG = "assets/branding/logo/icon/icon-blue.svg"
OUT = "crates/cli/src/banner.txt"
DEFAULT_WIDTH = 46


def render(width: int) -> None:
    # 2x vertical resolution: one character cell covers two pixel rows via ▀.
    png = cairosvg.svg2png(url=SVG, output_width=width * 4)
    image = Image.open(__import__("io").BytesIO(png)).convert("RGBA")

    # Crop to the ink bounding box so the art fills its cell budget.
    alpha = image.getchannel("A")
    image = image.crop(alpha.getbbox())
    image = image.resize((width, max(2, round(image.height / image.width * width / 2)) * 2))

    pixels = image.load()
    lines = []
    for y in range(0, image.height, 2):
        line = []
        for x in range(image.width):
            top = pixels[x, y][3] > 96
            bottom = pixels[x, y + 1][3] > 96
            line.append("█" if top and bottom else "▀" if top else "▄" if bottom else " ")
        lines.append("".join(line))

    # Keep trailing spaces: uniform-width lines let the CLI compose a fixed
    # info column without re-measuring.
    with open(OUT, "w") as handle:
        handle.write("\n".join(line for line in lines if line.strip()) + "\n")
    print(f"wrote {OUT} ({len(lines)} rows x {width} cols)")


if __name__ == "__main__":
    render(int(sys.argv[1]) if len(sys.argv) > 1 else DEFAULT_WIDTH)
