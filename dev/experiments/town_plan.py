#!/usr/bin/env python3
"""A town's plan as JSON, from a stopped server's world (`enterprise_town_plans.dat`, the plan `TownPlans` fixes):

    python3 dev/experiments/town_plan.py WORLD TOWN OUT.json [--width 14]

Writes its bell, its walls (inner and outer rectangles), its streets as polylines with widths (the four avenues from
the forum's edge to the outer wall, the lanes inside the inner wall), its forum, and its plots (rectangles with the
street they front and the way their door faces) as `TownPlans.frontages` and `suburbs` cut them for builds `--width`
wide (a house 10 wide with its side yards: 14), so a terrain grader (the studio's, thread 0013) can work on the plan.
Its geometry is `TownPlans`'s, ported: change one, change both. Pure Python, as `world_render.py` (whose reader it uses).
"""
import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import world_render  # noqa: E402

GATE, LANE, STRIP, FORUM = 3, 4, 3, 6  # TownPlans.GATE, LANE, STRIP; Forums.REACH


def within(inner):
    x0, z0, x1, z1 = inner
    m = STRIP + 1
    return x0 + m, z0 + m, x1 - m, z1 - m


def lane_lines(plan, ns):
    w = within(plan["inner"])
    start = plan["bell"][0] if ns else plan["bell"][2]
    lo, hi = (w[0], w[2]) if ns else (w[1], w[3])
    out, at = [], start - plan["ward"]
    while at - LANE >= lo:
        out.append(at)
        at -= plan["ward"]
    at = start + plan["ward"]
    while at + LANE <= hi:
        out.append(at)
        at += plan["ward"]
    return out


def on_lane(plan, x, z):
    w = within(plan["inner"])
    if not plan["lanes"] or not (w[0] <= x <= w[2] and w[1] <= z <= w[3]):
        return False
    return any(lx - LANE // 2 - 1 <= x <= lx + LANE - LANE // 2 for lx in lane_lines(plan, True)) or \
        any(lz - LANE // 2 - 1 <= z <= lz + LANE - LANE // 2 for lz in lane_lines(plan, False))


def runs(lo, hi, streets):
    out, start = [], None
    for v in range(lo, hi + 2):
        kept = v > hi or any(s[0] - s[1] - 1 <= v <= s[0] + s[2] + 1 for s in streets)
        if not kept and start is None:
            start = v
        elif kept and start is not None:
            out.append((start, v - 1))
            start = None
    return out


def slots(lo, hi, width, gap):
    length = hi - lo + 1
    n = (length + gap) // (width + gap)
    out = []
    for i in range(n):
        a, b = lo + i * (length + gap) // n, lo + (i + 1) * (length + gap) // n - gap
        out.append((a, b - a))
    return out


def plots(plan, width):
    bx, bz = plan["bell"][0], plan["bell"][2]
    depth = (plan["ward"] - LANE - 2) // 2
    w = within(plan["inner"])
    ns = [(bx, GATE // 2, GATE // 2, "avenue")] + [(x, LANE // 2, LANE - LANE // 2 - 1, "lane") for x in lane_lines(plan, True)]
    ew = [(bz, GATE // 2, GATE // 2, "avenue")] + [(z, LANE // 2, LANE - LANE // 2 - 1, "lane") for z in lane_lines(plan, False)]
    out = []
    for s in ns:
        for lo, hi in runs(w[1], w[3], ew):
            for a, n in slots(lo, hi, width, 0):
                out.append((s[0] - s[1] - 2 - depth + 1, a, "east", depth, n, s))
                out.append((s[0] + s[2] + 2, a, "west", depth, n, s))
    for s in ew:
        for lo, hi in runs(w[0], w[2], ns):
            for a, n in slots(lo, hi, width, 0):
                out.append((a, s[0] - s[1] - 2 - depth + 1, "south", n, depth, s))
                out.append((a, s[0] + s[2] + 2, "north", n, depth, s))

    def keep(p):
        x, z, _, sx, sz, _ = p
        if x < w[0] or z < w[1] or x + sx - 1 > w[2] or z + sz - 1 > w[3]:
            return False
        for dx in range(sx):
            for dz in range(sz):
                if abs(x + dx - bx) <= GATE // 2 + 1 or abs(z + dz - bz) <= GATE // 2 + 1 or on_lane(plan, x + dx, z + dz):
                    return False
        f = FORUM + 1
        return not (x <= bx + f and x + sx - 1 >= bx - f and z <= bz + f and z + sz - 1 >= bz - f)

    inside = [p for p in out if keep(p)]
    # The suburbs: along the avenues between the walls.
    i, o = plan["inner"], plan["outer"]
    m, a = STRIP + 1, GATE // 2
    sub = []
    for lo, hi in ((o[1] + m, i[1] - m), (i[3] + m, o[3] - m)):
        for s, n in slots(lo, hi, width, 0):
            sub.append((bx - a - 2 - depth + 1, s, "east", depth, n, ns[0]))
            sub.append((bx + a + 2, s, "west", depth, n, ns[0]))
    for lo, hi in ((o[0] + m, i[0] - m), (i[2] + m, o[2] - m)):
        for s, n in slots(lo, hi, width, 0):
            sub.append((s, bz - a - 2 - depth + 1, "south", n, depth, ew[0]))
            sub.append((s, bz + a + 2, "north", n, depth, ew[0]))
    return [{"x": x, "z": z, "sx": sx, "sz": sz, "door": door, "street": f"{st[3]}:{'x' if st in ns else 'z'}={st[0]}",
             "suburb": k >= len(inside)} for k, (x, z, door, sx, sz, st) in enumerate(inside + sub)]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("world")
    ap.add_argument("town")
    ap.add_argument("out")
    ap.add_argument("--width", type=int, default=14)
    args = ap.parse_args()
    data = world_render.saved(Path(args.world) / "data" / "enterprise_town_plans.dat")
    data = data.get("data", data)
    e = data["towns"][args.town]
    bell = e["bell"]
    v = bell & ((1 << 64) - 1)  # BlockPos.asLong: x 26 bits, z 26, y 12

    def signed(n, bits):
        return n - (1 << bits) if n >= 1 << (bits - 1) else n
    bell = [signed(v >> 38, 26), signed(v & 0xFFF, 12), signed((v >> 12) & ((1 << 26) - 1), 26)]
    plan = {"bell": [bell[0], bell[1], bell[2]], "inner": list(e["inner"]), "outer": list(e["outer"]),
            "ward": e.get("ward", 20), "lanes": bool(e.get("lanes")), "avenues": bool(e.get("avenues"))}
    bx, bz, o, f = plan["bell"][0], plan["bell"][2], plan["outer"], FORUM + 1
    w = within(plan["inner"])
    streets = [{"kind": "avenue", "width": GATE, "line": line} for line in (
        [[bx, bz - f], [bx, o[1]]], [[bx, bz + f], [bx, o[3]]], [[bx - f, bz], [o[0], bz]], [[bx + f, bz], [o[2], bz]])]
    streets += [{"kind": "lane", "width": LANE, "line": [[x, w[1]], [x, w[3]]]} for x in lane_lines(plan, True)]
    streets += [{"kind": "lane", "width": LANE, "line": [[w[0], z], [w[2], z]]} for z in lane_lines(plan, False)]
    out = {"town": int(args.town), **plan, "forum": {"centre": [bx, bz], "reach": FORUM},
           "edge": {"inner": plan["inner"], "outer": plan["outer"]}, "streets": streets,
           "plots": plots(plan, args.width), "plot_width": args.width}
    Path(args.out).write_text(json.dumps(out, indent=1))
    print(f"town {args.town}: bell {plan['bell']}, {len(streets)} streets, {len(out['plots'])} plots "
          f"({sum(p['suburb'] for p in out['plots'])} in the suburbs)")


if __name__ == "__main__":
    main()
