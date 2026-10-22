#!/usr/bin/env python3
"""How fast does a stocked Merchant trade through its goods, level up, and take on every villager?

Drives the headless dev server over RCON (start it with `just server-setup` then `just server`):
builds the testbench market at 0 -60 0, stocks the Merchant with 18 shulkers of trade goods,
switches on every goods-for-emerald trade it can see, then sprints one in-game day at a time and
logs levels, managed lockers, emeralds earned and goods left. New emerald trades that appear as
villagers level up are switched on each morning.

    python3 dev/experiments/merchant_ramp.py [max_days] [--all]

--all switches on every trade, not just goods-for-emerald ones, so the Merchant also spends the emeralds it
earns (bookshelves, arrows, ...). Villager trades are rolled at random; without --all, a villager whose novice
trades all cost emeralds never levels, which starves the Merchant.

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


COIN_SPURS = {"spur": 1, "bevel": 8, "sprocket": 16, "cog": 64, "crown": 512, "sun": 4096}


def coin_spurs():
    """Value of the Numismatics coins in the Merchant's stock, in spurs."""
    listing = rcon(f"enterprise stock {DESK}")
    return sum(COIN_SPURS[coin] * int(n) for coin, n in re.findall(r"numismatics:([a-z]+) (\d+)", listing))


def mint_rate():
    match = re.search(r"([\d.]+) spurs per emerald", rcon("enterprise mint"))
    return float(match.group(1)) if match else 0.0


def stock_counts():
    """Item totals in the Merchant's stock, shulker contents included (one line per item, so no truncation)."""
    return {item: int(count) for item, count in re.findall(r"minecraft:([a-z_]+) (\d+)", rcon(f"enterprise stock {DESK}"))}


def locker_trades_logged():
    return sum(int(n) for n in re.findall(r"Locker trades: (\d+)", SERVER_LOG.read_text(errors="replace")))


ALL_TRADES = "--all" in sys.argv


def switch_on_emerald_trades():
    listing = rcon(f"enterprise lockers {DESK}")
    managed = re.search(r"(\d+)/(\d+) locker", listing)
    switched = 0
    workstation = None
    for line in listing.splitlines():
        site = re.match(r"\s+\w+ at (-?\d+), (-?\d+), (-?\d+),", line)
        if site:
            workstation = " ".join(site.groups())
        trade = re.match(r"\s+\[off\] (\S+>minecraft:[a-z_]+)$" if ALL_TRADES else r"\s+\[off\] (\S+>minecraft:emerald)$", line)
        if trade and workstation:
            rcon(f'enterprise switch {DESK} {workstation} "{trade.group(1)}" true')
            switched += 1
    return (int(managed.group(1)), int(managed.group(2))) if managed else (0, 0), switched


def main():
    study_mode.java_only('merchant_ramp.py')
    numbers = [arg for arg in sys.argv[1:] if arg.isdigit()]
    max_days = int(numbers[0]) if numbers else 30
    rcon("forceload add 0 0 16 16", "execute positioned 0 -60 0 run function testbench:market/build",
         "time set 1000", "tick sprint 200")
    time.sleep(3)
    rcon("execute positioned 0 -60 0 run function testbench:market/stock_shulkers")
    start = stock_counts()
    goods_start = sum(start.get(g, 0) for g in GOODS)
    print(f"stocked: {goods_start} goods in {len(start)} kinds\n")
    print("day | Merchant lvl (xp) | lockers | " + " | ".join(f"{w[:5]:>9}" for w in WORKERS)
          + " | trades | emeralds | coins (spurs) | rate | goods left | trades on")
    trades_before = locker_trades_logged()
    idle_days = 0
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
              + f" | {trades_now - trades_before:>6} | {emeralds:>8} | {coin_spurs():>13} | {mint_rate():>5.2f}"
              + f" | {goods:>10} | +{switched}")
        idle_days = idle_days + 1 if trades_now == trades_before else 0
        trades_before = trades_now
        sys.stdout.flush()
        if idle_days >= 3:
            print("\nNo trades for 3 days: stopping.")
            break


if __name__ == "__main__":
    main()
