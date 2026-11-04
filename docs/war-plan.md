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
   lifting their score, its guards; beaten and held `raze_hold_days` (10) by the attackers, it's razed: its people
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
keeping a reserve for their soldiers. Still to do: the mod's side of a ruin.

## Checks, sized to what they test

Unit tests for each slice; the study host's world for the councils (a few dozen days); one lane of seed 1717 only
when the whole is in, and short (John: "don't run so many long battles").
