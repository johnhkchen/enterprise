#!/usr/bin/env python3
"""Two generated villages of a survival world, made towns as a player would, run day by day under the core.

    just server-setup-survival 12345     # a normal world of that seed, no testbench
    just server                          # in the background
    python3 dev/experiments/survival_study.py DAYS OUT.log [--loaded] [--chests]

The nearest village to spawn and the nearest one to it beyond SPREAD blocks are found (`locate structure`), each
loaded a minute while its villagers settle, and given what a player gives a village to trade: a Merchant's Desk at
the surface by the village's centre, a chest by it with STARTING emeralds, and a Merchant. The core founds a town at
each desk, a route opens from the first to the second, and the trades the villagers' lockers would buy are switched
on (a locker needs a chest touching its workstation: a generated village has none, so most have no locker, as a
player finds it). Then the days run, the first village kept loaded as a player standing in it would (`--loaded`:
both), the other left to the core. With --chests, the player reads each town's ledger and puts a chest by each
workstation it names (one with no chest touching it), as the ledger asks, before the trades are switched on. Every five days each town's ledger is logged; at the end, where the core's journal
is (`enterprise-sim --trace` it into SQLite).
"""

import re
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "scripts"))
import rcon as rcon_client  # noqa: E402

SPREAD = 400  # the second village at least this far from the first
STARTING = 32  # emeralds in each desk's chest
_sock = None


def rcon(*commands):
    global _sock
    out = []
    for command in commands:
        for attempt in range(2):
            try:
                if _sock is None:
                    import socket
                    _sock = socket.create_connection((rcon_client.HOST, rcon_client.PORT), timeout=120)
                    rcon_client.packet(_sock, 1, 3, rcon_client.PASSWORD)
                    if rcon_client.read(_sock)[0] == -1:
                        sys.exit("RCON login failed")
                out.append(rcon_client.command(_sock, 2, command))
                break
            except (OSError, ConnectionError):
                _sock = None
                if attempt:
                    raise
    return "\n".join(out)


def gametime():
    for _ in range(20):
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


def locate(x, z):
    """The nearest village to (x, z): its (x, z), or None."""
    said = rcon(f"execute positioned {x} 0 {z} run locate structure #minecraft:village")
    found = re.search(r"at \[(-?\d+), [^,]+, (-?\d+)\]", said)
    return (int(found.group(1)), int(found.group(2))) if found else None


def surface(x, z):
    """The ground's height at (x, z) (a marker summoned over the surface, read and gone)."""
    rcon(f"execute positioned {x} 0 {z} positioned over world_surface run summon minecraft:marker ~ ~ ~ {{Tags:[\"survey\"]}}")
    said = rcon("data get entity @e[type=minecraft:marker,tag=survey,limit=1] Pos[1]")
    rcon("kill @e[type=minecraft:marker,tag=survey]")
    found = re.search(r"(-?\d+(?:\.\d+)?)d", said)
    if not found:
        raise RuntimeError(f"no surface at {x} {z}: {said}")
    return int(float(found.group(1)))


def load(x, z, r=64):
    rcon(f"forceload add {x - r} {z - r} {x + r} {z + r}")


def unload(x, z, r=64):
    rcon(f"forceload remove {x - r} {z - r} {x + r} {z + r}")


def market(village):
    """A desk, a chest of emeralds by it and a Merchant, at the surface by the village's centre: the desk's x y z."""
    x, z = village
    load(x, z)
    sprint(100)  # the chunks generate and load
    y = surface(x, z)
    rcon(f"setblock {x} {y} {z} enterprise:merchants_desk",
         f"setblock {x + 1} {y} {z} minecraft:chest",
         f"item replace block {x + 1} {y} {z} container.0 with minecraft:emerald {STARTING}",
         f"summon minecraft:villager {x}.5 {y} {z + 1}.5 {{PersistenceRequired:1b,Xp:1,"
         f"VillagerData:{{profession:\"enterprise:merchant\",level:1,type:\"minecraft:plains\"}}}}")
    return f"{x} {y} {z}"


def main():
    days = int(sys.argv[1])
    out = Path(sys.argv[2])
    log = out.open("w")

    def say(line):
        print(line)
        log.write(line + "\n")
        log.flush()

    if "The core's mint" not in rcon("enterprise mint"):
        sys.exit("the server isn't running the core (ENTERPRISE_SOCIETY=java?)")
    rcon("gamerule doDaylightCycle true", "time set 1000")
    first = locate(0, 0)
    if first is None:
        sys.exit("no village near spawn: try another seed")
    second = None
    for dx, dz in [(1, 0), (0, 1), (-1, 0), (0, -1), (1, 1), (-1, -1), (1, -1), (-1, 1)]:
        v = locate(first[0] + dx * SPREAD, first[1] + dz * SPREAD)
        if v and abs(v[0] - first[0]) + abs(v[1] - first[1]) >= SPREAD:
            second = v
            break
    if second is None:
        sys.exit(f"no second village beyond {SPREAD} blocks of {first}")
    say(f"villages at {first} and {second}, {abs(first[0] - second[0]) + abs(first[1] - second[1])} blocks apart")
    desks = [market(first), market(second)]
    for d in desks:
        say(f"{d}: " + rcon(f"enterprise core stock {d}").strip().splitlines()[0])
    say("route: " + rcon(f"enterprise route add {desks[0]} {desks[1]}").strip().splitlines()[-1])
    sprint(1200)  # the villagers settle: their workstations claimed, the census counts them
    if "--chests" in sys.argv:
        for d in desks:
            for x, y, z in re.findall(r"at (-?\d+) (-?\d+) (-?\d+): no chest touching it", rcon(f"enterprise ledger {d}")):
                x, y, z = int(x), int(y), int(z)
                for dx, dz in [(1, 0), (-1, 0), (0, 1), (0, -1)]:
                    if "Changed the block" in rcon(f"setblock {x + dx} {y} {z + dz} minecraft:chest keep"):
                        say(f"{d}: a chest by the workstation at {x} {y} {z}")
                        break
        sprint(200)  # the lockers placed at the next looks
    for d in desks:
        say(f"{d}: " + rcon(f"enterprise autoswitch {d} selling").strip().splitlines()[-1])
        say(f"{d}: " + rcon(f"enterprise core lockers {d}").strip().splitlines()[0])
    if "--loaded" not in sys.argv:
        unload(*second)
    for day in range(1, days + 1):
        started = time.time()
        if day % 4 == 1 and day > 1:
            for v, d in zip([first, second], desks):
                load(*v, r=8)
                said = rcon(f"enterprise autoswitch {d} unlocked").strip().splitlines()
                if v == second and "--loaded" not in sys.argv:
                    unload(*v, r=8)
                if said and not said[-1].startswith("Switched on 0 "):
                    say(f"day {day}: {d} {said[-1]}")
        sprint(24000)
        say(f"day {day} ({time.time() - started:.0f}s)")
        if day % 5 == 0 or day == days:
            for v, d in zip([first, second], desks):
                load(*v, r=8)
                say(f"-- day {day}, {d}\n" + rcon(f"enterprise ledger {d}").strip())
                if v == second and "--loaded" not in sys.argv:
                    unload(*v, r=8)
    say(rcon("enterprise core journal").strip())


if __name__ == "__main__":
    main()
