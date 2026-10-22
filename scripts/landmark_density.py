#!/usr/bin/env python3
"""Crowding report for the indoor landmark zones (tower, datacenter, lab), stdlib only.

    python3 -I scripts/landmark_density.py [props.txt]

Anchors come from content/zone_geometry.ron. The optional props file is the output of
`cargo test -p game_client zone_props -- --ignored --nocapture` (lines `PROP zone model x z solid`).
Mirrors the `landmark_density_gate` test in floors/zones.rs: interactable pairs under 2.5 m, the
busiest 10 m cells, and anchors that sit on the straight line from spawn to boss / exit elevator.
"""
import math
import re
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
NOT_INTERACTABLE = ("pack_", "night_", "rare_", "player_spawn", "graveyard", "boss", "world_boss", "elevator")
ZONES = [f"tower/{i}" for i in range(1, 6)] + ["datacenter/1", "datacenter/2", "lab/1", "lab/2"]


def anchors(text, zone):
    start = text.index(f'id: "{zone}"')
    end = text.index("blockers:", start)
    rows = re.findall(r'\("([^"]+)", \(([-0-9.e]+), [-0-9.e]+, ([-0-9.e]+)\)', text[start:end])
    return {n: (float(x), float(z)) for n, x, z in rows}


def seg_dist(p, a, b):
    ax, az, bx, bz = *a, *b
    dx, dz = bx - ax, bz - az
    t = max(0.0, min(1.0, ((p[0] - ax) * dx + (p[1] - az) * dz) / (dx * dx + dz * dz or 1)))
    return math.dist(p, (ax + t * dx, az + t * dz))


def main():
    text = (ROOT / "content" / "zone_geometry.ron").read_text()
    props = {}
    if len(sys.argv) > 1:
        for line in Path(sys.argv[1]).read_text().splitlines():
            f = line.split()
            if f and f[0] == "PROP":
                props.setdefault(f[1], []).append((float(f[3]), float(f[4])))
    for z in ZONES:
        a = anchors(text, z)
        things = {n: p for n, p in a.items() if not n.startswith(NOT_INTERACTABLE)}
        close = sorted(
            (math.dist(things[p], things[q]), p, q)
            for i, p in enumerate(sorted(things))
            for q in sorted(things)[i + 1 :]
            if math.dist(things[p], things[q]) < 2.5
        )
        cells = Counter((math.floor(x / 10), math.floor(y / 10)) for x, y in things.values())
        pc = Counter((math.floor(x / 10), math.floor(y / 10)) for x, y in props.get(z, []))
        lines = [(a["player_spawn"], a[t]) for t in ("boss", "elevator_exit") if t in a]
        on_line = [
            n for n, p in things.items()
            if n not in ("elevator_exit",) and any(seg_dist(p, s, e) < 1.5 for s, e in lines)
        ]
        print(
            f"{z}: {len(things)} interactables, {len(props.get(z, []))} props, pairs<2.5m {len(close)}, "
            f"max 10m cell {max(cells.values())} / props {max(pc.values(), default=0)}, on spawn-line {len(on_line)}"
        )
        for d, p, q in close:
            print(f"   {d:.1f} m {p} / {q}")


if __name__ == "__main__":
    main()
