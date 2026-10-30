#!/usr/bin/env python3
"""How alike two runs' game sides were, day by day: what the mod told the core (each journal's inputs, from
`enterprise-sim --trace`), with what one run of the game can't repeat taken out (fixtures.py's audit of a reuse).

    python3 dev/experiments/compare_runs.py A.db B.db [FIRST LAST]     # the inputs, day by day
    python3 dev/experiments/compare_runs.py A.db B.db --at DAY         # the towns at a day: alike or not, and why

Two restarts of the same point with the same code aren't alike to the byte (seed 1717: they part at the first request):
a villager is a new entity each time (its tag), a batch of ground is sent in the order the chunks loaded, and a hex's
water is its last survey's (a chunk loaded a tick apart reads its edge a little otherwise). So each input is taken
without its tags and with its numbers to two places, and a day is the multiset of them: two days' difference is the
share of the inputs one has and the other hasn't. That is too fine to judge by: on the ledger a day is a few dozen
inputs, and two restarts of seed 1717's day 30 differed by up to half of a day's (9% of ten days').

What a run comes to is steadier: its towns at a day (`--at`), each one's villagers and land and its hands' trades and
levels, the towns' money and the crops the core holds as the game counted them. Restarts of a point against the run that
kept it (seed 1717, no change): from day 20, 30 or 40, the same towns, villagers and land every time, 0 to 2 hands of 100
in another trade or level, money together 34 against 25 at most; from day 0, whose day 1 converts every village, a
town's villagers 2 either way and 7 hands of 77, and the crops counted on day 0 341 against 287. Two runs are alike at a
day when their towns and land are the same, each town's villagers within VILLAGERS, at most HANDS of their hands differ,
and their money and crops held are within MONEY and CROPS of each other. Twice the crops counted (a test of the audit)
held 566 against 2,066 at day 1 and wasn't alike; the same restart with no change was.
"""

import json
import sqlite3
import sys
from collections import Counter

# What restarts of a point with no change differed by, at most, with room: a town's villagers (a restart of day 1, which
# converts every village: 2 either way; of a later point: none), hands in another trade or level (day 1: 7 of 77; later:
# 2 of 100), the towns' money together, and the crops the core holds as counted (day 0: 341 against 287).
VILLAGERS = 2
HANDS = 0.10
MONEY = 0.35
CROPS = 0.40
# What a run can't repeat, by input: its villagers' tags (each run's own entities).
UNREPEATABLE = {"tag", "hand_tag"}


def norm(value):
    if isinstance(value, float):
        return round(value, 2)
    if isinstance(value, dict):
        return tuple(sorted((k, norm(v)) for k, v in value.items() if k not in UNREPEATABLE))
    if isinstance(value, list):
        return tuple(sorted((norm(v) for v in value), key=repr))
    return value


def days(db, first, last):
    """{day: Counter of normalized inputs} over FIRST..LAST."""
    out = {}
    for day, js in sqlite3.connect(db).execute(
            "select day, json from inputs where day between ? and ? and kind != 'Advance'", (first, last)):
        out.setdefault(day, Counter())[norm(json.loads(js))] += 1
    return out


def differ(a, b):
    """The share of two days' inputs one has and the other hasn't (0: alike, 1: nothing shared)."""
    union = sum((a | b).values())
    return sum(((a - b) + (b - a)).values()) / union if union else 0.0


def compare(a_db, b_db, first, last):
    a, b = days(a_db, first, last), days(b_db, first, last)
    return {d: differ(a.get(d, Counter()), b.get(d, Counter())) for d in range(first, last + 1)}


def towns(db, day):
    """The towns at a day: {name: (villagers, land)}, their hands {(town, trade, level): n}, their money together."""
    con = sqlite3.connect(db)
    rows = con.execute("select name, villagers, land, money from towns where day = ?", (day,)).fetchall()
    hands = Counter(con.execute("select town, trade, level from hands where day = ?", (day,)).fetchall())
    return {n: (v, land) for n, v, land, _ in rows}, hands, sum(m for *_, m in rows)


def crops(db, day):
    """The crop plots the core holds at `day`, as the game counted them: each chunk's last count by then, together."""
    last = {}
    for js in (j for (j,) in sqlite3.connect(db).execute(
            "select json from inputs where kind = 'Crops' and day <= ? order by rowid", (day,))):
        c = json.loads(js)["Crops"]
        last[(c["hex"]["q"], c["hex"]["r"], c["chunk_x"], c["chunk_z"])] = sum(n["count"] for n in c["counts"])
    return sum(last.values())


def alike(a_db, b_db, day):
    """Whether two runs came to the same towns at `day` (see the module's doc): (alike, reasons they aren't)."""
    (ta, ha, ma), (tb, hb, mb) = towns(a_db, day), towns(b_db, day)
    why = []
    if not ta or not tb:
        return False, [f"no towns at day {day} in {'the first' if not ta else 'the second'}"]
    for n in sorted(set(ta) | set(tb)):
        a, b = ta.get(n), tb.get(n)
        if a is None or b is None or a[1] != b[1] or abs(a[0] - b[0]) > VILLAGERS:
            why.append(f"{n}: {a or 'none'} against {b or 'none'} (villagers, land)")
    differ = sum(((ha - hb) + (hb - ha)).values()) // 2
    if differ > HANDS * max(sum(ha.values()), 1):
        why.append(f"{differ} hands of {sum(ha.values())} differ in trade or level")
    if abs(ma - mb) > MONEY * max(ma, mb, 1):
        why.append(f"money together {ma:.0f} against {mb:.0f}")
    ca, cb = crops(a_db, day), crops(b_db, day)
    if abs(ca - cb) > CROPS * max(ca, cb, 1):
        why.append(f"crops held {ca:.0f} against {cb:.0f}")
    return not why, why


def main():
    a, b = sys.argv[1], sys.argv[2]
    if "--at" in sys.argv:
        ok, why = alike(a, b, int(sys.argv[sys.argv.index("--at") + 1]))
        print("alike" if ok else "not alike: " + "; ".join(why))
        return
    first, last = (int(sys.argv[3]), int(sys.argv[4])) if len(sys.argv) > 4 else (0, 10 ** 6)
    for d, x in compare(a, b, first, last).items():
        print(f"day {d}: {x:.1%} differ")


if __name__ == "__main__":
    main()
