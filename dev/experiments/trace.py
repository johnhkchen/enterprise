#!/usr/bin/env python3
"""Follows one town through a running lane: every few in-game days, its farm plan (crops worked, plots idle, crops
sold forward), mood, and the contracts and rents it's party to, with each one's latest history. Read-only: it
loads nothing, so the run goes on as it would.

    python3 dev/experiments/trace.py LANE TOWN [--every DAYS] [--out FILE]

TOWN is a ridge study key (S1..S4, T, E1..E3) or name (Cloverby). Runs until the lane's server stops; start it once
the lane is up (forks.py prints "server up"). Output goes to FILE (default: stdout), a block per poll.
"""

import os
import re
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
CORNERS = {"S1": (1295, 995), "S2": (995, 1595), "S3": (1844, 1844), "S4": (4995, 995), "T": (1393, 1396),
           "E1": (1650, 1350), "E2": (1300, 1750), "E3": (2050, 1300)}
NAMES = {"S1": "Meadowworth", "S2": "Wheatfield", "S3": "Hollowwick", "S4": "Greenwick", "T": "Haygate",
         "E1": "Barleyby", "E2": "Cloverby", "E3": "Lindenfield"}
KEEP = re.compile(r"farms |idle |sold forward|mood |^  #|^  rent |^    day ")


def rcon(lane, *commands):
    env = {**os.environ, "ENTERPRISE_LANE": str(lane)}
    done = subprocess.run([sys.executable, str(REPO / "scripts/rcon.py"), *commands], env=env, capture_output=True,
                          text=True)
    return done.stdout if done.returncode == 0 else None


def day(lane):
    """In-game days since the world began (game time: the study resets the time of day each day)."""
    out = rcon(lane, "time query gametime")
    found = re.search(r"time is (\d+)", out or "")
    return int(found.group(1)) // 24000 if found else None


def snapshot(lane, key):
    x, z = CORNERS[key]
    bell, desk = f"{x + 7} -60 {z + 4}", f"{x + 7} -60 {z + 7}"
    out = rcon(lane, f"enterprise territory {bell}", f"enterprise contracts {desk}")
    if out is None:
        return None
    lines, history = [], 0
    for line in out.splitlines():
        if not KEEP.search(line):
            continue
        if line.startswith("    day "):
            history += 1
            if history > 3:
                continue  # the latest three of each contract's record
        else:
            history = 0
        lines.append(line.rstrip())
    return lines


def main():
    args = sys.argv[1:]
    every, out = 5, None
    if "--every" in args:
        i = args.index("--every")
        every = int(args[i + 1])
        args = args[:i] + args[i + 2:]
    if "--out" in args:
        i = args.index("--out")
        out = open(args[i + 1], "a")
        args = args[:i] + args[i + 2:]
    if len(args) != 2:
        sys.exit(__doc__)
    lane, town = int(args[0]), args[1]
    key = town if town in CORNERS else next((k for k, n in NAMES.items() if n.lower() == town.lower()), None)
    if key is None:
        sys.exit(f"no town {town}: one of {', '.join(f'{k} {n}' for k, n in NAMES.items())}")
    last = None
    while True:
        now = day(lane)
        if now is None:
            break  # the lane's server has stopped
        if last is None or now >= last + every:
            last = now
            lines = snapshot(lane, key)
            if lines is None:
                break
            print(f"=== {NAMES[key]}, game day {now}", file=out or sys.stdout, flush=True)
            print("\n".join(lines), file=out or sys.stdout, flush=True)
        time.sleep(5)


if __name__ == "__main__":
    main()
