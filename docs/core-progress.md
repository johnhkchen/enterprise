# Core progress: where the parity work stands

The live to-do list for building the Rust core to parity (the plan is `docs/core-plan.md`; how trees work is
`docs/worktrees.md`). Updated by the integrator at each landing. Last update: 2026-10-17, wave 1a landed and the
conflict magnets fixed; waves 1b to 5 running as one workflow.

## Now

- **Main:** `6a4e7e0`. 265 tests pass (`just core-test`, a few seconds). Parity: **100 covered, 20 partial, 46
  skipped, 96 left of 262** (`just parity`). `just core-fingerprint`: `f9fa99c2e718069a` (trading pairs across other
  towns' land, and towns reviewing their tolls).
- **Waves 1b to 5 run as one workflow:** each task gets its own tree when everything it waits for has landed, and
  landings are one at a time, the earliest in the plan's order first among those done.

## Waves 1b to 5

Models re-decided before starting (16 Opus, 16 Sonnet, no Haiku): Opus where the work is structural, cross-cutting or
subtle; Sonnet for well-specified ports. The plan's choice is in brackets.

| Order | Wave | Task | Model / effort (plan's) | Waits for | Landed | Notes |
|---|---|---|---|---|---|---|
| 9 | 1b | `bargain-and-par` | opus/high (sonnet/high) | `cash-book` | `fe48704` | 4 BargainTests pinned; `Prices` table; wagons sell at the bargained mid-batch price (dial `bargain`). Gaps: the sale's par is `shelf` (no 64 floor, no inputs term), seller level the home Merchant's (not the crew's). At landing: `set_trade` leaves recorded locker capacity alone |
| 10 | 1b | `advances-and-refunds` | sonnet/medium (sonnet/medium) | `cash-book`, `save-load` | `66af5d0` | 2 ContractTests pinned; CaravanTests advance partial (inputs follow is sourcing's). At landing: refunds before advances (mod's order), the seller's wagon collects at the buyer (was missing), both before bills |
| 11 | 1b | `road-food` | opus/high (opus/medium) | `cash-book`, `bargain-and-par` | `264e1fe` | Food is the road's cost where content has food: eaten from home, bought on the way, a purse for the far end's, packed where short, waits then short rations; 8 tags, short rations partial (no stocktakes). Gaps: one mouth, no crown reserve or shop buying, `keepForFood`. At landing: advances leave room for the road's food (they kept wagons home for good), the purse keeps it paying advances, the team's fodder (hay, spare wheat, else bought), `foodEaten` both ways |
| 12 | 2 | `lockers` | opus/high (sonnet/high) | `roster`, `cash-book` | `cead554` | structural: rostered towns' income; five later tasks build on it. 11 tags, 3 partial (shulker stock, nearest by hand index, `needed` set by hand). At landing: capacity one restock's worth a day as the mod (384 paper, was 768), a hand leaving takes its locker, restock after the farm, review after settlers |
| 13 | 2 | `squads` | opus/high (sonnet/high) | `roster`, `road-food` | — | threads a crew through Plan, Decide and Turn beside road-food |
| 14 | 2 | `farms-and-wages` | opus/high (sonnet/high) | `roster`, `cash-book` | `026b50d` | the largest port. Plan, wages, credit, sharecropping, labour market, fields, stores, deposits; 11 tags, 2 partial (block scan, tolls). Hooks: `Farm.needs`, `draws`, `stores_wanted`. At landing: every town spawns a farm (inert till set up), paid share back to 1 with no wage running, sales from the farm's start |
| 15 | 2 | `jobs` | sonnet/medium (sonnet/medium) | `roster`, `lockers` | `fee3be2` | The bag of marbles (supply, signals, idle plots, halls, comforts), the seeded draw, a 6-emerald stall beyond the reserve or on the households' savings, slack hands retrain where no road comes; both JobsTests tags pinned. Gaps: no site, a Merchant stands for the desk, `forContracts` is `Lockers::needed`, no training or squad gates yet. At landing: no stall while the town raises settlers or on a party's day (`Territory.grow`), after the farm and the census, `rules::free`; herds' bound +9 (late keepers' seeds) confirmed |
| 16 | 2 | `territory-growth` | sonnet/medium (sonnet/medium) | `hex-map`, `cash-book` | `4aa28bd` | Claims, culture to two rings, guards, militia, tolls; 7 tags. At landing: food all or nothing, tolls floored in spurs, posts by q then r, a first count a day's culture |
| 17 | 2 | `workshop` | sonnet/medium (sonnet/medium) | `roster` | `e57577f` | The mod's seven conversions by name, gain order, whole batches, `input_demand` two steps back; JobsTests paper pinned, comforts partial. Gaps: comfort worth not `Desires.reckon`, no room limit, runs after the touch's use. At landing: random worlds keep the base goods' comfort and food flags |
| 18 | 2 | `harvest-and-rents` | sonnet/medium (sonnet/medium) | `advances-and-refunds` | `e714595` | Harvest term judged on a window with shortfalls rolled, rents with coupons, `ForwardSales.offer`, set-aside from sales and the workshop; 2 ContractTests pinned, the rents deed partial (no Numismatics vendor, dealer on synthetic inputs). Gaps: no crates/earmarks, `RentDealer.deal`, `Accountant.approve`. At landing: rotation crops only (was cane and berries too), both towns' money beyond `reserve` (was trading capital), buy-back keeps the road home's food, `forward_advance_share` dial, base generator left alone |
| 19 | 2 | `atlas-and-books` | opus/high (sonnet/medium) | `hex-map`, `squads` | — | stocktakes change what every plan reads |
| 20 | 2 | `herds` | sonnet/medium (sonnet/medium) | `roster`, `workshop` | `fab3ed5` | All 8 HusbandryTests pinned; seed, feed owed whole, growth to the pasture, hunger, shearing, culls. Gaps: no fence or entities, first tending not skipped; random worlds' sheep goods replace cookie and book in half the seeds. At landing: products to `Farm.pending`, feed to `Farm.feed` (demand and needs), after the farm accrues, `MAX_DAYS` cap, rosterless towns tended, cows culled by kind |
| 21 | 3 | `chokepoints` | opus/high (opus/medium) | `territory-growth` | `6a4e7e0` | Crossings and their post options, `best_plan` every 3 days (80%/95% of the detour, 25%/35% caps, a fee to 3 emeralds), one guard hired a review, `bridge_value`, `Takings`; all 8 TollTests tags pinned. Gaps: bare routes unseen, fractional spurs, `set_toll` still clamps to the constant (mines-and-kit: armour needs territory_growth's check raised to `ARMOURED_TOLL_RATE`). At landing: one road-food price (`map::per_hex_at_bread`), the wagon test fed as main's road-food asks |
| 22 | 3 | `services` | opus/high (sonnet/high) | `squads`, `herds`, `farms-and-wages`, `atlas-and-books` | — | a registry over seven Java files that later tasks read |
| 23 | 3 | `mines-and-kit` | sonnet/medium (sonnet/medium) | `territory-growth`, `services` | — |  |
| 24 | 3 | `desires-and-backhauls` | sonnet/high (sonnet/medium) | `bargain-and-par`, `atlas-and-books` | — |  |
| 25 | 3 | `crates-and-papers` | opus/high (sonnet/medium) | `harvest-and-rents`, `advances-and-refunds` | — | changes what cargo is, through arrive and plan |
| 26 | 3 | `freight-hubs` | opus/xhigh (opus/high) | `crates-and-papers`, `atlas-and-books`, `territory-growth` | — |  |
| 27 | 3 | `sourcing-and-procurement` | sonnet/high (sonnet/high) | `lockers`, `atlas-and-books`, `services` | — |  |
| 28 | 3 | `war-army` | opus/xhigh (opus/high) | `roster`, `mines-and-kit` | — |  |
| 29 | 3 | `bank-and-discounts` | opus/high (sonnet/high) | `advances-and-refunds`, `crates-and-papers` | — | finance hooks into Settle and Collect |
| 30 | 4 | `post` | opus/high (sonnet/high) | `freight-hubs`, `bank-and-discounts` | — | over hubs, bills and the bank |
| 31 | 4 | `training-and-tenders` | sonnet/medium (sonnet/medium) | `lockers`, `crates-and-papers`, `jobs` | — |  |
| 32 | 4 | `player-orders` | sonnet/medium (sonnet/medium) | `lockers`, `desires-and-backhauls`, `atlas-and-books` | — |  |
| 33 | 4 | `realms` | sonnet/medium (sonnet/low) | `squads`, `road-food` | — |  |
| 34 | 4 | `buildings-and-works` | sonnet/medium (sonnet/medium) | `chokepoints`, `crates-and-papers`, `cash-book` | — |  |
| 35 | 4 | `crown` | opus/high (opus/medium) | `war-army`, `mint-rate`, `sourcing-and-procurement`, `realms`, `training-and-tenders` | — |  |
| 36 | 4 | `stranded-paper` | sonnet/medium (sonnet/medium) | `bank-and-discounts`, `freight-hubs`, `training-and-tenders` | — |  |
| 37 | 4 | `settlers-rule` | sonnet/medium (haiku/low) | `roster`, `hex-map` | `5830058` | Parties to the nearest village or open ground, capital off the books on the road; ExpeditionTests settle partial. Gaps: spare is `rules::free` (no wage float), no `inRange`, road every 4 days the core's. At landing: Nitwits stay home, only a Merchant's town sends, only the nearest village (as `findNearestMapStructure`), new towns' land 40 not 4, claimed hexes not open |
| 38 | 5 | `random-worlds-sweep` | opus/high (sonnet/medium) | everything above | — | reads every slice; finds what they broke together |
| 39 | 5 | `fixed-point-money` | opus/xhigh (opus/high) | `random-worlds-sweep` | — |  |
| 40 | 5 | `parity-report` | sonnet/medium (haiku/low) | `fixed-point-money` | — | haiku only for trivial edits |

## Wave 1a: all 8 landed

| Task | Model | Landed | Notes |
|---|---|---|---|
| `look-steps` | haiku | `769ce90` | Steps Work, Pay, Review, Mail. Bevy chains at most 20 sets: two chains joined at Sell |
| `parity-triage` | haiku | `aa7270e` | Skip list (46 lines, 47 tests); two trade-loop tests (one tagged partial: no crates yet) |
| `contract-tags` | haiku | `793a47e` | Five ContractTests; `Incoterm::price` (no caller yet) |
| `mint-rate` | haiku | `1e8465f` | `Mint` resource; fixed the same-seed test (it compared nothing) |
| `save-load` | opus | `69241f1` | One blob (postcard); dials by name; random worlds save and load at day 60. The mint added to the save at landing |
| `roster` | opus | `8d621f9` | Hands with trade, level, XP; babies; villagers follow it. Roster added to the save at landing |
| `hex-map` | opus | `46d39b2` | Hexes, ground, A* roads, tolls as in `postToll`. Map and `RoutePath` added to the save at landing |
| `cash-book` | opus | `a318a84` | Every transfer a posting; the Accountant. At landing: births post and push the roster's baby; `post_legs` so road food and purse leave as one sum (bit-identical worlds); the book saved |

## Integrator checks at each landing

Read the diff and the agent's summary; check each `parity:` tag really pins its Java test (mark `parity-partial:`
otherwise); check new state is in `saved!` (`tests/save.rs` fails otherwise) and money moves through `post`; no
`Cargo.toml` changes; `just tree-land NAME` (rebases, gates, pushes); `just tree-rm NAME`; update this file.

## Fixed after wave 1a: the conflict magnets

Every 1a landing conflicted in the same places. Now (`24bfc0b`): the lists slices add to are files of their own merged
by union (`sim/core/src/slices.rs` with `saved!` and the `register` calls, `dials.rs` one dial a line, `lib.rs`,
`tests/common/slices.rs`, `parity-skip.txt`); a slice's part of the random worlds is a file of its own with a random
stream of its own; host calls go in the slice's own `impl Society`; `tests/save.rs` fails while state isn't saved;
dependencies are the integrator's; `trees.py` locks its registry, so agents can make trees at the same time.

## Parked

- `park/warp` (branch): a fast-forward mode for the NeoForge server, untested. For after parity, when the binding
  work brings the Java server back.

## Lessons from wave 1a

- Haiku did the mechanical tasks well and fast (1 to 3 minutes each); Opus the structural ones in 6 to 8 minutes.
- Agents flagged their own weak spots honestly (a tag that claimed too much; a float-sensitive test); read their
  summaries before landing.
- Slices branched before a structural one landed miss its conventions (the save list): land structural tasks first,
  and check new state against `saved!` at landing (now a test).
- "Bit-identical" needs a fingerprint, not a claim: cash-book's own check passed on its old base, but on main a split
  debit moved one town's money by an ulp. `just core-fingerprint` before and after.
- Name the unit wherever a brief floors money: the core counts emerald-worth, the mod floors tolls and fees in spurs
  (1/16 emerald), so `floor()` on the core's money is 16 times coarser (`territory-growth` tolled small purses nothing).
- A brief that names a Java method as the reference gets ported a half at a time: `payAdvances` has two loops (the
  buyer's wagon pays, the seller's wagon collects) and the second was missing. Briefs list each branch of the
  reference, and its call order against its neighbours in the step (`settleOwing`, `payAdvances`, `presentBills`).
- Iterate towns in `Town.id` order where floats add into shared totals: the look's queries go in archetype order, so a
  component inserted on some towns mid-run reorders them against a loaded world and load-then-run differs in the last
  bit (`bargain-and-par` gave every town `LockerCapacity` from `add_town` to dodge it). Briefs: spawn a town's
  components in `add_town`, never insert one later.
- A slice's random-world `extend` may rename the base goods (conversions match by name) but never rewrite their
  flags: `workshop` made every good plain but cookie, book and bread, and a third of the worlds lost all food and
  comforts for every slice. Briefs: rename, or add, never overwrite what the base world drew.
- A slice that spawns towns or routes mid-run needs Bevy's `Schedules` resource made up front (`Society::new` and
  `save::restore` now do, from `settlers-rule`): made at the first run, it took an entity index a load closes, and a
  loaded world saved a different blob.
- `load_then_run_equals_never_saved_to_the_bit` catches the archetype trap only for components the generator sets:
  `farms-and-wages` passed in its tree, and failed after rebasing (its farms inserted after `add_town`, on main's
  more varied worlds). A slice's component goes on every town in `add_town`, inert until a host call sets it up.
- Briefs written before a neighbour lands go stale on it: `settlers-rule` left out the mod's territory check because
  claims didn't exist when it was briefed, and they had landed by its landing. At landing, grep the brief's "not in the
  core yet" against main.
- A brief's formula can be wrong against the mod: `lockers`' said capacity is count × uses × 2 restocks, the mod's
  `canTake` counts one restock as a day (decisions.md: a Librarian's 384 paper), and the slice ported the brief. At
  landing, check a brief's numbers against the Java and `docs/decisions.md`, not only the tests against the brief.
- An agent's "not ported, so left out" can halve a gate: `road-food` dropped the team's fodder from the trip's cost
  because fodder wasn't ported, and every food world's wagons went with half the load the mod asks. And a new cost
  must reach every place that sizes money against the old one: advances still left room for `road_cost`, not the
  road's food. Briefs that replace a cost: grep the old one's every reader.
- State kept by roster index must follow `settlers::take_hand` (it shifts every later hand): `lockers` now has
  `Lockers::take_hand`, called beside it. Briefs for `jobs`, `training-and-tenders` and anything else keyed by hand:
  either the same, or a stable hand id.
- Agents reach for the nearest helper, not the mod's: `harvest-and-rents` reckoned `territory.reserve` as trading
  capital alone (the core has `rules::farms::reserve`, with the wage float) and `Territory.ROTATION` as all field
  crops (cane and berries too; now `farms::rotation`). Briefs name the core's counterpart of each Java helper they
  cite. And a slice whose rule changes other slices' worlds changes the fingerprint: it doesn't add a switch to the
  base generator to hide it.
- Agents port the Java method and miss its caller's gates: `Jobs.employ` is called from `Territory.grow` only when the
  town isn't raising settlers, has no hand to train and no squad short, and `jobs` hired the hands a party was waiting
  for. Briefs name the reference's call site and each condition around it.
