#!/usr/bin/env python3
"""Regenerates the game-icons.net item icons in assets/icons/ from the GLYPHS table below.

Why: the styled 64x64 icons (white glyph on a coloured bevelled gradient) were made one at a time; this keeps the
recipe reproducible. The frame/gradient is not redrawn: each theme borrows it from an existing icon of that colour
(glyph-free border + a per-row gradient taken from column 3, which no glyph ever touches), so new icons match the
old ones pixel for pixel. The glyph is rasterized at 44x44 and placed at offset (10, 10), measured from badge.png.

Needs PIL and rsvg-convert on PATH; glyph SVGs (CC BY 3.0, https://github.com/game-icons/icons) are fetched into a
cache dir. Run from the repo root: python3 scripts/game_icons.py [cache_dir] [key ...]
Output is always 8-bit RGBA (16-bit PNGs crash WebGL2).
"""
import io
import shutil
import subprocess
import sys
import urllib.request
from pathlib import Path

from PIL import Image

ICONS = Path("assets/icons")
RAW = "https://raw.githubusercontent.com/game-icons/icons/master/{}.svg"

# Colour = bonus the item grants: an existing icon of that colour provides the frame.
THEME_REF = {
    "red": "medal",
    "blue": "pizza",
    "violet": "chart_up",
    "gold": "logo",
    "teal": "calendar",
    "green": "padlock",
}

# key -> (author folder/glyph, theme)
GLYPHS = {
    "access_badge": ("andymeneely/police-badge", "red"),
    "delivery_receipt": ("delapouite/door", "teal"),
    "scooter_helmet": ("delapouite/full-motorcycle-helmet", "red"),
    "oat_milk_prompt": ("rihlsul/milk-carton", "blue"),
    "stress_ball": ("delapouite/throwing-ball", "green"),
    "gpu_candle": ("lorc/candle-flame", "violet"),
    "rack_screw": ("delapouite/screw", "red"),
    "token_invoice": ("delapouite/price-tag", "blue"),
    "agent_memory": ("lorc/microchip", "gold"),
    "cron": ("lorc/clockwork", "teal"),
    "queue_ticket": ("delapouite/human-pyramid", "red"),
    "priority_low": ("lorc/snail", "green"),
    "thermal_paste": ("lorc/dripping-tube", "gold"),
    "incident_report": ("delapouite/thermometer-hot", "red"),
    "fan_blade": ("delapouite/computer-fan", "teal"),
    "demo_laptop": ("delapouite/laptop", "violet"),
    "term_sheet": ("delapouite/scroll-quill", "red"),
    "prompt_certificate": ("delapouite/ribbon-medal", "blue"),
    "rack_chair": ("delapouite/server-rack", "red"),
    "still_typing": ("delapouite/discussion", "gold"),
    "benchmark": ("delapouite/podium-winner", "teal"),
    "visitor_badge": ("delapouite/mug-shot", "red"),
    "judgment": ("lorc/semi-closed-eye", "gold"),
    "lighter": ("delapouite/lighter", "violet"),
    "room_booking": ("delapouite/checklist", "teal"),
    "wont_fix": ("sbed/cancel", "red"),
    "human_inside": ("delapouite/person", "red"),
    "demo_lanyard": ("lorc/ribbon", "green"),
    # Economy: gear slots, consumables, junk and the Story Points coin.
    "gear_headset": ("delapouite/headphones", "violet"),
    "gear_hoodie": ("delapouite/hoodie", "blue"),
    "gear_mug": ("delapouite/coffee-cup", "gold"),
    "gear_beanie": ("delapouite/winter-hat", "teal"),
    "gear_propeller": ("delapouite/propeller-beanie", "green"),
    "gear_vr": ("delapouite/vr-headset", "violet"),
    "gear_shades": ("delapouite/sunglasses", "red"),
    "gear_idcard": ("delapouite/id-card", "teal"),
    "gear_extinguisher": ("delapouite/fire-extinguisher", "red"),
    "gear_stein": ("lorc/beer-stein", "gold"),
    "gear_tshirt": ("delapouite/t-shirt", "green"),
    "gear_desk": ("delapouite/desk", "teal"),
    "gear_keyboard": ("delapouite/keyboard", "blue"),
    "coffee_pot": ("delapouite/coffee-pot", "teal"),
    "pills": ("delapouite/medicine-pills", "green"),
    "sandwich": ("delapouite/sandwich", "gold"),
    "poster": ("lorc/scroll-unfurled", "violet"),
    "junk_ticket": ("delapouite/ticket", "blue"),
    "junk_trash": ("delapouite/trash-can", "green"),
    "junk_candy": ("lorc/wrapped-sweet", "red"),
    "story_points": ("delapouite/two-coins", "gold"),
    # Item expansion: visible gear variants, junk, trophies and snacks.
    "gear_tophat": ("lorc/top-hat", "gold"),
    "gear_partyhat": ("delapouite/party-hat", "violet"),
    "gear_hardhat": ("delapouite/mining-helmet", "gold"),
    "gear_ringlight": ("delapouite/smartphone", "violet"),
    "gear_cape": ("delapouite/cape", "red"),
    "gear_antenna": ("delapouite/robot-antennas", "teal"),
    "gear_soda": ("guard13007/soda-can", "blue"),
    "gear_moka": ("delapouite/moka-pot", "gold"),
    "gear_keytar": ("delapouite/musical-keyboard", "violet"),
    "gear_vest": ("lorc/leather-vest", "gold"),
    "gear_usb": ("delapouite/usb-key", "green"),
    "gear_tie": ("delapouite/tie", "blue"),
    "gear_trophy": ("delapouite/trophy-cup", "gold"),
    "junk_donut": ("delapouite/donut", "gold"),
    "cake": ("lorc/cake-slice", "violet"),
    "junk_socks": ("delapouite/socks", "teal"),
    "alarm": ("delapouite/alarm-clock", "red"),
    "stopwatch": ("lorc/stopwatch", "teal"),
    "briefcase": ("delapouite/briefcase", "gold"),
    "newspaper": ("delapouite/newspaper", "blue"),
    "cookie": ("delapouite/cookie", "gold"),
    "chocolate": ("rihlsul/chocolate-bar", "red"),
    "soda_bottle": ("caro-asercion/soda-bottle", "green"),
    "microphone": ("delapouite/microphone", "violet"),
    "paper_plane": ("delapouite/paper-plane", "blue"),
    "book_pile": ("delapouite/book-pile", "red"),
    "desk_lamp": ("lorc/desk-lamp", "gold"),
    "battery_low": ("priorblue/battery-25", "green"),
}

GLYPH_PX, OFFSET, FRAME = 44, 10, 3


def fetch(path, cache):
    f = cache / (path.replace("/", "__") + ".svg")
    if not f.exists():
        with urllib.request.urlopen(RAW.format(path), timeout=30) as r:  # raises on non-200
            f.write_bytes(r.read())
    return f.read_text()


def template(theme):
    ref = Image.open(ICONS / f"{THEME_REF[theme]}.png").convert("RGBA")
    px = ref.load()
    for y in range(FRAME, 64 - FRAME):
        for x in range(FRAME, 64 - FRAME):
            px[x, y] = px[FRAME, y]
    return ref


def render(svg, theme):
    # Drop the opaque black background square so only the glyph is rasterized.
    svg = svg.replace('<path d="M0 0h512v512H0z"/>', "")
    png = subprocess.run(
        ["rsvg-convert", "-w", str(GLYPH_PX), "-h", str(GLYPH_PX), "-"],
        input=svg.encode(), capture_output=True, check=True,
    ).stdout
    g = Image.open(io.BytesIO(png)).convert("RGBA")
    # White glyph: keep only alpha, force colour white.
    white = Image.new("RGBA", g.size, (255, 255, 255, 255))
    white.putalpha(g.getchannel("A"))
    img = template(theme)
    img.alpha_composite(white, (OFFSET, OFFSET))
    return img


def main():
    args = sys.argv[1:]
    cache = Path(args[0]) if args else Path("/tmp/game_icons_cache")
    cache.mkdir(parents=True, exist_ok=True)
    keys = args[1:] or list(GLYPHS)
    assert shutil.which("rsvg-convert"), "rsvg-convert not found"
    for k in keys:
        path, theme = GLYPHS[k]
        img = render(fetch(path, cache), theme)
        img.save(ICONS / f"{k}.png")
        print(k, path, theme)


if __name__ == "__main__":
    main()
