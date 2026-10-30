#!/usr/bin/env python3
"""One survival study on a lane of its own: a world of SEED in core/run/lanes/N, its server, the study, the server
stopped. The ground comes from a fixture when one was kept for these flags (fixtures.py), else it's made and kept.

    python3 dev/experiments/survival_lane.py N SEED DAYS OUTPREFIX [--from DAY] [--audit | --no-audit] [-- study flags]

`--from DAY`: carry on from the run's point of that day or the latest before it (not the latest that holds).

A point is reused across any change staleness.toml doesn't name, and the reuse audited: the lane says which files
changed, carries on from the point before it instead, and at the end compares its towns at the reused point's day with
the kept run's (compare_runs.py). Alike: the catalog was right that those changes don't touch the world by then. Not
alike: it was wrong, and the lane says so and writes it to core/run/fixtures/audits.log (the files belong in the
catalog, with their reason). `--audit` audits a reuse with no such change (the noise of a restart), `--no-audit` skips it.

The study's flags default to the seed 1717 network (--chests --villages 5 --reach 160 --near -736,208 --roads 1000
--pregen 300). OUTPREFIX.log is the study's log, .out its output, .server.log the server's. The server's watchdog is
off (max-tick-time=-1: a long sprint isn't a hung server).
"""

import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(Path(__file__).parent))
import compare_runs  # noqa: E402
import fixtures  # noqa: E402

AUDITS = fixtures.STORE / "audits.log"


def trace(journal, db):
    """The journal replayed into SQLite (`enterprise-sim --trace`), for compare_runs."""
    subprocess.run(fixtures.SIM + ["--trace", str(journal.resolve()), str(db.resolve())], cwd=REPO / "sim",
                   capture_output=True, check=True)
    return db

DEFAULT = ["--chests", "--villages", "5", "--reach", "160", "--near", "-736,208", "--roads", "1000", "--pregen", "300"]


def main():
    if len(sys.argv) < 5:
        sys.exit(__doc__)
    n, seed, days, out = int(sys.argv[1]), sys.argv[2], sys.argv[3], sys.argv[4]
    flags = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else DEFAULT
    mine = sys.argv[:sys.argv.index("--")] if "--" in sys.argv else sys.argv
    latest = int(mine[mine.index("--from") + 1]) if "--from" in mine else None
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
    run = fixtures.run_dir(seed, flags)
    found = fixtures.resume_point(run, int(days), print, latest)
    audit = None
    if found and int(found[0].name[3:]) > 0 and "--no-audit" not in mine:
        changed = fixtures.unlisted_changes(found[0])
        if changed or "--audit" in mine:
            reused = found[0]
            day = int(reused.name[3:])
            print(f"audit: day {day}'s point is reused across {len(changed)} changes the catalog doesn't name"
                  + (f" ({', '.join(changed[:4])}{', ...' if len(changed) > 4 else ''})" if changed else "")
                  + "; carrying on from the point before it, to compare at its day")
            audit = (reused, day, changed)
            found = fixtures.resume_point(run, day, print, day - 1)
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
    if audit:
        judge(audit, run, out)


def judge(audit, run, out):
    """The audit's verdict: the towns at the reused point's day, this run's against the kept run's."""
    reused, day, changed = audit
    found = re.findall(r"journal: (\S+\.ejl)", Path(f"{out}.log").read_text())
    if not found:
        print("audit: this run named no journal; not judged")
        return
    kept_journal = run / "journals" / json.loads((reused / "world" / "study.json").read_text())["chain"][-1][0]
    mine = trace(Path(found[-1]), Path(f"{out}.audit-new.db"))
    kept = trace(kept_journal, Path(f"{out}.audit-kept.db"))
    ok, why = compare_runs.alike(str(kept), str(mine), day)
    verdict = ("alike: the catalog holds" if ok
               else "NOT alike: the catalog missed what made it stale (" + "; ".join(why[:6]) + ")")
    print(f"audit: day {day}, {verdict}")
    AUDITS.parent.mkdir(parents=True, exist_ok=True)
    with AUDITS.open("a") as log:
        log.write(json.dumps({"when": time.strftime("%Y-%m-%d %H:%M"), "point": str(reused.relative_to(REPO)),
                              "changed": changed, "alike": ok, "why": why}) + "\n")


if __name__ == "__main__":
    main()
