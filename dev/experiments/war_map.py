#!/usr/bin/env python3
"""The war for the region on its hexes, day by day, from a replay's trace (`enterprise-sim --trace`).

    python3 dev/experiments/war_map.py TRACE.db OUT.html [--title "Seed 1717's War"] [--note "..."]

The ground beneath (its land, water, rivers and heights: the trace's `ground`); each side's zone of control over it,
a gradient from its towns (by their people) and squads (by their soldiers), fading with distance; each town's land
outlined by its side (the Horde, the Alliance, no one's people) from its founding; the battles for a
town as they're fought, held and won; a town razed, its ruin; the squads on the march; and each caravan road as wide
as the trips it carried in the 30 days before (the trace's `trips` on its `paths`). Under the map, by day: the Horde's
towns, the wagons on the roads, Theramore's chest. Reads the trace's `places`, `claims`, `towns`, `battles`, `squads`,
`razed`, `war`, `chests`, `trips` and `paths` (a trace written before `places` and `claims` has no map to draw).
Publish OUT.html as an Artifact.
"""
import json
import sqlite3
import sys
from pathlib import Path


def rows(db, sql):
    return db.execute(sql).fetchall()


def main():
    args = sys.argv[1:]
    if len(args) < 2:
        sys.exit(__doc__)
    trace, out = args[0], Path(args[1])
    opt = lambda name, default: args[args.index(name) + 1] if name in args else default
    title, note = opt("--title", "Seed 1717's War"), opt("--note", "")
    db = sqlite3.connect(trace)
    last = rows(db, "select max(day) from towns")[0][0]
    side = {"the Horde": 0, "the Alliance": 1}
    places = [{"t": t, "name": n, "side": side.get(f, 2), "q": q, "r": r, "first": d}
              for t, n, f, q, r, d in rows(db, "select town, name, faction, q, r, first_day from places order by town")]
    razed = {t: d for d, t in rows(db, "select day, town from razed")}
    for p in places:
        p["razed"] = razed.get(p["t"])
    pop = {}
    for d, t, v in rows(db, "select day, town, villagers from towns"):
        pop.setdefault(t, {})[d] = round(v)
    for p in places:
        days = pop.get(p["t"], {})
        p["pop"], held = [], 0
        for d in range(last + 1):
            held = days.get(d, held)
            p["pop"].append(held)
    claims = rows(db, "select day, q, r, town from claims order by day")
    battles = rows(db, "select day, battle, q, r, coalesce(town, -1), case attacker when 'the Horde' then 0 when 'the Alliance' then 1 "
                       "else -1 end, case holder when 'the Horde' then 0 when 'the Alliance' then 1 else -1 end, held_days, horde, "
                       "alliance from battles where won_by is null")
    squads = rows(db, "select day, case side when 'the Horde' then 0 else 1 end, capital, soldiers, q, r from squads where soldiers > 0")
    war = {d: (h, m, mc) for d, h, m, mc in rows(db, "select day, horde_towns, marines, mercenaries from war")}
    chest = {d: round(left) for d, left in rows(db, "select day, left_over from chests where capital = 'Theramore'")}
    first_war = min(war) if war else last
    series = {"horde": [], "marines": [], "chest": []}
    for d in range(last + 1):
        h, m, _ = war.get(d, (None, None, None))
        series["horde"].append(h)
        series["marines"].append(m)
        series["chest"].append(chest.get(d))
    paths = {}
    for route, step, q, r in rows(db, "select route, step, q, r from paths order by route, step"):
        paths.setdefault(route, []).append([q, r])
    trips = rows(db, "select route, left_day, takings from trips")
    used = sorted({t[0] for t in trips if t[0] in paths})
    # The wagons on the roads in each day's last 30, all roads together.
    per_day = [0] * (last + 1)
    for _, d, _ in trips:
        if 0 <= d <= last:
            per_day[d] += 1
    trade = [sum(per_day[max(0, d - 29):d + 1]) for d in range(last + 1)]
    traded_to = max((t[1] for t in trips), default=0)
    events = []
    for p in places:
        if p["first"] > 1:
            events.append([p["first"], f"{p['name']} founded", p["side"]])
    for d, t, by in rows(db, "select day, town, by_faction from razed"):
        name = next((p["name"] for p in places if p["t"] == t), f"town {t}")
        events.append([d, f"{name} razed", side.get(by, 2)])
    for (d,) in rows(db, "select min(day) from war"):
        if d is not None:
            events.append([d, "War declared", 1])
    for d, who in rows(db, "select min(day), won_by from war where won_by is not null group by won_by"):
        events.append([d, f"Won by {who.removeprefix('the ')}", side.get(who, 2)])
    for d, in rows(db, "select min(day) from war where landed = 1"):
        if d is not None:
            events.append([d, "Theramore lands", 1])
    # The supply wagons (a trace written before `supplies` has none): on the hexes by day, and each one taken.
    try:
        supplies = rows(db, "select day, case side when 'the Horde' then 0 else 1 end, q, r, round(food), escort, homeward "
                            "from supplies")
        tallies = rows(db, "select day, wagons_sent_alliance, wagons_delivered_alliance, wagons_taken_alliance, "
                           "wagons_taken_horde from war order by day")
    except sqlite3.OperationalError:
        supplies, tallies = [], []
    wagons = {d: [sent, through, taken, horde_taken] for d, sent, through, taken, horde_taken in tallies}
    prev = [0, 0]
    for d, _, _, taken, horde_taken in tallies:
        for i, (n, who) in enumerate([(taken, "Theramore's supply wagon taken"), (horde_taken, "A Horde supply wagon taken")]):
            if (n or 0) > prev[i]:
                events.append([d, who if n - prev[i] == 1 else f"{who} ({n - prev[i]})", 0 if i == 0 else 1])
            prev[i] = n or 0
    events.sort(key=lambda e: e[0])
    lands = ["Plains", "Woods", "Taiga", "Desert", "Shore", "Hills", "Water"]
    try:
        ground = [[q, r, lands.index(l) if l in lands else 0, round(w * 100), None if h is None else round(h), int(rv)]
                  for q, r, l, w, h, rv in rows(db, "select q, r, land, water, height, river from ground")]
    except sqlite3.OperationalError:
        ground = []
    data = {"ground": ground, "last": last, "firstWar": first_war, "places": places, "claims": claims, "battles": battles,
            "squads": squads, "supplies": supplies, "wagons": wagons, "series": series, "trade": trade, "tradedTo": traded_to, "events": events,
            "routes": [{"id": r, "path": paths[r]} for r in used],
            "trips": [[r, d] for r, d, _ in trips if r in paths]}
    page = Path(__file__).with_name("war_map.html").read_text()
    page = page.replace("__TITLE__", title).replace("__NOTE__", note).replace("__DATA__", json.dumps(data, separators=(",", ":")))
    out.write_text(page)
    print(f"{out}: {len(places)} towns, {len(claims)} claims, {len(battles)} battle-days, {len(squads)} squad-days, "
          f"{len(trips)} trips, {len(supplies)} wagon-days, {out.stat().st_size // 1024} KB")


if __name__ == "__main__":
    main()
