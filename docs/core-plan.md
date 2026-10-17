# Core plan: the Rust core to significant parity

The next phase, divided into tasks an integrator hands to agents in worktrees (`docs/worktrees.md`). No Java is
built or run: the mod's source is the reference and its 262 game tests are the spec (`python3 scripts/parity.py`).

**What "significant parity" means here:** every society rule the mod plays is in the core and pinned by a Rust test
that claims the Java test it matches; what's left uncovered is Minecraft's own (blocks, entities, items, the client,
Numismatics coins) and sits on the skip list with a reason. In numbers: of 262 Java tests, 47 are proposed for the
skip list (46 lines: the client file's two count as one); the plan covers the other 215 (about 190 in full, the
rest in part, where a test asserts a core rule through a Minecraft fixture). Done means: every Java package but `client`, `mixin` and the block/item halves of
`cargo` and `locker` has a Rust module; the random worlds exercise every mechanism; conservation, solvency and
bit-for-bit determinism hold every day; a world saves to a blob and loads back.

## 1. The gap

Today: 16 covered, 3 partial, 2 skipped, 241 left. The core has the trade loop, contracts with incoterms and trust,
bills and the reserve, people (wages, comforts, mood, homes, births, land). Its state is two components wide
(`Trades {makes, uses}`, `People`) and knows nothing of trades, land, terrain, the crown or war.

| Area (Java package) | Tests left | Core | Minecraft (skip) | What the core lacks |
|---|---|---|---|---|
| Caravans (`caravan`) | 49 | 45 | 1 (community by JOB_SITE) + 3 off to other areas | squads and crew, road food, the atlas and books, backhauls, desires, hubs and forwarding, realms |
| Contracts, bills (`contract`, `market/Bank,Discounts,Stranded`) | 14 + 5 counting-house | 18 | 0 | incoterm pricing tags, advances, harvest term and rents, bank, discount desk, stranded paper, cash book, accountant |
| Money (`money`) | 8 + 4 bargain | 6 | 6 (coin items, Blaze Banker) | mint rate and debasement, bargaining by level, par from capacity |
| Post, forwarder (`market/Post,Forwarder`) | 6 | 6 | 0 | letters, copies and bills by post, calls, consignment |
| Lockers (`locker`) | 7 + 16 | 14 | 4 blocks + 2 chunk-loading | villager trades as the town's income, the Merchant's span and level, catch-up replay |
| Market (`market/Broker,Shops,Rents,Tenders,Procurement,MetalExchange`) | 9 + 12 logistics | 15 | 1 enchant algebra + 5 chest/slot management | player orders, sourcing, tenders, training, procurement pacing, metal exchange |
| Territory, tolls, hexes (`territory`, `hex`) | 17 + 19 + 3 | 36 | 3 (surveys, names, coin items) | the hex map, terrain costs, pathing, culture growth, tolls and chokepoints, bridges' worth, guards, farms and wages |
| People and work (`territory/Jobs,Labour,Husbandry,Mines,Smithing,Workshop`, `service`, `settlement`) | 4+7+3+9+5+6+5+6+2 | 32 | 15 (structures, villager AI, enchanting, census POIs) | roster of trades, jobs draw, labour market, herds, workshop, mines and kit, services, masters |
| Buildings (`territory/Buildings,Works`, growth) | 5 + 8 | 7 | 6 (block placement) | works for wages, walls, halls, counting house |
| War, crown (`army`, `market/Sovereign,Requisitions`) | 7 (+2 caravan, +1 logistics) | 10 | 0 | the army, attrition, rations, levies, mercenaries, requisitions, the front |
| Cargo (`cargo`) | 4 | 2 | 2 (crate block/item) | shipments as a unit: consignee, papers, space, stationery |
| Infrastructure | — | — | — | save/load blob, dials set by name, more look steps, fixed-point money |

### Proposed skip-list entries (46 lines, 47 tests, grouped)

Add to `scripts/parity-skip.txt` (task `parity-triage`). One line each, reason after `#`.

**Client, registration, villager AI (4):** `MerchantScreenTests.*` (there already); `MerchantTests.deskIsMerchantJobSite`,
`MerchantTests.villagerTakesDeskJob` (POI/profession registration); `JobsTests.aStallIsALockerWithAWorker` (places a
lectern and hires a villager entity).

**Containers, slots, chunk loading (13):** `LockerTests.findsContainersTouchingWorkstations`, `.otherDesksAreNotLockers`,
`.deskStockIsTouchingContainers`, `.doubleChestCountsOnceWithBothHalves` (block adjacency); `LockerTradingTests.catchUpWaitsForItsMerchant`,
`.villagerWithLockerAsksToRestock` (chunk loading, a mixin hook); `LogisticsTests.anOffMapMarketKeepsItsWarehouseOnTheLedger`
(ledger-only storage is the core's only kind), `.aBuildingsChestsAreOneStoreAndHoldWhatItsFor`, `.aStorageCrateHoldsNineStacksInASlotAndTheStockReachesIn`,
`.shortOfRoomTheMerchantRepacksCratesPickedAt`, `.shortOfRoomTheMerchantPacksIdleGoodsIntoColdStorage` (chest slot management:
the core's warehouse has no slots; capacity is the binding's); `CargoTests.aCrateHoldsNineStacksAndItsPapers`,
`.aBrokenCrateKeepsItsGoodsAndPapers` (the crate block and item).

**Structures and world geometry (10):** `BuildingTests.aWatchtowerHasALadderUp`; `MetalTests.aMineHeadHasALadderDown`;
`HusbandryTests.fishermenBuildADockWhereLandMeetsWater`; `GrowthTests.aCottageFitsOnOpenGroundAndHasABed`,
`.fieldsAndWaterAreNoPlaceForAHome`, `.fieldsRingTheTownAndCutNoOneOff`, `.wallsRiseOnGroundAndLeaveFieldsOpen`,
`.aWallRingsTheBellWithFourGates` (the ring's size reaches the core as a blocks-needed number); `TollTests.surveysSeeWater`
(heightmap scan), `.marketsGetNamesThatStick` (names from biomes). Two more are mostly Minecraft but keep a core rule worth
a `parity-partial:` claim, so they stay off the list: GrowthTests childrenGrowUpWhileAway (see `roster`) and
ExpeditionTests aMerchantSettlesAVillageNobodyTradesWith (see `settlers-rule`).

**Census by pathfinding and POIs (6):** `SettlementTests.*` — bells, beds and workstations as POIs, villagers' walkability,
loaded chunks. The core takes a town's people as data.

**Numismatics coins and bank (7):** `MoneyTests.surplusEmeraldsBecomeCoinsInStock`, `.coinsAreRemintedIntoFewStacks`,
`.sellingOnlyMerchantsKeepNoReserve`, `.reserveIsNotSold`, `.blazeBankerTakesTheDeposit`, `.firstPlayerToOpenTheBankerOwnsTheEarnings`
(the emerald/coin item split and the 64-emerald working reserve are the binding's; the core keeps one money and the
mint's rate); `TerritoryTests.farmersTakeCoinsAtTheRegionsRate` (coin items with change).

**Enchanting (4):** `MarketTests.aCommissionIsEnchantedToOrder`; `MasterTests.aMasterClericEnchantsWithLapis`,
`.aClericLaysABooksEnchantmentOn`, `.aMasterLibrarianCopiesABook` (vanilla enchantment rolls and anvil rules; kit
quality and wear reach the core as numbers).

**Other (2):** `MoodTests.comfortsAreTags` (item tags; the kinds table is content); `CaravanTests.onlyTheDesksCommunityIsMustered`
(JOB_SITE memory and an 8-block radius).

## 2. Design decisions the plan makes

These bind every task. They're the plan's, not yet the decisions doc's; the integrator writes them up at landing.

1. **Trades are content; a town's people are a roster.** `Content.trades` lists the mod's 14 trades (merchant,
   cartographer, librarian, shepherd, leatherworker, fisherman, cleric, farmer, toolsmith, armorer, weaponsmith,
   mason, fletcher, butcher, plus `none` and `nitwit`). A town's `Roster` is a `Vec<Hand>` (trade, level, xp, baby,
   away). `People.villagers` becomes derived from the roster when one exists; towns without a roster behave as today.
2. **A locker is a hand's switched-on offers.** Offers (`good → emeralds`, uses per restock, two restocks a day) are
   content per trade; the town's income is what its managed lockers buy, within the mod's demand cap and the
   Merchant's discount. `Trades.uses` stays for towns without rosters.
3. **One money, and a mint.** The core's money is emerald-worth. The mint is a resource with the mod's pressure
   curve and debasement; the crown's minting enters through the ledger; the rate is read by the host and by
   realm prices, metal premiums and tolls. Coin items are the binding's.
4. **Places are hexes; roads are paths.** A town has a hex; a route has a path with a cost from terrain (water, relief,
   bridges) and tolls. `add_route(days, cost)` stays for tests that want a bare road.
5. **Every transfer is a posting.** `cash_book::post(world, from, to, amount, reason)` moves money between accounts
   (a town's treasury, its households, a wagon's purse, the outside) and the cash book tallies by reason and day;
   `check.rs` reconciles from postings. New rules use `post`, never `money.0 -=`.
6. **Food is the road's cost.** A wagon eats food points (1 per 10 blocks, 320 a leg) from home going out and at the far
   end coming back, buying it at the local price; `road_cost` in money remains the fallback when content has no food.
7. **The state saves.** Every component and resource derives `serde::Serialize, Deserialize`; `Society::save() -> Vec<u8>`
   and `Society::load(bytes)`; load-then-run equals never-saved to the bit.
8. **Randomness is a seeded generator in the state** (`Rng` resource), used only where the mod rolls dice (the jobs
   draw, Merchant XP, soldiers falling). Expected-value forms are preferred where the mod's roll is per head
   (attrition as a rate carried as a fraction), so results don't swing with town size.
9. **Minecraft fixtures become numbers.** Where a Java test pins a rule through blocks or entities, the Rust test
   pins the same rule with the test's numbers (plots, food points, hands) and claims it `parity:`; where only part of
   the assertion is the rule, `parity-partial:`.
10. **Systems in a step are ordered explicitly** (`.after(other_system)`); nothing relies on insertion order. The
    look gains steps `Work`, `Pay`, `Review` (after `Touch`) and `Mail` (after `Arrive`) so slices don't crowd `Touch`.

## 3. The common brief (paste first in every `.tree/TASK.md`)

```
You are building the Enterprise core: a society for factory games, in Rust (sim/core, bevy_ecs 0.20, single thread,
builds for WebAssembly), without Java. Read, in order: docs/worktrees.md (the rules you work under), docs/architecture.md,
docs/core-plan.md (your task's section, and section 2's decisions), docs/core-math.md, and the parts of docs/decisions.md
your task names. Read the Java it names as the reference: core/src/main/java/dev/b28/enterprise/. Never build or run Java.

Rules (docs/worktrees.md): stay in your tree. Your slice lives in its own files: data and components in
sim/core/src/SLICE.rs, systems in sim/core/src/rules/SLICE.rs with a `register(&mut Schedule)`, host calls in an
`impl Society` block in your module (Society's fields are pub(crate)), tests in sim/core/tests/SLICE.rs. Then one line
each in the shared lists, which git merges by union (add lines only, never change or remove one):
sim/core/src/slices.rs (your state in `saved!`, your `register` call), sim/core/src/dials.rs (your dials, one line each,
`name: type = default,` with the mod's defaults, as a block under a comment naming your slice at the end of the list),
sim/core/src/lib.rs (`pub mod`), sim/core/tests/common/slices.rs (your generator's name). Other shared files
(state.rs, rules/mod.rs, society.rs, arrive.rs, plan.rs) take additions only, in your own lines. Order your systems
with `.after(...)`; no clock, no HashMap iteration, no randomness but the world's seeded Rng. Every new component or
resource derives serde::{Serialize, Deserialize} and Default and is listed in `saved!` (tests/save.rs fails while a
world holds unlisted state). Every money transfer goes through cash_book::post (post_legs for one payment in several
legs), never `money.0 -=`. No new dependencies: Cargo.toml and Cargo.lock are the integrator's (serde, postcard and
bevy_ecs are there); if you need a crate, say so in your summary. Cite the Java file and methods in your module's doc
comment; port the mod's constants as `pub const` or dials with the mod's defaults.

Every behaviour gets a test in sim/core/tests/SLICE.rs. A test that matches a Java game test says so on the line above:
`// parity: File.method` (or `parity-partial:` when the Java test asserts more than the rule, or the Rust test pins
less than it does). Add your mechanism to the random worlds in sim/core/tests/common/slices/SLICE.rs (`extend` for a
new world, `daily` for a host's doings, `check` for your invariants; each gets a random stream of its own) so 300
seeds exercise it; don't edit tests/random_worlds.rs or the base generator. Conservation and solvency (check.rs) must
hold every day and the_same_seed_gives_the_same_world must pass. A refactor that mustn't change behaviour shows
`just core-fingerprint` the same as main's.

Gates: `cd sim && cargo test --release` passes; `python3 scripts/parity.py` shows your tags. Commit in your tree with
messages in the repo's style (what changed and why, measured), ending with the two attribution lines. Don't push,
don't land. When done, write a short summary at the end of .tree/TASK.md: what landed, what's partial and why, and any
Java you found unclear.
```

## 4. Tasks

Each task is one agent in one tree, an hour or two. The format: what it's for, the Java to read (methods, not
files), the Java tests it claims, the Rust it adds and the shared files it touches, the math it adopts, acceptance,
the model and effort, and its brief (pasted after the common brief).

Tests marked *(partial)* claim `parity-partial:`. "Touches" lists the shared files it adds lines to.

---

### Wave 1: foundations and independent ports

#### 1.1 `look-steps`

| | |
|---|---|
| Goal | Add the steps every later slice joins: `Work`, `Pay`, `Review` after `Touch`; `Mail` after `Arrive`. No behaviour. |
| Java | none |
| Covers | none |
| Rust | touches `rules/mod.rs` (the `Step` enum, the chain, the doc comment) |
| Accept | tests pass unchanged; the doc comment says what each step is for |
| Model | **haiku / low** — a ten-line, fully specified edit. |

Brief:
```
# look-steps
In sim/core/src/rules/mod.rs, add to `Step` and to the `.chain()` in look_schedule, in this order: after Touch:
Work (towns' people make goods: farms, herds, mines, gathering, the workshop), Pay (wages, guards' upkeep, rents' coupons),
Review (a town's daily decisions: jobs, fields, tolls, buildings, commissions); after Arrive and before Papers: Mail
(letters arrive). Update the module doc comment's numbered list. Nothing else changes. Run the gates.
```

#### 1.2 `parity-triage`

| | |
|---|---|
| Goal | The skip list, and tags for behaviour the core already has but hasn't claimed. |
| Java | the test bodies of the two claimed tests only: `CaravanTests.townsKeepTheirTradingCapital`, `.aDestinationThatCantPayGetsNothing` |
| Covers | `CaravanTests.townsKeepTheirTradingCapital`, `CaravanTests.aDestinationThatCantPayGetsNothing` (new small tests in `tests/trade_loop.rs`) |
| Skips | the 46 entries in section 1, grouped and commented as there |
| Rust | adds two tests to `tests/trade_loop.rs`; touches `scripts/parity-skip.txt` only |
| Accept | `python3 scripts/parity.py` shows 47 skipped, 18 covered; `--left` lists no skipped test |
| Model | **haiku / medium** — mechanical: the list is written out in the plan; two tests against existing API (`free`, `trading_capital`, `sell`). |

Brief:
```
# parity-triage
1. Write scripts/parity-skip.txt from docs/core-plan.md section 1 "Proposed skip-list entries": one `File.method # reason`
   a line, grouped under the same five headings as comments. Keep the existing MerchantScreenTests line.
2. In sim/core/tests/trade_loop.rs add two tests: (a) a mill holding exactly its trading capital (reserve_days of what
   its people use, at worth) buys nothing from a wagon, and one holding more buys with the excess
   (`// parity: CaravanTests.townsKeepTheirTradingCapital`); (b) a far end with 0 money buys nothing and the wagon
   brings everything home (`// parity: CaravanTests.aDestinationThatCantPayGetsNothing`). Use farm_and_mill.
3. Run `python3 scripts/parity.py` and `--left`; the summary line should read 18 covered, 47 skipped.
```

#### 1.3 `contract-tags`

| | |
|---|---|
| Goal | Pin five contract behaviours the core already implements (or nearly), and add the one function missing: `Incoterm::price`. |
| Java | `contract/Incoterm.java` `price` (~42) and the three flags; `contract/ContractBook.java` `review` lines 301–360 (the CPT/DAP count at 323, the EXW branch 338–343, the partial branch 345–353), `Record.trust`/`at` (66–71), `missed` (558, teething) |
| Covers | `ContractTests.eachIncotermPricesItsBurdens`, `.underCarriagePaidToTheRoadIsTheBuyers`, `.exWorksGoodsLeftUncollectedAreTheBuyersMiss`, `.trustIsATrackRecordThatOneBadWeekDoesntUndo`, `.anInstallmentDeliveredInPartIsKeptInPart` |
| Rust | adds tests to `tests/contracts.rs`; adds `Incoterm::price(ex_works, carriage, tolls)` in `contracts.rs` |
| Accept | five new parity tags; each test uses the Java test's numbers (1/0.1/0.2 → 1, 1.1, 1.1, 1.3; 20 kept then 2 missed > 0.7; 27 of 30 → kept 2.7, missed between 0 and 0.3 at teething weight) |
| Model | **haiku / medium** — the rules exist in `contracts.rs` (`BREAKING_SHORTFALL`, `TEETHING_WEIGHT`, `seller_bears_road`, the `seller_holds` callback); the work is reading them and writing tests with given numbers. |

Brief:
```
# contract-tags
sim/core/src/contracts.rs already judges installments (review), keeps the trust record (Record::trust, kept/missed with
teething and half-lives) and knows incoterms. Read it, then the Java named in docs/core-plan.md 1.3. Add
`impl Incoterm { pub fn price(self, ex_works: f64, carriage: f64, tolls: f64) -> f64 }`: carriage added where the seller
carries, tolls where it pays them. Write one test per Java test in sim/core/tests/contracts.rs with the Java test's
numbers, tagged `// parity: ContractTests.<method>`. For underCarriagePaidToTheRoadIsTheBuyers use Book::ship then
review past the grace: CPT counts shipped goods, DAP doesn't. For exWorks use review's seller_holds closure returning
true. If a behaviour is missing, add the smallest change that gives it, in contracts.rs, and say so in your summary.
```

#### 1.4 `save-load`

| | |
|---|---|
| Goal | The state as one blob: `Society::save() -> Vec<u8>`, `Society::load(&Content-independent bytes)`, and dials set by name and saved with the world. |
| Java | `Dials.java` `set` (208), `save` (229), `load` (234), `apply` (220) — only for what "a dial set for the world" means |
| Covers | `ContractTests.aDialIsSetForTheWorldAndSavedWithIt`; and `ContractTests.aBrokenContractOwesBackItsUnearnedAdvance` *(partial: the save/load half; `advances-and-refunds` does the refund)* |
| Rust | adds `save.rs` (serialization of resources and components, entity order by `Town.id` / `Route.seq`), `serde` + `postcard` (or `bincode`) to `Cargo.toml` (must build for `wasm32-wasip1`); adds `Dials::set(name, value)` / `reset(name)` / `names()`; touches `state.rs`, `content.rs`, `contracts.rs`, `bills.rs`, `people.rs` (derive lines only), `society.rs` (`save`, `load`, `set_dial`), `lib.rs` |
| Math | — |
| Accept | a test: run 50 days, save, load into a fresh `Society`, run both to 100: outputs equal and every town's money `to_bits()` equal; `set_dial("shelf_days", 6.0)` survives save/load, `reset` restores the default; `cargo build --target wasm32-wasip1` still builds |
| Model | **opus / medium** — cross-cutting (every state type), and the entity-order and resource-registry design must be right for everyone who comes after. |

Brief:
```
# save-load
Give the core the save format docs/architecture.md promises: one blob. Add serde (derive) and a compact binary format
that builds for wasm32-wasip1 (postcard preferred). Derive Serialize/Deserialize on every resource and component in
state.rs, content.rs (Content, Good, Dials), contracts.rs (Book, Contract, Record...), bills.rs, people.rs. Write
sim/core/src/save.rs: a `Saved` struct holding every resource and, per town (in Town.id order) and per route (in
Route.seq order), its components; `Society::save(&mut self) -> Vec<u8>` and `Society::load(bytes) -> Result<Society, String>`
that respawns entities in the same order so Towns/routes vectors and Entity ids line up. Add `Dials::set(&mut self,
name: &str, value: f64) -> Result<(), String>`, `reset(name)`, `names() -> Vec<&'static str>` (a match over the fields;
a small macro is fine), and `Society::set_dial`. Tests in sim/core/tests/save.rs: load-then-run equals never-saved to
the bit over 100 days on a random world (reuse the generator in tests/random_worlds.rs by copying its `world` fn or
moving it to a shared tests/common/mod.rs); a dial set is saved and reset restores the default
(`// parity: ContractTests.aDialIsSetForTheWorldAndSavedWithIt`); a broken contract's state survives save/load
(`// parity-partial: ContractTests.aBrokenContractOwesBackItsUnearnedAdvance`). Document in the common brief's spirit:
every later type derives serde, and say so in save.rs's doc comment.
```

#### 1.5 `cash-book`

| | |
|---|---|
| Goal | Double-entry postings between accounts, a cash book per town by reason and day, and the Accountant's day budget as approvals; `check.rs` reconciles from postings. |
| Java | `money/CashBook.java` `note` (43), `totals` (94), `KEPT_DAYS`; `money/Treasury.java` `withdraw` (38), `available`, `deposit`; `money/Accountant.java` `free` (28), `today` (60), `left` (40), `leftFor` (50), `approve` (76), `allow` (90); `territory/Territory.java` `reserve` (1768), `payroll` |
| Covers | `CountingHouseTests.theCashBookTalliesMoneyByWhatItWasFor`, `CountingHouseTests.theAccountantKeepsTwoSpendersOffTheSameMoney` |
| Rust | adds `cash_book.rs` (`Account` enum: `Treasury(TownId)`, `Households(TownId)`, `Wagon(RouteId)`, `Outside(Reason)`; `post(...)`; `CashBook` resource keyed by town, reason, day; `Accountant::approve/allow` built on the existing `Budget`); migrates existing transfers in `rules/touch.rs`, `rules/arrive.rs`, `rules/plan.rs`, `rules/contracts.rs`, `rules/bills.rs`, `people.rs` to `post`; touches `state.rs` (Ledger gains nothing; `Budget` gains `approved`), `society.rs` (`cash_book(town, days)`), `lib.rs` |
| Math | **Double-entry bookkeeping (REA ledger)** — tick it in `docs/core-math.md` |
| Accept | all existing tests pass; `conserved()` is now computed from postings against `Outside` and agrees with the old ledger on every random world; a test posts 4 under "wages" and 2 under "lockers: sold wheat" and reads them back; the Accountant test with the Java numbers (free 40, share 0.5 → 20; approve 12 yes, 12 no; allow(20) = 8; a bill of 30.6 due within budget_days leaves free < 10) |
| Model | **opus / medium** — it rewrites every money line in the core and defines the account model everyone else posts to; getting it wrong costs every later task. |

Brief:
```
# cash-book
Adopt double-entry bookkeeping (docs/core-math.md). Add sim/core/src/cash_book.rs: `enum Account { Treasury(TownId),
Households(TownId), Wagon(RouteId), Outside }`, `enum Reason` (or `&'static str` labels: "wages", "road food", "sale",
"contract", "bill", "home", "child", "comforts"...), and `pub fn post(ledger: &mut Ledger, book: &mut CashBook, from:
Account, to: Account, amount: f64, reason: Reason, now: u64)` that moves the money (callers pass the two balances by
&mut, or the function takes a small `Accounts` view) and records both sides. `CashBook` is a resource: per town, a
BTreeMap<(day, reason), f64> kept KEPT_DAYS (60); `Society::cash_book(town, days) -> Vec<(day, reason, amount)>`. Keep
`Ledger.income/road/spent` as the Outside account's running totals so check.rs's conserved() keeps working, then make
conserved() read them from postings. Migrate every `money.0 +=`/`-=` and `purse` move in rules/*.rs and people.rs to
post(); the trade-loop tests must pass unchanged. Add the Accountant on top of `Budget`: `approve(town, amount) -> bool`
(takes the amount off today's budget), `allow(town, amount) -> f64` (what's left of it), with free money as in
rules/mod.rs::free (money less trading capital less bills due within budget_days). Tests in tests/cash_book.rs with the
Java numbers from CountingHouseTests (plan 1.5). Tag both. Tick the REA row in docs/core-math.md.
```

#### 1.6 `roster`

| | |
|---|---|
| Goal | Trades as content and a town's people as a roster of hands with trade, level, xp, baby and away; levelling as the mod does it (villagers at once, the Merchant at player thresholds). |
| Java | `caravan/Squad.java` `LEADS`, `TRADES` (the lists; not muster); `territory/Labour.java` `NONE`, `NITWIT`, `hands` (116); `locker/LockerTrading.java` `rewardTradeXp` (402), `rewardMerchantXp` (412), `merchantXpForNextLevel` (423), `MERCHANT_LEVEL_XP`; `merchant/Merchant.java` `lockerLimit` (80); `settlement/Census.java` `value` (24) for what a resident is; `territory/Growth.java` `bear` (219), `growUp` (252) for babies; `docs/decisions.md` "Merchant" (XP mirrors a player's), "Settlements", "The census keeps a roster" |
| Covers | `LockerTests.lockerLimitGrowsWithLevel`, `LockerTradingTests.merchantLevelsMatchPlayerLevels`, `LockerTradingTests.aVillagerLevelsUpAtOnceFromLockerTrades` *(partial: the level rule; the trade itself is `lockers`)*, `GrowthTests.childrenGrowUpWhileAway` *(partial: a baby is grown after 24,000 ticks of elapsed time, loaded or not)* |
| Rust | adds `roster.rs` (`Trade` content, `Hand`, `Roster` component, `TradeId`, levelling fns, `Roster::count(trade)`, `levels(trade)`, `idle()`, `babies()`, `best(trade)`); touches `content.rs` (`Content.trades` with the 16 names and `leads`/`squad` flags, `Content::trade(name)`), `state.rs` (`Roster`), `society.rs` (`set_roster(town, &[(trade, level)])`, `add_hand`, `roster(town)`), `people.rs` (births add a baby hand when a roster exists; `villagers` derived), `lib.rs` |
| Math | — |
| Accept | random worlds give every town a roster (sized to its villagers) and still conserve; existing people tests pass with villagers derived; the Merchant's XP thresholds 315/1395/4020/6895 and 2·level lockers; a villager at 10 xp is level 2 at once; a baby grows up by elapsed ticks |
| Model | **opus / high** — the people model every work slice builds on (jobs, squads, services, levies, training); reconciling it with the existing `People.villagers` and births needs judgment. |

Brief:
```
# roster
Adopt decision 1 of docs/core-plan.md section 2. Add sim/core/src/roster.rs. Content: `pub struct Trade { name, leads:
bool (Squad.LEADS: merchant, cartographer, librarian, shepherd, leatherworker, fisherman), squad: bool (Squad.TRADES adds
cleric, farmer, toolsmith, armorer, weaponsmith, mason, fletcher, butcher), none/nitwit flags }`, `Content.trades` with
the sixteen, `Content::trade(name) -> Option<TradeId>`. State: `pub struct Hand { trade: TradeId, level: u8 (1..=5), xp:
u32, baby_until: Option<u64>, away: bool }`, `#[derive(Component)] pub struct Roster { hands: Vec<Hand>, merchant:
Option<usize> (the desk's Merchant: a hand that never travels) }`. Levelling: villagers level at vanilla thresholds (10,
70, 150, 250 xp) at once; the Merchant at MERCHANT_LEVEL_XP {315, 1395, 4020, 6895}; `merchant_xp_for_next_level`.
`Roster::locker_limit() = 2 * clamp(merchant level, 1, 5)`. A baby is a hand with baby_until = born + 24000 ticks and
counts as a resident, not a worker. Make `People.villagers` follow the roster: a system in Touch (ordered after
catch_up) sets villagers = hands.len() when a Roster exists; people.rs births push a baby hand (trade none) instead of
adding 1.0 when a Roster exists. Host calls: `set_roster(town, &[(&str, u8)])`, `add_hand(town, trade, level)`,
`roster(town) -> Roster`. Random worlds: give each town a roster of its villagers with random squad trades. Tests in
tests/roster.rs with the parity tags listed in plan 1.6. Every struct derives serde (save-load is landing alongside; if
it hasn't, add the derives anyway behind `serde` in Cargo.toml).
```

#### 1.7 `hex-map`

| | |
|---|---|
| Goal | Hexes, terrain costs and A* paths; towns placed on hexes; routes whose length and cost come from their path. |
| Java | `hex/Hex.java` all (91 lines: `at`, `round`, `centerX/Z`, `distance`, `DIRECTIONS`, `neighbors`, `line`); `caravan/RoutePlanner.java` `plan` (53), `postToll` (100), `MARGIN`; `territory/Terrain.java` `crossingFactor` (152), `waterShare` (136), `relief` (112), `record` (127), `recordHeights` (98), `IMPASSABLE`, `RELIEF_PER_FACTOR`, `CLIFF`; `caravan/Caravans.java` `perHex` (2048), `terrain` (2057), `plan(..., boat)` (2132), `BOAT_FACTOR`, `foodNeeded` (130), `travelTicks` (138); `territory/Works.java` `bridged` (216) for what a bridged hex means; `docs/decisions.md` "Hex grid", "Water makes chokepoints", "Mountains", "Route distance" |
| Covers | `HexTests.hexCentresMapBackToTheirHex`, `.hexesAreEightyBlocksAcross`, `.lineRunsHexByHex`; `TollTests.withNoTollsThePathIsStraight`, `.waterCantBeCrossed`, `.steepGroundIsHardGoingAndACliffStopsACaravan`, `.aRouteFindsThePassThroughAMountainRange`, `.aPriceyTownIsGoneAround`, `.aCheapTollIsPaid`, `.aFloodedHexIsImpassable` *(partial: the share > 0.5 rule; the biome guess is Minecraft's)*, `.aBridgedHexIsOpenGround` *(partial: the flag)*, `.aFishermanSailsWhereCaravansCantWalk` *(partial: by boat an impassable water hex costs 0.8; who leads is `squads`)* |
| Rust | adds `hex.rs` (axial coords, 80 across, line, distance), `map.rs` (`Map` resource: `BTreeMap<Hex, Ground { water: f64, relief: f64, bridged: bool }>`, `Posts` input as `BTreeMap<Hex, (TownId, toll_rate, fee)>`, `plan(from, to, per_hex, value, free_towns, boat, avoid) -> Option<Path { hexes, cost }>`); touches `state.rs` (`Town.hex: Option<Hex>`, `Route.path: Option<Vec<Hex>>`), `society.rs` (`place_town(town, x, z)`, `set_ground(hex, water, relief)`, `bridge(hex)`, `add_road(from, to, every_days)` deriving `travel_ticks` from path length × 80 blocks at 1,200 blocks/day and `road_cost` from the path's cost), `lib.rs` |
| Math | groundwork for **spatial price equilibrium** (distance is now a cost) and **contraction hierarchies** (not now) |
| Accept | the HexTests numbers (east neighbour 80 away, line of distance+1 hexes); the TollTests fixture (start (0,0), goal (12,0), river at q=6 with a bridge, PER_HEX 8.5, value 1000: 0.25 toll → around, 0.01 → straight, relief 16 → factor 3, 40 → impassable, boat 0.8); `add_road` between placed towns gives a route the trade loop runs; random worlds place towns and use roads on random ground and still conserve |
| Model | **opus / medium** — A* with an inadmissible boat heuristic, terrain as data, and the Route change others build on (chokepoints, tolls, hubs, freight). |

Brief:
```
# hex-map
Adopt decision 4 of docs/core-plan.md section 2. Port hex/Hex.java to sim/core/src/hex.rs exactly (pointy-top axial,
ACROSS 80, SIZE 80/sqrt(3), the DIRECTIONS order east first, cube rounding, `line` with the 1e-6 nudge, distance).
Add sim/core/src/map.rs: a `Map` resource of `Ground { water_share, relief, bridged }` per hex (unknown hexes are dry
and flat), `crossing_factor` = (1 + 2·water)·(1 + relief/8), infinite at water > 0.5 or relief >= 32, 1 for a bridged
hex that's mostly water, 0.8 by boat for an impassable water hex; A* over hexes within RoutePlanner's corridor (MARGIN
6) with cost per step = per_hex·factor(next) + toll(next) where toll comes from a `Posts` argument (hex → owner, rate,
fee) charging rate·value + fee once per owner, never for towns in `free`; start and goal always factor 1. Note the boat
factor makes the distance heuristic inadmissible: use a consistent heuristic (scale it by 0.8 when boat is allowed) and
say so. State: `Town.hex: Option<Hex>`, `Route.path: Option<Vec<Hex>>`. Society: `place_town(town, x, z)`,
`set_ground(hex, water, relief)`, `bridge(hex)`, `add_road(from, to, every_days) -> RouteId` with travel_ticks from the
path's hex count × 80 blocks at 1,200 blocks a day (TICKS_PER_BLOCK 20) and road_cost from the path's cost at per_hex =
2·(80/10)·bread's value (Caravans.perHex). Keep add_route as is. Tests in tests/hex_map.rs, built on TollTests' fixture
with the plan's numbers; tag as listed in plan 1.7. Random worlds: place towns on a 12×12 hex patch with random
water/relief and connect some by add_road.
```

#### 1.8 `bargain-and-par`

| | |
|---|---|
| Goal | Prices as a content table, par from locker capacity, and the mod's bargain between two Merchants by level; wagons sell at the bargained price. |
| Java | `money/Prices.java` (the table and `emeraldsEach` 145: only to seed `Good.value`s for the host's content); `money/Bargain.java` all (`SELLER_FLOOR`, `sellerFloor`, `buyerCeiling`, `shortage`, `between` 29, `price`); `merchant/Warehouse.java` `par` (138), `capacity` (113), `recordCapacity` (78), `MIN_PAR`, `PAR_DAYS`; `caravan/Caravans.java` `sell` (2431: the price at mid-batch, seller `merchantLevel`, buyer `residentLevel`), `buyForHome` (1889: ceiling full value), `buySurplus` (2778: `RESALE_CEILING` 0.75); `docs/decisions.md` "Merchants bargain by level" |
| Covers | `BargainTests.noDealAtPar`, `.shorterShelvesPayMore`, `.theHigherLevelMerchantTakesMoreOfTheGap`, `.parIsShelfDaysOfWhatTheLockersCanTake` |
| Rust | adds `bargain.rs` (`seller_floor`, `buyer_ceiling`, `shortage`, `between`, `price`; `par(content, capacity_per_day, wanted)`); changes `rules/arrive.rs::sell` to price each good by `bargain::price(value, shortage at mid-batch, seller level, buyer level)` with levels from the rosters' Merchants (1 when absent) and skip the good when no deal; `rules/plan.rs::plan_sale` values the load at the expected bargained price; touches `content.rs` (dial `bargain: bool` default true, so a test can pin the old flat price), `state.rs` (`Trades.capacity: Vec<f64>`, what lockers can take a day, default = `uses`), `lib.rs` |
| Math | the bargain is the mod's; note in `docs/core-math.md` (tâtonnement row) that a per-town price now exists at the point of sale |
| Accept | the four BargainTests with their numbers (equal levels 0.75; seller 5 vs 1 → 0.5 + 0.5·5/6; par 64 wanted with no capacity, 0 unwanted, 5×768 for two lockers of 384); the trade-loop tests still pass (adjust expectations only where a bargained price changes a measured ratio, and say so in the commit) |
| Model | **sonnet / high** — pure functions with given numbers, but it edits `sell` and `plan_sale`, the core's busiest rules; needs care with the existing tests. |

Brief:
```
# bargain-and-par
Port money/Bargain.java to sim/core/src/bargain.rs: seller_floor(v) = 0.5v; buyer_ceiling(v, shortage) = v·(0.5 +
0.5·clamp(shortage, 0, 1)); shortage(on_hand, par) = max(0, 1 − on_hand/par) (0 if par <= 0); between(floor, ceiling,
seller_level, buyer_level) = None if ceiling <= floor else floor + (ceiling − floor)·s/(s+b) with levels clamped to at
least 1; price(value, shortage, s, b). Port Warehouse.par: par(capacity_per_day, wanted) = wanted ? max(64, ceil(capacity
· shelf_days)) : ceil(capacity·shelf_days); add `Trades.capacity` (what a town's lockers can take of a good a day;
set_trade sets it to uses) and make rules/mod.rs::shelf use it when > 0. Change rules/arrive.rs::sell: for each good the
unit price is bargain::price(value, shortage(stock, shelf) at mid-batch, seller's Merchant level, buyer's Merchant
level) (levels from Roster if the component exists, else 1); None means the good doesn't sell here (log nothing). Behind
dial `bargain` (default true) so a test can keep the flat price. Value a plan's load in plan_sale at the same expected
price. Tests in tests/bargain.rs: the four BargainTests with their numbers, tagged. Re-run the trade-loop and contracts
tests; where a measured ratio shifts because sales now land between 0.5 and 1.0 of worth, adjust the expectation and
explain in the commit message with before/after numbers.
```

#### 1.9 `road-food`

| | |
|---|---|
| Goal | Food as the road's cost: a wagon eats food points going out (from home) and coming back (bought at the far end), carries a purse for them, takes provisions when the far end can't feed it, waits then goes home on short rations, and no food and no money means no wagon. |
| Java | `caravan/Caravans.java` `foodNeeded` (130), `inRange` (134), `travelTicks` (138), `dispatch` 296–377 (crew against food, `fedThere`/`provisions` 313–324, `roadPurse`, the funding check 364–377, `fromHome`/`foodCoin` and the withdrawals 514–521), `fodderOnHand`/`eatFodder` (1334–1350), `spareFoodAt`/`foodKnownAt` (2584–2602), `arriveAtDestination` food tail 730–780 (`stock.eat`, `waitingSince`, `MAX_WAIT_TICKS`, half pace), `arriveHome` 1147 (deposit), `CaravanLedger.foodPromisedAt` (134); `merchant/MerchantStock.java` `eat` (274), `foodNutrition` (255), `nutrition` (0 for comforts); `docs/decisions.md` "Food limits travel", "Caravans don't sit for days at a hungry stop", "A poor town packs its own road food" |
| Covers | `CaravanTests.caravanSellsItsCargoAndBuysItsFood`, `.aHubThatCantFundThePurseStaysHome`, `.noFoodNoMoneyNoCaravan`, `.withoutBreadAWagonBuysItsRoadFood`, `.foodCapsALegAt3200Blocks`, `.aCaravanGoesHomeOnShortRationsRatherThanWaitForDays`, `MoodTests.treatsAreNotRoadFood`; upgrades `CaravanTests.aTownsWagonWaitsForALoadWorthItsRoadFood` from partial to full |
| Rust | adds `rules/road.rs` (provisioning in `Decide` before the departure check, eating at `Turn`, waiting state); changes `rules/plan.rs::decide` (the gate: worth ≥ road's food cost both ways; the purse; no food and no money → stay) and `rules/arrive.rs::turn` (buy food at the far end at local value into the far end's treasury, or wait `wagon_wait_days`/2, then half pace); touches `state.rs` (`Wagon.provisions`, `Wagon.waiting_since`, `Route.blocks`), `content.rs` (dials `food_capacity 320`, `blocks_per_food 10`, `purse_per_food_point 0.05`, `max_wait_ticks 12000`), `society.rs` (`add_route` gains a blocks figure; routes from `add_road` have it) |
| Math | — (the departure rule stays the expected-profit test) |
| Accept | with bread as a food good, the farm-and-mill fixture eats and buys bread by the Java numbers (1 point per 10 blocks; a 4-block leg = 1 point; a bread = 5 points); 3,200 blocks in range, 3,210 not; a far end with no food: the wagon waits half a day then goes home at half pace with `short_rations` counted; conservation holds with food leaving through `Ledger.used` and money moving town-to-town; worlds with no food good behave exactly as before (bit-equal outputs on the existing tests) |
| Model | **opus / medium** — it changes the meaning of `road_cost` in the core's central rule (`decide`), with a fallback to keep every existing test, and the mod's food logic has quirks (`foodKnownAt` treats an unknown stocktake as plenty). |

Brief:
```
# road-food
Adopt decision 6 of docs/core-plan.md section 2. Read the Java named in plan 1.9. In the core, food goods are those
with `Good.food > 0` (points each); comforts are never food (treatsAreNotRoadFood). A route gains `blocks` (from its
path, or add_route's days × 1,200); food needed a leg = max(1, ceil(blocks/10)) per mouth (1 mouth until squads land;
read `Wagon.crew` if present), capped by food_capacity 320 (a leg over that is out of range: add_route/add_road return
Err). Departure (rules/plan.rs::decide): the wagon eats the outbound leg's food from home's warehouse (whole items,
heartiest-last is not needed: cheapest food first) and carries a purse of food × purse_per_food_point for the return
leg; if home has no food it buys the points' worth in money instead (foodCoin); if home can't pay purse + food it stays
("can't fund"), and no food and no money means no wagon. If the far end's last known food (its stock, less what wagons
already bound there will eat) is short, take provisions for both legs from home. The gate "worth its road food" compares
the load's worth to both legs' food at value. At the far end (rules/arrive.rs::turn, Leg::Out): eat the return leg's
food from the far end's warehouse, paying its treasury at local value from the purse (counted as the far end's income
in the ledger/cash book); no food: wait, and after max_wait_ticks/2 go home on short rations at half pace, counting
`short_rations` on the RouteState. Keep money `road_cost` as the fallback when content has no food good, so every
existing test is bit-identical. Tests in tests/road_food.rs with the Java numbers (plan 1.9), tagged; upgrade the
trade_loop.rs partial tag to full. Random worlds: a third of worlds get a bread good and food trades.
```

#### 1.10 `advances-and-refunds`

| | |
|---|---|
| Goal | The advance term: a share of a contract's worth paid up front, carried by the buyer's wagon with the copy, worked off at the same share of each delivery, owed back when the contract breaks. |
| Java | `contract/Contract.java` `worth` (151), `advanceDue` (157), `recoupFrom` (168); `contract/ContractBook.java` `advanced` (368), `recouped` (375), `refunds` (400), `broken` (516), `owe` (406), `paidOwing` (415), `owing` (423); `contract/Advance.java`; `caravan/Caravans.java` `collectPressing` (1273: `advancesDue` makes a trip pressing), `exWorksPurse` (1798), `payAdvances` (1618), `settleContracts` 1409 (the `recoupFrom` deduction), `settleOwing` (1568); `docs/decisions.md` "Money before the goods: advances", "Work to make comes in small installments" |
| Covers | `ContractTests.anAdvanceIsPaidUpFrontAndWorkedOff`, `ContractTests.aBrokenContractOwesBackItsUnearnedAdvance` (the refund half; `save-load` has the other), `CaravanTests.theAdvanceRidesWithTheCopyAndTheInputsFollow` *(partial: the advance rides and is paid; inputs follow in `sourcing-and-procurement`)* |
| Rust | adds `rules/advances.rs` (`PlanPapers`: the buyer's wagon carries the advance due with the copy; `Papers`: pays it, the rest by bill where trusted; `Settle`: recoups; `Judge`: on break, the unearned part becomes `owing` back; the next wagon between the two collects what the seller can spare); touches `contracts.rs` (`Contract.advance_share`, `advanced`, `recouped`, `refund`; `Book::owe/owing/paid_owing`), `society.rs` (`draft` gains an `advance` share; `owing(a, b)`), `content.rs` (dial `advance_share` 1/3) |
| Math | **principal-agent / collateral contracts** (the advance as the first collateral; tick as "in use: advances") |
| Accept | worth 12 with a 1/3 advance → 4 due, 0 once paid, each of 3 deliveries recoups 1 and the last takes the rest; a DAP contract broken undelivered has refund 4 listed in `refunds(seller, buyer)` and the next wagon collects it; random worlds draft some contracts with advances; conservation holds (the advance is a town-to-town transfer by wagon) |
| Model | **sonnet / medium** — a self-contained extension of `contracts.rs` with exact Java numbers and clear hooks (the PlanPapers/Papers/Settle steps exist). |

Brief:
```
# advances-and-refunds
Read the Java in plan 1.10 and sim/core/src/contracts.rs, rules/contracts.rs, rules/bills.rs. Add an advance to
contracts: `Contract { advance_share: f64, advanced: f64, recouped: f64, refund: f64 }`, `worth() = quantity ·
installments · price`, `advance_due() = worth·share − advanced`, `recoup_from(payment) = min(payment·share, advanced −
recouped)` with the last installment taking whatever is left. Book::draft takes the share (0 for none; dial
advance_share default 1/3 for drafts that ask for one). Systems in sim/core/src/rules/advances.rs: PlanPapers — a
buyer's wagon to the seller carries the advance due in its purse (plan.purse) and the trip is pressing while an advance
is due; Papers — at the seller's it pays the advance from the purse (post to the seller's treasury), the remainder as a
bill on the buyer where trust >= MIN_TRUST; Settle — deliveries are paid less recoup_from; Judge — when a contract
breaks, advanced − recouped becomes `refund`, listed by Book::refunds(seller, buyer) and owed in Book::owing; Collect —
the next wagon between the two collects what the seller can spare beyond its trading capital. Tests in
tests/advances.rs with the Java numbers (plan 1.10), tagged (the CaravanTests one partial). Random worlds: a third of
drafted contracts carry an advance. Tick the collateral row in docs/core-math.md as "in use: advances".
```

#### 1.11 `mint-rate`

| | |
|---|---|
| Goal | The mint: a spur/emerald rate that falls with selling and recovers, a batch priced along the curve, a 5% spread on buying coin, and debasement from minting. |
| Java | `money/Mint.java` all 130 lines (`pressure` 48, `debasement` 73, `rate` 85, `quote` 90, `sell` 95, `buyCost` 107, `buy` 115, `spursFor` 123, `mint` 78 and the constants) |
| Covers | `MoneyTests.rateFallsWithSellingAndRecovers`, `MoneyTests.theMerchantChangesCoinForEmeraldsToPayItsVillagers` *(partial: the spread and the funds limit; coins and the bank are Numismatics)*, `WarTests.mobilisingTheSovereignMintsAndBuysGoldThroughAShop` *(partial: minting debases; the rest is `crown`)* |
| Rust | adds `mint.rs` (`Mint` resource: `pressure`, `pressure_at`, `minted`, `minted_at`; `rate(now)`, `quote(n, now)`, `sell(n, now)`, `buy_cost(n, now)`, `buy`, `mint(spurs, now)`); touches `society.rs` (`mint() -> &Mint`, `mint_rate(now)`), `lib.rs` |
| Math | — (an exponential decay computed on touch, as the core already does) |
| Accept | rate 16 at rest; selling 1,728 pays 19,164 (= floor(16·1728·ln 2)) and leaves the rate 8; 3 days later 16/1.5; `buy_cost(2) > 2·rate`; minting 27,648 doubles the rate, halving back over 30 days; `Mint` is in the save and in the random worlds (a few towns sell emeralds daily) |
| Model | **haiku / medium** — one small file of closed-form functions with every constant and expected value given. |

Brief:
```
# mint-rate
Port money/Mint.java to sim/core/src/mint.rs as a resource `Mint { pressure: f64, pressure_at: u64, minted: f64,
minted_at: u64 }` (derive serde, Default). Constants: BASE_SPURS_PER_EMERALD 16, PRESSURE_SCALE 1728, HALF_LIFE 3 days,
DEBASEMENT_SCALE 27648, DEBASEMENT_HALF_LIFE 30 days, BUY_SPREAD 0.05 (days in ticks via Content.day_ticks). pressure(now)
decays by 0.5^(elapsed/half_life) when touched; debasement(now) = 1 + minted(now)/27648 with its own half-life; rate =
16·debasement/(1 + pressure/1728); spurs_for(n) = floor(16·1728·ln((1728+p+n)/(1728+p))); quote(n) = that × debasement,
changing nothing; sell(n) pays it and adds n to pressure; buy_cost(n) = ceil(n·rate·1.05); buy(n) takes n off pressure,
not below −864; mint(spurs) adds to minted. Insert the resource in Society::new; add Society::mint_rate(). Tests in
tests/mint.rs with the numbers in plan 1.11, tagged as listed. Random worlds: give a few towns a daily emerald sale
through the mint so the resource is exercised and saved.
```

---

### Wave 2: the town's people at work (on `roster`, `hex-map`, `cash-book`, `bargain-and-par`)

#### 2.1 `lockers`

| | |
|---|---|
| Goal | A town's income as its lockers' trades: each managed hand's switched-on offers run at restock (twice a day), within the demand cap, at the Merchant's discount, levelling villager and Merchant; the Merchant manages `2·level` lockers, contract-needed and switched-on ones first; missed days replay on touch. |
| Java | `locker/LockerTrading.java` `trade` (138–235: order, inflation filter, sort by price, budget, supply, the `tradeOnce` loop), `tradeOnce` (362), `price` (352), `merchantReputation` (358), `isInflated` (347), `demandIfUnused` (60), `floorCooling` (70), `rewardTradeXp` (402), `rewardMerchantXp` (412), `supply` (318), `collect` (332); `locker/LockerCatchUp.java` `catchUp` (131), `missedRestocks` (170), `settledUntil` (149), `startClock` (163); `locker/Lockers.java` `near` (65–89: the comparator), `worth` (181); `market/Apprentices.java` `neededTrades` (51); `docs/decisions.md` "Lockers", "Merchant" |
| Covers | `LockerTradingTests.enabledTradeRunsAtRestock`, `.switchedOffTradeDoesNotRun`, `.inflatedPriceIsSkipped`, `.masterMerchantGetsADiscount`, `.lockerRestocksDoNotSinkDemand`, `.unloadedDaysAreReplayedOnLoad`, `.neverRestockedOwesNothing`, `.noMerchantNoTrading` *(partial: no Merchant hand, no income from lockers)*, `.merchantFillsLockerFromShulkerAndCollects` *(partial: supplied from the warehouse, results collected)*, `.aVillagerLevelsUpAtOnceFromLockerTrades` (completing roster's partial); `LockerTests.limitPrefersLockersWithTradesSwitchedOn`, `.limitKeepsNearestLockers` *(partial: the limit and order; distance as a tie-break on hand index)*; `LogisticsTests.aMerchantShortOfLockersTendsTheOnesItsContractsNeedFirst`; `JobsTests.aMerchantValuesALockerByWhatItsTradesWouldDoForTheTown` |
| Rust | adds `lockers.rs` (`Offer { good, count, emeralds, uses, demand, multiplier, on: bool }` per hand; `Lockers` component: which hands are managed, `restocked_until`; `worth`), `rules/lockers.rs` (in `Work`, after roster's count: replay restocks since last touch, 2 a day, at most 60 days; the Merchant's span and order); touches `content.rs` (`Trade.offers` defaults for the mod's trades from `Prices`: e.g. librarian 24 paper → 1 emerald, 16 uses, mult 0.05), `state.rs` (`Lockers`), `society.rs` (`switch(town, hand, offer, on)`, `lockers(town)`), `lib.rs` |
| Math | **order-up-to / bullwhip**: note the restock cap as the per-node order policy; add a test that two restocks a day don't amplify a daily swing upstream |
| Accept | the Java fixture (24 paper → 1 emerald, 16 uses, 2 xp, mult 0.05): 64 paper → 2 trades, 16 left, 2 emeralds; 128 → 5 trades, villager level 2; a demand of 100 skips; level-5 Merchant → price 19, 3 trades, 7 left; 3 days and 100 ticks → 6 restocks and the clock exactly 3 days on, alternating trade/skip to 48 emeralds; `Lockers::worth` with the Java numbers (carrots 5 → 5, 100 → 16, bread wanted → 8); income flows through the cash book as "lockers: sold <good>" |
| Model | **sonnet / high** — a 400-line Java rule with vanilla demand quirks, but every step and every expected number is written down; the trap is replacing `Trades.uses` cleanly. |

Brief:
```
# lockers
Adopt decision 2 of docs/core-plan.md section 2. Read the Java in plan 2.1 and sim/core/src/roster.rs. A town with a
Roster gets a `Lockers` component: for each hand with a trade, its offers (from Content.trades[t].offers, each `Offer {
good, count, emeralds, uses: u8, demand: i32, multiplier: f64, on: bool }`; every offer starts off), and which hands the
Merchant manages. A restock: for each managed hand, demand cools as vanilla does but never below 0 (floorCooling); each
switched-on offer whose price isn't inflated (floor(count·demand·mult) > 0 means skip) runs up to `uses` times while
the warehouse holds `count` of the good, at price = clamp(count − floor(rep·mult), 1, 64) where rep = {0, 30, 55, 80,
110}[merchant level − 1]; each run moves the goods out (Ledger.used, as today's catch_up does) and emeralds in (post
"lockers: sold <good>"), gives the hand 2 xp (level at once) and the Merchant 3 + rng(4) xp (+5 on a level-up); then
demand += uses_run − uses (vanilla's updateDemand). Restocks happen twice a day: on touch, replay the whole restocks
since restocked_until (at most 60 days), moving restocked_until by whole days; a town never restocked starts its clock
at now and owes nothing. The Merchant manages locker_limit() hands, ordered: trades a contract needs first (hook: a
`needed: Vec<TradeId>` on Lockers that `training-and-tenders` will fill), then current clients, then worth descending,
then switched-on first, then hand index. `Lockers::worth(hand, supply) = Σ min(uses·emeralds, supply in emeralds) over
buying offers + Σ 0.5·uses·emeralds over selling offers the town wants`. When a town has Lockers, its income comes from
them, not from Trades.uses: set Trades.uses[g] = 0 for goods its offers buy, and Trades.capacity[g] = the day's
capacity (count·uses·2 summed) so par follows. Tests in tests/lockers.rs with every number in plan 2.1, tagged as
listed. Random worlds: towns with rosters get offers and some switched on; conservation must hold. Add a bullwhip test
(a daily swing in supply doesn't grow through two restocks) and note it in docs/core-math.md.
```

#### 2.2 `squads`

| | |
|---|---|
| Goal | Who crews a wagon: eligibility, the lead, extras only where they help, hiring a hand as a travelling Merchant, training a squad, porters and packs, saddles, the cap of four; crew leave the roster as `away` and come back. |
| Java | `caravan/Squad.java` `eligible` (130), `available` (208), `order`, `muster` (148), `leads`, `hire` (229), `carries`, `MAX`, `PACK_STACKS`, `LLAMA_STACKS`, `SIGNING_EMERALDS`; `caravan/Commissions.java` `train` (99), `squadShort` (81), `TRAINING_EMERALDS`; `caravan/Caravans.java` `dispatch` 223–303 (extras, wanted trades, porters, the food trim, porter-run) and 380 (slots), 528–537 (saddle pace), `sendPortersHome` (1136), `porterRun` (1299); `caravan/CaravanLedger.java` `walkHome` (42), `walkersHome`; `docs/decisions.md` "Caravans are staffed by squads", "Towns train their squads", "A squad is never more than 4", "Crew" |
| Covers | `CaravanTests.noSquadNoCaravan`, `.aTownShortOfCrewTakesOnAHand`, `.aTownWithARouteTrainsItsSquad`, `.extraMembersGoOnlyWhereTheyHelp`, `.aClericGoesAlongButNeverLeads`, `.aSquadIsNeverMoreThanFour`, `.babiesAndTheDesksMerchantStayHome`, `.aLeatherworkersPacksCarryMore`, `.aPorterCarriesOutAndWalksHome`, `.aSaddledCaravanRidesFaster`, `.theSquadLeavesTheWorldAndComesBack` *(partial: who goes and the food they eat; the entities are Minecraft's)* |
| Rust | adds `squad.rs` (`Needs`, `muster`, `hire`, `train`, capacity), `rules/squad.rs` (`Plan`: muster for each idle route and record the crew and slots on the plan; `Decide`: mark hands away, eat crew food (with `road-food`); `Turn` home: release; porters walk home after one leg; `Review`: train one lead a count); touches `state.rs` (`Wagon.crew: Vec<usize>`, `Wagon.porters`, `Plan.slots`, `RouteState.saddle_wear`), `content.rs` (dials `signing_emeralds 2`, `training_emeralds 4`, `pack_stacks 9`, `saddled_pace 4/3`, `saddle_wear 0.1`), `society.rs` (`crew(route)`) |
| Math | — |
| Accept | the Java numbers: Cartographer + Shepherd with no needs → 1; with animals and 3 extras → 2; extras 0 → 1; a lone Cleric → none; six candidates → 4; a baby or the desk's Merchant never goes; a Librarian + Leatherworker with 32 stacks wanted → crew 2, slots 36; Librarian + Nitwit → porter, crew 1 after delivery and the hand back a leg later; a saddle in stock → 60 ticks for an 80-tick leg; a town with routes and a spare idle hand trains a Merchant for 4 emeralds; wagons on routes whose town has no roster behave as before |
| Model | **sonnet / high** — pure selection rules over a list, well pinned, but it threads a crew through `Plan`/`Decide`/`Turn` alongside `road-food`'s changes; the agent must read `road-food`'s landed code first. |

Brief:
```
# squads
Read the Java in plan 2.2, then sim/core/src/roster.rs and rules/plan.rs, rules/arrive.rs, rules/road.rs as landed.
Port Squad.java to sim/core/src/squad.rs over the roster: eligible = adult, trade in Content squad trades or none/nitwit,
not the desk's Merchant, not working a managed locker, not away; available = eligible sorted by (trade's index in the
TRADES order, level desc, hand index); `Needs { librarian, cartographer, shepherd, water, wanted: Vec<TradeId> }`;
muster(available, needs, extras, porters): the lead is a Fisherman if by water else the first hand whose trade `leads`
(none → empty squad); then each wanted trade in TRADES order while size <= extras and < 4 and not already aboard; then
idle hands as porters (nitwits first) up to min(porters, min(4, extras+1) − size). hire: when leads at home + leads out <
routes, the first idle non-nitwit hand becomes a Merchant level 1 for signing_emeralds (post "signing"). train (Review,
once a day): a town with routes trains one idle adult in the first LEADS trade its squad lacks, up to min(4, routes+1)
leads, for training_emeralds. Capacity = 27 + 9 per carrier (Leatherworker, or an idle porter); porters needed =
ceil((wanted stacks − 27 − packs)/9). In Plan: muster for each idle route (extras = hands at home − other idle routes at
this town, so every route keeps a lead), record crew and slots on the Plan; a route with no lead gets no wagon, with
reason "no squad". In Decide: crew go away, each a mouth for road-food. At the far end: porters leave once the cargo
fits without them and walk home, arriving a leg later (a `Walkers` list on RouteState). At home: crew return. A saddle
good in stock makes pace 4/3 (travel/pace) and wears 0.1 a trip. Tests in tests/squad.rs with every number in plan
2.2, tagged. Random worlds: towns with rosters crew their routes; towns without keep the one-mouth wagon.
```

#### 2.3 `farms-and-wages`

| | |
|---|---|
| Goal | Farms as plots worked by Farmers (32 a level, half an item a plot a day, scaled by mood), fields chosen by demand and focus, wages as a share that follows the labour market, credit then sharecropping, stores that bank the surplus, and the households' savings on call. |
| Java | `territory/Territory.java` `plan` (1020), `usesOf` (1087), `plots` (989), `accrue` (1147–1163), `wagePerDay` (265), `valuePerDay` (271), `payWages` (1263), `touch` 1368–1411 (delivery, share-cropping), `bank` (1211), `loose` (1182), `wantsStore` (1191), `storeFor` (1170), `depositsOnCall` (1495), `borrowDeposits` (1510), `settleDeposits` (1530), `yieldFactor` (1729), `cultureFactor` (1734), `recordCropSale` (299); `territory/Households.java` `tightness` (45), `share` (56), `payIn` (65); `territory/Fields.java` `choose` (51–65), `allowed` (97), `wanted` (107); `territory/Focus.java` `of` (18); `docs/decisions.md` "Territory, farms and tolls" (farmers, wages), "Wages stay in the world", "Sharecroppers", "The Merchant tells its Farmers what sells", "Towns grow before they trade" |
| Covers | `TerritoryTests.farmersEarnHalfWhatTheyGrow`, `.paidFarmersDeliverTheWholeHarvest`, `.unpaidFarmersWorkOnCreditThenTakeTheirShare`, `.wagesClimbTowardEverythingTheHarvestSellsFor`, `.wagesEaseToTheFloorWhenNothingSells`, `.aGranaryBanksTheWheatTheWarehouseNeedNotKeep`, `.aRootCellarIsWantedWhereRootsPileUpAndBanksThem`, `.aMerchantBorrowsItsHouseholdsSavingsUpToItsShare`, `.farmersProduceWhatGrowsInTheirHex` *(partial: plots → capacity → yield; the block scan is Minecraft's)*, `.produceAndTollsLandInTheMerchantsChest` *(partial: produce accrues and lands on touch)*; `LabourTests.wagesFollowTheLabourMarketAndOwnersTakeMore`; `JobsTests.fieldsGrowWhatTheVillageWouldBuy`; `MoodTests.contentFarmersGrowMore` |
| Rust | adds `farms.rs` (`Farm` component: plots per crop, turned, fallow, pending produce, wage owed, credit, labour share, sales EMA, freeholds, stores; `Focus`), `rules/farms.rs` (`Work`: accrue produce by plan; `Pay`: wages from the treasury to the households' purse, credit, sharecropping; `Review`: the field choice, rotation, stores wanted); touches `state.rs` (`Farm`), `content.rs` (`Good.crop: bool`, `Good.feed`, dials `labor_share 0.5`, `min/max_labor_share`, `share_half_life_days 2`, `sales_half_life_days 3`, `credit_days 10`, `deposit_share 0.5`, `deposit_interest 0.002`, `deposit_keep 10`, `plots_per_farmer_level 32`, `yield_per_plot_day 0.5`, `field_plots 81`, `field_emeralds 4`), `society.rs` (`set_plots(town, good, n)`, `farm(town)`), `people.rs` (reuse `People.purse` as the households' savings; the deposits borrow from it) |
| Math | **logistic growth with carrying capacity** is already people's; here adopt the **(s, S) reorder** view for stores: a store banks above `loose = max(128, par)` |
| Accept | the Java numbers: wage/day = Σ perDay/itemsPerEmerald × 0.5 with wheat at 20; 10 emeralds pay floor(wage·10) over 10 days and deliver floor(perDay·10); no coin → credit up to 10 × value × 0.5 then wages unpaid ≥ 1 and the chest gets perDay·10·yield·(1 − share) ± 2; sold daily → share > 0.55 after 10 days; nothing sold → 0.25 ≤ share < 0.27; granary + 384 wheat → 128 loose, 256 banked; `wantsStore` false at 64 carrots, true at 384; purse 100 → 50 on call, 30 borrowed then 20; `yieldFactor(3) ≈ 1.3`, `cultureFactor(2) = 1.5`; Focus (5,8,2000)=Grow, (9,9,2000)=Build, (9,12,2000)=Trade; fields: paper 48 → cane, + potato 640 → potato |
| Model | **sonnet / high** — the largest well-specified port in the plan (a dozen tests with numbers), but the arithmetic is given line by line and the shape (accrue on touch, pay on touch) matches the core's. |

Brief:
```
# farms-and-wages
Read the Java in plan 2.3 (Territory.java is 2,000 lines: read only the named methods) and sim/core/src/people.rs,
roster.rs, cash_book.rs. Add sim/core/src/farms.rs: `Farm { plots: BTreeMap<GoodId, f64>, turned, fallow, pending:
Vec<f64>, wage_owed, credit, labor_share (starts 0.5), sales_ema, freehold_share, stores: Vec<Store>, last }`. Capacity =
Σ hands.farmer 32·max(1, level) plots; plan() assigns plots use by use: herds' feed first (hook: a `feed_demand` fn that
`herds` fills), then goods the town uses by price, then the rest by price and most planted; each tended plot yields 0.5 a
day × yield_factor(mood) = clamp(1 + 0.1·mood, 0.5, 1.5); produce accrues in pending and lands in Stock on touch as whole
items (Ledger.made). Wages (Pay): value/day = Σ perDay·value; wage/day = value/day × Households.share(labor_share,
freehold) where share = (1−f)·L + f·max(L, 0.75); pay whole emeralds from the treasury to People.purse (post "wages");
short of money, credit up to 10 days × value × 0.5; beyond that farmers take their wage in kind (kept = 1 − (1 −
paid_share)·share of the crop) and People.paid_share drives mood as today. The labour share drifts toward
clamp(sales/value × tightness, 0.25, 1) closing half the gap every 2 days, sales an EMA with a 3-day half-life of what
lockers sold of crops; tightness = untended ? min(1.5, 1 + untended/capacity) : max(0.5, min(1, tended/capacity)).
Review (daily): Focus from villagers (8), beds and food (2 × 320 points); choose a field (81 plots, 4 emeralds beyond
the reserve) of the crop with most unmet demand by value (cane counting as paper; food-only while growing); rotate a
field's worth of an unwanted crop; a store (granary for wheat, root cellar for carrot/potato/beetroot; 8 emeralds, a
Farmer of level 2) is wanted when the crop piles up to twice loose = max(128, par) and banks the rest. Deposits:
on_call = max(0, min(purse, 0.5·(purse + deposits) − deposits)); borrow moves purse → treasury at 0.2%/day interest,
repaid beyond the reserve and a keep of 10. Also cultureFactor(mood) = clamp(1 + 0.25·mood, 0.25, 2) as a pub fn (used by
territory-growth). Tests in tests/farms.rs with every number in plan 2.3, tagged. Random worlds: towns with Farmers get
plots.
```

#### 2.4 `jobs`

| | |
|---|---|
| Goal | An idle hand draws a trade from a bag of marbles: what its villagers would buy of the town's supply, the comforts the town lacks, idle plots for a Farmer, work wanted for a Mason; stalls cost 6; a slack trade retrains where there are no roads. |
| Java | `territory/Jobs.java` `bag` (95), `buys` (79), `supply` (71), `comfortMarbles` (122), `draw` (154), `employ` (179), `forContracts`/`tradeFor`, `TRADES` (41), `FARMER_IDLE_EMERALDS`, `STALL_EMERALDS`, `SLACK_DAYS`; `docs/decisions.md` "Work for idle hands", "A bag of marbles", "Comforts in the job draw", "A trade with nothing to do gives way" |
| Covers | `JobsTests.anIdleHandDrawsATradeByWhatItWouldMeet`, `JobsTests.aComfortTheTownLacksDrawsTheTradeThatMakesIt` |
| Rust | adds `rules/jobs.rs` (`Review`: one idle adult a day takes a trade when its stall is paid for), `jobs.rs` (`bag`, `draw` with the world's `Rng`); touches `state.rs` (`Rng` resource if `roster` didn't add it: xorshift64* seeded from `Society::new(content, seed)`), `content.rs` (dials `stall_emeralds 6`, `comfort_marbles 0.2`, `slack_days 5`, `farmer_idle_emeralds 0.5`), `society.rs` (`seed`) |
| Math | the draw is a categorical sample; keep it reproducible (Rng in state) |
| Accept | the Java numbers: 3 emeralds of paper → Librarian 3 and Cartographer 3; carrots 1 → Farmer 1; a second Librarian halves to 1.5; 40 idle wheat/day → Farmer 8, 4/day → 0; 8 of building → Mason 8, 0 with a Mason; furnishings 0, clothing 0.5, treats 1 with savings 500 → Shepherd 100, Leatherworker 50, none for a trade present; over 1,000 draws at 3:3:1 the Librarian comes up 330–530 times; the same seed draws the same |
| Model | **sonnet / medium** — a scoring rule with given numbers and a seeded draw; the only design is where the `Rng` lives. |

Brief:
```
# jobs
Read the Java in plan 2.4 and sim/core/src/roster.rs, lockers.rs, farms.rs (as landed; where farms hasn't landed,
take idle plots' value as a field on Farm you read if present). If state.rs has no `Rng` resource, add one
(xorshift64*, as tests/random_worlds.rs does; `Society::new` takes a seed, default 1; `Society::with_seed`). Port Jobs
to sim/core/src/jobs.rs: supply = 4 days of farm output + the warehouse, in emeralds; bag(town) gives each squad/work
trade marbles = Σ over its buying offers of min(uses·emeralds, supply of that good) / (1 + hands of that trade) +
signals (hook: a `Signals` resource other slices fill; empty now) + 4 × idle plots' daily value for a Farmer if >= 0.5 +
building work wanted for a Mason without one (hook) + comfort marbles: for each comfort kind the town lacks (met < 1),
for the trade whose workshop makes it (Content: shepherd→carpet/furnishings, leatherworker→boots/clothing,
librarian→book/reading, farmer→cookie/treats) and that the town hasn't, (1 − met) × savings(People.purse) × 0.2.
draw(bag, rng) picks in proportion. Review (daily): a contract's trade first (hook `needed` on Lockers), else one idle
adult draws when the town can pay stall_emeralds beyond its reserve (post "stall"); in a town with no routes, a hand
whose offers did nothing for slack_days reviews retrains (never farmer, mason, cleric). Tests in tests/jobs.rs with
plan 2.4's numbers, tagged. Random worlds: idle hands take trades.
```

#### 2.5 `territory-growth`

| | |
|---|---|
| Goal | A town claims its hex at its first count and grows by culture out to two rings; a held hex stays held; guards at posts eat and are paid or desert; militia stand watch unpaid; wagons pay tolls to posted land they cross. |
| Java | `territory/Territory.java` `claim` (707), `afterCensus` 727–741 (the culture block), `grow(GlobalPos, Hex)` (833), `nextHexCost` (232), `upkeepGuards` (1865), `setGuards` (1816), `setPosts` (1806), `postOwner` (1797), `tollRate` (1787), `oweToll` (1453); `settlement/Census.java` `value` (24); `caravan/Caravans.java` `payTolls` (1912–1960), `ends` (2040); `territory/Labour.java` `hands` (116) for militia; `docs/decisions.md` "Territory, farms and tolls" (growth, posts, guards), "Guards on credit" |
| Covers | `TerritoryTests.homeHexFirstThenGrowthWithCulture`, `.growthStopsAtTwoRings`, `.aHeldHexStaysWithItsOwner`, `.fedAndPaidGuardsStay`, `.unfedGuardsDesert`, `.caravansPayTollsOnlyToLandTheyCross`; `LabourTests.handsStandWatchForTheirFood` |
| Rust | adds `territory.rs` (`Territory` resource: hex → owner; per town `Holding { hexes, culture, posts: Vec<Hex>, guards, toll_rate, passage_fee, deserted }`), `rules/territory.rs` (`Review`: culture and claims daily; `Pay`: guards' food and pay, desertion; `Turn` home: tolls on the purse to each posted owner crossed); touches `state.rs`, `content.rs` (dials `culture_per_hex 5`, `max_rings 2`, `guard_food 6`, `guard_pay 0.5`, `guard_credit_days 3`, `default_toll_rate 0.05`), `society.rs` (`claim(town)`, `set_posts(town, hexes)`, `set_guards(town, n)`, `set_toll(town, rate)`, `holding(town)`) |
| Math | — |
| Accept | the Java numbers: census value 5 → 2 hexes after a day, 3 after three; 1,000 days → 19 hexes within distance 2; a rival on ring 1 → home only; 2 guards 5 days → nobody deserts, 12 bread eaten (60 points); 4 guards, no food, 3 days → fewer than 4; a 1,000-block path crossing two posted hexes of one town at 5% and another's → 50 then 47 of the coin carried, nothing to a town off the path; one idle hand and one post → militia 1, pay 0, bread eaten |
| Model | **sonnet / medium** — small arithmetic rules with exact numbers on top of `hex-map`; the chokepoint search is deliberately left to 3.1. |

Brief:
```
# territory-growth
Read the Java in plan 2.5 and sim/core/src/hex.rs, map.rs (as landed), farms.rs::culture_factor (or define it here if
farms hasn't landed: clamp(1 + 0.25·mood, 0.25, 2)). Add sim/core/src/territory.rs: `Territory` resource (hex → owner
TownId; per town a `Holding`). Review (daily, each town with a hex): claim its home hex if unowned; add culture =
(census value + extras hook: wall 2, library 3, caravanserai 1) × days × culture_factor(mood), where census value =
hands (babies count 1, confined weight is the binding's); while culture >= 5 × max(1, hexes held), claim the unowned
neighbour of any held hex within 2 of home that's nearest home's position (then lowest q, then r) and pay the cost.
Posts: `Holding.posts` are held hexes with a guard; guards = posts; militia = min(posts, idle hands) stand watch fed but
unpaid. Pay (on touch): food owed = posts × 6 × days from the warehouse (whole items); pay owed = (posts − militia) × 0.5
× days from the treasury (post "guards"), on credit up to 3 days; kept = min(fed, paid), stay = floor(posts × kept),
the earliest-posted stay, the rest desert (counted). Tolls (Turn, Leg::Home, before the purse is banked): group the
route's path by owner, skipping the route's own two towns; for each owner with a post on the path, toll = min(carried,
floor(carried × rate + fee)) moves from the wagon's purse to the owner's treasury (post "toll"), once per owner, in path
order. Default rate 0.05; `set_toll`. Tests in tests/territory.rs with plan 2.5's numbers, tagged. Random worlds: towns
on the map claim and grow; some post guards.
```

#### 2.6 `workshop`

| | |
|---|---|
| Goal | Trades convert goods on touch: the mod's seven conversions, whole batches, gated by a trade on the roster, how much is wanted (comfort par, food, the lockers' par), the most gain per emerald of input first, and demand traced back two steps for the farms. |
| Java | `territory/Workshop.java` `CONVERSIONS` (35), `potential` (61), `comfortWanted` (219), `inputDemand` (104), `convert` (176), `gain` (151), `inputsWorth` (156), `run` (240); `territory/Comforts.java` `kindOf` (60), `kinds`; `docs/decisions.md` "Trades convert raw goods", "Where the next item earns most", "Potential becomes demand" |
| Covers | `JobsTests.caneIsPressedIntoPaper`, `JobsTests.townsMakeTheirOwnComforts` |
| Rust | adds `workshop.rs` (`Conversion { trades, inputs: Vec<(GoodId, f64)>, output, out }` in content; `wanted`, `gain`, `run`, `input_demand`), `rules/workshop.rs` (`Work`, after farms' produce lands); touches `content.rs` (`Content.conversions` with the seven, matched by good name so hosts without a good skip it), `society.rs` (`add_conversion`) |
| Math | **Leontief input-output** in its simplest form: `input_demand` is the two-step back-substitution; write it as a small matrix walk and tick the row "started" |
| Accept | 40 cane → 40 paper; 10 wheat → 3 bread and 1 left; 10 wheat + 2 cane → 8 cookies, 6 wheat left, cookie gain > 1.4 × paper gain; nothing runs without the trade; a comfort is wanted up to `comfort_par − held` in items × (1 + routes out); bread wanted while food < 2 × 320; conservation holds (inputs `used`, outputs `made`) |
| Model | **sonnet / medium** — a table and three small functions with given expected outputs. |

Brief:
```
# workshop
Read the Java in plan 2.6 and sim/core/src/people.rs (comfort_par, food_points), roster.rs, farms.rs. Add
sim/core/src/workshop.rs: `Conversion { trades: Vec<TradeId>, inputs: Vec<(GoodId, f64)>, output: GoodId, out: f64 }`;
Content.conversions built by name from the mod's table (librarian/cartographer: 1 sugar_cane → 1 paper; farmer: 3 wheat
→ 1 bread; shepherd: 2 white_wool → 3 white_carpet; leatherworker: 4 leather → 1 leather_boots; fisherman: 1 cod → 1
cooked_cod; farmer: 2 wheat + 1 sugar_cane → 4 cookie; librarian: 3 paper + 1 leather → 1 book), skipping any whose
goods the host's content lacks. wanted(output): a comfort → ceil((comfort_par − held worth of its kind) × items per
emerald) × (1 + routes out of the town); bread/cooked cod → (2·320 − food points)/5; else par − held if some locker buys
it. gain = out × value / inputs' worth. On touch (Work, after farms' delivery), for each conversion whose trade is on the
roster, in gain order, run whole batches = min(ceil(wanted/out), inputs available/in), leaving forward-sold stock alone
(hook: `set_aside(good)` from harvest-and-rents, 0 if absent). input_demand(good) traces a wanted output back through
its inputs two steps (books → paper → cane) and is exposed for farms' field choice. Tests in tests/workshop.rs with plan
2.6's numbers, tagged. Random worlds: towns with the trades and goods convert. Note the Leontief row in
docs/core-math.md as started.
```

#### 2.7 `harvest-and-rents`

| | |
|---|---|
| Goal | Forward crop sales (a harvest term: a four-day window, shortfalls roll over at a quarter of a miss, arrears past two installments miss), rents (a capital of eight seasons' crop for a coupon a season, arrears, default, buy-back) and the dealer's spread; owed crop is set aside from spot sales. |
| Java | `contract/Harvest.java` `windowTicks` (19), `carried`; `contract/ContractBook.java` `review` 320–337 (the harvest branch), `close` (269); `contract/Rents.java` `capital` (114), `sell` (119), `review` (136), `issue` (151–183), `setBearer` (205), `takeAccrued` (213), `redeem` (241); `caravan/ForwardSales.java` `offer` (44), `setAside` (114), `earmarked` (139); `market/RentDealer.java` `price` (34); `market/RentDeeds.java` `quote` (83: ask/bid = price × (1 ± spread)); `docs/decisions.md` "Forward crop sales, rents and a dealer in them" |
| Covers | `ContractTests.aThinHarvestRollsOverWhereAPlainContractMisses`, `ContractTests.aRentPaysACouponEachSeasonAndCarriesItsArrears`, `MarketTests.aPlayerBuysARentsDeedCollectsItsCropAndSellsItBack` *(partial: the rent's coupon cycle, bearer and accrued crop, ask and bid; the vendor is Numismatics)* |
| Rust | adds `harvest.rs` (`Harvest { window_days, carried }` term on `Contract`; `Rents` resource: `Rent { farm, holder, crop, quantity, capital, season, arrears, coupon: Option<contract id>, bearer, accrued }`; `set_aside`; the forward `offer` rule; the dealer's `price`); `rules/harvest.rs` (`Judge`: the harvest branch; `Review`: seasons issue coupons, defaults; `Arrive`: a market whose lockers want a crop offers the farm a forward sale, a third up front); touches `contracts.rs` (`Contract.harvest: Option<Harvest>`), `content.rs` (dials `harvest_window_days 4`, `harvest_arrears 2`, `harvest_rolled_weight 0.25`, `rent_season_days 10`, `rent_purchase_seasons 8`, `rent_arrears_seasons 2`, `dealer_spread 0.1`, `sow_days 4`, `forward_installments 10`), `society.rs` (`sell_rent`, `rents()`, `set_aside(town, good)`) |
| Math | — |
| Accept | the Java numbers: 16 a day for 5 installments with 8 delivered: the first rolls, arrears 40 > 32 miss, what's owed at the last misses; a plain contract misses 3 and breaks; a rent's capital 8 × 64 × 0.05, the coupon in force at once at price 0, a 24 shortfall → next coupon 88, still active at 88 arrears, defaulted at 152; a dealer's ask/bid a tenth either side; set-aside keeps owed crop from `sell` and the workshop |
| Model | **sonnet / medium** — extends `contracts.rs` with two terms whose judging rules and numbers are given. |

Brief:
```
# harvest-and-rents
Read the Java in plan 2.7, docs/decisions.md "Forward crop sales, rents and a dealer in them", and sim/core/src/
contracts.rs, rules/contracts.rs, rules/advances.rs. Add a `Harvest { window_days, carried }` term to Contract and the
harvest branch to Book::review: judged window_days after its day, not GRACE; a short installment rolls its shortfall
onto the next (trust: missed × 0.25); arrears past harvest_arrears × quantity, or what's owed at the last installment,
are misses. Add sim/core/src/harvest.rs with `Rents`: sell(farm, holder, crop, quantity, price): capital = 8 × quantity ×
price paid holder → farm (post "rent"); each season (10 days) issue a coupon: a one-installment contract for quantity +
arrears at price 0 with Harvest{carried: true}, in force at once; a shortfall rides on the next coupon; arrears > 2 ×
quantity default the rent (farm's miss ×3); bearer rents accrue crop collected for the deed's holder; redeem (buy back)
at the capital when the farm can spare twice it. Dealer: price = capital × (0.5 + 1.5 × how short the hub's markets are
of the crop); ask/bid = price × (1 ± dealer_spread). Forward offer (Arrive, Sell step after the sale): a buyer whose
lockers use a crop more than its farms grow offers the farm town whose wagon came a forward sale: what its lockers use a
round trip, ten installments at today's price, first due after sow_days, a third up front (advance), capped by what the
farm could add (idle plots, spare hands, half its crop, less promised). set_aside(town, good) = owed by the end of the
next window − delivered − on the road; make rules/arrive.rs::sell and workshop leave it (export the fn; the workshop
reads it if present). Tests in tests/harvest.rs with plan 2.7's numbers, tagged (the MarketTests one partial). Random
worlds: some forward sales and rents.
```

#### 2.8 `atlas-and-books`

| | |
|---|---|
| Goal | What a Merchant knows: the markets within word of mouth and those its wagons and crossings taught it; stocktakes of the far end (a Librarian's books) that every "what do they hold" decision reads; the Merchant's books of what was sent, sold and bought and the last price seen; the tally of reasons; and a town short of comforts commissioning its own route. |
| Java | `caravan/Atlas.java` `known` (57), `learn` (42), `learnBoth`, `markets`, `reach`, `WORD_OF_MOUTH`; `merchant/Warehouse.java` `recordStocktake` (186), `stocktakeTime`, `onHand` (203); `caravan/CaravanServices.java` `booksStale` (113), `copyBooks` (116); `merchant/Books.java` `enter` (113), `file` (156), `lastPrice` (185), `why` (122), `tally`, `keeps`; `caravan/Caravans.java` `pickCargo` (2361: the reasons), `dispatch` 498 and 570 (home stocktake, SENT), `arriveAtDestination` 696–703 (BOUGHT, the log), `arriveHome` 1160–1192 (HOME, file, news → Atlas, towns crossed learn both ends); `caravan/Commissions.java` `consider` (46), `COMMISSION_SPARE`; `merchant/CommissionPayload.java` `commission` (43: known and in range); `docs/decisions.md` "News travels with carriers: each desk keeps an atlas", "Towns commission their own caravans", "The town's shop, the Merchant's books, and its reasons" |
| Covers | `CaravanTests.aMerchantNotesWhyGoodsGoOrStay`, `.aTownShortOfComfortsCommissionsItsOwnCaravan`, `.bothMerchantsKeepBooksOfATrip`, `.aLibrarianBringsBackTheBooks` *(partial: the stocktake is taken by a Librarian aboard; needs `squads`)*, `.caravansAreCommissionedAtTheDesk` *(partial: a route may be opened only to a known market in range; the screen is Minecraft's)* |
| Rust | adds `atlas.rs` (`Atlas` resource: per town the markets known; `Stocktakes`: per (observer, market) the stock seen and when; `Books`: per town entries by kind and day, kept 10/60/365 days by Merchant/Librarian/library; `Whys` tally), `rules/atlas.rs` (`Plan`: wagons plan on the far end's last stocktake, not its live stock, and record why each good goes or stays; `Turn`: SENT/SOLD/BOUGHT/HOME entries, news learned; `Review`: commissions); touches `state.rs`, `content.rs` (dials `word_of_mouth 400`, `commission_spare 8`, `stale_books_days 3`), `society.rs` (`known(town)`, `books(town)`, `whys(town)`, `stocktake(observer, market)`, `commission(from, to) -> Result`) |
| Math | — |
| Accept | the Java fixture: a far end last seen at 500 wheat over a par of 64 → "not sent: they hold their fill"; cane none → "we have none"; paper sent; a WHY from a wagon's log is tallied, not paged; a town of 5 without comforts, 30 emeralds and a known hub holding cookies commissions a route; home's books show SENT, SOLD with takings, the far end's BOUGHT, `last_price` set; a route to an unknown or out-of-range market is refused |
| Model | **sonnet / medium** — bookkeeping with clear records; the one design point (plan on the stocktake, not live stock) is stated. |

Brief:
```
# atlas-and-books
Read the Java in plan 2.8 and sim/core/src/rules/plan.rs, rules/arrive.rs, hex.rs/map.rs, squad.rs (as landed). Add
sim/core/src/atlas.rs: `Atlas` (per town: BTreeSet<TownId> known; known(town) also includes every placed town within
word_of_mouth 400 blocks); learn on a wagon's return (both ends know each other; every town whose hex the path crossed
learns both ends); `Stocktakes` (observer, market) → (Vec<f64> stock, when): home's is taken as each wagon leaves; the
far end's when a Librarian is aboard (or, for towns without rosters, at every visit, so today's behaviour holds) and
it's stale after 3 days; `Books` per town: entries (day, kind SENT/SOLD/BOUGHT/HOME, with: TownId, good, count, price),
kept 10 days (60 with a Librarian hand, 365 with a library hook), `last_price(town, market, good)`; `Whys`: a tally
(with, good, why) → (count, items, last day), never paged. Change rules/plan.rs::plan_sale to size a load for the far
end by its last stocktake (par − seen, less what's aboard other wagons bound there), falling back to live stock when
there's none, and to record a reason per good ("they hold their fill", "we have none", "set aside", "no room aboard",
"sent"). Review: Commissions.consider — a town with no route of its own, a comfort kind met < 1, and
commission_spare beyond its reserve opens a route (add_road if both placed, else add_route at the straight distance)
to the nearest known market in range whose last stocktake holds that kind. Society::commission(from, to) refuses a
market not known or out of range. Tests in tests/atlas.rs with plan 2.8's numbers, tagged (two partial). Random worlds:
atlases fill as wagons run.
```

#### 2.9 `herds`

| | |
|---|---|
| Goal | Towns keep herds on the ledger: seeded by a town's first Shepherd, Butcher or Leatherworker; fed from the warehouse (feed joins the farms' demand); growing 15% a day to 16 a pasture or dwindling hungry; shorn, culled above the breeding stock for meat and hides. |
| Java | `territory/Husbandry.java` `KINDS` (37), `seed` (69), `feedDemand` (84), `capacity` (94), `tend` (101–140) and the constants; `docs/decisions.md` "Herds", "Leatherworkers and Fishermen" (cows) |
| Covers | `HusbandryTests.shepherdsAndButchersBringAnimals`, `.leatherworkersKeepCowsForHides`, `.aFedFlockGrowsAndIsShorn`, `.aHungryFlockDwindles`, `.butchersSlaughterWhatTheFullPastureBreeds`, `.aCulledHerdGrowsToItsPastureFirst`, `.aSecondPastureHoldsTwiceTheHerd`, `.aHerdTendedOftenEatsOnlyItsShare` |
| Rust | adds `herds.rs` (`Herds` component: per kind count (fractional), feed owed, seeded flags, pastures; `Kind` content: feed good(s) and rate, meat good and yield), `rules/herds.rs` (`Work`, before farms' plan so feed demand is known); touches `state.rs`, `content.rs` (`Content.animals` by good names; dials `pasture_animals 16`, `breed_per_day 0.15`, `starve_per_day 0.1`, `cull_share 0.75`, `breeding_stock 4`, `slaughter_per_day 0.5`, `wool_per_sheep_day 0.5`), `society.rs` (`herds(town)`, `set_pastures`) |
| Math | **logistic growth with carrying capacity** (the pasture); tick the row for herds |
| Accept | the Java numbers: a Shepherd brings 3 sheep, a Butcher 2 pigs and 2 chickens, a Leatherworker 2 cows; 4 sheep → 1 wheat/day feed demand; 3 sheep, 64 wheat, 4 days → more than 4 sheep, wool > 5, exactly 3 wheat eaten; 6 sheep unfed 4 days → fewer; 16 pigs 2 days → 12–16 pigs, porkchop > 4; 4 pigs tended 80 times a quarter-day → more than 11; two pastures, 16 sheep, 3 days → more than 20; 4 sheep tended 100 times a hundredth-day → 1–2 wheat eaten, more than 4.4 sheep; 16 cows culled toward 12 over 2 days → leather > 2, beef > 4, and 9 leather → 2 boots through the workshop |
| Model | **sonnet / medium** — one `tend` function with fractional counts and carried feed; every expected number is given. |

Brief:
```
# herds
Read territory/Husbandry.java (150 lines) and docs/decisions.md "Herds"; then sim/core/src/roster.rs, farms.rs,
workshop.rs as landed. Add sim/core/src/herds.rs: `Content.animals`: sheep (0.25 wheat/day; 1.5 mutton; wool), cow
(0.25 wheat; 2 beef; 1 leather), pig (0.25 of carrot/potato/beetroot; 2 porkchop), chicken (0.1 wheat; 1 chicken),
matched by good name. `Herds` component: count per kind (f64), feed owed per kind, seeded per kind, pastures (max(1,
n)). Seed once: a Shepherd hand 3 sheep, a Butcher 2 pigs + 2 chickens, a Leatherworker 2 cows. tend(days): capacity =
16 × pastures; keep = max(4, 0.75 × capacity / kinds present); per kind: due = owed + count × feed × days, eat
floor(due) from the warehouse (Ledger.used; any of the kind's feed goods), owed = due − eaten; if owed < 1 grow
min(room, count × 0.15 × days) else fed = max(0, 1 − (owed − 1)/need), lose count × 0.1 × (1 − fed) × days and cap owed
at 1; a Shepherd shears wool += count × 0.5 × days (Ledger.made); a Butcher culls any kind and a Leatherworker cows:
above keep, slaughter min(surplus, surplus × 0.5 × days) for meat each and 1 leather a cow. feed_demand() per good for
farms (export; farms reads it if present). Runs in Work before farms' plan. Tests in tests/herds.rs with plan 2.9's
numbers, tagged. Random worlds: towns with the trades keep herds. Tick the logistic-growth row in docs/core-math.md for
herds.
```

---

### Wave 3: trade between towns in full (on wave 2)

#### 3.1 `chokepoints`

| | |
|---|---|
| Goal | Towns work out which of their hexes to guard and what to ask: each crossing shows what a post there would cost the wagon in detour; every three days the town picks the posts and rate (up to 80% of the detour and 25%, more with armed and armoured guards) and a passage fee; what a bridge would be worth, and where a second one is worth nothing. |
| Java | `caravan/Caravans.java` `postOptions` (1981), `better` (2022), `impassableNeighbours`, `bridgeValue` (2083), `observeCrossing` call in `payTolls`; `territory/Chokepoints.java` `Crossing.bearableRate` (58), `fewestPostsFor` (63), `bearableFee` (85), `bestPlan` (105), `incomePerDay`; `territory/Territory.java` `observeCrossing` (1838), `recordTraffic`, `reviewTolls` (1939–1965), `guardCostPerDay`, `passageFee`; `territory/Smithing.java` `ARMED_DETOUR_SHARE`, `ARMOURED_TOLL_RATE`; `caravan/Route.java` `expectedValue` (218); `docs/decisions.md` "Towns work out whether they hold a chokepoint", "Bridges", "Tolls a caravan" |
| Covers | `TollTests.onlyPostsCollect`, `.aTownOnOpenGroundPostsNoGuard`, `.aTownHoldingTheOnlyCrossingTollsIt`, `.aTownTollsUpToTheDetour`, `.armedAndArmouredGuardsAskMore`, `.twoBridgesTakeTwoPosts`, `.aBridgeIsWorthMostWhereNothingCrosses`, `.aSecondBridgeBesideTheFirstIsWorthLittle` |
| Rust | adds `rules/chokepoints.rs` (`Turn`: each crossing of a town's land records the route and tries posts; `Review` every 3 days: the plan), `chokepoints.rs` (`Crossing`, `post_options`, `best_plan`, `bridge_value`); touches `territory.rs` (`Holding.routes_seen`, `options`, `armed`, `armoured` shares as hooks for `mines-and-kit`), `content.rs` (dials `detour_share 0.8`, `max_toll_rate 0.25`, `passage_fee_cap 3`, `max_posts_per_route 6`, `toll_review_days 3`, `forget_after_reviews 4`), `society.rs` (`review_tolls(town)`, `bridge_value(town, hex)`) |
| Math | **spatial price equilibrium**: the rate settles at the detour's cost; add a check that a town never asks more than going around costs |
| Accept | the TollTests fixture with PER_HEX 8.5 and value 1,000: open ground → detour ≤ one hex, no guard after review (7 spurs a day < 11.2 guard cost); the only crossing → post on the bridge at 0.25 with a fee > 0 and the next wagon pays floor(250 + fee); a ford six hexes off → rate 0.8·detour/1000 and wagons still take the bridge; armed 1 → min(0.25, 0.95·detour/1000); armour 1 with no way round → above 0.25 up to 0.35; two adjacent crossings → both posted at 0.25 after two reviews; a bridge where nothing crosses is worth min(0.25·300, 0.8·300) = 75 a day; beside the town's own posted bridge, 0 |
| Model | **opus / medium** — a greedy search over post sets with tie-breaks, a payback decision and three interacting caps; the Java is spread over three files and the tests pin exact outcomes of the search. |

Brief:
```
# chokepoints
Read the Java in plan 3.1 and sim/core/src/territory.rs, map.rs, hex.rs as landed. On each wagon's crossing of a
town's land (Turn, home leg, alongside tolls): record the route (value = spurs collected per trip, 300 before the
first) and try posts: post_options — repeatedly, over the town's hexes on the wagon's path, place a post on the hex
that makes the cheapest path around dearest (ties: most impassable neighbours, then nearest home), re-plan around the
posts so far (map::plan with the posts, the town not free), until the path keeps off its land, there's no way at all,
or 6 posts; each option records its detour (road cost difference, infinite when no way) and the other towns' tolls on
it. Review every toll_review_days: for each route seen (forgotten after 4 reviews unseen), bearable rate =
min(cap, share × detour / value) with share = 0.8 + 0.15 × armed share and cap = 0.25 + 0.10 × armoured share; fee =
max(0, min(3 emeralds × rate in spurs, share × detour − rate × value)); best_plan picks the set of posts and rate whose
income (Σ routes' min(rate × value, bearable) + fee, one trip a day each) less guards' pay (0.5 + 6/30 emeralds a day
each) is greatest and positive; drop unwanted posts at once, hire one guard a review. bridge_value(town, hex): for each
route the town knows that isn't its own, plan with and without the hex bridged (the town itself free), saves = cost
without − cost with (the whole value when there's no way without), worth = min(0.25 × value, 0.8 × saves) a trip, one a
day; 0 on dry land. Hooks: Holding.armed and armoured shares (mines-and-kit sets them; default 0). Tests in
tests/chokepoints.rs on TollTests' fixture with plan 3.1's numbers, tagged. Random worlds: toll reviews run. Add a
check in tests: a town's rate × value never exceeds the detour it asks against (spatial equilibrium); tick that row.
```

#### 3.2 `services`

| | |
|---|---|
| Goal | What each trade does for a town, at home or abroad: a registry of services (trade, where, wanted now, do it), the most skilled of the trade doing it; abroad the town served pays into the wagon's purse and trust rises; the built ones: blessing, bookbinding, harvest help, map swap, survey, droving, and the Crafts recipes (smelting, kit, arrows, bows, saddles, smoking). |
| Java | `service/Service.java`, `service/Services.java` `all/of` (52–61), `home` (139), `abroad` (103), `wantedAbroad` (85), `residents`, `skills`; `service/Visit.java` (`planning`, `hear`); `service/TownServices.java` `register` (49), `bless` (109), `blessingWanted` (100), `harvestWanted` (118), `helpHarvest` (132), `swapWanted` (160), `swapMaps` (166); `service/Crafts.java` `register` (44), `recipe` (151), `wanted` (194), `batches` (178), `make` (201), `kit` (121), `booksWanted` (303), `contractInputs` (247); `caravan/CaravanServices.java` `drovingWanted` (49), `drove` (63), `surveyWanted` (94), `survey` (99); `caravan/Cartography.java` `townsAlong` (32); `territory/Territory.java` `bless` (1685), `owe` (1712); `docs/decisions.md` "Services", "The metal chain" (smelting abroad) |
| Covers | `ServiceTests.everyTradeSaysWhatItDoes`, `.aClericAtHomeBlessesItsTown`, `.aLibrarianBindsBooksWhileReadingIsShort`, `.aVisitingFarmerBringsInTheUntendedHarvest`, `.aMapSwapTeachesBothTowns`; `MetalTests.anArmorerSmeltsAndAToolsmithForgesHoes`, `.aVisitingArmorerSmeltsForATownWithoutOne`; `CaravanTests.aVisitingClericBlessesATownWithoutOne`, `.aShepherdSellsSheepWhereThePastureIsShort` *(partial: the herd carried and sold where pasture is short, at a bargained price; pens are Minecraft's)*, `.aCartographerBringsHomeAMapAndTheTownsOnTheWay` *(partial: towns within two hexes of the road become known at home; the map item is Minecraft's)* |
| Rust | adds `services.rs` (`Service` trait or enum with `trade`, `where_`, `describe`, `wanted(town, planning, budget)`, `perform(...) -> fee`; `Services` registry built from content; `Crafts` recipes as `Recipe { trade, inputs, output, out, per_level, fee_each, min_level }`), `rules/services.rs` (`Work`: home services after the workshop; `Services` step abroad: each squad trade's services, paid from the far end's money beyond its reserve into the purse, trust +0.1 a paid service; `Plan`: `Needs.wanted` for `squads` from `wanted(planning = true)`); touches `people.rs` or `state.rs` (`People.blessed_until`, mood +0.5 while blessed), `content.rs` (dials `blessing_emeralds 2`, `blessing_days 2`, `map_emeralds 0.25`, `trust_per_service 0.1`), `society.rs` (`services_of(trade)`, `done(town)`), `territory.rs` hook (untended plots from farms) |
| Math | — |
| Accept | the Java numbers: 11 trades each with a service and a description; a Cleric's town is blessed past now after a touch; 9 paper + 5 leather → 3 books, 2 leather left; 80 idle plots and a Novice visitor → 16 wheat for a fee between 0 and its worth; a map swap teaches both towns and the one that learns more pays 0.25 a market; 16 raw iron + 2 coal with an Armorer, a Toolsmith and 2 Farmers → 2 hoes, 12 ingots left, hoe share 1.0; a visiting Armorer with budget 5 → 16 ingots for 0.32; a visiting Cleric blesses a town of 5 for 2 emeralds and the route earns it; a Shepherd's 3 sheep sell where pasture (2 a villager) is short |
| Model | **sonnet / high** — a registry pattern plus a dozen small rules with given numbers; the breadth (seven Java files) is the load. |

Brief:
```
# services
Read the Java in plan 3.2 (service/* is 1,500 lines: the named methods) and sim/core/src/roster.rs, squad.rs, herds.rs,
farms.rs, workshop.rs, atlas.rs as landed. Add sim/core/src/services.rs: `enum Where { Home, Abroad, Road }`, a
`Service` (name, trade, where, min_level, description, `wanted(ctx) -> bool`, `perform(ctx) -> fee`) and a registry
`Services::all()` built from content, with the built ones: cleric home_blessing (wanted if blessed_until < now + a day;
blessed a day) and blessing abroad for a town without a Cleric (2 emeralds; 2 + level days; mood target +0.5 while
blessed, hook into people.rs's mood target); librarian bookbinding (Crafts: 3 paper + 1 leather → 1 book, 16 a level,
fee 0.05 abroad for a town without a Librarian, wanted up to the comfort par); farmer harvest help abroad (a day's
harvest of idle plots up to 32 × level, fee = worth × the town's labour share, within budget); cartographer map swap
(each learns the other's known markets; the one that learns more pays 0.25 a market) and survey (towns within 2 hexes
of the path learned at home; the road marked mapped, stale after 10 days); shepherd droving (herd up to 8 animals from
home; sell where villagers × 2 − pasture animals is short at a bargained price; unsold go home); Crafts recipes as in
the Java table (smelting 8 raw iron + 1 coal → 8 ingots, 4 a level, 0.02 abroad; hoes, pickaxes, swords, armour, bows
from the kit table, 2 a level; arrows; shields; banners, smoking and saddles at level 5), batches = min(ceil(wanted/out),
per_level × skill, inputs, 64/out, budget/(fee × out)), kit wanted = wearers + contracted − held. Home services run in
Work after the workshop; abroad in the Services step by each crew trade, the most skilled aboard, paid from the far end's
money beyond its reserve into the purse (post "service: <name>"), trust += 0.1 a paid service; planning: `wanted(planning
= true)` fills squads' Needs.wanted. Tests in tests/services.rs with plan 3.2's numbers, tagged (two partial). Random
worlds: crews carry services.
```

#### 3.3 `mines-and-kit`

| | |
|---|---|
| Goal | What the land gives and what kit does: a Toolsmith's mine (ore per level a day, lapis when deep), hands gathering what their hex's ground gives, Fletchers hunting with bows, kit wearing out (slower mended, enchanted), tool and bow shares, and armed and armoured guards. |
| Java | `territory/Mines.java` constants (17–19); `territory/Territory.java` `touch` 1328–1365 (mine, labour, hunting, wear), `sinkDeepMine` (434); `territory/Labour.java` `hauls` (54), `biome` (108), `hands` (116), `work` (130), `room` (95), `NITWIT_FACTOR`, `KEEP`, `KEEP_WANTED`; `territory/Hunting.java` `game` (19); `territory/Smithing.java` `wear` (125–140), `wearRate` (97), `quality` (86), `share` (111), `wearers`, `hunting` (65), `armed` (73), `KITS`, `TOOL_BONUS`, `SNARES`; `service/MasterServices.java` `investWanted` (114), `invest` (119) (the deep mine only); `docs/decisions.md` "The metal chain", "Labour, Masons, Fletchers and masters" (hands, Fletchers), "Inventory waste" (tools for hands) |
| Covers | `MetalTests.aToolsmithsMineBringsUpOre`, `.toolsWearOutSlowerWhenMended`; `MasterTests.unbreakingKitLastsLonger`, `.aMasterToolsmithSinksTheMineDeep`; `LabourTests.aNitwitGathersWhatTheLandGives`; `BuildingTests.aFletcherHuntsWithABow`, `.sparesBowsArmTheGuards` |
| Rust | adds `kit.rs` (`Kit` content: tool/weapon/armour kinds by good name, lifetime days, inputs; `Smithing`: wear, shares, armed, hunting; `Ground.kind` gathering tables keyed by a `Land` enum in `map.rs`: woods, taiga, desert, shore, hills, plains, water), `rules/kit.rs` (`Work`: mine, gather, hunt; `Pay`: wear), `rules/mines.rs` folded in; touches `map.rs` (`Ground.land`), `territory.rs` (`Holding.armed/armoured/hunting` shares, `mine: Option<Mine { deep }>`), `content.rs` (dials `raw_iron_per_level_day 2`, `coal 0.5`, `lapis 0.5`, `tool_bonus 0.5`, `hand_tool_days 16`, `mine_emeralds 6`, `deep_mine_emeralds 8`), `society.rs` (`set_land(hex, land)`, `kit(town)`) |
| Math | — |
| Accept | the Java numbers: a level-2 Toolsmith → 4 raw iron and 1 coal a day; 4 hoes, 2 farmers, 16 days → 2 used, 1 when mended; two Unbreaking III hoes (wear rate 1/4) → 0 used; Toolsmith 5 + Mason 3 + 30 emeralds → a deep mine, 2 lapis next day; a Nitwit's first day's haul = floor(perDay × 1.5) of its land's table; a Fletcher with a bow → hunting 1.0 and feathers; armed(2 bows, 1 sword; 2 guards, 1 hunter) = 1.0, armed(4 guards, floor 0.5) = 0.5 |
| Model | **sonnet / medium** — tables and rates, all given; the only judgment is replacing biomes with a land kind per hex. |

Brief:
```
# mines-and-kit
Read the Java in plan 3.3 and sim/core/src/map.rs, territory.rs, roster.rs, services.rs (Crafts' kit table) as landed.
Add `Land { Plains, Woods, Taiga, Desert, Shore, Hills, Water }` to map.rs Ground (default Plains) with Labour.hauls's
tables by land (logs/sticks/apples/string in woods; spruce and berries in taiga; sand and cactus in desert; clay, cane,
ink by water; stone and flint in hills; seeds, flowers, sticks, flint on plains) and Hunting.game (beef, pork, chicken 1
each, feathers 2, leather 0.5 in temperate; rabbit 2, hide 1, feathers 1 cold or dry; feathers 1 by water). Add
sim/core/src/kit.rs: kit kinds by good name (hoe 16 days/2 ingots, pickaxe 16/3, sword 20/2, chestplate 40/8, bow
20/3 sticks + 3 string; axe 16), `Kit { good, count, enchant_level: f64, unbreaking: u8 }` entries in a `Kits` component;
wear_rate = 1/(1 + unbreaking) × (mended ? 0.5 : 1) × (smithy ? 0.8 : 1); quality = 1 + 0.15 × level up to 1.5; shares:
tool share = tools/wearers; hunting = min(1, bows/hunters); armed = max(floor, min(1, (swords + max(0, bows −
hunters))/guards)); armoured = chestplates/guards. Work: a town with a Toolsmith and 6 emeralds digs a mine; each
Toolsmith brings up 2 raw iron and 0.5 coal a level a day (×1.5 with pickaxes), 0.5 lapis a level when deep (Toolsmith
5 + Mason 3 + 8 emeralds); idle hands (nitwit × 1.5) each gather one held hex's table × (1 + 0.5 × tool share), stopping
at 16 of a good the town doesn't use (64 if it does); Fletchers hunt levels × (0.25 + 0.75 × hunting) × game. Pay: kit
wears by days × rate; worn-out kit is removed. Set Holding.armed/armoured/hunting each touch. Tests in tests/kit.rs
with plan 3.3's numbers, tagged. Random worlds: hexes get a land kind; towns with Toolsmiths mine.
```

#### 3.4 `desires-and-backhauls`

| | |
|---|---|
| Goal | What a town will pay for a comfort by how short it is (up to twice worth, half when glutted), what a Merchant buys to sell on (only at a 15% margin after the road), a town's own shopping trip (whole items, from money beyond its reserve), comforts judged from the last stocktake less what's been used since, and buying a town's surplus for the trip home only where another town on the network wants it. |
| Java | `caravan/Desires.java` all (`MOST`, `GLUT`, `MARGIN`, `worthBuying` 125, `haulEach` 86, `outlet` 99, `reckon`, `willPay`); `caravan/Caravans.java` `comfortWants` (2207), `pickComforts` (2233), `dispatch` 326–337 and 472–494 (shopping purse, comfort block), `buyForHome` (1868), `buySurplus` (2742), `networkWants` (2623), `buyBackhaul` (2673), `sell` 2431 comfort branch; `caravan/Commissions.java` `shoppingList` (141); `market/ShopBoard.java` `wants` (193), `add` (222) (what the shop lists and at what price); `docs/decisions.md` "Desires", "Towns buy comforts by caravan", "Hauling only what sells", "The town's shop" |
| Covers | `CaravanTests.caravansAllowForComfortsUsedSinceTheStocktake`, `.aTownShortOfClothesBuysAPairOfBoots`, `.aTownsOwnCaravanShopsForHome`, `.aMerchantBuysToSellOnOnlyAtAMargin`, `.caravansBuyATownsSurplusForTheTripHome`, `.caravansDontBuySurplusNobodyWants`; `MoodTests.theShopListsWhatTheTownIsAfterAtWhatItWouldPay` *(partial: the price the town would pay and the listing as news; the vendor is Minecraft's)* |
| Rust | adds `desires.rs` (`will_pay`, `worth_buying`, `haul_each`, `outlet`; `Listings` per town: good → price asked, as news other Merchants read), `rules/desires.rs` (`PlanBuy`: the shopping purse and the far end's comfort wants from its stocktake less `villagers × 0.1 × days`; `Buy`: `buy_for_home` at bargained prices up to full value, whole items; `Buy` after: `buy_surplus` for the network's wants at a 0.75 ceiling, keeping the far end's food par; `Sell` comfort branch: the far end pays by desire, from money beyond its reserve); touches `content.rs` (dials `desire_most 2.0`, `desire_glut 0.5`, `resale_margin 0.15`, `resale_ceiling 0.75`, `comfort_margin 1.5`), `state.rs` (`Wagon.shopping: Vec<f64>`, `Plan.shopping`), `society.rs` (`listings(town)`) |
| Math | **square-root price impact** stays "maybe": note that `will_pay` is the mod's shortage-linear impact |
| Accept | the Java numbers: a town of 5 wants no treats right after a stocktake and 1.0 emerald's worth two days later; boots at 4 emeralds against a 3-emerald par → exactly 1 pair; a town of 5 with 20 emeralds and no comforts brings home 54 cookies; worth_buying(0.40, outlet 0.50) true, (0.45) false; haul_each in (0, 0.01); carrots bought for the trip home only when another route's end wants them and the far end keeps 320 food points |
| Model | **sonnet / medium** — pricing rules with given numbers, slotted into the `Buy`/`PlanBuy` steps that exist for them. |

Brief:
```
# desires-and-backhauls
Read the Java in plan 3.4 and sim/core/src/people.rs (comfort_par, comfort_held), rules/arrive.rs, rules/plan.rs,
atlas.rs, bargain.rs as landed. Add sim/core/src/desires.rs: will_pay(value, met) = value × (met < 1 ? 1 + (1 − met) ×
(2.0 − 1) : 0.5) capped by what households and spare money can pay; worth_buying(price, outlet) = price <= outlet.each ×
(1 − 0.15); haul_each(route, good) = the good's share of a stack's road food (2 × food × 0.05 / 27 / stack); outlet =
the best last price for the good among the Merchant's own routes' far ends (atlas Books). Change rules/arrive.rs::sell's
comfort branch: the far end pays by desire (will_pay) from money beyond its reserve (trading capital + wage float) and
within the Accountant's budget. PlanBuy: a town's own route (no sale load) carries a shopping purse = min(spare, Σ kinds
(comfort_par − held)) if >= max(0.5, 2 × food × 0.05); the far end's comfort wants = its last stocktake's gap plus
villagers × 0.1 × days since, loaded ×1.5 over (comfort_margin) from what home holds beyond its own par. Buy (far end):
buy_for_home per kind within the listed budget, cheapest items first, whole items (ceil), at bargain::between(0.5v, v,
levels), keeping road-food money; then buy_surplus: goods another route's far end wants (its par − held − aboard), up to
0.75 × value, never below the far end's 320 food points, into the wagon (post "backhaul"). Listings: a town lists each
comfort it lacks, or a good its lockers take and holds none of, at will_pay, only while it has a road for it and money for
a lot; other Merchants read listings at the far end into Books.last_price. Tests in tests/desires.rs with plan 3.4's
numbers, tagged (one partial). Random worlds: comforts and backhauls flow.
```

#### 3.5 `crates-and-papers`

| | |
|---|---|
| Goal | A shipment as a unit: goods crated with a bill of lading (consignee, contract, settled) and a copy, taking the cargo space of what's in it; the consignee's Merchant opens what's made out to it and the next wagon between the parties carries what's owed; a farm crates what falls due in lots, out of reach of spot sales; documents cost stationery or a scribe. |
| Java | `cargo/Crates.java` `pack` (160), `shipping` (211), `store` (226), `receive` (277), `consignedTo` (267), `stacks` (131), `takeEmpty`; `cargo/Papers.java` `billOfLading`; `cargo/Paperwork.java` `drawn` (68), `settle` (107), `book` (153); `caravan/ForwardSales.java` `earmark` (163–210); `caravan/Caravans.java` `pickContracted` (1371), `settleContracts` (1409: whole crates stored), `lots` (2335), `arriveAtDestination` 646–654 (crates fetched home), `settleOwing` (1568); `contract/ContractBook.java` `owe` (406), `owing` (423), `paidOwing` (415); `docs/decisions.md` "Crates and bills of lading", "Goods stay crated from shipper to buyer", "Earmarked in crates" |
| Covers | `CargoTests.aCrateTakesTheSpaceOfWhatsInIt`, `.documentsUseStationeryOrAScribe`; `CaravanTests.contractedGoodsShipInCratesWithTheirPapers`, `.whatAPlayerBroughtIsPaidForByTheNextCaravan`, `.aCaravanFetchesHomeCratesMadeOutToIt`, `.goodsForSaleTravelCratedAndAreStoredAsTheyCome` *(partial: one good per crate, stored crated; slots are Minecraft's)*; `MarketTests.aFarmCratesOwedCropInLotsNotAsItTrickles`, `.aFarmWithNoRoomForCratesDrawsNoPapers` *(partial: a room check stands in for free slots as a dial `crate_room`)*, `.aFarmCratesWhatItOwesOutOfReachUntilItShips`; `CountingHouseTests.aCountingHousesClerksDrawUpPapersCheaply` *(partial: the scribe's fee; the counting house's half is `buildings-and-works`)* |
| Rust | adds `cargo.rs` (`Shipment { good, count, consignee, contract: Option<usize>, settled: bool, booked: Vec<..> hook }`, `Shipments` component per town and `Wagon.shipments`; `stacks()`; `Paperwork`: per town documents drawn, paper and ink used, scribe fees), `rules/cargo.rs` (`PlanOwed`: contracted goods load as shipments with papers first; `Settle`: a shipment made out to the far end is received: delivered under its contract, or owed if unsettled; `Collect`: shipments made out to home are fetched; `Work`: a farm earmarks owed crop in lots); touches `state.rs` (`Wagon.shipments`, `Plan.shipments`), `content.rs` (dials `crate_emeralds 0.125`, `scribe_emeralds 0.1`, `ink_per_document 1/8`, `empties_kept 16`), `society.rs` (`deliver_shipment(town, shipment)` for a player's crate, `shipments(town)`, `paperwork(town)`) |
| Math | — |
| Accept | the Java numbers: a shipment of 3 stacks plus a loose stack = 4 stacks, an empty 1; 8 bills of lading use 8 paper and 1 ink with no fee, 8 more without paper pay the scribe; a DDP contract's 64 paper ride as a shipment naming the far end and the contract and settle on arrival; a player's unsettled crate of 16 flint at 0.25 → delivered 16, owing 4.0, collected by the next wagon; a settled shipment made out to the hub is fetched home; a harvest contract crates 40 at once, not 10 by 10, drawing one bill; no room → no papers; set-aside drops to 0 once crated and the crate reopens when the contract closes |
| Model | **sonnet / medium** — a record type and its hand-offs through steps that exist; the Java's item mechanics drop away. |

Brief:
```
# crates-and-papers
Read the Java in plan 3.5 and sim/core/src/contracts.rs, rules/contracts.rs, rules/advances.rs, harvest.rs, rules/plan.rs,
rules/arrive.rs as landed. Add sim/core/src/cargo.rs: `Shipment { good, count, shipper, consignee, contract:
Option<usize>, settled: bool, copy: bool }` with stacks() = max(1, ceil(count/stack)); `Shipments` component (a town's
crated goods, out of Stock's reach: lockers, workshop and spot sales see only Stock) and `Wagon.shipments`; packing a
new shipment costs crate_emeralds (post "crates") and draws a bill of lading (and a contract copy): `Paperwork` per
town counts documents and uses 1 paper and 1/8 ink each from Stock, or pays scribe_emeralds a document when out of
paper. PlanOwed: contracted goods the seller carries go as shipments (earmarked ones first, then loose goods packed),
one good a shipment, before any sale load; the Decide trim respects them. Settle: a shipment consigned to the far end is
received: its contract copy delivered, its goods delivered under the contract (whole shipment kept crated in Shipments,
not Stock, until used: expose `take(town, good, n)` that empties shipments first); an unsettled shipment with no wagon
to pay (a player's: Society::deliver_shipment) books `Book::owe(contract, counted × price)` and the next wagon between the
two carries the money (Papers step: settle_owing both ways). Collect: a settled shipment made out to home is fetched.
Work: earmark — for each harvest contract the town sells, want = owed by the end of the next window − shipped −
earmarked; lot = min(want, 64 × stack); crate it only when Stock holds the whole lot and there's room (dial crate_room:
shipments ≤ room); a shipment whose contract is no longer in force goes back to Stock. Tests in tests/cargo.rs with plan
3.5's numbers, tagged (three partial). Random worlds: contracts ship as shipments.
```

#### 3.6 `freight-hubs`

| | |
|---|---|
| Goal | Hubs that emerge where routes pass, carriers selling space by the stack (backhaul at a quarter), forwarders taking crates bound on for a fee and booking them on the soonest run, blocks of space bought and paid per run, drayage by idle hands, stops on the way, and routes reorganising by landed price. |
| Java | `market/Freight.java` `Leg` (123), `legs`/`legsFrom` (215), `isHub` (210), `stackPrice` (273), `legPrice` (285), `forwardingPrice` (294), `handOver` (741), `book` (331), `booked` (315), `forward` (870), `blockRoom` (565), `hasBlock` (575), `buyBlocks` (591), `runBlock` (619), `loadBooked` (962), `nextRun`, `runCost` (437); `caravan/Caravans.java` `atHub` (789), `callAtStop` (1038), the hub-emergence and reorganising rules (search "storehouse", "wind"); `docs/decisions.md` "Freight: carriers, forwarders, drayage", "Mountains, and hubs that emerge", "Routes reorganise by price; backhauls; llamas", "Tolls a caravan, and the network reorganising", "The ridge study made honest" |
| Covers | `CaravanTests.aForwarderTakesCratesBoundOnAndTheirRunLoadsThemFirst`, `CaravanTests.theForwarderBuysABlockOfSpaceItFillsAndPaysForEachRun` |
| Rust | adds `freight.rs` (`Hub` component: storehouse, legs, blocks, draymen; `Leg`, `stack_price`, `leg_price`, `forwarding_price`, `Booking`), `rules/freight.rs` (`Review`: a market becomes a hub when ≥ 3 routes pass or end on its land (or 2 pass), it can feed callers and pay 4 emeralds; a hub opens runs to far ends of routes calling, one a day; a long run's owner compares its road with the hub's legs and winds it down at ≤ 3/4; `Freight` step: at a hub, a wagon hands over shipments bound for markets the hub serves for fee = stacks × forwarding + drayage × 1.2, the forwarder books them on the soonest run (blocks first), booked shipments load first, blocks bought and paid per run; stops on the way: a route whose path crosses a hub's land calls there out and home); touches `state.rs` (`Route.stops: Vec<(Entity, u64)>`, `Wagon.at_stop`), `content.rs` (dials `freight_margin 0.25`, `freight_share 0.5`, `forward_margin 0.2`, `backhaul_share 0.25`, `block_runs 5`, `block_price 0.8`, `dray_stacks 16`, `dray_wage 0.5`, `hub_routes 3`, `storehouse_emeralds 4`, `stop_fee_share 0.25`), `society.rs` (`make_hub(town)`, `hub(town)`, `book(shipment, leg, stacks)`) |
| Math | **min-cost network flow** stays "maybe": add a yardstick test comparing a hub's routing of three towns' shipments with the direct roads' cost, and note it |
| Accept | the Java numbers: with no draymen `hand_over` takes nothing; a shipment given one ticket loads first and stays consigned to its buyer; a first `forward` with no block leaves the shipment unbooked and remembers the demand, `buy_blocks` makes a block on the carrier's home leg at min(27 × 0.5, wanted) stacks × 5 runs at 0.8 × leg price (home leg at a quarter), the next `forward` books it, `run_block` pays > 0 and counts a run down; on a three-town map with a pass town, routes crossing it make it a hub and it opens its own runs; conservation holds across fees, fares and drayage |
| Model | **opus / high** — the most entangled slice in the mod (Freight.java is 1,000 lines over Caravans' stops), with emergent behaviour pinned only by two tests and the decisions doc's measurements; needs judgment on what to carry over. |

Brief:
```
# freight-hubs
Read docs/decisions.md's five freight sections first, then the Java in plan 3.6 (Freight.java's named methods;
Caravans.atHub and callAtStop), then sim/core/src/cargo.rs, atlas.rs, territory.rs, hex.rs/map.rs, rules/plan.rs,
rules/arrive.rs as landed. Add sim/core/src/freight.rs and rules/freight.rs. A `Hub` component: storehouse (bool),
legs (every route touching the hub: its own runs out, and other markets' runs calling and going home), blocks
(carrier leg → Block { stacks, runs left, price, wanted }), draymen = idle hands + 1 (16 stacks a day each at 0.5
emerald). Prices: stack_price(leg) = the carrier's road food over its last wagon's capacity × (1 + 0.25); leg_price =
stack_price × 0.25 on a home leg; forwarding_price(leg) adds the hub's margin. Hubs emerge (Review): a market with a
Merchant hand (or no roster), routes passing through its land or ending there (≥ 3, or 2 passing), food to feed
callers and 4 emeralds builds a storehouse and becomes a hub. Stops (Plan/Arrive): a route whose path crosses a hub's
land calls there out and home: arrival set to that far along the road, the rest after; at the stop a wagon buys food
for the road home if the far end may not feed it (road-food's rule), hands the forwarder shipments bound for markets
the hub serves (fee = stacks × forwarding_price + carting 2 × stacks/4 carts × 0.5 × 1.2, paid from the purse; no
draymen free → the shipment waits aboard), loads shipments booked on its run (first), carries the hub's contract copies
on. Forwarder: books each handed-over shipment on the soonest leg toward its consignee: a block with room first, else
tickets at stack_price; unbooked demand is remembered and buy_blocks (daily) buys a block of min(27 × 0.5, ceil(wanted))
stacks × 5 runs at 0.8 × leg_price on the leg with demand; run_block pays stacks × price per run and counts down,
resizing to a smoothed average (weight 0.5). Reorganising (Review): a hub opens a run of its own to a far end of an
established route calling there (5+ trips), one a day; the owner of a long run calling at a hub that runs on compares its
road food with a short run to the hub plus freight for its usual load (carted, with margin) and winds the long run down
at <= 3/4, reasons recorded in Whys. Stop fee: a hub charges each caller a quarter of the share of detour, 0.25 base
(post "stop fee"). Tests in tests/freight.rs: the two CaravanTests with plan 3.6's numbers, tagged; a three-town map with
a pass town that becomes a hub and opens runs; a yardstick comparing hub routing cost with direct roads (note the
min-cost-flow row). Random worlds: some maps have a hub.
```

#### 3.7 `sourcing-and-procurement`

| | |
|---|---|
| Goal | A town short of a good contracts a supplier (nearest known holder with a road: Ex Works if its own wagons go there, else DAP; worth + 10%, carriage and tolls by the terms; half its kit kept; trial lots below trust), its Merchant buys from its own lockers what contracts want within two days, a contract's inputs raise the par so wagons bring them, and the capital's metal exchange buys nearest holder first at a premium that rises as it runs short. |
| Java | `market/Broker.java` `source` (287–349), `means` (379–432), `landed` (450), `coming` (460), `makerLot`, `TRIAL_LOT`, `SUPPLIER_MARGIN`; `market/Procurement.java` `wanted` (74), `soon` (101), `procureAt`/`procure` (123–129), `PACE_TICKS`; `contract/ContractBook.java` `toDeliver` (226), `review` 301 (inputs via `Warehouse.setInputs`); `service/Crafts.java` `contractInputs` (247); `merchant/Warehouse.java` `par` (138: at least the inputs), `setInputs`; `market/MetalExchange.java` `premium` (160), `target` (147), `METALS` (52), `metal` (142); `caravan/Caravans.java` `wantsAt` (2267: inputs first), `collectPressing` 1273 (the inputs clause); `locker/LockerTrading.java` `trade` 182–235 (procured runs capped by `soon`); `docs/decisions.md` "Supply chains, back to back", "Inputs follow contracts", "Pacing", "The metal exchange buys by contract", "Trust never freezes trade" |
| Covers | `ContractTests.aTownShortOfGoodsSourcesThemFromASupplier`, `.belowTrustTheyStillDealOnTrialTerms`; `LogisticsTests.aMerchantBuysForWhatsDueSoonNotTheWholeContract`, `.theExchangeBuysMetalFromTheNearestHolderFirst`; `LockerTradingTests.atItsRestockTheMerchantBuysWhatItsContractsWantOfAVillager`; completes `CaravanTests.theAdvanceRidesWithTheCopyAndTheInputsFollow` (upgrade `advances-and-refunds`' partial to full) |
| Rust | adds `sourcing.rs` (`source(town, good, count)`, `means`, `landed`; `Procurement::wanted/soon`; `MetalExchange` resource: targets per metal × phase, `premium`), `rules/sourcing.rs` (`Review`: a town whose contracts or orders want goods it can't make sources them; the capital's exchange sources metal; `Work` before lockers: switch on selling offers the town's contracts want and cap their runs at what's due within two days; inputs of contracts to make raise `Trades.capacity`/par); touches `content.rs` (dials `supplier_margin 0.1`, `trial_lot 4`, `pace_days 2`, `maker_lot 16`, `maker_lead_days 3`, `maker_installments 4`; `Content.metals` by name with targets 192/192/96/12/32), `state.rs` (`Trades.inputs: Vec<f64>`), `society.rs` (`source(town, good, n)`, `exchange()`) |
| Math | — |
| Accept | the Java numbers: 5 pickaxes wanted, the supplier holds 6 → a contract for 3 (half its kit kept), EXW since the buyer's wagons go there, DRAFTED; trust 3 kept/12 missed → still a contract, for 4, no advance; a contract of 10 swords one a day → `soon` 3, `wanted` 10; a master smith's sword offer switched on at restock and at least one bought; holding 0 of 192 → premium 1.5, a need of 100 → 40 from the near mine and 60 from overseas, at target → 1; a town contracted for swords has iron in its par and the next wagon brings it |
| Model | **sonnet / high** — several small rules across Broker, Procurement and MetalExchange with exact numbers; the hook into `lockers` (switching offers on, capping runs) needs a careful read of landed code. |

Brief:
```
# sourcing-and-procurement
Read the Java in plan 3.7 and sim/core/src/contracts.rs, lockers.rs, atlas.rs, services.rs (Crafts inputs), rules/
advances.rs as landed. Add sim/core/src/sourcing.rs. source(town, good, count): the nearest known market (atlas) whose
last stocktake holds the good and has a road between them; means = held − par − food reserve, half its kit for kit
goods; terms EXW if the buyer's wagons go there else DAP; price = worth × 1.1 then carriage and tolls by Incoterm::price;
one open contract per good; below MIN_TRUST a trial lot of 4, one installment, no advance; with trust an advance of a
third for goods to make (lots of 16, 4 installments a day apart after a 3-day lead); with no road the want stays a
signal (Signals resource: fill jobs' hook). Procurement: wanted(town) from Book::to_deliver (what the town's contracts
still owe), soon = owed(now + pace_days); in Work before lockers run: switch on a locker's selling offers for goods the
town's contracts want and cap their runs at soon; inputs: for each contract to make (Crafts recipe for the good), its
inputs raise Trades.inputs, and par/shelf is at least the inputs, so PlanSale carries them (inputs first, a stack each
when slots are short) and a wagon carrying inputs is pressing. MetalExchange (a resource on the capital when `crown`
names one; until then Society::open_exchange(town)): metals iron 192, coal 192, flint 96, diamond 12, gold 32 (×2
mobilising, ×3 at war: read a Phase resource if present, default peace); premium = clamp(1 + 0.5 × gap, 0.75, 1.5);
daily it sources the shortfall nearest holder first (a town with a mine, then the overseas market off the map if
realms has landed; else only towns). Tests in tests/sourcing.rs with plan 3.7's numbers, tagged; upgrade
tests/advances.rs's partial tag on theAdvanceRidesWithTheCopyAndTheInputsFollow to full with an inputs assertion.
Random worlds: towns source what they lack.
```

#### 3.8 `war-army`

| | |
|---|---|
| Goal | The army as a population at the front: recruits muster and enlist; the quartermaster wants kit sets, arrows and rations for round trips; soldiers eat 8 points a day, heartiest food first; risk by kit (weapons, armour points, shields, enchantments) and hunger; the fallen are gone and their town grieves; the hungry desert. |
| Java | `army/Army.java` `enlist` (287), `muster` (273), `embark` (279), `wants` (306), `rationDays` (328), `foodPoints`, `heartiestFirst` (341), `mess` (218), `heartiness` (81, 87), `draw`, `risk` (351–366), `armourRisk` (128), `points` (367), `campaign` (419–545), `demobilise`, the `WEAPONS`/`ARMOUR` tables (95–125), `MEATS`; `territory/Territory.java` `mourn` (and the grief term in `moodTarget` ~1593); `docs/decisions.md` "War footing" (campaigning, the fallen, an army eats), "Feeding the front", "Mercenaries, meat" (heartiness), "Armour comes in sets" |
| Covers | `WarTests.anArmyEatsFromTheDepot`, `.theFrontKeepsRationsForTheRoad`, `.theArmyEatsItsHeartiestFoodFirst`, `.soldiersFallLessInBetterKit`, `.theFallenAreGoneAndTheirTownGrieves`; `LogisticsTests.soldiersAreSafestInWholeSets` |
| Rust | adds `army.rs` (`Army` resource: soldiers by home town with kit worn, waiting recruits, tally; `Kit` tables by good name: weapon power/risk/days, armour points/days; `heartiness(good)` from `Good.food` and a `saturation` field; `risk`, `wants`, `ration_days`, `mess`), `rules/army.rs` (`Pay`/daily: `campaign` on the front's stock); touches `content.rs` (`Good.saturation`, `Good.meat: bool`; dials `death_base 0.02`, `ration 8`, `hungry_risk 1.5`, `desertion 0.1`, `ration_spare_days 2`, `arrows_per_day 8`), `state.rs` (`People.grief: Vec<(u64, f64)>` → mood −0.5 a fallen soldier for 10 days, at most 2), `society.rs` (`enlist(town, n)`, `army()`, `set_front(town)`) |
| Math | **Lanchester / attrition as a rate**: the day's fallen and deserted are expected values (soldiers × risk) carried as fractions, not per-head rolls, so the same inputs give the same losses; tick the row |
| Accept | the Java numbers: 5 soldiers and 64 bread: the first campaign day arms the clock, the second eats ≥ 40 points (8 loaves), 56 left; `ration_days(1500 blocks) = 11 > ration_days(0) = 5`, two recruits waiting → bread × 5 ≥ 2 × 8 × 11 wanted; 16 bread + 1 cooked beef, `mess(8)` eats the beef and leaves all 16 bread; heartiness(meat) > 0, bread 0, (1.2) = 0; risk bare 0.064 > iron sword + chestplate + shield ≈ 0.0216 > diamond ≈ 0.0158 > diamond with Sharpness V ≈ 0.0118; chestplate alone > iron set > diamond set = 0.5 × 0.02, `wants` includes iron boots; 20 unarmed and unfed over 30 days: some fall, size = 20 − fallen − deserted, hungry days > 0, and their town's mood target carries the grief |
| Model | **opus / high** — the combat model: translating per-head dice into deterministic attrition while matching the pinned risk numbers to three decimals, and designing the front as a population the crown supplies. |

Brief:
```
# war-army
Read docs/decisions.md "War footing", "Feeding the front", "Mercenaries, meat, and the front's supply", "Armour comes
in sets" and the Java in plan 3.8 (Army.java's named methods and tables); then sim/core/src/roster.rs, kit.rs,
people.rs as landed. Add sim/core/src/army.rs and rules/army.rs. Kit by good name: weapons (power, risk, days):
diamond sword 7/0.8/12, iron 6/1.0/10, stone 5/1.25/6, wooden 4/1.4/4, diamond axe 6/0.95/12, iron axe 5/1.15/10, stone
axe 4/1.35/6, bow 6/1.1/10; armour points (helmet/chest/legs/boots): diamond 3/8/6/3, iron 2/6/5/2, chain 2/5/4/1,
leather 1/3/2/1, lasting 30/20/16/10 days; shield risk 0.85 / 12 days. risk(soldier) = 0.02 × (weapon ? its risk : 2.0)
× max(0.6, 1 − 0.05 × sharpness) × armour_risk(points: 1.6 unarmoured down to 0.5 at 20 points, linear) × max(0.5, 1 −
0.06 × protection) × (shield ? 0.85 : 1); hungry × 1.5; hearty meals × (1 − 0.15 × heartiness). heartiness(good) =
clamp((saturation per food point − 1.2)/..., 0, 1) as the Java, any meat at least 1/3. `Army` resource: soldiers (home
town, kit worn with days left, hungry days), waiting recruits at the capital, fallen/deserted/soldier-days tally per
town. wants(ration_days) = an iron set each, shields for half, arrows × 2 days, food = mouths × 8 × ration_days;
ration_days(blocks) = 3 × ceil(2 × travel/24000) + 2. Daily (Pay step, when a front town is set): mess — eat mouths × 8
points from the front's Stock, heartiest first; fed share = eaten/needed; desert the hungry at 0.1 × (1 − 0.5 × hearty)
× hungry soldiers (expected value, carried as a fraction, applied to whole soldiers when it reaches 1); draw kit the
best there is for what each lacks, wear it by a day (×1/(1+unbreaking)); fall = Σ risk × (hungry ? 1.5 : 1) as an
expected count carried as a fraction (Lanchester-style attrition; document why no per-head roll); the fallen come off
their town's roster (away hands) and their town grieves: People.grief entries (−0.5 each for 10 days, at most −2 on the
mood target: hook into people.rs's mood target); pay 0.25 a soldier-day owed to the home town (Book::owe hook, or a
`Pay` ledger the crown settles). enlist/muster/demobilise as host calls. Tests in tests/army.rs with plan 3.8's numbers
(risk to three decimals), tagged. Random worlds: a few worlds raise an army at a front town. Tick Lanchester in
docs/core-math.md.
```

#### 3.9 `bank-and-discounts`

| | |
|---|---|
| Goal | A bank at a market deals through its own wagons: discounting a town's bills not yet due for coin now (1% and 1% a day), redeeming its own, endorsing others and carrying them home to present; lending against a contract in force up to half the goods to come at 3%, repaid first from the payment; the discount desk's pricing by goods backing and its book's caps; a bill nobody can carry lapses after 12 days. |
| Java | `market/Bank.java` `deal` (195), `discount` (209), `lend` (257), `repayFrom` (177), `collect` (292), `owedOn` (169), `FLOAT`, `LOAN_SHARE`, `LOAN_INTEREST`; `market/Discounts.java` `quote` (95–122), `limit` (75), `book` (52), `onDrawee` (58), `backed` (80), `isForeign` (69); `contract/Bills.java` `lapse` (193), `endorse` (166), `redeem` (172), `grace` (the debt-scaled grace), `HONOURED_WEIGHT`; `caravan/Caravans.java` `dispatch` 344 (`bankFloat`), `collectExWorks` 1506 (repay first), `presentBills` (1752); `docs/decisions.md` "Pacing and the crown's bank", "Bills of exchange", "Grace by the size of the debt" |
| Covers | `CaravanTests.aBankBuysATownsBillForCoinNow`, `.aBankLendsAgainstAContractAndIsRepaidFromThePayment`; `CountingHouseTests.theDiscountDeskPricesByGoodsAndCapsItsBook`; `ContractTests.aBillNoOneCanCarryLapses` |
| Rust | adds `bank.rs` (`Bank` component: loans by contract, bills bought; `discount_price`, `lend`, `repay_from`; `Discounts`: `quote`, `limit`, `book`, `backed`), `rules/bank.rs` (`PlanPapers`: a bank's wagon carries a float of 48 more; `Papers` at the far end: deal — buy the town's bills not due at face × (1 − 0.01 − 0.01 × days), redeem bills on itself, endorse others home; lend where a town's spare < half the goods to come; `Settle`/`Collect`: repay from the payment first; `Judge`: a bill with no route either way and no hub to buy it lapses 12 days past due, the drawee's miss); touches `bills.rs` (`endorse`, `redeem`, `lapse`, `grace` scaled by debt: 8 days + a day per 100 emeralds owed the same holder), `content.rs` (dials `discount_fee 0.01`, `discount_per_day 0.01`, `loan_share 0.5`, `loan_interest 0.03`, `bank_float 48`, `bill_grace_days 8`, `bill_grace_emeralds_per_day 100`, `discount_book_share 1.0`, `discount_drawee_share 0.35`, `discount_unbacked 0.04`, `discount_throttle 0.03`, `lapse_days 12`), `society.rs` (`open_bank(town)`, `bank(town)`, `quote(bank, bill)`) |
| Math | — |
| Accept | the Java numbers: a bill of 10 × 1.02 due in 4 days bought back by its own drawee's bank for about 9.69, PAID, `bills_paid < 10.2`; a town with no money selling 64 books at 0.5 EXW is lent more than 5 and on collection owes about 0 with more than 5 repaid; with means 100 a bill taking a drawee past 35% of the book is refused, backed is cheaper than unbacked by > 0.039, a full warehouse costs ~0.03 more, 65 taken means full; a bill with no route either way is OUTSTANDING at due + 12 days − 1 and DISHONOURED a tick later |
| Model | **sonnet / high** — extends `bills.rs` with exact pricing rules; the Bank's hooks into Settle/Collect need a careful read of landed advances and cargo code. |

Brief:
```
# bank-and-discounts
Read docs/decisions.md "Pacing and the crown's bank", "Bills of exchange", "Stranded paper" (the grace paragraph) and
the Java in plan 3.9; then sim/core/src/bills.rs, rules/bills.rs, rules/advances.rs, rules/contracts.rs as landed. Add
sim/core/src/bank.rs and rules/bank.rs. A `Bank` component on a market (Society::open_bank): it deals only where its
own wagons call, from its treasury, carrying bank_float 48 more in the purse. Papers step at the far end: discount —
buy the town's bills not yet due for face × (1 − 0.01 − 0.01 × days to due) (post "discount"); a bill on the bank's
own market is redeemed; others are endorsed to the bank (Bills::endorse: payee changes) and carried home to present;
lend — a town with a contract in force and spare money < 0.5 × worth of goods still to come is lent up to that half
less advances and loans already, at 3% (owed = principal × 1.03); Settle/Collect — when the bank's wagon pays for
those goods the loan comes out first (repay_from); Bank::collect takes what's left of a loan on a done or broken
contract from what the town can spare. Discounts desk (Discounts): quote(bill) refuses when the book (bills held, as a
share discount_book_share of means) is full or a drawee would pass 0.35 of it (0.5 foreign); price = face less fee,
less 0.04 unbacked (no goods on the bill's shipment), less 0.03 throttle when the warehouse is full, less 0.05 past 12
days. Bills: grace = 8 days from first presenting + a day per 100 emeralds the drawee owes the same holder; lapse —
a bill whose payee has no route to the drawee either way (and no hub that buys it: hook for stranded-paper) is
dishonoured 12 days past due, the drawee's miss. Tests in tests/bank.rs with plan 3.9's numbers, tagged. Random
worlds: one town in some worlds opens a bank.
```

---

### Wave 4: the crown, the post, players and the rest (on wave 3)

#### 4.1 `post`

| | |
|---|---|
| Goal | Letters: a copy of a contract or a bill goes by post on the post day when no wagon runs between the parties, at a speed and for postage; the drawee honours a mailed bill and remits coin by post; a hub books a call a day where it pays and declines where it doesn't; a town's bookings filling a wagon bring the forwarder to collect; goods left on consignment sell and the money comes back by post. |
| Java | `market/Post.java` `arrival` (82), `post` (90), `atDesk` (100), `read` (112), `write` (133), `mailCopies` (143), `mailBills` (320), `honour` (354), `remit` (179), `callIn` (241–282), `payAid` (285), `due` (299), `bookCall` (189), `tripCost`; `market/Forwarder.java` `booked` (170), `review` (186–208), `collecting` (211), `consignUnsold` (264–316), `sold` (319), `remit` (345–383); `market/Freight.java` `runCost` (437); `docs/decisions.md` search "post" in the freight and stranded sections |
| Covers | `PostTests.aLetterLeavesOnThePostDayAndWaitsAtTheDesk`, `.aHubBooksACallOneADayAndDeclinesOneThatDoesntPay`, `.aBillGoesByPostAndTheCoinComesBack`, `.aTownsBookingsFillingAWagonBringTheForwarderToCollect`, `.goodsOnConsignmentSellAndTheMoneyComesBackByPost`, `.whatATownsWagonCantSellAtTheHubIsLeftOnConsignment`; `CaravanTests.aContractsCopyGoesByPost` |
| Rust | adds `post.rs` (`Letter { kind: Copy/Bill/Remit, from, to, leaves, arrives, payload }`, `Mail` resource; `arrival(now, blocks)`), `rules/post.rs` (`Mail` step: letters arrived are read: copies delivered, bills presented and honoured from money beyond the reserve (borrowing deposits), remits deposited; `Review`: once a day each town writes: copies a day after drafting where the other party doesn't know and no wagon runs, bills due where no wagon runs; a hub's calls; the forwarder's collections and consignment); touches `content.rs` (dials `post_days 3`, `post_speed 2000`, `postage 0.1`, `call_margin 0.25`, `call_stop 1.0`, `calls_per_day 1`, `crown_aid 0.5`, `collect_stacks 8`, `collect_days 6`, `consign_commission 0.1`), `state.rs` (`Route.on_call: bool`), `society.rs` (`mail()`, `book_call(town, hub, value)`) |
| Math | — |
| Accept | the Java numbers: postage 0.1 (5 → 4 with change); 4,000 blocks at 2,000 a day leaves on the next post day and has arrived by t + 9 days; a copy drafted at one desk alone is in force at the far end after it reads its mail at t + 12 days with no wagon sent; calls of 200, 200 and 1 from 400 blocks: two booked on different days, the 1 declined, a route on call to the booked town and `due` only on its day; a 5.1 bill mailed, PAID when read, the holder's money up by more than 4 when the remit arrives; a 2-stack booking waits, 12 stacks are collected, the 2 after 7 days; a lot sold for 100 → 10 commission, 5 fare, 85 remitted; 64 leather left on consignment at a hub whose par is 160 later sells and 144 goes home |
| Model | **sonnet / high** — six tests with exact numbers over two Java files; the hub-and-forwarder half depends on reading `freight-hubs`' landed code. |

Brief:
```
# post
Read the Java in plan 4.1 and sim/core/src/contracts.rs, bills.rs, freight.rs, cargo.rs, rules/bank.rs as landed. Add
sim/core/src/post.rs and rules/post.rs (a `Mail` step exists after Arrive). arrival(now, blocks) = the next multiple of
post_days × day_ticks, plus max(2400, blocks/post_speed × day_ticks); posting costs postage (post "postage"). Daily
(Review), each town writes: a Copy for each contract it knows that the other party doesn't, a day after drafting, when
no wagon runs between them and none is pending; a Bill for each bill it holds that's due, when no wagon runs either
way. Mail step: a Copy read puts the contract in force (Book::deliver_copy); a Bill read is honoured from money beyond
the reserve (borrowing the households' deposits if farms' hook exists), PAID, and a Remit letter carries the coin back
to the holder (post "remit" on arrival); unpaid, it waits its grace. Calls: Post::call_in(town, hub, value) — a hub
books a call (a route on_call from the hub to the town) where value × call_margin >= call_stop × (trip cost − crown aid:
0.5 on the first visit if a crown exists, else 0), trip cost = ceil(blocks/10) × 2 × 2 × 0.05; one call a day, later
calls on later days; `due(route)` true only on its day. Forwarder: a town's bookings (shipments for markets the hub
serves, fare prepaid at run cost/27 × 1.25, × 1.5 on call) are collected when they reach 8 stacks or wait 6 days; what
a town's wagon can't sell at the hub is left on consignment if the hub's and its runs' pars have room; sold, 10%
commission and the fare come off and the rest is remitted by post when it's at least an emerald or the lot is done.
Tests in tests/post.rs with plan 4.1's numbers, tagged. Random worlds: copies and bills go by post where no wagon runs.
```

#### 4.2 `training-and-tenders`

| | |
|---|---|
| Goal | Training as a contract: a term paid when the census shows a new master (a third up front), judged on the roster; the Merchant trains only under one, switching on what the pupil buys and buying its work when there's somewhere for it to go (an offtake crates it for the buyer with a bill); tenders posted on a board, weighed by a town (price against a stall, coal and iron per level, time), taken or carried on for 15% less. |
| Java | `contract/Training.java` `forSeller`; `contract/ContractBook.java` `reviewTraining` (449–479), `trained` (496), `at` (501), `noteOfftake`; `market/Apprentices.java` `target` (40), `pupil` (146), `review` (156), `afterTrade` (78), `collectable` (125), `neededTrades` (51); `market/Tenders.java` `worth` (161–192), `read` (142), `consider` (195–225), `take` (229), `terms` (248), `post` (103), `on` (119), `lapse`, `RELAY_MARGIN`, `LEVEL_COST`, `DAYS_PER_LEVEL`; `docs/decisions.md` "Training is a contract", "The Merchant trains only under contract", "The Merchant trains smarter", "Why training broke" |
| Covers | `LogisticsTests.aTrainingContractIsPaidWhenTheCensusShowsTheMaster`, `.aMerchantTakesATenderWorthItsWhile`, `.aMerchantWithNoOneToTrainCarriesTheTenderOn`; `LockerTradingTests.theMerchantTrainsASmithOnlyUnderATrainingContract`, `.underAnOfftakeThePupilsWorkIsCratedForTheBuyerWithABill` |
| Rust | adds `training.rs` (`Training { trade, level, bonus, offtake, already }` term on `Contract`; `Tender`, `Board` resource per town; `worth`), `rules/training.rs` (`Judge`: a training contract completes when the seller's roster shows `already + 1` hands of the trade at the level, paying the rest of the bonus (less the advance); `Work` before lockers: the pupil (the trade's hand furthest along) has its buying offers switched on and its cheapest work bought when the warehouse has none of what it buys, within advances held and the budget, only when the work has somewhere to go; under an offtake the work is crated for the buyer with a bill drawn on it (where a wagon of the buyer's calls); `Review`: tenders read off boards carried by wagons: take if worth > 0 (bonus − 2 emeralds a level − a stall if the trade is lacking, with 2 days a level + 2 of supply within the due), else carry on at 0.85 of the bonus and 2 days sooner; a tender nobody takes lapses and is reposted at +25%); touches `contracts.rs` (`Contract.training`), `lockers.rs` (`needed` filled), `jobs.rs` hook (a contract's trade first), `content.rs` (dials `training_bonus 24`, `relay_margin 0.15`, `level_cost 2`, `days_per_level 2`, `supply_days 2`), `society.rs` (`tender(town, trade, level, bonus, due)`, `board(town)`) |
| Math | — |
| Accept | the Java numbers: bonus 24 with a third up front → advance 8; in force while the smith is a Journeyman, COMPLETED with 16 owing once the roster shows level 5; `already = 1` needs another master; a level-2 weaponsmith to 5 = 3 levels, 6 emeralds, 8 days: a tender of 3 is worth < 0 and not taken, one due in 3 days impossible, 40 in 20 days taken and known only by the town; a hub with no smith but a road carries it on at < 40 and 2 days sooner; without a contract the coal offer stays off; under one it's on and flagged, the axe bought as training, xp gained, all off at level 5; under an offtake 12 axes are crated for the buyer and a bill of ≥ 4 drawn |
| Model | **sonnet / medium** — well-pinned rules on top of landed advances, lockers and cargo; the Java is clear about every number. |

Brief:
```
# training-and-tenders
Read docs/decisions.md's training sections (plan 4.2) and the Java named there; then sim/core/src/contracts.rs, rules/
advances.rs, lockers.rs, cargo.rs, bills.rs, jobs.rs as landed. Add a `Training { trade, level, bonus, offtake: f64,
already: u32 }` term to Contract (price 0, quantity 0; worth = bonus for the advance). Judge: when the seller's roster
shows more than `already` hands of the trade at the level, the contract completes and the buyer owes bonus − advanced
(Book::owe, carried by the next wagon). Apprentices (Work, before lockers run): for each training contract a town
sells, its pupil is the trade's hand furthest along: switch on its buying offers (flag them training; lockers' `needed`
lists the trade so the Merchant manages it first; jobs takes the trade up if the town lacks it); when the warehouse has
none of what it buys, buy its cheapest work (a selling offer) as a lesson while advances held + the Accountant's budget
allow, only if the work has somewhere to go: an offtake, or the town wants it, or room (dial); at the level, all off.
Offtake: the pupil's paid work is crated at once as a Shipment made out to the buyer, settled: true, and a bill drawn on
the buyer for its worth × offtake, where a wagon of the buyer's calls or a hub runs to it. Tenders: a `Board` per town
(tenders: trade, level, bonus, due, posted by, relay of); wagons carry boards both ways (Arrive). Review: a town reading
a tender weighs it: worth = bonus − 2 × levels to go − (stall 6 if it lacks the trade) if time allows (2 days a level
+ 2 within due) else nothing; worth > 0 → take: draft the training contract (an advance of a third where trusted); else,
with roads to towns of its own, carry it on: repost on its own board at 0.85 × bonus and due − 2 days, its own contract
with the first buyer done when the town's is; a tender nobody takes lapses at due and is reposted at +25%. Tests in
tests/training.rs with plan 4.2's numbers, tagged. Random worlds: a few tenders.
```

#### 4.3 `player-orders`

| | |
|---|---|
| Goal | How a player's shop reaches the society: a buying shop is a standing order a master Merchant fills from spare stock at the shop's price (if at least 90% of worth), as an Ex Works contract of as many daily lots as the purse pays for, up to 10; what it can't fill is an open order the region works on (wagons bound there bring it; signals post what no market has) and the town's own wagons buy it up as surplus; a player's selling stall is supply the Merchant buys for its orders at up to worth; the town's own shop sells its biggest surplus in lots of 16 at 10% over. |
| Java | `market/Broker.java` `broker` (99), `contractFor` (172), `deliver` (197), `fill` (226), `spare` (153), `buy` (481), `want` (473), `signals` (525), `sellSurplus` (576), `pay` (832), `MASTER`, `AGENT_FLOOR`, `MAKER_CEILING`, `OWN_MARKUP`, `OWN_LOT`, `SHOP_INSTALLMENTS`; `market/Exchange.java` `wanted` (124), `setOrders`, `Tally`; `market/Shops.java` `own` (59), `lots`, `offer`; `caravan/Caravans.java` `wantsThere` (2324); `docs/decisions.md` "The master Merchant and players' shops" |
| Covers | `MarketTests.aMasterMerchantFillsAPlayersOrder`, `.whatItCantFillIsAskedOfTheRegion`, `.itBuysFromAPlayersStallForItsOrders`, `.itsOwnShopSellsTheSurplus`; `CaravanTests.aShopsOrderAtTheFarEndRidesOut` |
| Rust | adds `orders.rs` (`Order { town, good, lot, price, purse, open }`, `Stall { town, good, lot, price, stock }`, `Signals` resource (what the region wants that no market has, with the premium)), `rules/orders.rs` (`Review` each look, a master Merchant: fill orders from spare stock (contract EXW, lots a day), buy from stalls for orders and lockers' wants, the town's own shop; `PlanSale`: open orders at the far end count as wants); `messages.rs` gains `Input::Order`, `Input::Stall`, `Output::OrderFilled`, `Output::Signal`; touches `content.rs` (dials `agent_floor 0.9`, `maker_ceiling 1.0`, `signal_premium 1.5`, `own_markup 1.1`, `own_lot 16`, `shop_installments 10`), `society.rs` (`order`, `stall`, `orders(town)`, `signals()`), `atlas.rs` hook (signals ride with wagons) |
| Math | — |
| Accept | the Java numbers: an order of 16 paper at 16 spurs a lot with a purse of 40 → a contract of 2 installments EXW, 16 today and 32 by tomorrow, the purse down to 8, 32 paper left, COMPLETED; an order for 2 nautilus shells × 10 lots → 20 wanted and a signal (no market has them); a stall selling 4 iron for 12 against an order of 4 for 24 → both stall lots bought, filled a lot a day, ≥ 24 profit; the town's own shop lists 64 sticks and banks a 9-spur sale; an order at the far end for apples puts apples in the next wagon's load; a Merchant below level 5 does none of it |
| Model | **sonnet / medium** — the player-facing surface, pinned by five tests with exact numbers; the only design is the two new `Input`s. |

Brief:
```
# player-orders
Read docs/decisions.md "The master Merchant and players' shops" and the Java in plan 4.3; then sim/core/src/messages.rs,
contracts.rs, lockers.rs, atlas.rs, rules/plan.rs as landed. Add `Input::Order { town, good, lot, price, purse }` (a
player's buying shop) and `Input::Stall { town, good, lot, price, stock }` (a selling shop) to messages.rs with
`Society::order`/`stall`, and `Output::OrderFilled`, `Output::Signal`. Add sim/core/src/orders.rs and rules/orders.rs.
Each look (Review), a town whose Merchant is level 5: fill — for each order, if the price >= 0.9 × worth, draft an EXW
contract with the shop for min(10, purse/price) daily lots at the shop's price (in force at once), deliver a lot as it
falls due from spare stock (held − par), paid from the purse (post "order"); what it can't fill is `open` and counts as
a want at this town (wagons bound here load it, PlanSale hook; its own wagons buy it as surplus at the far end,
desires' hook) and, if no known market's stocktake has it and no stall here sells it, a Signal with a premium of 1.5
rides with wagons (Signals resource; jobs reads it). buy — from stalls here, what orders and lockers want, at up to
worth (1.5 × for a signalled good). own shop — the town's biggest surplus good (not food, not money, nothing ordered)
listed in lots of 16 at 1.1 × worth; a player's purchase arrives as Input::Pay with the goods leaving Stock (add
`Input::Buy { town, good, count }` paying the listed price). Tests in tests/orders.rs with plan 4.3's numbers, tagged.
Random worlds: a few orders and stalls as inputs.
```

#### 4.4 `realms`

| | |
|---|---|
| Goal | Markets off the map: a place at a real position with a ledger warehouse and its own money, never visited, that caravans, contracts and bills treat like any town; realms beyond the border that make what our villagers can't and want some of what they make, running their own wagons on our routes with a fixed-level Merchant and no squad, paying the crown 10% customs. |
| Java | `merchant/OffMap.java` `open` (66), `is`, `sells` (85), `stock`; `trade/Realms.java` `define` (100), `open` (118), `make` (199), `isForeign` (154), `customs` (163), `USES_PER_DAY`; `caravan/Caravans.java` `dispatch`'s `foreign` branch, `sell` (the realm's locker want), `arriveAtDestination` 694 (customs), `settleContracts` for an off-map buyer, `ensureLoaded` (2812); `docs/decisions.md` "Everything runs on the logistics framework" (markets off the map), "Realms beyond the border, wares" |
| Covers | `CaravanTests.aCaravanDeliversToAMarketOffTheMap`, `CaravanTests.aForeignRealmSendsItsOwnTraders` |
| Rust | adds `realms.rs` (`OffMap` marker component: a town with no roster that's always "loaded", doesn't sell food unless told; `Realm { makes: Vec<(good, per_day, keep)>, wants: Vec<good> at 64 a day, merchant_level 4, crew 2 }`), `rules/realms.rs` (`Work`: a realm tops up what it makes; `Plan`: a realm's route needs no squad; `Sell`: customs 10% of a foreign wagon's takings to the crown's treasury if one exists); touches `society.rs` (`add_off_map(name, x, z, money)`, `add_realm(name, x, z, makes, wants)`), `content.rs` (dials `customs 0.1`, `realm_uses_per_day 64`, `foreign_merchant_level 4`) |
| Math | — |
| Accept | the Java numbers: a DAP contract for 32 paper to an off-map market that sells no food delivers all 32 into its ledger and pays from its money, the wagon carrying its own food both ways; the March's wagon sells leather at a town, goes home (trips ≥ 1) with takings; customs land in the crown's treasury when one is named; conservation holds with a realm's making through `Ledger.made` |
| Model | **sonnet / low** — in the core an off-map market is just a town without people; the realm is a small production and a flag on dispatch. |

Brief:
```
# realms
Read docs/decisions.md "Markets off the map" and "Realms beyond the border" and the Java in plan 4.4; then
sim/core/src/society.rs, rules/plan.rs, rules/road.rs, squad.rs as landed. Add sim/core/src/realms.rs: an `OffMap`
marker on a town (no roster, never counted, sells no food unless `sells_food`; road-food treats it as a far end that
can't feed, so wagons take provisions both ways); a `Realm { makes: Vec<(GoodId, per_day, keep)>, wants: Vec<GoodId>,
level: 4, crew: 2 }` component: each day it makes per_day of each good up to keep (Ledger.made), starting with 3 days'
worth, and uses 64 a day of what it wants (Trades.uses/capacity so par and desire follow); the Coast and the March as
content presets by good names (Coast: pipe tobacco, leaf, cocoa, bread; wants wheat, paper, iron, books. March: leather,
wool, bread; wants wheat, bread, carrot, iron). Its routes dispatch without a squad (squads skip towns with a Realm:
crew 2, Merchant level 4 for bargaining). Sell: a foreign wagon's takings pay 10% customs to the crown's treasury if a
`Crown` resource names a capital (hook; else nothing). Society::add_off_map(name, x, z, money), add_realm(...).
Tests in tests/realms.rs with plan 4.4's numbers, tagged. Random worlds: some worlds have a realm.
```

#### 4.5 `buildings-and-works`

| | |
|---|---|
| Goal | Public works and halls for wages: masons lay 32 blocks a day each with stone from the town's land (16 a hex a day), two masons at an emerald a day, one project at a time, the first call starting the clock; a bridge's blocks from its deck's size, a wall's from its ring (+1 mood, 2 culture a day when whole), a square (+0.5 mood), halls partnered by trades at level 3 (a counting house needs both its trades; clerks halve the stationery and charge 0.02 a document; it widens the Accountant's budget), a visiting Mason's day's work for a day's wage, flower beds (+0.5). |
| Java | `territory/Works.java` `work` (333), `help` (308), `underway` (303), `start` (230), `startWall`, `startBuilding` (267), `finish` (243), `walled`, `builtOf`, `consider` (the bridge payback), `deck` (599: only `size()`), `ring` (509: only `size()`), `MASONS`, `MASON_PAY_PER_DAY`, `BLOCKS_PER_MASON_DAY`, `STONE_PER_HEX_DAY`, `PAYBACK_DAYS`, `WALL_MOOD`, `WALL_CULTURE_PER_DAY`, `WALL_START_EMERALDS`; `territory/Buildings.java` `Kind` (sizes, `partnered` 48, `COUNTING_HOUSE` 85, `SMITHY` 94, `STORES`), `has` (120), `SQUARE_MOOD`, `FLOWER_MOOD`, `LIBRARY_CULTURE_PER_DAY`; `service/BuildServices.java` `helpWanted` (204), `help` (213), `HIRED_SKILL`; `service/MasterServices.java` `flowersWanted` (237), `plantFlowers` (244); `territory/Territory.java` `moodTarget` (1587–1600: wall, square, flowers, blessing, grief); `cargo/Paperwork.java` `settle` (the counting-house branch); `docs/decisions.md` "Walls", "Labour, Masons, Fletchers and masters" (Masons build), "Sharecroppers, hired builders", "Bridges" |
| Covers | `GrowthTests.masonsBuildABridgeForWages`, `.aWalledTownFeelsSafe`; `BuildingTests.aSquareMakesItsTownHappier`, `.aVisitingMasonLaysADaysWork`; `CountingHouseTests.aCountingHouseNeedsBothItsTrades`, completes `.aCountingHousesClerksDrawUpPapersCheaply`; `MasterTests.aMasterFarmerPlantsFlowerBeds` *(partial: master Farmer + square + 8 flowers → +0.5 mood)* |
| Rust | adds `works.rs` (`Project { kind: Bridge(hex)/Wall/Hall(Kind), blocks_needed, laid, wage_owed, started }`, `Works` component: projects, built halls, `walled`; `Hall` kinds with size, partner trades and level, effects), `rules/works.rs` (`Pay`: masons' wages and blocks laid; `Review`: what to build (bridge by payback within 30 days, a wall when content with 12 to spare and nothing better, halls when content and holding twice the cost); hired builders for towns without a Mason); touches `people.rs` (mood target hooks: wall +1, square +0.5, flowers +0.5), `territory.rs` (culture extras: wall 2, library 3, caravanserai 1; `bridge(hex)` on finish), `cash_book.rs`/`cargo.rs` hooks (counting house: budget ×1.5 and ×2 days, stationery ×0.5, clerk 0.02), `content.rs` (dials `masons 2`, `mason_pay 1`, `blocks_per_mason_day 32`, `stone_per_hex_day 16`, `payback_days 30`, `wall_blocks` (default 252 × 3), `wall_start_emeralds 12`), `society.rs` (`start_project`, `works(town)`) |
| Math | — |
| Accept | the Java numbers: 4 hexes held → 64 blocks a day; 3 days → 192 laid and 6 emeralds paid (64 → 58), done within 10 days for a 450-block deck; a finished wall, 3 villagers, no comforts, 30 days → mood ≈ 1 − 3/8; a square → mood target +0.5; a temple under way and no Mason → help wanted, a level-2 visitor lays 64 (of 196) for 2 emeralds; a counting house needs a Librarian 3 and a Merchant 3 where a smithy needs any one smith at 3; 10 documents with no stationery cost 1.0, 0.2 with a counting house; a master Farmer with a square and 8 flowers → +0.5 |
| Model | **sonnet / medium** — wages and block counts with given numbers; block geometry reduces to two sizes taken as data. |

Brief:
```
# buildings-and-works
Read docs/decisions.md "Walls", "Masons build", "Hired builders", "Bridges" and the Java in plan 4.5 (Works.deck and
ring only for how many blocks they produce: a bridge deck ≈ 450, a wall ring ≈ 252 columns × 3 high); then
sim/core/src/territory.rs, people.rs, roster.rs, cash_book.rs, cargo.rs (Paperwork) as landed. Add sim/core/src/works.rs
and rules/works.rs. `Project { kind, blocks_needed, laid, wage_owed, started: Option<u64> }`; one project at a time;
the first touch only starts the clock; then blocks a day = min(masons 2 × 32, hexes held × 16), wages = 2 × 1 emerald ×
days paid whole from the treasury (post "masons"); unpaid masons don't lay. Finish: a bridge marks map.bridged(hex); a
wall sets Works.walled (mood target +1 × share standing, culture +2 a day whole); a hall is built. Halls (`Kind`: square
9×1, temple 7×7×4, library, smithy, watchtower 3×7, granary 7×4, root cellar 5×3, stone docks, caravanserai, counting
house): each has partner trades and a level (partnered: any one at the level, or every one for the counting house:
Librarian 3 and Merchant 3) and an effect (square +0.5 mood; library 3 culture; temple: blessings double; smithy: kit
wear ×0.8; counting house: Accountant budget share ×1.5 and days ×2, documents use half the stationery and clerks charge
0.02 instead of the scribe's 0.1). Review: a town with no project under way and mood >= 0.5 picks: a bridge whose value
(chokepoints::bridge_value) repays its wages within 30 days and it can pay half now; else a wall with 12 emeralds to
spare; else a hall it wants (jobs' Mason marbles hook) when it holds twice the cost beyond its reserve. Hired builders:
a town without a Mason hires for halls up to level 3 at the same wages. Visiting Mason (services hook, Abroad): wanted
when a project is under way, no Mason in town and budget >= 1 × skill; lays 32 × skill blocks for 1 × skill emeralds. A
master Farmer with a square and 8 flowers in stock plants beds: +0.5 mood. Tests in tests/works.rs with plan 4.5's
numbers, tagged (one partial). Random worlds: content towns build.
```

#### 4.6 `crown`

| | |
|---|---|
| Goal | The sovereign: a capital whose treasury mints each day (debasing the spur), phases (peace, mobilising, war), a stockpile of kit and 20 days' food (half meat) sourced by requisitions (posted per good for 5 days at the crown's price; towns bid at landed cost plus half the way; after 2 days the cheapest offers are awarded whole lots with bonus clauses and a third up front); training tenders for masters; at war, levies from towns the capital's wagons reach, mercenaries over the sea when short of 24, the front off the map supplied by the capital's wagons day by day; at peace, home. |
| Java | `market/Sovereign.java` `mobilise` (260), `mint` (435), `declare` (294), `openFront` (392), `peace` (416), `targets` (457), `STOCKPILE`/`stockpile` (110–117), `foodReserve` (193), `bonuses` (140), `requisition` (335), `offerTraining` (590), `hireMercenaries` (355), `onServerTick` (476: the daily loop and the front's orders); `market/Requisitions.java` `post` (172), `read` (288), `bid` (318), `award` (362), `Requisition.ceiling` (102); `contract/Bonus.java` `met` (23); `contract/Contract.java` `priceOf` (204); `contract/Levy.java`; `caravan/Caravans.java` `raiseLevy` (1662), `hireMercenaries` (1721), `recruitsOwedAt` (2606), `arriveHome` 1201 (muster); `docs/decisions.md` "War footing", "Trust, advances and the capital's port", "The crown's procurement agent", "Mercenaries, meat", "Feeding the front" |
| Covers | `WarTests.bonusClausesPayForQualityWithoutRequiringIt`, completes `.mobilisingTheSovereignMintsAndBuysGoldThroughAShop` *(partial stays: the gold shop is Numismatics)*; `CaravanTests.aRequisitionTakesTheBestOfferForTheWholeLot`, `.aCaravanBringsMercenariesHiredOverTheSea` |
| Rust | adds `crown.rs` (`Crown` resource: capital, phase, theatre, treasury account, mint per day, stockpile targets, planned army 24; `Requisition { good, count, by, ceiling, bids, awarded }` on a `Board`; `Bonus { at_least: Vec<(enchant, level)>, percent }` and `Levy { soldiers, pay }` terms), `rules/crown.rs` (`Review` daily: mint, the stockpile's shortfall → requisitions and tenders, bids read off boards carried by wagons, awards after 2 days; at war: levies drafted with towns reached, mercenaries, the front's orders for food heartiest first and kit best first, `army::campaign`; peace: demobilise), `Settle`: a levy delivers soldiers (hands marked away → recruits), `Papers`: levy pay owed home carried by the capital's wagons; touches `contracts.rs` (`Contract.bonus`, `levy`; `price_of(item quality)`), `content.rs` (dials `mobilising_premium 1.5`, `war_premium 2.5`, `gold_premium 3`, `training_bonus 24`, `soldier_pay 0.25`, `mercenary_pay 1.0`, `mercenaries_most 8`, `levy_share 0.2`, `levy_most 4`, `planned_army 24`, `ration_store_days 20`, `meat_points 1920`, `period_days 5`, `bid_days 2`, `bid_share 0.5`, `make_markup 0.25`, `ceiling_carriage 1.1`, `front_distance 1500`), `society.rs` (`name_capital(town, spurs_per_day)`, `mobilise`, `declare(theatre)`, `peace`, `crown()`, `tally()`) |
| Math | **Poisson arrivals** stays planned; note that requisition periods and bid days are fixed windows as in the mod |
| Accept | the Java numbers: a sword contract at 2 with a bonus (Unbreaking 1 + Sharpness 2 → +25%) pays 2 plain and 2.5 for Sharpness 3 + Unbreaking 1; mobilising sets the phase, mints (debasement rises), `targets` include diamond swords, `food_reserve(capital)` = 384 × 5 + 1,920 = 3,840 and 0 elsewhere, peace resets it; a requisition for 40 bread gets one bid below the ceiling per reader, no award before 2 days, then exactly one contract for the bid's count with `open = 40 − count`; a levy of 3 with the overseas market delivers 3 recruits waiting at the capital; the war tally counts soldiers raised, fallen, deserted, kit and food |
| Model | **opus / medium** — the crown ties army, contracts, requisitions, mint, realms and roster together; pinned by four tests but shaped mostly by the decisions doc's runs, so judgment on what to carry. |

Brief:
```
# crown
Read docs/decisions.md's war sections (plan 4.6) and the Java named there; then sim/core/src/army.rs, mint.rs,
contracts.rs, rules/advances.rs, sourcing.rs, realms.rs, training.rs, cargo.rs as landed. Add sim/core/src/crown.rs and
rules/crown.rs. `Crown` resource: capital TownId, phase (Peace/Mobilising/War), theatre, mint_per_day (spurs minted into
the capital's treasury daily through Mint::mint and the Ledger as `minted` income), the stockpile (16 diamond swords, 8
iron, 8 of each diamond armour piece, 16 of each iron, 6 chain and 6 leather chestplates, 12 shields, 8 bows, 256
arrows, 384 bread; food 20 days for 24 at 8 points, half meat (1,920 points)), targets() and food_reserve(town). Terms:
`Bonus` (percent added when every listed enchantment is met at its level; Contract::price_of(kit) sums met clauses) and
`Levy { soldiers, pay_per_day }` (EXW, one installment: delivered when the town's hands ride to the capital). Daily
(Review): mint; the stockpile's and the army's shortfall for the next 5 days beyond contracts and open requisitions →
one Requisition per good on the capital's board (count, by day, ceiling = worth × 1.1 × 1.1 × premium 1.5/2.5; meat as
one requisition in food points); boards ride with wagons (atlas hook); a town reading one bids what it can supply by
the day (spare stock and what its offers make) at landed cost (+25% for goods to make) plus half the way to the ceiling,
one bid a reader; after 2 days the cheapest offers are awarded until covered, each a single contract for its whole lot
with the crown's bonuses and an advance of a third where trusted. Training tenders for 2 master Weaponsmiths, 2
Armorers, 1 Toolsmith at 24 × level/5 × premium (×1.25 a lapse). War: declare opens the front (an OffMap town 1,500
blocks north with the treasury's account and a route from the capital), drafts a Levy with each town the capital's
wagons reach: max(1, min(4 + barracks, idle + round(rest × 0.2))) hands, idle first, then a trade it has others of, then
the least skilled, never a baby or the Merchant; levied hands ride to the capital (away) and wait as recruits; short of
24 the crown hires mercenaries from the overseas realm by a Levy at 1.0 a day (at most 8, one open at a time); the
front's quartermaster orders from the capital daily what army::wants lacks (heartiest food the capital holds first, then
bread; best kit first), delivered by the capital's wagons; levy pay 0.25 a soldier-day owed home, carried by the
capital's wagons; army::campaign runs on the front's stock. Peace: demobilise, minting stops. Tally per town. Tests
in tests/crown.rs with plan 4.6's numbers, tagged. Random worlds: a few worlds name a capital and go to war for 20 days.
```

#### 4.7 `stranded-paper`

| | |
|---|---|
| Goal | A sound town isn't ruined by a closed road: a hub buys a bill a town holds on a market whose wagons don't call (face less 2%, 10% more past its day, less the forwarding of its goods) for coin as far as it trusts the drawee and can spare, the rest in its own bills in up to six installments; freight on credit (a bill to the hub due in 30 days, up to half the goods' worth); no offtake without a collector; dead paper goes back on the shelf; a bill nobody can carry lapses; citizens' deposits honour a bill. |
| Java | `market/Stranded.java` `reaches` (34), `bill` (44), `crate` (52), `buy` (83–160), `INSTALLMENTS`; `market/Discounts.java` `quote` (the stale branch, `STALE_DISCOUNT`); `contract/Bills.java` `lapse` (193), `grace`; `market/Apprentices.java` `collectable` (125); `caravan/ForwardSales.java` `earmark` 166 (the stale-crate sweep); `market/Freight.java` the credit branch (search `freight_lien`); `docs/decisions.md` "Stranded paper: a sound town ruined by a closed road" |
| Covers | `ContractTests.aHubBuysATownsStrandedPaperForCoinAndItsOwnBill` |
| Rust | adds `rules/stranded.rs` (`Papers` at a hub or where a hub's wagon calls: buy stranded bills; `Freight`: credit fares; `Judge`: dead shipments return to stock); touches `bank.rs` (`Discounts` stale branch), `bills.rs` (`collectable(town, drawee)` through hubs; lapse exemption), `cargo.rs` (offtake only with a collector), `content.rs` (dials `stale_bill_discount 0.1`, `freight_credit_days 30`, `freight_lien 0.5`, `stranded_installments 6`) |
| Math | — |
| Accept | the Java numbers: a past-due bill of face 100 × 1.02: the hub pays coin up to face × (1 − 0.02 − 0.1) × trust(hub, drawee), the rest as its own bill, the shipment goes with the bill to the hub's forwarder, the bill endorsed to the hub; a town's goods ride through the hub on credit when it can't pay the fare; a bill collectable through a hub doesn't lapse; conservation holds |
| Model | **sonnet / medium** — one Java method with exact arithmetic over landed bank, freight and cargo code. |

Brief:
```
# stranded-paper
Read docs/decisions.md "Stranded paper: a sound town ruined by a closed road" and the Java in plan 4.7; then
sim/core/src/bank.rs, bills.rs, freight.rs, cargo.rs, farms.rs (deposits) as landed. Add rules/stranded.rs. Where a
hub's wagon calls at a town, or a town's wagon brings its paper to a hub (Papers step): for each bill the town holds on
a drawee whose wagons don't call there (or past its day) that the hub runs to, price = face × (1 − 0.02 − (past due ?
0.10 : 0)) − forwarding of the shipment it paid for; pay coin up to price × trust(hub, drawee) as far as the hub can
spare beyond its reserve, the rest as the hub's own bills in up to 6 installments a bill term apart, each at most half
what its treasury could pay (beyond that, don't buy); endorse the bill to the hub; the shipment goes with it to the
forwarder. Never a bill on the hub itself (presented there at face). Freight on credit: a hub's wagon carries a town's
shipments for markets the hub serves when the town can't pay the fare: the unpaid part is a bill to the hub due in 30
days, up to half the goods' worth. No offtake without a collector: training's offtake crates and bills only where a
buyer's wagon calls or a hub runs to the buyer (Bills::collectable). Dead paper: a shipment under a contract no longer in
force returns to Stock unless paid for by an outstanding bill (then it goes with the bill); a bill gets its grace from
first presenting; one nobody can carry lapses 12 days past due (bank's rule; exempt when collectable through a hub).
Citizens' deposits: a Merchant short of coin to honour a bill borrows its households' savings (farms' deposits). Tests
in tests/stranded.rs with plan 4.7's numbers, tagged. Random worlds: a closed road (remove a route mid-run) leaves
stranded paper that a hub buys.
```

#### 4.8 `settlers-rule`

| | |
|---|---|
| Goal | The settling decision only: a town grown past growing raises a party every 8 days at most, sending a travelling Merchant with capital to a village nobody trades with (4–8 hexes out), or founding a town on open ground; the party is off the world on the road; the new market opens with the capital and a route home. |
| Java | `caravan/Expeditions.java` `consider` (130), `capital` (218), `homeAgain` (398), constants; `docs/decisions.md` "Settlers" |
| Covers | `ExpeditionTests.aMerchantSettlesAVillageNobodyTradesWith` *(partial: the party's capital lands in the new town's warehouse and a route opens; the bell and desk are Minecraft's)* |
| Rust | adds `rules/settlers.rs` (`Review`); touches `content.rs` (dials `settling_capital 30`, `founding_capital 42`, `days_between 8`, `min_hexes 4`, `max_hexes 8`), `society.rs` (`add_village(x, z)` as a candidate; `settle(town)`) |
| Math | — |
| Accept | a town of 8+ with food raises a party once per 8 days; the party's hands are away for the road's time; the new town has the capital (`max(10, min(30, spare/2))` emeralds, 14 in all for a village) and a route from home; conservation holds |
| Model | **haiku / low** — a small rule with explicit constants and one partial test. |

Brief:
```
# settlers-rule
Read docs/decisions.md "Settlers" and caravan/Expeditions.java `consider` and `capital`; then sim/core/src/roster.rs,
hex.rs, society.rs as landed. Add rules/settlers.rs (Review, daily): a town with 8+ villagers, food at least twice its
par, and at least days_between (8) days since its last party, with a candidate village (Society::add_village(x, z):
a placed town with no Merchant and no route, 4–8 hexes from home and 4+ from any market) sends one idle hand as a
travelling Merchant with capital = max(10, min(settling_capital, spare/2)) emeralds and 64 bread (14 emeralds in all
at the floor); with no village, founds a town on an unclaimed hex 4–8 out with founding_capital. The party is away for
the road's time (hex distance × 80 blocks at 1,200 a day); on arrival the village gets a Merchant hand, the capital in
its treasury (post "settling") and a route home (add_road). Test in tests/settlers.rs, tagged
`// parity-partial: ExpeditionTests.aMerchantSettlesAVillageNobodyTradesWith`. Random worlds: some worlds hold a
village.
```

---

### Wave 5: closing the phase

#### 5.1 `random-worlds-sweep`

| | |
|---|---|
| Goal | Property-based testing in earnest: the random worlds exercise every mechanism landed, with invariants beyond conservation (no stall: every route with goods and a buyer runs within a month; no town's mood below −8; no contract in force forever; wages paid or credited; the save round-trips every seed), and a sweep report of what the worlds do. |
| Java | none |
| Covers | none (it hardens every claim) |
| Rust | rewrites `tests/random_worlds.rs` (a generator that turns each mechanism on by seed: rosters, lockers, farms, herds, map and roads, hubs, a bank, a crown at war; 300 seeds × 120 days in under 10 s), adds `tests/common/mod.rs` shared by every test file; `sim/host` prints a sweep summary |
| Math | **property-based testing**: tick the row |
| Accept | every seed passes every invariant; the time budget holds; failures print the seed and day; `the_same_seed_gives_the_same_world` and a save/load round-trip run on every seed |
| Model | **sonnet / medium** — needs the whole core's API in view, but every mechanism has a host call to turn it on. |

Brief:
```
# random-worlds-sweep
Read every sim/core/tests/*.rs and sim/core/src/society.rs. Rebuild tests/random_worlds.rs so each seed turns on a
random subset of mechanisms through Society's host calls (rosters and lockers, farms, herds, a hex map with roads and
ground, posts and tolls, hubs, a bank, realms, orders, a crown that mobilises and goes to war for 20 days) and checks
every day: conserved, solvent, no mood below −8, no route with goods and a paying buyer idle for 30 days, no contract
in force past its last deadline + grace × 3, wages paid or credited, and every 30 days a save/load round-trip that
continues bit-equal. Keep 300 seeds × 120 days under 10 s in release; shrink the map if needed. Move the generator to
tests/common/mod.rs and have the other test files reuse it where they build random worlds. Print the failing seed and
day. Give sim/host a `--sweep N` that runs N seeds and prints what the worlds did (trips, contracts, mood ranges).
Tick property-based testing in docs/core-math.md.
```

#### 5.2 `fixed-point-money`

| | |
|---|---|
| Goal | Money and counts as fixed-point integers so replays agree across hosts (WebAssembly in Java against native), before the Minecraft binding. |
| Java | none; `docs/core-math.md` fixed-point row; `docs/worktrees.md` pain 6 |
| Covers | none (it protects every claim) |
| Rust | adds `fixed.rs` (`Money` as i64 in thousandths of an emerald, or a `Fixed` newtype with checked ops and `Display`); changes every `f64` money field and every rate (`labor_share`, `trust`, mood) that feeds money; `check.rs` tolerances become exact |
| Math | **fixed-point arithmetic**: tick the row |
| Accept | all tests pass with exact equality where tolerances were; the wasm build's outputs on a saved script equal the native build's byte for byte (a test that runs the script natively and checks against a committed digest; the wasm run is documented as a manual step until a runner is in the repo) |
| Model | **opus / high** — numerics that touch every file: rounding conventions (where the mod floors, where it ceils), overflow, and the half-lives' exponentials; it must land alone, last. |

Brief:
```
# fixed-point-money
Read docs/core-math.md (fixed-point) and every sim/core/src file. Introduce a `Fixed` newtype (i64 in 1/1000 emerald;
goods counts stay f64 only where the mod keeps fractions: herds, pending produce; else whole items as i64 or f64 with
.floor() at the boundary) with checked add/sub/mul-by-ratio and the mod's rounding at each site (floor for payments
and prices in spurs, ceil for costs as the Java does: check each call site's Java). Replace f64 money in state,
contracts, bills, cash_book, every rule and host view. Exponential decays (trust half-lives, mint pressure, mood)
compute in f64 and round once into Fixed at the boundary, with a note on why. Make check.rs's conserved() exact.
Every test passes with exact comparisons where tolerances were; commit a digest of a 200-day script's outputs
(tests/replay_digest.txt) that the replay test checks, and document in sim/README how to run the same script under
wasm32-wasip1 (wasmtime) to compare. Tick the row. Land last, alone.
```

#### 5.3 `parity-report`

| | |
|---|---|
| Goal | The phase's closing numbers: `docs/parity.md` from `scripts/parity.py --md`, a check that every tag names a real Java test and every skip has a reason, and a list of partials with what's missing. |
| Java | none |
| Rust | none; writes `docs/parity.md`; small fixes to `scripts/parity.py` if tags are malformed |
| Accept | `parity.py --md docs/parity.md` runs clean; no unknown tags; the summary line matches the plan's target (≥ 190 covered of 215 non-skipped) or the shortfall is listed by task |
| Model | **haiku / low** — a script run and a tidy-up. |

Brief:
```
# parity-report
Run `python3 scripts/parity.py`, `--left`, and `--md docs/parity.md`. Fix any tag that names a Java test that doesn't
exist (misspelt method names) in the Rust test files, and any skip line without a reason. In docs/parity.md's header,
list every `parity-partial:` with one line on what's missing (from the test's comment). Report the final counts.
```

## 5. Waves

Tasks in one wave run at the same time: they add their own files and only add lines to the shared files. A task
"waits for" another when it reads its landed code. Land in the order given within a wave: structural first, so
rebases are short. The integrator can start a later wave's task early when its "waits for" column has landed.

| Wave | Task | Model / effort | Waits for | Lands |
|---|---|---|---|---|
| 1a | `look-steps` | haiku / low | — | 1 |
| 1a | `parity-triage` | haiku / medium | — | 2 |
| 1a | `contract-tags` | haiku / medium | — | 3 |
| 1a | `mint-rate` | haiku / medium | — | 4 |
| 1a | `save-load` | opus / medium | — | 5 |
| 1a | `cash-book` | opus / medium | — | 6 |
| 1a | `roster` | opus / high | — | 7 |
| 1a | `hex-map` | opus / medium | — | 8 |
| 1b | `bargain-and-par` | sonnet / high | `cash-book` (it edits `sell`) | 9 |
| 1b | `advances-and-refunds` | sonnet / medium | `cash-book`, `save-load` | 10 |
| 1b | `road-food` | opus / medium | `cash-book`, `bargain-and-par` (both edit `arrive.rs`) | 11 |
| 2 | `lockers` | sonnet / high | `roster`, `cash-book` | 12 |
| 2 | `squads` | sonnet / high | `roster`, `road-food` | 13 |
| 2 | `farms-and-wages` | sonnet / high | `roster`, `cash-book` | 14 |
| 2 | `jobs` | sonnet / medium | `roster`, `lockers` (reads offers; start after lockers' API is visible) | 15 |
| 2 | `territory-growth` | sonnet / medium | `hex-map`, `cash-book` | 16 |
| 2 | `workshop` | sonnet / medium | `roster` | 17 |
| 2 | `harvest-and-rents` | sonnet / medium | `advances-and-refunds` | 18 |
| 2 | `atlas-and-books` | sonnet / medium | `hex-map`; `squads` for the Librarian (partial otherwise) | 19 |
| 2 | `herds` | sonnet / medium | `roster`, `workshop` (boots) | 20 |
| 3 | `chokepoints` | opus / medium | `territory-growth` | 21 |
| 3 | `services` | sonnet / high | `squads`, `herds`, `farms-and-wages`, `atlas-and-books` | 22 |
| 3 | `mines-and-kit` | sonnet / medium | `territory-growth`, `services` (the kit table; or define it first and let services read it) | 23 |
| 3 | `desires-and-backhauls` | sonnet / medium | `bargain-and-par`, `atlas-and-books` | 24 |
| 3 | `crates-and-papers` | sonnet / medium | `harvest-and-rents`, `advances-and-refunds` | 25 |
| 3 | `freight-hubs` | opus / high | `crates-and-papers`, `atlas-and-books`, `territory-growth` | 26 |
| 3 | `sourcing-and-procurement` | sonnet / high | `lockers`, `atlas-and-books`, `services` (Crafts inputs) | 27 |
| 3 | `war-army` | opus / high | `roster`, `mines-and-kit` (kit kinds; or its own table, reconciled at landing) | 28 |
| 3 | `bank-and-discounts` | sonnet / high | `advances-and-refunds`, `crates-and-papers` | 29 |
| 4 | `post` | sonnet / high | `freight-hubs`, `bank-and-discounts` | 30 |
| 4 | `training-and-tenders` | sonnet / medium | `lockers`, `crates-and-papers`, `jobs` | 31 |
| 4 | `player-orders` | sonnet / medium | `lockers`, `desires-and-backhauls`, `atlas-and-books` | 32 |
| 4 | `realms` | sonnet / low | `squads`, `road-food` | 33 |
| 4 | `buildings-and-works` | sonnet / medium | `chokepoints`, `crates-and-papers`, `cash-book` | 34 |
| 4 | `crown` | opus / medium | `war-army`, `mint-rate`, `sourcing-and-procurement`, `realms`, `training-and-tenders` | 35 |
| 4 | `stranded-paper` | sonnet / medium | `bank-and-discounts`, `freight-hubs`, `training-and-tenders` | 36 |
| 4 | `settlers-rule` | haiku / low | `roster`, `hex-map` | 37 |
| 5 | `random-worlds-sweep` | sonnet / medium | everything above | 38 |
| 5 | `fixed-point-money` | opus / high | `random-worlds-sweep`; runs alone | 39 |
| 5 | `parity-report` | haiku / low | `fixed-point-money` | 40 |

**Concurrency:** 8 trees in 1a; 3 in 1b (start as 1a's land); 9 in wave 2; 9 in wave 3; 8 in wave 4; wave 5 is
sequential. With a cold build of 13 s a tree and 3 s after, the machine's limit is the integrator's attention, not RAM.

**Model mix:** 40 tasks — 6 haiku, 24 sonnet, 10 opus. Opus goes where structure is set (save, cash book, roster,
map, road food) or the Java is subtle and emergent (chokepoints, hubs, the army, the crown) or the numerics are
global (fixed point). Sonnet takes every self-contained port with a Java reference and pinned numbers. Haiku takes
tagging, lists and one-file ports whose expected values are written down.

**Ordering rules for the integrator:**
1. Land `look-steps`, `save-load` and `cash-book` before anything in wave 2 starts: every later brief assumes the
   steps, the derive rule and `post()`.
2. Land `bargain-and-par` before `road-food` and `desires-and-backhauls`: all three edit `arrive.rs::sell`/`turn`.
3. When a task's "waits for" hasn't landed, the brief says what to do (a hook, a default, a partial tag); the
   integrator reconciles at landing and may reopen the earlier tree for a one-line hook.
4. After each wave, run `python3 scripts/parity.py` and update the counts at the top of this file.

## 6. Risks

**Shared files still collect additions.** `content.rs`'s `Dials` will gain about 120 fields across the plan; two trees
adding dials in the same wave will rebase by hand if they touch adjacent lines. Mitigation: each task appends its dials
as one block under a comment with its name, at the end of the struct and of `Default`; if it bites in wave 2, a
follow-up splits `Dials` into per-module structs (`Dials.farms`, ...) with `set(name)` paths like `farms.labor_share`.
Same for `state.rs` and `society.rs`: own blocks, appended.

**`arrive.rs` and `plan.rs` are edited by four wave-1/2 tasks** (`cash-book`, `bargain-and-par`, `road-food`,
`atlas-and-books`'s stocktake-based sizing, `desires-and-backhauls`' comfort branch). The ordering rules above
sequence them; expect one hand-merge each. A cleaner cut, if the integrator prefers: `cash-book` first extracts
`sell`'s per-good body into a function `sell_good(...)` that the others then change in their own lines.

**Roster against `People.villagers`.** Two sources of truth for a day: births, mood and comforts use `villagers`;
work slices use hands. `roster` makes `villagers` derived; until it lands, random worlds keep both consistent by
construction. Risk: a later task reads `villagers` where it should count working hands (babies, away). Reviewers check.

**Ordering within a step.** Bevy doesn't order systems in one set unless told. Every brief says `.after(...)`; the
reviewer checks that each new system names what it follows (the mod's `Territory.touch` order is: guards, herds, docks,
mine, labour, hunting, kit wear, wages, delivery, workshop, home services, kit shares, shop). `look-steps` gives room,
but inside `Work` the chain is herds → farms → mines/gathering → workshop → services.

**Bargained prices change the trade loop's measured ratios.** `bargain-and-par` may move `a_buyer_short_of_cash...`
numbers. It's allowed to adjust expectations with before/after figures in the commit; the integrator checks the
direction of each finding still holds (fewer, fuller wagons still need buyers with cash).

**Food as the road's cost changes `decide`.** `road-food` keeps money `road_cost` where content has no food good, so
old tests are bit-identical; but worlds with food will see different departure patterns. Random worlds must cover both.

**Where the Java is unclear or quirky** (each brief points at it; the agent should port the rule, not the quirk,
and note it):
- `Workshop.potential` reads the global `Territory`, so in tests the workshop wants no comfort outputs and
  `aLibrarianBindsBooks` passes through the Crafts recipe. Port both paths; decide which runs first by gain.
- `Caravans.foodKnownAt` treats a market with no stocktake as having plenty of food (`MAX_VALUE`), which is why
  `caravansDontBuySurplusNobodyWants` passes. Port "unknown means plenty" as a stated assumption.
- `aTownShortOfClothesBuysAPairOfBoots`'s comment says a 2-emerald par; the code gives 3. Use the code.
- `TRUST_PER_SERVICE` is 0.1 in the Java, 0.02 in the decisions doc. Use the Java.
- `Growth.BIRTH_MOOD = 1` is dead; the live threshold is the dial 0.5 (the core already has `birth_mood 1.0`: `farms`
  or `jobs` should set the default to 0.5 and note it).
- `Desires` is reached by no Java test through the caravan path; its tests pin the pure functions only.
- `theFallenAreGoneAndTheirTownGrieves` never reaches `mourn` (no bell in the test); the core's test should pin grief.
- `aLeatherworkersPacksCarryMore` sends packs for stock the far end has no room for (`wantedStacks` counts home's
  whole stock). Port the rule as written; it's harmless.
- `Books.why` labels come from a `StackWalker` in `CashBook.caller`; pass labels explicitly.

**What the Rust host can't vet, and what to check instead.** Minecraft-fixture tests (chests, villagers, vendors)
become number tests of the same rule; the risk is a rule that only shows under the fixture's accidents (a chest's 27
slots, a villager wandering). The random worlds and the decisions doc's measured runs are the backstop: wave 5's
sweep should reproduce, qualitatively, three findings from the doc (a town without a road stagnates at mood ~+0.3
until it makes its own comforts; a hub emerges at a pass; a closed road strands paper that a hub buys). If a sweep
can't show one, that's a finding to write up, not a test to drop.

**Randomness.** The jobs draw and Merchant XP roll dice in the mod. The core uses a seeded `Rng` in the state (who
adds it: `roster` or `jobs`, whichever lands first; the brief says to check). Attrition is expected-value by decision
8; the war tests pin risk numbers, not kill counts, so this holds.

**Parity is self-reported.** A `parity:` tag is a claim. The integrator reads each tagged test against the Java test's
assertion before landing; the plan's "Covers" lists say which numbers to look for. Partial tags must say in a comment
what's missing.

**Fixed point last, alone.** It touches every file; nothing else can be in flight. If the phase runs long, it can slip
to the start of the binding phase without hurting parity.

**Not in this phase.** The event queue (discrete-event simulation): the fixed look is fast enough (10,000 days of nine
towns in half a second) and a queue would change every `register`. Contraction hierarchies, ARIO, Gillespie,
tau-leaping, replicator dynamics: no slice needs them yet. The Minecraft binding itself.
