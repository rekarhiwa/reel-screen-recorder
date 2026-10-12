#!/usr/bin/env python3
"""Build UTF-16 sources + Unicode RTF pages for Inno Setup (Kurdish)."""
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PAGES = ROOT / "installer" / "pages"

SOURCES = {
    "about-ckb.txt": """ڕیل چییە؟

ڕیل (REEL) ئەپێکی دێسکتۆپی کوردییە بۆ تۆمارکردنی شاشەی ویندۆز.
ڤیدیۆ بە فۆرماتی MP4 پاشەکەوت دەبێت، بە کۆدێکی HEVC یان H.264.

بۆ کێ دروستکراوە؟
• مامۆستا و خوێندکار — وانە و ڕوونکردنەوە
• دروستکەری ناوەڕۆک — ڤیدیۆی کورت و ڕاهێنان
• کارمەند و تیم — دیمۆ و کۆبوونەوەی دوور

تایبەتمەندی سەرەکی
• تۆماری یەک یان دوو مۆنیتەر
• مایکڕۆفۆن + هەڵبژاردنی ئامێر
• کامێرای فەیس و گرین سکرین
• قەلەم و شەیپ لە کاتی تۆماردا
• شۆرکەتی گشتی بۆ دەستپێ / وەستان / پۆز

تێبینی
پەنجەرەی ڕێکخستنی ڕیل و تولباری وێنەکێشان
لە ناو ڤیدیۆدا دەرناچن — تەنها تۆ دەیانبینیت.
""",
    "features-ckb.txt": """تایبەتمەندییەکانی ڕیل

گرتنی شاشە
• هەڵبژاردنی مۆنیتەر
• تۆماری دوو سکرین (دوو فایلی جیاواز)
• کوالیتی: نزم → 4K
• FPS: ٢٤ / ٣٠ / ٦٠
• کۆدێک: HEVC (کەم قەبارە) یان H.264

دەنگ و کامێرا
• مایکڕۆفۆن بە هەڵبژاردنی ئامێر
• کامێرای فەیس + فلتەر
• گرین سکرین

وێنەکێشان لە کاتی تۆمار
• قەلەم، هێڵ، بازنە، چوارگۆشە، تەیر
• تولبار لە ڤیدیۆدا دیار نییە

شۆرکەت (بنەڕەتی)
• Ctrl+Shift+R — دەستپێ / وەستان
• Ctrl+Shift+P — پۆز
• Ctrl+Shift+M — مایک
• Ctrl+Shift+C — کامێرا
• Ctrl+Shift+D — قەلەم
(دەتوانیت لە ناو ئەپدا بیانگۆڕیت)
""",
    "howto-ckb.txt": """چۆن بەکاری بهێنیت؟

١) دوای دامەزراندن، ڕیل لە Desktop یان Start Menu بکەوە.
٢) مۆنیتەر و کوالیتی هەڵبژێرە.
٣) ئەگەر پێویستت بە دەنگە، مایک چالاک بکە.
٤) دوگمەی سووری تۆمار داگرە — یان شۆرکەت بەکاربهێنە.
٥) بۆ پۆز: دوگمەی پۆز یان Ctrl+Shift+P.
٦) بۆ وەستان: دووبارە دوگمەی تۆمار داگرە.
٧) فایلەکە لە فۆڵدەری recordings پاشەکەوت دەبێت.
   دەتوانیت لە ناو ئەپدا «کردنەوەی فایل / فۆڵدەر» هەڵبژێریت.

ئامۆژگاری خێرا
• بۆ قەبارەی کەمتر: HEVC + کوالیتی «4K کەم ستۆرەیج»
• بۆ گونجاندنی زیاتر لەگەڵ پلەیەرەکان: H.264
• لە کاتی تۆماردا پەنجەرەی ڕیل بگرە — لە ڤیدیۆدا دەرناچێت
• وێنەکێشان تەنها کاتێک دیارە کە تۆمار دەکەیت

سوپاس کە ڕیل بەکاردەهێنیت!
https://github.com/rekarhiwa/reel-screen-recorder
""",
}


def to_rtf(text: str) -> str:
    parts = [
        r"{\rtf1\fbidis\ansi\ansicpg1256\deff0\adelang1025",
        r"{\fonttbl{\f0\fswiss\fcharset178 Segoe UI;}{\f1\fswiss\fcharset0 Segoe UI;}}",
        r"\viewkind4\uc1\pard\rtlpar\qr\f0\fs22",
    ]
    for ch in text.replace("\r\n", "\n").replace("\r", "\n"):
        if ch == "\n":
            parts.append(r"\par")
        elif ch in "\\{}":
            parts.append("\\" + ch)
        elif ord(ch) < 128:
            parts.append(ch)
        else:
            cp = ord(ch)
            if cp > 32767:
                cp -= 65536
            parts.append(f"\\u{cp}?")
    parts.append("}")
    return "".join(parts)


def main() -> None:
    PAGES.mkdir(parents=True, exist_ok=True)
    for name, text in SOURCES.items():
        text = text.strip() + "\n"
        txt_path = PAGES / name
        txt_path.write_text(text, encoding="utf-16")
        rtf_path = PAGES / name.replace(".txt", ".rtf")
        rtf_path.write_bytes(to_rtf(text).encode("ascii"))
        print(f"wrote {txt_path.name} + {rtf_path.name}")


if __name__ == "__main__":
    main()
