#!/usr/bin/env python3
"""A long run of the supply network, logged day by day to CSV, with an optional injection of goods.

Drives the headless dev server over RCON (`just server-setup`, then `just server`). Builds the caravan network
(hub warehouse at 1000 1000; settlements S1-S4 at 300, 600, 1,200 and 4,000 blocks, each with a farm and 64 emeralds;
toll town T on the S3 route), then tick-warps day by day. With --inject DAY, a spring (a node whose warehouse refills
every day with 9 shulkers of goods) opens near S1 that day and sends caravans to S1, S2 and S3. With --river, a river
runs east-west through T's row of hexes before anything is built: 31 hexes flooded, the two by T left dry as its
bridges, so the roads to S2 and S3 cross T's land or go far around.

Each day it writes one row per town and one per route to the CSV (ledger reads only; nothing is loaded).

With --comforts, the hub also stocks a shulker of comforts (treats, clothing, furnishings) for towns to buy.

    python3 dev/experiments/network_study.py DAYS OUT.csv [--inject DAY] [--river] [--comforts]
"""

import csv
import math
import re
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
RCON = [sys.executable, str(REPO / "scripts" / "rcon.py")]
LOG = REPO / "core" / "run" / "logs" / "debug.log"
HUB = (1000, 1000)
SPRING = (900, 860)  # behind the hub: its route to S3 also crosses the toll town
TOWNS = {"S1": (1295, 995), "S2": (995, 1595), "S3": (1844, 1844), "S4": (4995, 995), "T": (1393, 1396)}
ROUTED = ["S1", "S2", "S3", "S4"]
HEX_SIZE = 80 / math.sqrt(3)  # centre to corner
RIVER_ROW = 20  # T's row of hexes
RIVER_Q = range(-8, 25)
BRIDGES = {7, 8}  # T's home hex and the one its buildings spill into


def rcon(*commands):
    return subprocess.run(RCON + list(commands), capture_output=True, text=True, check=True).stdout


def gametime():
    return int(re.search(r"time is (\d+)", rcon("time query gametime")).group(1))


def sprint(ticks):
    start = gametime()
    rcon(f"tick sprint {ticks}")
    while gametime() < start + ticks:
        time.sleep(0.2)


def desk(corner):
    return f"{corner[0] + 7} -60 {corner[1] + 7}"


def bell(corner):
    return f"{corner[0] + 7} -60 {corner[1] + 4}"


def area(corner, size):
    return f"{corner[0] - 1} {corner[1] - 1} {corner[0] + size} {corner[1] + size}"


def number(pattern, text, default=0.0):
    match = re.search(pattern, text)
    return float(match.group(1)) if match else default


def trades_by_town():
    text = LOG.read_text(errors="replace") if LOG.exists() else ""
    totals = {name: 0 for name in TOWNS}
    for n, x, z in re.findall(r"Locker trades: (\d+) at (-?\d+), -60, (-?\d+)", text):
        for name, corner in TOWNS.items():
            if corner[0] <= int(x) <= corner[0] + 15 and corner[1] <= int(z) <= corner[1] + 15:
                totals[name] += int(n)
    return totals


def hex_centre(q, r):
    return HEX_SIZE * math.sqrt(3) * (q + r / 2), HEX_SIZE * 1.5 * r


def build_river():
    """Floods the surface layer of each river hex in strips that stay inside the hex (pointy-top), one hex at a time."""
    strips = [(0, 22, 39), (23, 30, 26), (31, 38, 12)]  # |dz| from, to, half-width
    for q in RIVER_Q:
        if q in BRIDGES:
            continue
        cx, cz = (round(v) for v in hex_centre(q, RIVER_ROW))
        rcon(f"forceload add {cx - 40} {cz - 40} {cx + 40} {cz + 40}")
        sprint(20)
        for low, high, half in strips:
            for sign in (1, -1) if low else (1,):
                z0, z1 = (cz - high, cz + high) if not low else sorted((cz + sign * low, cz + sign * high))
                rcon(f"fill {cx - half} -61 {z0} {cx + half} -61 {z1} water replace grass_block")
        rcon(f"forceload remove {cx - 40} {cz - 40} {cx + 40} {cz + 40}")
    sprint(400)  # let the chunks unload: each is surveyed as it goes
    for q in (RIVER_Q.start, 0, 6, 7, 8, 9):
        x, z = hex_centre(q, RIVER_ROW)
        print(rcon(f"enterprise hex {round(x)} -60 {round(z)}").strip())


def setup():
    hub_desk = f"{HUB[0] + 2} -60 {HUB[1] + 2}"
    rcon(f"forceload add {area(HUB, 6)}", f"execute positioned {HUB[0]} -60 {HUB[1]} run function testbench:network/hub")
    if "--comforts" in sys.argv:
        rcon(f"execute positioned {HUB[0]} -60 {HUB[1]} run function testbench:network/comforts")
    for corner in TOWNS.values():
        rcon(f"forceload add {area(corner, 27)}",
             f"execute positioned {corner[0]} -60 {corner[1]} run function testbench:network/settlement")
    rcon("time set 1000")
    sprint(300)
    for corner in TOWNS.values():
        rcon(f"enterprise autoswitch {desk(corner)} selling")
    sprint(600)
    for corner in TOWNS.values():
        rcon(f"enterprise census {bell(corner)}")
    for name in ROUTED:
        print(f"route hub -> {name}: " + rcon(f"enterprise route add {hub_desk} {desk(TOWNS[name])}").strip().splitlines()[-1])
    for corner in [HUB, *TOWNS.values()]:
        rcon(f"forceload remove {area(corner, 27)}")


def open_spring():
    spring_desk = f"{SPRING[0] + 2} -60 {SPRING[1] + 2}"
    rcon(f"forceload add {area(SPRING, 6)}",
         f"execute positioned {SPRING[0]} -60 {SPRING[1]} run function testbench:network/spring")
    sprint(100)
    for name in ["S1", "S2", "S3"]:
        print(f"route spring -> {name}: "
              + rcon(f"enterprise route add {spring_desk} {desk(TOWNS[name])}").strip().splitlines()[-1])
    rcon(f"forceload remove {area(SPRING, 6)}")


def refill_spring():
    rcon(f"forceload add {area(SPRING, 6)}")
    sprint(20)
    rcon(f"execute positioned {SPRING[0]} -60 {SPRING[1]} run function testbench:network/spring_refill",
         f"forceload remove {area(SPRING, 6)}")


def main():
    days = int(sys.argv[1])
    out = Path(sys.argv[2])
    inject = int(sys.argv[sys.argv.index("--inject") + 1]) if "--inject" in sys.argv else None
    if "--river" in sys.argv:
        build_river()
    setup()
    rows = []
    for day in range(1, days + 1):
        if inject == day:
            open_spring()
        elif inject is not None and day > inject:
            refill_spring()
        sprint(24000)
        rate = number(r"([\d.]+) spurs per emerald", rcon("enterprise mint"), 16)
        trades = trades_by_town()
        for name, corner in TOWNS.items():
            t = rcon(f"enterprise territory {bell(corner)}")
            rows.append({
                "day": day, "kind": "town", "name": name, "mint_rate": rate,
                "treasury": number(r"treasury ([\d.]+)", t),
                "hexes": number(r"holds (\d+) hex", t),
                "guards": number(r"(\d+) guard\(s\) posted", t),
                "posts": (re.search(r"posted at \[(.*?)\]", t) or [None, ""])[1],
                "toll_pct": number(r"toll ([\d.]+)%", t),
                "toll_income": number(r"\((\d+) spurs collected last review\)", t),
                "traffic": number(r"traffic (\d+) spurs a day", t),
                "deserted": number(r"(\d+) deserted", t),
                "farm_share_pct": number(r"\((\d+)% of the harvest", t),
                "harvested": number(r"(\d+) items harvested", t),
                "trades": trades[name],
                "mood": number(r"mood ([+-][\d.]+)", t),
            })
        routes = rcon("enterprise routes")
        for line in routes.splitlines():
            m = re.match(r"\s+#(\d+) ([-\d, ]+) -> ([-\d, ]+): (\d+) blocks, (\d+) trips, (\d+) items sold for (\d+) spurs, "
                         r"(\d+) bought for (\d+), (\d+) spurs on road food, (\d+) tolls paid, (\d+) spurs brought home", line)
            if m:
                origin = "spring" if m.group(2).startswith(str(SPRING[0] + 2)) else "hub"
                dest = next((n for n, c in TOWNS.items() if m.group(3).startswith(f"{c[0] + 7}, -60, {c[1] + 7}")), "?")
                rows.append({
                    "day": day, "kind": "route", "name": f"{origin}->{dest}", "mint_rate": rate,
                    "blocks": int(m.group(4)), "trips": int(m.group(5)), "sold_items": int(m.group(6)),
                    "sales_spurs": int(m.group(7)), "bought_items": int(m.group(8)), "purchase_spurs": int(m.group(9)),
                    "food_spurs": int(m.group(10)), "tolls": int(m.group(11)), "home_spurs": int(m.group(12)),
                })
        towns = [r for r in rows if r["day"] == day and r["kind"] == "town"]
        print(f"day {day:>2}: " + " | ".join(
            f"{r['name']} {r['treasury']:.0f}em {r['mood']:+.1f} {int(r['guards'])}g {r['toll_pct']:.0f}% {r['trades']}t"
            for r in towns)
            + f" | mint {rate:.1f}")
        sys.stdout.flush()
    fields = sorted({key for row in rows for key in row}, key=lambda k: (k not in ("day", "kind", "name"), k))
    with out.open("w", newline="") as f:
        writer = csv.DictWriter(f, fieldnames=fields)
        writer.writeheader()
        writer.writerows(rows)
    print(f"\nwrote {len(rows)} rows to {out}")


if __name__ == "__main__":
    main()
