#!/usr/bin/env python3
"""Runs the same study fork on several lanes side by side: one run is noise (moods swing a point between runs of the
same code), three show the spread. Each lane gets its own server (`just lane-restore`, `just lane`), restored from the
checkpoint, and the study resumes there.

    python3 dev/experiments/forks.py CHECKPOINT LABEL LANES OUTDIR -- DAYS [study flags...]

e.g. `forks.py core/run/checkpoints/era/day0 potential 3 /tmp/out -- 120 --comforts --war 366,396,436 --ridge --realms
--save-at 120`. Lane N writes OUTDIR/LABEL-N.csv and .out (the study's report) and saves its checkpoints under
LABEL-N; its server's log is OUTDIR/LABEL-N.server. Each lane stops its server when its study ends.
"""

import os
import socket
import subprocess
import sys
import threading
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]


def up(lane):
    try:
        socket.create_connection(("127.0.0.1", 25600 + 2 * lane), timeout=2).close()
        return True
    except OSError:
        return False


def run_lane(lane, checkpoint, label, out, study, delay):
    env = {**os.environ, "ENTERPRISE_LANE": str(lane)}
    name = f"{label}-{lane}"
    time.sleep(delay)  # Gradle's start-up, one lane at a time
    subprocess.run(["just", "lane-restore", str(lane), checkpoint], cwd=REPO, check=True, capture_output=True)
    with open(out / f"{name}.server", "w") as log:
        server = subprocess.Popen(["./gradlew", ":core:runServer", f"-Plane={lane}"], cwd=REPO, stdout=log,
                                  stderr=subprocess.STDOUT)
    started = time.time()
    while not up(lane):
        if server.poll() is not None or time.time() - started > 600:
            print(f"lane {lane}: the server didn't come up (see {out / (name + '.server')})", flush=True)
            server.kill()
            return
        time.sleep(3)
    print(f"lane {lane}: server up after {time.time() - started:.0f}s; study {name} begins", flush=True)
    days, *flags = study
    with open(out / f"{name}.out", "w") as report:
        code = subprocess.run([sys.executable, str(REPO / "dev/experiments/network_study.py"), days,
                               str(out / f"{name}.csv"), *flags, "--resume", checkpoint, "--label", name],
                              cwd=REPO, env=env, stdout=report, stderr=subprocess.STDOUT).returncode
    print(f"lane {lane}: study {name} ended ({'ok' if code == 0 else f'exit {code}'}) after "
          f"{(time.time() - started) / 60:.1f} min", flush=True)
    subprocess.run([sys.executable, str(REPO / "scripts/rcon.py"), "stop"], env=env, capture_output=True)
    try:
        server.wait(timeout=120)
    except subprocess.TimeoutExpired:
        server.kill()


def main():
    if "--" not in sys.argv or sys.argv.index("--") != 5:
        sys.exit(__doc__)
    checkpoint, label, lanes, out = sys.argv[1], sys.argv[2], int(sys.argv[3]), Path(sys.argv[4])
    study = sys.argv[6:]
    out.mkdir(parents=True, exist_ok=True)
    threads = [threading.Thread(target=run_lane, args=(n, checkpoint, label, out, study, 25 * (n - 1)))
               for n in range(1, lanes + 1)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()


if __name__ == "__main__":
    main()
