"""Generate Reel app logo assets: screen-monitor mark, PNG, ICO, icon_data.rs."""
from __future__ import annotations

import math
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "assets"
SRC = ROOT / "src"

INK = (14, 18, 24, 255)
PANEL = (24, 30, 40, 255)
COPPER = (212, 140, 74, 255)
COPPER_DIM = (156, 96, 48, 255)
COPPER_HOT = (232, 168, 98, 255)
CREAM = (236, 232, 224, 255)
SCREEN = (30, 38, 50, 255)
REC = (226, 75, 75, 255)


def rounded_rect_mask(size: int, radius: float) -> Image.Image:
    m = Image.new("L", (size, size), 0)
    d = ImageDraw.Draw(m)
    d.rounded_rectangle((0, 0, size - 1, size - 1), radius=radius, fill=255)
    return m


def lerp(a, b, t):
    return tuple(int(x + (y - x) * t) for x, y in zip(a, b))


def paint_bg(img: Image.Image) -> None:
    w, h = img.size
    px = img.load()
    for y in range(h):
        t = y / max(h - 1, 1)
        c = lerp(PANEL, INK, t * 0.85)
        for x in range(w):
            nx = (x / (w - 1) - 0.5) * 2
            ny = (y / (h - 1) - 0.5) * 2
            v = min(1.0, math.sqrt(nx * nx + ny * ny) * 0.55)
            px[x, y] = lerp(c, (8, 10, 14, 255), v * 0.35)


def draw_screen(draw: ImageDraw.ImageDraw, cx: float, cy: float, s: float) -> None:
    """Draw a monitor/screen icon centered at (cx, cy). s = half-width scale."""
    # Monitor body (bezel)
    bw = s * 1.55
    bh = s * 1.12
    bx0, by0 = cx - bw / 2, cy - bh / 2 - s * 0.08
    bx1, by1 = cx + bw / 2, cy + bh / 2 - s * 0.08
    radius = max(2, s * 0.12)
    stroke = max(2, int(round(s * 0.085)))

    # Outer bezel fill
    draw.rounded_rectangle((bx0, by0, bx1, by1), radius=radius, fill=COPPER_DIM)
    inset = s * 0.04
    draw.rounded_rectangle(
        (bx0 + inset, by0 + inset, bx1 - inset, by1 - inset),
        radius=max(1, radius - 1),
        fill=COPPER,
    )

    # Screen glass
    pad = s * 0.14
    sx0, sy0 = bx0 + pad, by0 + pad
    sx1, sy1 = bx1 - pad, by1 - pad * 1.15
    draw.rounded_rectangle(
        (sx0, sy0, sx1, sy1),
        radius=max(1, s * 0.06),
        fill=SCREEN,
    )
    # Soft screen glow / highlight strip
    glow_h = (sy1 - sy0) * 0.28
    draw.rounded_rectangle(
        (sx0 + 1, sy0 + 1, sx1 - 1, sy0 + glow_h),
        radius=max(1, s * 0.04),
        fill=(48, 58, 74, 255),
    )

    # REC dot on screen
    rr = s * 0.09
    rx = sx1 - pad * 0.55 - rr
    ry = sy0 + pad * 0.45 + rr
    draw.ellipse((rx - rr, ry - rr, rx + rr, ry + rr), fill=REC)

    # Stand neck
    neck_w = s * 0.16
    neck_h = s * 0.22
    ny0 = by1 - s * 0.02
    draw.rectangle(
        (cx - neck_w / 2, ny0, cx + neck_w / 2, ny0 + neck_h),
        fill=COPPER,
    )

    # Stand base
    base_w = s * 0.72
    base_h = s * 0.11
    by_base = ny0 + neck_h - s * 0.02
    draw.rounded_rectangle(
        (cx - base_w / 2, by_base, cx + base_w / 2, by_base + base_h),
        radius=max(1, s * 0.05),
        fill=COPPER_HOT,
    )
    # Base underside
    draw.rounded_rectangle(
        (cx - base_w / 2, by_base + base_h * 0.45, cx + base_w / 2, by_base + base_h),
        radius=max(1, s * 0.04),
        fill=COPPER_DIM,
    )

    # Thin cream edge hint on bezel top
    draw.line(
        (bx0 + radius, by0 + inset * 0.5, bx1 - radius, by0 + inset * 0.5),
        fill=(236, 232, 224, 70),
        width=max(1, stroke // 3),
    )


def _render_app_icon_raw(size: int) -> Image.Image:
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    paint_bg(img)
    d = ImageDraw.Draw(img)
    pad = size * 0.04
    d.rounded_rectangle(
        (pad, pad, size - 1 - pad, size - 1 - pad),
        radius=size * 0.18,
        outline=(42, 52, 68, 180),
        width=max(1, size // 128),
    )
    cx = cy = size / 2
    draw_screen(d, cx, cy + size * 0.02, size * 0.34)
    mask = rounded_rect_mask(size, size * 0.22)
    out = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    out.paste(img, (0, 0), mask)
    return out


def make_app_icon(size: int = 512) -> Image.Image:
    hi = _render_app_icon_raw(size * 3)
    return hi.resize((size, size), Image.Resampling.LANCZOS)


def make_mark(size: int = 256) -> Image.Image:
    hi = size * 3
    img = Image.new("RGBA", (hi, hi), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    draw_screen(d, hi / 2, hi / 2 + hi * 0.02, hi * 0.38)
    return img.resize((size, size), Image.Resampling.LANCZOS)


MARK_SVG = """<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 96 96" role="img" aria-hidden="true">
  <defs>
    <linearGradient id="bezel" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0%" stop-color="#e8a862"/>
      <stop offset="100%" stop-color="#9c6030"/>
    </linearGradient>
    <linearGradient id="glass" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0%" stop-color="#3a4658"/>
      <stop offset="100%" stop-color="#1e2632"/>
    </linearGradient>
  </defs>
  <!-- monitor body -->
  <rect x="14" y="16" width="68" height="48" rx="7" fill="url(#bezel)"/>
  <!-- screen -->
  <rect x="20" y="22" width="56" height="34" rx="4" fill="url(#glass)"/>
  <!-- soft highlight -->
  <rect x="22" y="24" width="52" height="10" rx="3" fill="#4a586c" opacity="0.55"/>
  <!-- rec dot -->
  <circle cx="68" cy="30" r="3.2" fill="#e24b4b"/>
  <!-- stand neck -->
  <rect x="44" y="64" width="8" height="10" rx="1.5" fill="#d48c4a"/>
  <!-- stand base -->
  <path d="M30 80h36c2.2 0 4-1.4 4-3.2 0-1.2-1-2.3-2.4-2.8L56 70H40l-11.6 4c-1.4.5-2.4 1.6-2.4 2.8 0 1.8 1.8 3.2 4 3.2z" fill="#e8a862"/>
  <rect x="28" y="78.5" width="40" height="5.5" rx="2.5" fill="#9c6030"/>
</svg>
"""

APP_SVG = """<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128" role="img" aria-label="ڕیل">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0%" stop-color="#181e28"/>
      <stop offset="100%" stop-color="#0e1218"/>
    </linearGradient>
    <linearGradient id="bezel" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0%" stop-color="#e8a862"/>
      <stop offset="100%" stop-color="#9c6030"/>
    </linearGradient>
    <linearGradient id="glass" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0%" stop-color="#3a4658"/>
      <stop offset="100%" stop-color="#1e2632"/>
    </linearGradient>
  </defs>
  <rect x="4" y="4" width="120" height="120" rx="28" fill="url(#bg)"/>
  <rect x="8" y="8" width="112" height="112" rx="24" fill="none" stroke="#2a3444" stroke-width="1.5" opacity="0.9"/>
  <rect x="26" y="30" width="76" height="52" rx="8" fill="url(#bezel)"/>
  <rect x="33" y="37" width="62" height="36" rx="4" fill="url(#glass)"/>
  <rect x="35" y="39" width="58" height="11" rx="3" fill="#4a586c" opacity="0.55"/>
  <circle cx="86" cy="45" r="3.6" fill="#e24b4b"/>
  <rect x="60" y="82" width="8" height="12" rx="1.5" fill="#d48c4a"/>
  <rect x="42" y="96" width="44" height="7" rx="3.5" fill="#e8a862"/>
  <rect x="42" y="99" width="44" height="5" rx="2.5" fill="#9c6030"/>
</svg>
"""

# Compact mark for UI (same as MARK_SVG body)
UI_MARK_SVG = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 96 96">
            <defs>
              <linearGradient id="bezel" x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stop-color="#e8a862"/>
                <stop offset="100%" stop-color="#9c6030"/>
              </linearGradient>
              <linearGradient id="glass" x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stop-color="#3a4658"/>
                <stop offset="100%" stop-color="#1e2632"/>
              </linearGradient>
            </defs>
            <rect x="14" y="16" width="68" height="48" rx="7" fill="url(#bezel)"/>
            <rect x="20" y="22" width="56" height="34" rx="4" fill="url(#glass)"/>
            <rect x="22" y="24" width="52" height="10" rx="3" fill="#4a586c" opacity="0.55"/>
            <circle cx="68" cy="30" r="3.2" fill="#e24b4b"/>
            <rect x="44" y="64" width="8" height="10" rx="1.5" fill="#d48c4a"/>
            <rect x="28" y="78.5" width="40" height="5.5" rx="2.5" fill="#e8a862"/>
            <rect x="28" y="81" width="40" height="4" rx="2" fill="#9c6030"/>
          </svg>"""


def write_svg(path: Path) -> None:
    path.write_text(APP_SVG, encoding="utf-8")


def write_mark_svg(path: Path) -> None:
    path.write_text(MARK_SVG, encoding="utf-8")


def save_ico(img512: Image.Image, path: Path) -> None:
    sizes = [16, 24, 32, 48, 64, 128, 256]
    icons = [img512.resize((s, s), Image.Resampling.LANCZOS) for s in sizes]
    icons[0].save(path, format="ICO", sizes=[(s, s) for s in sizes], append_images=icons[1:])


def write_icon_data_rs(img48: Image.Image, path: Path) -> None:
    rgba = img48.convert("RGBA").tobytes()
    lines = [
        "// Auto-generated window icon pixels (RGBA)",
        "pub const WIDTH: u32 = 48;",
        "pub const HEIGHT: u32 = 48;",
        "pub const RGBA: &[u8] = &[",
    ]
    for i in range(0, len(rgba), 24):
        part = rgba[i : i + 24]
        lines.append("    " + ",".join(str(b) for b in part) + ",")
    lines.append("];")
    lines.append("")
    path.write_text("\n".join(lines), encoding="utf-8")


def main() -> None:
    ASSETS.mkdir(parents=True, exist_ok=True)
    icon = make_app_icon(512)
    mark = make_mark(256)

    icon_png = ASSETS / "icon.png"
    mark_png = ASSETS / "logo-mark.png"
    ico = ASSETS / "icon.ico"
    logo_svg = ASSETS / "logo.svg"
    mark_svg = ASSETS / "logo-mark.svg"

    icon.save(icon_png, "PNG")
    mark.save(mark_png, "PNG")
    save_ico(icon, ico)
    write_svg(logo_svg)
    write_mark_svg(mark_svg)

    img48 = icon.resize((48, 48), Image.Resampling.LANCZOS)
    write_icon_data_rs(img48, SRC / "icon_data.rs")
    icon.save(ASSETS / "reel-logo.png", "PNG")
    print("Wrote:", icon_png, mark_png, ico, logo_svg, mark_svg, SRC / "icon_data.rs")


if __name__ == "__main__":
    main()
