#!/usr/bin/env python3
"""A town's plan as GIS data for the asset studio (`docs/assets/README.md`, "GIS export"; code item C13).

    python3 dev/experiments/gis_export.py WORLD TRACE.db SERVER.log TOWN OUT_DIR [--region]

Reads a stopped server's world (its region files and saved data: the town plans, field maps, the planned roads), the
run's trace (`enterprise-sim --trace`) and the server's log (the bells), and writes into OUT_DIR (by convention
`assets-src/gis/<world>/<town-id>-<slug>/`):

- `town.json`: the town's state in the sim on the trace's last day.
- `plan.geojson`: its forum, bell, walls and their strips, gates, avenues, lanes, ward cells, home frontages, rings,
  farm tracks, holdings, fields, pastures, caravan roads, bridges, beds and workstations, and the hexes round it.
  Coordinates are `[x, -z]` in blocks (north up); `properties.mc` keeps exact block coordinates where they matter.
- `terrain/`: over the outer wall, the pastures' ring and 64 blocks round: `height.png` (16-bit, the ground's top y
  under trees + 64), `surface.png` (8-bit index of the top block, `surface.json`), `water.png` and `trees.png`
  (masks), `biome.png` (`biome.json`), each with a `.pgw` world file in the same `[x, -z]` frame.
- `map.png`: the surveyor's colours of the same area, hill-shaded.
- `region/` (with `--region`): the world's region files that cover the area.

Pure Python, as `world_render.py` (whose readers it uses). Stop the server first, so the world is saved.
"""
import json
import math
import shutil
import sqlite3
import struct
import sys
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from world_render import (MIN_Y, SIZE, Chunk, centre, chunks_in, colour, hex_at, png, saved,  # noqa: E402
                          unpacked)

# The plan's numbers, as the mod has them (`TownPlans`, `Forums`, `FieldMaps`, `Growth`).
STRIP, GATE, WARD, LANE, FORUM_REACH, HOME = 3, 3, 18, 2, 6, 5
GRAIN_DEPTH, PASTURES_BEYOND, PASTURE_DEPTH, FIELD = 40, 3 + 1 + 20, 56, 9
AROUND = 64
JOB_SITES = {"blast_furnace": "armorer", "smoker": "butcher", "cartography_table": "cartographer",
             "brewing_stand": "cleric", "composter": "farmer", "barrel": "fisherman", "fletching_table": "fletcher",
             "cauldron": "leatherworker", "lectern": "librarian", "stonecutter": "mason", "loom": "shepherd",
             "smithing_table": "toolsmith", "grindstone": "weaponsmith"}


def gj(x, z):
    """A block place as GeoJSON `[x, -z]`."""
    return [round(x, 2), round(-z, 2)]


def rect(x0, z0, x1, z1):
    """A rectangle's ring of block corners (inclusive block bounds), as GeoJSON."""
    return [[gj(x0, z0), gj(x1 + 1, z0), gj(x1 + 1, z1 + 1), gj(x0, z1 + 1), gj(x0, z0)]]


def feature(kind, geometry, **props):
    return {"type": "Feature", "geometry": geometry, "properties": {"kind": kind, **props}}


def polygon(rings):
    return {"type": "Polygon", "coordinates": rings}


def line(points):
    return {"type": "LineString", "coordinates": [gj(x, z) for x, z in points]}


def point(x, z):
    return {"type": "Point", "coordinates": gj(x, z)}


def grow(r, by):
    return [r[0] - by, r[1] - by, r[2] + by, r[3] + by]


def lane_lines(bell, inner, north_south):
    """`TownPlans.laneLines`: every WARD from the bell's line, inside the inner wall off its strip."""
    w = grow(inner, -(STRIP + 1))
    at = bell[0] if north_south else bell[2]
    lo, hi = (w[0], w[2]) if north_south else (w[1], w[3])
    out = []
    a = at - WARD
    while a - LANE >= lo:
        out.append(a)
        a -= WARD
    a = at + WARD
    while a + LANE <= hi:
        out.append(a)
        a += WARD
    return sorted(out)


def on_street(bell, inner, x, z, lanes_ns, lanes_ew):
    if abs(x - bell[0]) <= GATE // 2 + 1 or abs(z - bell[2]) <= GATE // 2 + 1:
        return True
    return any(lx - 2 <= x <= lx + 1 for lx in lanes_ns) or any(lz - 2 <= z <= lz + 1 for lz in lanes_ew)


def frontages(bell, inner, lanes_ns, lanes_ew):
    """`TownPlans.frontages`: a home's corner and door each side of every lane and avenue, off every street."""
    w = grow(inner, -(STRIP + 1))
    bx, bz = bell[0], bell[2]
    out = []
    ns = [(bx, 1, 1)] + [(x, 1, 0) for x in lanes_ns]
    ew = [(bz, 1, 1)] + [(z, 1, 0) for z in lanes_ew]
    for s, left, right in ns:
        for z in range(w[1], w[3] - HOME + 2, HOME + 1):
            out.append((s - left - 2 - HOME + 1, z, "east"))
            out.append((s + right + 2, z, "west"))
    for s, left, right in ew:
        for x in range(w[0], w[2] - HOME + 2, HOME + 1):
            out.append((x, s - left - 2 - HOME + 1, "south"))
            out.append((x, s + right + 2, "north"))
    keep = []
    f0 = FORUM_REACH + 1
    for x, z, door in out:
        if x < w[0] or z < w[1] or x + HOME - 1 > w[2] or z + HOME - 1 > w[3]:
            continue
        if any(on_street(bell, inner, x + a, z + b, lanes_ns, lanes_ew) for a in range(HOME) for b in range(HOME)):
            continue
        if x <= bx + f0 and x + HOME - 1 >= bx - f0 and z <= bz + f0 and z + HOME - 1 >= bz - f0:
            continue
        keep.append((x, z, door))
    return keep


def from_edge(r, x, z):
    inside = r[0] <= x <= r[2] and r[1] <= z <= r[3]
    dx = r[0] - x if x < r[0] else x - r[2] if x > r[2] else min(x - r[0], r[2] - x)
    dz = r[1] - z if z < r[1] else z - r[3] if z > r[3] else min(z - r[1], r[3] - z)
    return inside, (min(dx, dz) if inside else max(dx if x < r[0] or x > r[2] else 0, dz if z < r[1] or z > r[3] else 0))


def in_ring(inner, outer, ring, x, z):
    """`FieldMaps.in`."""
    (ii, ie), (oi, oe) = from_edge(inner, x, z), from_edge(outer, x, z)
    if ring == "GARDEN":
        return not ii and ie > STRIP + 1 and oi and oe > STRIP + 1
    return not oi and STRIP + 1 < oe <= STRIP + 1 + GRAIN_DEPTH


def hull(points):
    """The convex hull of points, as a closed ring (Andrew's monotone chain)."""
    pts = sorted(set(points))
    if len(pts) < 3:
        return None

    def cross(o, a, b):
        return (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    lower, upper = [], []
    for p in pts:
        while len(lower) >= 2 and cross(lower[-2], lower[-1], p) <= 0:
            lower.pop()
        lower.append(p)
    for p in reversed(pts):
        while len(upper) >= 2 and cross(upper[-2], upper[-1], p) <= 0:
            upper.pop()
        upper.append(p)
    ring = lower[:-1] + upper[:-1]
    return [gj(x, z) for x, z in ring + ring[:1]]


def unpack_pos(v):
    """`BlockPos.asLong`'s x and z."""
    v &= (1 << 64) - 1
    x = v >> 38
    z = (v << 26 & ((1 << 64) - 1)) >> 38
    if x >= 1 << 25:
        x -= 1 << 26
    if z >= 1 << 25:
        z -= 1 << 26
    return x, z


def png_grey(width, height, rows, bits):
    """A greyscale PNG of rows of values, 8 or 16 bits."""
    raw = b"".join(b"\x00" + (struct.pack(f">{width}H", *r) if bits == 16 else bytes(r)) for r in rows)

    def chunk(tag, data):
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xffffffff)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, bits, 0, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))


def world_file(x0, z0):
    """A `.pgw` for a raster whose top-left pixel is block (x0, z0), in the `[x, -z]` frame."""
    return "\n".join(map(str, [1.0, 0.0, 0.0, -1.0, x0 + 0.5, -z0 - 0.5])) + "\n"


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    if len(args) != 5:
        sys.exit(__doc__)
    world, trace, log, town, out = Path(args[0]), args[1], Path(args[2]), int(args[3]), Path(args[4])
    out.mkdir(parents=True, exist_ok=True)
    (out / "terrain").mkdir(exist_ok=True)

    bells = {}
    for ln in log.read_text(errors="replace").splitlines():
        if "lays its forum round its bell at" in ln:
            tail = ln.partition("Town ")[2]
            t = int(tail.split()[0])
            bells[t] = tuple(int(v.strip()) for v in tail.split(" at ")[1].split("(")[0].split(","))
    if town not in bells:
        sys.exit(f"no bell for town {town} in {log}")
    bell = bells[town]
    plans = {int(k): v for k, v in saved(world / "data" / "enterprise_town_plans.dat").get("towns", {}).items()}
    if town not in plans:
        sys.exit(f"town {town} has no plan in {world}")
    plan = plans[town]
    inner, outer = list(plan["inner"]), list(plan["outer"])
    fmap = saved(world / "data" / "enterprise_field_maps.dat").get("towns", {}).get(str(town), {})

    # --- the town in the sim -----------------------------------------------------------------------------------------
    db = sqlite3.connect(trace)
    day = db.execute("select max(day) from towns").fetchone()[0]
    names = dict(db.execute("select distinct town, name from towns"))
    row = db.execute("select name, villagers, beds, mood, money, target, comforts, land from towns where day=? and town=?",
                     (day, town)).fetchone()
    name = row[0] if row else names.get(town, f"Town {town}")
    state = {
        "id": town, "name": name, "kingdom": None, "tier": None, "day": day,
        "bell": {"x": bell[0], "y": bell[1], "z": bell[2]},
        "villagers": row[1] if row else None, "beds": row[2] if row else None,
        "mood": round(row[3], 3) if row else None, "mood_target": round(row[5], 3) if row else None,
        "treasury_emeralds": round(row[4], 1) if row else None, "comforts_met": row[6] if row else None,
        "hands": [{"trade": t, "level": lv, "away": bool(a)} for t, lv, a in db.execute(
            "select trade, level, away from hands where day=? and town=? order by hand", (day, town))],
        "comforts": {k: m for k, m in db.execute("select kind, met from comforts where day=? and town=?", (day, town))},
        "herds": [{"animal": a, "count": round(c, 1), "pastures": p} for a, c, p in db.execute(
            "select animal, count, pastures from herds where day=? and town=?", (day, town))],
        "farms": [{"crop": c, "plots": p} for c, p in db.execute(
            "select crop, plots from farms where day=? and town=? and plots>0", (day, town))],
        "works": [{"what": w, "progress": round(p, 2), "done": bool(d)} for w, p, d in db.execute(
            "select what, progress, done from works where day=? and town=?", (day, town))],
        "stock": {g: round(c, 1) for g, c in db.execute(
            "select good, count from stock where day=? and town=? and count>0", (day, town))},
        "routes": [{"to": names.get(b if a == town else a, b if a == town else a), "outbound": a == town, "carrier": c,
                    "trips": tr, "blocks": bl, "sold": round(s or 0, 1), "bought": round(bo or 0, 1)}
                   for a, b, c, tr, bl, s, bo in db.execute(
                       "select from_town, to_town, carrier, trips, blocks, sold, bought from routes "
                       "where day=? and (from_town=? or to_town=?)", (day, town, town))],
        "notes": "kingdom and tier aren't in the sim yet (README C2); the studio proposes them for the pilot",
    }

    # --- the area, and its blocks ------------------------------------------------------------------------------------
    reach = STRIP + 1 + PASTURES_BEYOND + PASTURE_DEPTH + AROUND
    x0, z0, x1, z1 = outer[0] - reach, outer[1] - reach, outer[2] + reach, outer[3] + reach
    w, h = x1 - x0 + 1, z1 - z0 + 1
    height = [[0] * w for _ in range(h)]
    surface = [[0] * w for _ in range(h)]
    water = [[0] * w for _ in range(h)]
    trees = [[0] * w for _ in range(h)]
    biome = [[0] * w for _ in range(h)]
    rgb = [[(24, 28, 34)] * w for _ in range(h)]
    kinds = [[None] * w for _ in range(h)]
    blocks, biomes = {}, {}
    points = []  # beds, job sites, bells, desks
    regions = set()
    for rf in sorted((world / "region").glob("r.*.mca")):
        _, rx, rz, _ = rf.name.split(".")
        rx, rz = int(rx), int(rz)
        if (rx + 1) * 512 <= x0 or rx * 512 > x1 or (rz + 1) * 512 <= z0 or rz * 512 > z1:
            continue
        regions.add(rf)
        for nbt in chunks_in(rf):
            if nbt.get("Status") not in ("minecraft:full", "full"):
                continue
            c = Chunk(nbt)
            if c.top is None or (c.cx + 1) * 16 <= x0 or c.cx * 16 > x1 or (c.cz + 1) * 16 <= z0 or c.cz * 16 > z1:
                continue
            sec_biomes = {}
            for s in nbt.get("sections", []):
                b = s.get("biomes")
                if b and "palette" in b:
                    pal = b["palette"]
                    data = unpacked(b.get("data", ()), max(1, (len(pal) - 1).bit_length()), 64) if len(pal) > 1 else [0] * 64
                    sec_biomes[s["Y"]] = (pal, data)
            for lz in range(16):
                for lx in range(16):
                    X, Z = c.cx * 16 + lx, c.cz * 16 + lz
                    if not (x0 <= X <= x1 and z0 <= Z <= z1):
                        continue
                    i = lz * 16 + lx
                    top = c.top[i] - 1 + MIN_Y
                    ground = c.ground[i] - 1 + MIN_Y
                    name = c.block(lx, top, lz)
                    # The ground under any trunk standing on it.
                    g, gname = ground, c.block(lx, ground, lz)
                    while gname.endswith("_log") and g > MIN_Y:
                        g -= 1
                        gname = c.block(lx, g, lz)
                    col, k = colour(name, c.block(lx, top + 1, lz))
                    zi, xi = Z - z0, X - x0
                    rgb[zi][xi], kinds[zi][xi] = col, k
                    height[zi][xi] = max(0, min(65535, g + 64))
                    surface[zi][xi] = blocks.setdefault(gname, len(blocks))
                    water[zi][xi] = 255 if k == "water" else 0
                    trees[zi][xi] = 255 if k == "tree" or g < ground else 0
                    sy = g >> 4
                    if sy in sec_biomes:
                        pal, data = sec_biomes[sy]
                        bi = data[((g & 15) >> 2) * 16 + (lz >> 2) * 4 + (lx >> 2)]
                        bname = pal[bi] if bi < len(pal) else "minecraft:plains"
                        biome[zi][xi] = biomes.setdefault(bname, len(biomes))
                    # What villagers use, in the blocks under the top (a home's roof is over its beds).
                    for y in range(max(MIN_Y, top - 14), top + 1):
                        n = c.block(lx, y, lz).split(":")[-1]
                        if n.endswith("_bed") or n in JOB_SITES or n == "bell" or n.startswith("merchant_desk") \
                                or n == "desk":
                            points.append((n, X, y, Z))
    print(f"area {w} x {h} from x {x0}, z {z0}; {len(blocks)} surface blocks, {len(biomes)} biomes", file=sys.stderr)

    # --- the plan ----------------------------------------------------------------------------------------------------
    bx, by, bz = bell
    feats = []
    feats.append(feature("bell", point(bx + 0.5, bz + 0.5), mc=[bx, by, bz]))
    r = FORUM_REACH
    feats.append(feature("forum", polygon(rect(bx - r, bz - r, bx + r, bz + r)), size=[2 * r + 1, 2 * r + 1]))
    for wall, rr in (("wall_inner", inner), ("wall_outer", outer)):
        feats.append(feature(wall, polygon(rect(*rr)), mc=rr, built=wall == "wall_inner",
                             note="the inner is the town's wall work; the outer is kept clear, to build later"))
        o, i = grow(rr, STRIP), grow(rr, -STRIP)
        feats.append(feature("wall_strip", polygon(rect(*o) + [list(reversed(rect(*i)[0]))]), wall=wall,
                             width=2 * STRIP + 1))
        for side, gx, gz, facing in (("north", bx, rr[1], "north"), ("south", bx, rr[3], "south"),
                                     ("west", rr[0], bz, "west"), ("east", rr[2], bz, "east")):
            feats.append(feature("gate", point(gx + 0.5, gz + 0.5), wall=wall, width=GATE, facing=facing,
                                 mc=[gx, gz]))
    for dx, dz, end in ((0, -1, outer[1]), (0, 1, outer[3]), (-1, 0, outer[0]), (1, 0, outer[2])):
        a = (bx + 0.5 + dx * (FORUM_REACH + 1), bz + 0.5 + dz * (FORUM_REACH + 1))
        b = (bx + 0.5, end + 0.5) if dx == 0 else (end + 0.5, bz + 0.5)
        feats.append(feature("avenue", line([a, b]), width=GATE))
    lanes_ns, lanes_ew = (lane_lines(bell, inner, True), lane_lines(bell, inner, False)) if plan.get("lanes") else ([], [])
    wi = grow(inner, -(STRIP + 1))
    for x in lanes_ns:
        feats.append(feature("lane", line([(x + 0.5, wi[1] + 0.5), (x + 0.5, wi[3] + 0.5)]), width=LANE))
    for z in lanes_ew:
        feats.append(feature("lane", line([(wi[0] + 0.5, z + 0.5), (wi[2] + 0.5, z + 0.5)]), width=LANE))
    # Ward cells: the ground between the streets (avenues and lanes), off them.
    xs = sorted([wi[0] - 2] + [x for x in lanes_ns] + [bx] + [wi[2] + 3])
    zs = sorted([wi[1] - 2] + [z for z in lanes_ew] + [bz] + [wi[3] + 3])
    for a, b in zip(xs, xs[1:]):
        for c_, d in zip(zs, zs[1:]):
            cx0, cx1 = a + 2, b - 2
            cz0, cz1 = c_ + 2, d - 2
            if cx1 - cx0 >= 4 and cz1 - cz0 >= 4:
                feats.append(feature("ward_cell", polygon(rect(cx0, cz0, cx1, cz1)), size=[cx1 - cx0 + 1, cz1 - cz0 + 1],
                                     mc=[cx0, cz0, cx1, cz1]))
    for fx, fz, door in frontages(bell, inner, lanes_ns, lanes_ew):
        feats.append(feature("frontage", polygon(rect(fx, fz, fx + HOME - 1, fz + HOME - 1)), door=door, size=[HOME, HOME],
                             mc=[fx, fz]))
    # Rings: the gardens between the walls, the grain past the outer, the pastures beyond.
    gi, go = grow(inner, STRIP + 1), grow(outer, -(STRIP + 1))
    feats.append(feature("ring", polygon(rect(*go) + [list(reversed(rect(*gi)[0]))]), ring="gardens"))
    ga, gb = grow(outer, STRIP + 1), grow(outer, STRIP + 1 + GRAIN_DEPTH)
    feats.append(feature("ring", polygon(rect(*gb) + [list(reversed(rect(*ga)[0]))]), ring="grain"))
    pa, pb = grow(outer, PASTURES_BEYOND), grow(outer, PASTURES_BEYOND + PASTURE_DEPTH)
    feats.append(feature("ring", polygon(rect(*pb) + [list(reversed(rect(*pa)[0]))]), ring="pastures"))

    # The field map: its farm tracks, its holdings (the Voronoi parcels, as the ground nearer each seed than any
    # other's, within 40 and in its ring) and its fields.
    tracks = fmap.get("tracks", [])
    for t in tracks:
        ln = t.get("line", [])
        pts = [(ln[i] / 4, ln[i + 1] / 4) for i in range(0, len(ln) - 1, 2)]
        if len(pts) >= 2:
            feats.append(feature("farm_track", line(pts), ring=t.get("ring", "").lower(), width=2,
                                 trade_path=t.get("path")))
    ps = list(fmap.get("parcels", []))
    seeds = [(ps[i], ps[i + 1], ps[i + 2], ps[i + 3] == 1) for i in range(0, len(ps) - 4, 5)]
    rings = [t.get("ring") for t in tracks]
    cells = {k: [] for k in range(len(seeds))}
    for zz in range(z0, z1 + 1, 2):
        for xx in range(x0, x1 + 1, 2):
            best, d = -1, 40 ** 2
            for k, (sx, sz, _, _) in enumerate(seeds):
                dd = (xx - sx) ** 2 + (zz - sz) ** 2
                if dd < d:
                    best, d = k, dd
            if best >= 0 and seeds[best][2] < len(rings) and in_ring(inner, outer, rings[seeds[best][2]], xx, zz):
                cells[best].append((xx, zz))
    for k, (sx, sz, track, ns) in enumerate(seeds):
        ring = hull(cells[k])
        if ring:
            feats.append(feature("holding", polygon([ring]), seed=gj(sx + 0.5, sz + 0.5), track=track,
                                 strips="north-south" if ns else "east-west",
                                 ring=(rings[track] if track < len(rings) else "").lower(),
                                 note="hull of the ground nearer this seed than any other's; the mod also keeps it "
                                      "on its seed's side of every road and track"))
    for v in fmap.get("taken", []):
        fx, fz = unpack_pos(v)
        crop = None
        for zz in range(fz, fz + FIELD):
            for xx in range(fx, fx + FIELD):
                if 0 <= zz - z0 < h and 0 <= xx - x0 < w and kinds[zz - z0][xx - x0] == "field":
                    crop = crop or "farmed"
        feats.append(feature("field", polygon(rect(fx, fz, fx + FIELD - 1, fz + FIELD - 1)), taken=True, crop=crop,
                             mc=[fx, fz], size=[FIELD, FIELD]))
    # Pastures: birch-fenced rectangles, from the blocks.
    seen = set()
    for zz in range(h):
        for xx in range(w):
            if kinds[zz][xx] != "fence" or (xx, zz) in seen:
                continue
            stack, comp = [(xx, zz)], []
            seen.add((xx, zz))
            while stack:
                a, b = stack.pop()
                comp.append((a, b))
                for da, dz_ in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                    n = (a + da, b + dz_)
                    if 0 <= n[0] < w and 0 <= n[1] < h and n not in seen and kinds[n[1]][n[0]] == "fence":
                        seen.add(n)
                        stack.append(n)
            if len(comp) >= 16:
                ax0, az0 = min(a for a, _ in comp) + x0, min(b for _, b in comp) + z0
                ax1, az1 = max(a for a, _ in comp) + x0, max(b for _, b in comp) + z0
                feats.append(feature("pasture", polygon(rect(ax0, az0, ax1, az1)), size=[ax1 - ax0 + 1, az1 - az0 + 1],
                                     grass=max(0, (ax1 - ax0 - 1) * (az1 - az0 - 1)), mc=[ax0, az0, ax1, az1]))
    # Caravan roads planned over the land (the world's enterprise-roads.json), those touching the area.
    bell_hex = {t: hex_at(b[0] + 0.5, b[2] + 0.5) for t, b in bells.items()}
    routes = {}
    for a, b, carrier, trips in db.execute(
            "select from_town, to_town, carrier, trips from routes where day=?", (day,)):
        routes[frozenset((a, b))] = (carrier, (routes.get(frozenset((a, b)), (None, 0))[1] or 0) + (trips or 0))
    roads = world / "enterprise-roads.json"
    for road in (json.loads(roads.read_text()) if roads.exists() else []):
        pts = road["line"]
        if not any(x0 <= x <= x1 and z0 <= z <= z1 for x, z in pts):
            continue
        ends = [t for t, hx in bell_hex.items() if hx in (tuple(road["path"][0]), tuple(road["path"][-1]))]
        carrier, trips = routes.get(frozenset(ends), (None, None)) if len(ends) == 2 else (None, None)
        feats.append(feature("road", line(pts), towns=[names.get(t, t) for t in ends], carrier=carrier, trips=trips,
                             width=3, note="planned over the land: a wagon's steepest 1 in 12, a llama's 1 in 3"))
    # Bridges: plank decks over water.
    for zz in range(h):
        for xx in range(w):
            if kinds[zz][xx] == "bridge" and (xx, zz) not in seen:
                seen.add((xx, zz))
                feats.append(feature("bridge", point(xx + x0 + 0.5, zz + z0 + 0.5), mc=[xx + x0, zz + z0]))
    for n, X, Y, Z in points:
        kind = "bed" if n.endswith("_bed") else "bell_block" if n == "bell" else "desk" if "desk" in n else "workstation"
        feats.append(feature(kind, point(X + 0.5, Z + 0.5), block=n, trade=JOB_SITES.get(n), mc=[X, Y, Z]))
    # The hexes round the town, for context.
    hq, hr = hex_at(bx + 0.5, bz + 0.5)
    for q in range(hq - 4, hq + 5):
        for rr_ in range(hr - 4, hr + 5):
            cx, cz = centre(q, rr_)
            if not (x0 - SIZE <= cx <= x1 + SIZE and z0 - SIZE <= cz <= z1 + SIZE):
                continue
            ring = [gj(cx + SIZE * math.cos(math.radians(60 * k - 30)), cz + SIZE * math.sin(math.radians(60 * k - 30)))
                    for k in range(7)]
            feats.append(feature("hex", polygon([ring]), q=q, r=rr_, home=(q, rr_) == (hq, hr)))

    collection = {"type": "FeatureCollection", "name": f"{town}-{name}",
                  "crs_note": "Minecraft blocks; coordinates [x, -z] so north is up; properties.mc in block x, z (y)",
                  "features": feats}
    (out / "plan.geojson").write_text(json.dumps(collection, indent=1))
    (out / "town.json").write_text(json.dumps(state, indent=1))

    # --- rasters -----------------------------------------------------------------------------------------------------
    t = out / "terrain"
    (t / "height.png").write_bytes(png_grey(w, h, height, 16))
    (t / "surface.png").write_bytes(png_grey(w, h, surface, 8))
    (t / "surface.json").write_text(json.dumps({str(i): n for n, i in blocks.items()}, indent=1))
    (t / "water.png").write_bytes(png_grey(w, h, water, 8))
    (t / "trees.png").write_bytes(png_grey(w, h, trees, 8))
    (t / "biome.png").write_bytes(png_grey(w, h, biome, 8))
    (t / "biome.json").write_text(json.dumps({str(i): n for n, i in biomes.items()}, indent=1))
    for n in ("height", "surface", "water", "trees", "biome"):
        (t / f"{n}.pgw").write_text(world_file(x0, z0))
    (t / "README.txt").write_text(
        "height.png: 16-bit, the ground's top y (under trees) + 64. surface.png: 8-bit index into surface.json.\n"
        "water.png, trees.png: masks. biome.png: 8-bit index into biome.json. Every .pgw: one pixel a block,\n"
        f"x = {x0} + column, -z = {-z0} - row (top-left pixel is block x {x0}, z {z0}).\n")
    hs = [v for row in height for v in row if v]
    lo, hi = (min(hs), max(hs)) if hs else (0, 1)
    rows = []
    for zz in range(h):
        rowb = bytearray()
        for xx in range(w):
            rr, gg, b = rgb[zz][xx]
            y = height[zz][xx]
            if y and kinds[zz][xx] != "water":
                nw = height[zz - 1][xx - 1] if zz > 0 and xx > 0 and height[zz - 1][xx - 1] else y
                f = max(0.62, min(1.32, 1 + 0.11 * (y - nw))) * (0.9 + 0.2 * (y - lo) / max(1, hi - lo))
                rr, gg, b = min(255, int(rr * f)), min(255, int(gg * f)), min(255, int(b * f))
            rowb += bytes((rr, gg, b))
        rows.append(rowb)
    (out / "map.png").write_bytes(png(w, h, rows))
    (out / "map.pgw").write_text(world_file(x0, z0))

    if "--region" in sys.argv:
        (out / "region").mkdir(exist_ok=True)
        for rf in sorted(regions):
            shutil.copy2(rf, out / "region" / rf.name)
    counts = {}
    for f in feats:
        counts[f["properties"]["kind"]] = counts.get(f["properties"]["kind"], 0) + 1
    print(json.dumps(counts), file=sys.stderr)
    print(f"wrote {out}", file=sys.stderr)


if __name__ == "__main__":
    main()
