#!/usr/bin/env python3
"""One survival study on a lane of its own: a world of SEED in core/run/lanes/N, its server, the study, the server
stopped. The ground comes from a fixture when one was kept for these flags (fixtures.py), else it's made and kept.

    python3 dev/experiments/survival_lane.py N SEED DAYS OUTPREFIX [-- study flags]

The study's flags default to the seed 1717 network (--chests --villages 5 --reach 160 --near -736,208 --roads 1000
--pregen 300). OUTPREFIX.log is the study's log, .out its output, .server.log the server's. The server's watchdog is
off (max-tick-time=-1: a long sprint isn't a hung server).
"""

import os
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(Path(__file__).parent))
import fixtures  # noqa: E402

DEFAULT = ["--chests", "--villages", "5", "--reach", "160", "--near", "-736,208", "--roads", "1000", "--pregen", "300"]


def main():
    if len(sys.argv) < 5:
        sys.exit(__doc__)
    n, seed, days, out = int(sys.argv[1]), sys.argv[2], sys.argv[3], sys.argv[4]
    flags = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else DEFAULT
    lane = REPO / "core" / "run" / "lanes" / str(n)
    lane.mkdir(parents=True, exist_ok=True)
    subprocess.run(["rm", "-rf", str(lane / "world"), str(lane / "config")], check=True)
    subprocess.run(["cp", "-R", str(REPO / "core/run/config"), str(lane / "config")], check=True)
    (lane / "eula.txt").write_text("eula=true\n")
    (lane / "server.properties").write_text("\n".join([
        r"level-type=minecraft\:normal", f"level-seed={seed}", "enable-rcon=true", "rcon.password=dev",
        f"rcon.port={25600 + 2 * n}", "online-mode=false", "spawn-protection=0", "difficulty=normal",
        "server-ip=127.0.0.1", f"server-port={25599 + 2 * n}", "max-tick-time=-1"]) + "\n")
    terrain = fixtures.terrain_dir(seed, flags)
    started = time.time()
    found = fixtures.resume_point(fixtures.run_dir(seed, flags), int(days), print)
    if found:
        point, saves = found
        fixtures.restore_point(point, lane / "world")
        print(f"points: restored {point.relative_to(REPO)} in {time.time() - started:.0f}s (made in {saves:.0f}s)")
    elif fixtures.ready(terrain):
        started = time.time()
        saves = fixtures.restore(terrain, lane / "world")
        print(f"terrain: restored {terrain.relative_to(REPO)} in {time.time() - started:.0f}s (made in {saves}s)")
    else:
        print(f"terrain: none kept for these flags; the study makes {terrain.relative_to(REPO)}")
    started = time.time()
    with open(f"{out}.server.log", "w") as log:
        server = subprocess.Popen(["just", "lane", str(n)], cwd=REPO, stdout=log, stderr=subprocess.STDOUT)
    while "Done (" not in Path(f"{out}.server.log").read_text(errors="replace"):
        if server.poll() is not None:
            sys.exit(f"lane {n}'s server stopped before it was up: {out}.server.log")
        time.sleep(3)
    print(f"server: up in {time.time() - started:.0f}s")
    env = dict(os.environ, ENTERPRISE_LANE=str(n))
    with open(f"{out}.out", "w") as o:
        subprocess.run(["caffeinate", "-dimsu", sys.executable, "-u", "dev/experiments/survival_study.py", days,
                        f"{out}.log", *flags, "--fixtures"], cwd=REPO, env=env, stdout=o, stderr=subprocess.STDOUT)
    subprocess.run([sys.executable, "scripts/rcon.py", "stop"], cwd=REPO, env=env, capture_output=True)
    server.wait()
    for line in Path(f"{out}.log").read_text().splitlines():
        if line.startswith(("stage", "replay", "point", "resumed")):
            print(line)


if __name__ == "__main__":
    main()
