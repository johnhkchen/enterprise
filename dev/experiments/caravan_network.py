#!/usr/bin/env python3
"""A caravan network: one hub warehouse supplying three settlements and collecting their coins.

Drives the headless dev server over RCON (`just server-setup`, then `just server`). Builds a hub at 1000 -60 1000
(18 shulkers of goods, 9 of bread, a Blaze Banker) and settlements 300, 600 and 1,200 blocks away whose only stock
is a shulker of bread. Routes run hub -> each settlement; a fourth at 4,000 blocks should be refused for food. Then
everything is left unloaded (rounds and caravans load what they need) and each day is logged.

    python3 dev/experiments/caravan_network.py [days]
"""

import re
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
RCON = [sys.executable, str(REPO / "scripts" / "rcon.py")]
LOG = REPO / "core" / "run" / "logs" / "debug.log"
HUB = (1000, 1000)
# Market corner, so the desk sits at +7 +7; distances are desk to desk from the hub desk at +2 +2.
SETTLEMENTS = {"S1 (300)": (1295, 995), "S2 (600)": (995, 1595), "S3 (1200)": (1844, 1844), "S4 (4000)": (4995, 995)}


def rcon(*commands):
    return subprocess.run(RCON + list(commands), capture_output=True, text=True, check=True).stdout


def gametime():
    return int(re.search(r"time is (\d+)", rcon("time query gametime")).group(1))


def sprint(ticks):
    start = gametime()
    rcon(f"tick sprint {ticks}")
    while gametime() < start + ticks:
        time.sleep(0.4)


def desk_of(corner):
    return f"{corner[0] + 7} -60 {corner[1] + 7}"


def area(corner, size):
    return f"{corner[0] - 1} {corner[1] - 1} {corner[0] + size} {corner[1] + size}"


def trades_by_settlement():
    text = LOG.read_text(errors="replace") if LOG.exists() else ""
    totals = {name: 0 for name in SETTLEMENTS}
    for n, x, z in re.findall(r"Locker trades: (\d+) at (-?\d+), -60, (-?\d+)", text):
        for name, corner in SETTLEMENTS.items():
            if corner[0] <= int(x) <= corner[0] + 15 and corner[1] <= int(z) <= corner[1] + 15:
                totals[name] += int(n)
    return totals


def main():
    days = int(sys.argv[1]) if len(sys.argv) > 1 else 6
    hub_desk = f"{HUB[0] + 2} -60 {HUB[1] + 2}"
    rcon(f"forceload add {area(HUB, 6)}", f"execute positioned {HUB[0]} -60 {HUB[1]} run function testbench:network/hub")
    for corner in SETTLEMENTS.values():
        rcon(f"forceload add {area(corner, 16)}",
             f"execute positioned {corner[0]} -60 {corner[1]} run function testbench:network/settlement")
    rcon("time set 1000")
    sprint(300)  # villagers claim their workstations while everything is loaded
    for name, corner in SETTLEMENTS.items():
        rcon(f"enterprise autoswitch {desk_of(corner)} selling")
    for name, corner in SETTLEMENTS.items():
        reply = rcon(f"enterprise route add {hub_desk} {desk_of(corner)}").strip().splitlines()[-1]
        print(f"{name}: {reply}")
    for corner in [HUB, *SETTLEMENTS.values()]:
        rcon(f"forceload remove {area(corner, 16)}")
    print("\nEverything unloaded; no players. Day by day:\n")
    for day in range(1, days + 1):
        sprint(24000)
        print(f"-- day {day}: settlement trades so far {trades_by_settlement()}")
        for line in rcon("enterprise routes").splitlines():
            if line.strip().startswith("#"):
                print("  " + re.sub(r"\s+", " ", line.strip()))
    rcon(f"forceload add {area(HUB, 6)}")
    sprint(40)
    print("\nHub stock at the end:")
    print("\n".join("  " + l.strip() for l in rcon(f"enterprise stock {hub_desk}").splitlines()
                    if "Blaze" in l or "emerald" in l or "bread" in l or "paper" in l))


if __name__ == "__main__":
    main()
