#!/usr/bin/env python3
"""Each town's farmland as a mask, from a stopped server's world (`enterprise_field_maps.dat`, the bocage `Bocage` lays):

    python3 dev/experiments/fields_mask.py WORLD OUT.json [TOWN ...]

Writes, per town, the ground its farmland was looked for over (`x0`, `z0`, `sx`, `sz`), each block's field (`field`: row
by row from the north-west, -1 for none, run-length coded as [value, count, ...]) and each field's use, crops, relief
and area, so a census (the studio's `py/field_census.py`) can count field cells exactly rather than guess them from the
blocks. Pure Python, as `world_render.py` (whose reader it uses).
"""
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import world_render  # noqa: E402


def runs(values):
    out = []
    for v in values:
        if out and out[-2] == v:
            out[-1] += 1
        else:
            out += [v, 1]
    return out


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    world, out = Path(sys.argv[1]), Path(sys.argv[2])
    only = set(sys.argv[3:])
    data = world_render.saved(world / "data" / "enterprise_field_maps.dat")
    data = data.get("data", data)
    towns = {}
    for town, m in sorted(data["towns"].items()):
        if only and town not in only:
            continue
        land = m.get("land")
        if land is None or len(land) < 4:
            continue
        x0, z0, sx, sz = land[0], land[1], land[2], land[3]
        fields = []
        for f in m.get("fields", []):
            at = f["at"]
            use, _, crops = f["state"].partition(":")
            fields.append({"use": use, "crops": crops.split(",") if crops else [], "bounds": at[3:7],
                           "relief": at[7] if len(at) > 7 else None, "area": at[8] if len(at) > 8 else None})
        towns[town] = {"x0": x0, "z0": z0, "sx": sx, "sz": sz, "field": runs(list(m["landField"])), "fields": fields}
    out.write_text(json.dumps({"towns": towns}))
    for town, t in towns.items():
        print(f"town {town}: {t['sx']} x {t['sz']} from {t['x0']}, {t['z0']}; {len(t['fields'])} fields")


if __name__ == "__main__":
    main()
