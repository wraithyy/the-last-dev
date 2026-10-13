#!/usr/bin/env python3
"""Draws the PWA icons (a terminal-prompt keycap in the HUD palette) into web/icons. Needs Pillow."""
import os
from PIL import Image, ImageDraw

OUT = os.path.join(os.path.dirname(__file__), "..", "web", "icons")
BG = (16, 18, 22)
KEY_TOP = (51, 56, 71)
KEY_SIDE = (15, 18, 26)
RIM = (102, 112, 138)
ACCENT = (92, 209, 219)
WARM = (255, 140, 64)
SS = 4  # supersample for smooth edges


def draw(size, maskable):
    n = size * SS
    img = Image.new("RGB", (n, n), BG)
    d = ImageDraw.Draw(img)
    # Maskable icons are cropped to a circle-ish safe zone, so the art stays inside the middle 60%.
    span = 0.6 if maskable else 0.78
    half = n * span / 2
    c = n / 2
    box = (c - half, c - half, c + half, c + half)
    r = half * 0.28
    lip = half * 0.1
    d.rounded_rectangle((box[0], box[1] + lip, box[2], box[3] + lip), r, fill=KEY_SIDE)
    d.rounded_rectangle(box, r, fill=RIM)
    inset = half * 0.05
    d.rounded_rectangle(
        (box[0] + inset, box[1] + inset, box[2] - inset, box[3] - inset), r * 0.85, fill=KEY_TOP
    )
    # ">_" prompt
    w = half * 0.09
    x0, x1 = c - half * 0.5, c - half * 0.1
    cy = c - half * 0.05
    for a, b in (((x0, cy - half * 0.4), (x1, cy)), ((x1, cy), (x0, cy + half * 0.4))):
        d.line((a, b), fill=ACCENT, width=int(w * 2))
        d.ellipse((a[0] - w, a[1] - w, a[0] + w, a[1] + w), fill=ACCENT)
        d.ellipse((b[0] - w, b[1] - w, b[0] + w, b[1] + w), fill=ACCENT)
    d.rounded_rectangle(
        (c + half * 0.05, cy + half * 0.3, c + half * 0.55, cy + half * 0.3 + w * 2.2), w, fill=WARM
    )
    return img.resize((size, size), Image.LANCZOS)


os.makedirs(OUT, exist_ok=True)
for name, size, maskable in (
    ("icon-192.png", 192, False),
    ("icon-512.png", 512, False),
    ("icon-maskable-512.png", 512, True),
    ("apple-touch-icon.png", 180, False),
):
    draw(size, maskable).save(os.path.join(OUT, name), optimize=True)
