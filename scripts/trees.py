#!/usr/bin/env python3
"""Worktrees for agents working side by side: each its own branch, folder, lanes and task, landed on main one at a time.

    python3 scripts/trees.py new NAME [--brief TEXT | --brief-file FILE] [--from REF] [--java]
    python3 scripts/trees.py list
    python3 scripts/trees.py env [NAME]            the tree's environment, as `export` lines
    python3 scripts/trees.py check [NAME] [--java] its gates: the Rust core's tests (and, with --java, the game tests)
    python3 scripts/trees.py sync [NAME]           rebase it onto main
    python3 scripts/trees.py land NAME [--no-push] rebase, check, fast-forward main, push
    python3 scripts/trees.py rm NAME [--force]
    python3 scripts/trees.py heavy [--slots K] -- CMD...   run CMD holding K of the machine's heavy-job slots

A tree lives in .trees/NAME (ignored by git) on branch tree/NAME. Its .tree/ folder (not tracked) holds its task brief
(TASK.md) and environment (env). The Rust core is built and vetted with no Java: a tree's gates are `cargo test` in
sim/, and the Java mod is only read, as the reference (scripts/parity.py says which of its game tests the core's tests
cover). With --java a tree is set up to run the mod too: it gets a slot (1 to 20) whose lanes are 10*slot+1 to
10*slot+9 (game port 25599+2N, RCON 25600+2N), so two trees' servers never meet, and the study checkpoints and server
config linked from the main tree (they're 10 GB).

Java jobs are what the machine runs out of: a Gradle daemon takes up to 2 GB, a game-test server about 2 more, a
lane's server about 1.5. `heavy` holds one of ENTERPRISE_HEAVY_SLOTS (default 3) machine-wide slots while a command
runs, waiting for one to free; `just gametest` and forks.py's lanes take theirs through it.
"""

import argparse
import fcntl
import json
import os
import subprocess
import sys
import time
from contextlib import contextmanager
from pathlib import Path

MAX_SLOTS = 20  # a slot fixes a tree's lane ports (25599+2N, N up to 209): clear of 25565/25566/25575/25576


def git(*args, cwd=None, check=True, capture=True):
    r = subprocess.run(["git", *args], cwd=cwd, check=False, text=True,
                       capture_output=capture)
    if check and r.returncode != 0:
        sys.exit(f"git {' '.join(args)} failed:\n{r.stderr.strip()}")
    return r.stdout.strip() if capture else ""


def common_dir():
    return Path(git("rev-parse", "--path-format=absolute", "--git-common-dir"))


def main_tree():
    return common_dir().parent


def trees_dir():
    """Inside the main tree (ignored by git): agents keep the project's working directory, permissions and memory."""
    return main_tree() / ".trees"


def registry_path():
    return common_dir() / "trees.json"


def load():
    p = registry_path()
    return json.loads(p.read_text()) if p.exists() else {}


def save(reg):
    registry_path().write_text(json.dumps(reg, indent=2) + "\n")


def current_name():
    here = Path.cwd().resolve()
    for name, t in load().items():
        if here == Path(t["path"]) or Path(t["path"]) in here.parents:
            return name
    return None


def tree(name):
    name = name or current_name()
    reg = load()
    if not name or name not in reg:
        sys.exit(f"no tree {name!r}: `trees.py list` shows them")
    return name, reg[name]


def env_of(name, t):
    base = 10 * t["slot"]
    return {
        "ENTERPRISE_TREE": name,
        "ENTERPRISE_SLOT": str(t["slot"]),
        "ENTERPRISE_LANE_BASE": str(base),
        "ENTERPRISE_LANE": str(base + 1),  # rcon.py and a study without forks.py: the tree's first lane
    }


# ---------------------------------------------------------------- heavy-job slots, machine-wide


@contextmanager
def heavy_slots(count=1, label=""):
    """Holds `count` of the machine's heavy-job slots for the duration (waits for them to free)."""
    total = int(os.environ.get("ENTERPRISE_HEAVY_SLOTS", "3"))
    count = max(1, min(count, total))
    folder = common_dir() / "heavy"
    folder.mkdir(exist_ok=True)
    held = []
    waited = False
    try:
        while len(held) < count:
            for i in range(total):
                if len(held) >= count:
                    break
                path = folder / f"slot-{i}.lock"
                f = open(path, "a+")
                try:
                    fcntl.flock(f, fcntl.LOCK_EX | fcntl.LOCK_NB)
                except BlockingIOError:
                    f.close()
                    continue
                f.seek(0)
                f.truncate()
                f.write(f"{os.environ.get('ENTERPRISE_TREE', 'main')}: {label}\n")
                f.flush()
                held.append(f)
            if len(held) < count:
                if not waited:
                    holders = [p.read_text().strip() for p in sorted(folder.glob("slot-*.lock")) if p.read_text().strip()]
                    print(f"waiting for {count - len(held)} heavy slot(s) of {total}; held by: {'; '.join(holders)}",
                          file=sys.stderr, flush=True)
                    waited = True
                time.sleep(2)
        yield
    finally:
        for f in held:
            f.seek(0)
            f.truncate()
            fcntl.flock(f, fcntl.LOCK_UN)
            f.close()


# ---------------------------------------------------------------- commands


def cmd_new(args):
    reg = load()
    if args.name in reg:
        sys.exit(f"tree {args.name!r} exists at {reg[args.name]['path']}")
    used = {t["slot"] for t in reg.values()}
    free = [s for s in range(1, MAX_SLOTS + 1) if s not in used]
    if not free:
        sys.exit(f"all {MAX_SLOTS} slots taken: land or remove a tree first")
    path = trees_dir() / args.name
    branch = f"tree/{args.name}"
    trees_dir().mkdir(exist_ok=True)
    git("worktree", "add", "-b", branch, str(path), args.base, cwd=main_tree())
    t = {"slot": free[0], "path": str(path), "branch": branch, "base": git("rev-parse", args.base),
         "created": time.strftime("%Y-%m-%d %H:%M")}
    reg[args.name] = t
    save(reg)
    if args.java:
        # The shared, untracked run data: checkpoints (10 GB) and the servers' config, linked from the main tree.
        run = path / "core" / "run"
        run.mkdir(parents=True, exist_ok=True)
        for shared in ["checkpoints", "config"]:
            src = main_tree() / "core" / "run" / shared
            if src.exists() and not (run / shared).exists():
                (run / shared).symlink_to(src)
    notes = path / ".tree"
    notes.mkdir(exist_ok=True)
    (notes / "env").write_text("".join(f"export {k}={v}\n" for k, v in env_of(args.name, t).items()))
    brief = Path(args.brief_file).read_text() if args.brief_file else (args.brief or "")
    (notes / "TASK.md").write_text(f"# {args.name}\n\n{brief.strip()}\n\nSee docs/worktrees.md for how trees work.\n")
    lanes = f", lanes {10 * t['slot'] + 1}-{10 * t['slot'] + 9}" if args.java else ""
    print(f"tree {args.name}: {path} on {branch}, slot {t['slot']}{lanes}")
    print(f"  cd {path} && source .tree/env")


def ahead_behind(branch):
    out = git("rev-list", "--left-right", "--count", f"main...{branch}", check=False)
    try:
        behind, ahead = out.split()
        return int(ahead), int(behind)
    except ValueError:
        return 0, 0


def cmd_list(args):
    reg = load()
    if not reg:
        print("no trees: `trees.py new NAME --brief ...` makes one")
        return
    for name, t in sorted(reg.items(), key=lambda kv: kv[1]["slot"]):
        path = Path(t["path"])
        if not path.exists():
            print(f"  {name:16} slot {t['slot']}  (folder gone: `trees.py rm {name} --force`)")
            continue
        ahead, behind = ahead_behind(t["branch"])
        dirty = len([l for l in git("status", "--porcelain", cwd=path).splitlines() if l.strip()])
        last = git("log", "-1", "--format=%cr: %s", t["branch"], check=False)[:70]
        print(f"  {name:16} slot {t['slot']}  {ahead} ahead, {behind} behind main, {dirty} uncommitted  | {last}")


def cmd_env(args):
    name, t = tree(args.name)
    for k, v in env_of(name, t).items():
        print(f"export {k}={v}")


def changed_paths(path, branch):
    base = git("merge-base", "main", branch, cwd=path)
    committed = git("diff", "--name-only", base, branch, cwd=path).splitlines()
    working = [l[3:] for l in git("status", "--porcelain", cwd=path).splitlines()]
    return set(committed) | set(working)


def run_gates(path, branch, name, java=False):
    """The tree's gates: the Rust core's tests; with `java`, the mod's game tests too. True if all pass."""
    ok = True
    if (path / "sim" / "Cargo.toml").exists():
        print(f"[{name}] cargo test --release (sim)", flush=True)
        r = subprocess.run(["cargo", "test", "--release", "-q"], cwd=path / "sim", text=True, capture_output=True)
        failed = [l for l in (r.stdout + r.stderr).splitlines() if "FAILED" in l or "panicked" in l or "error" in l]
        passed = sum(int(l.split()[3]) for l in r.stdout.splitlines() if l.startswith("test result: ok."))
        print(f"[{name}]   {'ok' if r.returncode == 0 else 'FAILED'}: {passed} tests passed" +
              ("".join(f"\n[{name}]     {l}" for l in failed[:8])), flush=True)
        ok &= r.returncode == 0
    changed = changed_paths(path, branch)
    touched_java = any(p.startswith(("core/", "pack/")) for p in changed)
    if java:
        print(f"[{name}] game tests (Java or the pack changed)", flush=True)
        env = {**os.environ, **env_of(name, load()[name])} if name in load() else os.environ
        with heavy_slots(1, f"gametest in {name}"):
            subprocess.run(["rm", "-rf", "core/run/gametest/world"], cwd=path)
            r = subprocess.run(["./gradlew", ":core:runGameTestServer"], cwd=path, text=True, capture_output=True, env=env)
        out = r.stdout + r.stderr
        summary = [l for l in out.splitlines() if "required tests" in l or "failed at" in l]
        good = any("All" in l and "required tests passed" in l for l in summary)
        print(f"[{name}]   {'ok' if good else 'FAILED'}" + "".join(f"\n[{name}]     {l[-160:]}" for l in summary[:10]),
              flush=True)
        ok &= good
    elif touched_java:
        print(f"[{name}] Java or the pack changed, but game tests aren't run in this phase (--java to run them)",
              flush=True)
    return ok


def cmd_check(args):
    name, t = tree(args.name)
    sys.exit(0 if run_gates(Path(t["path"]), t["branch"], name, args.java) else 1)


def cmd_sync(args):
    name, t = tree(args.name)
    path = Path(t["path"])
    if git("status", "--porcelain", cwd=path):
        sys.exit(f"{name} has uncommitted changes: commit them first")
    r = subprocess.run(["git", "rebase", "main"], cwd=path, text=True, capture_output=True)
    if r.returncode != 0:
        print(r.stdout + r.stderr)
        sys.exit(f"{name}: the rebase onto main stopped on a conflict; resolve it in {path} (git rebase --continue)")
    print(f"{name}: on top of main ({git('rev-parse', '--short', 'main')})")


def cmd_land(args):
    name, t = tree(args.name)
    path = Path(t["path"])
    home = main_tree()
    if git("rev-parse", "--abbrev-ref", "HEAD", cwd=home) != "main" or git("status", "--porcelain", "-uno", cwd=home):
        sys.exit("the main tree must be on main with nothing uncommitted")
    cmd_sync(argparse.Namespace(name=name))
    ahead, _ = ahead_behind(t["branch"])
    if ahead == 0:
        sys.exit(f"{name} has nothing to land")
    if not run_gates(path, t["branch"], name, args.java):
        sys.exit(f"{name}: gates failed; not landed")
    git("merge", "--ff-only", t["branch"], cwd=home)
    print(f"landed {name}: {ahead} commit(s) on main ({git('rev-parse', '--short', 'main')})")
    if not args.no_push:
        git("push", "-q", "origin", "main", cwd=home)
        print("pushed")


def cmd_rm(args):
    reg = load()
    if args.name not in reg:
        sys.exit(f"no tree {args.name!r}")
    t = reg[args.name]
    ahead, _ = ahead_behind(t["branch"])
    if ahead and not args.force:
        sys.exit(f"{args.name} has {ahead} commit(s) not on main: land it, or --force to drop them")
    if Path(t["path"]).exists():
        git("worktree", "remove", *(["--force"] if args.force else []), t["path"], cwd=main_tree())
    else:
        git("worktree", "prune", cwd=main_tree())
    git("branch", "-D" if args.force else "-d", t["branch"], cwd=main_tree(), check=False)
    del reg[args.name]
    save(reg)
    print(f"removed {args.name}")


def cmd_heavy(args):
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command:
        sys.exit("heavy: a command to run")
    with heavy_slots(args.slots, " ".join(command)[:80]):
        sys.exit(subprocess.run(command).returncode)


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    n = sub.add_parser("new")
    n.add_argument("name")
    n.add_argument("--brief")
    n.add_argument("--brief-file")
    n.add_argument("--from", dest="base", default="main")
    n.add_argument("--java", action="store_true", help="set the tree up to run the mod too (lanes, checkpoints)")
    sub.add_parser("list")
    for c in ["env", "sync"]:
        sub.add_parser(c).add_argument("name", nargs="?")
    c = sub.add_parser("check")
    c.add_argument("name", nargs="?")
    c.add_argument("--java", action="store_true", help="run the mod's game tests too")
    l = sub.add_parser("land")
    l.add_argument("name")
    l.add_argument("--no-push", action="store_true")
    l.add_argument("--java", action="store_true", help="run the mod's game tests too")
    r = sub.add_parser("rm")
    r.add_argument("name")
    r.add_argument("--force", action="store_true")
    h = sub.add_parser("heavy")
    h.add_argument("--slots", type=int, default=1)
    h.add_argument("command", nargs=argparse.REMAINDER)
    args = p.parse_args()
    {"new": cmd_new, "list": cmd_list, "env": cmd_env, "check": cmd_check, "sync": cmd_sync, "land": cmd_land,
     "rm": cmd_rm, "heavy": cmd_heavy}[args.cmd](args)


if __name__ == "__main__":
    main()
