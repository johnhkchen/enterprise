#!/usr/bin/env python3
"""Compares runs side by side: each town's money, mood and trades on the last day (from the study's CSV), and the share
of each comfort met (from the run's last checkpoint), as mean and range over a label's lanes.

    python3 dev/experiments/compare_lanes.py OUTDIR LABEL [OUTDIR LABEL ...]

Each OUTDIR holds LABEL-N.csv from forks.py; checkpoints are core/run/checkpoints/LABEL-N/dayD.
"""

import csv
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import world_map as wm  # noqa: E402

REPO = Path(__file__).resolve().parents[2]
# The ridge study's towns by their corner (network_study.py): the bell is 7 east and 4 south of it.
CORNERS = {"S1": (1295, 995), "S2": (995, 1595), "S3": (1844, 1844), "S4": (4995, 995), "T": (1393, 1396),
           "E1": (1650, 1350), "E2": (1300, 1750), "E3": (2050, 1300)}
TOWNS = list(CORNERS)
BELLS = {(x + 7, z + 4): name for name, (x, z) in CORNERS.items()}
KINDS = ["treats", "reading", "clothing", "furnishings", "leisure"]


def last_day(path):
    rows = list(csv.DictReader(open(path)))
    day = max(int(r["day"]) for r in rows)
    return day, {r["name"]: r for r in rows if int(r["day"]) == day and r["kind"] == "town"}


def comforts(label, day):
    """Comfort met by town position (bell x, z), from the checkpoint at that day."""
    data = wm.load(REPO / "core/run/checkpoints" / label / f"day{day}/world/data/enterprise_territory.dat")
    out = {}
    for s in data.get("settlements", []):
        x, _, z = s["bell"]["pos"]
        out[(x, z)] = s.get("comfortMet", {})
    return out


def spread(values):
    if not values:
        return "-"
    mean = sum(values) / len(values)
    return f"{mean:7.2f} [{min(values):.2f}..{max(values):.2f}]" if max(values) < 50 else \
        f"{mean:7.0f} [{min(values):.0f}..{max(values):.0f}]"


def main():
    pairs = list(zip(sys.argv[1::2], sys.argv[2::2]))
    if not pairs:
        sys.exit(__doc__)
    for outdir, label in pairs:
        lanes = sorted(Path(outdir).glob(f"{label}-*.csv"))
        runs = []
        for csv_path in lanes:
            day, towns = last_day(csv_path)
            runs.append((day, towns, comforts(csv_path.stem, day)))
        print(f"\n== {label}: {len(runs)} lanes, day {runs[0][0] if runs else '?'} ==")
        print(f"{'town':5} {'emeralds':>22} {'mood':>22} {'trades':>22}  comforts met (mean of lanes)")
        for town in TOWNS:
            rows = [r[1][town] for r in runs if town in r[1]]
            em = [float(r["treasury"]) for r in rows]
            mood = [float(r["mood"]) for r in rows]
            trades = [float(r["trades"]) for r in rows]
            met = []
            for kind in KINDS:
                shares = [r[2].get(bell, {}).get(kind, 0.0) for r in runs for bell, name in BELLS.items() if name == town]
                met.append(f"{kind[:4]} {sum(shares) / max(1, len(shares)):4.0%}")
            print(f"{town:5} {spread(em):>22} {spread(mood):>22} {spread(trades):>22}  " + " ".join(met))


if __name__ == "__main__":
    main()
