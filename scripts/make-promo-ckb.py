#!/usr/bin/env python3
"""Create Kurdish promo images with real Rabar/Noto Arabic text."""
from pathlib import Path

import arabic_reshaper
from bidi.algorithm import get_display
from PIL import Image, ImageDraw, ImageFont, ImageFilter

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "assets" / "promo"
FONT_RABAR = ROOT / "assets" / "fonts" / "Rabar.ttf"
FONT_NOTO = ROOT / "assets" / "fonts" / "NotoSansArabic-Regular.ttf"
FONT_LATIN = ROOT / "assets" / "fonts" / "Outfit-Bold.ttf"

TEAL = (61, 214, 198)
RED = (255, 59, 78)
TEXT = (242, 245, 248)
MUTE = (138, 150, 165)
BG_TOP = (20, 27, 36)
BG_BOT = (10, 14, 19)


def ckb(s: str) -> str:
    return get_display(arabic_reshaper.reshape(s))


def font(size: int, prefer_rabar: bool = True) -> ImageFont.FreeTypeFont:
    path = FONT_RABAR if prefer_rabar and FONT_RABAR.exists() else FONT_NOTO
    try:
        return ImageFont.truetype(str(path), size=size)
    except OSError:
        return ImageFont.truetype(str(FONT_NOTO), size=size)


def latin(size: int) -> ImageFont.FreeTypeFont:
    try:
        return ImageFont.truetype(str(FONT_LATIN), size=size)
    except OSError:
        return font(size)


def bg(w=1600, h=900) -> Image.Image:
    img = Image.new("RGB", (w, h), BG_BOT)
    d = ImageDraw.Draw(img)
    for y in range(h):
        t = y / (h - 1)
        r = int(BG_TOP[0] * (1 - t) + BG_BOT[0] * t)
        g = int(BG_TOP[1] * (1 - t) + BG_BOT[1] * t)
        b = int(BG_TOP[2] * (1 - t) + BG_BOT[2] * t)
        d.line([(0, y), (w, y)], fill=(r, g, b))
    # soft teal glow
    glow = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    gd = ImageDraw.Draw(glow)
    gd.ellipse([w * 0.2, -h * 0.35, w * 0.8, h * 0.45], fill=(*TEAL, 40))
    glow = glow.filter(ImageFilter.GaussianBlur(80))
    img = Image.alpha_composite(img.convert("RGBA"), glow).convert("RGB")
    return img


def rounded(draw: ImageDraw.ImageDraw, box, radius, fill, outline=None, width=2):
    draw.rounded_rectangle(box, radius=radius, fill=fill, outline=outline, width=width)


def center_text(draw, xy, text, fnt, fill=TEXT):
    x, y = xy
    bbox = draw.textbbox((0, 0), text, font=fnt)
    tw, th = bbox[2] - bbox[0], bbox[3] - bbox[1]
    draw.text((x - tw / 2, y - th / 2), text, font=fnt, fill=fill)


def pill(draw, xy, text, fnt, fill=(30, 40, 50), outline=TEAL):
    pad_x, pad_y = 22, 12
    bbox = draw.textbbox((0, 0), text, font=fnt)
    tw, th = bbox[2] - bbox[0], bbox[3] - bbox[1]
    x, y = xy
    box = [x - tw / 2 - pad_x, y - th / 2 - pad_y, x + tw / 2 + pad_x, y + th / 2 + pad_y]
    rounded(draw, box, 999, fill=fill, outline=outline, width=2)
    center_text(draw, (x, y), text, fnt, TEXT)


def save(img: Image.Image, name: str):
    OUT.mkdir(parents=True, exist_ok=True)
    path = OUT / name
    img.save(path, "JPEG", quality=92, optimize=True)
    print("wrote", path)


def make_hero():
    img = bg()
    d = ImageDraw.Draw(img)
    w, h = img.size

    center_text(d, (w / 2, 120), ckb("ڕیل"), font(96), TEXT)
    center_text(d, (w / 2, 210), ckb("سکرین ڕیکۆردەر"), font(42), TEAL)

    # app card
    card = [w / 2 - 260, 280, w / 2 + 260, 720]
    rounded(d, card, 36, fill=(18, 24, 32), outline=TEAL, width=2)

    center_text(d, (w / 2, 340), ckb("ئامادە"), font(28), MUTE)
    center_text(d, (w / 2, 420), "00:00:00", latin(64), TEXT)

    # REC button
    cx, cy, r = w / 2, 540, 58
    d.ellipse([cx - r - 8, cy - r - 8, cx + r + 8, cy + r + 8], outline=RED, width=4)
    d.ellipse([cx - 22, cy - 22, cx + 22, cy + 22], fill=RED)

    center_text(d, (w / 2, 650), ckb("دەستپێکردنی تۆمار"), font(30), TEAL)
    center_text(d, (w / 2, 820), ckb("تۆ دەیبینیت — لە ڤیدیۆدا دەرناچێت"), font(26), MUTE)
    save(img, "reel-01-hero.jpg")


def make_recording():
    img = bg()
    d = ImageDraw.Draw(img)
    w, h = img.size

    center_text(d, (w / 2, 100), ckb("گرتنی شاشە و کوالیتی"), font(56), TEXT)
    center_text(d, (w / 2, 170), ckb("هەموو ئەوەی پێویستتە بۆ تۆماری پیشەیی"), font(26), MUTE)

    # dual monitors
    for i, ox in enumerate((-280, 280)):
        mx, my = w / 2 + ox, 380
        rounded(d, [mx - 170, my - 110, mx + 170, my + 90], 18, fill=(15, 20, 28), outline=TEAL, width=2)
        d.rectangle([mx - 30, my + 90, mx + 30, my + 110], fill=(40, 48, 58))
        center_text(d, (mx, my - 10), ckb(f"مۆنیتەر {i + 1}"), font(28), TEXT)
        center_text(d, (mx, my + 40), "MP4", latin(22), TEAL)

    badges = [
        (0.18, 0.72, "١٢٠ فرەیم"),
        (0.38, 0.72, "کوالیتی بەرز"),
        (0.58, 0.72, "کۆدێک HEVC"),
        (0.78, 0.72, "مایکڕۆفۆن"),
        (0.28, 0.84, "فۆرماتی MP4"),
        (0.50, 0.84, "دوو سکرین"),
        (0.72, 0.84, "VP9 · H.264"),
    ]
    bf = font(24)
    for px, py, label in badges:
        pill(d, (w * px, h * py), ckb(label) if any(ord(c) > 127 for c in label) else label, bf)

    save(img, "reel-02-recording.jpg")


def make_annotate():
    img = bg()
    d = ImageDraw.Draw(img)
    w, h = img.size

    center_text(d, (w / 2, 90), ckb("توڵی وێنەکێشان"), font(56), TEXT)
    center_text(d, (w / 2, 155), ckb("لە کاتی تۆماردا بنووسە و نیشانە بکە"), font(26), MUTE)

    # fake screen
    rounded(d, [180, 210, w - 180, 620], 24, fill=(14, 18, 24), outline=(80, 90, 100), width=2)
    # pen stroke
    d.line([(320, 360), (520, 300), (700, 380), (900, 320)], fill=RED, width=8, joint="curve")
    # arrow
    d.line([(980, 450), (1250, 380)], fill=(224, 154, 69), width=7)
    d.polygon([(1250, 380), (1210, 360), (1220, 410)], fill=(224, 154, 69))
    # select box
    box = [400, 420, 720, 560]
    for i in range(0, 320, 16):
        d.line([(box[0] + i, box[1]), (box[0] + min(i + 8, 320), box[1])], fill=TEAL, width=3)
        d.line([(box[0] + i, box[3]), (box[0] + min(i + 8, 320), box[3])], fill=TEAL, width=3)
    for i in range(0, 140, 16):
        d.line([(box[0], box[1] + i), (box[0], box[1] + min(i + 8, 140))], fill=TEAL, width=3)
        d.line([(box[2], box[1] + i), (box[2], box[1] + min(i + 8, 140))], fill=TEAL, width=3)

    tools = ["ماوس", "قەلەم", "هایلایتەر", "تیر", "چوارگۆشە", "بازنە", "سێلێکت", "ماجیک", "سڕینەوە"]
    bar = [140, 680, w - 140, 820]
    rounded(d, bar, 28, fill=(16, 20, 28), outline=TEAL, width=2)
    tf = font(20)
    n = len(tools)
    for i, name in enumerate(tools):
        x = bar[0] + 60 + i * ((bar[2] - bar[0] - 120) / (n - 1))
        d.ellipse([x - 18, 710, x + 18, 746], outline=TEAL if i == 1 else MUTE, width=2)
        if i == 1:
            d.ellipse([x - 10, 718, x + 10, 738], fill=RED)
        center_text(d, (x, 780), ckb(name), tf, TEAL if i == 1 else TEXT)

    center_text(d, (w / 2, 860), ckb("تولبار لە ڤیدیۆدا دەرناچێت"), font(24), MUTE)
    save(img, "reel-03-annotate.jpg")


def make_camera():
    img = bg()
    d = ImageDraw.Draw(img)
    w, h = img.size

    center_text(d, (w / 2, 90), ckb("کامێرا و گرین سکرین"), font(56), TEXT)
    center_text(d, (w / 2, 155), ckb("فەیس‌کام، فلتەر و لابردنی پاشبنەما"), font(26), MUTE)

    # camera bubble
    cx, cy, r = 480, 420, 150
    d.ellipse([cx - r - 6, cy - r - 6, cx + r + 6, cy + r + 6], outline=TEAL, width=5)
    d.ellipse([cx - r, cy - r, cx + r, cy + r], fill=(28, 36, 46))
    d.ellipse([cx - 55, cy - 70, cx + 55, cy + 40], fill=(70, 85, 100))  # head
    d.ellipse([cx - 95, cy + 30, cx + 95, cy + 150], fill=(55, 70, 85))  # shoulders
    center_text(d, (cx, cy + r + 40), ckb("کامێرای فەیس"), font(28), TEAL)

    # controls panel
    panel = [820, 250, 1420, 680]
    rounded(d, panel, 24, fill=(16, 22, 30), outline=(70, 80, 95), width=2)
    center_text(d, (1120, 290), ckb("ڕێکخستنەکان"), font(32), TEXT)

    rows = [
        (350, "شێوە: بازنە / چوارگۆشە"),
        (410, "فلتەر: نەرم · ڕوون · گەرم"),
        (470, "لابردنی پاشبنەما"),
        (530, "ڕەنگی کلیل + نەرمی لێوار"),
    ]
    for y, label in rows:
        pill(d, (1120, y), ckb(label), font(22), fill=(24, 32, 42), outline=(61, 214, 198))

    # swatches
    colors = [(34, 197, 94), (59, 130, 246), (217, 70, 239), (20, 20, 20)]
    for i, col in enumerate(colors):
        x = 1020 + i * 55
        d.ellipse([x, 590, x + 36, 626], fill=col, outline=TEXT, width=2)

    center_text(d, (w / 2, 820), ckb("لە کاتی تۆماردا لەگەڵ شاشەکەت دەردەکەوێت"), font(24), MUTE)
    save(img, "reel-04-camera.jpg")


if __name__ == "__main__":
    make_hero()
    make_recording()
    make_annotate()
    make_camera()
    print("done")
