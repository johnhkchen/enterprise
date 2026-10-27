#!/usr/bin/env python3
"""Floor plans of a structure, from above, and how usable its interior is (John: "a common language for talking about
interior spaces ... your own overhead floor plan analysis tool"; "sane interior design, both in terms of room layout,
walkway length to room volume ratios").

    python3 scripts/assets/floorplan.py STRUCTURE.nbt [...] [--out DIR] [--json] [--check]

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
HEARTH = ("furnace", "campfire", "fireplace")


def job(n):
    """A villager's job site (the decorative twins, `decor_*`, are none)."""
    return not n.startswith("decor_") and any(j in n for j in JOB_SITES)


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


LAST = {}  # the last analysis's walk (for looking into a route)


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
    LAST["dist"], LAST["interior"] = dist, interior
    # Destinations: a cell beside each bed, workstation, chest, hearth: the nearest reached.
    used = {p: name(blocks, p) for p in blocks if any(w in name(blocks, p) for w in USED)}
    beds = {p for p, n in used.items() if n.endswith("_bed") and blocks[p][1].get("part") == "head"}
    def beside(p):
        return [n for n in interior if abs(n[0] - p[0]) + abs(n[2] - p[2]) == 1 and n[1] in (p[1], p[1] + 1)]
    def foot(p):
        f = {"north": (0, 1), "south": (0, -1), "east": (-1, 0), "west": (1, 0)}[blocks[p][1].get("facing", "north")]
        return (p[0] + f[0], p[1], p[2] + f[1])
    # Where villagers go (the studio, thread 0008): beds, workstations, the hearth, chests. Shelves, casks and decor are
    # used floor for idle but no destination: a route to each of them would make the whole room circulation.
    def goes_to(n):
        return n.endswith("_bed") or job(n) and "barrel" not in n or "chest" in n or any(
            h in n for h in HEARTH)
    routes, unreached = [], []
    for p, n in sorted(used.items()):
        if n.endswith("_bed") and p not in beds or not goes_to(n):
            continue
        # A bed is got into from beside its head or its foot.
        reach = [c for c in beside(p) + (beside(foot(p)) if p in beds else []) if c in dist]
        if not reach:
            if n.endswith("_bed") or job(n):
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
    def step(p):
        return "stairs" in name(blocks, (p[0], p[1] - 1, p[2]))
    for y in levels:
        # A stair's steps belong to no storey.
        floor = {p for p in interior if p[1] == y and not step(p)}
        if len(floor) < 4:
            continue
        # Rooms: the floor and what stands on it (furniture doesn't divide a room or narrow it: thread 0008), joined
        # level, split only at walls and doors.
        furnished = {(u[0], y, u[2]) for u in used if u[1] == y and support(name(blocks, (u[0], y - 1, u[2])))
                     and (u[0], y, u[2]) not in outside}
        space = floor | furnished
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
                    if n in space and n not in seen and not door(name(blocks, n)):
                        seen.add(n)
                        q.append(n)
            room_space = room
            room = [p for p in room_space if p in floor]
            xs, zs = [p[0] for p in room], [p[2] for p in room]
            contents = {used[u] for u in used if any(abs(u[0] - p[0]) + abs(u[2] - p[2]) <= 1 and u[1] in (y, y - 1, y + 1)
                                                     for p in room)}
            use = ("chamber" if any(n.endswith("_bed") for n in contents) else
                   "work" if any(job(n) for n in contents) else
                   "stair" if all(half(p) for p in room) else
                   "landing" if not any(abs(e[0] - p[0]) + abs(e[2] - p[2]) == 1 and e[1] == p[1] for e in entrances
                                        for p in room) and any(
                       step((p[0] + dx, p[1] + dy, p[2] + dz)) or "stairs" in name(blocks, (p[0] + dx, p[1], p[2] + dz))
                       for p in room for dx, dz in ((1, 0), (-1, 0), (0, 1), (0, -1)) for dy in (0, 1, -1)) else
                   "hall" if any(abs(e[0] - p[0]) + abs(e[2] - p[2]) == 1 and e[1] == p[1] for e in entrances for p in room) else
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
                          "least_width": least_width(room_space), "windows": windows,
                          "clear_height": sorted(heights)[len(heights) // 2] if heights else 0,
                          "low_cells": sum(1 for h in heights if h < 3),
                          "volume": sum(heights), "contents": sorted(contents), "cells": room})
        # A landing is designed circulation, counted whole, but not toward the share's cap (thread 0004): the share
        # judges the routes left over, not the landing a stair needs.
        landing = set()
        for r in rooms:
            if r["use"] == "landing":
                circulation |= set(r["cells"])
                landing |= set(r["cells"])
        circ = len((circulation & floor) - landing)
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
    # Annotations (John: "an annotation system so your input is not detached"): each finding pinned to a cell, numbered
    # on the plan, and written out as text that can be searched.
    notes = []
    level_of = {s_["feet_y"]: i for i, s_ in enumerate(storeys)}

    def note(kind, at, words):
        for old in notes:
            if old["kind"] == kind and old["at"] == list(at):
                old["text"] += "; " + words.split("passes through")[0].replace("the way to ", "and to ").strip()
                return
        st = level_of.get(at[1], min(level_of.items(), key=lambda kv: abs(kv[0] - at[1]))[1] if level_of else 0)
        notes.append({"n": len(notes) + 1, "storey": st + 1, "at": list(at), "kind": kind, "text": words})
    for b in sorted(beds):
        facing = blocks[b][1].get("facing", "north")
        f = {"north": (0, 1), "south": (0, -1), "east": (-1, 0), "west": (1, 0)}[facing]
        side = [(1, 0), (-1, 0)] if facing in ("north", "south") else [(0, 1), (0, -1)]
        if not any((c[0] + dx, c[1], c[2] + dz) in interior for c in (b, (b[0] + f[0], b[1], b[2] + f[1])) for dx, dz in side):
            note("unmade-bed", b, "bed with no free long side: it can't be got into or made")
    for n_, p_, path_ in routes:
        if not n_.endswith("_bed"):
            continue
        dest = room_of.get(path_[-1])
        for c in path_[:-1]:
            r_ = room_of.get(c)
            if r_ and r_[2] == "chamber" and r_ != dest:
                note("through-room", c, f"the way to the bed at {list(p_)} passes through this chamber")
                break
    for si, st in enumerate(storeys):
        floor = {c for r in st["rooms"] for c in r["cells"]}
        idle = {c for c in floor if c not in circulation and c not in near_used}
        if st["idle_share"] > 0.4 and idle:
            # The largest idle patch: its middle cell.
            best, seen = [], set()
            for c in sorted(idle):
                if c in seen:
                    continue
                patch, q2 = [], deque([c])
                seen.add(c)
                while q2:
                    p2 = q2.popleft()
                    patch.append(p2)
                    for dx, dz in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                        n2 = (p2[0] + dx, p2[1], p2[2] + dz)
                        if n2 in idle and n2 not in seen:
                            seen.add(n2)
                            q2.append(n2)
                best = max(best, patch, key=len)
            mx = sum(c[0] for c in best) / len(best)
            mz = sum(c[2] for c in best) / len(best)
            mid = min(best, key=lambda c: (c[0] - mx) ** 2 + (c[2] - mz) ** 2)
            note("idle-floor", mid, f"{len(idle)} of {len(floor)} cells idle on this storey ({len(best)} here): "
                                    "no use beside them, no route over them")
        for r in st["rooms"]:
            mx = sum(c[0] for c in r["cells"]) / len(r["cells"])
            mz = sum(c[2] for c in r["cells"]) / len(r["cells"])
            mid = min(r["cells"], key=lambda c: (c[0] - mx) ** 2 + (c[2] - mz) ** 2)
            # Rooms 3 wide or more; a landing or a passage 2 (interiors.md).
            if r["least_width"] < (2 if r["use"] in ("passage", "landing") else 3) and r["use"] != "stair" \
                    and r["floor"] > 4:
                note("narrow", mid, f"{r['use']} only {r['least_width']} wide across most of it")
            if r["use"] == "chamber" and r["windows"] == 0:
                note("dark", mid, "chamber with no window")
            if r["clear_height"] < 3:
                note("low", mid, f"{r['use']} with {r['clear_height']} of clear height")
    for n_, p_ in unreached:
        note("unreached", p_, f"{n_} no villager can walk to")
    # Walk: each bed's route over the storeys it crosses, a storey's worth being the root of its floor (a bed on the
    # third floor may walk three storeys' worth); the mean over the beds.
    per = math.sqrt(floor_total / max(1, len(storeys)))
    walks = [len(path_) / (per * (1 + level_of.get(path_[-1][1], 0))) for n_, _, path_ in routes if n_.endswith("_bed")]
    walk = round(sum(walks) / len(walks), 2) if walks else None
    report = {
        "file": Path(file).name, "size": [sx, sy, sz], "storeys": len(storeys), "floor": floor_total,
        "rooms": sum(len(s["rooms"]) for s in storeys), "beds": len(beds), "made_beds": made,
        "unreached": [f"{n} at {list(p)}" for n, p in unreached],
        # Circulation over the storeys of two rooms or more: a one-room storey's routes are its use, not corridors
        # (thread 0008); none such, no share to judge.
        "circulation_share": (round(sum(s["circulation"] for s in storeys if len(s["rooms"]) > 1)
                                    / sum(s["floor"] for s in storeys if len(s["rooms"]) > 1), 2)
                              if any(len(s["rooms"]) > 1 for s in storeys) else None),
        "idle_share": round(sum(s["idle_share"] * s["floor"] for s in storeys) / floor_total, 2),
        # The walk to a bed for the size of a storey (thread 0008: over the whole floor, a small two-storey house paid
        # for its stair with too little floor to divide by).
        "walk": walk,
        "through_rooms": through,
        "notes": notes,
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


# The legend: one table, drawn into every plan (John: "a cheap to include and edit legend"). Each key is what the
# drawing paints; edit a colour or a label here and every plan follows.
LEGEND = [
    ("wall", (60, 56, 52), "wall"),
    ("window", (120, 200, 230), "window"),
    ("door", (130, 80, 40), "door"),
    ("entrance", (40, 160, 70), "street door"),
    ("hall", (236, 214, 170), "hall"),
    ("chamber", (196, 214, 238), "chamber"),
    ("work", (238, 200, 160), "work room"),
    ("store", (214, 206, 190), "store"),
    ("passage", (232, 228, 214), "passage, landing"),
    ("room", (226, 226, 210), "room, no use"),
    ("stair", (150, 150, 150), "stair steps"),
    ("bed", (70, 110, 200), "bed"),
    ("job", (230, 140, 50), "workstation"),
    ("furniture", (150, 110, 70), "furniture"),
    ("route", (210, 70, 60), "route from the street door"),
    ("note", (220, 0, 160), "annotation (numbered)"),
]
COL = {k: c for k, c, _ in LEGEND}
COLOURS = {k: COL[k] for k in ("hall", "chamber", "work", "store", "stair", "passage", "room")} | {"landing": COL["passage"]}

# A 3 x 5 pixel font for the legend and the annotations' numbers (no dependencies): each glyph its five rows.
FONT = {
    'A': ".#.#.#####.##.#", 'B': "##.#.###.#.###.", 'C': ".###..#..#...##", 'D': "##.#.##.##.###.",
    'E': "####..##.#..###", 'F': "####..##.#..#..", 'G': ".###..#.##.#.##", 'H': "#.##.#####.##.#",
    'I': "###.#..#..#.###", 'J': "..#..#..##.#.#.", 'K': "#.##.###.#.##.#", 'L': "#..#..#..#..###",
    'M': "#.########.##.#", 'N': "##.#.##.##.##.#", 'O': ".#.#.##.##.#.#.", 'P': "##.#.###.#..#..",
    'Q': ".#.#.##.###..##", 'R': "##.#.###.#.##.#", 'S': ".###...#...###.", 'T': "###.#..#..#..#.",
    'U': "#.##.##.##.####", 'V': "#.##.##.##.#.#.", 'W': "#.##.########.#", 'X': "#.##.#.#.#.##.#",
    'Y': "#.##.#.#..#..#.", 'Z': "###..#.#.#..###", '0': "####.##.##.####", '1': ".#.##..#..#.###",
    '2': "##...#.#.#..###", '3': "##...#.#...###.", '4': "#.##.####..#..#", '5': "####..##...###.",
    '6': ".###..####.####", '7': "###..#.#..#..#.", '8': "####.#####.####", '9': "####.####..###.",
    ' ': "...............", '-': "......###......", ':': "....#.....#....", '.': ".............#.",
    ',': "..........#.#..", '(': ".#.#..#..#...#.", ')': ".#...#..#..#.#.", '/': "..#..#.#.#..#..",
    '%': "#.#..#.#.#..#.#", '#': "#.#####.#####.#", '+': "....#.###.#....", "'": ".#..#..........",
}


def text(img, x, y, words, col, px=2):
    """Writes words into an image (rows of pixel tuples) at (x, y), 3 x 5 glyphs scaled by px."""
    for ch in words.upper():
        g = FONT.get(ch, FONT[" "])
        for r in range(5):
            for c in range(3):
                if r * 3 + c < len(g) and g[r * 3 + c] == "#":
                    for a in range(px):
                        for b in range(px):
                            yy, xx = y + r * px + a, x + c * px + b
                            if 0 <= yy < len(img) and 0 <= xx < len(img[0]):
                                img[yy][xx] = col
        x += 4 * px



def draw(out, sizes, blocks, storeys, circulation, entrances, interior=frozenset(), notes=(), scale=14):
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
                           (230, 140, 50) if job(n) else
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
        for nt in notes:
            if nt["storey"] == i + 1:
                x, z = nt["at"][0], nt["at"][2]
                fill(x, z, COL["note"], inset=1)
                label = str(nt["n"])
                text(img, x * scale + (scale - 8 * len(label) + 2) // 2, z * scale + (scale - 10) // 2, label,
                     (255, 255, 255))
        text(img, 2, 2, f"{i + 1}", (0, 0, 0))
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
    # The legend under the storeys: a swatch and its label each, from LEGEND.
    width = max(w, 4 * 2 * 24 * 2)
    rows = [r + bytes((255, 255, 255)) * (width - w) for r in rows]
    legend = [[(255, 255, 255)] * width for _ in range(14 * ((len(LEGEND) + 1) // 2) + 8)]
    for k, (_, col, label) in enumerate(LEGEND):
        x0, y0 = gap + (k % 2) * (width // 2), 4 + (k // 2) * 14
        for a in range(10):
            for b in range(10):
                legend[y0 + b][x0 + a] = col
        text(legend, x0 + 14, y0 + 1, label, (40, 40, 40))
    rows += [bytes(v for px in row for v in px) for row in legend]
    w, h = width, len(rows)
    for old in out.glob("*.png"):
        old.unlink()
    f = out / "plan.png"
    f.write_bytes(png(w, h, rows))
    return [f.name]


# The thresholds (docs/assets/interiors.md, v0.1): `--check` holds a house to them. A shared quality.json (thread
# 0007) overrides them where it has an "interior" section.
THRESHOLDS = {"made_beds": "all", "unreached": 0, "through_rooms": 0, "circulation_share": [0.15, 0.45],
              "idle_share": [0, 0.45], "walk": [0, 2.0], "floor_per_bed": [4, 8.5]}


def thresholds():
    """The bands from the shared quality.json's "interior" section (beside this file in the shared tree, or our
    docs/assets/quality.json), in its own words; the built-in ones where it has none."""
    for q in (Path(__file__).with_name("quality.json"), Path(__file__).resolve().parents[2] / "docs/assets/quality.json"):
        if q.exists():
            i = json.loads(q.read_text()).get("interior", {})
            t = dict(THRESHOLDS)
            if "circulation" in i:
                t["circulation_share"] = i["circulation"]
            if "idle_max" in i:
                t["idle_share"] = [0, i["idle_max"]]
            if "walk_max" in i:
                t["walk"] = [0, i["walk_max"]]
            for k in ("floor_per_bed", "made_beds", "through_rooms", "multi_storey_min_width", "circulation_by_storeys"):
                if k in i:
                    t[k] = i[k]
            return t
    return THRESHOLDS


def failures(r, t):
    """What a report misses of the thresholds, in words."""
    out = []
    if t.get("made_beds") == "all" and r["made_beds"] < r["beds"]:
        out.append(f"{r['beds'] - r['made_beds']} of {r['beds']} beds not made")
    if len(r["unreached"]) > t.get("unreached", 0):
        out.append(f"unreached: {r['unreached']}")
    if r["through_rooms"] > t.get("through_rooms", 0):
        out.append(f"{r['through_rooms']} routes through chambers")
    narrowest = t.get("multi_storey_min_width")
    if narrowest and r["storeys"] > 1 and r["size"][0] - 2 < narrowest:
        out.append(f"a {r['size'][0] - 2}-wide body of {r['storeys']} storeys: under {narrowest} wide is one storey")
    for k in ("circulation_share", "idle_share", "walk", "floor_per_bed"):
        lo, hi = t[k]
        if k == "circulation_share" and str(r["storeys"]) in t.get("circulation_by_storeys", {}):
            lo, hi = t["circulation_by_storeys"][str(r["storeys"])]
        if r[k] is not None and not lo <= r[k] <= hi:
            out.append(f"{k} {r[k]} outside {lo} to {hi}")
    return out


def main():
    args = [a for i, a in enumerate(sys.argv[1:], 1) if not a.startswith("--") and sys.argv[i - 1] != "--out"]
    out = Path(sys.argv[sys.argv.index("--out") + 1]) if "--out" in sys.argv else None
    reports = []
    for path in args:
        report, sizes, blocks, storeys, interior, circulation, entrances, beds = analyse(path)
        if out:
            d = out / Path(path).stem
            d.mkdir(parents=True, exist_ok=True)
            report["plans"] = draw(d, sizes, blocks, storeys, circulation, entrances, interior, report["notes"])
            # The annotations as text beside the plan, to read and to search.
            (d / "notes.md").write_text(f"# {report['file']}: floor plan notes\n\n" + "".join(
                f"{n['n']}. storey {n['storey']}, at {n['at']}, {n['kind']}: {n['text']}\n" for n in report["notes"]))
        reports.append(report)
        if "--json" not in sys.argv:
            print(f"{report['file']}: {report['storeys']} storeys, floor {report['floor']}, {report['rooms']} rooms; "
                  f"beds {report['made_beds']}/{report['beds']} made; circulation {report['circulation_share']}, "
                  f"idle {report['idle_share']}, walk {report['walk']}, through rooms {report['through_rooms']}, "
                  f"floor a bed {report['floor_per_bed']}"
                  + (f"; UNREACHED {report['unreached']}" if report["unreached"] else ""))
            for n in report["notes"]:
                print(f"  note {n['n']} (storey {n['storey']}, {n['at']}, {n['kind']}): {n['text']}")
            for si, s in enumerate(report["per_storey"]):
                print(f"  storey {si + 1} (feet y {s['feet_y']}): floor {s['floor']}, circulation {s['circulation_share']},"
                      f" idle {s['idle_share']}")
                for r in s["rooms"]:
                    print(f"    {r['use']:8} floor {r['floor']:3}  {r['bbox'][0]}x{r['bbox'][1]}  least width "
                          f"{r['least_width']}  clear {r['clear_height']} ({r['low_cells']} low)  windows {r['windows']}  "
                          f"{', '.join(r['contents'])}")
    if "--json" in sys.argv:
        print(json.dumps(reports, indent=1))
    if "--check" in sys.argv:
        t, bad = thresholds(), 0
        for r in reports:
            miss = failures(r, t)
            bad += bool(miss)
            print(("FAIL  " if miss else "PASS  ") + r["file"] + ("".join(f"\n  {m}" for m in miss)))
        sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
