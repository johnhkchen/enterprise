#!/usr/bin/env python3
"""Floor plans of a structure, from above, and how usable its interior is (John: "a common language for talking about
interior spaces ... your own overhead floor plan analysis tool"; "sane interior design, both in terms of room layout,
walkway length to room volume ratios").

    python3 scripts/assets/floorplan.py STRUCTURE.nbt [...] [--out DIR] [--json]

For each storey: the interior (air no way in from outside but a door), its rooms (floor split at doors), what each room
is (by what's in it), the routes a villager walks from the street door to every bed, workstation and stair, and an
overhead plan (DIR/<name>-<storey>.png). The metrics, defined in docs/assets/interiors.md:

- floor (GFA): interior cells a villager can stand in; clear height over them.
- rooms: each one's floor, its least width, its use (hall, chamber, work, store, stair, passage), its windows.
- circulation: the cells on the shortest routes from the door to every destination, as a share of the floor.
- idle floor: floor neither on a route nor beside anything used (a bed, a workstation, a chest, a hearth, a table).
- walk: the routes' mean length over the root of the floor (how far you walk for the size of the house).
- made beds: beds with a free cell along a long side (a bed you can get into).
- through rooms: chambers a route to someone else's bed passes through.

Reads gzipped structure NBT with `dev/experiments/world_render.py`'s reader. No dependencies.
"""
import gzip
import json
import math
import sys
from collections import deque
from pathlib import Path

# world_render.py: beside this file (the studio's shared tree, py/) or in our repo's dev/experiments.
sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "dev/experiments"))
from world_render import Reader, png  # noqa: E402

AIR = ("air", "cave_air", "void_air", "structure_void")
PASSABLE_WORDS = ("_door", "carpet", "pressure_plate", "torch", "_sign", "button", "rail",
                  "flower", "sapling", "short_grass", "tall_grass", "fern", "snow", "lantern", "banner", "item_frame",
                  "candle", "lever", "tripwire", "vine", "ladder", "_head", "skull", "frame")
OBSTACLE = ("_bed", "chest", "barrel", "table", "lectern", "loom", "stonecutter", "grindstone", "furnace", "smoker",
            "composter", "cauldron", "brewing_stand", "anvil", "bookshelf", "campfire", "crafting_table", "decor_")
USED = ("_bed", "chest", "barrel", "table", "lectern", "loom", "stonecutter", "grindstone", "furnace", "smoker",
        "composter", "cauldron", "brewing_stand", "anvil", "bookshelf", "campfire", "crafting_table", "decor_", "lantern")
JOB_SITES = ("lectern", "loom", "stonecutter", "grindstone", "smoker", "blast_furnace", "composter", "cauldron",
             "brewing_stand", "smithing_table", "fletching_table", "cartography_table", "barrel")
WINDOW = ("glass",)


def load(path):
    root = Reader(gzip.decompress(Path(path).read_bytes())).root()
    size = root["size"]
    pal = [(p["Name"].split(":")[-1], p.get("Properties", {})) for p in root["palette"]]
    blocks = {}
    for b in root["blocks"]:
        x, y, z = b["pos"]
        blocks[x, y, z] = pal[b["state"]]
    return size, blocks


def name(blocks, p):
    return blocks.get(p, ("air", {}))[0]


def passable(n):
    return n in AIR or any(w in n for w in PASSABLE_WORDS) and not n.endswith("_bed")


def door(n):
    return n.endswith("_door")


def support(n):
    """Floor to stand on: not air, a plant, a door, a fence or a wall top, nor furniture (a villager may climb onto a
    bed or a chest, but it isn't floor)."""
    return not passable(n) and not any(w in n for w in ("fence", "_wall", "pane", "bars")) and not any(
        u in n for u in OBSTACLE)


def analyse(file):
    (sx, sy, sz), blocks = load(file)
    inside = lambda p: 0 <= p[0] < sx and 0 <= p[1] < sy and 0 <= p[2] < sz
    # Outside: the air reached from beyond the template without passing a door, a window or a wall.
    outside = set()
    q = deque()
    for x in range(-1, sx + 1):
        for y in range(-1, sy + 1):
            for z in range(-1, sz + 1):
                if not inside((x, y, z)):
                    outside.add((x, y, z))
    for p in list(outside):
        for d in ((1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)):
            n = (p[0] + d[0], p[1] + d[1], p[2] + d[2])
            if inside(n) and n not in outside and passable(name(blocks, n)) and not door(name(blocks, n)):
                outside.add(n)
                q.append(n)
    while q:
        p = q.popleft()
        for d in ((1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)):
            n = (p[0] + d[0], p[1] + d[1], p[2] + d[2])
            if inside(n) and n not in outside and passable(name(blocks, n)) and not door(name(blocks, n)):
                outside.add(n)
                q.append(n)
    # Where a villager can stand (feet at p): passable here and over its head, something to stand on below.
    stand = set()
    for x in range(sx):
        for z in range(sz):
            for y in range(1, sy - 1):
                p = (x, y, z)
                if passable(name(blocks, p)) and passable(name(blocks, (x, y + 1, z))) \
                        and support(name(blocks, (x, y - 1, z))):
                    stand.add(p)
    interior = {p for p in stand if p not in outside or door(name(blocks, p))}
    # Entrances: door cells with outside standing ground next to them.
    entrances = [p for p in interior if door(name(blocks, p)) and any(
        (p[0] + dx, p[1], p[2] + dz) in outside and (p[0] + dx, p[1], p[2] + dz) in stand
        for dx, dz in ((1, 0), (-1, 0), (0, 1), (0, -1)))]
    interior -= {p for p in interior if door(name(blocks, p)) and p in entrances}
    # Moves: level to a neighbour; up or down a block onto or off a stair or slab (a half step).
    def half(p):
        below = name(blocks, (p[0], p[1] - 1, p[2]))
        return "stairs" in below or "slab" in below
    def moves(p):
        for dx, dz in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            for dy in (0, 1, -1):
                n = (p[0] + dx, p[1] + dy, p[2] + dz)
                if n in interior and (dy == 0 or half(n) or half(p)):
                    if dy == 1 and not passable(name(blocks, (p[0], p[1] + 2, p[2]))):
                        continue
                    yield n
    starts = [n for e in entrances for n in moves(e)] or ([min(interior)] if interior else [])
    dist, prev = {}, {}
    q = deque()
    for s in starts:
        dist[s] = 1
        q.append(s)
    while q:
        p = q.popleft()
        for n in moves(p):
            if n not in dist:
                dist[n] = dist[p] + 1
                prev[n] = p
                q.append(n)
    # Destinations: a cell beside each bed, workstation, chest, hearth: the nearest reached.
    used = {p: name(blocks, p) for p in blocks if any(w in name(blocks, p) for w in USED)}
    beds = {p for p, n in used.items() if n.endswith("_bed") and blocks[p][1].get("part") == "head"}
    def beside(p):
        return [n for n in interior if abs(n[0] - p[0]) + abs(n[2] - p[2]) == 1 and n[1] in (p[1], p[1] + 1)]
    def foot(p):
        f = {"north": (0, 1), "south": (0, -1), "east": (-1, 0), "west": (1, 0)}[blocks[p][1].get("facing", "north")]
        return (p[0] + f[0], p[1], p[2] + f[1])
    routes, unreached = [], []
    for p, n in sorted(used.items()):
        if n.endswith("_bed") and p not in beds or n == "lantern":
            continue
        # A bed is got into from beside its head or its foot.
        reach = [c for c in beside(p) + (beside(foot(p)) if p in beds else []) if c in dist]
        if not reach:
            if n.endswith("_bed") or any(j in n for j in JOB_SITES):
                unreached.append((n, p))
            continue
        end = min(reach, key=lambda c: dist[c])
        path = [end]
        while path[-1] in prev:
            path.append(prev[path[-1]])
        routes.append((n, p, path[::-1]))
    circulation = {c for _, _, path in routes for c in path}
    near_used = {c for p in used for c in beside(p)}
    # Storeys: by feet height, the floor of each.
    levels = sorted({p[1] for p in interior})
    storeys = []
    for y in levels:
        floor = {p for p in interior if p[1] == y}
        if len(floor) < 4:
            continue
        # Rooms: floor cells joined level, split at doors (a door is its own cell between rooms).
        seen, rooms = set(), []
        for c in sorted(floor):
            if c in seen or door(name(blocks, c)):
                continue
            room, q = [], deque([c])
            seen.add(c)
            while q:
                p = q.popleft()
                room.append(p)
                for dx, dz in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                    n = (p[0] + dx, y, p[2] + dz)
                    if n in floor and n not in seen and not door(name(blocks, n)):
                        seen.add(n)
                        q.append(n)
            xs, zs = [p[0] for p in room], [p[2] for p in room]
            contents = {used[u] for u in used if any(abs(u[0] - p[0]) + abs(u[2] - p[2]) <= 1 and u[1] in (y, y - 1, y + 1)
                                                     for p in room)}
            use = ("chamber" if any(n.endswith("_bed") for n in contents) else
                   "work" if any(j in n for n in contents for j in JOB_SITES) else
                   "stair" if all(half(p) for p in room) else
                   "hall" if any(abs(e[0] - p[0]) + abs(e[2] - p[2]) == 1 for e in entrances for p in room) else
                   "store" if any("chest" in n or "barrel" in n for n in contents) else
                   "passage" if len(room) <= 4 or min(max(xs) - min(xs), max(zs) - min(zs)) == 0 else "room")
            windows = sum(1 for p in room for dx, dz in ((1, 0), (-1, 0), (0, 1), (0, -1))
                          for dy in (0, 1) if any(w in name(blocks, (p[0] + dx, p[1] + dy, p[2] + dz)) for w in WINDOW))
            heights = []
            for p in room:
                h = 0
                while passable(name(blocks, (p[0], p[1] + h, p[2]))) and p[1] + h < sy:
                    h += 1
                heights.append(h)
            rooms.append({"use": use, "floor": len(room), "bbox": [max(xs) - min(xs) + 1, max(zs) - min(zs) + 1],
                          "least_width": least_width(room), "windows": windows,
                          "clear_height": sorted(heights)[len(heights) // 2] if heights else 0,
                          "low_cells": sum(1 for h in heights if h < 3),
                          "volume": sum(heights), "contents": sorted(contents), "cells": room})
        circ = len(circulation & floor)
        idle = len([c for c in floor if c not in circulation and c not in near_used])
        storeys.append({"feet_y": y, "floor": len(floor), "rooms": rooms, "circulation": circ,
                        "circulation_share": round(circ / len(floor), 2), "idle_share": round(idle / len(floor), 2)})
    floor_total = sum(s["floor"] for s in storeys) or 1
    bed_routes = [len(path) for n, p, path in routes if n.endswith("_bed")]
    made = 0
    for b in beds:
        props = blocks[b][1]
        facing = props.get("facing", "north")
        foot = {"north": (0, 1), "south": (0, -1), "east": (-1, 0), "west": (1, 0)}[facing]
        cells = [b, (b[0] + foot[0], b[1], b[2] + foot[1])]
        side = [(1, 0), (-1, 0)] if facing in ("north", "south") else [(0, 1), (0, -1)]
        if any((c[0] + dx, c[1], c[2] + dz) in interior for c in cells for dx, dz in side):
            made += 1
    chambers = {i for s in storeys for i, r in enumerate(s["rooms"]) if r["use"] == "chamber"}
    through = 0
    room_of = {}
    for si, s in enumerate(storeys):
        for ri, r in enumerate(s["rooms"]):
            for c in r["cells"]:
                room_of[c] = (si, ri, r["use"])
    for n, p, path in routes:
        if not n.endswith("_bed"):
            continue
        dest_room = room_of.get(path[-1])
        passed = {room_of[c] for c in path[:-1] if c in room_of and room_of[c][2] == "chamber" and room_of[c] != dest_room}
        through += len(passed)
    report = {
        "file": Path(file).name, "size": [sx, sy, sz], "storeys": len(storeys), "floor": floor_total,
        "rooms": sum(len(s["rooms"]) for s in storeys), "beds": len(beds), "made_beds": made,
        "unreached": [f"{n} at {list(p)}" for n, p in unreached],
        "circulation_share": round(sum(s["circulation"] for s in storeys) / floor_total, 2),
        "idle_share": round(sum(s["idle_share"] * s["floor"] for s in storeys) / floor_total, 2),
        "walk": round(sum(bed_routes) / len(bed_routes) / math.sqrt(floor_total), 2) if bed_routes else None,
        "through_rooms": through,
        "floor_per_bed": round(sum(r["floor"] for st in storeys for r in st["rooms"] if r["use"] == "chamber")
                               / len(beds), 1) if beds else None,
        "per_storey": [{k: v for k, v in s.items() if k != "rooms"} | {"rooms": [{k: v for k, v in r.items() if k != "cells"}
                                                                               for r in s["rooms"]]} for s in storeys],
    }
    return report, (sx, sy, sz), blocks, storeys, interior, circulation, entrances, beds


def least_width(cells):
    """How narrow the room gets: each cell's shorter run of floor across it (east-west or north-south), the tenth
    percentile over the room's cells (the least alone is a corner's or a doorway's)."""
    s = set(cells)
    widths = []
    for (x, y, z) in cells:
        runs = []
        for dx, dz in ((1, 0), (0, 1)):
            run = 1
            for sgn in (-1, 1):
                a = (x + sgn * dx, y, z + sgn * dz)
                while a in s:
                    run += 1
                    a = (a[0] + sgn * dx, y, a[2] + sgn * dz)
            runs.append(run)
        widths.append(min(runs))
    widths.sort()
    return widths[len(widths) // 10] if widths else 0


COLOURS = {"hall": (236, 214, 170), "chamber": (196, 214, 238), "work": (238, 200, 160), "store": (214, 206, 190),
           "stair": (200, 200, 200), "passage": (232, 228, 214), "room": (226, 226, 210)}


def draw(out, sizes, blocks, storeys, circulation, entrances, interior=frozenset(), scale=14):
    sx, sy, sz = sizes
    sheets = []
    for i, s in enumerate(storeys):
        y = s["feet_y"]
        w, h = sx * scale, sz * scale
        img = [[(250, 250, 248)] * w for _ in range(h)]

        def fill(x, z, col, inset=0):
            for a in range(x * scale + inset, (x + 1) * scale - inset):
                for b in range(z * scale + inset, (z + 1) * scale - inset):
                    img[b][a] = col
        for x in range(sx):
            for z in range(sz):
                n0, n1 = name(blocks, (x, y, z)), name(blocks, (x, y + 1, z))
                if not passable(n0) or not passable(n1):
                    n = n1 if not passable(n1) else n0
                    col = ((120, 200, 230) if any(g in n for g in WINDOW) else
                           (70, 110, 200) if n.endswith("_bed") else
                           (230, 140, 50) if any(j in n for j in JOB_SITES) or "decor_" in n else
                           (150, 110, 70) if any(f in n for f in USED) else
                           (60, 56, 52))
                    fill(x, z, col)
        for r in s["rooms"]:
            for (x, _, z) in r["cells"]:
                fill(x, z, COLOURS.get(r["use"], (226, 226, 210)))
        # The stair: its steps underfoot or rising ahead on this storey.
        for x in range(sx):
            for z in range(sz):
                if any("stairs" in name(blocks, (x, y + dy, z)) and any((x, y + dy + k, z) in interior for k in (0, 1))
                       for dy in (-1, 0, 1)) and (x, y, z) not in [
                        c for r in s["rooms"] for c in r["cells"] if r["use"] != "stair"]:
                    fill(x, z, (150, 150, 150), inset=1)
        for (x, yy, z) in circulation:
            if yy == y:
                fill(x, z, (210, 70, 60), inset=scale // 3)
        for (x, yy, z) in entrances:
            if yy == y:
                fill(x, z, (40, 160, 70))
        for x in range(sx):
            for z in range(sz):
                if name(blocks, (x, y, z)).endswith("_door"):
                    fill(x, z, (130, 80, 40), inset=scale // 4)
        sheets.append(img)
    # One sheet: the storeys side by side, ground floor first, a gap between.
    gap = scale
    w = sum(len(img[0]) for img in sheets) + gap * (len(sheets) + 1)
    h = max(len(img) for img in sheets) + 2 * gap
    rows = []
    for b_ in range(h):
        row = bytearray((255, 255, 255) * gap)
        for img in sheets:
            r = b_ - gap
            row += bytes(v for px in img[r] for v in px) if 0 <= r < len(img) else bytes((255, 255, 255)) * len(img[0])
            row += bytes((255, 255, 255)) * gap
        rows.append(bytes(row))
    for old in out.glob("*.png"):
        old.unlink()
    f = out / "plan.png"
    f.write_bytes(png(w, h, rows))
    return [f.name]


def main():
    args = [a for i, a in enumerate(sys.argv[1:], 1) if not a.startswith("--") and sys.argv[i - 1] != "--out"]
    out = Path(sys.argv[sys.argv.index("--out") + 1]) if "--out" in sys.argv else None
    reports = []
    for path in args:
        report, sizes, blocks, storeys, interior, circulation, entrances, beds = analyse(path)
        if out:
            d = out / Path(path).stem
            d.mkdir(parents=True, exist_ok=True)
            report["plans"] = draw(d, sizes, blocks, storeys, circulation, entrances, interior)
        reports.append(report)
        if "--json" not in sys.argv:
            print(f"{report['file']}: {report['storeys']} storeys, floor {report['floor']}, {report['rooms']} rooms; "
                  f"beds {report['made_beds']}/{report['beds']} made; circulation {report['circulation_share']}, "
                  f"idle {report['idle_share']}, walk {report['walk']}, through rooms {report['through_rooms']}, "
                  f"floor a bed {report['floor_per_bed']}"
                  + (f"; UNREACHED {report['unreached']}" if report["unreached"] else ""))
            for si, s in enumerate(report["per_storey"]):
                print(f"  storey {si + 1} (feet y {s['feet_y']}): floor {s['floor']}, circulation {s['circulation_share']},"
                      f" idle {s['idle_share']}")
                for r in s["rooms"]:
                    print(f"    {r['use']:8} floor {r['floor']:3}  {r['bbox'][0]}x{r['bbox'][1]}  least width "
                          f"{r['least_width']}  clear {r['clear_height']} ({r['low_cells']} low)  windows {r['windows']}  "
                          f"{', '.join(r['contents'])}")
    if "--json" in sys.argv:
        print(json.dumps(reports, indent=1))


if __name__ == "__main__":
    main()
