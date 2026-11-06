#!/usr/bin/env python3
"""The war for the region on its hexes, day by day, from a replay's trace (`enterprise-sim --trace`).

    python3 dev/experiments/war_map.py TRACE.db OUT.html [--title "Seed 1717's War"] [--note "..."] [--label "One hero"]
        [--also LABEL TRACE2.db NOTE ...] [--world WORLD [--step 4] [--world-note "as it stood on day 360"]]
        [--run-world N WORLD NOTE ...]

Each `--also` adds another run of the same seed (another dial's war), switched between on the page. `--world`: the
world's own ground beneath the hexes, from its region files (a run's last kept point, `RUN/dayN/world`): its rivers'
courses, fields by crop, woods and towns as they stood, a pixel for each `--step` blocks (about 15s); the hexes'
terrain where it never generated. `--run-world N WORLD NOTE`: run N's own ground instead (0 the first, each `--also`
after it), its ruins as that run left them. With a world, its caravans and roads follow the roads it built
(`enterprise-roads.json`) where it has them.

The ground beneath (its land, water, rivers and heights: the trace's `ground`); each side's zone of control over it,
a gradient from its towns (by their people) and squads (by their soldiers), fading with distance; each town's land
outlined by its side (the Horde, the Alliance, no one's people) from its founding; the battles for a
town as they're fought, held and won; a town razed, its ruin; the squads on the march; and each caravan road as wide
as the trips it carried in the 30 days before (the trace's `trips` on its `paths`). Under the map, by day: the Horde's
towns, the wagons on the roads, Theramore's chest. Reads the trace's `places`, `claims`, `towns`, `battles`, `squads`,
`razed`, `war`, `chests`, `trips` and `paths` (a trace written before `places` and `claims` has no map to draw). Each
fight's place as it comes, by its dead (`battles`' fallen, `battle_fights`) and each supply wagon taken (`raids`),
fading after a while.
Publish OUT.html as an Artifact.
"""
import json
import math
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
    title = opt("--title", "Seed 1717's War")
    runs = [{"label": opt("--label", ""), "note": opt("--note", ""), "data": build(trace)}]
    for i, a in enumerate(args):
        if a == "--also":
            runs.append({"label": args[i + 1], "note": args[i + 3], "data": build(args[i + 2])})
    step = int(opt("--step", "4"))
    worlds = {}
    if "--world" in args:
        worlds = {i: (opt("--world", ""), opt("--world-note", "")) for i in range(len(runs))}
    for i, a in enumerate(args):
        if a == "--run-world":
            worlds[int(args[i + 1])] = (args[i + 2], args[i + 3])
    made = {}
    for i, run in enumerate(runs):
        if i in worlds:
            if worlds[i] not in made:
                made[worlds[i]] = ground_of(runs[0]["data"]["ground"], Path(worlds[i][0]), step, worlds[i][1])
            run["world"] = list(made).index(worlds[i])
            run["data"]["roads"] = roads_of(Path(worlds[i][0]))
    world = list(made.values())
    page = Path(__file__).with_name("war_map.html").read_text()
    page = (page.replace("__TITLE__", title).replace("__DATA__", json.dumps(runs, separators=(",", ":")))
            .replace("__WORLD__", json.dumps(world, separators=(",", ":"))))
    out.write_text(page)
    print(f"{out}: {len(runs)} runs, {out.stat().st_size // 1024} KB")


def ground_of(ground, world, step, note):
    """The world's own ground beneath, from its region files (`world_render.overview`): the hexes' bounds."""
    from world_render import overview, SIZE
    xs = [SIZE * math.sqrt(3) * (g[0] + g[1] / 2) for g in ground]
    zs = [SIZE * 1.5 * g[1] for g in ground]
    x0, z0 = int(min(xs) - SIZE) // step * step, int(min(zs) - SIZE) // step * step
    x1, z1 = int(max(xs) + SIZE), int(max(zs) + SIZE)
    img, w, h = overview(world, x0, z0, x1, z1, step)
    return {"img": img, "x0": x0, "z0": z0, "step": step, "w": w, "h": h, "note": note}


def roads_of(world):
    """The roads the world has built (`enterprise-roads.json`): each between two towns' hexes, its line in blocks
    (every other point, to a block), for the caravans to follow."""
    try:
        roads = json.loads((world / "enterprise-roads.json").read_text())
    except FileNotFoundError:
        return []
    return [{"a": r["path"][0], "b": r["path"][-1],
             "line": [[round(x), round(z)] for x, z in r["line"][::2] + r["line"][-1:]]} for r in roads]


def build(trace):
    """A run's war, from its trace, as the page reads it."""
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
    squads = rows(db, "select day, case side when 'the Horde' then 0 else 1 end, capital, soldiers, q, r, squad from squads "
                       "where soldiers > 0")
    war = {d: (h, m, mc) for d, h, m, mc in rows(db, "select day, horde_towns, marines, mercenaries from war")}
    chest = {d: round(left) for d, left in rows(db, "select day, left_over from chests where capital = 'Theramore'")}
    first_war = min(war) if war else last + 1
    series = {"horde": [], "marines": [], "chest": []}
    for d in range(last + 1):
        h, m, _ = war.get(d, (None, None, None))
        series["horde"].append(h)
        series["marines"].append(m)
        series["chest"].append(chest.get(d))
    paths = {}
    for route, step, q, r in rows(db, "select route, step, q, r from paths order by route, step"):
        paths.setdefault(route, []).append([q, r])
    trips = rows(db, "select route, left_day, takings, home_day, left_tick, home_tick from trips")
    used = sorted({t[0] for t in trips if t[0] in paths})
    # The wagons on the roads in each day's last 30, all roads together.
    per_day = [0] * (last + 1)
    for _, d, *_ in trips:
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
        supplies = rows(db, "select day, case side when 'the Horde' then 0 else 1 end, q, r, round(food), escort, homeward, "
                            "wagon from supplies")
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
    # The fights as they come: each battle's day of fighting at its hex, its dead by side that day (the trace's
    # fallen, summed); and each supply wagon taken, where it was last seen on its way out.
    fights = {}
    try:
        seen = {}
        for d, battle, q, r, fh, fa in rows(db, "select day, battle, q, r, fallen_horde, fallen_alliance from battles "
                                               "order by battle, day"):
            was = seen.get(battle, (0, 0))
            if fh > was[0] or fa > was[1]:
                fights[(d, battle)] = [d, q, r, fh - was[0], fa - was[1], 0]
            seen[battle] = (fh, fa)
    except sqlite3.OperationalError:
        pass
    hexes = {(d, b): (q, r) for d, b, q, r in rows(db, "select day, battle, q, r from battles")}
    for d, b in rows(db, "select distinct day, battle from battle_fights"):
        if (d, b) not in fights and (d, b) in hexes:
            fights[(d, b)] = [d, *hexes[(d, b)], 0, 0, 0]
    try:
        taken = [[d, q, r, 0, 0, 1] for d, q, r in rows(db, "select day, q, r from raids")]
    except sqlite3.OperationalError:
        taken = []
    fights = sorted(list(fights.values()) + taken)
    lands = ["Plains", "Woods", "Taiga", "Desert", "Shore", "Hills", "Water"]
    try:
        ground = [[q, r, lands.index(l) if l in lands else 0, round(w * 100), None if h is None else round(h), int(rv)]
                  for q, r, l, w, h, rv in rows(db, "select q, r, land, water, height, river from ground")]
    except sqlite3.OperationalError:
        ground = []
    return {"fights": fights, "ground": ground, "last": last, "firstWar": first_war, "places": places, "claims": claims, "battles": battles,
            "squads": squads, "supplies": supplies, "wagons": wagons, "series": series, "trade": trade, "tradedTo": traded_to, "events": events,
            "routes": [{"id": r, "path": paths[r]} for r in used],
            "trips": [[r, d, h, lt, ht] for r, d, _, h, lt, ht in trips if r in paths]}


if __name__ == "__main__":
    main()
