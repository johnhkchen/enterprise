#!/usr/bin/env python3
"""A town founded on open ground by hand, as a player would found one (a desk where no village is), on a running server:

    python3 dev/experiments/found_town.py X Z [--people N]

Loads the ground round (X, Z), sets a bell, a Merchant's Desk and its chest at the surface there, and has the core found
a town at the desk (`enterprise core stock`). What follows is the mod's, as for settlers' towns: its plan drawn for the
town it'll grow to, its land shaped, its first homes and workshops in its kingdom's style along one main street (the
studio's founded towns, thread 0012). For a study: a founded town in minutes, not the 60 days settlers take.
`--people N` brings N villagers to it (none by default), so it outgrows its first beds and its streets open.
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import survival_study as study  # noqa: E402


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    x, z = int(sys.argv[1]), int(sys.argv[2])
    study.load(x, z, 24)
    study.sprint(100)  # the chunks generate and load
    y = study.surface(x, z)
    said = study.rcon(f"setblock {x} {y} {z} minecraft:bell[attachment=floor]",
                      f"setblock {x + 3} {y} {z} enterprise:merchants_desk",
                      f"setblock {x + 4} {y} {z} minecraft:chest",
                      f"enterprise core stock {x + 3} {y} {z}")
    print(said.strip().splitlines()[-1])
    people = int(sys.argv[sys.argv.index("--people") + 1]) if "--people" in sys.argv else 0
    for k in range(people):
        study.rcon(f"summon minecraft:villager {x + 1 + k % 3}.5 {y + 1} {z + 2 + k // 3}.5 {{PersistenceRequired:1b}}")
    if people:
        print(f"{people} villagers brought to it")


if __name__ == "__main__":
    main()
