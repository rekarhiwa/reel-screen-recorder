#!/usr/bin/env python3
"""Promo images with Kurdish text in Rabar, rendered by Edge."""
from __future__ import annotations

import base64
import subprocess
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "assets" / "promo"
FONT_RABAR = ROOT / "assets" / "fonts" / "Rabar.ttf"
EDGE = Path(r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe")
if not EDGE.exists():
    EDGE = Path(r"C:\Program Files\Microsoft\Edge\Application\msedge.exe")

TEAL = "#3dd6c6"
WHITE = "#f2f5f8"
MUTE = "#a8b2bf"
BG = "#0e1319"

_FONT_B64: str | None = None


def rabar_font_uri() -> str:
    """Embed Rabar as data URI so Edge can load it from file:// HTML."""
    global _FONT_B64
    if _FONT_B64 is None:
        if not FONT_RABAR.exists():
            raise FileNotFoundError(f"Rabar font missing: {FONT_RABAR}")
        _FONT_B64 = base64.b64encode(FONT_RABAR.read_bytes()).decode("ascii")
    return f"data:font/ttf;base64,{_FONT_B64}"


def load_bg(name: str) -> Image.Image:
    p = OUT / name
    if not p.exists():
        p = (
            Path.home()
            / ".cursor"
            / "projects"
            / "c-Users-rekar-Desktop-project-screen"
            / "assets"
            / name
        )
    return Image.open(p).convert("RGBA")


def shade(img: Image.Image) -> Image.Image:
    w, h = img.size
    overlay = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    d = ImageDraw.Draw(overlay)
    for y in range(int(h * 0.26)):
        a = int(200 * (1 - y / (h * 0.26)))
        d.line([(0, y), (w, y)], fill=(8, 12, 18, a))
    for i, y in enumerate(range(int(h * 0.72), h)):
        t = i / max(1, h - int(h * 0.72))
        d.line([(0, y), (w, y)], fill=(8, 12, 18, min(230, int(50 + 190 * t))))
    return Image.alpha_composite(img, overlay)


def render_text_png(lines: list[tuple[str, int, str]], width: int = 1600) -> Image.Image:
    parts = []
    for text, size, color in lines:
        parts.append(
            f'<div style="font-size:{size}px;color:{color};font-weight:400;'
            f'font-family:Rabar,sans-serif">{text}</div>'
        )
    font_uri = rabar_font_uri()
    html = f"""<!DOCTYPE html>
<html lang="ckb" dir="rtl"><head><meta charset="utf-8">
<style>
@font-face {{
  font-family: "Rabar";
  src: url("{font_uri}") format("truetype");
  font-weight: 400;
  font-style: normal;
  font-display: block;
}}
html,body{{margin:0;padding:0;background:{BG};}}
#box{{
  box-sizing:border-box;width:{width}px;padding:28px 24px;text-align:center;
  font-family:"Rabar" !important;
  font-weight:400 !important;
  direction:rtl;unicode-bidi:isolate;
  letter-spacing:0;word-spacing:0;
}}
#box div{{
  line-height:1.45;margin:0;padding:2px 0;
  font-family:"Rabar" !important;
  font-weight:400 !important;
}}
</style></head><body>
<div id="box">{"".join(parts)}</div>
<script>
(async function(){{
  try {{ await document.fonts.load('400 64px Rabar'); }} catch (e) {{}}
  await document.fonts.ready;
  document.documentElement.setAttribute('data-fonts','1');
}})();
</script>
</body></html>"""

    OUT.mkdir(parents=True, exist_ok=True)
    html_path = OUT / "_edge_text.html"
    shot_path = OUT / "_edge_text.png"
    html_path.write_text(html, encoding="utf-8")
    if shot_path.exists():
        shot_path.unlink()

    est_h = 80 + sum(max(size, 24) + 24 for _, size, _ in lines)
    subprocess.run(
        [
            str(EDGE),
            "--headless=new",
            "--disable-gpu",
            "--hide-scrollbars",
            f"--window-size={width},{max(est_h, 220)}",
            "--virtual-time-budget=20000",
            f"--screenshot={shot_path.resolve()}",
            html_path.resolve().as_uri(),
        ],
        check=False,
        capture_output=True,
    )
    if not shot_path.exists():
        raise RuntimeError("Edge screenshot missing")

    img = Image.open(shot_path).convert("RGBA")
    # Make near-background pixels transparent, then crop.
    px = img.load()
    w, h = img.size
    target = tuple(int(BG[i : i + 2], 16) for i in (1, 3, 5))
    for y in range(h):
        for x in range(w):
            r, g, b, a = px[x, y]
            if abs(r - target[0]) < 12 and abs(g - target[1]) < 12 and abs(b - target[2]) < 12:
                px[x, y] = (0, 0, 0, 0)
    bbox = img.getbbox()
    if bbox:
        img = img.crop(bbox)
    return img


def paste_center(base: Image.Image, overlay: Image.Image, y_frac: float):
    x = (base.width - overlay.width) // 2
    y = int(y_frac * base.height - overlay.height / 2)
    base.alpha_composite(overlay, (max(0, x), max(0, y)))


def save(img: Image.Image, name: str):
    path = OUT / name
    img.convert("RGB").save(path, "JPEG", quality=94, optimize=True)
    print("wrote", path.name)


def pill_label(base: Image.Image, text: str, cx: float, cy: float, size: int = 22):
    chip = render_text_png([(text, size, WHITE)], width=900)
    pad = 10
    bw, bh = chip.width + pad * 2, chip.height + pad * 2
    x0, y0 = int(cx - bw / 2), int(cy - bh / 2)
    ov = Image.new("RGBA", base.size, (0, 0, 0, 0))
    od = ImageDraw.Draw(ov)
    od.rounded_rectangle(
        [x0, y0, x0 + bw, y0 + bh],
        radius=999,
        fill=(12, 18, 26, 230),
        outline=(61, 214, 198, 255),
        width=2,
    )
    base.alpha_composite(ov)
    base.alpha_composite(chip, (x0 + pad, y0 + pad))


def make_hero():
    img = shade(load_bg("reel-bg-01-hero.jpg"))
    paste_center(img, render_text_png([("ڕیل", 84, WHITE)]), 0.10)
    paste_center(img, render_text_png([("سکرین ڕیکۆردەر", 40, TEAL)]), 0.20)
    paste_center(img, render_text_png([("دەستپێکردنی تۆمار", 36, TEAL)]), 0.88)
    paste_center(img, render_text_png([("تۆ دەیبینیت — لە ڤیدیۆدا دەرناچێت", 24, MUTE)]), 0.95)
    save(img, "reel-01-hero.jpg")


def make_recording():
    img = shade(load_bg("reel-bg-02-recording.jpg"))
    w, h = img.size
    panel = Image.new("RGBA", img.size, (0, 0, 0, 0))
    d = ImageDraw.Draw(panel)
    d.rounded_rectangle([40, int(h * 0.62), w - 40, h - 28], radius=28, fill=(8, 12, 18, 175))
    img = Image.alpha_composite(img, panel)

    paste_center(img, render_text_png([("گرتنی شاشە و کوالیتی", 52, WHITE)]), 0.09)
    paste_center(img, render_text_png([("هەموو ئەوەی پێویستتە بۆ تۆماری پیشەیی", 26, MUTE)]), 0.17)

    rows = [
        (0.72, ["١٢٠ فرەیم", "کوالیتی بەرز", "کۆدێک HEVC", "مایکڕۆفۆن"]),
        (0.86, ["فۆرماتی MP4", "دوو سکرین", "VP9 و H.264", "AVI و WMV"]),
    ]
    for yf, labels in rows:
        n = len(labels)
        for i, label in enumerate(labels):
            pill_label(img, label, w * (0.15 + i * (0.7 / (n - 1))), h * yf, 22)
    save(img, "reel-02-recording.jpg")


def make_annotate():
    img = shade(load_bg("reel-bg-03-annotate.jpg"))
    w, h = img.size
    paste_center(img, render_text_png([("توڵی وێنەکێشان", 52, WHITE)]), 0.09)
    paste_center(img, render_text_png([("لە کاتی تۆماردا بنووسە و نیشانە بکە", 26, MUTE)]), 0.17)

    # Icon centers from reel-bg-03 toolbar (drawing tools only). Labels sit
    # below the bar so icons stay fully visible.
    tools = [
        (299, "قەڵەم", WHITE),
        (375, "هایلایتەر", WHITE),
        (448, "فڵچە", TEAL),
        (521, "تیر", WHITE),
        (600, "سێلێکت", WHITE),
        (690, "سڕینەوە", WHITE),
        (780, "ماجیک", WHITE),
    ]
    y_label = int(h * 0.92)
    for cx, name, color in tools:
        chip = render_text_png([(name, 17, color)], width=340)
        x = int(cx - chip.width / 2)
        y = y_label - chip.height // 2
        img.alpha_composite(chip, (max(0, min(w - chip.width, x)), max(0, y)))
    paste_center(img, render_text_png([("تولبار لە ڤیدیۆدا دەرناچێت", 18, MUTE)]), 0.98)
    save(img, "reel-03-annotate.jpg")


def make_camera():
    img = shade(load_bg("reel-bg-04-camera.jpg"))
    w, h = img.size
    paste_center(img, render_text_png([("کامێرا و گرین سکرین", 52, WHITE)]), 0.09)
    paste_center(img, render_text_png([("فەیس‌کام، فلتەر و لابردنی پاشبنەما", 26, MUTE)]), 0.17)

    panel = Image.new("RGBA", img.size, (0, 0, 0, 0))
    d = ImageDraw.Draw(panel)
    d.rounded_rectangle(
        [int(w * 0.52), int(h * 0.28), w - 50, int(h * 0.78)],
        radius=22,
        fill=(10, 16, 24, 215),
    )
    img = Image.alpha_composite(img, panel)

    labels = [
        "کامێرای فەیس",
        "شێوە: بازنە / چوارگۆشە",
        "فلتەر: نەرم · ڕوون · گەرم",
        "لابردنی پاشبنەما",
        "ڕەنگی کلیل + نەرمی لێوار",
    ]
    for i, label in enumerate(labels):
        pill_label(img, label, w * 0.74, h * 0.36 + i * h * 0.08, 21)
    paste_center(img, render_text_png([("لە کاتی تۆماردا لەگەڵ شاشەکەت دەردەکەوێت", 24, MUTE)]), 0.92)
    save(img, "reel-04-camera.jpg")


if __name__ == "__main__":
    make_hero()
    make_recording()
    make_annotate()
    make_camera()
    # cleanup temps
    for p in OUT.glob("_edge*"):
        p.unlink(missing_ok=True)
    for p in OUT.glob("_segoe*"):
        p.unlink(missing_ok=True)
    print("done")
