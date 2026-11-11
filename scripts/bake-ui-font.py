#!/usr/bin/env python3
"""Bake the embedded UI atlas. Requires Pillow and the DejaVu Sans system fonts."""
from pathlib import Path
import struct
from PIL import Image, ImageDraw, ImageFont

root = Path(__file__).resolve().parent.parent
size, tile, columns = 32, 48, 16
count = 95 * 2
width = columns * tile
height = ((count + columns - 1) // columns) * tile
atlas = Image.new("L", (width, height))
draw = ImageDraw.Draw(atlas)
draw.rectangle((0, 0, 1, 1), fill=255)  # Solid shapes sample this texel.
metrics = bytearray()
for weight, name in enumerate(["DejaVuSans.ttf", "DejaVuSans-Bold.ttf"]):
    font = ImageFont.truetype(f"/usr/share/fonts/truetype/dejavu/{name}", size)
    for code in range(32, 127):
        char = chr(code)
        i = weight * 95 + code - 32
        x, y = (i % columns) * tile + 4, (i // columns) * tile + 4
        left, top, right, bottom = font.getbbox(char, anchor="ls")
        draw.text((x - left, y - top), char, font=font, fill=255, anchor="ls")
        metrics.extend(struct.pack("<7f", x, y, right - left, bottom - top, left, top, font.getlength(char)))
atlas.save(root / "assets/ui-font.png", optimize=True)
(root / "assets/ui-font.bin").write_bytes(metrics)
print(f"Baked {count} glyphs into {width}x{height}")
