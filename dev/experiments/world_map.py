#!/usr/bin/env python3
"""The cartographer's map of a study's world, drawn from its saved data after a run.

    python3 dev/experiments/world_map.py [world/data dir] [out.html] [title]

Draws the hex land (cliffs and water shaded, each town's land tinted), every market by name (hubs ringed, the capital
starred, markets off the map pinned at the edge), and every caravan route along its planned hex path, its width by the
trips it has made. Reads only the world's saved files: run it after the server has stopped (or saved).
"""
import gzip
import html
import math
import struct
import sys
from pathlib import Path

ACROSS = 80
SIZE = ACROSS / math.sqrt(3)
CLIFF = 32


# --- a minimal reader for the gzipped NBT the game saves --------------------------------------------------------------
def _read(f, t):
    if t == 1: return struct.unpack(">b", f.read(1))[0]
    if t == 2: return struct.unpack(">h", f.read(2))[0]
    if t == 3: return struct.unpack(">i", f.read(4))[0]
    if t == 4: return struct.unpack(">q", f.read(8))[0]
    if t == 5: return struct.unpack(">f", f.read(4))[0]
    if t == 6: return struct.unpack(">d", f.read(8))[0]
    if t == 7: n = _read(f, 3); return f.read(n)
    if t == 8: n = struct.unpack(">H", f.read(2))[0]; return f.read(n).decode("utf-8", "replace")
    if t == 9:
        et = f.read(1)[0]; n = _read(f, 3)
        return [_read(f, et) for _ in range(n)]
    if t == 10:
        d = {}
        while True:
            tt = f.read(1)[0]
            if tt == 0: return d
            name = _read(f, 8)
            d[name] = _read(f, tt)
    if t == 11: n = _read(f, 3); return list(struct.unpack(">%di" % n, f.read(4 * n)))
    if t == 12: n = _read(f, 3); return list(struct.unpack(">%dq" % n, f.read(8 * n)))
    raise ValueError(t)


def load(path):
    if not path.exists():
        return {}
    with gzip.open(path, "rb") as f:
        t = f.read(1)[0]; _read(f, 8)
        return _read(f, t).get("data", {})


# --- hexes, as the game reckons them (pointy-top, axial) --------------------------------------------------------------
def hex_at(x, z):
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


def centre(q, r):
    return SIZE * math.sqrt(3) * (q + r / 2), SIZE * 1.5 * r


def corners(q, r):
    cx, cz = centre(q, r)
    return [(cx + SIZE * math.cos(math.radians(60 * i - 30)), cz + SIZE * math.sin(math.radians(60 * i - 30)))
            for i in range(6)]


def chunk_xz(packed):
    x = packed & 0xFFFFFFFF
    z = (packed >> 32) & 0xFFFFFFFF
    return (x - (1 << 32) if x >= 1 << 31 else x), (z - (1 << 32) if z >= 1 << 31 else z)


def pos(gp):
    p = gp["pos"]
    return p[0], p[2]


PALETTE = ["#44679b", "#b4582f", "#4f7d4a", "#8a5a9b", "#b08a2e", "#2f8a8a", "#9b4458", "#5a6b7d"]


def main():
    data = Path(sys.argv[1] if len(sys.argv) > 1 else "core/run/world/data")
    out = Path(sys.argv[2] if len(sys.argv) > 2 else "world_map.html")
    title = sys.argv[3] if len(sys.argv) > 3 else "The realm, by its cartographer"

    names = {pos(n["desk"]): n["name"] for n in load(data / "enterprise_names.dat").get("names", [])}
    territory = load(data / "enterprise_territory.dat")
    settlements = territory.get("settlements", [])
    bell_to_desk = {pos(s["bell"]): pos(s["desk"]) for s in settlements if "desk" in s}
    owners = {}
    for h in territory.get("hexes", []):
        owners[(h["q"], h["r"])] = bell_to_desk.get(pos(h["bell"]), pos(h["bell"]))
    routes = load(data / "enterprise_caravans.dat").get("routes", [])
    depots = load(data / "enterprise_depots.dat").get("depots", [])
    hubs = {pos(d["desk"]) for d in depots if d.get("kind") == "STOREHOUSE"}
    sovereign = load(data / "enterprise_sovereign.dat")
    capital = pos(sovereign["capital"]) if "capital" in sovereign else None
    off_map = {pos(m["at"]) for m in load(data / "enterprise_off_map.dat").get("markets", []) if "at" in m}

    # Relief by hex, from each chunk's lowest and highest surveyed surface.
    relief = {}
    for dim in load(data / "enterprise_terrain.dat").get("heights", []):
        for chunk, packed in zip(dim.get("chunks", []), dim.get("heights", [])):
            cx, cz = chunk_xz(chunk)
            hx = hex_at(cx * 16 + 8 + 0.5, cz * 16 + 8 + 0.5)
            low, high = (packed >> 16 & 0xFFFF) - 512, (packed & 0xFFFF) - 512
            lo, hi = relief.get(hx, (10 ** 6, -10 ** 6))
            relief[hx] = (min(lo, low), max(hi, high))

    # Water by chunk (only chunks surveyed: the rest are taken as dry), and the hexes' share of it.
    water = {}
    for dim in load(data / "enterprise_terrain.dat").get("dimensions", []):
        for chunk, wet in zip(dim.get("chunks", []), dim.get("water", b"")):
            water[chunk_xz(chunk)] = wet

    def wet_share(q, r):
        cx, cz = centre(q, r)
        wet = total = 0
        for chx in range(int(cx - SIZE) // 16 - 1, int(cx + SIZE) // 16 + 2):
            for chz in range(int(cz - SIZE) // 16 - 1, int(cz + SIZE) // 16 + 2):
                if hex_at(chx * 16 + 8.5, chz * 16 + 8.5) == (q, r):
                    wet += water.get((chx, chz), 0)
                    total += 16
        return wet / total if total else 0

    bridges = []
    for b in load(data / "enterprise_works.dat").get("bridges", []):
        h = b.get("hexes", [])
        if len(h) >= 6 and b.get("done"):
            bridges.append(((h[0], h[1]), (h[2], h[3]), (h[4], h[5]), pos(b["owner"])))

    # The map's extent: the markets on the map, and the paths, with a margin.
    markets = set(bell_to_desk.values()) | {pos(r["from"]) for r in routes} | {pos(r["to"]) for r in routes}
    routed = ({pos(r["from"]) for r in routes} | {pos(r["to"]) for r in routes}) - off_map
    on_map = [m for m in markets if m not in off_map]
    framed = routed or set(on_map)
    xs = [m[0] for m in framed] or [0]
    zs = [m[1] for m in framed] or [0]
    for r in routes:
        if pos(r["from"]) in off_map or pos(r["to"]) in off_map:
            continue  # its road runs off the map
        path = r.get("path", [])
        for i in range(0, len(path) - 1, 2):
            cx_, cz_ = centre(path[i], path[i + 1])
            xs.append(cx_); zs.append(cz_)
    pad = 160
    x0, x1, z0, z1 = min(xs) - pad, max(xs) + pad, min(zs) - pad, max(zs) + pad
    width = 1100
    scale = width / (x1 - x0)
    height = (z1 - z0) * scale

    def px(x, z):
        return (max(x0, min(x1, x)) - x0) * scale, (max(z0, min(z1, z)) - z0) * scale

    owners_list = sorted({o for o in owners.values()})
    tint = {o: PALETTE[i % len(PALETTE)] for i, o in enumerate(owners_list)}

    svg = []
    # Land: every hex in the frame, cliffs dark, steep ground shaded, towns' land tinted.
    step = SIZE * math.sqrt(3)
    for r_ in range(math.floor(z0 / (SIZE * 1.5)) - 2, math.ceil(z1 / (SIZE * 1.5)) + 3):
        for q_ in range(math.floor(x0 / step - r_ / 2) - 2, math.ceil(x1 / step - r_ / 2) + 3):
            cx, cz = centre(q_, r_)
            if not (x0 - SIZE <= cx <= x1 + SIZE and z0 - SIZE <= cz <= z1 + SIZE):
                continue
            lo, hi = relief.get((q_, r_), (0, 0))
            rel = max(0, hi - lo)
            fill, opacity = "var(--land)", 1
            wet = wet_share(q_, r_)
            if wet > 0.5:
                fill = "var(--sea)"
            elif wet > 0.15:
                fill = "var(--shore)"
            elif rel >= CLIFF:
                fill = "var(--cliff)"
            elif rel >= 8:
                fill = "var(--hill)"
            pts = " ".join(f"{a:.1f},{b:.1f}" for a, b in (px(x, z) for x, z in corners(q_, r_)))
            svg.append(f'<polygon points="{pts}" fill="{fill}" stroke="var(--grid)" stroke-width="0.6"/>')
            owner = owners.get((q_, r_))
            if owner is not None:
                svg.append(f'<polygon points="{pts}" fill="{tint[owner]}" fill-opacity="0.18" stroke="none">'
                           f'<title>{html.escape(names.get(owner, str(owner)))}\'s land</title></polygon>')

    # Bridges: a deck across the hex, between the neighbours it joins.
    for hexpos, a_, b_, owner in bridges:
        (ax, az), (bx, bz) = px(*centre(*a_)), px(*centre(*b_))
        mx, mz = px(*centre(*hexpos))
        ax, az, bx, bz = (mx + ax) / 2, (mz + az) / 2, (mx + bx) / 2, (mz + bz) / 2
        svg.append(f'<line x1="{ax:.1f}" y1="{az:.1f}" x2="{bx:.1f}" y2="{bz:.1f}" stroke="var(--deck)" '
                   f'stroke-width="9" stroke-linecap="square"><title>{html.escape(names.get(bell_to_desk.get(owner, owner), "a town"))}'
                   f"'s bridge</title></line>")

    # Routes along their paths, width by trips.
    most = max([r.get("trips", 0) for r in routes] or [1]) or 1
    for r in sorted(routes, key=lambda r: r.get("trips", 0)):
        path = r.get("path", [])
        a, b = pos(r["from"]), pos(r["to"])
        points = [px(*a)]
        for i in range(0, len(path) - 1, 2):
            points.append(px(*centre(path[i], path[i + 1])))
        points.append(px(*b))
        trips = r.get("trips", 0)
        w = 1.2 + 9 * math.sqrt(trips / most)
        colour = tint.get(a, "var(--ink)")
        dash = ' stroke-dasharray="6 5"' if r.get("stopping") else ""
        label = (f"{names.get(a, a)} → {names.get(b, b)}: {trips} trips, {r.get('itemsDelivered', 0)} items delivered, "
                 f"{r.get('stops', 0)} stops on the way; {r.get('status', '')}")
        line = " ".join(f"{x:.1f},{z:.1f}" for x, z in points)
        svg.append(f'<polyline points="{line}" fill="none" stroke="{colour}" stroke-width="{w:.1f}" '
                   f'stroke-linecap="round" stroke-linejoin="round" stroke-opacity="0.85"{dash}>'
                   f'<title>{html.escape(label)}</title></polyline>')

    # Markets: name, a ring for a hub, a star for the capital, a pin at the edge for one off the map.
    for m in sorted(markets):
        x, z = px(*m)
        name = names.get(m, f"{m[0]}, {m[1]}")
        far = m in off_map or not (x0 <= m[0] <= x1 and z0 <= m[1] <= z1)
        if far:
            svg.append(f'<rect x="{x - 5:.1f}" y="{z - 5:.1f}" width="10" height="10" fill="var(--paper)" '
                       f'stroke="var(--ink)" stroke-width="1.5"/>')
            # Off the map: its pin at the edge, its name tucked inside the frame below it.
            svg.append(f'<text x="{min(max(x, 8), width - 8):.1f}" y="{z + 22:.1f}" '
                       f'text-anchor="{"start" if x < width / 2 else "end"}" class="name">'
                       f'{html.escape(name)} <tspan class="tag">(off the map)</tspan></text>')
            continue
        else:
            if m in hubs:
                svg.append(f'<circle cx="{x:.1f}" cy="{z:.1f}" r="11" fill="none" stroke="var(--ink)" '
                           f'stroke-width="2.5"/>')
            svg.append(f'<circle cx="{x:.1f}" cy="{z:.1f}" r="6" fill="{tint.get(m, "var(--ink)")}" '
                       f'stroke="var(--paper)" stroke-width="2"/>')
        if m == capital:
            name = "★ " + name
        tag = " · hub" if m in hubs else ""
        # Labels clear of a near neighbour: to the left of a market with another close on its right.
        crowded = any(o != m and 0 < o[0] - m[0] < 420 and abs(o[1] - m[1]) < 90 for o in markets)
        anchor, dx = ("end", -13) if crowded else ("start", 13)
        svg.append(f'<text x="{x + dx:.1f}" y="{z + 4:.1f}" text-anchor="{anchor}" class="name">{html.escape(name)}'
                   f'<tspan class="tag">{tag}</tspan></text>')

    total_trips = sum(r.get("trips", 0) for r in routes)
    page = f"""<title>Cartographer's Map</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link href="https://fonts.googleapis.com/css2?family=Karla:wght@400;600&family=Lora:wght@600&display=swap" rel="stylesheet">
<style>
/* One map, framed like a sheet on the table; the key below it. b28 palette: steel blue, off-white, cream, ink. */
:root {{ --paper:#faf8f5; --cream:#f2efe9; --ink:#1c1917; --land:#efe9dc; --hill:#d9cdb4; --cliff:#8c7f6a;
  --grid:#e2dacb; --muted:#6b6259; --steel:#44679b; --sea:#a9c0d8; --shore:#cdd9e0; --deck:#7a5230; --shadow:rgba(28,25,23,.12); --lift:rgba(255,255,255,.55);
  --display:Lora, Georgia, serif; --body:Karla, "Helvetica Neue", Arial, sans-serif; }}
@media (prefers-color-scheme: dark) {{ :root:not([data-theme="light"]) {{ --paper:#1c1917; --cream:#26221f;
  --ink:#f2efe9; --land:#2b2723; --hill:#3d362f; --cliff:#7a6e5c; --grid:#332e29; --muted:#a69d92; --steel:#8fb0dc;
  --sea:#2c4058; --shore:#30393f; --deck:#b98a5a; --shadow:rgba(0,0,0,.4); --lift:rgba(255,255,255,.04); color-scheme:dark; }} }}
:root[data-theme="dark"] {{ --paper:#1c1917; --cream:#26221f; --ink:#f2efe9; --land:#2b2723; --hill:#3d362f;
  --cliff:#7a6e5c; --grid:#332e29; --muted:#a69d92; --steel:#8fb0dc; --sea:#2c4058; --shore:#30393f;
  --deck:#b98a5a; --shadow:rgba(0,0,0,.4);
  --lift:rgba(255,255,255,.04); color-scheme:dark; }}
body {{ background:var(--paper); color:var(--ink); font-family:var(--body); }}
main {{ max-width:1160px; margin:0 auto; padding-block:24px 40px; padding-inline:16px; }}
h1 {{ font-family:var(--display); font-weight:600; margin:0 0 4px; font-size:1.7rem; text-wrap:balance; }}
p.sub {{ margin:0 0 16px; color:var(--muted); max-width:65ch; }}
.map {{ background:var(--cream); border-radius:20px; padding:12px; box-shadow:6px 6px 18px var(--shadow),
  -4px -4px 12px var(--lift); }}
svg {{ width:100%; height:auto; display:block; }}
.name {{ font:600 13px var(--body); fill:var(--ink); paint-order:stroke; stroke:var(--cream); stroke-width:3px; }}
.tag {{ font-weight:400; fill:var(--muted); }}
.key {{ display:flex; flex-wrap:wrap; gap:8px 20px; margin-top:14px; color:var(--muted); font-size:.95rem; }}
.swatch {{ display:inline-block; width:14px; height:14px; border-radius:4px; vertical-align:-2px; margin-right:6px; }}
</style>
<main>
<h1>{html.escape(title)}</h1>
<p class="sub">{len(on_map)} markets on the map, {len(routes)} routes, {total_trips} caravan trips in all. Hover a route
for its trips and how it ended the run.</p>
<div class="map"><svg viewBox="0 0 {width} {height:.0f}" role="img" aria-label="Map of markets and caravan routes">
{chr(10).join(svg)}
</svg></div>
<div class="key">
<span><span class="swatch" style="background:var(--cliff)"></span>cliffs: no way across</span>
<span><span class="swatch" style="background:var(--hill)"></span>steep ground</span>
<span><span class="swatch" style="background:var(--sea)"></span>water</span>
<span><span class="swatch" style="background:var(--deck)"></span>bridge</span>
<span>line width: trips made</span><span>dashed: winding down</span><span>ringed: freight hub</span>
<span>★ the capital</span><span>square: off the map</span>
</div></main>"""
    out.write_text(page)
    print(f"map: {out}")


if __name__ == "__main__":
    main()
