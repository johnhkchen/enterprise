#!/usr/bin/env python3
"""How fast does a stocked Merchant trade through its goods, level up, and take on every villager?

Drives the headless dev server over RCON (start it with `just server-setup` then `just server`):
builds the testbench market at 0 -60 0, stocks the Merchant with 18 shulkers of trade goods,
switches on every goods-for-emerald trade it can see, then sprints one in-game day at a time and
logs levels, managed lockers, emeralds earned and goods left. New emerald trades that appear as
villagers level up are switched on each morning.

    python3 dev/experiments/merchant_ramp.py [max_days]
"""

import re
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
RCON = [sys.executable, str(REPO / "scripts" / "rcon.py")]
SERVER_LOG = REPO / "core" / "run" / "logs" / "debug.log"  # locker trades log at debug level
DESK = "7 -60 7"
WORKERS = ["Librarian", "Cartographer", "Fletcher", "Farmer"]
GOODS = ["paper", "stick", "wheat", "potato", "carrot", "beetroot", "pumpkin", "melon", "flint",
         "glass_pane", "book", "string", "feather", "ink_sac", "compass"]


def rcon(*commands):
    return subprocess.run(RCON + list(commands), capture_output=True, text=True, check=True).stdout


def value(command, pattern, default=None):
    match = re.search(pattern, rcon(command))
    return int(match.group(1)) if match else default


def gametime():
    return value("time query gametime", r"time is (\d+)")


def sprint_day():
    start = gametime()
    rcon("tick sprint 1d")
    while gametime() < start + 24000:
        time.sleep(0.5)


def villager(name):
    level = value(f"data get entity @e[type=villager,name={name},limit=1] VillagerData.level", r": (\d+)", 0)
    xp = value(f"data get entity @e[type=villager,name={name},limit=1] Xp", r": (\d+)", 0)
    return level, xp


def stock_counts():
    """Item totals in the Merchant's stock, shulker contents included (one line per item, so no truncation)."""
    return {item: int(count) for item, count in re.findall(r"minecraft:([a-z_]+) (\d+)", rcon(f"enterprise stock {DESK}"))}


def locker_trades_logged():
    return sum(int(n) for n in re.findall(r"Locker trades: (\d+)", SERVER_LOG.read_text(errors="replace")))


def switch_on_emerald_trades():
    listing = rcon(f"enterprise lockers {DESK}")
    managed = re.search(r"(\d+)/(\d+) locker", listing)
    switched = 0
    workstation = None
    for line in listing.splitlines():
        site = re.match(r"\s+\w+ at (-?\d+), (-?\d+), (-?\d+),", line)
        if site:
            workstation = " ".join(site.groups())
        trade = re.match(r"\s+\[off\] (\S+>minecraft:emerald)$", line)
        if trade and workstation:
            rcon(f'enterprise switch {DESK} {workstation} "{trade.group(1)}" true')
            switched += 1
    return (int(managed.group(1)), int(managed.group(2))) if managed else (0, 0), switched


def main():
    max_days = int(sys.argv[1]) if len(sys.argv) > 1 else 30
    rcon("forceload add 0 0 16 16", "execute positioned 0 -60 0 run function testbench:market/build",
         "time set 1000", "tick sprint 200")
    time.sleep(3)
    rcon("execute positioned 0 -60 0 run function testbench:market/stock_shulkers")
    start = stock_counts()
    goods_start = sum(start.get(g, 0) for g in GOODS)
    print(f"stocked: {goods_start} goods in {len(start)} kinds\n")
    print("day | Merchant lvl (xp) | lockers | " + " | ".join(f"{w[:5]:>9}" for w in WORKERS)
          + " | trades | emeralds | goods left | trades on")
    trades_before = locker_trades_logged()
    idle_days = 0
    last_emeralds = 0
    for day in range(1, max_days + 1):
        (managed, limit), switched = switch_on_emerald_trades()
        sprint_day()
        m_level, m_xp = villager("Merchant")
        workers = [villager(w) for w in WORKERS]
        counts = stock_counts()
        emeralds = counts.get("emerald", 0)
        goods = sum(counts.get(g, 0) for g in GOODS)
        trades_now = locker_trades_logged()
        print(f"{day:>3} | {m_level} ({m_xp:>4})        | {managed}/{limit:<5} | "
              + " | ".join(f"L{lvl} ({xp:>4})" for lvl, xp in workers)
              + f" | {trades_now - trades_before:>6} | {emeralds:>8} | {goods:>10} | +{switched}")
        trades_before = trades_now
        sys.stdout.flush()
        idle_days = idle_days + 1 if emeralds == last_emeralds else 0
        last_emeralds = emeralds
        if idle_days >= 3:
            print("\nNo trades for 3 days: stopping.")
            break


if __name__ == "__main__":
    main()
