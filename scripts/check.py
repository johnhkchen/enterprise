#!/usr/bin/env python3
"""The gates for what changed, not for everything (John, 2026-10-23: the CPU went to rerunning all of it, every time).

    python3 scripts/check.py [--base REF] [--only A,B] [--full]     (`just check`, `just check --full`)

What changed is everything since REF (default origin/main: what isn't pushed yet), committed or not. Then:
- the Rust core (sim/): untouched, its tests don't run. Test files alone changed: only theirs. Its sources changed: the
  crate's tests, the random-worlds sweeps short (a tenth of the worlds, tests/common `seeds`); `--full` runs them whole
  (SWEEP=full). The wire's sources changed: `just wire-check` as well.
- the mod (core/, pack/, binding/: its JUnit tests first): untouched, no game tests. Changed: it compiles, then the game test classes whose files changed,
  and any named by `--only`, in Java mode; in core mode too when the binding or the Rust core changed. A change that
  touches no test class compiles only: name the classes it affects with `--only`.
- the asset studio's validator (scripts/assets/): its tests, and every shipped structure, when either changed.
- `--full`: everything, the whole Rust suite and both game test suites. Run it once, before a landing or a push, not
  while building.

A gate that passed is kept (build/check-passed/), keyed by its command and every file it reads (sim/ for the Rust
gates; sim/, core/, binding/ and pack/ for the game tests): the same gate on the same files isn't run again, it says
"passed already". A failure is never kept. `--again` runs them all regardless.
"""
import argparse
import hashlib
import os
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TESTS = "core/src/main/java/dev/b28/enterprise/test/"


def git(*args):
    return subprocess.run(["git", *args], cwd=ROOT, text=True, capture_output=True, check=True).stdout


def changed(base):
    committed = git("diff", "--name-only", base).splitlines()
    untracked = git("ls-files", "--others", "--exclude-standard").splitlines()
    return {p for p in committed + untracked if p}


PASSED = ROOT / "build" / "check-passed"
AGAIN = "--again" in sys.argv
# What each gate reads: the Rust gates the core's crates, the game tests the mod and the core it runs.
RUST_INPUTS = ("sim",)
GAME_INPUTS = ("sim", "core", "binding", "pack", "gradle.properties", "settings.gradle", "justfile")


def key(label, cmd, inputs):
    """The gate's command and the bytes of every file it reads (tracked or new, not ignored), hashed."""
    files = git("ls-files", "--cached", "--others", "--exclude-standard", *inputs).split()
    h = hashlib.sha256(" ".join([label, *cmd, os.environ.get("SWEEP", "")]).encode())
    for f in sorted(set(files)):
        path = ROOT / f
        if path.is_file():
            h.update(f.encode() + b"\0" + path.read_bytes())
    return h.hexdigest()[:16]


def run(label, cmd, cwd=ROOT, inputs=None):
    k = key(label, cmd, inputs) if inputs else None
    if k and not AGAIN and (PASSED / k).exists():
        print(f"== {label}: passed already on these files ({(PASSED / k).read_text().strip()})", flush=True)
        return True
    print(f"== {label}: {' '.join(cmd)}", flush=True)
    start = time.monotonic()
    r = subprocess.run(cmd, cwd=cwd)
    took = time.monotonic() - start
    print(f"== {label}: {'ok' if r.returncode == 0 else 'FAILED'} ({took:.0f}s)", flush=True)
    if k and r.returncode == 0:
        PASSED.mkdir(parents=True, exist_ok=True)
        (PASSED / k).write_text(f"{time.strftime('%Y-%m-%d %H:%M')}, {took:.0f}s saved\n")
    return r.returncode == 0


def rust(paths, full):
    """Cargo invocations for the Rust changes: [(label, args)]. The core's tests are one binary, `all` (its
    tests/all.rs), beside `replay`, `scenarios` and `wire_fixtures`; a file's tests are its module's (`lockers::`)."""
    if full:
        return [("rust (all, SWEEP=full)", ["cargo", "test", "--release", "-q"])]
    # A crate is a folder with a Cargo.toml (sim/scenarios and sim/wire are the core's data, its tests read them).
    crates = {p.split("/")[1] for p in paths if p.count("/") >= 2}
    if crates - {c for c in crates if (ROOT / "sim" / c / "Cargo.toml").exists()}:
        crates = {c for c in crates if (ROOT / "sim" / c / "Cargo.toml").exists()} | {"core"}
    own = {"replay", "scenarios", "wire_fixtures"}
    jobs = []
    for crate in sorted(crates):
        mine = [p for p in paths if p.startswith(f"sim/{crate}/")]
        base = ["cargo", "test", "--release", "-q", "-p", package(crate)]
        if crate != "core" or any(not p.startswith("sim/core/tests/") for p in mine) \
                or any(p.startswith("sim/core/tests/common/") for p in mine):
            jobs.append((f"rust {crate}", base))
            continue
        files = sorted({Path(p).stem for p in mine if p.endswith(".rs") and p.count("/") == 3})
        alone = [a for f in files if f in own for a in ("--test", f)]
        if alone:  # their own binaries: a filter after `--` would apply to them too
            jobs.append((f"rust core: {', '.join(f for f in files if f in own)}", base + alone))
        modules = [f"{f}::" for f in files if f not in own and f != "all"]
        if modules:
            jobs.append((f"rust core: {', '.join(modules)}", base + ["--test", "all", "--", *modules]))
    return jobs


def package(crate):
    for line in (ROOT / "sim" / crate / "Cargo.toml").read_text().splitlines():
        if line.startswith("name"):
            return line.split("=", 1)[1].strip().strip('"')
    return crate


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--base", default="origin/main")
    ap.add_argument("--only", default="", help="game test classes to run, comma-separated")
    ap.add_argument("--full", action="store_true")
    ap.add_argument("--again", action="store_true", help="run gates that passed already on these files")
    a = ap.parse_args()
    paths = changed(a.base)
    sim = {p for p in paths if p.startswith("sim/")}
    mod = {p for p in paths if p.startswith(("core/", "pack/", "binding/"))}
    print(f"since {a.base}: {len(paths)} files changed, {len(sim)} in sim/, {len(mod)} in the mod"
          + (" (--full: everything runs)" if a.full else ""), flush=True)
    ok = True
    # The asset studio's validator and its tests (`scripts/assets`, docs/assets/README.md), and every shipped structure
    # it judges, whenever either changed.
    assets = {p for p in paths if p.startswith(("scripts/assets/", "core/src/main/resources/data/enterprise/structure/"))}
    if assets or a.full:
        ok &= run("asset validator tests", ["node", "--test", "scripts/assets/check-asset.test.mjs"])
        # Each kingdom's folder (`floor.nbt` beside them is the game tests' arena, not an asset).
        # Each kingdom's buildings by kind (not its `part/` sets: a part is a piece of a building, judged in the
        # buildings assembled from it).
        for kind in sorted(p for p in (ROOT / "core/src/main/resources/data/enterprise/structure").glob("*/*")
                           if p.is_dir() and p.name != "part"):
            ok &= run(f"asset validator: {kind.parent.name}/{kind.name}", ["node", "scripts/assets/check-asset.mjs",
                                                                          str(kind.relative_to(ROOT))])
    if sim or a.full:
        if a.full:
            os.environ["SWEEP"] = "full"
        for label, cmd in rust(sim, a.full):
            ok &= run(label, cmd, ROOT / "sim", RUST_INPUTS)
        if a.full or any("wire" in p for p in sim):
            ok &= run("wire-check", ["just", "wire-check"], inputs=GAME_INPUTS)
    if not (mod or sim or a.full or a.only):
        print("nothing in the mod or the core changed: no game tests", flush=True)
        return ok
    if a.full:
        ok &= run("game tests, Java mode", ["just", "gametest"], inputs=GAME_INPUTS)
        ok &= run("game tests, core mode", ["just", "gametest-core"], inputs=GAME_INPUTS)
        return ok
    classes = {Path(p).stem for p in mod if p.startswith(TESTS) and p.endswith("Tests.java")}
    classes |= {c for c in a.only.split(",") if c}
    if any(p.startswith("binding/") for p in mod):
        ok &= run("binding tests", ["./gradlew", ":binding:test", "-PwasmProfile=wasm-dev", "-q"])
    if mod or sim:
        ok &= run("compile", ["./gradlew", ":core:compileJava", "-PwasmProfile=wasm-dev", "-q"])
    if not ok:
        return ok
    if not classes:
        print("no test class changed or named: compiled only (`--only A,B` to run some, `--full` for all)", flush=True)
        return ok
    only = ",".join(sorted(classes))
    if mod:
        ok &= run(f"game tests, Java mode: {only}", ["just", "gametest", only], inputs=GAME_INPUTS)
    if sim or any(p.startswith("binding/") or "/binding/" in p for p in mod):
        ok &= run(f"game tests, core mode: {only}", ["just", "gametest-core", only], inputs=GAME_INPUTS)
    return ok


if __name__ == "__main__":
    sys.exit(0 if main() else 1)
