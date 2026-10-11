from pathlib import Path

from PIL import Image

src = Path(r"C:\Users\rekar\.cursor\projects\c-Users-rekar-Desktop-project-screen\assets\reel-icon-new.jpg")
assets = Path(r"C:\Users\rekar\Desktop\project\screen\assets")
src_icon = assets / "icon_src.png"
out_png = assets / "icon.png"
out_ico = assets / "icon.ico"
out_mark = assets / "logo-mark.png"
out_logo = assets / "reel-logo.png"
icon_rs = Path(r"C:\Users\rekar\Desktop\project\screen\src\icon_data.rs")

im = Image.open(src).convert("RGBA")
w, h = im.size
side = min(w, h)
left = (w - side) // 2
top = (h - side) // 2
im = im.crop((left, top, left + side, top + side))

base = im.resize((512, 512), Image.Resampling.LANCZOS)
base.save(src_icon, "PNG")
base.resize((256, 256), Image.Resampling.LANCZOS).save(out_png, "PNG")
base.resize((256, 256), Image.Resampling.LANCZOS).save(out_logo, "PNG")
base.resize((96, 96), Image.Resampling.LANCZOS).save(out_mark, "PNG")

sizes = [(256, 256), (128, 128), (64, 64), (48, 48), (32, 32), (16, 16)]
base.resize((256, 256), Image.Resampling.LANCZOS).save(
    out_ico,
    format="ICO",
    sizes=sizes,
)

icon48 = base.resize((48, 48), Image.Resampling.LANCZOS)
pixels = list(icon48.getdata())
lines = []
row = []
for i, (r, g, b, a) in enumerate(pixels):
    row.extend([str(r), str(g), str(b), str(a)])
    if (i + 1) % 6 == 0:
        lines.append("    " + ",".join(row) + ",")
        row = []
if row:
    lines.append("    " + ",".join(row) + ",")

content = (
    "// Auto-generated window icon pixels (RGBA)\n"
    "pub const WIDTH: u32 = 48;\n"
    "pub const HEIGHT: u32 = 48;\n"
    "pub const RGBA: &[u8] = &[\n"
    + "\n".join(lines)
    + "\n];\n"
)
icon_rs.write_text(content, encoding="utf-8")

(assets / "reel-icon-new.jpg").write_bytes(src.read_bytes())

print("ico", out_ico.stat().st_size)
print("png", out_png.stat().st_size)
print("icon_data pixels", len(pixels))
print("done")
