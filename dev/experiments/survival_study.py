#!/usr/bin/env python3
"""Two generated villages of a survival world, made towns as a player would, run day by day under the core.

    just server-setup-survival 8675309   # a normal world of that seed, no testbench (its villages 496 blocks apart;
                                         # 12345's nearest pair are 1,488 apart, a road no load pays for)
    just server                          # in the background
    python3 dev/experiments/survival_study.py DAYS OUT.log [--loaded] [--chests] [--starting N] [--villages N] [--near X,Z] [--roads N]

The nearest village to spawn and the nearest one to it beyond SPREAD blocks (looking round it 300 to 700 blocks out, twelve ways) are found (`locate structure`), each
loaded a minute while its villagers settle, and given what a player gives a village to trade: a Merchant's Desk at
the surface by the village's centre, a chest by it (the core gives the town its founding purse; `--starting N` puts
N emeralds more in it), and a Merchant. The core founds a town at
each desk, a route opens from the first to the second, and the trades the villagers' lockers would buy are switched
on (a locker needs a chest touching its workstation: a generated village has none, so most have no locker, as a
player finds it). Then the days run, the first village kept loaded as a player standing in it would (`--loaded`:
both), the other left to the core. With --chests, the player reads each town's ledger and puts a chest by each
workstation it names (one with no chest touching it), as the ledger asks, before the trades are switched on. Every five days each town's ledger is logged; at the end, where the core's journal
is (`enterprise-sim --trace` it into SQLite).

With --villages N (more than two), a network: the villages round spawn (looked for 1,600 blocks every way), N of them
each at least NEIGHBOUR blocks from the others, the nearest to the first; each a town, with roads to its two nearest
within ROAD_REACH blocks. The first is kept loaded, the rest left to the core.
"""

import math
import re
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "scripts"))
import rcon as rcon_client  # noqa: E402

SPREAD = 400  # the second village at least this far from the first
NEIGHBOUR = 250  # a network's villages at least this far apart (--villages)
# A network's roads: to a town's two nearest within this (`--roads N`; not --reach, the ground kept loaded).
ROAD_REACH = int(sys.argv[sys.argv.index("--roads") + 1]) if "--roads" in sys.argv else 800
STARTING = 0  # emeralds a player puts in each desk's chest, beyond the founding purse the core gives a town (64)
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


def surface(x, z, tries=10):
    """The ground's height at (x, z) (a marker summoned over the surface, read and gone)."""
    rcon(f"execute positioned {x} 0 {z} positioned over world_surface run summon minecraft:marker ~ ~ ~ {{Tags:[\"survey\"]}}")
    said = rcon("data get entity @e[type=minecraft:marker,tag=survey,limit=1] Pos[1]")
    rcon("kill @e[type=minecraft:marker,tag=survey]")
    found = re.search(r"(-?\d+(?:\.\d+)?)d", said)
    if not found:
        if tries > 0:  # its chunks still generating (a wide --reach takes a while)
            sprint(100)
            return surface(x, z, tries - 1)
        raise RuntimeError(f"no surface at {x} {z}: {said}")
    return int(float(found.group(1)))


# How far round each village the study keeps the ground loaded: `--reach N` (64 by default) to take in its farmland
# and farmsteads, which lie past its outer wall.
REACH = int(sys.argv[sys.argv.index("--reach") + 1]) if "--reach" in sys.argv else 64


def quarters(x, z, r):
    """The square round (x, z) as boxes of 256 chunks at most: one forceload takes no more, and refuses more without a
    word (a reach past 120 is split into quarters)."""
    if r <= 120:
        return [(x - r, z - r, x + r, z + r)]
    return [(x - r, z - r, x - 1, z - 1), (x, z - r, x + r, z - 1), (x - r, z, x - 1, z + r), (x, z, x + r, z + r)]


def load(x, z, r=None):
    r = REACH if r is None else r
    for x0, z0, x1, z1 in quarters(x, z, r):
        rcon(f"forceload add {x0} {z0} {x1} {z1}")


def unload(x, z, r=None):
    r = REACH if r is None else r
    for x0, z0, x1, z1 in quarters(x, z, r):
        rcon(f"forceload remove {x0} {z0} {x1} {z1}")


def market(village):
    """A desk, a chest of emeralds by it and a Merchant, at the surface by the village's centre: the desk's x y z."""
    x, z = village
    load(x, z)
    sprint(100)  # the chunks generate and load
    y = surface(x, z)
    rcon(f"setblock {x} {y} {z} enterprise:merchants_desk",
         f"setblock {x + 1} {y} {z} minecraft:chest",
         *(f"item replace block {x + 1} {y} {z} container.{i} with minecraft:emerald {min(64, STARTING - 64 * i)}"
           for i in range((STARTING + 63) // 64)),
         f"summon minecraft:villager {x}.5 {y} {z + 1}.5 {{PersistenceRequired:1b,Xp:1,"
         f"VillagerData:{{profession:\"enterprise:merchant\",level:1,type:\"minecraft:plains\"}}}}")
    return f"{x} {y} {z}"


def replay_check(said):
    """Whether the journal's replay answers every request as the game's core did (the `.fnv` beside it: each response's
    FNV-1a), or the first request it parts at: a replay's figures (`--trace`) are the game's only when they match."""
    import subprocess
    found = re.search(r"journal: (\S+\.ejl)", said)
    if not found:
        return "replay: no journal named"
    journal = Path(found.group(1))
    fnv = journal.with_name(journal.name + ".fnv")
    if not fnv.exists():
        return f"replay: no {fnv.name} beside the journal"
    ran = subprocess.run(["cargo", "run", "--release", "-q", "-p", "enterprise-sim", "--", "--responses", str(journal)],
                         cwd=REPO / "sim", capture_output=True, text=True)
    game, replay = fnv.read_text().split(), ran.stdout.split()
    for i, (a, b) in enumerate(zip(game, replay)):
        if a != b:
            return (f"replay PARTS from the game at request {i} of {len(game)}: keep this world (don't set up another); "
                    "first check sim/core wasn't changed since the server's `just build`")
    if len(game) != len(replay):
        return f"replay: {len(replay)} responses, the game {len(game)}"
    return f"replay: all {len(game)} responses as the game's"


def dist(a, b):
    return math.hypot(a[0] - b[0], a[1] - b[1])


def network(first, many, days, say):
    """N villages, each a town, roads to each one's two nearest within ROAD_REACH: the days run as main's."""
    found = {first}
    for gx in range(-1600, 1601, 400):
        for gz in range(-1600, 1601, 400):
            v = locate(first[0] + gx, first[1] + gz)
            if v:
                found.add(v)
    chosen = [first]
    for v in sorted(found, key=lambda v: dist(v, first)):
        if len(chosen) < many and all(dist(v, c) >= NEIGHBOUR for c in chosen):
            chosen.append(v)
    say(f"{len(found)} villages found; the network's {len(chosen)}: " + ", ".join(f"{v}" for v in chosen))
    desks = [market(v) for v in chosen]
    for d in desks:
        say(f"{d}: " + rcon(f"enterprise core stock {d}").strip().splitlines()[0])
    roads = set()
    for i, v in enumerate(chosen):
        near = sorted((j for j in range(len(chosen)) if j != i and dist(v, chosen[j]) <= ROAD_REACH),
                      key=lambda j: dist(v, chosen[j]))[:2]
        for j in near:
            roads.add((i, j))
    for i, j in sorted(roads):
        said = rcon(f"enterprise route add {desks[i]} {desks[j]}").strip().splitlines()[-1]
        say(f"road {i} -> {j} ({dist(chosen[i], chosen[j]):.0f} blocks): {said}")
    sprint(1200)  # the villagers settle
    if "--chests" in sys.argv:
        for d in desks:
            for x, y, z in re.findall(r"at (-?\d+) (-?\d+) (-?\d+): no chest touching it", rcon(f"enterprise ledger {d}")):
                x, y, z = int(x), int(y), int(z)
                for dx, dz in [(1, 0), (-1, 0), (0, 1), (0, -1)]:
                    if "Changed the block" in rcon(f"setblock {x + dx} {y} {z + dz} minecraft:chest keep"):
                        break
        sprint(200)
    for d in desks:
        say(f"{d}: " + rcon(f"enterprise autoswitch {d} selling").strip().splitlines()[-1])
    if "--loaded" not in sys.argv:
        for v in chosen[1:]:
            unload(*v)
    for day in range(1, days + 1):
        started = time.time()
        if day % 4 == 1 and day > 1:
            for v, d in zip(chosen, desks):
                load(*v, r=8)
                rcon(f"enterprise autoswitch {d} unlocked")
                if v != first and "--loaded" not in sys.argv:
                    unload(*v, r=8)
        sprint(24000)
        say(f"day {day} ({time.time() - started:.0f}s)")
        if day == 2:
            # A road refused at the start (no way over land) may go by ship now that the towns' harbours are laid.
            for i, j in sorted(roads):
                said = rcon(f"enterprise route add {desks[i]} {desks[j]}").strip().splitlines()[-1]
                say(f"day {day}: road {i} -> {j} again: {said}")
        if day % 5 == 0 or day == days:
            for v, d in zip(chosen, desks):
                load(*v, r=8)
                say(f"-- day {day}, {d}\n" + rcon(f"enterprise ledger {d}").strip())
                if v != first and "--loaded" not in sys.argv:
                    unload(*v, r=8)
            # Every road, its carrier ("by ship: ..."), trips and sales; towns founded since show here too.
            say(f"-- day {day}, roads\n" + rcon("enterprise routes").strip())
    said = rcon("enterprise core journal").strip()
    say(said)
    say(replay_check(said))


def main():
    global STARTING
    if "--starting" in sys.argv:
        STARTING = int(sys.argv[sys.argv.index("--starting") + 1])
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
    # `--near X,Z`: the first village the nearest to there, not to spawn (a seed's coast away from spawn).
    near = sys.argv[sys.argv.index("--near") + 1].split(",") if "--near" in sys.argv else (0, 0)
    first = locate(int(near[0]), int(near[1]))
    if first is None:
        sys.exit("no village near spawn: try another seed")
    many = int(sys.argv[sys.argv.index("--villages") + 1]) if "--villages" in sys.argv else 2
    if many > 2:
        network(first, many, days, say)
        return
    # The nearest village at least SPREAD out, looking each way (a player founds a town's neighbour near, not the first
    # one found: the first study's pair were 1,488 blocks apart, a road its wagon's goods never paid for).
    found = set()
    for r in (SPREAD - 100, SPREAD + 100, SPREAD + 300):
        for k in range(12):
            a = 2 * math.pi * k / 12
            v = locate(int(first[0] + r * math.cos(a)), int(first[1] + r * math.sin(a)))
            if v and abs(v[0] - first[0]) + abs(v[1] - first[1]) >= SPREAD:
                found.add(v)
    second = min(found, key=lambda v: abs(v[0] - first[0]) + abs(v[1] - first[1])) if found else None
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
    said = rcon("enterprise core journal").strip()
    say(said)
    say(replay_check(said))


if __name__ == "__main__":
    main()
