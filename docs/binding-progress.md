# Binding progress: where the Minecraft binding stands

The live to-do list for the Minecraft binding (the plan is `docs/binding-plan.md`; how trees work is
`docs/worktrees.md`). Updated by the integrator at each landing. Last update: 2026-10-23, `crown-views` landed.

## Now

- **2026-10-25, a town's hands that wander past its reach are still counted.** Greenfell's roster fell from 7 to 4 by
  day 14 with no one dead: three villagers had walked up the hill 64 to 80 blocks east and stopped where the study's
  loaded ground stops ticking. The census looks 48 blocks round the bell, and a villager not seen there was kept only if
  it couldn't be looked for (its last chunk unloaded); these could be, alive, so they were dropped as gone. Now a hand
  whose villager is alive anywhere is counted as it is (`PeopleAdapter.census`; game test
  `aHandWanderedPastItsTownsReachIsStillCounted`). 30 days on seed 12345: Greenfell 7 at the end (6 from day 15 to
  24: one Farmer died falling 20 blocks into a sinkhole inside the fenced ring, likely onto a lower fence's top where
  the ring steps down a slope).
- **2026-10-25, a town's ground made safe when it's founded (`settlement/Grounds`).** In the survival world (seed
  12345) Greenfell's villagers died over and over in a 1-wide alley a village left between a house and a hill cut:
  they stepped off the hill onto the roof's eave and fell 4 to 6 blocks into the alley (a server-side fall log found
  where each fall began). Now, once a desk is founded (and once a founded town is built), the ground within 40 blocks
  is gone over at the server's next tick: pits up to 3 wide and 2 or more deep between natural banks filled with the
  bank's block, a 1-wide alley between natural ground and a building filled and fenced on top (or its new ground leads
  onto the roof), never a column a door opens onto; then natural ground with a drop of 4 or more beside it, side or
  corner, fenced; anyone in a filled pit lifted out. Greenfell: 62 columns filled, 178 fences; 30 days, no villager
  died and no fall hurt more than 1 (it was 2 deaths and 27 falls of 3 to 6.6 by day 4). Game tests
  `aTrenchByTheDeskIsFilledAndWhoeverIsInItLiftedOut`, `aDropTooWideToFillIsFencedAtTheTop`,
  `anAlleyBetweenAHouseAndAHillIsFilledAndFencedOffTheRoof`. Still open from the same run: Greenfell's roster falls
  from 7 to 4 by day 14 with no one dead (its census sees only 4 from day 11, 3 beds counted), and its wagon never
  goes (the hub can't fund the trip: 0 emeralds from day 5).
- **2026-10-25, the survival world's findings fixed.** (1) A town keeps back the road money a round trip on each of
  its routes out takes (`Trades.road_reserve`, set first at each touch, in `reserve`): fields, stalls, lessons and homes
  can't spend its wagon (test `farms::a_town_keeps_its_wagons_road_money_from_a_field`). (2) A villager that dies is its
  hand lost even when the core hasn't its name yet or the hands' cache is stale (`HandsAdapter.lose` reloads, then
  `PeopleAdapter.unnamed`): no replacement spawned for a dead hand (game test
  `aVillagerThatDiesBeforeItsNameIsTakenIsItsHandLost`). (3) The ledger's People section names what keeps villagers
  from lockers: each workstation with no chest touching it, where it stands, and each villager with no workstation
  (`LockerAdapter.noLockers`; game test `theLedgerNamesTheWorkstationsWithNoChest`).
- **2026-10-25, the first survival world (findings, nothing fixed yet).** `just server-setup-survival 12345`,
  `survival_study.py 30`: two generated villages 1,488 blocks apart made towns as a player would (a desk at the
  surface by the centre, a chest of 32 emeralds, a Merchant), a route between, the first village loaded:
  1. Greenfell (loaded) spent its 32 emeralds on day 0 to day 4 (a field 4, a shop stall 12, training 4, signing 2,
     a home 10) and couldn't fund its wagon from then on ("the hub can't fund the trip", 43 times).
  2. Its villagers died: four fell at a ledge 23 blocks from the bell ("Merchant hit the ground too hard", twice:
     the second the replacement the census spawned); the hands no census saw for 10 days left the roster, 7 to 2 by
     day 25, both Merchants gone by day 12 ("no Merchant at the hub" 381 times). Only 3 beds counted for 7.
  3. No lockers in either town: a generated village's workstations have no chest touching them (0 of 2 managed).
  4. Elmfell (unloaded) kept its 8, spent 24 on stalls and a mine, mood -2 (no comforts, no trade).
- **2026-10-25, towns found 6 to 10 hexes out.** Three lanes from the same day 0, 120 days with realms, the dials
  `min_hexes`/`max_hexes` set a lane each: 4-8 (the mod's) founded 18 towns, new towns' mood -0.21, old towns' 1.92;
  6-10: 14, -0.52, 1.69; 8-12: 11, -0.68, 1.30; trade the same (113-120 trips, about 27,500 items). 6-10 (480 to 800
  blocks, about vanilla villages' spacing) is the core's default now (John). The settlers tests pin the mod's 4-8.
  How often a town founds is `days_between` (8), the stronger knob for fewer towns.
- **2026-10-25, settlers' towns are built in the world.** The founding view: a town a host's town's settlers found
  is the host's too (its lockers the host's), and is asked for at every look (`Output::Found { town, name, x, z }`,
  `Settlers.unbuilt`) until a desk of its stands (`Input::AddPoint`). `LandAdapter` loads its ground (5x5 chunks,
  held till built) and builds the Java rules' settlement round the spot (`Expeditions.buildTown`, now shared: a bell,
  the desk and its chest, a Farmer's stall, a lectern and a cartography table, two cottages), then gives the core the
  desk; its settlers are spawned by the census once it's counted. The hold on host towns' settlers is lifted.
  Schema `aaa60f4817571ebc`. A 120-day study with realms: 17 towns founded and built (`Newfound 0 founded at 723, -60,
  1525`...), 3-4 villagers each in their 4 beds; the young ones spend their capital on a field, a stall, walls and
  children (moods -1 to +1); the old towns 17-22 villagers, moods +1.4 to +2.7 (the hub +1.92). Test
  `settlers::a_hosts_settlers_town_is_asked_of_the_host_until_its_desk_stands`.
- **2026-10-25, Masons learn from building, and core studies open realms.** Size costs a point a 8 villagers by
  design (John: the extra people are soldiers); halls offset some of it, but four towns with a Mason never built a
  square in 120 days: a square needs a level-2 Mason, and a Mason levels only at its locker, whose first trade buys
  clay no town has (all four stayed level 1). Now a day a town's works are laid, its Masons at home earn a trade's XP
  (`MASON_WORK_XP`, 2: an Apprentice in five days of wall). Leisure is pipe tobacco, imported from the Coast by design:
  `network_study.py --realms` now runs in core mode (it builds the coast's sea and Fishermen, opens the Coast with the
  hub; the March only with `--ridge`). 60 days: the Coast's fleets made 11 trips to the hub (293 items), leisure met
  0.67 of days at the hub, 0.22-0.23 in S1 and S2, 0 where no wagon of the hub's goes. The trace's `works` table.
  Test `works::a_mason_learns_from_the_works_it_builds`.
- **2026-10-25, a town short of beds builds homes at any mood.** The network study's hub (its pen has no beds:
  `network/hub`, `network/squad`) held 5 villagers and 0 beds for 120 days, mood -2.16: half a point a villager
  without a bed kept it under the 0.5 a home needs, so it never built one (the mod's `Territory` has the same gate).
  Now villagers short of beds get homes whatever the mood (`people.rs`); the mood gate is for growing past them.
  Forked 120 days from the same day 0: the hub 5/0 to 21/22, mood -2.16 to -0.24; Hollowwick, short of two beds,
  -0.25 to +0.13. The sweep's mood floor counts every villager as one that may be without a bed (beds can now come
  faster than mood follows). Also: the scenarios' checker read its files with serde_json's default float parsing,
  which can come back a bit off (`0.9621110847109535` read as `...536`): `float_roundtrip` in the core's
  dev-dependencies. Test `people::villagers_without_beds_get_homes_whatever_the_mood`.
- **2026-10-25, farms grow what the town's comforts need.** Wheatfield's farm (2 Farmers, about 96 items a day) had
  planted 10 fields of potatoes nobody worked, for a Farmer's locker that would buy 832 a day: a busy town planted for
  "a crop it would buy and grows none of", and unworked plots never counted as grown. And cane (its paper, so its
  books) scored 0.8 against potatoes' 32: a comfort's inputs ranked at the item's plain price. Now a busy farm plants
  only for a crop it has no plots of (`rules/farms.rs` `field_wanted`; plots lying idle count as grown), and a comfort
  the town is short of makes its inputs worth what a short town pays for it (`input_demand`, `will_pay`'s eagerness:
  up to `desire_most`, 2x, with none of it met). Forked from the same day 0, 120 days: Wheatfield 6 to 25 villagers,
  mood -0.68 to +1.02, treats met 0.03 to 0.96, furnishings 0.81, reading 0.45; one field of each crop. Hollowwick
  18 villagers, mood -0.25 (a point a villager past 8 costs mood). Tests
  `farms::a_busy_town_plants_a_crop_it_has_none_of_not_more_of_one_lying_idle`,
  `workshop::a_short_comforts_inputs_are_worth_what_a_short_town_pays_for_it`. The trace's `farms` and
  `farm_towns` tables (plots, plan, idle, demand, census, draws, needs; focus, fields planted).
- **2026-10-25, a workshop's comfort inputs before its lockers.** Wheatfield ended a 120-day study with 1,610
  emeralds, 19 leather and no comforts (reading met 0.29 of days, mood -0.68): its Librarian's and Cartographer's
  lockers bought every paper that came, so its books (3 paper and a leather) had none. Now before each restock a town
  keeps back of each input what the comforts its trades make and it's short of would take, as far as its other inputs
  go (no leather, no paper kept), and its lockers buy beyond it (`workshop::keeps`, `Lockers.workshop_keeps`, not
  saved). Forked from the same day 0: Wheatfield's reading still 0.29, mood -0.63: no paper comes after the hub's
  wagons stop (day 57), and nobody grows the cane paper is made of. Test
  `workshop::a_workshop_keeps_back_from_the_lockers_what_its_comforts_short_need`.
- **2026-10-25, towns grow in core mode.** A 120-day study's towns never grew: every one sat at its bed count (6-7
  villagers) for 120 days with moods above +1. A wire-founded town's `People.land` was 0, so the core's home rule
  (`beds + 2 <= land`) never fired; and a home it built had no output (filed at people-adapter as "beds the next census
  overwrites"). Now `AddTown` gives a host's town a settlers' town's open land (40 beds); a home the households build
  is a work the host lays (`Work::Home`, `LandBook.homes` until `Built`; `LandAdapter` builds the Java rules' cottage at
  `Growth.homeSite`); a census counts the beds it found plus an unlaid home's two. Schema `81d6ff04f74eef89`. Tests:
  `census::a_home_the_core_builds_is_laid_by_the_host_and_its_beds_count_till_then`,
  `aHomeTheCoreBuiltIsLaidWithItsTwoBeds` (312 both modes).
  Grown, the towns sent settlers: 15 core-only towns (`Newfound`, 2 villagers each, mood -0.25, never growing) in
  120 days, their 30 villagers gone from the world for towns nobody sees (the mod can't show a founded town yet).
  Held for now (John): a host's town (its lockers the host's, `locker_host.rs`) sends no settlers
  (`rules/settlers.rs`); the founding view (bell, desk, cottages built where the party goes) is the next step for
  settlers. Test `settlers::a_hosts_town_sends_no_settlers_until_the_host_can_show_what_they_found`.
- **2026-10-24, each town rolls its own dice.** One world `Rng` drawn by every town meant a change in one town
  re-rolled every other's: S1's Leatherworker did more locker trades (each rolls for the Merchant's XP), so roadless
  Greenwick's slack Fletcher retrained as a Leatherworker (cows, no wool, furnishings 0) where the base run drew a
  Shepherd (sheep, carpets, furnishings 0.77): its mood 0.22 against 1.42 was dice. Now `Dice` (saved) holds a stream
  a town, made at its first roll from the world's seed (never rolled itself) and its id (splitmix64); the locker XP
  roll and the jobs draw take the town's. Save shape changed (FORMAT stays 1, alpha: no compatibility): checkpoints
  from before don't open. Test `jobs::a_towns_dice_are_its_own`; replay digest and scenarios' saves written again.
  Checked: the Leatherworker fork again from a new day-0 checkpoint (`dice`), three lanes each: Greenwick and Hayfield
  (no road to S1) come out the same to the hundredth with or without S1's new hand (-0.32, 0.28; money 60, 50);
  Meadowworth +2.62 to +3.66 (reading 0.77 to 0.90), the hub -0.14 to +0.58, Hollowwick 1.21 to 1.41.
- **2026-10-24, a Leatherworker gives S1 reading.** `network_study.py --add-hand S1:leatherworker` (S1:none, the
  fair side) forked from a recipes day-0 checkpoint, three lanes each (identical within a side): Meadowworth's cows
  give leather its Librarian binds at once (leather never above 3 held), reading met 0.84 of days (0.52), mood
  +3.01 (+1.57), money 838 (800), beef 69 by day 30. The core opened 11 routes, not 9, once S1 had its new hand, so
  the rest moved too: Wheatfield mood 1.84 (2.71), Greenwick 0.22 (1.42), Hollowwick 1.55 (1.38), network sales
  24,110 (22,679).
- **2026-10-24, villagers work the game's recipes.** A trade may craft or smelt what it deals in (buys or sells in
  its vanilla trades), from the game's crafting-table and furnace recipes (`binding/VillagerCrafts`): on top of the
  mod's 7 conversions (which keep their outputs), recipes over goods the economy already prices, with no input that
  comes back from the bench and none that undoes another; plus `EXTRAS` the game has no recipe for (a Librarian breaks
  a bookshelf into 3 books) and a `DENIED` list. 41 recipes in the dev pack: arrows and bows, tools and armour, smelted
  iron and gold, leather armour, rabbit hide into leather, wool from string, paintings, banners, writable books. No
  wire change (the pack carried any recipes already); a world keeps the recipes it was made with. Books from bought
  bookshelves cost 3 emeralds each against a book's 0.25, so no town buys them (the margin rule stays, John's call):
  reading comes from paper and leather, and the study's towns have no Leatherworker, so none. A fresh 30-day study:
  Greenwick spins the hub's string into wool and carpets (mood -0.31 to +1.41), writable books in four towns, bows in
  three; hub mood -0.88 (-1.82); two fresh worlds with different routes, so partly noise. Game test
  `theModsContentPackIsTakenWholeByTheCore` checks the bookshelf, the arrows and that every recipe's trades deal in
  its output.
- **2026-10-24, the network trades past day 17.** From day 17 no wagon went anywhere: the hub's warehouse is a fixed
  stock (the study's design: no restock without `--inject`), and its towns bought only 7 of its 15 goods, since the
  study switched trades on once at setup and offers a villager unlocks by levelling up stayed off (flint, pumpkin,
  melon, ink, compass: 1,728 each, never sold). `/enterprise autoswitch <desk> unlocked` switches on buying offers
  still off at lockers already set up; the study runs it every 4 days in core mode. 30 days: 26,320 items sold
  (12,071 before), wagon takings 1,473 (373), trips to day 28, hub mood -1.82 (-2.83), S1-S3 treasuries 352/738/633
  (about 120 before). Game test: `aTownsTerritoryAndTheWarAreTheCoresForTheStudies` checks `unlocked`. Found with
  the trace's `stock` and `lockers`. Also seen: `CoreBindingTests` run alone fails its buying-vendor test (all 311
  pass together).
- **2026-10-24, no wagon goes for its purse alone.** A shopping trip's money counted as its load's worth whenever the
  far end held any comfort it wanted, so from day 20 the hub sent empty wagons every 4 days, 6-12 of road food each,
  to bring home 14 items for 0.68 (found with the trace's `trips`). Now the purse counts only up to what the far end
  would sell of the list beyond its own keep (`buy_for_home`'s rule), plus a glut there home uses, at its discount on
  what the money buys (`buy_backhaul`'s rule, shared as `glut_of`/`used_at_home`). The study's journal replayed as a
  what-if: the hub's empty runs stay home; Wheatfield's and Meadowworth's glut runs (141, 133, 512 items) still go. A
  core-only change: the Java rules keep the old count. Test `desires::a_shopping_trip_is_worth_what_the_far_end_would_sell_not_the_money`;
  replay digest written again.
- **2026-10-23, runs traced.** The server's core writes a journal of every request it took (`binding/core/Journal`,
  `world/enterprise-journal/<time>.ejl`, one a server run, 10 kept, `journal=off` for none; `/enterprise core journal`;
  `core report` copies it). `enterprise-sim --trace FILE run.db` replays it through the native core and writes SQLite
  (`sim/host/src/trace.rs`): inputs, outputs, each day's towns, stock, routes, lockers and whys, every decided plan
  (worth vs cost, went or stayed) with its per-good reasons, and the cash books. A stayed plan's reasons are kept on
  `RouteState.last_plan` (not saved, never a rule): the core's tally drops them. The 30-day study: 165 KB, 663
  requests, replayed in 0.5 s to the same trips and sales. Tests: `JournalTest` (17, a replay through a trap saves
  the run's bytes), `trace::tests` (a traced run ends where the native run did).
- **2026-10-23, the hub's wagons go.** The A4 "no caravan in 30 days" was the study, not the core: under the core a
  town wants only what its switched-on lockers buy, a locker counts only once a loaded villager has claimed its
  workstation, and the study switched trades on (and unloaded its towns) before any had. It now waits, then
  switches: 19 hub trips and about 15,000 items sold in 30 days, S1-S3 moods +1.2 to +1.4 (docs/alpha-smoke.md 1).
- **2026-10-23, A4: the smoke run (docs/alpha-smoke.md).** The alpha passes section 5: the jar runs outside the dev run
  (a plain NeoForge server), no trap, pause or exception, cash books explained, no negatives, a restart reads the same,
  the report writes, no Java rule wrote data. **Not yet playable as a trading game: no caravan left in 30 days.** Each
  hub route waits "for a fuller load" with 0 items (the core's `plan.rs`: a load must be worth the road). Next: why
  the sale load is empty, with a Rust test of a fresh hub and town that must trade within ten days. Also fixed: the
  network study under the core (its towns founded by `enterprise core stock`; their census areas loaded).
- **2026-10-23, A3 landed: the alpha build.** Core is the default society (only `java` picks the Java rules); a world
  keeps the mode it began in (`binding/WorldMode`, `enterprise_society.txt`; a marker-less world with Java data and no
  core save is Java); `0.1.0-alpha.1` in gradle.properties and pack.toml; `/enterprise core version` and `core report`
  (`world/enterprise-reports/day<N>-<time>/`: version, every town's ledger, cash book 30, whys, the save);
  `prism_sync.py` writes `society=core`; the testbench cottage built by blocks; `docs/alpha.md` and two welcome lines.
  Checked live: default mode core, the marker, version, report, the cottage, and a core world opened in Java mode
  pausing. Both suites 311 of 311. The ledger footer still shows mod version and schema, not the core's commit (that
  needs a save; `core version` has it). Next: A4 `alpha-smoke`.
- **2026-10-23, A2 landed: the stubs.** In core mode: the 17 refused commands say "not in the alpha yet" (or "gone" for
  `bank` and `tenders`) and what to use instead; the Merchant screen's coin rate is the core mint's; no Java locker count
  where the core keeps no town (0); a crate says once that packed goods aren't counted; a villager's death leaves the Java
  roster alone; an operator who joins a paused or unstarted core is told why (`binding/PausedNotice`, also the ledger's
  line). `AlphaStubsTests` both modes; both suites 310 of 310 (one trample flake in `TerritoryTests` passed on rerun).
  Next: A3 `alpha-build`.
- **2026-10-23, A1 landed: the ledger.** The Merchant screen's Ledger tab and `/enterprise ledger <desk>` (one set of
  lines, from existing queries, no wire change): Money, Goods, People, Wagons, Papers, Mint, each line naming its query.
  `LedgerTests` passes in both modes (its one fix: the command indents a section's lines, so a good prints `    wheat 30`);
  both suites 309 of 309, core baseline now 309. Checked over RCON on the testbench desk. Next: A2 `alpha-stubs`.
- **2026-10-23, later still: no worktrees this phase (John).** The CPU went to rebuilding and rerunning everything:
  each tree its own Rust build from scratch, and every check the whole Rust suite plus both game test suites, run by the
  building, auditing and landing agents in turn (up to 10 full checks a task). Work is now on main, one task at a time,
  gated by `just check` (scripts/check.py): only what changed since origin/main, the slow Rust tests and the full game
  suites only with `--full`, once before a push. `just gametest A,B` runs only those classes (mixin/GameTestOnly). The
  parked trees (`alpha-ledger` and the four fixture trees, WIP commits on their branches) are brought onto main one at a
  time. No fan-out until the CPU per task is under control.
- **2026-10-23, later: one tree at a time (John).** Five agents shared the efficiency cores the CPU cap leaves while
  John is at the Mac: each full test run took 25 to 60 minutes and the agents sat idle, spending tokens, waiting on the
  same CPU. The fixture trees are parked (`fixtures-caravans`, `-land`, `-people` with a WIP commit each,
  `fixtures-market` at its commit; their worktrees stay). The alpha goes first, one tree at a time: `alpha-ledger`,
  then `alpha-stubs`, `alpha-build`, `alpha-smoke`. A tree runs only its own tests while it builds; the full suite runs
  once, at landing. The fixture trees resume after the alpha.

- **2026-10-23.** `crown-views` landed (31 of 35): all of B4 is in. Left: `delete-java-rules`, then
  `scenarios-in-game`, `upgrade-drill`. Core mode now has stock, money, people, land, lockers, caravans, pallets, the
  commands' books, the players' shops and the crown (the sovereign, its realms, the markets off the map, the gold
  shop) from the core; the Java rules are unused in core mode and go with `delete-java-rules`, and every command still
  on them says "Java mode only until <task>" in core mode.

- **Main:** `be4bf7b`. 973 tests pass (`cd sim && cargo test --release`); `just core-wasm-replay` matches native;
  `just wire-check` passes (schema `41c307f6fa0993a8`, 66 files); `just scenarios-wasm` and `just binding-test` pass.
  Game tests: "All 308 required tests passed" in Java mode, and `just gametest-core`: 308 of 308 (baseline now 308).
  `just core-fingerprint`: `d8a270619eff9cdb`, unmoved (base worlds behave as before). Compare later refactors
  against it.
- **The crown** (B4.10, `sim/core/src/crown_host.rs`, `adapters/CrownAdapter`, `CrownShop` saved at FORMAT 1: dev
  worlds' `enterprise_core.dat` rebuilt): `Input::OffMap`, `Capital`, `Mobilise`, `Declare`, `Peace`, `CrownShop`
  (35-40); `Output::WarDeclared`, `PeaceMade`, `BuildCrownShop`, `CrownShop`, `CrownShopDown` (23-27); `Query`/`Answer`
  `Realms`, `Metals` (26-27); `PointKind::Market` (a market off the map, a realm, the front, the market across the sea:
  every one a point). The gold shop's card is a crown account kept funded to two lots' price from the treasury
  (`post`, "gold shop"), the gold sold into it delivered and sent to the mint. `/enterprise war capital|mobilise|
  declare|peace`, `realms`, `realms open` and `metals` ask the core. **At landing:** the gold shop is asked for again
  at each look that counts the capital while the crown is mobilised or at war and none stands (`want_crown_shop`; the
  audit's blocker: the one request waited in the host's unsaved `toBuild` and was lost on a restart, with no floor, or
  with no dimension); a shop taken down out of reach is remembered (`CrownShop.down`) and told `CrownShopDown` again when
  next counted, the host keeping its point until it can take the vendor away (it would have been taken for a player's
  buy order with the crown's card); a vendor the core won't take is taken away again; the books scenario's hub paid 50
  again (500 came in by mistake). **Open:** no vendors in core mode for the metal exchange, the requisitions or the
  freight hubs (players can't sell metals or kit at the port); the front and the market across the sea are points with
  no block (`desk_of` falls back to them, a "desk" at height 0 in `Answer::Routes`/`RouteBooks`); the gold shop's card
  keeps its balance at peace (outside the books), takes only matching gold and never clears other items a player put in,
  and a second `Input::CrownShop` replaces the first without taking it down; a `CrownShop` shown after its vendor unloaded
  loses its `fund` (the treasury paid, the card not credited) and its `taken` gold is counted again next look; the site
  is the nearest open floor 3-10 blocks from the desk, not 10 north; WarTests still call the Java rules (three core-mode
  twins in CoreBindingTests, so core mode's 308 overstates the crown's cover); the `crown_host` random-worlds slice
  opens no market and declares no war (about 1% of seeds broke the mood floor with no rule broken); `trade_points`'
  slice now clamps a town stock of -1.8e-15 (seed 207, day 44) at 0: some core rule leaves a negative float residue in
  `Stock`, not chased.
- **Shops** (B4.9, `sim/core/src/shop_host.rs`, `adapters/ShopAdapter`, `Vendors` saved at FORMAT 1: dev worlds'
  `enterprise_core.dat` rebuilt): the core says what each vendor is and the mod shows it. `Input::Vendor { point, read:
  VendorRead }` (31: a host's count of one vendor, facts only), `Board` (32), `DeedShop` (33), `DeedBrought` (34);
  `Output::OrderFilled` gained `point`, `StallBought`, `Shelf`, `Deed`, `DeedPaid`, `BuildShop` new; `Query`/`Answer`
  `Shops` (24) and `Deeds` (25). A buying vendor is a player's standing order (a good, lot or price changed is a new
  order), a selling one a stall, one touching the desk the town's own shop, one put up for the board the board's (paid
  four emeralds), one by a hub's desk a rent's deed shop. `/enterprise core shops <desk>` and `/enterprise market` read
  the core. `Shops.collect` sums the coins now (Numismatics' cached sum survived `clear()`: phantom takings every look,
  in Java mode too). **At landing:** a look deals only with the vendors counted for it (`Vendors.counted`, cleared as each
  `Advance` ends; the audit's blocker: an unloaded vendor's order was filled, its stall bought from and its shelf
  stocked on the books while the host dropped the outputs); a vendor's box is emptied only where the core says the coin
  is the town's (touching the desk, or a deed shop its `Query::Deeds` names; the audit's second blocker: a player's
  vendor reselling a deed lost its takings each look); a price change is a new order (`Broker.contractFor` matches the
  delivery by its price; random worlds' seed 7 broke the paid-at-most-the-price check otherwise); `bank`, `bank open`
  and `tenders` now say "Java mode only until B5.1 delete-java-rules" (no core rules shown for them). **Open:** coin is
  taken out of a box while the look's inputs are built, before the core accepts them (a refused look loses it);
  `filled`/`bought` don't check the card was charged or the lots were still there (outputs come a tick after the
  count); the deed vendor's card is refunded from the treasury after every switch to selling while the dealer account
  keeps the old balance (a little town money parked out of sight each cycle); `present` blocks the server thread on
  `settle()`; the crown's, metal exchange's, requisitions' and freight's vendors near a desk are classified like
  players' (a crown buying vendor with a funded card would become an order) until `crown-views`; enchanted commissions
  are Idle; the dealer's rent trades between towns have no core rule; items of another good left in the town's own or
  board shop are discarded on a relist; MarketTests, MoodTests' board test and the deed test still call `Broker`,
  `ShopBoard` and `RentDeeds` directly (five core-mode twins in CoreBindingTests, the board's and deed's on made-up
  outputs); `Deed`, `DeedPaid` and `BuildShop` appear in no scenario, only the golden files; the save test doesn't hold
  a deed vendor.
- **Commands and studies** (B4.11, `sim/core/src/commands_host.rs`, `adapters/BooksAdapter`, `adapters/CommandsAdapter`,
  no new saved state): `Query`/`Answer` `CashBook`, `Books`, `Whys`, `Trust`, `Contracts` (16-20), `Territory`, `War`,
  `RouteBooks` (21-23), in the Java commands' words so the studies' regexes read both modes. Kept study:
  `network_study.py` (default flags, `--save-at`, `--label`, `--resume`, `--stop-when`), the core's numbers under
  `society=core`; `trace.py`, `forks.py`, `compare_lanes.py` work in both; `merchant_ramp`, `farm_wages`,
  `caravan_network`, `standalone_town` exit "Java mode only (retired for sim/host)", `paper_report` refuses a core
  checkpoint, `world_map` is only noted. **Open:** the kept study has never been run under `society=core` (only its
  regexes, by `CoreBindingTests.aTownsTerritoryAndTheWarAreTheCoresForTheStudies`, on a fresh fixture with no trips);
  contracts' and rents' `day N:` lines are worked out from state (two always carry today, and trace.py keeps only the
  first three, so a contract's `completed`/`broken` line is dropped there); core-mode territory lacks crossings, herd,
  kit and flower-bed lines, tolls owed always none; a few command-side rules in Java (`autoswitch`'s which-offers,
  `townNear`'s 48-block bell-to-desk match, territory's mood and tightness words) belong in the core; town names fetched
  by a whole `Territory` each; `depot add/remove` still write Java's `Depots` beside the core's points;
  `name <desk> <name>` and `switch` refused until `delete-java-rules`.
  `just core-fingerprint`: `5e6b6d51a3b4061e` (unmoved by `pallet-views`; the replay digest moved with the save's shape
  and the new `pallet_host` random-worlds slice). Flaky: `TerritoryTests.farmersProduceWhatGrowsInTheirHex` ("counted 8",
  neighbours trampling its plots) failed once in core mode in the tree's gate run, then passed.
- **Pallets** (B4.12, `adapters/PalletAdapter`, `sim/core/src/pallet_host.rs`, `Stowage` saved at FORMAT 1: dev worlds'
  `enterprise_core.dat` rebuilt): the core implements pallets, the mod draws them, no pallet rules in Java. Each package
  is an item tagged `enterprise:package` (`PackageTag`: "1 of 3, to Wheatfell, contract 7"), the bill of lading in the
  first's documents slot, in its town's desk's chests. Each look in core mode: destroyed package items told as
  `PackageLost` (applied at once); crates found in loaded desks matched to the core's pallets (stale ones taken out,
  carried ones told as `PalletBrought`/`PackageBrought` after the look's `Advance`); `Query::Stow` with each desk's
  empty slots (whole or not at all, the core's say), the offered pallets put in and told `Stowed`. `PalletOpened` and
  `PalletMoved` off a warehouse clear the crates at once. Wire: `Input::Pack` (29, `/enterprise core pack`),
  `Input::Stowed` (30), `Query`/`Answer::Pallets`/`Pallet`/`Stow`/`Manifest` (12-15), `PalletView`, `ManifestLine`,
  `PackPackage` (the content pack's packing kinds, `ModPack.PACKAGES`, which also registers `enterprise:barrel` and
  `bundle`). `/enterprise core pallets <desk>`. Measured: a look's pallet happenings with two desks and a pallet of three
  crates 0.185 ms of the tick; a pallet drawn as items 0.03 ms. **For John:** in core mode the warehouse's pallets take
  desk chest slots at each look before the stocktake, ahead of loose goods (what then doesn't fit stays on the books as
  `Unmoved`), so desks with many pallets show less loose stock; a lost package books goods (used, reason lost), no money.
  **Open:** the superbill is only `Query::Manifest` and a command line, not a paper; no crates appear at a wagon's
  unloading (a pallet made out home is opened as it unloads; no wire input drops one at a town not its consignee);
  empty crates, depots and lockers show no pallets, nor does the Merchant's screen; `CrateMenu`/`CrateScreen` untouched;
  `PackGood` has no package or handling (every Minecraft good packs in crates); package kinds read by index from the
  mod's table, not the save's content; destroyed crates are kept in memory until the next look (a stop in between loses
  the `PackageLost`); Create's belts and funnels discarding a crate item untested; `cargo/Cargo` reads
  `binding/ModPack.PACKAGES` at registration; no player-facing Pack (op command only); `cargo/Crates`, `Papers`,
  `Paperwork` stay Java mode's until `delete-java-rules`.
- **Wagons** (B4.8, `adapters/WagonAdapter`, `sim/core/src/wagon_host.rs`, no new saved state in the core): the caravans
  are the core's; the world shows a wagon's crew. `Input::Route { from, to, send }` (Input 28: the Merchant's screen's
  Send/Stop and `/enterprise route add`/`remove` in core mode) opens a route if a leg's food reaches desk to desk and
  there's a way, stops one by winding it down (its wagon comes home and stays), and reopens a wound-down one with its
  number and books on the same food terms. `Output::HandAway { town, hand, tag }` / `HandBack { town, hand, tag,
  trade, level, xp }` (Output 16/17, by hand id: a wagon's crew, a porter, a soldier): the adapter stows the villager
  (`Squad.stow`; one not loaded is stowed at a look once it is) and lets it out by its town's desk (`Squad.release`,
  waiting for an unloaded desk), the stowed villagers in the view's own `enterprise_core_stowed.dat` beside
  `enterprise_core.dat`, written with the host's save. A hand back counts as seen by the census. `Query::Routes` /
  `Answer::Routes` (Query/Answer 11; `RouteLine`, `WagonLine`, `MarketLine`): the screen's caravans and squad,
  `/enterprise core routes <desk>`, `/enterprise routes` in core mode. `WagonLeft`/`Sold`/`Home` are only logged.
  Measured in the game-test server: a villager stowed 0.073 ms, let out 0.27 ms, a look's catch-up with nothing waiting
  0.0001 ms, a town's routes asked 0.073 ms. **No pallets in Java** (John, 2026-10-18: pallets live in the core only):
  a wagon's crates and the post's papers are the core's pallets (a crate a pallet's package, its papers the bill of
  lading and the wagon's superbill); `pallet-views` (B4.12) draws them from `PalletPacked`/`Moved`/`Opened`, still
  `unshown`. **For John:** stopping a route now winds it down rather than deleting it (Java's `removeRoute`), and
  starting it again reopens the same route. **Open:** herds and pack llamas on a wagon (none in the core); crews the
  Java ledger stowed before a switch to core mode are never released; a hand back never seen leaving gets no villager;
  losing `enterprise_core_stowed.dat` loses the crews (the census ages them out after 10 days); the screen's "known
  markets only" and `reachable` pre-check sit in `WagonAdapter.commission` (Input::Route doesn't enforce the first); the
  core-mode screen loses Java's "road mapped, N town(s) seen" status, and `RouteLine.status` is English with `sold` as
  `f64`; `/enterprise routes` lists only routes touching towns with a point; `CommissionPayload.commission()` and
  `MerchantView.build()` stay on the Java rules; CaravanTests (and Cargo, Post, Service, Master) still call the Java
  `Caravans.poll`, with three core-mode twins in CoreBindingTests.
- **Lockers** (B4.6, `adapters/LockerAdapter`, `sim/core/src/locker_host.rs`): the core runs the lockers, the mod places
  them, shows the Merchant's screen and counts their chests. A town founded through the wire (`AddTown`) has a hand's
  locker only where its host places one (`Lockers.placed`, saved); towns the core keeps as numbers keep every hand's.
  `Input::Locker { hand, at }` (each look in core mode, before the stocktakes: each loaded villager's workstation with a
  container touching it, its `Locker` point added, untied points removed; a villager or workstation not loaded is left
  as the core has it), `Input::Switch { hand, offer, good, on }` (the screen's switches), `Query::Lockers` /
  `Answer::Lockers` (`LockerView`, `OfferView`: what the screen and `/enterprise core lockers <desk>` show;
  `/enterprise lockers` routes there in core mode). A locker's chests are counted with the desk's: what the town uses
  comes out of them first, what's left is carried to the desk and depots as far as they have room. The restock mixin
  runs the Java locker rules only in Java mode, so `Rounds` and `LockerCatchUp` have nothing to do in core mode (kept
  for Java mode until `delete-java-rules`). A switched-on emerald-spending trade now gives a core-mode town money's
  64-emerald reserve. Measured: a town's lockers placed at a look 0.18 ms of the tick (one town, two villagers), a
  locker's chests counted 0.009 ms. **For John:** a locker is emptied into the desk at each look that finds room and
  nothing is put in one first (the Java lockers kept their leftover inputs; decisions.md "Lockers and the binding");
  the plan's `Switch { point, offer, on }` became hand-keyed with a `good` stale check, and `Output::Trades` became the
  `Query::Lockers` question. **Open:** the screen shows every offer of the villager's level, not its two rolled ones,
  and players' own trades don't reach the core's demand (`Society::set_offers` not wired); the screen's `limit` comes
  from Java's `Merchant.lockersAt`; emeralds a player leaves in a locker are counted as neither goods nor cash; a
  villager never loaded since joining has no locker until it loads; settlers' towns keep every hand's locker;
  LockerTradingTests run the Java rules under `underJavaRules` in core mode, and LockerTests and MerchantScreenTests
  still test Java (the core's coverage is two CoreBindingTests and the Rust tests; neither game test goes through
  `CoreHost.look`'s own `place(server)`, and the cost with many towns is unmeasured); `/enterprise core lockers` reads
  wire types outside `adapters/`. The save grew at FORMAT 1 (`Lockers.placed`): dev worlds' `enterprise_core.dat` are
  rebuilt.
- **Land** (B4.7, `adapters/LandAdapter`, `sim/core/src/land.rs`): the host surveys, the core decides, the host lays.
  `Input::Ground` (a hex's water share, relief and `LandKind` from `Terrain`, both modes: hexes whose chunks were
  surveyed again, and at a town's first sight in a run those within 8 the core has otherwise, 64 a look at most);
  `Input::Crops` (one chunk's crop plots in a hex, kept by chunk, so an unloaded chunk keeps its count); `Input::Survey`
  (core mode, each loaded town each look: its bell; its farm set to the plots counted in the hexes it holds plus the
  fields planted and not laid; crops counted every 20 looks). At the end of a look every surveyed town is asked for its
  works, `Output::Build { town, work, share }` (a bridge or hall once finished, the wall a quarter at a time, a field as
  soon as planted), until the host says `Input::Built`; the mod lays them with its own placement (`Works.layDeck`,
  `raiseColumns`, `Buildings.site`/`build`, `Growth.farmSite`/`Fields.plant`) at the start of the next look, once every
  look sent is shown. `Query::Land`, `Query::Ground`, `/enterprise core land <desk>`. Measured: a town's first look of a
  run 1.1 to 4.2 ms of the tick, later looks about 0.15 ms, one hex's crops 0.4 to 0.74 ms. **Open:** no wooden pier
  (`Territory.buildDock`: stone docks go up at the pier's start with no pier), pastures' fences or mine heads; docks,
  pastures and the free/confined split aren't surveyed; settlers' founded towns have no desk; town names still drift
  from `Names`; every town's crops are counted on the same look, unstaggered (a few loaded towns, or one holding 7 to
  19 hexes, would pass the plan's 5 ms for that tick); a field is paid for before the host looks for its site, so one
  with no room is paid for, yields on the farm and is asked for at every survey (Java's `Fields.consider` finds the site
  first), and the site rule accepts any spot while the town holds no hex; a field's Crops recount isn't resent after a
  refused look (up to 20 looks uncounted); `LandBook.shown` is keyed by what the work is (a work started again would
  not be shown again; no rule restarts one today); the look path still asks the core synchronously from the tick
  (crops, ground); none of row 7's acceptance tests (Territory, Toll, Building, Husbandry, Metal, Mood) drives the
  core, and in the world only a hall at a given site is laid (a 7x7 arena holds no wall, bridge or field: those are
  pinned in Rust and `sim/scenarios/land.json`). The save grew at FORMAT 1 (`LandBook`): dev worlds'
  `enterprise_core.dat` are rebuilt.
- **Money** (B4.4, `adapters/MoneyAdapter`, `sim/core/src/till.rs`): money stays physical (emeralds and Numismatics coin
  in a desk's chests, a Blaze Banker's balance) and the treasury stays one number of emerald-worth. Each look a
  `Cashcount` follows each desk's `Stocktake`: what a player or hopper added or took is booked as `PaidIn` or the new
  `Withdrawn`; then `Output::Cash` shows the treasury: income in as whole emeralds (fractions on the books), spending
  out emeralds first, then coin, then the Banker, then an emerald changed at the mint; emeralds under the reserve
  bought back from coin, above it sold to the mint (into the Banker, else chest coin with room). Each coin pile is a
  `Hoard` (spurs and the worth they stand for), so the rate moving makes or loses no money. `CashUnmoved` is what the
  host couldn't do; Java mode only counts (`writes=false`). `Query::Cash`, `Query::Mint`, `/enterprise core cash`,
  `core mint`. Measured: a desk's money counted 0.012 ms, written 0.024 ms. **For John:** a desk's first count
  (including one placed by a chest of emeralds) books the chest as money paid in and, in core mode, sells it to the
  mint at once. **Open:** the core's reserve has no wage float (`Territory.wageFloat`, land's); a sale whose coin
  write fails still leaves its pressure on the mint (rare: coin room is six slots, or more than 64 suns of coin with no
  Banker); only a town's first counted desk shows money; coin in shulker boxes isn't counted; a world switched from
  java to core makes its chests match the core's shadow treasury at the first look; sovereign minting and vendor
  payments have no wire message (`crown-views`, `shop-adapter`), and Java paths a player triggers (crates, the shop
  board, rent deeds) still use the Java `Mint` beside the core's until those land; MoneyTests and CountingHouseTests
  still call the Java rules (six core-mode twins in CoreBindingTests instead); the random-worlds check doesn't hold the
  till to the treasury (the scripted month does); `/enterprise core cash` reads wire types outside `adapters/`. The save
  grew at FORMAT 1 (Tills, `Reason::Withdrawn`): dev worlds' `enterprise_core.dat` are rebuilt.
- **Stock** (B4.3, `adapters/StockAdapter`, `adapters/StockChests`, `sim/core/src/chests.rs`): chests stay physical and
  show their town's stock. At each look every loaded desk and depot is counted (`Stocktake` with its empty slots,
  `free`; goods loose and in shulker boxes, storage crates left shut for `pallet-views`), and at the end of the `Apply`
  the core shows the town's stock in the chests it counted: `Output::Moved { point, good, count }`, the town's whole
  items against what all its points' chests are taken to hold, into desks then depots as far as each count shows room,
  out of them as far as each holds it. What the host can't do comes back as `Input::Unmoved` before the next count;
  what has no room, and fractions, wait on the town's books. A move is a number of items worked out against a count of
  the same look, so a chest is always counted before it's written, a hopper between count and write keeps its items,
  and an unloaded point isn't written (counted as its chunk loads, the next tick): nothing is queued. Java mode counts
  but never writes (every move sent back unmoved). `RemovePoint` takes a point's counted goods out of its town and a
  desk put back where a town's stood is that town again (`chests::FormerDesks`, saved): the double count is settled,
  and what chests gave up beyond what the town held is the point's `short`. Depots follow a building's floorplan
  (`merchant/Depots`, from `/enterprise depot add`): at each look its containers the core doesn't keep join as depots
  (a chest put back, one added). `Query::Chests`, `/enterprise core chests <point>`; desks placed unseen are founded
  when their chunk loads (in a fixed chunk order), the testbench's desk registers at load. Measured in the game-test
  server at machine load 10 to 20: a look counting four desks' 540 stacks 0.09 ms p50 of the tick (0.19 mean), a move
  written 0.007 ms, a 42 KB save 3.4 ms p50. **Open:** depot kinds (an armoury's war goods, a barracks' rations) have
  no core counterpart, and the floorplan stays Java's `Depots` record (its kind and its rules Java mode's) until a
  depot kind lands in the core; the rules' room is still the `warehouse_slots` dial, not the counted chests (`kit.rs`,
  `training.rs`); `free` counts only empty loose slots, not room in shulker boxes; goods in storage crates aren't town
  stock in core mode until `pallet-views`; any stack of an item counts whatever its components (a renamed or enchanted
  one is a town good); a move that can never be done (a good that maps to no item, every move in Java mode) is a
  Moved/Unmoved pair at every look; consignment pallets moving town stock show as `Moved` but no test covers it; desks in
  chunks loaded before the host started (spawn chunks) aren't scanned; the per-chunk POI scan while exploring isn't
  measured; LogisticsTests and MerchantTests' four stock tests still call `MerchantStock`, `Depots` and `ColdStore`
  directly (not rewritten onto the core; `Warehouse`, `Depots`' rules and `ColdStore` stay until `delete-java-rules`).
  The save grew at FORMAT 1: dev worlds' `enterprise_core.dat` are rebuilt.
- **People** (B4.5, `adapters/PeopleAdapter`, `sim/core/src/census.rs`): the host's census is a town's roster's
  source. In core mode each look counts every town whose desk and settlement (48 blocks round the bell nearest the
  desk, no path checks) are loaded and sends `Input::Census { town, beds, residents }`: a villager the core doesn't
  know joins (a child as a child); one it knows at home takes a profession or a higher level from the world, never
  `none` and never the desk's Merchant's trade; one away, another town's or with settlers is left as the core has it;
  one whose hand is gone comes back as `HandGone`; a hand at home no census has seen for 10 days has left
  (`census::Sightings`, saved). A villager not loaded and last seen where nothing is loaded is sent as the core has it,
  so it isn't aged out. A hand with no villager gets one spawned by the bell, named by `Input::TagHand` with every
  look until the roster shows the name taken, and before every save (`CoreHost.save`: a name left for the next look
  was lost with a stop, and the reload made the villager a second hand). Five outputs around an `Advance`: `Born`, `Settled`, `Departed`,
  `HandGone`, `HandLevel` (profession, level and XP written to the villager). `AddHand` stays beside `Census` (one
  villager joining on touch, the census the periodic whole): the plan's 1.3 `Census` is answered. `CensusTaker`'s
  counting half (`beds`, `villagersAround`, `nearestBell`) is shared with the Java census; `/enterprise core census
  <desk>`; `/enterprise census <bell>` refuses in core mode. Java mode sends no census. Measured in the game-test
  server: about 0.4 ms of the tick per loaded town per look (0.086 ms counting, 0.29 ms through the core). Divergences
  from `CensusTaker` in `docs/decisions.md`, "People and the binding". **For John:** the census never takes a
  villager's lost profession (`none`) or changes the desk's Merchant's trade. **Open:** settlers have no view (`Settled`
  and `Departed` tested in Rust only: wire-made towns have no hex, so no party leaves in the mod, and a founded town has
  no desk to spawn at; `land-adapter`); hands going away and coming home with caravans or a war produce no output
  (`wagon-adapter`); the core's home-building adds beds the next census overwrites until homes are placed, and the
  free/confined split isn't counted (`land-adapter`); in Java mode the core's people outputs are not shown (the
  adapter writes villagers only for a host that owns them, the chests' flag), but the core still advances its hands
  there, so its roster drifts from the world's until a core-mode census; vanilla-bred babies join as the core's children (two sources of births, as in Java mode); GrowthTests,
  JobsTests, LabourTests and ExpeditionTests still call the Java rules (`Jobs`, `Labour`, `Households`, `Growth`,
  `Expeditions` stay until `delete-java-rules`); two parity-partial tags added (GrowthTests.aContentTownWithASpareBedHasAChild,
  ExpeditionTests.aMerchantSettlesAVillageNobodyTradesWith); `Sightings.retired` keeps a tag per hand ever gone with
  settlers (unbounded); a core child's villager grows by vanilla ageing and `Growth`, not at the core's `baby_until`;
  `lastChunk` isn't saved, so after a restart every villager away from loaded ground is sent as the core has it (one
  removed with no death stays a hand); in core mode the Java `Settlements` roster goes stale beside the core's
  (`/enterprise books` reads it). The save grew at FORMAT 1: dev worlds'
  `enterprise_core.dat` are rebuilt.
- **Trade points and hands** (B4.2, `adapters/PointsAdapter`, `adapters/HandsAdapter`): every hand on a roster has a
  stable `HandId` (`roster::Hands`, saved: given as it joins, kept through a new trade, a caravan, a war, settlers;
  never reused) with the host's name for it (a villager's UUID) beside it, so the mod keeps no ids: both adapters are
  rebuilt from the core (`Query::Points`, `Query::Hands`) after a load. `Locker.hand` and `Lockers.clients` go by id.
  Wire: `Input::AddHand`, `Input::HandLost` (a hand away or gone by the time it applies, after the look's `Advance`, is
  a stale view, ignored), `Query::Hands`/`Answer::Hands` (`lockers` is the Merchant's cap by level). `/enterprise core
  points`, `/enterprise core hands <desk>`. `people-adapter` kept `AddHand` beside `Census` (see People). **Open:**
  each join or loss re-asks every town's hands. In core mode
  `Merchant.lockersAt` still falls back to Java's `lockerLimit` when the core has no town at the desk or refuses the
  villager (a Java rule deciding in core mode, logged only). Points other than desks have a registry but nothing
  registers them (their adapters). `PointsAdapter.Kind` and `HandsAdapter.Hand` hand-mirror `PointKind`/`HandView`.
  `Roster.merchant` and `Lockers.managed` are still places; `set_roster` keeps ids by place (fixtures); tags of hands
  the rules remove (a soldier fallen) are never pruned. The save grew at FORMAT 1: dev worlds' `enterprise_core.dat`
  are rebuilt.
- **The content pack** (B4.1, `binding/ModPack`): the mod's goods, comfort kinds, trades, recipes (`PackRecipe`),
  herds (`PackAnimal`), each good's stack weight (`PackGood.weight`, which was `content-weights`) and dials
  (`PackDial`) cross as one `ContentPack`; `Content::from_pack` names everything wrong at once. The pack's dials are
  the world's start (`Content.dial_defaults`, saved; `reset` returns to them). `Input::SetDial`, `Query::Dials`.
  `/enterprise dial` in core mode lists and sets the core's (`adapters/DialAdapter`); in Java mode it sets the Java
  dial and the core follows (and at start, for what the world had set). `Content::good` now matches a name with or
  without the host's namespace, so in core mode the core's bread, wheat and hay_block rules and the vanilla
  `default_offers` find the mod's goods (they found nothing before), and pallet caps bind on the pack's weights.
  **For John to confirm:** the weights (`ModPack.WEIGHTS`, finished by `content-weights`: raw metal, coal, bricks,
  ingots, logs and hay blocks 2 a stack, stone, cobblestone and sand 3, the rest 1; mirrored by hand in
  `sim/host/src/goods.rs`, nothing checks the two match; stone and brick weigh nothing in the game yet, since the
  mod's pack lists neither); the mod's `birth_mood` 0.5 goes in the pack and the core's default stays 1.0. **Open:** 7 mod dials have no core
  counterpart (`ModPack.JAVA_ONLY`: `dealer_take_profit`, `discount_days`, `correspondent_floor`, `wagon_pays`,
  `call_start`, `pastures_max`, `feed_days`), they come with the rules that read them; the pack's recipes, herds and
  trades are read from the Java rule tables (`Workshop.CONVERSIONS`, `Husbandry.KINDS`, `Squad.TRADES`) and `Dials`,
  `Prices`, `Comforts` stay until `delete-java-rules`; dials keep two saved stores (Java's `Dials` and the core's), so
  a world switched between modes runs them apart (a core-mode setting unseen by Java, a Java-mode one never synced to
  core mode); `ContractTests.aDialIsSetForTheWorldAndSavedWithIt` still tests the Java catalogue (rule API, retires
  with `Dials`: `CoreBindingTests.aDialSetInTheCoreIsKeptByItsSaveAndResetToThePacks` is its core twin); `Content`
  grew a field at FORMAT 1, so existing dev worlds' `enterprise_core.dat` (testbench, checkpoints) need rebuilding; a
  running world keeps the pack it was made with; recipes, herds, weights and comforts can't be asked back over the wire.
- **The society switch** (B3.2, `binding/Society.java`): `java|core`, read once at mod construction from the system
  property `enterprise.society`, then env `ENTERPRISE_SOCIETY`, then `society=` in `config/enterprise.properties`;
  default java. In core mode the 11 rule tick listeners (and `LockerCatchUp`'s join, which queues for its tick) aren't
  registered and `CoreClock` is the only clock; views (joins, deaths, chunks, desk listeners) stay. `just gametest-core`
  runs the suite in `core/run/gametest-core` and checks passes per class against `scripts/gametest-core-baseline.txt`;
  `trees.py`'s `--java` gates (check and land) now run it after the Java suite. **Core mode is a frozen society today**:
  only `StockAdapter` applies anything, and the baseline passes because the tests call the Java rule classes directly,
  so it binds only as B4 tasks move tests onto the core. The mode is per process, not saved with the world: a world run
  in one mode and then the other advances the Java SavedData and `enterprise_core.dat` apart, unsaid. Open: the
  `CensusTaker` poller stays off in core mode (its counting half now feeds `PeopleAdapter`); no JVM test covers
  `Society.parse`.
- **The mod runs the core** (B2.1, `dev.b28.enterprise.binding`): `CoreHost` (started with the server, saved with the
  overworld, closed at stop; the core on `CoreWorker`), `CoreClock` (a look every 1,200 ticks), `Outcomes` (every
  Output routed to "unshown" until its adapter lands), `ModPack` (Prices' goods), `adapters/StockAdapter` (a desk a
  player places is a town, one broken a point removed; touching chests counted at each look); `/enterprise core stock
  <desk>`. The save is `SaveStore`'s plain file, `world/enterprise_core.dat` beside `level.dat`, not a SavedData.
  Measured by proxy in the game-test server (not a headless server's whole tick): a look 0.05 to 0.07 ms on the
  server's tick, 2.4 to 2.6 ms on the core's thread, a 39 KB save 2.2 ms. **Every server now runs the core beside the
  Java rules**, Java mode included: the java|core switch is `core-mode`'s. **Pallets have no Java** and get none: the
  three pallet outputs reach `Outcomes` and stop there until `pallet-views` (the mod's crates drawn from the core's
  messages) is placed.
- **Open from `proof-in-mod`:** the jar-in-jar (binding plus Chicory's runtime, wasi, wasm, log at `chicory_version`
  in `gradle.properties`) is built but never launched outside a dev run: one Prism launch before release. Registrations
  and `/enterprise core stock` wait on the server thread behind a look still running (seconds on a long catch-up), and
  so does a save (`settle`). For `stock-adapter`: desks changed unseen (explosions, pistons, `/setblock`, Create), the
  testbench desk (placed by a datapack, registered only by the command), shulkers, depots, `Moved` write-back. The
  core copies a town's name from Java's `Names` at founding, which re-names once the land is surveyed: the copies can
  drift (`people-adapter` left it: for `land-adapter`, which surveys the land). `CoreHost.of` ignores its server
  (one host a JVM).
- **Saves are versioned** (docs/binding-plan.md 1.4): `save::migrate`'s chain (empty while `FORMAT` is 1, its recipe
  for the first bump in `save/migrate/mod.rs`), `Load` refusing by number what the chain doesn't reach, `Core.start`
  saying a jar from two commits at start, `SaveStore` for `enterprise_core.dat` and `enterprise_core.prev.dat`.
  **For John to confirm:** prev.dat is copied at the first save after a migration and never deleted (the plan's "kept
  ... until the next save" read that way; deleting it at the next save instead is one line in `SaveStore.save()`).
  The migrating load path runs end to end only over the template's chain until the first real format bump.
- **The core runs in the JVM** (`binding/`, a plain Java 21 Gradle subproject, Chicory 1.7.5): `Core.call(Request)`,
  the trap path (fresh instance, replay the checkpoint and the Applies since, throw `CoreTrap`; stuck if the replay
  fails, or if a state-changing request's response can't be read), `CoreWorker` (the core on its own thread). The module
  is compiled to JVM classes at build time (`CompileCore`, a plain JavaExec), and the build fails if any function falls
  back to Chicory's interpreter (`ENT_ALLOW_INTERPRETED=1` to build anyway). Gradle runs `wire-check` before
  compileJava and the codec check before the tests. A wire change: `just wire-gen`, `just wire-fixtures`, then `just
  binding-test` (ScenariosTest and FixturesTest fail until the fixtures and postcard twins are written again).
- **Missed: the look bar** (docs/binding-plan.md section 4): a compiled look is 2.34 ms on the study world (38x native),
  9.1 ms on 25 towns (17x), against under 1 ms. HotSpot won't JIT the 75 compiled methods over 8,000 bytes (the hot
  systems). **Not planned yet, for John to place:** a core task splitting those functions in Rust (CompileCore's report
  lists them; an estimated 3x), touching `people::live`, `rules/plan.rs`'s `decide` and `plan_sale`, `services::plan`,
  `desires::plan_shopping` and more.
- **For John to accept (a design change beyond the plan):** looks run off the server tick on `CoreWorker`, so outputs
  come back a tick or more after the server gathered a look's happenings: the chests counted for a `Stocktake` can
  change before the outputs arrive, and a trapped look's inputs are dropped (not retried). The plan's 1.5 said a look
  over 1 ms stays where it is and the clock spreads it; `proof-in-mod`'s `CoreClock` builds on whichever is chosen.
- **Open:** `trees.py check` runs only `cargo test`, not `just binding-test`, so a wire change that skips `just
  wire-fixtures` lands with a red binding-test unless the integrator runs it (as here). `Core`'s replay log grows
  until the next Save (600 looks replay in about 1.4 s on the study world).
- **The wire is generated** (`sim/wire-gen`, `just wire-gen`): `sim/wire/registry.yaml` (serde-reflection 0.6.0's
  trace from `Request` and `Response`) and Java 21 in `binding/src/generated/java/dev/b28/enterprise/wire/` (a record
  per struct, a sealed interface or a Java enum per enum, postcard codecs, the runtime, `Wire.ABI`/`SCHEMA`). Never edit
  the generated folder by hand. `wire::SCHEMA` is now the FNV-1a 64 of the committed registry, computed when the core
  compiles: any wire change moves it, so `just wire-gen` then `just wire-fixtures` (the golden Request and every
  scenario carry the schema) and commit all of it. Enums are append-only against the committed registry (`--renumber`
  says so out loud); struct field order and a variant's payload aren't held, they only move the schema. A doc comment
  on a wire type is copied into the Java, so editing one fails `wire-check` until `just wire-gen`. A new wire enum
  needs a line in `registry.rs::trace_wire`. The schema is above `i64::MAX`: a host reading the JSON fixtures parses
  `schema` as an unsigned 64-bit number, not a double.
- Java's `fromBytes` refuses trailing bytes; Rust's `postcard::from_bytes` ignores them.
- **Golden files and scenarios are in** (`sim/wire/fixtures/NAME.bin` + `.json` for 17 wire types;
  `sim/scenarios/{stocktake,inputs,refusals}.json`, played by `tests/scenarios.rs`): `just wire-fixtures` writes both
  after a wire type or a rule moved. Samples are hand-written (`wire-gen` added the registry check, not
  serde-reflection's samples). A host compares a refusal by its kind and numbers, never its `why` prose: `chicory-host`
  must do the same. No scenario shows an Output yet (each needs a master Merchant or a route, and no wire input makes
  either): the first task that adds such an input adds the scenario. Ids cross as `u32` (`wire::TownNo`, `RouteNo`,
  `GoodNo`, an order's place, the pallet wire's places and lines too): the core counts in `usize` and narrows through
  `wire::no`, which panics past `u32::MAX` rather than truncate. Each scenario step carries postcard hex twins
  (`request_postcard`, `response_postcard`), so a host needs no JSON reader of wire types.
- **The module is built** (`sim/wasm`, `just wasm`): `enterprise_core.wasm`, 4,070,146 bytes, ABI 1, exports
  `ent_abi`/`ent_alloc`/`ent_call`/`ent_free` and `memory`, imports only WASI's `fd_write`, `proc_exit`,
  `environ_get`, `environ_sizes_get` (no clock, no `random_get`), no `_initialize` for Chicory to call. The landing
  gate (`trees.py check`) doesn't set `ENT_WASM_REQUIRED`, so on a machine without node or the `wasm32-wasip1` target
  the module tests skip and count as passes: `just wasm-test` is the one that insists.
- **The wire is in** (`sim/core/src/wire/`): `wire::handle(&mut Option<Society>, Request) -> Response` and
  `handle_bytes` (postcard). `SaveFile` is checked only for its format (`save::FORMAT` stays 1 while dev worlds are rebuilt).
- **Trade points are core state already** (`points.rs`, landed with `wire-types` because the proof needs them):
  `AddTown`, `AddPoint`, `RemovePoint`, `Stocktake`, `Query::Points`. `points-and-hands` (B4.2) builds on them for
  the Java side and adds hands; it doesn't make points again.
- **Settled by `stock-adapter`:** a point broken and placed again no longer counts its chests twice, and chests that
  lose more than the town holds keep it as the point's `short`. Still open: `Query::Stock` answers as last touched.
- **Placed at landing, for John to confirm the order:** `pallet-views` (B4.12 below). The mod gets no pallet rules:
  it registers its crate items and blocks once and draws each pallet's contents, moves and openings from the core's
  three pallet outputs, as `PointsAdapter`/`HandsAdapter` draw points and hands. The weights it needs are in the pack
  (`content-pack`, `content-weights`); since `packing` it also needs a `PackPallets` command for
  `Society::pack_pallets` (set-aside and `crate_room` guards), `Good.package` and `.handling` on the wire's `Good`
  (barrels, bundles, goods that need care), and since `superbills` the manifest on the wire.
- **The game tests are sorted** (`python3 scripts/parity.py --binding`, every test in `docs/binding-tests.md`): 150
  world fixtures (the binding's acceptance tests), 66 rule API (retire with their class), 46 Minecraft's own (view
  tests), of 262. About twice the fixtures the plan first guessed (121), because shared helpers in other test files
  build the world (`CaravanTests.network`, `LabourTests.town`): the B4/B5 acceptance work is about twice the estimate.
  Two rule-API tests have only a partial Rust twin and go to the core first: `CargoTests.aCrateTakesTheSpaceOfWhatsInIt`,
  `TollTests.aBridgedHexIsOpenGround`. 27 rule-API tests hand the level to a Java rule (marked *level*): check each
  rule reads no terrain before its test retires.
- **Pallets** live in the core only (`docs/decisions.md`, 2026-10-18): the mod draws them through the binding, with no
  pallet rules in Java; weights are content-pack data.

## Tasks

| Order | Task | Model / effort | Waits for | Landed | Notes |
|---|---|---|---|---|---|
| B1.1 | `wasm-digest` | opus / high | — | `c8fbc0a` | Every ambiguous system pair pinned as it ran natively; `tests/schedule.rs` guards new ones |
| B1.2 | `wire-types` | opus / high | `wasm-digest` | `a91fab3` | `wire/`: Request/Command/Response/Refusal, Input/Output moved there (re-exported from `messages.rs`), Query/Answer (`Stock`, `Points`), ContentPack and `Content::from_pack`, Point, SaveFile; a look is checked whole before any of it applies. Beyond the brief: trade points as core state (`points.rs`, in `saved!`) and four inputs, with a `trade_points` random-worlds slice. At landing: rebased onto pallet-types (digest written again); an `Advance` more than 36,500 days past now refused (`u64::MAX` would have looped without end); `RemovePoint`/`Stocktake` naming another kind than the point there refused. Left: SCHEMA by hand (`wire-gen`), fixtures (`wire-fixtures`), re-placed point counts twice (`stock-adapter`) |
| B1.3 | `wasm-exports` | sonnet / medium | `wire-types` | `128274b` | `sim/wasm` (cdylib + rlib), four exports over `wire::handle_bytes`, one society per instance; `[profile.wasm]` (panic abort, stripped, lto) and the workspace line accepted; `wasi-run.mjs --reactor`. At landing: the proof loads its own save over another world and carries on with trade, then saves again (16 requests, byte for byte in Node's WASI); the framing test asserts every refusal it claimed; the nested build names its target folder. Left: the wire has no people inputs yet, so the proof's days move little (real simulation across native and wasm is `core-wasm-replay`) |
| B1.4 | `wire-gen` | opus / high | `wire-types` | `a5e771e` | `sim/wire-gen`: registry.yaml and 21 Java files from the wire types (refuses flatten, untagged, maps, tuples, arrays, u128, char, units, Java name clashes, a type not defined in wire/, one nothing sends; enums append-only); `wire::SCHEMA` the registry's FNV-1a 64; `just wire-gen`/`wire-check`/`wire-javac` (216 codec checks against postcard's bytes); the core's three source-scan tests retired for it. At landing: the source scan's serde-attribute and path-field refusals carried over (serde-reflection traces Deserialize only, so `skip_serializing_if` would have traced plainly and misread); a backslash in a doc escaped (javac's unicode escapes); rebased onto wasm-exports and wire-fixtures, fixtures and scenarios written again for the new schema; plan's serde-reflection version corrected. Left: Gradle wiring and the codec check in a gate (`chicory-host`), ids `usize`, struct field order not held |
| B1.5 | `wire-fixtures` | sonnet / medium | `wire-types` | `6a0b53c` | Golden `.bin`/`.json` for 17 wire types (every variant, numeric and length edges); three scenarios (`stocktake`, `inputs` with every Input and Query, `refusals` with every Refusal, NaN and cut bytes as hex), the same answers natively and under Node (`just scenarios-wasm`); `wasi-run.mjs` preopens sim/ as `/sim`. At landing: refusals matched by kind and numbers, not prose (postcard's error text, a Spot's Debug); Outputs counted by the coverage test and noted unreachable from the wire; the variant-place check covers every wire enum; stocktake's notes say its answer counts a delivery and the desk's first count apart (stock-adapter's double count: the answers move then). Left: serde-reflection samples and `wire-check` (`wire-gen`), ids `usize` (`wire-gen`), no Output scenario |
| B1.6 | `chicory-host` | opus / high | `wasm-exports`, `wire-gen`, `wire-fixtures` | `d48478f` | Wire ids `u32`; `binding/` on Chicory 1.7.5 (build-time compiled), `Core` with its trap path, `CoreWorker`, scenarios' postcard twins; `just binding-test` (87 JVM tests), `just binding-spike`. Spike: a compiled look 2.34 ms (38x native) on the study world, bar of 1 ms missed (HotSpot interprets 75 methods over 8,000 bytes); every other bar met (instantiation 76+89 ms, jar +2.3 MB, heap 12 MB, Save 10 ms / Load 36 ms). At landing: rebased onto pallet-messages and pallet-claims with the pallet wire's ids made `u32` too (the audit's blocker; schema `0c8b594f0b2eba2a`); `wire::no` narrows ids loudly where outputs used `as u32`; `Core` goes stuck on an unreadable response to a state-changing request; the build fails on any interpreter fallback; game tests run (262 pass). Left: the hot-function split (a core task to place), CoreWorker's off-tick looks for John to accept, binding-test outside `trees.py check` |
| B1.7 | `versioning` | sonnet / high | `chicory-host` | `25e0895` | `save::migrate` (chain, `OLDEST`, recipe for the first bump), `Load` refusing unreachable formats by number and a header that disagrees with its `SaveFile`; `tests/saves.rs` with the format-0 template and the golden-save test (no goldens while `FORMAT` is 1); `Core.start` probes ABI and schema; `SaveStore` with prev.dat; 98 JVM tests. At landing: `save::upgraded_with`, so the template runs the migrating load path (header off, payload through the chain) end to end; the plan's Built paragraph moved after its list with the SavedData choice for B2.1. Left: prev.dat's lifetime for John, SavedData or plain file (B2.1), the first real migration as the first end-to-end test |
| B2.1 | `proof-in-mod` | opus / high | `versioning` | `84802f2` | The mod depends on `binding` (dev classpath, jar-in-jar with Chicory); `CoreHost`, `CoreClock`, `Outcomes`, `ModPack`, `adapters/StockAdapter`, `/enterprise core stock`; `enterprise_core.dat` as `SaveStore`'s plain file; `Query::Goods` added to the core (schema `208b58181a4f9956`, FORMAT 1); two game tests (264 pass). Measured in the game-test server: 0.05-0.07 ms a look on the tick, 2.4-2.6 ms on the core's thread, 2.2 ms a save. At landing: the desk listeners log the core's failures instead of throwing into a placement and listen at LOWEST (cancelled events don't reach the core); `found` advances the core to now before `AddTown`; a look at start catches a new society up to an old world; a look counts a desk only with its neighbours' chunks loaded; the plan's diagram, 1.4 and 1.5 say plain file. Left: a Prism launch of the jar-in-jar, sync calls waiting behind a look, unseen desk changes (`stock-adapter`), `Names` drift, the java/core switch (`core-mode`) |
| B3.1 | `gametest-triage` | sonnet / medium | — | `59b7b77` | `parity.py --binding` writes `docs/binding-tests.md`: 150 fixture, 66 rule API, 46 own; 2 rule-API tests with only a partial twin go to the core first. At landing: helpers followed across test files and a discarded entity counted (5 tests moved to fixture: BuildingTests' three and CountingHouseTests' clerks via `LabourTests.town`, `GrowthTests.childrenGrowUpWhileAway`); rule-API tests that pass the level to a rule marked *level* (27); the plan's section 2 counts updated |
| B3.2 | `core-mode` | sonnet / medium | `proof-in-mod`, `gametest-triage` | `5532ae3` | `Society` (java\|core, per process: property, env, `config/enterprise.properties`; default java) gates the 11 rule tick listeners; `CoreClock` the only clock in core mode; `just gametest-core` (own folder, `GameTestReport`, `scripts/gametest_core.py`, baseline 264 of 264). At landing: `LockerCatchUp`'s join gated with its tick (it queued jobs nothing drained), the core-mode report cleared before a run and an empty or missing one fails (a stale 264 could pass), the Java run pins `society=java` (an exported env var switched it), `trees.py`'s `--java` gates run the core-mode suite. One flake at landing (`wagesClimbTowardEverythingTheHarvestSellsFor`, share 0.5; passed on the retry, no Java changed in the rebase). Left: core mode is frozen until the adapters land, `CensusTaker`'s counting half off with its rule half (for `people-adapter`), the mode not saved with the world, no JVM test of `Society.parse` |
| B4.1 | `content-pack` | sonnet / high | `core-mode` | `f48f053` | `ContentPack` carries goods (with stack weights: `content-weights` folded in), comforts, trades, recipes, herds and dials, checked whole by `Content::from_pack`; the pack's dials are the world's start; `SetDial`/`Query::Dials`; `/enterprise dial` through the core in core mode, mirrored to it in Java mode; `Content::good` matches with or without a namespace; 3 game tests (267 both modes), schema `e66ad7c365d84fd0`. At landing: a dial documented with its group shows the group's words (79 of 219 had none in core mode); the command reads an adapter record, not the wire's `DialView`; `mirror` checks the core's dial names instead of its English error, and warns when Java takes a value the core refuses; start-up sync sends dials in name order. Left: weights and `birth_mood` for John, 7 Java-only dials, Java tables still the pack's source until `delete-java-rules`, two dial stores across a mode switch, no Java class deleted |
| B4.1a | `content-weights` | sonnet / medium | `content-pack`, `pallet-caps` | `0cc5292` | The pack's weights finished as data, no Java rule: stone 3, coal, brick and hay_block 2 joined `ModPack.WEIGHTS`; the same table in `sim/host/src/goods.rs` (plus a test); a decisions.md entry with the numbers and why. Measured: the 365-day study (default and `--river --services --metal --trades --freight`) and the 300-seed sweep identical with and without (wagons leave with about 2 stacks, far under the 27 cap), ms a day unchanged. At landing: stone and brick said to wait for content the mod's pack lists (neither is priced or in a recipe, so both weights are lookups only in the game), hay's 2 given its reason, the tables called hand-kept, comments wrapped. Left: John to confirm the numbers, no check that the mod's and the host's tables match, the weights bite only once handling units fill wagons |
| B4.2 | `points-and-hands` | opus / high | `content-pack` | `bf44ab1` | Stable `HandId` in the core's roster with the host's tag beside it (`roster::Hands`, saved), lockers and clients by id; `AddHand`, `HandLost`, `Query::Hands`; `PointsAdapter` and `HandsAdapter` rebuilt from the core; `/enterprise core points`, `hands <desk>`; 2 game tests (269 both modes), fingerprint unchanged. At landing: rebased onto packing, content-weights and superbills (digest written again); a `HandLost` whose hand an earlier `Advance` in the look sent away panicked the core (a wasm trap): now a stale view, ignored, tested; the random-worlds check refuses more lockers than hands; `Answer::Hands.lockers` documented as the cap. Left: only the Merchant joins, on its screen (`people-adapter`: the census, births, levels written back, `AddHand` against the plan's `Census`), Java's `lockerLimit` as core mode's fallback, other points' registration (their adapters), positions for `merchant`/`managed`, unpruned tags |
| B4.3 | `stock-adapter` | opus / xhigh | `points-and-hands` | `d19bb85` | Chests stay physical and show the town's stock: `Stocktake` gained `free`; `Output::Moved`, `Input::Unmoved`, `Query`/`Answer::Chests` (`chests.rs`, `FormerDesks` saved); `RemovePoint` takes the chests' goods out and a desk put back is its town again; `StockChests` (loose and shulker contents, crates left shut) and `StockAdapter` (counts each look, writes in core mode, sends every move back in Java mode, catches up loaded chunks, founds unseen desks, depots from a floorplan); 8 game tests (280 both modes), fingerprint unchanged, schema `a7833bce12ce3273`. At landing: depots follow their building's floorplan at each look (a chest broken and put back, or added since, was never the core's again: the audit's blocker; game test `aChestPutBackInAStorehouseIsItsTownsDepotAgain`); chunks caught up in a fixed order (desks found unseen got run-dependent town numbers); an `Unmoved` at a locker, shop or banker ignored (tested); rebased onto people-adapter and itinerary-drops (Unmoved Input 19, Moved Output 13; registry, Java, fixtures, scenarios and digest written again). Left: depot kinds in the core, the rules' room from counted chests, shulker room in `free`, crates (`pallet-views`), components ignored when counting, endless Moved/Unmoved for an unmovable good, spawn-chunk desks, the stock tests in LogisticsTests and MerchantTests not rewritten onto the core, Java's stock rules until `delete-java-rules` |
| B4.4 | `money-adapter` | sonnet / high | `stock-adapter` | `926498c` | A town's money in its desk's chests and Blaze Banker, counted and shown by the core (`till.rs`, `Tills` saved, coin piles as `Hoard`s of spurs and worth): `Cashcount`, `CashUnmoved`, `Output::Cash`, `Query::Cash`/`Mint`, `Reason::Withdrawn`; `MoneyAdapter` (counts after each desk stocktake, writes coin as the fewest stacks, the Banker's balance, emeralds), `StockChests`' coin methods; `money.json` scenario; 6 game tests (287 both modes), fingerprint unchanged, schema `ccb902b478f9a78f`. At landing: emeralds put only in the room the look's stock moves left (show_cash took room from the stocktake's free slots, so a nearly full desk was written emeralds every look that came back unmoved; `show_cash_after`, test `emeralds_are_put_only_in_the_room_the_stocks_moves_leave`); a withdrawal never posted below nothing (a negative treasury would have taken a negative `Withdrawn`); rebased onto people-adapter's landing fixes (binding-tests.md regenerated). Left: reserve 0 in core mode until `locker-adapter`, no wage float, a failed coin write's mint pressure kept, one desk a town shown, the money tests not rewritten onto the core, two mints until `shop-adapter`/`delete-java-rules`, John to confirm a desk's first count sells its chest |
| B4.5 | `people-adapter` | opus / high | `points-and-hands` | `2874c6b` | The census as a town's roster's source (`census.rs`, `Sightings` saved): `Census`, `TagHand`, `Resident`; outputs `Born`, `Settled`, `Departed`, `HandGone`, `HandLevel`; `PeopleAdapter` counts loaded towns (48 blocks round the bell, no path checks), spawns villagers for hands with none and writes profession, level and XP; `CensusTaker`'s counting half shared; `/enterprise core census <desk>`; the first Output scenario (`people`); 3 game tests (272 both modes), fingerprint unchanged, schema `e0b54576b14c0176`. At landing: a villager not loaded and last seen where nothing is loaded sent as the core has it (it was aged out and came back a new hand, breaking lockers and clients); names sent with every look until the roster takes them (a refused look lost them and hands were spawned twice); `/enterprise census` refuses in core mode; rebased onto consignment-pallets (digest written again); decisions.md "People and the binding". Left: settlers' views and founded towns' desks (`land-adapter`), away hands' outputs (`wagon-adapter`), census beds overwriting the core's homes, no free/confined split, births across a mode switch, the Java people rules and their tests until `delete-java-rules`, John to confirm the census never takes `none` or the Merchant's trade. After landing (`cf4110f`, the audit ran after the land): names of villagers just spawned sent before every save (the audit's blocker: a save and stop before the next look doubled a born or settled hand; game test `aVillagerSpawnedForAHandIsItsHandsInTheSave`), people outputs not shown in Java mode (all five, not only births). Left besides: `Sightings.retired` unbounded, two clocks for a child's growing up, vanilla breeding unsuppressed, `lastChunk` not saved, the Java `Settlements` roster stale in core mode, the look path's people outputs (Settled, Departed, HandGone, HandLevel) not driven end to end in a game test, no cap on towns counted per look |
| B4.6 | `locker-adapter` | opus / high | `stock-adapter`, `people-adapter` | `97e92b8` | The core runs the lockers, the mod places them (`locker_host.rs`, `Lockers.placed` saved): `Input::Locker`, `Input::Switch`, `Query`/`Answer::Lockers` (`LockerView`, `OfferView`); locker chests counted and emptied into the desk, used from first; `LockerAdapter` places each loaded villager's locker each look in core mode and builds the Merchant's screen from the core; the restock mixin runs the Java locker rules only in Java mode (`Rounds`, `LockerCatchUp` idle in core mode); `/enterprise core lockers <desk>`; `lockers.json` scenario; 2 game tests (292 both modes), fingerprint unchanged, schema `190c9fa413786e45`. No pallet code in Java (pallets are the core's; `pallet-views` draws them). At landing: rebased onto money-adapter and land-adapter (the audit's blocker: Switch and Locker renumbered to Input 26 and 27, Lockers to Query and Answer 10; registry, Java, fixtures, scenarios and digest written again; locker goods carried to a desk already take room before `show_cash`'s emeralds, via `slots_taken`); a villager whose workstation isn't loaded keeps the locker the core has (it was untied and its point removed, booking its goods out); the screen in core mode never falls back to the Java lockers (their switches wrote `MerchantBook`); offers shown at the Merchant's price; a test message corrected. Money's core-mode reserve no longer 0 once a spending trade is switched on. Left: rolled offers and players' trades (`set_offers`), the screen's limit from Java, emeralds in lockers uncounted, the locker tests on the Java rules, wire types in the command, John to confirm emptying lockers into the desk and the reshaped Switch/Trades |
| B4.7 | `land-adapter` | opus / high | `people-adapter` | `ad6fa04` | The host's survey into the core and the core's works laid by the host (`land.rs`, `LandBook` saved): `Ground`, `Crops`, `Survey`, `Built` in; `Build` out (bridge, wall by quarters, hall, field); `Query::Land`, `Query::Ground`; `LandAdapter` sends the ground in both modes and in core mode surveys loaded towns, counts crops every 20 looks and lays builds with the mod's placement; 3 game tests (290 both modes), fingerprint unchanged, schema `03941e1d6b77b641`. No pallet code in Java (the core owns pallets; `pallet-views` draws crates from its outputs). At landing: rebased onto money-adapter (wire variants after money's, fixtures, scenarios and digest written again); a work's chunks held loaded with a ticket of LandAdapter's own lasting four looks (the audit's blocker: Works' 200-tick ticket was gone before the next look asked again, so a work away from a player was never laid); builds laid at the start of the next look, once every look sent is shown (no look settled inside another's outcomes, a work laid and not yet said never laid again); `surveyNow` shows and lays what's waiting before taking what's to be said, and says it again when the host is paused; `CoreHost` no longer knows `Input.Built` (`LandAdapter.watch`); the hands scenario's unexplained level change reverted. Left: wooden pier, pastures, mine heads, dock/pastures/confined survey, settlers' desks, `Names` drift, unstaggered crop counts, fields paid before their site, row 7's tests on the Java rules |
| B4.8 | `wagon-adapter` | opus / high | `locker-adapter`, `land-adapter`, `money-adapter` | `cbe2285` | The core's routes started and stopped by the host, its wagons' crews stowed and let out by hand id (`wagon_host.rs`, no new saved state): `Input::Route`, `Output::HandAway`/`HandBack`, `Query`/`Answer::Routes` (`RouteLine`, `WagonLine`, `MarketLine`); `WagonAdapter` stows and lets out villagers (`enterprise_core_stowed.dat`), the Merchant's screen's caravans and Send/Stop and `/enterprise route`/`routes` on the core in core mode, `/enterprise core routes <desk>`; `wagons.json` scenario; 3 game tests (295 both modes), fingerprint unchanged, schema `ac8763985aaf102c`. No pallet, crate or paper code in Java (pallets are the core's; `pallet-views` draws them). At landing: a hand let out at a look takes its trade, level and XP from the core then (a `HandLevel` while its desk was unloaded was lost), a second `HandAway` keeps a villager already stowed, a wound-down route reopens only within a leg's food (test), the plan's 1.3 outputs row names HandAway/HandBack. Left: crate and paper views (`pallet-views`), herds and llamas, Java-mode crews across a switch, the screen's known-market check in Java, English `status` on the wire, row 8's tests on the Java rules, John to confirm routes wound down not deleted |
| B4.9 | `shop-adapter` | sonnet / high | `money-adapter`, `wagon-adapter` | `c3a00b4` | Numismatics vendors, the shop board and rent deeds are the core's, the mod the view (`shop_host.rs`, `Vendors` saved; `ShopAdapter`): `Input::Vendor` (a host's count, facts only), `Board`, `DeedShop`, `DeedBrought`; `OrderFilled.point`, `StallBought`, `Shelf`, `Deed`, `DeedPaid`, `BuildShop`; `Query`/`Answer::Shops`/`Deeds`; `shops.json` scenario; 5 game tests (305 both modes), fingerprint moved only by `OrderFilled`'s new field (`d8a270619eff9cdb`, stripped `5e6b6d51a3b4061e`), schema `dc7793b63586c15f`; `Shops.collect` sums coins (phantom takings in both modes). At landing: rebased onto pallet-views and commands-and-studies (Input 31-34, Query/Answer 24-25; registry, Java, fixtures, scenarios and digest written again); a look deals only with vendors counted for it (the audit's blocker: unloaded vendors were filled, bought from and stocked on the books); a box emptied only for the desk's own or a core-named deed shop (the audit's second blocker); a vendor's price changed is a new order (seed 7's paid check); bank and tenders wait on `delete-java-rules`; the refusals scenario's doc comment back. Left: coin collected before the look is accepted, unchecked card charges and stall lots, the deed card refunded each cycle, the crown's and others' vendors classified as players' (`crown-views`), enchanted commissions, row 9's tests on the Java rules, `Deed`/`DeedPaid`/`BuildShop` in no scenario |
| B4.10 | `crown-views` | sonnet / medium | `wagon-adapter`, `shop-adapter` | `be4bf7b` | The sovereign, its realms and the markets off the map are the core's, the mod the view (`crown_host.rs`, `CrownShop` saved; `CrownAdapter`): `Input::OffMap`/`Capital`/`Mobilise`/`Declare`/`Peace`/`CrownShop` 35-40, `Output::WarDeclared`/`PeaceMade`/`BuildCrownShop`/`CrownShop`/`CrownShopDown` 23-27, `Query`/`Answer::Realms`/`Metals` 26-27, `PointKind::Market`; `crown.json` scenario; 3 game tests (308 both modes), fingerprint `d8a270619eff9cdb` unmoved, schema `41c307f6fa0993a8`. At landing: the gold shop asked for again at each look counting the capital while none stands (the audit's blocker: the one request was lost with the host's unsaved `toBuild`); a shop taken down out of reach told to come down again when next counted (`CrownShop.down`); a refused vendor taken away; the books scenario's 500 back to 50. Left: no metal exchange, requisitions or freight vendors in core mode; the front and across the sea are points only; the card's balance at peace, a `fund` lost on an unloaded show; WarTests on the Java rules; the random-worlds slice opens no market and no war; a -1.8e-15 stock residue (`trade_points` clamps it) |
| B4.11 | `commands-and-studies` | sonnet / medium | `wagon-adapter` | `62ac9e3` | Every `/enterprise` command asks the core or says "Java mode only until <task>" in core mode (`commands_host.rs`, no new saved state; `BooksAdapter`, `CommandsAdapter`): `Query`/`Answer` `CashBook`, `Books`, `Whys`, `Trust`, `Contracts` (with history lines and rents), `Territory`, `War`, `RouteBooks`; `books.json` and `commands.json` scenarios; `network_study.py` the kept in-game study, the other studies Java mode only (`study_mode.py`); 2 game tests (300 both modes), fingerprint unchanged, schema `2121b86805df21de`. At landing: squashed and rebased onto pallet-views (queries renumbered after the pallets' to 16-23, commands_host's town-name helper renamed beside pallet_host's, registry, Java, fixtures and scenarios written again); `route remove` takes route 0 (core mode numbers from 0); freight, post, forwarder and paperwork wait on `delete-java-rules`, not the landed pallet-views; `--resume` refuses a checkpoint of the other mode; the autoswitch assertion made real; a duplicated doc line, the plan table's pipes and the world_map claim fixed. Left: the kept study never run under `society=core`, `day N:` lines worked out from state, territory's missing detail lines, a few command-side rules in Java, `depot add/remove`'s Java `Depots` |
| B4.12 | `pallet-views` | opus / high | `stock-adapter`, `content-weights` | `650078e` | Placed at `points-and-hands`' landing: the mod's crates and chests drawn from the core's pallet outputs, no pallet rules in Java; `PackPallets`, `Good.package`/`.handling` and the manifest on the wire. Widened at `wagon-adapter`'s landing: also row 8's crate and paper views, `CrateItem`, the crate block, `CrateMenu` and `client/CrateScreen`, and the post's papers (`DocumentsItem`: a pallet's bill of lading, a wagon's superbill), all from `PalletPacked`/`PalletMoved`/`PalletOpened`; `cargo/Crates`, `Papers`, `Paperwork` stay Java mode's until `delete-java-rules`. Built: the core's pallets drawn as tagged package items in the desks' chests (`pallet_host.rs`, `Stowage` saved; `PalletAdapter`), `Input::Pack`/`Stowed`, `Query`/`Answer::Pallets`/`Pallet`/`Stow`/`Manifest`, `PalletView`, `ManifestLine`, `PackPackage`; `pallets.json` scenario; 3 game tests (298 both modes), fingerprint unchanged, schema `b8fe731e20bb5c21`. At landing: a pallet found at its desk and said `Stowed` isn't drawn a second time by the same look's `Stow` (it was asked before the inputs applied); package crates aren't storage boxes to `MerchantStock` (`/enterprise stock` and the Java rules reached into packages 2..n); `Input::Pack` refuses a fractional count. Left: the superbill as a paper, crates at a wagon's unloading, `CrateMenu`/`CrateScreen` and empties, `Good.package`/`.handling` on the wire, the save's own package kinds, destroyed crates unsaved between looks, John to confirm pallets take chest slots ahead of loose goods |
| B5.1 | `delete-java-rules` | opus / high | all of B4 | | |
| B5.2 | `scenarios-in-game` | sonnet / medium | `delete-java-rules` | | |
| B5.3 | `upgrade-drill` | sonnet / medium | `delete-java-rules` | | |
