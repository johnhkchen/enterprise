#!/usr/bin/env python3
"""Draws the crate and documents textures (16x16 PNGs) into the core mod's assets. Pure Python, no imaging library.

    python3 dev/assets/cargo_textures.py
"""
import struct
import zlib
from pathlib import Path

OUT = Path(__file__).resolve().parents[2] / "core/src/main/resources/assets/enterprise/textures"


def png(pixels):
    h, w = len(pixels), len(pixels[0])
    raw = b"".join(b"\x00" + b"".join(struct.pack("4B", *p) for p in row) for row in pixels)

    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)

    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))


PLANK = [(166, 128, 82, 255), (156, 119, 75, 255), (176, 137, 89, 255)]
FRAME = (98, 72, 42, 255)
NAIL = (60, 58, 56, 255)
GAP = (120, 90, 56, 255)


def crate(top):
    px = [[PLANK[(y // 4 + x // 7) % 3] for x in range(16)] for y in range(16)]
    for y in range(16):
        if y % 4 == 3:
            for x in range(16):
                px[y][x] = GAP  # seams between the boards
    for i in range(16):
        for t in (0, 1):
            px[t][i] = px[15 - t][i] = px[i][t] = px[i][15 - t] = FRAME
    for i in range(2, 14):  # a brace across: X on top, a diagonal on the sides
        px[i][i] = FRAME
        if top:
            px[i][15 - i] = FRAME
    for x, y in ((1, 1), (14, 1), (1, 14), (14, 14)):
        px[y][x] = NAIL
    return px


def documents():
    clear = (0, 0, 0, 0)
    paper, shade, line, twine = (236, 226, 200, 255), (206, 192, 160, 255), (120, 110, 96, 255), (150, 96, 52, 255)
    px = [[clear] * 16 for _ in range(16)]
    for sheet, (ox, oy) in enumerate(((4, 2), (2, 3), (3, 4))):
        for y in range(oy, oy + 10):
            for x in range(ox, ox + 10):
                px[y][x] = shade if sheet < 2 else paper
    for y in (7, 9, 11):
        for x in range(5, 11):
            px[y][x] = line
    for y in range(3, 15):  # twine round the bundle
        px[y][8] = twine
    for x in range(2, 14):
        px[9][x] = twine
    return px


(OUT / "block").mkdir(parents=True, exist_ok=True)
(OUT / "item").mkdir(parents=True, exist_ok=True)
(OUT / "block/crate_side.png").write_bytes(png(crate(False)))
(OUT / "block/crate_top.png").write_bytes(png(crate(True)))
(OUT / "item/documents.png").write_bytes(png(documents()))
print("wrote crate_side, crate_top, documents")
