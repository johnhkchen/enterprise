#!/usr/bin/env python3
"""The surveyor's map of a study's world: its blocks from above, after a run (core mode).

    python3 dev/experiments/world_render.py WORLD TRACE.db SERVER.log OUT.html [--margin 96]

Reads the world's own region files (stop the server first, so they're saved) and draws each column's top: trees,
roads and the towns' forums, homes and other buildings, fields by crop, pastures (birch-fenced, their grass tinted),
water, and the land itself hill-shaded by its height. Over it, from the run's trace (`enterprise-sim --trace`): each
caravan route along its hex path, as wide as the trips it made, with wagons moving along it, more where it carried
more; and each town by name at its bell (the forum lines of SERVER.log). Pure Python (no numpy): a minute or two.
"""
import base64
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


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    margin = 96
    if "--margin" in sys.argv:
        margin = int(sys.argv[sys.argv.index("--margin") + 1])
    world, trace, log, out = Path(args[0]), args[1], Path(args[2]), Path(args[3])

    # Towns: their bells (the forum lines of the server's log) and names (the trace's).
    db = sqlite3.connect(trace)
    names = dict(db.execute("select distinct town, name from towns"))
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
            routes.append({"from": frm, "to": to, "trips": trips, "carrier": carrier, "hexes": hexes})

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
    image = base64.b64encode(png(w, h, rows)).decode()

    towns = []
    for t, (x, y, z) in sorted(bells.items()):
        v, m, c = folk.get(t, (0, 0, 0))
        towns.append({"id": t, "name": names.get(t, f"Town {t}"), "x": x - x0, "z": z - z0, "y": y,
                      "villagers": v, "mood": m, "comforts": c})
    for r in routes:
        r["path"] = [[p[0] - x0, p[1] - z0] for p in r["path"]]
    data = {"w": w, "h": h, "x0": x0, "z0": z0, "lo": lo, "hi": hi, "day": last, "towns": towns, "routes": routes,
            "counts": counts}
    page = TEMPLATE.replace("__IMAGE__", image).replace("__DATA__", json.dumps(data))
    out.write_text(page)
    print(f"wrote {out} ({len(page) // 1024} KB)", file=sys.stderr)


TEMPLATE = Path(__file__).with_name("world_render.html").read_text() if Path(__file__).with_name("world_render.html").exists() else ""

if __name__ == "__main__":
    main()
