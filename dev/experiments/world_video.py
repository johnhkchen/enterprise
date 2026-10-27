#!/usr/bin/env python3
"""A film of a survey map (world_render.py's page): its caravans on the road day by day, for sharing (an MP4 that
Discord plays inline: H.264, yuv420p, under its 10 MB).

    python3 dev/experiments/world_video.py SURVEY.html OUT.mp4 [--width 720] [--seconds-per-day 0.5]

Reads the page's map and data, draws the towns' names onto the map once, and on each frame the day and the caravans
on the road that day, each from when it left to when it was home (out to the far town, then back: squares wagons,
rounds llama trains), over the roads they wore into the land; pipes the frames to ffmpeg. Needs Pillow
(`python3 -m venv v && v/bin/pip install pillow`, run it with v/bin/python) and ffmpeg with libx264.
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


def option(name, default):
    return float(sys.argv[sys.argv.index(name) + 1]) if name in sys.argv else default


def main():
    args = [a for i, a in enumerate(sys.argv[1:], 1)
            if not a.startswith("--") and sys.argv[i - 1] not in ("--width", "--seconds-per-day")]
    width = int(option("--width", 720))
    per_day = option("--seconds-per-day", 0.5)
    page = open(args[0]).read()
    image = base64.b64decode(re.search(r"data:image/png;base64,([A-Za-z0-9+/=]+)", page).group(1))
    data = json.loads(re.search(r"const D = (\{.*?\});\n", page, re.S).group(1))
    full = decode_png(image)
    f = width / full.width
    height = int(full.height * f) // 2 * 2
    base = full.resize((width, height), Image.NEAREST)
    draw = ImageDraw.Draw(base)

    # Each route's length along its road, for placing its caravans.
    for rt in data["routes"]:
        lens = [0.0]
        for (ax, az), (bx, bz) in zip(rt["path"], rt["path"][1:]):
            lens.append(lens[-1] + math.hypot(bx - ax, bz - az))
        rt["lens"], rt["total"] = lens, lens[-1]

    # The towns' names.
    font, small = ImageFont.truetype(FONT, 22), ImageFont.truetype(FONT, 18)
    for t in data["towns"]:
        x, y = t["x"] * f, t["z"] * f
        draw.ellipse((x - 4, y - 4, x + 4, y + 4), fill=INK)
        draw.text((x, y - 12), t["name"], font=font, anchor="ms", fill=INK, stroke_width=3, stroke_fill=PAPER)
    trips = sum(len(r.get("runs", [])) for r in data["routes"])

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
    last = data["day"]
    for k in range(int((last + 1) * per_day * fps)):
        day = k / fps / per_day
        frame = base.copy()
        pen = ImageDraw.Draw(frame)
        pen.text((16, height - 16), f"Seed 8675309 · day {int(day)} of {last} · {trips} caravan trips",
                 font=small, anchor="ls", fill=PAPER, stroke_width=3, stroke_fill=INK)
        for rt in data["routes"]:
            for a, b in rt.get("runs", []):
                if day < a or day > b:
                    continue
                # Out to the far town and home again, the trip's time halved.
                fr = (day - a) / max(1e-6, b - a)
                x, z = along(rt, (fr * 2 if fr <= 0.5 else (1 - fr) * 2) * rt["total"])
                x, z = x * f, z * f
                if rt.get("carrier") == "llama":
                    pen.ellipse((x - 5, z - 5, x + 5, z + 5), fill=WAGON, outline=EDGE, width=2)
                else:
                    pen.rectangle((x - 6, z - 4, x + 6, z + 4), fill=WAGON, outline=EDGE, width=2)
        ff.stdin.write(frame.tobytes())
    ff.stdin.close()
    ff.wait()
    print(f"wrote {args[1]}", file=sys.stderr)


if __name__ == "__main__":
    main()
