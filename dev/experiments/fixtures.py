#!/usr/bin/env python3
"""Worlds a survival study made once and keeps, so the next lane starts where the last one could (John, 2026-10-30:
"we're continually recomputing what can be turned into a fixture after the first time").

    python3 dev/experiments/fixtures.py list                 # what's kept, its size, what each saved
    python3 dev/experiments/fixtures.py prune [GB]           # the least recently used past GB (default 5) deleted

Each fixture is keyed by what made it, so it's made again only when that changes:
Kept state is reused unless something staleness.toml names changed: each key is made of those files (and the seed and
flags), nothing else. A point notes every mod file's hash (manifest.json), so a lane reusing it across a change to a
file no entry names says which, and audits the reuse (survival_lane.py).
- terrain (`core/run/fixtures/seed-S/terrain-KEY`): the world after the villages are found and the ground pregenerated
  and surveyed, before any town. Keyed by the seed, the study's flags that place and size the ground (--villages,
  --near, --pregen, --reach) and the catalog's terrain entries (John: "why do we need to pregenerate unless we're on a
  fresh seed?"). The core's own state and journal are left out: the next run's core starts fresh on it.
- points (`core/run/fixtures/seed-S/run-KEY/dayN`): the world after the towns are set up (day 0), after day 1 (two
  minutes and more: every town's layout is built) and every POINT_EVERY days. Keyed by the terrain, every study flag
  and the catalog's points entries. The Rust core may
  change under them: a new build replays the run's journals (`enterprise-sim --chain`) and the first response it gives
  otherwise is the first request the change touches; the last point before it is still the run as the new build would
  have made it. Its core is rebased (the new build's save after replaying the journals up to the point), and the study
  carries on from its day.

Copies are APFS clones (`cp -c`): a 200 MB world is copied in a second and takes room only as it changes.
"""

import hashlib
import json
import shutil
import struct
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
STORE = REPO / "core" / "run" / "fixtures"
# The study's flags that decide which ground is generated (the rest act once towns are set up).
GROUND_FLAGS = ("--villages", "--near", "--pregen", "--reach")
# What a fixture leaves behind: the core's state and journal (a new build's core starts fresh), the server's lock.
LEFT_OUT = ("session.lock", "enterprise_core.dat", "enterprise_core_stowed.dat", "enterprise-journal")
# What makes kept state stale, and why (staleness.toml): nothing else does.
CATALOG = Path(__file__).with_name("staleness.toml")
# The files a point notes the state of (manifest.json), to say which changed since that no entry names.
WATCHED = ("core", "binding", "pack", "gradle.properties", "settings.gradle", "dev/experiments/survival_study.py")
POINT_EVERY = 10


def digest(*parts):
    h = hashlib.sha256()
    for p in parts:
        h.update(p if isinstance(p, bytes) else str(p).encode())
        h.update(b"\0")
    return h.hexdigest()[:12]


def tracked(*paths):
    """The repo's files under `paths` (tracked or new, not ignored), sorted."""
    listed = subprocess.run(["git", "ls-files", "--cached", "--others", "--exclude-standard", *paths], cwd=REPO,
                            capture_output=True, text=True, check=True).stdout.split()
    return sorted({f for f in listed if (REPO / f).is_file()})


def catalogued(section):
    """The files the catalog's `section` entries name: what makes that kind of state stale."""
    import fnmatch
    import tomllib
    out = set()
    for entry in tomllib.loads(CATALOG.read_text())[section]:
        take = [g for g in entry["paths"] if not g.startswith("!")]
        leave = [g[1:] for g in entry["paths"] if g.startswith("!")]
        roots = {g.split("*")[0].rstrip("/") or "." for g in take}
        for f in tracked(*roots):
            if any(fnmatch.fnmatch(f, g) for g in take) and not any(fnmatch.fnmatch(f, g) for g in leave):
                out.add(f)
    return sorted(out)


def contents(files):
    return [part for f in files for part in (f, (REPO / f).read_bytes())]


def manifest():
    """Every watched file's hash now: what a point notes, to tell later which changed."""
    return {f: digest((REPO / f).read_bytes()) for f in tracked(*WATCHED)}


def unlisted_changes(point):
    """The watched files changed since the point was kept that no catalog entry names (those named are in its key)."""
    noted = point / "manifest.json"
    if not noted.exists():
        return ["(no manifest: kept before the catalog)"]
    was, now = json.loads(noted.read_text()), manifest()
    named = set(catalogued("points"))
    return sorted(f for f in set(was) | set(now) if was.get(f) != now.get(f) and f not in named)


def run_dir(seed, argv):
    """Where a study's points are kept: its terrain, all its flags, and the files the catalog says make points stale."""
    flags = [a for a in argv if a not in ("--fixtures", "--fresh-terrain")]
    terrain = terrain_dir(seed, argv)
    return terrain.parent / f"run-{digest(terrain.name, *flags, *contents(catalogued('points')))}"


def clone(src, dest):
    """`src` copied to `dest` (gone first) as an APFS clone: instant, room taken only as either changes."""
    shutil.rmtree(dest, ignore_errors=True)
    dest.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(["cp", "-cR", str(src), str(dest)], check=True)


def requests(journal):
    """How many requests a journal holds (each a 4-byte length and its bytes, after the magic)."""
    data = journal.read_bytes()
    i, n = 8, 0
    while i + 4 <= len(data):
        i += 4 + struct.unpack(">i", data[i:i + 4])[0]
        if i > len(data):
            break
        n += 1
    return n


def ground_flags(argv):
    return [f"{a}={argv[i + 1]}" for i, a in enumerate(argv) if a in GROUND_FLAGS and i + 1 < len(argv)]


def terrain_dir(seed, argv):
    key = digest(seed, *ground_flags(argv), *contents(catalogued("terrain")))
    return STORE / f"seed-{seed}" / f"terrain-{key}"


def ready(fixture):
    return (fixture / "fixture.json").exists() and "--fresh-terrain" not in sys.argv


def save(world, fixture, made_in, what):
    """The world as it stands copied into the fixture (the caller has the server's saving paused), with a note of
    what it was and how long it took to make: the time each later restore saves."""
    tmp = fixture.with_name(fixture.name + ".tmp")
    clone(world, tmp / "world")
    for name in LEFT_OUT:
        for f in (tmp / "world").glob(name):
            shutil.rmtree(f) if f.is_dir() else f.unlink()
    (tmp / "fixture.json").write_text(json.dumps({"what": what, "made": time.strftime("%Y-%m-%d %H:%M"),
                                                  "seconds": round(made_in), "used": 0}, indent=1))
    shutil.rmtree(fixture, ignore_errors=True)
    tmp.rename(fixture)


def restore(fixture, world):
    """The fixture's world copied to `world` (gone first); its note's uses counted. The seconds it saves."""
    clone(fixture / "world", world)
    note = json.loads((fixture / "fixture.json").read_text())
    note["used"] += 1
    note["last_used"] = time.strftime("%Y-%m-%d %H:%M")
    (fixture / "fixture.json").write_text(json.dumps(note, indent=1))
    (fixture / "fixture.json").touch()
    return note["seconds"]


SIM = ["cargo", "run", "--release", "-q", "-p", "enterprise-sim", "--"]


def save_point(world, run, day, chain, least, state, made_in):
    """The world after `day` kept as a point of the run: `chain` the journals that made it ([name, from, to], each in
    run/journals), `state` what the study needs to carry on (written into the copy as study.json).

    The core's save is taken among its requests, and one may be journaled between the save and the count (seed 1717:
    two points of three were a request short): the save is somewhere from `least` requests of the chain's last journal
    to its count. Which is found by replaying the chain to each (this source is the build the game ran) and keeping
    the one whose save is the game's, byte for byte; none (the source changed since the build): the count, said."""
    point = run / f"day{day}"
    save(world, point, made_in, f"day {day}")
    shutil.rmtree(point / "world" / "enterprise-journal", ignore_errors=True)
    for f in ("enterprise_core.dat", "enterprise_core_stowed.dat"):  # kept, unlike a terrain's: the run's own
        if (world / f).exists():
            shutil.copy2(world / f, point / "world" / f)
    game = (point / "world" / "enterprise_core.dat").read_bytes()
    aligned = False
    for to in range(chain[-1][2], max(least, chain[-1][1]) - 1, -1):
        trial = chain[:-1] + [[chain[-1][0], chain[-1][1], to]]
        n = sum(b - a for _, a, b in trial)
        out = (point / "rebased.dat").resolve()
        subprocess.run(SIM + ["--chain", "--save", str(n), str(out), *chain_args(run, trial)], cwd=REPO / "sim",
                       capture_output=True)
        if out.exists() and out.read_bytes() == game:
            chain, aligned = trial, True
            break
    (point / "world" / "study.json").write_text(json.dumps(dict(state, day=day, chain=chain, aligned=aligned), indent=1))
    (point / "manifest.json").write_text(json.dumps(manifest(), indent=0, sort_keys=True))
    return aligned


def keep_journal(journal, run):
    """The run's journal (and its .fnv) copied into run/journals, as it stands: its name there."""
    (run / "journals").mkdir(parents=True, exist_ok=True)
    for f in (journal, journal.with_name(journal.name + ".fnv")):
        shutil.copy2(f, run / "journals" / f.name)
    return journal.name


def chain_args(run, chain):
    return [f"{(run / 'journals' / name).resolve()}:{a}:{b}" for name, a, b in chain]


def answers(run, chain):
    """The game's answers to the chain's requests, as its .fnv files have them."""
    out = []
    for name, a, b in chain:
        out += (run / "journals" / f"{name}.fnv").read_text().split()[a:b]
    return out


def resume_point(run, days, say, latest=None):
    """The latest point of the run before `days` this build would have made the same, its core rebased onto this build
    (`world/enterprise_core.dat` replaced by the build's own save at that point): (point, seconds it saves), or None."""
    points = sorted((p.parent for p in run.glob("day*/fixture.json")), key=lambda p: -int(p.name[3:]))
    for point in points:
        day = int(point.name[3:])
        if day >= days or (latest is not None and day > latest):
            continue
        chain = json.loads((point / "world" / "study.json").read_text())["chain"]
        n = sum(b - a for _, a, b in chain)
        rebased = (point / "rebased.dat").resolve()
        replay = subprocess.run(SIM + ["--chain", "--save", str(n), str(rebased), *chain_args(run, chain)],
                                cwd=REPO / "sim", capture_output=True, text=True)
        if replay.returncode != 0:
            say(f"points: day {day}'s journals won't replay: {replay.stderr.strip()[-300:]}")
            continue
        game, mine = answers(run, chain), replay.stdout.split()
        parted = next((i for i, (a, b) in enumerate(zip(game, mine)) if a != b), None)
        if parted is not None:
            say(f"points: day {day} no longer holds: this build answers request {parted} of its {n} otherwise")
            continue
        same = rebased.read_bytes() == (point / "world" / "enterprise_core.dat").read_bytes()
        say(f"points: day {day} holds ({n} requests answered alike); its core "
            + ("the same, byte for byte" if same else "rebased onto this build"))
        return point, json.loads((point / "fixture.json").read_text())["seconds"]
    return None


def restore_point(point, world):
    clone(point / "world", world)
    shutil.copy2(point / "rebased.dat", world / "enterprise_core.dat")
    note = json.loads((point / "fixture.json").read_text())
    note["used"] += 1
    (point / "fixture.json").write_text(json.dumps(note, indent=1))


def size(path):
    return sum(f.stat().st_size for f in path.rglob("*") if f.is_file())


def kept():
    return sorted((p.parent for p in STORE.glob("seed-*/**/fixture.json")),
                  key=lambda p: (p / "fixture.json").stat().st_mtime)


def main():
    cmd = sys.argv[1] if len(sys.argv) > 1 else "list"
    if cmd == "list":
        total = 0
        for f in kept():
            note, mb = json.loads((f / "fixture.json").read_text()), size(f) / 1e6
            total += mb
            print(f"{f.relative_to(STORE)}: {note['what']}, {mb:.0f} MB, made {note['made']} in {note['seconds']}s, "
                  f"used {note['used']} times ({note['used'] * note['seconds'] / 60:.0f} min saved)")
        print(f"{total / 1e3:.1f} GB in {STORE.relative_to(REPO)}")
    elif cmd == "prune":
        cap = float(sys.argv[2]) * 1e9 if len(sys.argv) > 2 else 5e9
        fixtures = kept()
        total = sum(size(f) for f in fixtures)
        for f in fixtures:  # oldest used first
            if total <= cap:
                break
            total -= size(f)
            shutil.rmtree(f)
            print(f"pruned {f.relative_to(STORE)}")
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
