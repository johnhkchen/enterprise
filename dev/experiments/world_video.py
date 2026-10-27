#!/usr/bin/env python3
"""A short video of a survey map (world_render.py's page): its wagons running their routes, for sharing (an MP4 that
Discord plays inline: H.264, yuv420p, under its 10 MB).

    python3 dev/experiments/world_video.py SURVEY.html OUT.mp4 [--width 720] [--seconds 12]

Reads the page's map and data, draws the routes and the towns' names onto the map once, the wagons on each frame, and
pipes the frames to ffmpeg. Needs Pillow (`python3 -m venv v && v/bin/pip install pillow`, run it with v/bin/python)
and ffmpeg with libx264.
"""
import base64
import json
import math
import re
import struct
import subprocess
import sys
import zlib

from PIL import Image, ImageDraw, ImageFont

FONT = "/System/Library/Fonts/Supplemental/Georgia Bold.ttf"
ROUTE = (181, 86, 43)
WAGON = (232, 120, 64)
EDGE = (250, 246, 238)
INK = (29, 35, 43)
PAPER = (247, 248, 249)


def decode_png(data):
    """Our own PNG writer's output (8-bit RGB, every row filter 0) as an image."""
    w, h = struct.unpack(">II", data[16:24])
    raw = b""
    i = 8
    while i < len(data):
        n = struct.unpack(">I", data[i:i + 4])[0]
        if data[i + 4:i + 8] == b"IDAT":
            raw += data[i + 8:i + 8 + n]
        i += 12 + n
    raw = zlib.decompress(raw)
    pixels = b"".join(raw[y * (w * 3 + 1) + 1:(y + 1) * (w * 3 + 1)] for y in range(h))
    return Image.frombytes("RGB", (w, h), pixels)


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--") and not a.replace(".", "").isdigit()]
    width = int(sys.argv[sys.argv.index("--width") + 1]) if "--width" in sys.argv else 720
    seconds = float(sys.argv[sys.argv.index("--seconds") + 1]) if "--seconds" in sys.argv else 12
    page = open(args[0]).read()
    image = base64.b64decode(re.search(r"data:image/png;base64,([A-Za-z0-9+/=]+)", page).group(1))
    data = json.loads(re.search(r"const D = (\{.*?\});\n", page, re.S).group(1))
    full = decode_png(image)
    f = width / full.width
    height = int(full.height * f) // 2 * 2
    base = full.resize((width, height), Image.NEAREST)
    draw = ImageDraw.Draw(base)

    # The routes, drawn once: as wide as their trips.
    for rt in data["routes"]:
        if not rt["trips"]:
            continue
        draw.line([(x * f, z * f) for x, z in rt["path"]], fill=ROUTE, width=max(2, int(math.sqrt(rt["trips"]))),
                  joint="curve")
        lens = [0.0]
        for (ax, az), (bx, bz) in zip(rt["path"], rt["path"][1:]):
            lens.append(lens[-1] + math.hypot(bx - ax, bz - az))
        rt["lens"], rt["total"] = lens, lens[-1]
        rt["wagons"] = max(1, round(math.sqrt(rt["trips"]) * 1.4))

    # The towns' names, and the title.
    font, small = ImageFont.truetype(FONT, 22), ImageFont.truetype(FONT, 18)
    for t in data["towns"]:
        x, y = t["x"] * f, t["z"] * f
        draw.ellipse((x - 4, y - 4, x + 4, y + 4), fill=INK)
        draw.text((x, y - 12), t["name"], font=font, anchor="ms", fill=INK, stroke_width=3, stroke_fill=PAPER)
    trips = sum(r["trips"] for r in data["routes"])
    draw.text((16, height - 16), f"Seed 8675309 · day {data['day']} · {trips} caravan trips", font=small,
              anchor="ls", fill=PAPER, stroke_width=3, stroke_fill=INK)

    def along(rt, d):
        i = 1
        while i < len(rt["lens"]) - 1 and rt["lens"][i] < d:
            i += 1
        (ax, az), (bx, bz) = rt["path"][i - 1], rt["path"][i]
        seg = rt["lens"][i] - rt["lens"][i - 1] or 1
        t = (d - rt["lens"][i - 1]) / seg
        return ax + (bx - ax) * t, az + (bz - az) * t

    fps = 30
    ff = subprocess.Popen(["ffmpeg", "-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgb24",
                           "-s", f"{width}x{height}", "-r", str(fps), "-i", "-", "-c:v", "libx264", "-preset", "slow",
                           "-crf", "24", "-pix_fmt", "yuv420p", "-movflags", "+faststart", args[1]],
                          stdin=subprocess.PIPE)
    for k in range(int(seconds * fps)):
        frame = base.copy()
        pen = ImageDraw.Draw(frame)
        t = k / fps
        for ri, rt in enumerate(data["routes"]):
            if not rt["trips"]:
                continue
            for j in range(rt["wagons"]):
                # Out and home again, 60 blocks a second, each wagon its own share of the round.
                rnd = 2 * rt["total"]
                d = (t * 60 + (j / rt["wagons"]) * rnd + ri * 37) % rnd
                x, z = along(rt, d if d <= rt["total"] else rnd - d)
                pen.rectangle((x * f - 5, z * f - 4, x * f + 5, z * f + 4), fill=WAGON, outline=EDGE, width=2)
        ff.stdin.write(frame.tobytes())
    ff.stdin.close()
    ff.wait()
    print(f"wrote {args[1]}", file=sys.stderr)


if __name__ == "__main__":
    main()
