#!/usr/bin/env python3
"""A long run of the supply network, logged day by day to CSV, with an optional injection of goods.

Drives the headless dev server over RCON (`just server-setup`, then `just server`). Builds the caravan network
(hub warehouse at 1000 1000; settlements S1-S4 at 300, 600, 1,200 and 4,000 blocks, each with a farm and 64 emeralds;
toll town T on the S3 route), then tick-warps day by day. With --inject DAY, a spring (a node whose warehouse refills
every day with 9 shulkers of goods) opens near S1 that day and sends caravans to S1, S2 and S3. With --river, a river
runs east-west through T's row of hexes before anything is built: 31 hexes flooded, the two by T left dry as its
bridges, so the roads to S2 and S3 cross T's land or go far around. With --river-bridge, the river runs through the row
south of T instead, with no dry crossing at all: S3 can't be reached until a town bridges it.

Each day it writes one row per town and one per route to the CSV (ledger reads only; nothing is loaded).

With --fleet, three trained Fishermen and a Leatherworker join the hub's squad: boats across water, fish to sell,
packs for more cargo.

With --services, a trained Cleric and Farmer join the hub's squad: they go along where a town wants a blessing or
help with its untended fields, and each town's services are printed at the end.

With --metal, the metal chain: a trained Toolsmith in S1 (it digs a mine), a Toolsmith, Armorer and Weaponsmith in T
(mine, smelter and smiths: tools for its farmers, arms and armour for its guards), and an Armorer and a Toolsmith in
the hub's squad (they smelt and forge for towns without). Each town's mine, kit and services are printed at the end.

With --trades, the rest of the trades: S1 takes in two Nitwits (they gather), a Mason (it builds), a Cleric and a
Fletcher (a temple and a watchtower to build with the Mason; the Fletcher hunts) and a master Farmer (flower beds on
the square), with beds for them; T takes in two Nitwits, who stand watch at its posts as militia (with --river). The
towns' buildings, hands and services are printed at the end.

With --add-hand TOWN:TRADE (e.g. S1:leatherworker), on the study's first day that town is loaded, a villager of the
trade put by its bell with a bed, and the town held loaded 1,200 ticks so the core counts it, then let go;
TOWN:none loads and lets go the same way with no one put there, the fair side to fork against.

With --master, the hub's Merchant is a master: it brokers with players' shops (Numismatics vendors) round its desk,
and its market (orders, signals, takings) is printed at the end.

With --war M,W,P, a sovereign at the hub (its capital: the Merchant a master, a master Librarian and a Cleric at
court, an armoury of six double chests by the desk, the hub a settlement, its chunks kept loaded; S2 promoted with a
barracks) is named on day 1, with 500 spurs a day of peacetime revenue: its metal exchange buys iron, coal, flint and
diamonds by contract (from a market across the sea, off the map, when no town has them), and it tenders for master
smiths on its board for the region's Merchants to take. It mobilises on day M (minting 1,000 spurs a day, stockpiling
kit through contracts, buying gold at a shop), declares war on a rival faction on day W (levying soldiers, who ride to
the front, off the map, with the kit its quartermaster contracts for), and makes peace on day P. Each day's
/enterprise war line is logged, the exchange's book every ten days, and at the end the tally, the exchange, tenders,
buildings and trust.

With --comforts, the hub also stocks a shulker of comforts (treats, clothing, furnishings) for towns to buy, and the
spring, once open, refills a shulker of comforts and a stack of books every day.

Checkpoints: with --save-at D,D,... the world is copied after day D (0: right after setup) to
core/run/checkpoints/LABEL/dayD (LABEL from --label, else "study"), with the flags it was run with. To fork a run from
one: `just server-restore core/run/checkpoints/LABEL/dayD`, start the server, and run the study with --resume and the
same DIR; it skips setup and carries on from day D+1 (a checkpoint is good for code changes that touch nothing before
its day). A day that takes far longer than the others is reported: the machine slept (a closed lid on battery) or the
server stalled.

Stopping early: --stop-when E2.treasury<5 (or any.treasury<5, S3.mood<0, E3.villagers>=12; comma-separated, any one
will do) ends the run at the end of the first day a town meets it, saves a checkpoint of that day, and prints the
town's cash book for the last ten days (`/enterprise cashbook DESK [DAYS]`): what its money went on.

Under `society=core` (the kept check, John 2026-10-18: most Java-server studies retire for sim/host, this one stays as
the in-game check) every number in the per-day CSV, in `--stop-when` and in the cash book it prints comes from the core:
the mint rate from `Query::Mint`, each town's row from `Query::Territory` (treasury, mood, villagers, hexes, guards,
tolls, locker trades, ...), each route's row from `Query::RouteBooks`, the cash book from `Query::CashBook` (all through
`binding/adapters/CommandsAdapter` and `BooksAdapter`: one question each, the lines out). The same lines carry the Java
commands' words, so the regexes below read either mode's output; the core's differences: money in emeralds (tolls,
traffic, route totals in spurs at the mint's rate), town names the core's, route numbers from 0, and a town's
`locker trades` line replaces the debug log's. Only the default world runs in core mode (plus `--save-at`, `--label`,
`--resume`, `--stop-when`): the other flags (river, ridge, war, metal, trades, freight, ...) set up and read what is
still Java rules (docs/binding-plan.md "Studies") and exit saying "Java mode only (retired for sim/host)".
The cash book, books, whys, trust and contracts the study prints are the core's own in core mode;
`/enterprise core cashbook|books|whys|trust|contracts` asks the core the same in either mode, so a Java-mode run can be set
beside it (money in emeralds there, spurs in Java's).

    python3 dev/experiments/network_study.py DAYS OUT.csv [--inject DAY] [--river | --river-bridge] [--comforts] [--fleet] [--services] [--metal] [--trades] [--master] [--war M,W,P] [--realms] [--save-at D,... [--label NAME]] [--resume DIR] [--stop-when TOWN.field<value,...]
"""

import csv
import json
import math
import re
import shutil
import socket
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
RCON = [sys.executable, str(REPO / "scripts" / "rcon.py")]
HUB = (1000, 1000)
SPRING = (900, 860)  # behind the hub: its route to S3 also crosses the toll town
TOWNS = {"S1": (1295, 995), "S2": (995, 1595), "S3": (1844, 1844), "S4": (4995, 995), "T": (1393, 1396)}
if "--ridge" in sys.argv:  # towns east of the ridge, left to find their own trade (through the pass, or not at all)
    TOWNS.update({"E1": (1650, 1350), "E2": (1300, 1750), "E3": (2050, 1300)})
ROUTED = ["S1", "S2", "S3", "S4"]
HEX_SIZE = 80 / math.sqrt(3)  # centre to corner
RIVER_ROW = 20  # T's row of hexes
RIVER_Q = range(-8, 25)
BRIDGES = {7, 8}  # T's home hex and the one its buildings spill into
COURT_SMITHS = [("weaponsmith", "grindstone"), ("armorer", "blast_furnace"), ("toolsmith", "smithing_table")]


sys.path.insert(0, str(REPO / "scripts"))
import rcon as rcon_client  # noqa: E402

RUN = REPO / "core" / "run" / ("lanes/" + str(rcon_client.LANE) if rcon_client.LANE else "")  # ENTERPRISE_LANE: a lane's server
LOG = RUN / "logs" / "debug.log"

_sock = None


def rcon(*commands):
    """Commands over one RCON connection kept open for the run (a process per call was most of a day's time)."""
    global _sock
    out = []
    for command in commands:
        for attempt in range(2):
            try:
                if _sock is None:
                    _sock = socket.create_connection((rcon_client.HOST, rcon_client.PORT), timeout=60)
                    rcon_client.packet(_sock, 1, 3, rcon_client.PASSWORD)
                    if rcon_client.read(_sock)[0] == -1:
                        sys.exit("RCON login failed")
                out.append(f"> {command}\n{rcon_client.command(_sock, 2, command)}\n")
                break
            except (OSError, ConnectionError):
                _sock = None
                if attempt:
                    raise
    return "".join(out)


def gametime():
    for _ in range(20):  # an RCON reply now and then comes back garbled: ask again
        found = re.search(r"time is (\d+)", rcon("time query gametime"))
        if found:
            return int(found.group(1))
        time.sleep(0.5)
    raise RuntimeError("the server won't say what time it is")


def sprint(ticks):
    start = gametime()
    rcon(f"tick sprint {ticks}")
    while gametime() < start + ticks:
        time.sleep(0.2)


def flag(name, default=None):
    return sys.argv[sys.argv.index(name) + 1] if name in sys.argv else default


def save_checkpoint(day):
    """Copies the world as it stands after this day, saving paused so the copy is whole."""
    dest = REPO / "core" / "run" / "checkpoints" / flag("--label", "study") / f"day{day}"
    rcon("save-off", "save-all flush")
    try:
        if dest.exists():
            shutil.rmtree(dest)
        shutil.copytree(RUN / "world", dest / "world", ignore=shutil.ignore_patterns("session.lock"))
    finally:
        rcon("save-on")
    # The study's own running count (from the server's log, which the checkpoint doesn't carry).
    (dest / "study.json").write_text(json.dumps({"day": day, "argv": sys.argv[1:], "core": CORE,
                                                "trades": {} if CORE else trades_by_town()}, indent=1))
    print(f"checkpoint: day {day} saved to {dest.relative_to(REPO)}")


def desk(corner):
    return f"{corner[0] + 7} -60 {corner[1] + 7}"


def bell(corner):
    return f"{corner[0] + 7} -60 {corner[1] + 4}"


def area(corner, size):
    """The square from a town's corner to `size` past it; in core mode at least everything within the census's 48
    blocks of its centre (its bell, or the hub's desk), or the core never counts the town's people: no hands, no
    caravans."""
    if CORE:
        return f"{corner[0] - 48} {corner[1] - 48} {corner[0] + max(size, 8) + 48} {corner[1] + max(size, 8) + 48}"
    return f"{corner[0] - 1} {corner[1] - 1} {corner[0] + size} {corner[1] + size}"


def number(pattern, text, default=0.0):
    match = re.search(pattern, text)
    return float(match.group(1)) if match else default


_log_offset = 0
_trades = {}
CORE = False  # society=core: every number comes from the core's own answers (set by main from the server's own words)

# Flags that shape the world or read what is still the Java rules': Java mode only. Core mode runs the default world.
CORE_FLAGS = {"--save-at", "--label", "--resume", "--stop-when", "--add-hand", "--realms"}


def census(corner):
    """A town's census taken now: the Java rules' at its bell, in core mode the core's at its desk."""
    return rcon(f"enterprise core census {desk(corner)}" if CORE else f"enterprise census {bell(corner)}")


def trades_by_town():
    """Locker trades so far by town, reading only what the debug log has added since last time."""
    global _log_offset
    for name in TOWNS:
        _trades.setdefault(name, 0)
    if LOG.exists():
        with LOG.open("rb") as log:
            if LOG.stat().st_size < _log_offset:
                _log_offset = 0  # a new log
            log.seek(_log_offset)
            text = log.read().decode(errors="replace")
            _log_offset = log.tell()
        for n, x, z in re.findall(r"Locker trades: (\d+) at (-?\d+), -60, (-?\d+)", text):
            for name, corner in TOWNS.items():
                if corner[0] <= int(x) <= corner[0] + 15 and corner[1] <= int(z) <= corner[1] + 15:
                    _trades[name] += int(n)
    return dict(_trades)


def hex_at(x, z):
    """The hex a block lies in, as the game rounds it (pointy-top, axial)."""
    q = (math.sqrt(3) / 3 * x - z / 3) / HEX_SIZE
    r = (2.0 / 3 * z) / HEX_SIZE
    s = -q - r
    rq, rr, rs = round(q), round(r), round(s)
    dq, dr, ds = abs(rq - q), abs(rr - r), abs(rs - s)
    if dq > dr and dq > ds:
        rq = -rr - rs
    elif dr > ds:
        rr = -rq - rs
    return rq, rr


def hex_centre(q, r):
    return HEX_SIZE * math.sqrt(3) * (q + r / 2), HEX_SIZE * 1.5 * r


def overlaps_town(x0, z0, x1, z1):
    return any(x0 <= c[0] + 29 and x1 >= c[0] - 2 and z0 <= c[1] + 29 and z1 >= c[1] - 2 for c in TOWNS.values())


def load_area(x0, z0, x1, z1):
    """Force-loads a box and waits until each of its corners (and middle) is loaded: fills fail on unloaded ground."""
    rcon(f"forceload add {x0} {z0} {x1} {z1}")
    points = [(x0, z0), (x1, z0), (x0, z1), (x1, z1), ((x0 + x1) // 2, (z0 + z1) // 2)]
    for _ in range(60):
        if all("passed" in rcon(f"execute if loaded {x} -61 {z}") for x, z in points):
            return
        sprint(10)
    print(f"warning: {x0},{z0} to {x1},{z1} not all loaded")


def build_river(row, bridges):
    """Floods the surface layer of each river hex in strips that stay inside the hex (pointy-top), one hex at a time.
    Strips that would run under a town are left dry."""
    strips = [(0, 22, 39), (23, 30, 26), (31, 38, 12)]  # |dz| from, to, half-width
    for q in RIVER_Q:
        if q in bridges:
            continue
        cx, cz = (round(v) for v in hex_centre(q, row))
        rcon(f"forceload add {cx - 40} {cz - 40} {cx + 40} {cz + 40}")
        sprint(20)
        for low, high, half in strips:
            for sign in (1, -1) if low else (1,):
                z0, z1 = (cz - high, cz + high) if not low else sorted((cz + sign * low, cz + sign * high))
                if not overlaps_town(cx - half, z0, cx + half, z1):
                    rcon(f"fill {cx - half} -61 {z0} {cx + half} -61 {z1} water replace grass_block")
        rcon(f"forceload remove {cx - 40} {cz - 40} {cx + 40} {cz + 40}")
    sprint(400)  # let the chunks unload: each is surveyed as it goes
    for q in (RIVER_Q.start, 0, 6, 7, 8, 9):
        x, z = hex_centre(q, row)
        print(rcon(f"enterprise hex {round(x)} -60 {round(z)}").strip())


RIDGE = 2789  # x + z along the ridge: through T, between the capital's side and S3's
RIDGE_HALF = 70  # hexes whose centre lies this near the line are mountain


def build_ridge():
    """A mountain range across the land between the capital's side and S3's: each hex near the line x + z = RIDGE
    raised 40 blocks of stone, all but T's (the pass). A hex with a mountain's flank in it can't be crossed."""
    t = TOWNS["T"]
    tx, tz = t[0] + 7, t[1] + 7  # T's desk: its hex is the pass, the one way through
    cells = [(q, r) for q in range(-30, 60) for r in range(-10, 60)]
    pass_hex = hex_at(tx + 0.5, tz + 0.5)  # the hex the game puts T's desk in (not merely the nearest centre)
    raised = 0
    for q, r in cells:
        if True:
            cx, cz = (round(v) for v in hex_centre(q, r))
            if abs(cx + cz - RIDGE) > RIDGE_HALF or not 600 <= cx <= 2200 or (q, r) == pass_hex:
                continue
            # Raise exactly the chunks the game counts as this hex's (their middles in it), so no slope spills into
            # a neighbour (a pass with a cliff at its mouth is no pass); never town land.
            chunks = [(chx, chz) for chx in range((cx - 60) // 16, (cx + 60) // 16 + 1)
                      for chz in range((cz - 60) // 16, (cz + 60) // 16 + 1)
                      if hex_at(chx * 16 + 8.5, chz * 16 + 8.5) == (q, r)
                      and not overlaps_town(chx * 16, chz * 16, chx * 16 + 15, chz * 16 + 15)]
            if len(chunks) < 2:
                continue
            # One chunk left low (the one farthest out): relief is measured within a hex, and a hex raised end to end
            # would be a plateau, open going on top. Rugged, it's a mountain.
            chunks.sort(key=lambda c: math.hypot(c[0] * 16 + 8 - cx, c[1] * 16 + 8 - cz))
            chunks = chunks[:-1]
            load_area(cx - 48, cz - 48, cx + 48, cz + 48)
            for chx, chz in chunks:
                rcon(f"fill {chx * 16} -60 {chz * 16} {chx * 16 + 15} -21 {chz * 16 + 15} stone")
            rcon(f"enterprise survey {cx - 48} -60 {cz - 48} {cx + 48} -60 {cz + 48}")  # seen now, not at unload
            rcon(f"forceload remove {cx - 40} {cz - 40} {cx + 40} {cz + 40}")
            raised += 1
    # The pass is a ravine: T's hex flooded round the town, crossed only by the town's one great bridge.
    cx, cz = (round(v) for v in hex_centre(*pass_hex))
    load_area(cx - 48, cz - 48, cx + 48, cz + 48)
    town = (t[0] - 2, t[1] - 2, t[0] + 29, t[1] + 29)
    wet_chunks = [(chx, chz) for chx in range((cx - 60) // 16, (cx + 60) // 16 + 1)
                  for chz in range((cz - 60) // 16, (cz + 60) // 16 + 1)
                  if hex_at(chx * 16 + 8.5, chz * 16 + 8.5) == pass_hex
                  and not overlaps_town(chx * 16, chz * 16, chx * 16 + 15, chz * 16 + 15)]
    for attempt in range(4):  # fills on ground still loading come to nothing: check, and fill again
        for chx, chz in wet_chunks:  # the hex's own chunks (not its neighbours'), round the town
            rcon(f"fill {chx * 16} -61 {chz * 16} {chx * 16 + 15} -61 {chz * 16 + 15} water replace grass_block")
        rcon(f"enterprise survey {cx - 48} -60 {cz - 48} {cx + 48} -60 {cz + 48}")
        wet = number(r"(\d+)% water", rcon(f"enterprise hex {cx} -60 {cz}"))
        if wet > 50:
            break
        sprint(100)
    print(f"ravine: {wet:.0f}% water")
    print(rcon(f"enterprise bridge {tx} -60 {tz} {tx} -60 {tz}").strip().splitlines()[-1])
    rcon(f"forceload remove {cx - 48} {cz - 48} {cx + 48} {cz + 48}")
    print(f"ridge: {raised} hexes raised, the pass at T a ravine with one bridge")


def build_coast():
    """The capital a port: the sea west of it (the surface flooded hex by hex, west of x 950 across the road to the
    market across the sea), and Fishermen at the capital to captain its boats."""
    flooded = 0
    for q in range(-30, 60):
        for r in range(-10, 60):
            cx, cz = (round(v) for v in hex_centre(q, r))
            if not (440 <= cx <= 950 and 520 <= cz <= 1480):
                continue
            load_area(cx - 44, cz - 44, cx + 44, cz + 44)
            for attempt in range(4):
                for z in range(cz - 44, cz + 45, 22):
                    rcon(f"fill {cx - 44} -61 {z} {min(cx + 44, 950 + 40)} -61 {min(z + 21, cz + 44)} water "
                         f"replace grass_block")
                rcon(f"enterprise survey {cx - 48} -60 {cz - 48} {cx + 48} -60 {cz + 48}")
                if number(r"(\d+)% water", rcon(f"enterprise hex {cx} -60 {cz}")) > 50:
                    break
                sprint(100)
            rcon(f"forceload remove {cx - 44} {cz - 44} {cx + 44} {cz + 44}")
            flooded += 1
    rcon(f"forceload add {HUB[0] + 16} {HUB[1] + 10} {HUB[0] + 30} {HUB[1] + 24}")
    sprint(20)
    for i in range(2):
        # In the capital's own chunk (the one kept loaded), so they're there when a boat wants a captain.
        rcon(f"summon minecraft:villager {HUB[0] + 4 + i}.5 -60 {HUB[1] + 5}.5 {{PersistenceRequired:1b,Xp:1,"
             f"VillagerData:{{profession:\"minecraft:fisherman\",level:3,type:\"minecraft:plains\"}}}}",
             f"setblock {HUB[0] + 22 + 2 * i} -60 {HUB[1] + 18} minecraft:white_bed[facing=north,part=head]",
             f"setblock {HUB[0] + 22 + 2 * i} -60 {HUB[1] + 19} minecraft:white_bed[facing=north,part=foot]")
    rcon(f"enterprise census {HUB[0] + 4} -60 {HUB[1] + 16}")
    rcon(f"forceload remove {HUB[0] + 16} {HUB[1] + 10} {HUB[0] + 30} {HUB[1] + 24}")
    print(f"coast: {flooded} hexes of sea west of the capital, two Fishermen to captain its boats")


def setup():
    hub_desk = f"{HUB[0] + 2} -60 {HUB[1] + 2}"
    # The capital's town kept loaded (at war its people, Fishermen among them, must be there to sail and serve).
    rcon(f"forceload add {area(HUB, 31 if '--war' in sys.argv else 6)}",
         f"execute positioned {HUB[0]} -60 {HUB[1]} run function testbench:network/hub")
    if "--comforts" in sys.argv:
        rcon(f"execute positioned {HUB[0]} -60 {HUB[1]} run function testbench:network/comforts")
    if "--fleet" in sys.argv:
        for i, trade in enumerate(["fisherman", "fisherman", "fisherman", "leatherworker"]):
            rcon(f"summon minecraft:villager {HUB[0] + 1 + i % 3}.5 -60 {HUB[1] + 1 + i // 3}.5 {{PersistenceRequired:1b,Xp:1,"
                 f"VillagerData:{{profession:\"minecraft:{trade}\",level:1,type:\"minecraft:plains\"}}}}")
    if "--services" in sys.argv:
        for i, trade in enumerate(["cleric", "farmer"]):
            rcon(f"summon minecraft:villager {HUB[0] + 2 + i}.5 -60 {HUB[1] + 4}.5 {{PersistenceRequired:1b,Xp:1,"
                 f"VillagerData:{{profession:\"minecraft:{trade}\",level:2,type:\"minecraft:plains\"}}}}")
    if "--metal" in sys.argv:
        for i, trade in enumerate(["armorer", "toolsmith"]):
            rcon(f"summon minecraft:villager {HUB[0] + 4 + i}.5 -60 {HUB[1] + 4}.5 {{PersistenceRequired:1b,Xp:1,"
                 f"VillagerData:{{profession:\"minecraft:{trade}\",level:2,type:\"minecraft:plains\"}}}}")
    if "--war" in sys.argv:
        # The sovereign's court at the hub: a bell, beds, a master Librarian and a Cleric; kept loaded, as a capital is.
        rcon(f"forceload add {HUB[0] - 12} {HUB[1] - 15} {HUB[0] + 20} {HUB[1] + 22}",
             f"setblock {HUB[0] + 4} -60 {HUB[1] + 16} minecraft:bell[attachment=floor]")
        for i in range(4):
            rcon(f"setblock {HUB[0] + 2 * i} -60 {HUB[1] + 18} minecraft:white_bed[facing=north,part=head]",
                 f"setblock {HUB[0] + 2 * i} -60 {HUB[1] + 19} minecraft:white_bed[facing=north,part=foot]")
        for i, (trade, lvl) in enumerate([("librarian", 5), ("cleric", 3)]):
            rcon(f"summon minecraft:villager {HUB[0] + 2 + 2 * i}.5 -60 {HUB[1] + 15}.5 {{PersistenceRequired:1b,Xp:250,"
                 f"VillagerData:{{profession:\"minecraft:{trade}\",level:{lvl},type:\"minecraft:plains\"}}}}")
        # The sovereign's armoury by the desk: six double chests, one store with the desk's, for war goods.
        x0, z0 = HUB[0] + 8, HUB[1] + 2
        for row in (0, 3):
            for i in range(3):
                x = x0 + 1 + 2 * i
                rcon(f"setblock {x} -60 {z0 + 1 + row} minecraft:chest[facing=south,type=left]",
                     f"setblock {x + 1} -60 {z0 + 1 + row} minecraft:chest[facing=south,type=right]")
    if "--master" in sys.argv or "--war" in sys.argv:
        rcon(f"execute as @e[type=villager,x={HUB[0]},y=-61,z={HUB[1]},dx=8,dy=3,dz=8] "
             f"if data entity @s VillagerData{{profession:\"enterprise:merchant\"}} "
             f"run data merge entity @s {{VillagerData:{{level:5}},Xp:250}}")
    for corner in TOWNS.values():
        rcon(f"forceload add {area(corner, 27)}",
             f"execute positioned {corner[0]} -60 {corner[1]} run function testbench:network/settlement")
    if "--trades" in sys.argv:
        for name, trades in [("S1", [("nitwit", 1), ("nitwit", 1), ("mason", 3), ("cleric", 3), ("fletcher", 3),
                                     ("farmer", 5)]), ("T", [("nitwit", 1), ("nitwit", 1)])]:
            corner = TOWNS[name]
            for i, (trade, lvl) in enumerate(trades):
                rcon(f"summon minecraft:villager {corner[0] + 3 + 2 * i}.5 -60 {corner[1] + 16}.5 {{PersistenceRequired:1b,"
                     f"Xp:{0 if trade == 'nitwit' else 1},VillagerData:{{profession:\"minecraft:{trade}\",level:{lvl},"
                     f"type:\"minecraft:plains\"}}}}",
                     f"setblock {corner[0] + 3 + 2 * i} -60 {corner[1] + 18} minecraft:white_bed[facing=north,part=head]",
                     f"setblock {corner[0] + 3 + 2 * i} -60 {corner[1] + 19} minecraft:white_bed[facing=north,part=foot]")
    if "--metal" in sys.argv:
        for name, trades in [("S1", ["toolsmith"]), ("T", ["toolsmith", "armorer", "weaponsmith"])]:
            b = [int(v) for v in bell(TOWNS[name]).split()]
            for i, trade in enumerate(trades):
                rcon(f"summon minecraft:villager {b[0] + 1 + i}.5 {b[1]} {b[2] + 1}.5 {{PersistenceRequired:1b,Xp:1,"
                     f"VillagerData:{{profession:\"minecraft:{trade}\",level:2,type:\"minecraft:plains\"}}}}")
    if CORE:
        # A desk set by command in a chunk already loaded is unseen by the core until that chunk loads again: each
        # town founded at its desk now, as the testbench's load does (`enterprise core stock` founds one).
        for at in [hub_desk] + [desk(corner) for corner in TOWNS.values()]:
            rcon(f"enterprise core stock {at}")
    rcon("time set 1000")
    sprint(300)
    if not CORE:
        for corner in TOWNS.values():
            rcon(f"enterprise autoswitch {desk(corner)} selling")
    if "--war" in sys.argv:
        # The capital's Merchant a master again (it manages ten lockers).
        rcon(f"execute as @e[type=villager,x={HUB[0]},y=-61,z={HUB[1]},dx=8,dy=3,dz=8] "
             f"if data entity @s VillagerData{{profession:\"enterprise:merchant\"}} "
             f"run data merge entity @s {{VillagerData:{{level:5}},Xp:7000}}")
        print(rcon(f"enterprise depot add {hub_desk} armoury {HUB[0] + 8} -60 {HUB[1] + 2} {HUB[0] + 15} -59 {HUB[1] + 6}")
              .strip().splitlines()[-1])
    sprint(600)
    for corner in TOWNS.values():
        census(corner)
    if "--war" in sys.argv:
        # S2 promoted: a barracks, four beds for men under arms and two double chests for their kit and rations.
        c = TOWNS["S2"]
        rcon(f"fill {c[0] + 18} -61 {c[1] + 16} {c[0] + 24} -61 {c[1] + 22} minecraft:stone_bricks")
        for i in range(4):
            rcon(f"setblock {c[0] + 18 + 2 * i} -60 {c[1] + 17} minecraft:red_bed[facing=north,part=head]",
                 f"setblock {c[0] + 18 + 2 * i} -60 {c[1] + 18} minecraft:red_bed[facing=north,part=foot]")
        for x in (c[0] + 19, c[0] + 22):
            rcon(f"setblock {x} -60 {c[1] + 21} minecraft:chest[facing=north,type=right]",
                 f"setblock {x + 1} -60 {c[1] + 21} minecraft:chest[facing=north,type=left]")
        sprint(20)
        print(rcon(f"enterprise depot add {desk(c)} barracks {c[0] + 18} -60 {c[1] + 16} {c[0] + 24} -59 {c[1] + 22}")
              .strip().splitlines()[-1])
        rcon(f"enterprise census {bell(c)}")  # its beds counted
    if "--war" in sys.argv:
        rcon(f"enterprise census {HUB[0] + 4} -60 {HUB[1] + 16}")
        print(rcon(f"enterprise war capital {hub_desk} 500").strip())
    if "--ridge" in sys.argv or "--realms" in sys.argv:
        build_coast()  # the sea west of the hub, and its Fishermen to captain the Coast's boats
    if "--ridge" in sys.argv:
        build_ridge()
        # The pass town's carrying trade: Leatherworkers (packs), a Shepherd to lead, idle hands (porters, draymen),
        # and llamas in a pen by its desk.
        t = TOWNS["T"]
        rcon(f"forceload add {area(t, 27)}")
        sprint(20)
        for i, (trade, lvl) in enumerate([("leatherworker", 3), ("leatherworker", 3), ("shepherd", 3), ("nitwit", 1),
                                          ("nitwit", 1), ("nitwit", 1)]):
            rcon(f"summon minecraft:villager {t[0] + 3 + 2 * i}.5 -60 {t[1] + 20}.5 {{PersistenceRequired:1b,"
                 f"Xp:{0 if trade == 'nitwit' else 1},VillagerData:{{profession:\"minecraft:{trade}\",level:{lvl},"
                 f"type:\"minecraft:plains\"}}}}",
                 f"setblock {t[0] + 3 + 2 * i} -60 {t[1] + 22} minecraft:white_bed[facing=north,part=head]",
                 f"setblock {t[0] + 3 + 2 * i} -60 {t[1] + 23} minecraft:white_bed[facing=north,part=foot]")
        for i in range(4):
            rcon(f"summon minecraft:llama {t[0] + 4 + i}.5 -60 {t[1] + 9}.5 {{PersistenceRequired:1b,NoAI:1b,Tame:1b}}")
        rcon(f"item replace block {t[0] + 8} -60 {t[1] + 7} container.2 with minecraft:emerald 64")  # a carrying house's capital
        rcon(f"enterprise census {bell(t)}")
    if "--freight" in sys.argv:
        # S2 a freight hub: a storehouse (two double chests) its carriers and forwarder run; its own caravans to the
        # capital; S3's caravans to S2. The capital no longer runs the long road to S3: S3's goods come through S2.
        c = TOWNS["S2"]
        rcon(f"fill {c[0] + 18} -61 {c[1] + 25} {c[0] + 24} -61 {c[1] + 27} minecraft:stone_bricks")
        for x in (c[0] + 19, c[0] + 22):
            rcon(f"setblock {x} -60 {c[1] + 26} minecraft:chest[facing=north,type=right]",
                 f"setblock {x + 1} -60 {c[1] + 26} minecraft:chest[facing=north,type=left]")
        sprint(20)
        print(rcon(f"enterprise depot add {desk(c)} storehouse {c[0] + 18} -60 {c[1] + 25} {c[0] + 24} -59 {c[1] + 27}")
              .strip().splitlines()[-1])
    for name in ROUTED:
        if "--freight" in sys.argv and name == "S3":
            continue
        print(f"route hub -> {name}: " + rcon(f"enterprise route add {hub_desk} {desk(TOWNS[name])}").strip().splitlines()[-1])
    if "--ridge" in sys.argv:  # S2's caravans trade across the ridge too: two routes through the pass
        print("route S2 -> S3: " + rcon(f"enterprise route add {desk(TOWNS['S2'])} {desk(TOWNS['S3'])}")
              .strip().splitlines()[-1])
    if "--freight" in sys.argv:
        print("route S2 -> hub: " + rcon(f"enterprise route add {desk(TOWNS['S2'])} {hub_desk}").strip().splitlines()[-1])
        print("route S3 -> S2: " + rcon(f"enterprise route add {desk(TOWNS['S3'])} {desk(TOWNS['S2'])}")
              .strip().splitlines()[-1])
        print("route S2 -> S3: " + rcon(f"enterprise route add {desk(TOWNS['S2'])} {desk(TOWNS['S3'])}")
              .strip().splitlines()[-1])
    if CORE:
        # The core knows a locker only where a loaded villager has claimed its workstation (the binding's lockers),
        # and switches on only lockers it knows: so after the villagers settle, before their towns unload.
        sprint(1200)
        for corner in TOWNS.values():
            print(f"{desk(corner)}: " + rcon(f"enterprise autoswitch {desk(corner)} selling").strip().splitlines()[-1])
    for corner in [*([] if "--war" in sys.argv else [HUB]), *TOWNS.values()]:
        rcon(f"forceload remove {area(corner, 27)}")


def open_spring():
    spring_desk = f"{SPRING[0] + 2} -60 {SPRING[1] + 2}"
    rcon(f"forceload add {area(SPRING, 6)}",
         f"execute positioned {SPRING[0]} -60 {SPRING[1]} run function testbench:network/spring")
    sprint(100)
    for name in ["S1", "S2", "S3"]:
        print(f"route spring -> {name}: "
              + rcon(f"enterprise route add {spring_desk} {desk(TOWNS[name])}").strip().splitlines()[-1])
    if "--comforts" in sys.argv:
        rcon(f"execute positioned {SPRING[0]} -60 {SPRING[1]} run function testbench:network/comforts",
             f"item replace block {SPRING[0] + 1} -60 {SPRING[1] + 2} container.10 with minecraft:book 64")
    rcon(f"forceload remove {area(SPRING, 6)}")


def refill_spring():
    rcon(f"forceload add {area(SPRING, 6)}")
    sprint(20)
    rcon(f"execute positioned {SPRING[0]} -60 {SPRING[1]} run function testbench:network/spring_refill")
    if "--comforts" in sys.argv:  # a workshop's daily output of comforts too, books for reading
        rcon(f"execute positioned {SPRING[0]} -60 {SPRING[1]} run function testbench:network/comforts",
             f"item replace block {SPRING[0] + 1} -60 {SPRING[1] + 2} container.10 with minecraft:book 64")
    rcon(f"forceload remove {area(SPRING, 6)}")


STOP = re.compile(r"(\w+)\.(\w+)\s*(<=|>=|<|>)\s*(-?[\d.]+)")


def stop_met(towns):
    """The --stop-when conditions met today: TOWN.field<value (TOWN a key like E2, or any), comma-separated; e.g.
    any.treasury<5, S3.mood<0, E3.villagers>=12. Returns (why, town) for each town that meets one."""
    met = []
    for cond in [c for c in (flag("--stop-when", "") or "").split(",") if c.strip()]:
        m = STOP.fullmatch(cond.strip())
        if not m:
            sys.exit(f"--stop-when: can't read {cond!r} (TOWN.field<value)")
        who, field, op, value = m.group(1), m.group(2), m.group(3), float(m.group(4))
        for r in towns:
            if who not in ("any", r["name"]) or field not in r:
                continue
            v = float(r[field])
            if {"<": v < value, "<=": v <= value, ">": v > value, ">=": v >= value}[op]:
                met.append((f"{r['name']} {field} {v:g} {op} {value:g}", r["name"]))
    return met


def main():
    global CORE
    days = int(sys.argv[1])
    out = Path(sys.argv[2])
    CORE = "The core's mint" in rcon("enterprise mint")
    if CORE:
        retired = sorted(a for a in sys.argv[3:] if a.startswith("--") and a not in CORE_FLAGS)
        if retired:
            sys.exit(f"{' '.join(retired)}: Java mode only (retired for sim/host); under society=core this study runs the "
                     f"default world (with --save-at, --label, --resume, --stop-when, --add-hand, --realms)")
    inject = int(sys.argv[sys.argv.index("--inject") + 1]) if "--inject" in sys.argv else None
    save_at = {int(d) for d in flag("--save-at", "").split(",") if d}
    started = time.time()
    first = 1
    if "--resume" in sys.argv:
        saved = json.loads((Path(flag("--resume")) / "study.json").read_text())
        first = saved["day"] + 1
        _trades.update(saved.get("trades", {}))
        # Flags that shape the world before the first day must match the checkpoint's.
        shaping = {"--river", "--river-bridge", "--ridge", "--realms", "--war", "--comforts", "--fleet", "--services", "--metal",
                   "--trades", "--master"}
        theirs = {a for a in saved["argv"] if a in shaping}
        ours = {a for a in sys.argv if a in shaping}
        if theirs != ours:
            sys.exit(f"the checkpoint was set up with {sorted(theirs)}, not {sorted(ours)}")
        if saved.get("core", False) != CORE:
            mode = lambda core: "society=core" if core else "Java mode"
            sys.exit(f"the checkpoint was a {mode(saved.get('core', False))} study; this server runs {mode(CORE)}")
        print(f"resumed from {flag('--resume')}: day {saved['day']} done, carrying on from day {first}")
    else:
        if "--river" in sys.argv:
            build_river(RIVER_ROW, BRIDGES)
        elif "--river-bridge" in sys.argv:
            build_river(RIVER_ROW + 1, set())
        setup()
        print(f"setup took {time.time() - started:.0f}s")
        if 0 in save_at:
            save_checkpoint(0)
    rows = []
    war = [int(d) for d in sys.argv[sys.argv.index("--war") + 1].split(",")] if "--war" in sys.argv else None
    took = []
    for day in range(first, days + 1):
        day_started = time.time()
        if "--realms" in sys.argv and day == first:
            # The realms beyond the border: the Coast over the sea west of the capital, its fleets landing at the
            # capital's port; the March overland to the east, its caravans to the eastern towns.
            print(rcon(f"enterprise realms open coast {HUB[0] - 2398} -60 {HUB[1] + 2} {HUB[0] + 2} -60 {HUB[1] + 2}")
                  .strip().splitlines()[-1])
            if "E3" in TOWNS:  # the eastern towns are the ridge's (--ridge)
                print(rcon(f"enterprise realms open march 3400 -60 1350 {desk(TOWNS['E3'])}; {desk(TOWNS['E1'])}")
                      .strip().splitlines()[-1])
        if war and day == war[0]:
            print("day %d: " % day + rcon(f"enterprise war mobilise {HUB[0] + 2} -60 {HUB[1] + 2} 1000").strip().splitlines()[-1])
        if war and day == war[1]:
            print("day %d: " % day + rcon("enterprise war declare faction").strip().splitlines()[-1])
        if war and day == war[2]:
            print("day %d: " % day + rcon("enterprise war peace").strip().splitlines()[-1])
        if "--add-hand" in sys.argv and day == first:
            name, trade = flag("--add-hand").split(":")
            corner = TOWNS[name]
            b = [int(v) for v in bell(corner).split()]
            rcon(f"forceload add {area(corner, 27)}")
            sprint(20)  # the chunks load
            if trade != "none":
                rcon(f"summon minecraft:villager {b[0] + 2}.5 {b[1]} {b[2] + 2}.5 {{PersistenceRequired:1b,Xp:1,"
                     f"VillagerData:{{profession:\"minecraft:{trade}\",level:1,type:\"minecraft:plains\"}}}}",
                     f"setblock {b[0] + 4} -60 {b[2] + 2} minecraft:white_bed[facing=north,part=head]",
                     f"setblock {b[0] + 4} -60 {b[2] + 3} minecraft:white_bed[facing=north,part=foot]")
            sprint(1200)
            rcon(f"forceload remove {area(corner, 27)}")
            print(f"day {day}: {name} " + (f"takes in a {trade}" if trade != "none" else "loaded and let go"))
        if CORE and day % 4 == 1 and day > 1:
            # The offers a villager unlocked by levelling up, switched on as a player would on the Merchant's screen.
            for name, corner in TOWNS.items():
                rcon(f"forceload add {desk(corner).replace(' -60', '')}")  # the command reads a loaded desk
                said = rcon(f"enterprise autoswitch {desk(corner)} unlocked").strip().splitlines()
                rcon(f"forceload remove {desk(corner).replace(' -60', '')}")
                if said and not said[-1].startswith("Switched on 0 "):
                    print(f"day {day}: {name} {said[-1]}")
        if inject == day:
            open_spring()
        elif inject is not None and day > inject:
            refill_spring()
        sprint(24000)
        took.append(time.time() - day_started)
        usual = sorted(took)[len(took) // 2]
        if len(took) > 3 and took[-1] > max(60, 6 * usual):
            print(f"day {day} took {took[-1]:.0f}s, not the usual {usual:.0f}s: the machine slept (lid closed on "
                  f"battery?) or the server stalled; `pmset -g log | grep Sleep` tells which")
        rate = number(r"([\d.]+) spurs per emerald", rcon("enterprise mint"), 16)
        trades = {} if CORE else trades_by_town()
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
                "trades": number(r"locker trades: (\d+)", t) if CORE else trades[name],
                "mood": number(r"mood ([+-][\d.]+)", t),
                "villagers": number(r"\): (\d+) villagers", t),
                "beds": number(r"villagers, (\d+) beds", t),
                "homes": number(r"(\d+) home\(s\) built", t),
                "born": number(r"(\d+) child\(ren\) born", t),
                "wall_pct": number(r"wall (\d+)% built", t),
                "bridges": len(re.findall(r"bridge on", t)),
                "culture": number(r"culture ([\d.]+) of", t),
                "focus": (re.search(r"focus: (\w+)", t) or [None, ""])[1],
            })
        routes = rcon("enterprise routes")
        for line in routes.splitlines():
            m = re.match(r"\s+#(\d+) ([-\d, ]+) -> ([-\d, ]+): (\d+) blocks, (\d+) trips, (\d+) items sold for (\d+) spurs, "
                         r"(\d+) for services, (\d+) bought for (\d+), (\d+) spurs on road food, (\d+) tolls paid, "
                         r"(\d+) spurs brought home", line)
            if m:
                origin = "spring" if m.group(2).startswith(str(SPRING[0] + 2)) else "hub"
                dest = next((n for n, c in TOWNS.items() if m.group(3).startswith(f"{c[0] + 7}, -60, {c[1] + 7}")), "?")
                rows.append({
                    "day": day, "kind": "route", "name": f"{origin}->{dest}", "mint_rate": rate,
                    "blocks": int(m.group(4)), "trips": int(m.group(5)), "sold_items": int(m.group(6)),
                    "sales_spurs": int(m.group(7)), "service_spurs": int(m.group(8)), "bought_items": int(m.group(9)),
                    "purchase_spurs": int(m.group(10)), "food_spurs": int(m.group(11)), "tolls": int(m.group(12)),
                    "home_spurs": int(m.group(13)),
                })
        if war is not None:
            line = rcon("enterprise war").strip().splitlines()[-1]
            rows.append({"day": day, "kind": "war", "name": "sovereign", "mint_rate": rate, "war": line,
                         "treasury": number(r"treasury (\d+) spurs", line),
                         "minted": number(r"minted (\d+)", line),
                         "contracts": number(r"contracts (\d+)", line),
                         "delivered": number(r"delivered (\d+)", line),
                         "serving": number(r"army (\d+) serving", line),
                         "fallen": number(r"(\d+) fallen", line),
                         "bills": number(r"bills ([\d.]+)", line)})
            if day % 5 == 0 or day in war:
                print(f"  {line}")
            if day % 10 == 0:
                print("  " + rcon("enterprise metals").strip().replace("\n", "\n  "))
            sys.stdout.flush()
        towns = [r for r in rows if r["day"] == day and r["kind"] == "town"]
        print(f"day {day:>2}: " + " | ".join(
            f"{r['name']} {r['treasury']:.0f}em {r['mood']:+.1f} {int(r['villagers'])}v {int(r['hexes'])}h "
            f"{int(r['guards'])}g {r['toll_pct']:.0f}% {r['trades']}t"
            for r in towns)
            + f" | mint {rate:.1f} | {time.time() - day_started:.0f}s")
        sys.stdout.flush()
        met = stop_met(towns)
        if day in save_at or met:
            save_checkpoint(day)
        if met:
            print(f"stopped on day {day}: " + "; ".join(why for why, _ in met))
            for _, name in met:
                print(f"\n{name}: " + rcon(f"enterprise cashbook {desk(TOWNS[name])} 10").strip())
            break
    if war is not None:
        print("\n" + rcon("enterprise war tally").strip())
        print("\n" + rcon("enterprise war").strip())
        print("\n" + rcon(f"enterprise market {HUB[0] + 2} -60 {HUB[1] + 2}").strip())
        print("\n" + rcon("enterprise metals").strip())
        print("\n" + rcon("enterprise tenders").strip())
        print("\n" + rcon("enterprise depot").strip())
        print("\n" + rcon("enterprise freight").strip())

        print("\n" + rcon("enterprise trust").strip())
        print("\n" + rcon("enterprise contracts").strip())
        print("\n" + rcon("enterprise routes").strip())
        for name, corner in TOWNS.items():
            print(f"\n{name} trades: " + rcon(f"enterprise territory {bell(corner)}").strip().splitlines()[1])
    if "--ridge" in sys.argv:
        t = TOWNS["T"]
        for x, z in [(1200, RIDGE - 1200), (t[0] + 7, t[1] + 7)]:
            print("\n" + rcon(f"enterprise hex {x} -60 {z}").strip())
        print("\n" + rcon("enterprise freight").strip())
        print("\n" + rcon("enterprise paperwork").strip())
        print("\n" + rcon("enterprise post").strip())
        print("\n" + rcon("enterprise forwarder").strip())
        print("\n" + rcon("enterprise expeditions").strip())
        print("\n" + rcon("enterprise realms").strip())
        print("\n" + rcon(f"enterprise books {HUB[0] + 2} -60 {HUB[1] + 2}").strip())
        # Each market's reasons: why goods went or didn't, why its workshop stood idle.
        for name, corner in TOWNS.items():
            print("\n" + rcon(f"enterprise whys {desk(corner)}").strip())
        if "--realms" in sys.argv:
            print("\n" + rcon(f"enterprise whys {HUB[0] - 2398} -60 {HUB[1] + 2}").strip())
            print("\n" + rcon("enterprise whys 3400 -60 1350").strip())
        print("\nT: " + "\n".join(l for l in rcon(f"enterprise territory {bell(t)}").splitlines()
                                    if re.search(r"toll|guard|post|treasury|fee", l)))
    names = rcon("enterprise name")
    for label, corner in [("hub", HUB), *TOWNS.items()]:
        x, z = (HUB[0] + 2, HUB[1] + 2) if label == "hub" else (corner[0] + 7, corner[1] + 7)
        found = re.search(rf"\n  (\S.*?) at {x}, -60, {z}", names)
        print(f"{label} is {found.group(1) if found else '?'}")
    if "--metal" in sys.argv:
        for name in ["S1", "T"]:
            t = rcon(f"enterprise territory {bell(TOWNS[name])}")
            print(f"\n{name}: " + "\n".join(l for l in t.splitlines() if re.search(r"mine|farms|toll|guard|treasury", l)))
    if "--master" in sys.argv:
        rcon(f"forceload add {area(HUB, 6)}")
        sprint(1300)
        print("\nhub market: " + rcon(f"enterprise market {HUB[0] + 2} -60 {HUB[1] + 2}").strip())
    if "--trades" in sys.argv:
        for name in ["S1", "T"]:
            t = rcon(f"enterprise territory {bell(TOWNS[name])}")
            print(f"\n{name}: " + "\n".join(l for l in t.splitlines()
                                             if re.search(r"buildings|hands|mood|treasury|toll|guard|culture", l)))
    if "--services" in sys.argv or "--metal" in sys.argv or "--trades" in sys.argv:
        for name, corner in TOWNS.items():
            done = [l for l in rcon(f"enterprise services {desk(corner)}").splitlines()[2:] if not l.startswith("  its ")]
            print(f"\n{name}:\n" + "\n".join(done))
    fields = sorted({key for row in rows for key in row}, key=lambda k: (k not in ("day", "kind", "name"), k))
    with out.open("w", newline="") as f:
        writer = csv.DictWriter(f, fieldnames=fields)
        writer.writeheader()
        writer.writerows(rows)
    print(f"\nwrote {len(rows)} rows to {out}; {time.time() - started:.0f}s in all")


if __name__ == "__main__":
    main()
