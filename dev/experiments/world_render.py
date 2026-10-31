#!/usr/bin/env python3
"""The surveyor's map of a study's world: its blocks from above, after a run (core mode).

    python3 dev/experiments/world_render.py WORLD TRACE.db SERVER.log OUT.html [--margin 96] [--video FILM.mp4] [--title "Seed 1717 Survey"]
    python3 dev/experiments/world_render.py WORLD TRACE.db SERVER.log OUT.html --points core/run/fixtures/seed-1717/run-KEY

`--points RUN`: the world as it stood at each point a survival lane kept (RUN/dayN/world, fixtures.py), one frame a
point at the same bounds, the clock showing the latest frame by its day: the towns as they grew. A town's name is shown
from its founding (its first day in the trace).

Reads the world's own region files (stop the server first, so they're saved) and draws each column's top: trees,
roads and the towns' forums, homes and other buildings, fields by crop, pastures (birch-fenced, their grass tinted),
water, and the land itself hill-shaded by its height. Over it, from the run's trace (`enterprise-sim --trace`): each
caravan route along its hex path, as wide as the trips it made, with wagons moving along it, more where it carried
more; and each town by name at its bell (the forum lines of SERVER.log). Pure Python (no numpy): a minute or two.
"""
import base64
import gzip
import html
import json
import math
import sqlite3
import struct
import sys
import zlib
from pathlib import Path

ACROSS = 80
SIZE = ACROSS / math.sqrt(3)
MIN_Y = -64


# --- NBT, as a chunk in a region file holds it ------------------------------------------------------------------------
class Reader:
    def __init__(self, data):
        self.d = data
        self.i = 0

    def take(self, n):
        b = self.d[self.i:self.i + n]
        self.i += n
        return b

    def value(self, t):
        if t == 1: return struct.unpack(">b", self.take(1))[0]
        if t == 2: return struct.unpack(">h", self.take(2))[0]
        if t == 3: return struct.unpack(">i", self.take(4))[0]
        if t == 4: return struct.unpack(">q", self.take(8))[0]
        if t == 5: return struct.unpack(">f", self.take(4))[0]
        if t == 6: return struct.unpack(">d", self.take(8))[0]
        if t == 7: n = self.value(3); return self.take(n)
        if t == 8: n = struct.unpack(">H", self.take(2))[0]; return self.take(n).decode("utf-8", "replace")
        if t == 9:
            et = self.take(1)[0]; n = self.value(3)
            return [self.value(et) for _ in range(n)]
        if t == 10:
            out = {}
            while True:
                tt = self.take(1)[0]
                if tt == 0:
                    return out
                name = self.value(8)
                out[name] = self.value(tt)
        if t == 11: n = self.value(3); return struct.unpack(">%di" % n, self.take(4 * n))
        if t == 12: n = self.value(3); return struct.unpack(">%dq" % n, self.take(8 * n))
        raise ValueError(t)

    def root(self):
        t = self.take(1)[0]
        self.value(8)
        return self.value(t)


def saved(path):
    """A world's saved data file (gzipped NBT) as its `data` compound, or {}."""
    if not path.exists():
        return {}
    with gzip.open(path, "rb") as f:
        return Reader(f.read()).root().get("data", {})


def chunks_in(region_file):
    """Each saved chunk of a region file, as its NBT."""
    data = region_file.read_bytes()
    if len(data) < 8192:
        return
    for k in range(1024):
        loc = int.from_bytes(data[4 * k:4 * k + 3], "big")
        if loc == 0:
            continue
        at = loc * 4096
        length = int.from_bytes(data[at:at + 4], "big")
        if data[at + 4] != 2:
            continue
        yield Reader(zlib.decompress(data[at + 5:at + 4 + length])).root()


def unpacked(longs, bits, count):
    """Values packed `bits` each into longs, none across two (the game's since 1.16)."""
    per = 64 // bits
    mask = (1 << bits) - 1
    out = []
    for lv in longs:
        lv &= (1 << 64) - 1
        for s in range(per):
            out.append((lv >> (s * bits)) & mask)
            if len(out) == count:
                return out
    return out + [0] * (count - len(out))


class Chunk:
    def __init__(self, nbt):
        self.cx = nbt.get("xPos")
        self.cz = nbt.get("zPos")
        hm = nbt.get("Heightmaps", {})
        self.top = unpacked(hm.get("MOTION_BLOCKING", ()), 9, 256) if hm.get("MOTION_BLOCKING") else None
        self.ground = unpacked(hm.get("MOTION_BLOCKING_NO_LEAVES", ()), 9, 256) if hm.get("MOTION_BLOCKING_NO_LEAVES") else self.top
        self.sections = {}
        for s in nbt.get("sections", []):
            bs = s.get("block_states")
            if bs and "palette" in bs:
                self.sections[s["Y"]] = bs
        self._decoded = {}

    def block(self, x, y, z):
        """The block's name at chunk-local x, z and world y."""
        sy = y >> 4
        bs = self.sections.get(sy)
        if bs is None:
            return "minecraft:air"
        pal = bs["palette"]
        if len(pal) == 1:
            return pal[0]["Name"]
        idx = self._decoded.get(sy)
        if idx is None:
            bits = max(4, (len(pal) - 1).bit_length())
            idx = unpacked(bs.get("data", ()), bits, 4096)
            self._decoded[sy] = idx
        i = ((y & 15) << 8) | (z << 4) | x
        p = idx[i]
        return pal[p]["Name"] if p < len(pal) else "minecraft:air"


# --- what a block shows as from above ---------------------------------------------------------------------------------
CROPS = {"wheat": (214, 190, 86), "carrots": (226, 136, 44), "potatoes": (176, 156, 92), "beetroots": (164, 52, 72),
         "melon_stem": (120, 170, 70), "pumpkin_stem": (190, 150, 60)}
NATURAL = {
    "grass_block": (104, 148, 70), "dirt": (134, 98, 66), "coarse_dirt": (128, 96, 66), "podzol": (108, 80, 44),
    "rooted_dirt": (134, 98, 66), "mycelium": (120, 104, 116), "moss_block": (90, 130, 56), "mud": (70, 62, 58),
    "sand": (220, 208, 160), "red_sand": (190, 104, 50), "sandstone": (214, 200, 152), "red_sandstone": (186, 102, 46),
    "gravel": (138, 132, 128), "clay": (160, 166, 178), "snow_block": (240, 242, 246), "snow": (240, 242, 246),
    "ice": (160, 190, 240), "packed_ice": (150, 180, 236), "stone": (122, 122, 122), "andesite": (130, 130, 132),
    "granite": (150, 106, 90), "diorite": (186, 186, 188), "deepslate": (80, 80, 84), "tuff": (108, 108, 98),
    "calcite": (220, 222, 216), "terracotta": (152, 94, 68), "orange_terracotta": (162, 84, 38),
    "yellow_terracotta": (186, 134, 36), "white_terracotta": (210, 178, 160), "red_terracotta": (142, 60, 46),
    "brown_terracotta": (78, 52, 36), "light_gray_terracotta": (136, 106, 98), "farmland": (112, 80, 48),
}
WATER = (54, 100, 170)
ROAD = (196, 168, 112)
FORUM = (158, 160, 166)
HOUSE = (172, 106, 66)
LEAVES = (40, 88, 42)
LOG = (92, 70, 44)
FENCE = (236, 232, 216)
RAIL = (196, 54, 60)
BRIDGE = (120, 84, 50)
WALL = (70, 62, 84)
PASTURE = (150, 192, 98)
OTHER = (146, 140, 136)
HOUSE_WORDS = ("planks", "stairs", "slab", "cobblestone", "glass", "door", "wool", "_bed", "bookshelf", "hay_block",
               "bell", "lantern", "torch", "wall", "fence", "carpet", "brick", "log", "wood", "barrel", "composter",
               "cauldron", "lectern", "smoker", "furnace", "table", "loom", "stonecutter", "grindstone", "chest",
               "smooth_stone", "polished")


def colour(name, above):
    n = name.split(":")[-1]
    if n.endswith("_leaves"):
        return LEAVES, "tree"
    if n == "water" or n == "kelp" or n == "seagrass" or n == "bubble_column":
        return WATER, "water"
    if n == "dirt_path":
        return ROAD, "road"
    if n in ("stone_bricks", "polished_andesite"):
        return FORUM, "forum"
    if n in ("birch_fence", "birch_fence_gate"):
        return FENCE, "fence"
    if n.endswith("_fence") or n.endswith("_fence_gate"):
        return RAIL, "rail"
    if n == "farmland":
        a = above.split(":")[-1]
        return CROPS.get(a, NATURAL["farmland"]), "field"
    if n in NATURAL:
        return NATURAL[n], "land"
    if n.endswith("_log") and above.split(":")[-1].endswith("_leaves"):
        return LOG, "tree"
    if any(w in n for w in HOUSE_WORDS):
        return HOUSE, "house"
    return OTHER, "land"


# --- the hexes, as the game reckons them (pointy-top, axial) ----------------------------------------------------------
def centre(q, r):
    return SIZE * math.sqrt(3) * (q + r / 2.0), SIZE * 1.5 * r


def hex_at(x, z):
    """The hex a block's place lies in (`Hex.at`: axial, cube-rounded)."""
    q = (math.sqrt(3) / 3 * x - z / 3) / SIZE
    r = (2.0 / 3 * z) / SIZE
    s = -q - r
    rq, rr, rs = round(q), round(r), round(s)
    dq, dr, ds = abs(rq - q), abs(rr - r), abs(rs - s)
    if dq > dr and dq > ds:
        rq = -rr - rs
    elif dr > ds:
        rr = -rq - rs
    return rq, rr


NEIGHBOURS = ((1, 0), (-1, 0), (0, 1), (0, -1), (1, -1), (-1, 1))


def taut(path, bells):
    """A route pulled taut as the game lays its road (`Trails.taut`): from bell to bell, each stretch as straight as it
    goes within the route's hexes and those beside them."""
    band = set(path) | {(q + a, r + b) for q, r in path for a, b in NEIGHBOURS}
    points = [list(centre(q, r)) for q, r in path]
    for end in (0, len(path) - 1):
        for bx, _, bz in bells.values():
            if hex_at(bx + 0.5, bz + 0.5) == path[end]:
                points[end] = [bx + 0.5, bz + 0.5]

    def inside(a, b):
        steps = max(1, math.ceil(math.hypot(b[0] - a[0], b[1] - a[1]) / 2))
        return all(hex_at(a[0] + (b[0] - a[0]) * k / steps, a[1] + (b[1] - a[1]) * k / steps) in band
                   for k in range(steps + 1))
    out, at = [points[0]], 0
    while at < len(points) - 1:
        nxt = next((j for j in range(len(points) - 1, at + 1, -1) if inside(points[at], points[j])), at + 1)
        out.append(points[nxt])
        at = nxt
    return out


def png(width, height, rows):
    """A PNG of RGB rows (bytes each)."""
    raw = b"".join(b"\x00" + bytes(r) for r in rows)
    def chunk(tag, data):
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xffffffff)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))


def halved(image):
    """A frame (base64 PNG, as `png` writes it: no row filters) at half its sides, a pixel for each 2 x 2."""
    data = base64.b64decode(image)
    width, height = struct.unpack(">II", data[16:24])
    at, idat = 8, b""
    while at < len(data):
        n, tag = struct.unpack(">I", data[at:at + 4])[0], data[at + 4:at + 8]
        if tag == b"IDAT":
            idat += data[at + 8:at + 8 + n]
        at += 12 + n
    raw, stride = zlib.decompress(idat), 1 + 3 * width
    rows = [raw[z * stride + 1:(z + 1) * stride] for z in range(0, height, 2)]
    rows = [bytes(b for x in range(0, width, 2) for b in row[3 * x:3 * x + 3]) for row in rows]
    return base64.b64encode(png(len(range(0, width, 2)), len(rows), rows)).decode()


# The page's frames together at most: a page is published under 16 MB.
FRAMES_BUDGET = 12 * 2 ** 20


def picture(world, x0, z0, x1, z1, bells, step=1):
    """The world's columns from above, hill-shaded, as a PNG (base64), a pixel for each `step` x `step` blocks: (image,
    counts by kind, lowest, highest)."""
    w, h = x1 - x0 + 1, z1 - z0 + 1
    kind = [[None] * w for _ in range(h)]
    rgb = [[(24, 28, 34)] * w for _ in range(h)]
    height = [[None] * w for _ in range(h)]
    counts = {}
    for rf in sorted((world / "region").glob("r.*.mca")):
        _, rx, rz, _ = rf.name.split(".")
        rx, rz = int(rx), int(rz)
        if (rx + 1) * 512 <= x0 or rx * 512 > x1 or (rz + 1) * 512 <= z0 or rz * 512 > z1:
            continue
        for nbt in chunks_in(rf):
            if nbt.get("Status") not in ("minecraft:full", "full"):
                continue
            c = Chunk(nbt)
            if c.top is None or (c.cx + 1) * 16 <= x0 or c.cx * 16 > x1 or (c.cz + 1) * 16 <= z0 or c.cz * 16 > z1:
                continue
            for lz in range(16):
                for lx in range(16):
                    X, Z = c.cx * 16 + lx, c.cz * 16 + lz
                    if not (x0 <= X <= x1 and z0 <= Z <= z1):
                        continue
                    i = lz * 16 + lx
                    y = c.top[i] - 1 + MIN_Y
                    name = c.block(lx, y, lz)
                    col, k = colour(name, c.block(lx, y + 1, lz))
                    if name.endswith("spruce_planks") and "water" in c.block(lx, y - 1, lz):
                        col, k = BRIDGE, "bridge"
                    if k == "land" and name.endswith("_log"):
                        col, k = LOG, "tree"
                    # The town wall: its ring of columns round a bell (`Works.WALL_RADIUS`).
                    if k in ("house", "tree", "land") and any(
                            38 <= math.hypot(X - bx, Z - bz) <= 42 for bx, _, bz in bells.values()) and (
                            name.endswith("_log") or name.endswith("cobblestone") or name.endswith("sandstone")):
                        col, k = WALL, "wall"
                    rgb[Z - z0][X - x0] = col
                    kind[Z - z0][X - x0] = k
                    height[Z - z0][X - x0] = c.ground[i] - 1 + MIN_Y
                    counts[k] = counts.get(k, 0) + 1
    print("columns:", counts, file=sys.stderr)

    # Pastures: the land inside a birch fence (a fence within reach every way round it).
    reach = 24
    for z in range(h):
        row = kind[z]
        for x in range(w):
            if row[x] != "land":
                continue
            ok = True
            for dx, dz in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                hit = False
                for n in range(1, reach + 1):
                    a, b = x + dx * n, z + dz * n
                    if not (0 <= a < w and 0 <= b < h):
                        break
                    if kind[b][a] == "fence":
                        hit = True
                        break
                if not hit:
                    ok = False
                    break
            if ok:
                kind[z][x] = "pasture"
                rgb[z][x] = PASTURE

    # The land's relief: lit from the north-west, and a little lighter the higher it stands.
    hs = [y for row in height for y in row if y is not None]
    lo, hi = (min(hs), max(hs)) if hs else (0, 1)
    rows = []
    for z in range(h):
        row = bytearray()
        for x in range(w):
            r, g, b = rgb[z][x]
            y = height[z][x]
            if y is not None and kind[z][x] not in ("water",):
                nw = height[z - 1][x - 1] if z > 0 and x > 0 and height[z - 1][x - 1] is not None else y
                light = max(0.62, min(1.32, 1 + 0.11 * (y - nw)))
                lift = 0.9 + 0.2 * (y - lo) / max(1, hi - lo)
                f = light * lift
                r, g, b = (min(255, int(r * f)), min(255, int(g * f)), min(255, int(b * f)))
            row += bytes((r, g, b))
        rows.append(row)
    if step > 1:
        # A pixel for each step x step blocks (its north-west one): many frames of a wide map in a page under 16 MB.
        rows = [bytes(b for x in range(0, w, step) for b in row[3 * x:3 * x + 3]) for row in rows[::step]]
    image = base64.b64encode(png(len(range(0, w, step)), len(rows), rows)).decode()
    return image, counts, lo, hi


def town_plans(world, x0, z0):
    """The towns' plans and their holdings' edges in the world as saved, on the sheet."""
    # The towns' plans (`TownPlans`): their walls' rectangles, the inner to build, the outer kept clear.
    plans = []
    for key, p in saved(world / "data" / "enterprise_town_plans.dat").get("towns", {}).items():
        i, o = p.get("inner"), p.get("outer")
        if i and o:
            plans.append({"inner": [i[0] - x0, i[1] - z0, i[2] - x0, i[3] - z0],
                          "outer": [o[0] - x0, o[1] - z0, o[2] - x0, o[3] - z0]})
    # The towns' field maps (`FieldMaps`): each holding's edge, where the ground nearer one seed meets another's (or
    # the holding's reach), on a 2-block grid.
    holdings = []
    plan_of = {int(k): v for k, v in saved(world / "data" / "enterprise_town_plans.dat").get("towns", {}).items()}

    def in_ring(plan, ring, x, z):
        """As `FieldMaps.in`: off both walls' strips, between them (gardens) or up to 40 past the outer (grain)."""
        def edge(r):
            inside = r[0] <= x <= r[2] and r[1] <= z <= r[3]
            dx = r[0] - x if x < r[0] else x - r[2] if x > r[2] else min(x - r[0], r[2] - x)
            dz = r[1] - z if z < r[1] else z - r[3] if z > r[3] else min(z - r[1], r[3] - z)
            return inside, (min(dx, dz) if inside else max(dx if x < r[0] or x > r[2] else 0,
                                                           dz if z < r[1] or z > r[3] else 0))
        (ii, ie), (oi, oe) = edge(plan["inner"]), edge(plan["outer"])
        return (not ii and ie > 4 and oi and oe > 4) if ring == "GARDEN" else (not oi and 4 < oe <= 44)

    for key, m in saved(world / "data" / "enterprise_field_maps.dat").get("towns", {}).items():
        plan = plan_of.get(int(key))
        ps = list(m.get("parcels", []))
        rings = [t.get("ring") for t in m.get("tracks", [])]
        seeds = [(ps[i], ps[i + 1], rings[ps[i + 2]]) for i in range(0, len(ps) - 4, 5) if ps[i + 2] < len(rings)]
        if not seeds or not plan:
            continue
        g = 2
        xs, zs = [x for x, _, _ in seeds], [z for _, z, _ in seeds]
        gx0, gz0 = (min(xs) - 42) // g * g, (min(zs) - 42) // g * g
        nx, nz = (max(xs) + 42 - gx0) // g + 1, (max(zs) + 42 - gz0) // g + 1
        owner = {}
        for i in range(nx):
            for j in range(nz):
                x, z = gx0 + i * g, gz0 + j * g
                best, d = -1, 40 ** 2
                for k, (sx, sz, _) in enumerate(seeds):
                    dd = (x - sx) ** 2 + (z - sz) ** 2
                    if dd < d:
                        best, d = k, dd
                owner[i, j] = best if best >= 0 and in_ring(plan, seeds[best][2], x, z) else -1
        # Only where two holdings meet: their edges, not the ring's.
        for i in range(nx):
            for j in range(nz):
                o = owner[i, j]
                if o < 0:
                    continue
                x, z = gx0 + i * g - x0, gz0 + j * g - z0
                if i + 1 < nx and owner[i + 1, j] not in (o, -1):
                    holdings.append([x + g, z, x + g, z + g])
                if j + 1 < nz and owner[i, j + 1] not in (o, -1):
                    holdings.append([x, z + g, x + g, z + g])
    return plans, holdings


def main():
    args = [a for i, a in enumerate(sys.argv[1:], 1)
            if not a.startswith("--") and sys.argv[i - 1] not in ("--margin", "--video", "--title", "--points")]
    margin = 96
    if "--margin" in sys.argv:
        margin = int(sys.argv[sys.argv.index("--margin") + 1])
    world, trace, log, out = Path(args[0]), args[1], Path(args[2]), Path(args[3])

    # Towns: their bells (the forum lines of the server's log) and names (the trace's).
    db = sqlite3.connect(trace)
    names = dict(db.execute("select town, name from towns order by day"))  # each town's latest name
    # The day each town is first in the trace by its name: its founding (a village the run began with, its first day;
    # a settlers' village, "Village 7" till it's a town, the day it took its name).
    founded = dict(db.execute("select town, min(day) from towns where name not like 'Village %' group by town"))
    last = db.execute("select max(day) from towns").fetchone()[0]
    folk = {t: (v, round(m, 2), c) for t, v, m, c in
            db.execute("select town, villagers, mood, comforts from towns where day=?", (last,))}
    bells = {}
    for line in log.read_text(errors="replace").splitlines():
        if "lays its forum round its bell at" in line:
            head, _, tail = line.partition("Town ")
            town = int(tail.split()[0])
            x, y, z = (int(v.strip()) for v in tail.split(" at ")[1].split("(")[0].split(","))
            bells[town] = (x, y, z)
    routes = []
    for route, frm, to, trips, carrier in db.execute(
            "select route, from_town, to_town, trips, carrier from routes where day=?", (last,)):
        hexes = [(q, r) for q, r in db.execute("select q, r from paths where route=? order by step", (route,))]
        if len(hexes) >= 2:
            # Each trip it made: when it left and when it was home again, in days.
            runs = [[round(a / 24000, 3), round(b / 24000, 3)] for a, b in db.execute(
                "select left_tick, home_tick from trips where route=? and home_tick is not null order by left_tick",
                (route,))]
            routes.append({"from": frm, "to": to, "trips": trips, "carrier": carrier, "hexes": hexes, "runs": runs})

    # Each route along its road as the game planned it over the land (the world's enterprise-roads.json), else taut.
    planned = {}
    roads = world / "enterprise-roads.json"
    if roads.exists():
        for road in json.loads(roads.read_text()):
            hexes = [tuple(h) for h in road["path"]]
            planned[(hexes[0], hexes[-1])] = road["line"]
            planned[(hexes[-1], hexes[0])] = list(reversed(road["line"]))
    for r in routes:
        hexes = r.pop("hexes")
        line = planned.get((hexes[0], hexes[-1])) or taut(hexes, bells)
        r["path"] = [[round(x, 1), round(z, 1)] for x, z in line]
    xs = [b[0] for b in bells.values()] + [p[0] for r in routes for p in r["path"]]
    zs = [b[2] for b in bells.values()] + [p[1] for r in routes for p in r["path"]]
    x0, x1 = int(min(xs)) - margin, int(max(xs)) + margin
    z0, z1 = int(min(zs)) - margin, int(max(zs)) + margin
    w, h = x1 - x0 + 1, z1 - z0 + 1
    print(f"drawing {w} x {h} blocks from x {x0}, z {z0}", file=sys.stderr)

    # The world drawn as it stood at each of the run's kept points (`--points RUN`: its dayN/world), else as WORLD
    # stands: the frames the clock steps through.
    if "--points" in sys.argv:
        run = Path(sys.argv[sys.argv.index("--points") + 1])
        kept = sorted((int(q.name[3:]), q / "world") for q in run.glob("day*") if (q / "world" / "region").exists())
    else:
        kept = [(last, world)]
    # About a megabyte of PNG for 3.5 million blocks: a pixel for each step x step blocks, the least step that keeps the
    # frames under 12 MB together.
    step = 1
    while len(kept) * w * h / step ** 2 > 12 * 3.5e6:
        step += 1
    frames = []
    for d, wd in kept:
        print(f"day {d}: drawing {wd}", file=sys.stderr)
        image, counts, lo, hi = picture(wd, x0, z0, x1, z1, bells, step)
        ps, holds = town_plans(wd, x0, z0)
        frames.append({"day": d, "image": image, "plans": ps, "holdings": holds})
    print("columns:", counts, file=sys.stderr)
    # Too big still (a detailed world compresses worse than the guess): every frame halved till they fit.
    while sum(len(f["image"]) for f in frames) > FRAMES_BUDGET:
        print(f"frames {sum(len(f['image']) for f in frames) // 2 ** 20} MB: halved", file=sys.stderr)
        for f in frames:
            f["image"] = halved(f["image"])

    towns = []
    for t, (x, y, z) in sorted(bells.items()):
        v, m, c = folk.get(t, (0, 0, 0))
        towns.append({"id": t, "name": names.get(t, f"Town {t}"), "x": x - x0, "z": z - z0, "y": y,
                      "villagers": v, "mood": m, "comforts": c, "founded": founded.get(t, 0)})
    for r in routes:
        r["path"] = [[p[0] - x0, p[1] - z0] for p in r["path"]]
    data = {"frames": frames, "w": w, "h": h, "x0": x0, "z0": z0, "lo": lo, "hi": hi, "day": last, "towns": towns,
            "routes": routes, "counts": counts}
    film = ""
    if "--video" in sys.argv:
        film = base64.b64encode(Path(sys.argv[sys.argv.index("--video") + 1]).read_bytes()).decode()
    title = sys.argv[sys.argv.index("--title") + 1] if "--title" in sys.argv else "Survey"
    page = TEMPLATE.replace("__TITLE__", html.escape(title)).replace("__DATA__", json.dumps(data)).replace("__VIDEO__", film)
    out.write_text(page)
    print(f"wrote {out} ({len(page) // 1024} KB)", file=sys.stderr)


TEMPLATE = Path(__file__).with_name("world_render.html").read_text() if Path(__file__).with_name("world_render.html").exists() else ""

if __name__ == "__main__":
    main()
