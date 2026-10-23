#!/usr/bin/env python3
"""Per-class game test passes against a committed baseline (docs/binding-plan.md B3.2).

    python3 scripts/gametest_core.py REPORT.tsv                 print passes per class, fail if a baseline pass fell
    python3 scripts/gametest_core.py REPORT.tsv --write-baseline  write scripts/gametest-core-baseline.txt from it
    python3 scripts/gametest_core.py REPORT.tsv --only A,B      a run of only those classes: only their baseline counts

REPORT.tsv is core/run/gametest-core/report.tsv (Java: core/run/gametest/report.tsv): `pass|fail|optional-fail<TAB>test`
per line, written by the mod's GameTestReport. The baseline lists the tests that passed in core mode, one name a line;
a test that is in it and doesn't pass now is a fall (a test that newly passes is not, and is printed).
"""
import re
import sys
from collections import defaultdict
from pathlib import Path

BASELINE = Path(__file__).resolve().parent / "gametest-core-baseline.txt"


def read(path):
    results = {}
    for line in Path(path).read_text().splitlines():
        status, _, name = line.partition("\t")
        if name:
            results[name.split(":")[-1]] = status
    return results


TESTS = Path(__file__).resolve().parent.parent / "core/src/main/java/dev/b28/enterprise/test"


def classes():
    """Lowercased test method -> its test class, from the sources (the game reports only the method's name)."""
    found = {}
    for f in TESTS.glob("*.java"):
        for m in re.finditer(r"@GameTest\b[^\n]*\n(?:\s*@[^\n]*\n)*\s*(?:public\s+|static\s+)*void\s+(\w+)", f.read_text()):
            found[m.group(1).lower()] = f.stem
    return found


_CLASSES = classes()


def cls(name):
    return _CLASSES.get(name.split(".")[-1], "?")


def main():
    only = next((a.split("=", 1)[1] for a in sys.argv[1:] if a.startswith("--only=")), "")
    only = {c for c in only.split(",") if c}
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    if not args:
        sys.exit(__doc__)
    if not Path(args[0]).exists():
        sys.exit(f"no report at {args[0]}: the game test server didn't start")
    results = read(args[0])
    if not results:
        sys.exit(f"{args[0]} is empty: no test ran")
    passed = sorted(n for n, s in results.items() if s == "pass")
    per = defaultdict(lambda: [0, 0])
    for n, s in results.items():
        per[cls(n)][1] += 1
        per[cls(n)][0] += s == "pass"
    if "--write-baseline" in sys.argv:
        BASELINE.write_text("".join(n + "\n" for n in passed))
        print(f"baseline written: {len(passed)} of {len(results)} tests pass -> {BASELINE}")
    base = set(BASELINE.read_text().split()) if BASELINE.exists() else set()
    if only:
        base = {n for n in base if cls(n) in only}
    fell = sorted(base - set(passed))
    base_per = defaultdict(int)
    for n in base:
        base_per[cls(n)] += 1
    print(f"{'class':28} {'pass':>5} {'of':>4} {'baseline':>9}")
    for c in sorted(per):
        mark = "  <- fell" if per[c][0] < base_per[c] else ""
        print(f"{c:28} {per[c][0]:5} {per[c][1]:4} {base_per[c]:9}{mark}")
    print(f"total: {len(passed)} of {len(results)} pass; baseline {len(base)}")
    new = sorted(set(passed) - base)
    if new and base:
        print(f"newly passing ({len(new)}): " + ", ".join(new[:12]) + (" ..." if len(new) > 12 else ""))
    if fell:
        print(f"FELL below the baseline ({len(fell)}):")
        for n in fell:
            print(f"  {n}  ({results.get(n, 'not run')})")
        sys.exit(1)


main()
