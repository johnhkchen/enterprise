# Core progress: where the parity work stands

The live to-do list for building the Rust core to parity (the plan is `docs/core-plan.md`; how trees work is
`docs/worktrees.md`). Updated by the integrator at each landing. Last update: 2026-10-17, wave 1a landed and the
conflict magnets fixed; waves 1b to 5 running as one workflow.

## Now

- **Main:** `37f56f9`. 646 tests pass (`just core-test`, about 2.5 minutes, the random worlds' 300 seeds across the
  cores). Parity: **191 covered, 25 partial, 46 skipped, 0 left of 262** (`just parity`; `docs/parity.md` lists what
  each partial leaves out). `just core-fingerprint`: `8144f9be7947133b` (money in thousandths; `just core-wasm-replay`
  checks the WebAssembly build's 200-day digest against the native one); `cargo run --release -p enterprise-sim --
  --sweep 300` prints what the random worlds did; `just study 365` runs the network study's world.
- **Speed:** `perf-day` landed `f92903b`, about half the cost a day with the fingerprint unchanged (study world 4.36 to 2.09 ms a day over 10 seeds, toy 10,000 days 6.73 to 2.73): lockers' wants for every good in one pass, the services list and recipes cached in a `Local` rebuilt when `Content` changes (change `Content` only through bevy, or those caches go stale), comforts and set-asides in one pass. Then `plan-on-departure` landed `37f56f9` (the user's call): an idle route plans only when the mod's `poll` would try it (a day past its last departure and past `recheck_at`, a quarter day on after a wait or an unfunded trip, `Caravans` 173/374/438), routes with no Merchant or squad turned away unplanned (`squad::unmanned`), and a plan's reasons booked only for a wagon that goes (`dispatch` 433-441; a stayed try books its "no trip"). Study world 2.10 to 1.26 ms a day; its 365 days 419 to 258 trips, S1->hub's trips hung on a shopping purse that counts only while S1 is a cookie short, a flicker 20 tries a day caught. Open: whether a whole shopping purse should count as a trip's worth; turn-aways with no recheck tried once a look (the mod: every 100 ticks).
- **Native and WebAssembly agree again:** `wasm-digest` landed `c8fbc0a`. Bevy broke ties between unordered systems differently on wasm32 (`stranded::load` against `atlas::depart` lost stranded crates' Sent lines), so all 32 ambiguous pairs are pinned as they ran natively and `tests/schedule.rs` fails on any new unordered pair touching the same state. Next: pallets (`docs/pallets.md`) and the binding (`docs/binding-plan.md`) run as one workflow; the binding's progress in `docs/binding-progress.md`.
- **The plan is done (2026-10-18):** all 32 tasks of waves 1b to 5 landed, the workflow's 63 agents with no errors.
- **After the plan, the study world** (`sim/host`, the network study's hub, S1-S4 and T): `study-findings` landed
  `76ea783` (wagons bring what the far end's switched-on trades take, loads sized and judged as `pickCargo`/`saleLoad`,
  one wagon a route a day, the farm grows for the lockers and the workshop, ties on roads as the mod's heap, T in its
  hex, roads as long as `Route.distance`; 365 days: 0 to 419 trips). Still open: the core sends the settlements far
  more than the Java study did (S1 22,276 items by day 40 against 5,675), `marketWants`, roads planned once not at each
  departure, coins and the mint (rate stuck at 16), `Places::distance` hex centres not desks.

## Waves 1b to 5

Models re-decided before starting (16 Opus, 16 Sonnet, no Haiku): Opus where the work is structural, cross-cutting or
subtle; Sonnet for well-specified ports. The plan's choice is in brackets.

| Order | Wave | Task | Model / effort (plan's) | Waits for | Landed | Notes |
|---|---|---|---|---|---|---|
| 9 | 1b | `bargain-and-par` | opus/high (sonnet/high) | `cash-book` | `fe48704` | 4 BargainTests pinned; `Prices` table; wagons sell at the bargained mid-batch price (dial `bargain`). Gaps: the sale's par is `shelf` (no 64 floor, no inputs term), seller level the home Merchant's (not the crew's). At landing: `set_trade` leaves recorded locker capacity alone |
| 10 | 1b | `advances-and-refunds` | sonnet/medium (sonnet/medium) | `cash-book`, `save-load` | `66af5d0` | 2 ContractTests pinned; CaravanTests advance partial (inputs follow is sourcing's). At landing: refunds before advances (mod's order), the seller's wagon collects at the buyer (was missing), both before bills |
| 11 | 1b | `road-food` | opus/high (opus/medium) | `cash-book`, `bargain-and-par` | `264e1fe` | Food is the road's cost where content has food: eaten from home, bought on the way, a purse for the far end's, packed where short, waits then short rations; 8 tags, short rations partial (no stocktakes). Gaps: one mouth, no crown reserve or shop buying, `keepForFood`. At landing: advances leave room for the road's food (they kept wagons home for good), the purse keeps it paying advances, the team's fodder (hay, spare wheat, else bought), `foodEaten` both ways |
| 12 | 2 | `lockers` | opus/high (sonnet/high) | `roster`, `cash-book` | `cead554` | structural: rostered towns' income; five later tasks build on it. 11 tags, 3 partial (shulker stock, nearest by hand index, `needed` set by hand). At landing: capacity one restock's worth a day as the mod (384 paper, was 768), a hand leaving takes its locker, restock after the farm, review after settlers |
| 13 | 2 | `squads` | opus/high (sonnet/high) | `roster`, `road-food` | `4e37ea3` | Squads crew rostered towns' wagons: mustered route by route at Decide as `dispatch`, hire and training, porters, Leatherworker packs, saddles, no desk Merchant no wagon; 12 tags, 2 partial (`wantedAbroad` a host hook; the porter day looped). Water, fleet, herds, crew haggle not here. At landing: crates counted aboard for porters and `lastHome`, clip keeps crates' stacks, fetch by the crew's slots, hire at 1 xp, wanted stacks at 64, settlers' home keeps its Mason (works) |
| 14 | 2 | `farms-and-wages` | opus/high (sonnet/high) | `roster`, `cash-book` | `026b50d` | the largest port. Plan, wages, credit, sharecropping, labour market, fields, stores, deposits; 11 tags, 2 partial (block scan, tolls). Hooks: `Farm.needs`, `draws`, `stores_wanted`. At landing: every town spawns a farm (inert till set up), paid share back to 1 with no wage running, sales from the farm's start |
| 15 | 2 | `jobs` | sonnet/medium (sonnet/medium) | `roster`, `lockers` | `fee3be2` | The bag of marbles (supply, signals, idle plots, halls, comforts), the seeded draw, a 6-emerald stall beyond the reserve or on the households' savings, slack hands retrain where no road comes; both JobsTests tags pinned. Gaps: no site, a Merchant stands for the desk, `forContracts` is `Lockers::needed`, no training or squad gates yet. At landing: no stall while the town raises settlers or on a party's day (`Territory.grow`), after the farm and the census, `rules::free`; herds' bound +9 (late keepers' seeds) confirmed |
| 16 | 2 | `territory-growth` | sonnet/medium (sonnet/medium) | `hex-map`, `cash-book` | `4aa28bd` | Claims, culture to two rings, guards, militia, tolls; 7 tags. At landing: food all or nothing, tolls floored in spurs, posts by q then r, a first count a day's culture |
| 17 | 2 | `workshop` | sonnet/medium (sonnet/medium) | `roster` | `e57577f` | The mod's seven conversions by name, gain order, whole batches, `input_demand` two steps back; JobsTests paper pinned, comforts partial. Gaps: comfort worth not `Desires.reckon`, no room limit, runs after the touch's use. At landing: random worlds keep the base goods' comfort and food flags |
| 18 | 2 | `harvest-and-rents` | sonnet/medium (sonnet/medium) | `advances-and-refunds` | `e714595` | Harvest term judged on a window with shortfalls rolled, rents with coupons, `ForwardSales.offer`, set-aside from sales and the workshop; 2 ContractTests pinned, the rents deed partial (no Numismatics vendor, dealer on synthetic inputs). Gaps: no crates/earmarks, `RentDealer.deal`, `Accountant.approve`. At landing: rotation crops only (was cane and berries too), both towns' money beyond `reserve` (was trading capital), buy-back keeps the road home's food, `forward_advance_share` dial, base generator left alone |
| 19 | 2 | `atlas-and-books` | opus/high (sonnet/medium) | `hex-map`, `squads` | `7ba9c64` | stocktakes change what every plan reads. Atlas by word of mouth and wagons, a stocktake per market (fresh 3 days, else read live), books and the reasons' tally, stale books want a Librarian, commissions at the reviews; 3 tags pinned, 2 partial (the Librarian's far end read live, no "Stop caravans"). Gaps: comforts still sized on live stock, home's stocktake only when a wagon leaves, approximate unsold reasons, every town considered (not settlements only). At landing: the far end's room counts crates on their way (a crated load went twice), `wagon_wait_days` a dial, realms' far ends merged into the new plan, three Bevy orders pinned as main ran them |
| 20 | 2 | `herds` | sonnet/medium (sonnet/medium) | `roster`, `workshop` | `fab3ed5` | All 8 HusbandryTests pinned; seed, feed owed whole, growth to the pasture, hunger, shearing, culls. Gaps: no fence or entities, first tending not skipped; random worlds' sheep goods replace cookie and book in half the seeds. At landing: products to `Farm.pending`, feed to `Farm.feed` (demand and needs), after the farm accrues, `MAX_DAYS` cap, rosterless towns tended, cows culled by kind |
| 21 | 3 | `chokepoints` | opus/high (opus/medium) | `territory-growth` | `6a4e7e0` | Crossings and their post options, `best_plan` every 3 days (80%/95% of the detour, 25%/35% caps, a fee to 3 emeralds), one guard hired a review, `bridge_value`, `Takings`; all 8 TollTests tags pinned. Gaps: bare routes unseen, fractional spurs, `set_toll` still clamps to the constant (mines-and-kit: armour needs territory_growth's check raised to `ARMOURED_TOLL_RATE`). At landing: one road-food price (`map::per_hex_at_bread`), the wagon test fed as main's road-food asks |
| 22 | 3 | `services` | opus/high (sonnet/high) | `squads`, `herds`, `farms-and-wages`, `atlas-and-books` | `1ab4f22` | a registry over seven Java files that later tasks read. The registry in the mod's order with its recipes and kit table; home blessing, charting and crafts; trips take the trades the far end wants; blessing, harvest help, map swaps, surveys, droving and crafts abroad, paid into the purse, trust +0.1 each; 7 tags pinned, 3 partial (the Shepherd's pen, the Cartographer's map item, the hoes' kit share). Gaps: mending, mines, hunting, docks, blast smelting, master services; the budget's reserve lacks the guards' 3 days; animals beyond the pasture's room let go; towns along a road any placed town. At landing: home services before training's lessons, plan after `want_books`, drive after `atlas::depart` (Bevy's order pinned, fingerprint unchanged), droving's pasture only the driven kinds |
| 23 | 3 | `mines-and-kit` | sonnet/medium (sonnet/medium) | `territory-growth`, `services` | `b64f097` | Mines dug and sunk deep, ore by Toolsmith level, hands gathering by `Land`, Fletchers hunting, kit wearing out (mended, smithy, armoury, Unbreaking), shares, armed and armoured guards, `Farm.hands`; all 8 tags pinned, the services' hoe share upgraded to full. Gaps: work once a day on whole days, no mine site, deep mine after every home service (not in registration order), gatherings owed apart from the farm's (no `storeFor`), mending/armoury/enchanting services the host's. At landing: hands work hexes by q then r (were as claimed), the Fletcher test on the mod's Novice, deep mine and shares before the lockers' buying and trades (ran after), two Bevy orders pinned as they ran, `Kits` in the works' insert |
| 24 | 3 | `desires-and-backhauls` | sonnet/high (sonnet/medium) | `bargain-and-par`, `atlas-and-books` | `3464146` | `will_pay`/`reckon`, outlets and `worth_buying`, whole comforts paid from beyond the reserve, comforts loaded first at half again the stocktake's want less use since, shopping purses, surplus and glut bought for the trip home, shop listings as news; 7 tags pinned, the shop partial (vendor's Minecraft's). Gaps: ex-works funded before shopping (the mod: shopping first), shopping money counts toward worth only where the far end holds the kind, an uncounted market read live (mod: whole par wanted), listings only for locker goods, no crown reserve, bought goods loose. At landing: a glut bought for home's food only where home's food is known short (`foodKnownAt`: an untimed stocktake is plenty), the bank's float sees the shopping allowance, shop kept after the last reviews and buying after porters go home |
| 25 | 3 | `crates-and-papers` | opus/high (sonnet/medium) | `harvest-and-rents`, `advances-and-refunds` | `54d36bc` | changes what cargo is, through arrive and plan. Shipments with bills of lading, empties, earmarks, paperwork and scribes, `settleOwing` both ways; 6 tags, 4 partial (one good a crate, `crate_room` for chest slots, clerks are buildings-and-works'). Gaps: no paperwork for drafts or bills, empties take no slots, stored goods plain stock. At landing: spare empties and crates made out home first at the far end (the mod's order), Ex Works earmarks left to the paid collection, scribes' fees booked in full |
| 26 | 3 | `freight-hubs` | opus/xhigh (opus/high) | `crates-and-papers`, `atlas-and-books`, `territory-growth` | `9063831` | Hubs emerge at the review, sell space by the stack, book crates bound on and buy blocks, cart with idle hands; roads call at hubs; routes reorganise by price; both CaravanTests tags pinned. Gaps: no shop tills or vendor slot cap, no consignment/credit, `copiesVia`, stranded paper (held crates re-sent instead), rents at hubs; bare routes pass no hub; a hub's runs keep their own timetable, not `liner_days`; the day's later hubs see a wound-down run. At landing: a long run's usual load is sold, delivered and bought a trip (`itemsDelivered + itemsBought`, was loaded out), its tolls reckoned ahead as `tollShare` and `passageFees`, the landing unrounded; `Hub` inserted with the works (its own insert moved main's worlds) |
| 27 | 3 | `sourcing-and-procurement` | sonnet/high (sonnet/high) | `lockers`, `atlas-and-books`, `services` | `f188b8b` | Sourcing (nearest known holder with a road, Ex Works or Delivered At Place, trial lots, lots to make a third up front, copies ride the wagons), procurement capped at what's due in two days, contracts' inputs raise the par and go first, the capital's metal exchange; 5 tags pinned, the advance's inputs upgraded to full, none partial. Gaps: no hub means or armoury, orders and the crown's shortfall not among the wants, `makes` from lockers or the trade's defaults, signals for wants no road supplies (mod: no known holder), inputs' cargo share only for inputs, `drafted` only on sourced contracts. At landing: its systems ordered as they ran and main's two orders they moved pinned back, a town that stops brokering drops its signals, no second draft of a good from one supplier in a review |
| 28 | 3 | `war-army` | opus/xhigh (opus/high) | `roster`, `mines-and-kit` | `90861f6` | The army at the front as a population: eats heartiest first, draws and wears kit, falls and deserts at expected rates carried (no dice), the fallen off their roster and their town grieves, pay 0.25 a soldier-day; all 6 tags pinned. Gaps: no theatre (`enchant_level` is the enemy-suited enchantment), recruits to the front by `embark`, levy terms and the levy's trade rule the crown's, pay outside a levy in `Army.pay_owed`, a fallen carry left over lands on later recruits. At landing: kit wears out on the mod's raw sum against 1 (doubles' dust: an iron sword lasts 11 days, was 10), levy pay only where the contract is home's with the capital, fed share floored as the mod's, kit drawn by most enchantments, foods' equals by name |
| 29 | 3 | `bank-and-discounts` | opus/high (sonnet/high) | `advances-and-refunds`, `crates-and-papers` | `9d4f2d4` | Banks discount a town's bills, lend against its contracts and are repaid from the payment first; the hub's discount desk; grace scaled by debt; bills no one can carry lapse; all 4 tags pinned. Gaps: no spurs, Ex Works purse shrunk by the loan (mod draws a bill for the shortfall), hubs and realms hooks, float spare against every plan this look. At landing: training contracts lend on their bonus, the float counted spent at Decide without a second allow, the crew's mouths, `plan_papers` pinned as main runs it |
| 30 | 4 | `post` | opus/high (sonnet/high) | `freight-hubs`, `bank-and-discounts` | `8311493` | over hubs, bills and the bank. Letters on the post days, bills and copies by post where no regular run joins the two, a hub's calls (one a day) and runs on call, the forwarder's bookings, collections and consignment, remitted by post; all 6 PostTests pinned, the copy by post partial (posted only with no road; the mod posts whatever runs, and bills a post round past due). Gaps: crown's aid only with a capital, collection fare ×1.5 on call, no `collectForwarding` crates, porters ignore runs on call. At landing: only a run on call carries a call's purse and has its calls made (a regular run's go as it leaves), runs on call don't stop copies and bills going by post, remits need only the coin spare, the collection's fare on the towns' distance, routes in order where the post, shopping, bank float and road food sum (a load reordered them), lockers' review pinned before the works' |
| 31 | 4 | `training-and-tenders` | sonnet/medium (sonnet/medium) | `lockers`, `crates-and-papers`, `jobs` | `4839983` | Training as a contract judged on the roster, the Merchant's lessons, offtake crates with a bill, tenders read off boards by wagons (taken, carried on, lapsed and reposted +25%); 5 tags, none partial. Kept on purpose against the mod: a carried tender stays till its carrier's gone, uncollectable work stays in stock; Sovereign's shortfall repost is the crown's. At landing: its random worlds switched on (were commented out), free slots count crates, room as `hasRoomFor`, steps ordered |
| 32 | 4 | `player-orders` | sonnet/medium (sonnet/medium) | `lockers`, `desires-and-backhauls`, `atlas-and-books` | `efbb559` | A master Merchant fills players' buying shops under Ex Works contracts (trust per shop), asks the region for the rest, buys from selling shops, stocks the town's own; the desk's signals one list, the only stream to the jobs; all 5 tags pinned. Gaps: `wantedAt` without conversions or realm wants, no `roomFor` or commissions. At landing: a repriced shop keeps its contract, par floored at contract inputs (wanted goods), own shop keeps its price as the rate moves, marbles at `count*spursEach/rate`, whole lots only, largest shop's shortfall sourced, lockers' review pinned after territory's, off-map wagons don't consign at hubs (post's remit beyond the border) |
| 33 | 4 | `realms` | sonnet/medium (sonnet/low) | `squads`, `road-food` | `ff18f19` | Markets off the map (sell no food unless told), the Coast and the March (make, use 64 a day of what they want, buy by par, wagons at level 4, no squad), customs to a named capital; both tags pinned. Gaps: no warehouse slots, partners and Atlas the host's, `Crown` a stub for `crown`, demand in `Trades.capacity` (a later `record_capacity` wipes it), planners size a realm wagon's money on one mouth. At landing: a realm's wagon never waits for a load, two mouths' food at dispatch but eats as one (`Caravan.mouths`), no realm hands mustered, customs before redeem and relot, the foreign test on the mod's 2 bread, banks' random picks on the map, two Bevy orders pinned as main ran them |
| 34 | 4 | `buildings-and-works` | sonnet/medium (sonnet/medium) | `chokepoints`, `crates-and-papers`, `cash-book` | `58b610c` | masons' wages and blocks, bridges that repay, walls, the ten halls (hired builders without a Mason), flower beds; wall, square, flowers, counting house, granary and libraries take effect; 6 tags, 1 partial (flower beds a flag). Gaps: hall sites always found, other halls' effects for later slices, `farms::reserve` lacks the guards' 3 days. At landing: bridge, wall and hired halls weighed at the toll reviews (first only starts the clock), masons paid after guards and before farm wages, `set_culture_extras` keeps the works' share |
| 35 | 4 | `crown` | opus/high (opus/medium) | `war-army`, `mint-rate`, `sourcing-and-procurement`, `realms`, `training-and-tenders` | `ec52412` | The sovereign: a capital minting its revenue into the treasury, requisitions bid and awarded whole lots with bonus clauses, training tenders, levies on the towns its wagons reach, mercenaries over the sea, the front's daily orders, peace; 3 tags pinned, the mobilising test partial (its gold shop is Numismatics'). Gaps: bonuses not paid on delivery (cargo carries no enchantments), front orders free and at once, no ration store kept from road food, no hub boards, no barracks beds, every standing levy closed at peace, the campaign before the front's orders. At landing: a levy keeps last the only hand of a trade the town's contracts deliver (`Apprentices.neededTrades`), territory_growth's toll check allows armoured guards' higher cap (the crown's worlds name armour again), `war_tally` (player orders took `tally`), bridges weighed over routes in order (a load flipped one) |
| 36 | 4 | `stranded-paper` | sonnet/medium (sonnet/medium) | `bank-and-discounts`, `freight-hubs`, `training-and-tenders` | `67f4377` | the tagged test pinned; a hub buys stranded paper for coin and installments, freight on credit, the pupil's work waits for a hub; crates sold before the hand-over (desk first, then the stranded sale: declared); `Bills.tidy`, `Forwarder.collect` not ported; a longer bill's interest scaled by its terms (installments, 30-day credit at 15%), fixed at landing |
| 37 | 4 | `settlers-rule` | sonnet/medium (haiku/low) | `roster`, `hex-map` | `5830058` | Parties to the nearest village or open ground, capital off the books on the road; ExpeditionTests settle partial. Gaps: spare is `rules::free` (no wage float), no `inRange`, road every 4 days the core's. At landing: Nitwits stay home, only a Merchant's town sends, only the nearest village (as `findNearestMapStructure`), new towns' land 40 not 4, claimed hexes not open |
| 38 | 5 | `random-worlds-sweep` | opus/high (sonnet/medium) | everything above | `db7edea` | reads every slice; finds what they broke together. Seeds across the cores (`each_seed`: suite 11:24 to 2:25, fingerprint unchanged), sweep invariants daily, save round trip every 30 days with a one-day twin, `sim/host --sweep N`; partial: 10 s budget (38 s), the stall check is only "says why", mood checked against a floor not -8, the contract check amounts to "judged within a day", the same-seed test still 3 seeds; at landing: `--sweep` merged into the new study host, docs say what the checks amount to |
| 39 | 5 | `fixed-point-money` | opus/xhigh (opus/high) | `random-worlds-sweep` | `6765f0f` | money a `Fixed` (i64 thousandths) everywhere it's held, moved or owed, rounded per the mod's site rule (payer up, share down, host figure nearest), check.rs exact; the curves from + - * / alone; `tests/replay.rs` digest matches in wasm32-wasip1 under Node (`just core-wasm-replay`); partial: goods, rates and per-look accruals stay f64; at landing: training's second outcome expected 40 - ceil(40/3), wasi-run.mjs quiets its warning |
| 40 | 5 | `parity-report` | sonnet/medium (haiku/low) | `fixed-point-money` | `8e80e7e` | `docs/parity.md` from `parity.py --md`: every partial with what it leaves out (27 bare tags given reasons), reasonless partials and skips warned of, a trade_loop mis-tag dropped. At landing: 191 of 216 covered, the target met: three partials that pinned the mod's numbers promoted (growth, flooded hex, fisherman), the porter test pins the day it goes, crowding pinned with the mod's 8 in 3 beds (the unpaid-wages tag was wrong); header claims computed, fully covered tests left out of the partial table; haiku only for trivial edits |

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

- `parity-partial:` is for what the Rust test leaves out of the Java test's assertions, not for Minecraft's stand-ins:
  agents marked partial tests that pinned the mod's numbers with ground set on the map or a roster hand for an entity
  (three promoted at `parity-report`), and one tagged a test the Java's name promised but its assertions never made
  (unpaid wages). Check a partial against the Java's `assert` lines, not its name.
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
- A stopped and resumed build can leave its random worlds switched off: `training-and-tenders` landed its generator
  with its line in `tests/common/slices.rs` commented out, so 300 worlds never drew a tender, and its summary said
  nothing. At landing, check the slice's line there is live (and that its `check` runs).
- Once crates landed, a wagon's load is loose cargo *and* `shipments`: `squads`, synced onto `crates-and-papers`,
  still counted only `wagon.cargo` for its porters and `lastHome`, so porters walked home from a crated load. Briefs
  for anything that weighs a load: `cargo::stacks(content, &cargo, &shipments)`, as the mod's `Crates.stacks`.
- Fixtures swap hands at their peril: `squads` traded a test town's Mason for a Librarian, and once `buildings-and-works`
  landed the town took a Mason on at a stall. Add the hands a slice needs to a shared fixture; don't swap out others.
- Pins to ambiguous systems hold the order of the tree's base, not main's: `bank-and-discounts` pinned `plan_papers`
  before `plan_owing` as its base ran them, and after `squads` landed main ran them the other way, so the rebased
  tree moved every world by an ulp. Fingerprint main and the tree after the rebase, before landing; a pin follows
  main's order then (one test per pin: drop it, see which way matches).
- New systems can move other slices' ambiguous order with no edge to them: `realms` registered two systems of its own
  and Bevy swapped lockers' and farms' reviews, so worlds without a realm changed. Fingerprint with the slice's line in
  `tests/common/slices.rs` commented out (it must equal main's), and pin what moved; diff the order by printing
  `look_schedule().systems()` after `initialize`. A slice that adds towns also moves other slices' random picks of a
  town (banks repaid fell from 4 to 2): their picks go among the towns on the map.
- Even a component on every town in `add_town` can move main's worlds: `freight-hubs` inserted `Hub` in an insert of
  its own, and with no hub able to emerge and its slice off the fingerprint still moved (something still meets towns
  in archetype order); in the works' insert it didn't. Fingerprint with the rule unable to fire as well as the slice
  off, and add a new town component to an existing insert's bundle.
- Resources are entities in Bevy 0.20: `sourcing-and-procurement`'s two new resources moved the fingerprint with its
  systems unregistered and its slice off. Compare main's order (print `look_schedule().systems()` with the slice's
  `register` off and on) and fingerprint the slice's systems as no-ops against them unregistered, not against main's
  number. A neighbour's private system is pinned from its own `register` line.
- `Holding.hexes` is in claim order, the mod's `Territory.hexes` by q then r: `mines-and-kit` sent hands to its hexes
  as claimed, and a town with land of two kinds gathered the wrong one. Briefs that walk a town's hexes say "by q, then
  r" (`set_guards` sorts the same way).
- A tree's agent killed its own hung test with `pkill -f 'cargo test --release'`, which kills every tree's: a gate that
  fails with a count passed and no failing line is a killed run, so run it again. Briefs: stop your own run by its pid.
- Routes come in two archetypes (with a `RoutePath` and without) and a load puts them the other way round: a sum over
  every route's plan in query order (the shopping's and the bank float's `committed`, the road food `promised`) came out
  an ulp apart loaded and never saved once the post opened runs and a town planned routes of both kinds the same look.
  Briefs: any loop over routes that adds floats or spends a shared budget goes `in_order`, not `routes.iter()`.
- Ports "forgive the dust" where the mod doesn't: `war-army` ended kit at `worn >= 1 - 1e-9` and floored the fed share
  with an epsilon, where the mod compares its doubles raw, so 6-, 10-, 24- and 30-day kit lasted a day less than in the
  mod (ten tenths sum to just under 1). Briefs: keep the mod's comparisons and floors as written, dust and all; an
  epsilon only where the core's money or stock differs from the mod's ints.
- Money is a `Fixed` now (`fixed-point-money`): a brief that moves money names the rounding at each site (what a payer
  owes `Fixed::ceil`, a share paid out `Fixed::floor`, a host's figure `round`), since `From<f64>` and the `impl
  Into<Fixed>` APIs round to the nearest without saying so; and a tree that changes behaviour rewrites
  `tests/replay_digest.txt` after its rebase (`REPLAY_DIGEST=write cargo test --release --test replay`). For the
  binding: a spur at 16 an emerald is 62.5 thousandths, so spurs convert at the treasury, not posting by posting.
