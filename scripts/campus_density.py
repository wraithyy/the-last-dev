#!/usr/bin/env python3
"""Campus crowding report from content/zone_geometry.ron (campus section, stdlib only).

    python3 -I scripts/campus_density.py

Prints the same numbers as the `campus_density_gate` test in floors/zones.rs: 20 m cell counts,
the 40 m discs around the spawn and the plaza centre, and interactable pairs closer than 3 m.
"""
import math
import re
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
NOT_INTERACTABLE = ("pack_", "night_", "rare_", "player_spawn", "graveyard", "boss", "world_boss")
# Calendar return points: no door piece, no trigger; they share the calendar's point on purpose.
MINIGAME_DOORS = {
    f"door:{v}/1" for v in ("teambuilding", "review", "reorg", "migration", "hackathon", "openspace", "home")
}


def campus_anchors():
    text = (ROOT / "content" / "zone_geometry.ron").read_text()
    start = text.index('id: "campus"')
    end = text.index("blockers:", start)
    rows = re.findall(r'\("([^"]+)", \(([-0-9.e]+), [-0-9.e]+, ([-0-9.e]+)\)', text[start:end])
    return {n: (float(x), float(z)) for n, x, z in rows}


def main():
    a = campus_anchors()
    things = {n: p for n, p in a.items() if not n.startswith(NOT_INTERACTABLE) and n not in MINIGAME_DOORS}
    print(f"{len(a)} anchors, {len(things)} interactables")
    cells = Counter((math.floor(x / 20), math.floor(z / 20)) for x, z in things.values())
    print("busiest 20 m cells (x0, z0: count):")
    for (i, j), n in cells.most_common(6):
        print(f"  [{i * 20},{i * 20 + 20}) x [{j * 20},{j * 20 + 20}): {n}")
    for label, c in (("spawn", a["player_spawn"]), ("plaza (0,10)", (0.0, 10.0))):
        n = sum(1 for p in things.values() if math.dist(p, c) <= 40.0)
        print(f"interactables within 40 m of {label}: {n}")
    names = sorted(things)
    close = [
        (math.dist(things[p], things[q]), p, q)
        for i, p in enumerate(names)
        for q in names[i + 1 :]
        if math.dist(things[p], things[q]) < 3.0
    ]
    print(f"pairs under 3 m: {len(close)}")
    for d, p, q in sorted(close):
        print(f"  {d:.1f} m  {p} / {q}")


if __name__ == "__main__":
    main()
