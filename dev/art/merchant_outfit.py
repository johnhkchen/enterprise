#!/usr/bin/env python3
"""Paints the Merchant's profession overlay: a wandering-trader style travelling cloak.

Writes the 64x64 overlay for villagers and zombie villagers. UV regions follow
net.minecraft.client.model.VillagerModel (box at (u,v) of size w,h,d: front face at
(u+d, v+d), w x h; sides d wide either side; back after the left side; top at (u+d, v)).

    python3 dev/art/merchant_outfit.py
"""

import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "core/src/main/resources/assets/enterprise/textures/entity"

CLOAK = (68, 103, 155, 255)       # b28 steel blue
CLOAK_SHADE = (52, 80, 122, 255)
CLOAK_DEEP = (38, 58, 90, 255)
TRIM = (242, 239, 233, 255)       # b28 cream
STRAP = (122, 82, 48, 255)
BUCKLE = (201, 162, 82, 255)

px = [[(0, 0, 0, 0)] * 64 for _ in range(64)]


def rect(x0, y0, x1, y1, color):
    for y in range(y0, y1):
        for x in range(x0, x1):
            px[y][x] = color


def box(u, v, w, h, d, front, side, top, back=None):
    """Paint every face of a model box."""
    rect(u + d, v, u + d + w, v + d, top)                       # top
    rect(u + d + w, v, u + d + 2 * w, v + d, top)               # bottom
    rect(u, v + d, u + d, v + d + h, side)                      # right side
    rect(u + d, v + d, u + d + w, v + d + h, front)             # front
    rect(u + d + w, v + d, u + 2 * d + w, v + d + h, side)      # left side
    rect(u + 2 * d + w, v + d, u + 2 * d + 2 * w, v + d + h, back or front)  # back


# Cloak over the robe (jacket: texOffs 0,38; 8x20x6).
box(0, 38, 8, 20, 6, front=CLOAK, side=CLOAK_SHADE, top=CLOAK_SHADE, back=CLOAK_SHADE)
rect(9, 44, 11, 64, TRIM)          # open front edge
rect(0, 62, 28, 64, TRIM)          # hem, all the way round
# Pack strap across the chest, shoulder to hip, with a buckle.
for i in range(8):
    rect(6 + i, 45 + i, 7 + i, 47 + i, STRAP)
rect(10, 49, 12, 51, BUCKLE)

# Sleeves (arms: texOffs 44,22; 4x8x4 each). Hands (texOffs 40,38) stay bare.
box(44, 22, 4, 8, 4, front=CLOAK, side=CLOAK_SHADE, top=CLOAK_SHADE)
rect(48, 32, 52, 34, TRIM)         # cuffs

# Hood (hat layer: texOffs 32,0; 8x10x8). The face opening stays clear.
box(32, 0, 8, 10, 8, front=(0, 0, 0, 0), side=CLOAK_SHADE, top=CLOAK, back=CLOAK_DEEP)
rect(40, 8, 48, 10, CLOAK)         # brow
rect(40, 10, 41, 18, CLOAK)        # cheek edges
rect(47, 10, 48, 18, CLOAK)
rect(40, 8, 48, 9, TRIM)           # trim along the hood's edge
rect(48, 0, 56, 8, (0, 0, 0, 0))   # no underside: it would show as a plate under the chin


def png(pixels):
    raw = b"".join(b"\x00" + b"".join(struct.pack("4B", *p) for p in row) for row in pixels)

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)

    header = struct.pack(">IIBBBBB", 64, 64, 8, 6, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")


# "full": the hood replaces the biome hat (desert headwrap, taiga cap, ...).
META = '{\n  "villager": {\n    "hat": "full"\n  }\n}\n'
for kind in ("villager", "zombie_villager"):
    target = OUT / kind / "profession" / "merchant.png"
    target.write_bytes(png(px))
    target.with_name("merchant.png.mcmeta").write_text(META)
    print(f"wrote {target.relative_to(ROOT)}")
