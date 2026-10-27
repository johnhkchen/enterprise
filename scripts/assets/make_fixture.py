#!/usr/bin/env python3
"""The placer's test cottage (`territory/Kits`, `docs/assets/README.md`): a 5 x 5 home written as a vanilla structure
file with its metadata, the shape the studio delivers, so the placer is tested before a delivery lands.

    python3 scripts/assets/make_fixture.py     (writes core/src/main/resources/data/enterprise/structure/test/home/)

Authored as the contract says: front to the south (+z), origin at the north-west corner of the lowest layer, two
foundation layers, the floor on layer 2 (`ground`), two beds, a door in the south wall, a lantern inside.
"""
import gzip
import json
import struct
from pathlib import Path

OUT = Path(__file__).resolve().parents[2] / "core/src/main/resources/data/enterprise/structure/test/home"


class Tag:
    def __init__(self, kind, value):
        self.kind, self.value = kind, value


def byte(v): return Tag(1, v)
def short(v): return Tag(2, v)
def int_(v): return Tag(3, v)
def string(v): return Tag(8, v)
def list_(kind, items): return Tag(9, (kind, items))
def compound(d): return Tag(10, d)


def payload(t):
    k, v = t.kind, t.value
    if k == 1:
        return struct.pack(">b", v)
    if k == 2:
        return struct.pack(">h", v)
    if k == 3:
        return struct.pack(">i", v)
    if k == 8:
        b = v.encode()
        return struct.pack(">H", len(b)) + b
    if k == 9:
        kind, items = v
        return struct.pack(">bi", kind, len(items)) + b"".join(payload(i) for i in items)
    if k == 10:
        out = b""
        for name, child in v.items():
            nb = name.encode()
            out += struct.pack(">bH", child.kind, len(nb)) + nb + payload(child)
        return out + b"\x00"
    raise ValueError(k)


def nbt(root):
    return b"\x0a\x00\x00" + payload(root)


def main():
    blocks = {}  # (x, y, z) -> (name, props)
    for x in range(5):
        for z in range(5):
            for y in (0, 1):
                blocks[x, y, z] = ("minecraft:cobblestone", {})
            blocks[x, 2, z] = ("minecraft:cobblestone", {})
            edge_x, edge_z = x in (0, 4), z in (0, 4)
            for y in (3, 4):
                blocks[x, y, z] = (("minecraft:oak_log", {"axis": "y"}) if edge_x and edge_z
                                   else ("minecraft:oak_planks", {}) if edge_x or edge_z else ("minecraft:air", {}))
            blocks[x, 5, z] = ("minecraft:oak_slab", {"type": "bottom", "waterlogged": "false"})
    door = {"facing": "south", "hinge": "left", "open": "false", "powered": "false"}
    blocks[2, 3, 4] = ("minecraft:oak_door", {**door, "half": "lower"})
    blocks[2, 4, 4] = ("minecraft:oak_door", {**door, "half": "upper"})
    blocks[0, 4, 2] = ("minecraft:glass_pane", {"east": "true", "west": "false", "north": "false", "south": "false",
                                                "waterlogged": "false"})
    blocks[4, 4, 2] = ("minecraft:glass_pane", {"east": "false", "west": "true", "north": "false", "south": "false",
                                                "waterlogged": "false"})
    for x in (1, 3):
        blocks[x, 3, 2] = ("minecraft:white_bed", {"facing": "north", "part": "head", "occupied": "false"})
        blocks[x, 3, 3] = ("minecraft:white_bed", {"facing": "north", "part": "foot", "occupied": "false"})
    blocks[2, 3, 1] = ("minecraft:lantern", {"hanging": "false", "waterlogged": "false"})
    palette, index, entries = [], {}, []
    for pos in sorted(blocks, key=lambda p: (p[1], p[2], p[0])):
        name, props = blocks[pos]
        key = (name, tuple(sorted(props.items())))
        if key not in index:
            index[key] = len(palette)
            entry = {"Name": string(name)}
            if props:
                entry["Properties"] = compound({k: string(v) for k, v in sorted(props.items())})
            palette.append(compound(entry))
        entries.append(compound({"pos": list_(3, [int_(c) for c in pos]), "state": int_(index[key])}))
    root = compound({"DataVersion": int_(3955), "size": list_(3, [int_(5), int_(6), int_(5)]),
                     "palette": list_(10, palette), "blocks": list_(10, entries), "entities": list_(10, [])})
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "cottage_5x5.nbt").write_bytes(gzip.compress(nbt(root), mtime=0))
    meta = {
        "id": "enterprise:test/home/cottage_5x5", "rev": 1, "kingdom": "test", "kind": "home", "slot": "home",
        "tiers": ["hamlet", "village"], "style": "the placer's test cottage", "palette": "base", "variants": [],
        "footprint": [5, 5], "height": 6, "ground": 2, "mirror": True,
        "entrances": [{"at": [2, 3, 4], "facing": "south", "width": 1}],
        "beds": [{"head": [1, 3, 2], "foot": [1, 3, 3]}, {"head": [3, 3, 2], "foot": [3, 3, 3]}],
        "workstations": [], "lockers": [], "markers": [], "pilings": [], "entities": [],
        "notes": "Not an asset: the placer's game test places it (scripts/assets/make_fixture.py)."}
    (OUT / "cottage_5x5.json").write_text(json.dumps(meta, indent=2) + "\n")
    # A palette variant: its own blocks (the same here), its metadata its base's.
    (OUT / "cottage_5x5__plain.nbt").write_bytes((OUT / "cottage_5x5.nbt").read_bytes())
    (OUT / "cottage_5x5__plain.json").write_text(json.dumps({"id": "enterprise:test/home/cottage_5x5__plain",
                                                            "base": "enterprise:test/home/cottage_5x5",
                                                            "palette": "plain", "rev": 1}, indent=2) + "\n")
    print(f"wrote {OUT}")


if __name__ == "__main__":
    main()
