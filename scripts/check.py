#!/usr/bin/env python3
"""The gates for what changed, not for everything (John, 2026-10-23: the CPU went to rerunning all of it, every time).

    python3 scripts/check.py [--base REF] [--only A,B] [--full]     (`just check`, `just check --full`)

What changed is everything since REF (default origin/main: what isn't pushed yet), committed or not. Then:
- the Rust core (sim/): untouched, its tests don't run. A test file alone changed: only that test target. Its sources
  changed: the crate's tests, but not the slow ones (SLOW, minutes each); `--full` runs those too. The wire's sources
  changed: `just wire-check` as well.
- the mod (core/, pack/): untouched, no game tests. Changed: it compiles, then the game test classes whose files changed,
  and any named by `--only`, in Java mode; in core mode too when the binding or the Rust core changed. A change that
  touches no test class compiles only: name the classes it affects with `--only`.
- `--full`: everything, the whole Rust suite and both game test suites. Run it once, before a landing or a push, not
  while building.
"""
import argparse
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SLOW = {"random_worlds", "random_worlds_sweep"}  # minutes each under the CPU cap: --full only
TESTS = "core/src/main/java/dev/b28/enterprise/test/"


def git(*args):
    return subprocess.run(["git", *args], cwd=ROOT, text=True, capture_output=True, check=True).stdout


def changed(base):
    committed = git("diff", "--name-only", base).splitlines()
    untracked = git("ls-files", "--others", "--exclude-standard").splitlines()
    return {p for p in committed + untracked if p}


def run(label, cmd, cwd=ROOT):
    print(f"== {label}: {' '.join(cmd)}", flush=True)
    start = time.monotonic()
    r = subprocess.run(cmd, cwd=cwd)
    print(f"== {label}: {'ok' if r.returncode == 0 else 'FAILED'} ({time.monotonic() - start:.0f}s)", flush=True)
    return r.returncode == 0


def rust(paths, full):
    """Cargo invocations for the Rust changes: [(label, args)]."""
    if full:
        return [("rust (all)", ["cargo", "test", "--release", "-q"])]
    crates = {p.split("/")[1] for p in paths if p.count("/") >= 2}
    jobs = []
    for crate in sorted(crates):
        mine = [p for p in paths if p.startswith(f"sim/{crate}/")]
        tests_dir = ROOT / "sim" / crate / "tests"
        sources = [p for p in mine if not p.startswith(f"sim/{crate}/tests/")]
        if sources or any(p.startswith("sim/Cargo") for p in paths):
            targets = sorted(f.stem for f in tests_dir.glob("*.rs") if f.stem not in SLOW) if tests_dir.exists() else []
            args = ["--lib"] + [a for t in targets for a in ("--test", t)]
        else:
            targets = sorted({Path(p).stem for p in mine if p.endswith(".rs") and p.count("/") == 3} - SLOW)
            if not targets:
                continue
            args = [a for t in targets for a in ("--test", t)]
        jobs.append((f"rust {crate}", ["cargo", "test", "--release", "-q", "-p", package(crate)] + args))
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
    a = ap.parse_args()
    paths = changed(a.base)
    sim = {p for p in paths if p.startswith("sim/")}
    mod = {p for p in paths if p.startswith(("core/", "pack/"))}
    print(f"since {a.base}: {len(paths)} files changed, {len(sim)} in sim/, {len(mod)} in the mod"
          + (" (--full: everything runs)" if a.full else ""), flush=True)
    ok = True
    if sim or a.full:
        for label, cmd in rust(sim, a.full):
            ok &= run(label, cmd, ROOT / "sim")
        if a.full or any("wire" in p for p in sim):
            ok &= run("wire-check", ["just", "wire-check"])
    if not (mod or sim or a.full or a.only):
        print("nothing in the mod or the core changed: no game tests", flush=True)
        return ok
    if a.full:
        ok &= run("game tests, Java mode", ["just", "gametest"])
        ok &= run("game tests, core mode", ["just", "gametest-core"])
        return ok
    classes = {Path(p).stem for p in mod if p.startswith(TESTS) and p.endswith("Tests.java")}
    classes |= {c for c in a.only.split(",") if c}
    if mod or sim:
        ok &= run("compile", ["./gradlew", ":core:compileJava", "-q"])
    if not ok:
        return ok
    if not classes:
        print("no test class changed or named: compiled only (`--only A,B` to run some, `--full` for all)", flush=True)
        return ok
    only = ",".join(sorted(classes))
    if mod:
        ok &= run(f"game tests, Java mode: {only}", ["just", "gametest", only])
    if sim or any("/binding/" in p for p in mod):
        ok &= run(f"game tests, core mode: {only}", ["just", "gametest-core", only])
    return ok


if __name__ == "__main__":
    sys.exit(0 if main() else 1)
