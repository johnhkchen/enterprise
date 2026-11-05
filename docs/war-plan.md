# The war for the region: Theramore's landing

John, 2026-11-04: "an Alliance navy/marine presence arrives to defeat the Horde defenders, who've thus far held an
advantage in regional military capabilities, hence a projection of power from an off-map Alliance capital. The goal
of the Horde defenders is to hold off being overwhelmed and contest the alliance incursion long enough that their own
town spread and brought-in mercenaries exceed the Alliance's spend limit. Alliance's goal is to tip the tide of Horde
presence in the area and wipe them out before the spend limit is reached." And: "both sides will get a backing
capital (Orgrimmar, Theramore) with its own mercenary and war chest. The side that wipes out the other side wins, but
Theramore brings a larger reinforcement to balance against the region's outnumbered Alliance and has limited spend
before it must strategically withdraw. Orgrimmar's ability to send help is more limited, aiming to exhaust the spend
by using its local numerical advantage to pin the Alliance down and attrit them, but will send mercenaries as needed
to avoid total defeat as well."

Settled (John's answers): refugees go to the **nearest friendly town** (none in reach: they leave the map); a side is
wiped out when it has **no towns left** in the region (its capital off the map doesn't count); Theramore lands at the
**Alliance port nearest the Horde**, its base; attackers hold a town **10 days** in a row to raze it.

Built on what stands: the battle for a hex (`battle.rs`), the run-up's procurement on the crown's behalf, the
barracks, war on the engine of people at work (`war.rs`), off-map markets and mercenaries (`realms.rs`, `crown.rs`).

## Two years

John: "We'll do 1 year of natural growth and expansion, with skirmishes and garrisons needed to found cities. Then the
run up and war phase in year 2."

- **Year 1, growth and the frontier.** Towns grow and settle as they do, but a settling party leaves with a
  **garrison**: a few soldiers of the founding town escort it and stay as the new town's guards (a town with no hands
  to spare them founds nothing). Where a Horde and an Alliance party want land near each other, their garrisons
  **skirmish over the frontier claim** (`war::clash`): the loser's party turns back, or its new town is lost.
- **Year 2, the run-up and the war.** Theramore declares; the run-up (towns arm on their capitals' behalf, barracks,
  smiths trained); Theramore lands, and the war is fought to a side's last town or Theramore's last emerald.

## Slices

0. **Garrisons and frontier skirmishes** (year 1): a settling party's escort from its town, its guards once
   founded; rival parties' garrisons clash for a claim near both, the loser turned back or its town lost.
1. **Names by people.** Each people's settlement names (orc, tauren, goblin, dwarf, night elf, Stormwind, Lordaeron,
   Dalaran), a new town named from its founders' list; Theramore and Orgrimmar reserved (the capitals). Core
   `settlers.rs` and the mod's `Names.java` alike.
2. **Backing capitals.** Theramore and Orgrimmar as off-map markets, each its faction's capital: a war chest
   (`war_chest_theramore`, `war_chest_orgrimmar` dials, emeralds), spent on its soldiers' pay, kit, food, ships and
   mercenaries, and on the public spending of its region's towns (the run-up's procurement, barracks): every emerald
   of it counted against its limit. Theramore's spent, it withdraws: its soldiers sail home and the Horde has won.
3. **Theramore's landing.** Its expedition (`expedition_size` marines, well kitted, Journeymen at arms) sails to the
   Alliance port nearest the Horde, its base and camp; reinforced as its chest allows.
4. **A town's hex fought for.** A battle can target a town: its defenders its levy at home, its walls and watchtower
   lifting their score, its guards; beaten and held `raze_hold_days` (5) by the attackers, it's razed: its people
   refugees to the nearest town of their faction within reach (else gone), its stock looted or lost, its works gone,
   its hex free to settle (by the winner's settlers, as a won hex).
5. **The two war councils.** Each day each side picks its fights from its strength and its chest:
   - **The Alliance** strikes the Horde town it can take soonest (nearest its base, weakest defended), and calls on
     Theramore's chest for marines while it has it.
   - **The Horde** pins the Alliance down where it's outnumbered (contesting its camp and the open hexes round it),
     raises its towns' levies, and calls Orgrimmar's mercenaries only when near total defeat (fewer than
     `horde_last_stand` towns, or losing every fight).
   - **Won:** a side with no towns left in the region; or Theramore's chest spent (it withdraws: the Horde's).
6. **Its tally and trace.** By day: each side's towns, soldiers, chest spent and left, towns razed, refugees; the host
   reports it, the trace has it (`war_days`, `razed`).

## Done (2026-11-04)

Slices 0 to 5 are in (`docs/decisions.md`: settlers' garrisons, names by people, the capitals' war chests, the war for
the region). The Horde's fight (`docs/decisions.md`, the Horde puts up a fight): every town of a town's defenders levied, their
edge at home and behind walls, marines made up at a cost, Orgrimmar's mercenaries after a losing streak. The Horde pins the Alliance at its camp whenever it outnumbers it,
and Theramore makes its marines up in the field at a cost. The capitals pay their towns' barracks and kit at a pace,
keeping a reserve for their soldiers. Theramore can win, hard-won: a ruin still counts against the town limit
(the Horde refounded every town razed), 50 marines, razed after 5 days held, a chest of 28,000. Still to do: the mod's side of a ruin.

## Squads on the hexes (2026-11-05)

John: "let's use hex-based squad movement. I imagine there's strategies that can be enacted against the deathball
strategy of the marines." Settled: **town squads** (each town's levy a squad marching from home; Theramore's marines
land as squads; squads on one hex fight as one); the Horde's moves: **raid the weak, cut supply, skirmish and fall
back, mass at a chokepoint, fortify and surround**; **core only** for now (the trace and the map show squads; the
game shows battles as it does).

1. **Squads that march** (done, 2026-11-05). A squad: its side, its home (a town, or a capital's convoy), its soldiers, its hex, its goal
   and its road there (`map` ways, the terrain's cost), `squad_hexes_per_day`. Hostile squads on one hex fight
   (`war::clash_lifted`, the squads of a side there as one army); a town's hex held by its attackers with none of
   its defenders on it `raze_hold_days` is razed. The two-army battle becomes the squads on its hex.
2. **The Alliance's council** (done, 2026-11-05). Its marines march as one (the deathball) on the Horde town it can take soonest; its
   towns' squads hold home.
3. **The Horde's council** (done, 2026-11-05), each day each free squad given a goal:
   - **Skirmish and fall back:** a squad facing worse than `skirmish_odds` steps back a hex toward home, not fighting;
     on good odds or ground it fights.
   - **Mass at a chokepoint:** the squads gather on the hex of the marines' road where the ground favours defenders
     (a river crossing, a hill, a forest), ahead of them.
   - **Raid the weak:** a squad strikes an Alliance town the marines are `raid_days` or more from, with few at home:
     the deathball turns back or the town falls.
   - **Cut supply:** squads astride the road from the marines to their base: an army cut off draws no food or kit
     from its camp (it goes hungry: `Army.unfed`).
   - **Fortify and surround:** a town's defenders behind walls hold (`defence_lift`); squads on `surround_sides` of
     the six hexes round the marines cut them off on every side, their spirits falling each day.
4. **Ground in a fight** (done): a hill, forest or river crossing lifts the side holding it.
5. **The trace and the map** (the trace's `squads` done; the map to do): `squads` by day (side, home, hex, soldiers, goal); the war map's frames show them.
6. **Tuned by replays:** `numbers_weight`, `skirmish_odds`, the convoys, against seed 1717's year. The aim (John,
   2026-11-05): "a single alliance town with 50 marines shouldn't be able to totally raze a region of horde towns. On
   the other hand, that's exactly what we might want to happen with sufficient material support from the Theramore
   navy and a heroic unit leading things": the defaults a long war Theramore can't finish alone; a richer chest,
   ships more often and a hero, a hard-won Alliance victory; neither side's impossible. And (John): "an evenly matched
   hero count with well supplied navy barely pulls off a clearing of the region ... a second added hero unit on the
   horde side results in a stalemate". Done (2026-11-05): `docs/decisions.md`, heroes, and a war Theramore can win only
   well supplied.
7. **Heroes** (John: "Both sides get one"; done). `hero_alliance`, `hero_horde` (how many; off by default): when
   Theramore lands, each for `hero_cost` of its chest, once. A side's heroes lead its largest force: each fights as
   `hero_worth` of its soldiers, lifts its morale `hero_morale`, keeps it from routing, and falls in a fight lost
   `hero_fall_odds` of the time, for good.

8. **Supplies carried** (John: "Are we keeping track of how much supplies troops are carrying and need?"; built
   2026-11-05 but foraging: `docs/decisions.md`, supplies carried). As it
   stands (2026-11-05): an army at a battle eats `army_ration` (8 points) a soldier a day and draws its kit straight
   out of its camp's warehouse, however far: nothing carries it, against "nothing crosses distance without a carrier".
   A squad on the march eats nothing. Cut off (surrounded, or posts on its camp's road), it draws nothing. On seed 1717
   the marines' camp is Pyrewood, 20 hexes and more from their assaults; its farms keep 800 to 3,000 food points in its
   warehouse through the war, so the marines are never hungry and their deep strikes cost no food.
   - **A pack:** each soldier carries `pack_days` of rations (food points on `Soldier`), filled at its camp or home
     when it sets out. On the march and at a battle it eats from the battle's depot first, then its pack; an empty pack
     is a hungry soldier (fights worse, deserts: `Army::day` as now). A squad's days of food left is its smallest pack.
   - **Supply wagons:** a battle's army eats from its depot (`Battle::depot`, a stock), not its camp's warehouse.
     The camp sends a wagon when the depot holds under `depot_days` of the army's need: loaded from its warehouse
     (`wagon_load` food points, arrows and kit), it travels the hexes on its planned way at a wagon's pace (`Map::going`
     with `Carrier::Wagon`: roads quicker, no steeps), with `train_escort` soldiers, and unloads into the depot.
   - **Cutting supply becomes raiding the wagons:** a hostile squad that meets a wagon fights its escort; if it wins
     it takes the load (to its own side's nearest depot or town) and the wagon is lost. The Horde's council posts
     squads on the wagons' way rather than only astride the road; surrounded, no wagon gets through, as now.
   - **The marines' rations come from both** (John, 2026-11-05): each convoy brings `convoy_rations` days of the
     expedition's food, paid of Theramore's chest, into its base's warehouse; Pyrewood's farms make up the rest.
   - **An army forages** (John: "enemy fields first"): standing on a hex of an enemy town's land it eats from that
     town's fields (`forage` points a day a soldier, the town's harvest cut by as much), then its depot, then its
     packs: a deep strike lives off the land a while, and the invaded economy shows it.
   - **The trace and the map:** each battle's depot and each squad's days of food by day; wagons on the hexes with
     their load, delivered or taken; the map draws the wagons and a squad's food as a bar.
   - **Retuned after:** the two hero targets replayed again (3 to 5 minutes each). Longer supply lines should slow
     the deep strikes, so the chest or `hero_worth` will move.
   - **Checks:** a squad's pack empties on a long march and it goes hungry; a wagon fills a depot; a raided wagon
     starves the army it fed; a surrounded army gets no wagon.

## Checks, sized to what they test

Unit tests for each slice; the study host's world for the councils (a few dozen days); one lane of seed 1717 only
when the whole is in, and short (John: "don't run so many long battles").
