#!/usr/bin/env python3
"""Does a town left to itself find an economy? One settlement, no caravans, nothing switched on by hand.

Drives the headless dev server over RCON (`just server-setup`, then `just server`). Builds the network study's
settlement (a Merchant's desk, a farmer, librarian, cartographer and fletcher with empty lockers, a carrot field, 64
emeralds and a shulker of bread) at 3000 3000 with no trades switched on and no caravans, then tick-warps day by day
and logs what the ledger says: locker trades, money, what its farms grow, its mood, its fields and stalls.

With --herders, a Shepherd and a Butcher join, each with a workstation and a chest (a locker) by the pen.

    python3 dev/experiments/standalone_town.py [DAYS] [--herders]

Retired for sim/host (John, 2026-10-18): Java mode only. Under society=core it exits saying so.
"""

import re
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(Path(__file__).resolve().parent))
import study_mode  # noqa: E402
RCON = [sys.executable, str(REPO / "scripts" / "rcon.py")]
LOG = REPO / "core" / "run" / "logs" / "debug.log"
CORNER = (3000, 3000)


def rcon(*commands):
    return subprocess.run(RCON + list(commands), capture_output=True, text=True, check=True).stdout


def gametime():
    return int(re.search(r"time is (\d+)", rcon("time query gametime")).group(1))


def sprint(ticks):
    start = gametime()
    rcon(f"tick sprint {ticks}")
    while gametime() < start + ticks:
        time.sleep(0.2)


def number(pattern, text, default=0.0):
    match = re.search(pattern, text)
    return float(match.group(1)) if match else default


def main():
    study_mode.java_only('standalone_town.py')
    days = int(sys.argv[1]) if len(sys.argv) > 1 and sys.argv[1].isdigit() else 15
    x, z = CORNER
    bell = f"{x + 7} -60 {z + 4}"
    rcon(f"forceload add {x - 1} {z - 1} {x + 27} {z + 27}",
         f"execute positioned {x} -60 {z} run function testbench:network/settlement", "time set 1000")
    if "--herders" in sys.argv:
        for i, (trade, block) in enumerate([("shepherd", "loom"), ("butcher", "smoker")]):
            sx, sz = x + 3 + 4 * i, z + 10
            rcon(f"setblock {sx} -60 {sz} minecraft:{block}", f"setblock {sx + 1} -60 {sz} minecraft:chest",
                 f"summon minecraft:villager {sx} -60 {sz + 1} {{PersistenceRequired:1b,Xp:1,VillagerData:"
                 f"{{profession:\"minecraft:{trade}\",level:1,type:\"minecraft:plains\"}}}}")
    sprint(600)
    rcon(f"enterprise census {bell}", f"forceload remove {x - 1} {z - 1} {x + 27} {z + 27}")
    print("day | trades | money | farms | mood | fields | stalls | herd")
    for day in range(1, days + 1):
        sprint(24000)
        t = rcon(f"enterprise territory {bell}")
        log = LOG.read_text(errors="replace")
        trades = sum(int(n) for n, tx in re.findall(r"Locker trades: (\d+) at (\d+), -60", log) if abs(int(tx) - x) < 40)
        farms = ", ".join(f"{item} {float(rate):.0f}" for item, rate in re.findall(r"farms minecraft:([a-z_]+): ([\d.]+)", t))
        fields = len(re.findall(r"plants a field", log))
        stalls = len(re.findall(r"puts up a", log))
        herd = (re.search(r"herd: ([^;]*)", t) or [None, ""])[1]
        print(f"{day:>3} | {trades:>6} | {number(r'treasury ([\d.]+)', t):>5.0f} | {farms} | "
              f"{number(r'mood ([+-][\d.]+)', t):+.1f} | {fields} | {stalls} | {herd}")
        sys.stdout.flush()
    print(re.sub(r"(?m)^", "  ", "\n".join(l for l in log.splitlines() if "plants a field" in l or "puts up a" in l or "pasture" in l)))


if __name__ == "__main__":
    main()
