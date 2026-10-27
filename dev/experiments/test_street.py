#!/usr/bin/env python3
"""A test street for the asset studio's homes (`docs/assets` thread 0003, scale and footprints): kits laid along a
4-wide lane on a flat world, a villager for every bed, a night, then which beds were slept in.

    python3 dev/experiments/test_street.py OUT.json KIT [KIT ...] [--x 600] [--z 600] [--gap 1] [--depth 7]

Needs the dev server up (`just server-setup` then `just server`: a flat world). The street runs east from (x, z): a
4-wide lane of path, its homes on the north side facing south and on the south side facing north, in turn, each plot
`gap` apart. For each kit: where it was laid, how long it took, its beds, and at the night's middle which were
occupied. Writes OUT.json, and prints a line a kit. The world afterwards is the studio's to render (its region files
and level.dat: `--save DIR` copies them once saved).
"""
import json
import re
import shutil
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
sys.path.insert(0, str(Path(__file__).resolve().parent))
import rcon as rcon_client  # noqa: E402

_sock = None


def rcon(command):
    global _sock
    import socket
    if _sock is None:
        _sock = socket.create_connection((rcon_client.HOST, rcon_client.PORT), timeout=120)
        rcon_client.packet(_sock, 1, 3, rcon_client.PASSWORD)
        if rcon_client.read(_sock)[0] == -1:
            sys.exit("RCON login failed")
    return rcon_client.command(_sock, 7, command)


def option(name, default):
    return type(default)(sys.argv[sys.argv.index(name) + 1]) if name in sys.argv else default


def main():
    flags = {"--x", "--z", "--gap", "--depth", "--save"}
    args = [a for i, a in enumerate(sys.argv[1:], 1) if not a.startswith("--") and sys.argv[i - 1] not in flags]
    out, kits = Path(args[0]), args[1:]
    x0, z0, gap, depth = option("--x", 600), option("--z", 600), option("--gap", 1), option("--depth", 7)
    length = 0
    widths = []
    for kit in kits:
        rel = Path(kit.split(":")[0]) / "structure" / (kit.split(":")[1] + ".json")
        # The mod's own, or a delivery being tried as a world data pack (`core/run/world/datapacks/*/data`).
        meta = next((p / rel for p in [Path("core/src/main/resources/data"), *Path("core/run/world/datapacks").glob("*/data")]
                     if (p / rel).exists()), Path("core/src/main/resources/data") / rel)
        m = json.loads(meta.read_text())
        widths.append((kit, m["footprint"][0], m["footprint"][1], m))
        length += m["footprint"][0] + gap
    span = (length // 2 + 16)
    rcon(f"forceload add {x0 - 16} {z0 - depth - 16} {x0 + span} {z0 + depth + 20}")
    time.sleep(6)
    # The flat world's ground: its top block where the street runs.
    rcon(f"execute positioned {x0} 320 {z0} positioned over motion_blocking run summon minecraft:marker ~ ~ ~ "
         "{Tags:[\"street\"]}")
    said = rcon("data get entity @e[type=minecraft:marker,tag=street,limit=1] Pos[1]")
    rcon("kill @e[type=minecraft:marker,tag=street]")
    ground = int(float(re.search(r"(-?\d+(?:\.\d+)?)d", said).group(1))) - 1
    # The lane: 4 wide, east from (x0, z0) to (x0, z0 + 3).
    rcon(f"fill {x0 - 2} {ground} {z0} {x0 + span} {ground} {z0 + 3} minecraft:dirt_path")
    results = []
    at = {"north": x0, "south": x0}
    for i, (kit, w, d, m) in enumerate(widths):
        side = "north" if i % 2 == 0 else "south"
        x = at[side]
        at[side] += w + gap
        # North side: the plot's south edge a doorstep from the lane, facing south; south side facing north.
        corner_z = z0 - 2 - d + 1 if side == "north" else z0 + 3 + 2
        facing = "south" if side == "north" else "north"
        said = rcon(f"enterprise kit {kit} {x} {ground} {corner_z} {facing}")
        found = re.search(r"in (\d+) us; beds \[(.*)\]", said)
        beds = [tuple(int(v) for v in b.split(", ")) for b in re.findall(r"(-?\d+, -?\d+, -?\d+)", found.group(2))] \
            if found else []
        results.append({"kit": kit, "corner": [x, ground, corner_z], "facing": facing, "micros": int(found.group(1))
                        if found else None, "beds": beds, "laid": bool(found), "said": said.strip()[:300]})
        for bx, by, bz in beds:
            rcon(f"summon minecraft:villager {bx + 0.5} {by + 1} {bz + 0.5}")
    # A night: villagers claim beds by day and sleep in them at night.
    rcon("time set 6000")
    time.sleep(20)
    rcon("time set 18000")
    # Looked at twice through the night: a bed slept in at either look counts (villagers find their beds in their own
    # time; the first look alone missed a third of the beds a minute later full).
    for r in results:
        r["slept"] = [False] * len(r["beds"])
    for look in (25, 35):
        time.sleep(look)
        for r in results:
            for i, (bx, by, bz) in enumerate(r["beds"]):
                hit = rcon(f"execute if block {bx} {by} {bz} #minecraft:beds[occupied=true]")
                r["slept"][i] |= "passed" in hit.lower()
        print(f"{r['kit']}: {'laid' if r['laid'] else 'NOT laid'} facing {r['facing']} in {r['micros']} us, "
              f"{sum(r['slept'])} of {len(r['beds'])} beds slept in")
    out.write_text(json.dumps({"street": {"x": x0, "z": z0, "ground": ground, "gap": gap}, "kits": results}, indent=1))
    save = option("--save", "")
    if save:
        rcon("save-all")
        time.sleep(5)
        dest = Path(save)
        dest.mkdir(parents=True, exist_ok=True)
        world = Path("core/run/world")
        shutil.copy2(world / "level.dat", dest / "level.dat")
        for rf in (world / "region").glob("r.*.mca"):
            shutil.copy2(rf, dest / rf.name)
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
