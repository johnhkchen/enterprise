#!/usr/bin/env python3
"""Do farm wages settle into an equilibrium, or can farms print emeralds?

Drives the headless dev server over RCON (`just server-setup`, then `just server`). Builds a farm town (three Master
farmers on a 480-plot carrot field, about 256 carrots and 5.8 emeralds of wages a day, 32 emeralds of capital):
  A: one buyer: the farmer at the town's composter locker buys carrots (22 to the emerald, 16 a restock), switched on;
  B: nobody buys.
Both are left unloaded (rounds settle them) and logged daily: harvest delivered, wages paid and unpaid, and the town's
money (emeralds plus coins, in emeralds at the current rate).

    python3 dev/experiments/farm_wages.py [days] [A|B]
"""

import re
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
RCON = [sys.executable, str(REPO / "scripts" / "rcon.py")]
CORNER = (3000, 3000)
DESK = f"{CORNER[0] + 7} -60 {CORNER[1] + 7}"
BELL = f"{CORNER[0] + 7} -60 {CORNER[1] + 4}"
AREA = f"{CORNER[0] - 1} {CORNER[1] - 1} {CORNER[0] + 27} {CORNER[1] + 40}"
COIN_SPURS = {"spur": 1, "bevel": 8, "sprocket": 16, "cog": 64, "crown": 512, "sun": 4096}


def rcon(*commands):
    return subprocess.run(RCON + list(commands), capture_output=True, text=True, check=True).stdout


def gametime():
    return int(re.search(r"time is (\d+)", rcon("time query gametime")).group(1))


def sprint(ticks):
    start = gametime()
    rcon(f"tick sprint {ticks}")
    while gametime() < start + ticks:
        time.sleep(0.4)


def money():
    """The town's emeralds plus coins, in emeralds at the current rate (read with the town loaded)."""
    rcon(f"forceload add {AREA}")
    sprint(60)
    stock = rcon(f"enterprise stock {DESK}")
    rcon(f"forceload remove {AREA}")
    emeralds = sum(int(n) for n in re.findall(r"minecraft:emerald (\d+)", stock))
    spurs = sum(COIN_SPURS[c] * int(n) for c, n in re.findall(r"numismatics:([a-z]+) (\d+)", stock))
    carrots = sum(int(n) for n in re.findall(r"minecraft:carrot (\d+)", stock))
    rate = float(re.search(r"([\d.]+) spurs per emerald", rcon("enterprise mint")).group(1))
    return emeralds + spurs / rate, carrots


def main():
    days = int(next((a for a in sys.argv[1:] if a.isdigit()), 12))
    scenario = "B" if "B" in sys.argv[1:] else "A"
    rcon(f"forceload add {AREA}", f"execute positioned {CORNER[0]} -60 {CORNER[1]} run function testbench:network/farm_town",
         "time set 1000")
    sprint(900)
    if scenario == "A":
        rcon(f"execute positioned {CORNER[0] + 12} -60 {CORNER[1] + 12} run function testbench:network/farm_town_buyers")
        composter = f"{CORNER[0] + 12} -60 {CORNER[1] + 12}"
        print(rcon(f'enterprise switch {DESK} {composter} "minecraft:carrot+>minecraft:emerald" true').strip().splitlines()[-1])
    print(rcon(f"enterprise census {BELL}").strip().splitlines()[-1])
    rcon(f"forceload remove {AREA}")
    print(f"\nScenario {scenario}: " + ("one villager buys carrots" if scenario == "A" else "nobody buys") + "\n")
    print("day | harvested | wages paid | unpaid | money (emeralds) | carrots in stock")
    for day in range(1, days + 1):
        sprint(24000)
        report = rcon(f"enterprise territory {BELL}")
        wages = re.search(r"([\d.]+) paid, ([\d.]+) unpaid.*?(\d+) items harvested", report)
        paid, unpaid, harvested = (float(wages.group(1)), float(wages.group(2)), int(wages.group(3))) if wages else (0, 0, 0)
        held, carrots = money()
        print(f"{day:>3} | {harvested:>9} | {paid:>10.1f} | {unpaid:>6.1f} | {held:>16.1f} | {carrots:>16}")
        sys.stdout.flush()


if __name__ == "__main__":
    main()
