#!/usr/bin/env python3
"""Draws assets/icons/coffee_break.png (original art, CC0): the navy mug from the 3D model on a dark glowing field.

Run from the repo root: python3 scripts/coffee_icon.py
"""
from PIL import Image, ImageDraw, ImageFilter
import math

S = 4  # supersampling factor
N = 128 * S


def px(v):
    return int(v * S)


def radial(size, inner, outer, power=1.6):
    img = Image.new("RGBA", (size, size))
    pix = img.load()
    c = size / 2
    for y in range(size):
        for x in range(size):
            d = min(1.0, math.hypot(x - c, y - c) / (size * 0.62))
            t = d**power
            pix[x, y] = tuple(int(inner[i] * (1 - t) + outer[i] * t) for i in range(4))
    return img


img = radial(N, (28, 52, 110, 255), (4, 6, 16, 255))

# Soft blue glow behind the mug.
glow = Image.new("RGBA", (N, N), (0, 0, 0, 0))
ImageDraw.Draw(glow).ellipse([px(24), px(34), px(104), px(112)], fill=(70, 130, 255, 120))
img.alpha_composite(glow.filter(ImageFilter.GaussianBlur(px(10))))

d = ImageDraw.Draw(img)

# Mug body: a slightly tapered cup.
body = [(px(36), px(52)), (px(88), px(52)), (px(83), px(104)), (px(41), px(104))]
d.polygon(body, fill=(24, 44, 128, 255))
# Highlight on the left, shade on the right.
d.polygon([(px(40), px(56)), (px(49), px(56)), (px(47), px(100)), (px(43), px(100))], fill=(70, 100, 200, 255))
d.polygon([(px(78), px(56)), (px(86), px(56)), (px(81), px(100)), (px(75), px(100))], fill=(14, 26, 84, 255))
# Gold stripe.
d.polygon([(px(38), px(70)), (px(86), px(70)), (px(85), px(79)), (px(39), px(79))], fill=(242, 190, 40, 255))
# Handle.
d.arc([px(78), px(60), px(106), px(94)], 270, 90, fill=(24, 44, 128, 255), width=px(7))
# Coffee surface and rim.
d.ellipse([px(36), px(46), px(88), px(58)], fill=(222, 226, 244, 255))
d.ellipse([px(39), px(48), px(85), px(57)], fill=(40, 20, 10, 255))
d.ellipse([px(46), px(50), px(66), px(54)], fill=(96, 56, 28, 255))

# Steam: three wavy white wisps, blurred.
steam = Image.new("RGBA", (N, N), (0, 0, 0, 0))
sd = ImageDraw.Draw(steam)
for x0, ph in ((50, 0.0), (62, 1.7), (74, 3.1)):
    pts = [(px(x0 + 4 * math.sin(ph + y / 5.0)), px(y)) for y in range(14, 46)]
    sd.line(pts, fill=(235, 242, 255, 190), width=px(3.2), joint="curve")
img.alpha_composite(steam.filter(ImageFilter.GaussianBlur(px(1.6))))

# Round the corners like the other painterly icons.
mask = Image.new("L", (N, N), 0)
ImageDraw.Draw(mask).rounded_rectangle([0, 0, N - 1, N - 1], radius=px(10), fill=255)
img.putalpha(mask)

img.resize((128, 128), Image.LANCZOS).save("assets/icons/coffee_break.png")
