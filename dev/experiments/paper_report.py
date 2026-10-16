#!/usr/bin/env python3
"""How the paper did: forward crop sales, rents' coupons and rents, lane by lane, from each lane's last checkpoint.

    python3 dev/experiments/paper_report.py OUTDIR LABEL [OUTDIR LABEL ...] [--base CHECKPOINT]

Each OUTDIR holds LABEL-N.csv from forks.py; checkpoints are core/run/checkpoints/LABEL-N/dayD. With --base (the
checkpoint a fork started from), only contracts and rents written after it count: what the fork itself did.
"""

import csv
import sys
from collections import defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import world_map as wm  # noqa: E402

REPO = Path(__file__).resolve().parents[2]
# Desks by (x, z): the ridge study's towns (their corner + 7, 7) and the capital.
CORNERS = {"S1": (1295, 995), "S2": (995, 1595), "S3": (1844, 1844), "S4": (4995, 995), "T": (1393, 1396),
           "E1": (1650, 1350), "E2": (1300, 1750), "E3": (2050, 1300)}
NAMES = {"S1": "Meadowworth", "S2": "Wheatfield", "S3": "Hollowwick", "S4": "Greenwick", "T": "Haygate",
         "E1": "Barleyby", "E2": "Cloverby", "E3": "Lindenfield"}
DESKS = {(x + 7, z + 7): NAMES[k] for k, (x, z) in CORNERS.items()}


def town(gp):
    x, _, z = gp["pos"]
    return DESKS.get((x, z), f"{x},{z}")


def last_day(csv_path):
    return max(int(r["day"]) for r in csv.DictReader(open(csv_path)))


def load(checkpoint):
    data = Path(checkpoint) / "world/data"
    contracts = wm.load(data / "enterprise_contracts.dat") if (data / "enterprise_contracts.dat").exists() else {}
    rents = wm.load(data / "enterprise_rents.dat") if (data / "enterprise_rents.dat").exists() else {}
    return contracts.get("contracts", []), rents.get("rents", [])


def harvest(contract):
    return next((t for t in contract["terms"] if t.get("type") == "harvest"), None)


def delivery(contract):
    return next((t for t in contract["terms"] if t.get("type") == "delivery"), None)


def report(checkpoint, base_ids):
    contracts, rents = load(checkpoint)
    old_contracts, old_rents = base_ids
    forward = [c for c in contracts if harvest(c) and not harvest(c).get("carried") and c["id"] not in old_contracts]
    coupons = [c for c in contracts if harvest(c) and harvest(c).get("carried") and c["id"] not in old_contracts]
    rents = [r for r in rents if r["id"] not in old_rents]
    out = {}
    states = defaultdict(int)
    for c in forward:
        states[c["state"].lower()] += 1
    owed = sum(delivery(c)["quantity"] * c.get("missesCounted", 0) for c in forward)
    got = sum(c.get("delivered", 0) for c in forward)
    out["forward"] = len(forward)
    out["forward states"] = dict(states)
    out["forward delivered of judged"] = got / owed if owed else None
    out["forward rolled"] = sum(sum("rolled over" in line for line in c.get("history", [])) for c in forward)
    out["advances paid"] = sum(c.get("advanced", 0) for c in forward)
    out["advances owed back"] = sum(c.get("refund", 0) for c in forward)
    due = sum(delivery(c)["quantity"] for c in coupons if c["state"] != "ACTIVE")
    out["coupons"] = len(coupons)
    out["coupons delivered of closed"] = sum(c.get("delivered", 0) for c in coupons if c["state"] != "ACTIVE") / due \
        if due else None
    rstates = defaultdict(int)
    for r in rents:
        rstates[r["state"].lower()] += 1
    out["rents"] = len(rents)
    out["rent states"] = dict(rstates)
    out["rent capital"] = sum(r["capital"] for r in rents)
    out["rent arrears"] = sum(r.get("arrears", 0) for r in rents if r["state"] == "ACTIVE")
    out["rents with a player"] = sum(1 for r in rents if r.get("bearer"))
    # By farm: what each sold forward and delivered of what was judged.
    farms = defaultdict(lambda: [0, 0, 0])
    for c in forward:
        f = farms[town(c["seller"])]
        f[0] += 1
        f[1] += c.get("delivered", 0)
        f[2] += delivery(c)["quantity"] * c.get("missesCounted", 0)
    out["farms"] = {k: f"{v[0]} sold, {v[1]} of {v[2]} judged" for k, v in sorted(farms.items())}
    return out


def pct(x):
    return "-" if x is None else f"{x:.0%}"


def main():
    args = sys.argv[1:]
    base_ids = (set(), set())
    if "--base" in args:
        i = args.index("--base")
        contracts, rents = load(args[i + 1])
        base_ids = ({c["id"] for c in contracts}, {r["id"] for r in rents})
        args = args[:i] + args[i + 2:]
    pairs = list(zip(args[0::2], args[1::2]))
    if not pairs:
        sys.exit(__doc__)
    for outdir, label in pairs:
        print(f"\n== {label}" + (" (since the base)" if base_ids[0] else ""))
        for csv_path in sorted(Path(outdir).glob(f"{label}-*.csv")):
            day = last_day(csv_path)
            r = report(REPO / "core/run/checkpoints" / csv_path.stem / f"day{day}", base_ids)
            print(f"  {csv_path.stem} day {day}: forward {r['forward']} {r['forward states']}, delivered "
                  f"{pct(r['forward delivered of judged'])} of judged, {r['forward rolled']} rolled, advances "
                  f"{r['advances paid']:.1f} paid / {r['advances owed back']:.1f} owed back")
            print(f"    coupons {r['coupons']}, {pct(r['coupons delivered of closed'])} delivered; rents {r['rents']} "
                  f"{r['rent states']}, capital {r['rent capital']:.0f}, arrears {r['rent arrears']}, "
                  f"{r['rents with a player']} with players")
            for farm, line in r["farms"].items():
                print(f"    {farm}: {line}")


if __name__ == "__main__":
    main()
