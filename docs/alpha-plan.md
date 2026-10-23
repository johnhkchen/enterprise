# Alpha plan: Enterprise in core mode, playable and readable

John, 2026-10-23: "prepare for an alpha deployment so we aren't just running and implementing blind. Some of the
features planned can be stubbed, and we can use enough of a UI to read the ledger state that the system can run even
without full bindings in place." He chose an in-game screen over a web page.

The alpha is main once the four B4c trees (`fixtures-caravans`, `-land`, `-market`, `-people`) have landed, and
before `delete-java-rules`. It runs **core mode only**: the Rust core runs the society; the Java rules stay in the jar,
but none of them is reachable for a concern the core owns. Everything the core can't do yet says so in plain words. Read
with `docs/binding-progress.md` ("Now") and `docs/decisions.md` (the last three sections).

## 1. What a player can do in core mode today (main `be4bf7b`)

The evidence: `binding/adapters/*`, `sim/core/src/*_host.rs`, `CoreBindingTests`, `just gametest-core` (308 of 308,
**but** decisions.md 2026-10-23: those 308 counted fixtures whose bodies still call the Java rules, so they show little
of the core running a world). **No core-mode server has run a world for days:** `network_study.py` under `society=core`
has only had its regexes checked (`CoreBindingTests.aTownsTerritoryAndTheWarAreTheCoresForTheStudies`). The alpha's
first job is that run (section 5).

| Concern | State | What works / what doesn't (evidence) |
|---|---|---|
| Stock | works | Desk and depot chests counted at each look and written from `Moved` (`StockAdapter`, `chests.rs`). Doesn't: goods in placed storage crates aren't town stock; depot kinds (armoury, barracks) have no core rule; desks already in spawn chunks when the server starts aren't scanned (the testbench registers its desk with `enterprise core stock` in `load.mcfunction`); a -1.8e-15 stock residue (seed 207). |
| Money | works | Emeralds, coin and the Blaze Banker counted and written (`MoneyAdapter`, `till.rs`). Confusing: a town's emerald reserve is 0 unless a switched-on locker trade costs emeralds, so a desk's first count sells every emerald to the mint (into the Banker or coin), as the mod's `MoneyChanger` did. Missing: the wage float; coin in shulkers; only a town's first desk shows money. |
| People | works | The census is the roster's source; births, levels and departures are written to villagers (`PeopleAdapter`, `census.rs`). Doesn't: settlers have no view (a founded town has no desk); a villager's rolled offers aren't the core's (`set_offers` not wired); `Settlements::onDeath` still writes the Java roster. |
| Land | partly | Survey in; halls, bridges, wall and fields laid (`LandAdapter`, `land.rs`). Missing: wooden piers, pastures' fences, mine heads (`fixtures-land` gives stalls, mine heads, pastures and tolls core rules). **Refusal loop:** a field is paid for before its site is found, so a field with no room is paid for and asked for again at every survey. |
| Lockers | works | The core runs the trades; the screen's switches go to the core (`LockerAdapter`, `locker_host.rs`). Confusing: a locker is emptied into the desk at each look; the screen lists every offer of the villager's level, not the two it rolled. |
| Wagons | works | Routes opened and wound down from the Merchant's screen and `route add/remove`; crews stowed and let out (`WagonAdapter`, `wagon_host.rs`). Doesn't: herds and pack llamas; no crates appear at unloading; the screen lost "road mapped". |
| Shops | works | Numismatics vendors, the shop board and rent deeds (`ShopAdapter`, `shop_host.rs`). Risky: coin is taken out of a vendor's box before the core accepts the look (a refused look loses it); `present` blocks the server thread on `settle()`. |
| Crown | partly | Capital, mobilise, declare, peace, realms and the gold shop (`CrownAdapter`, `crown_host.rs`). Missing: no vendors for the metal exchange, requisitions or freight hubs (`fixtures-market` and `fixtures-caravans` are adding them); the front and the market across the sea are points with no block, shown as a "desk" at height 0. |
| Pallets | works | Pallets drawn as tagged package items in desk chests (`PalletAdapter`, `pallet_host.rs`). Missing: packing is an op command only (`core pack`); the superbill is a command line, not paper; a crate a player crafts and fills isn't counted. |
| Commands | works | The `core` subcommands (stock, points, chests, cash, mint, hands, census, land, lockers, cashbook, books, whys, trust, contracts, routes, pallets, pack, shops) and the core-mode `mint`, `territory`, `war`, `routes`, `dial`, `autoswitch` all ask the core. About 18 commands refuse with "Java mode only until B5.1 delete-java-rules" (`EnterpriseCommands.refuse`: survey, expeditions, cottage, bridge, hex, name, forwarder, post, paperwork, freight, bank, bank open, tenders, toll, services, settlements, switch). |

**Most likely to break or confuse a player**, worst first:
1. **The first long core-mode run.** Nobody has watched one. Expect a trap, a refused look (its inputs dropped, by
   design), or a town's books drifting from its chests.
2. **The jar-in-jar has never been launched outside a dev run** (Chicory, binding and wasi nested in the core jar). The
   first Prism launch is the first real one. A class-loading failure there means "The core didn't start; the world runs
   without it": a silently frozen world, since core mode registers no Java tick listeners.
3. **The mode is per process, not per world** (`Society.mode()`, read once from property, then env, then
   `config/enterprise.properties`, default java). If a Prism instance lacks the config file, it plays Java mode without a
   word, and a world opened in both modes runs two societies apart.
4. **A paused core says so only in the log** (`CoreHost.pausedWhy`: a save it refuses, a trap it can't recover from).
   The world goes still.
5. **Java numbers on core screens:** the Merchant's screen shows the Java `Mint`'s rate, which nothing advances in core
   mode (`MerchantView.build`), and `Merchant.lockersAt` falls back to Java's `lockerLimit`.
6. Emeralds vanish into the mint at a desk's first count (reserve 0); lockers emptied each look; the field refusal
   loop. Each of these is right by the rules, but a player can't see why without the ledger.
7. Silently dead: crafted crates, metals at the port, mine heads and piers, settlers' villages, `name`.

**The "Now" open items, triaged for the alpha.** These matter: the jar-in-jar launch, the mode being per process, the
shops' coin taken before acceptance, the field paid before its site, the paused core's silence, and that the kept study
has never run under core. These can wait: the look bar (2.3 to 9 ms on the core's thread, off the tick; fine for a few
towns), outputs coming a tick late, `prev.dat` never deleted, the replay log growing until a save, the crown's card
balance at peace, `lastChunk` not saved, the seven Java-only dials, town names drifting, and every "still calls the Java
rules" note about game tests.

## 2. Stubs: missing features made honest

The rule: in core mode a player action either reaches the core or is refused in plain words. It never quietly does
nothing, and it never calls a Java rule.

| Where | Today in core mode | The alpha's stub |
|---|---|---|
| Refused commands | "/enterprise X is Java mode only until B5.1 delete-java-rules" | `CommandsAdapter.javaOnly` reworded: "Not in the alpha yet: <one line on what to do instead>"; `bank` and `tenders` say "Gone: …" (they're on the lost list). The ones B4c gives core rules (bridge, toll, name, post, paperwork, freight, services) are no longer refused once it lands. |
| Merchant's screen rate | Java `Mint.rate` | The core's `Query::Mint` rate (one more query at open); the Banker's balance is physical, so it stays. |
| `Merchant.lockersAt` | Java `lockerLimit` when the core has no town | 0, with a "no town here" line: no Java fallback. |
| Crafted crate (`CrateBlock`, `CrateMenu`) | Java `Crates`/`Papers` menu; the goods aren't counted | In core mode, using a placed crate opens it as plain storage and says once "Crates you pack aren't counted in the alpha: keep goods loose in the desk's chests". Its Java papers are not run. |
| `Settlements::onDeath` | writes the Java roster | Skipped in core mode (the census owns the roster). `Growth::onJoin` stays: it is the villager's age view. |
| Ports with no vendor (metal exchange, requisitions, freight) | a vendor there is classified as a player's | If B4c hasn't landed them: in the player notes as missing. No new code. |
| Paused or missing core | log only | On every op's join and at the top of the ledger: "The society is paused: <why>". Also when the core didn't start at all, so a jar-in-jar failure shows on the first launch. |
| World opened in the other mode | silent drift | A world mode marker (section 4). A core-mode world opened by a Java-mode process, or the reverse, pauses the host and says why. |
| Field with no site | paid and asked every survey | Not a stub: the alpha doesn't fix it, but the ledger shows it (the cash book's field lines, the land works "asked, no site"). It goes to the smoke run's findings. |

## 3. The ledger screen

**Where: a third tab, "Ledger", on the Merchant's screen** (`client/MerchantScreen`, next to Lockers and Caravans).
Why there and not a ledger book item:
- The screen already exists.
- `MerchantView` is already built once, on the server, at open, from the core (`LockerAdapter.view`,
  `WagonAdapter.caravans`), so the ledger rides the same packet with no new menu, item, recipe, texture or payload.
- The desk is where a town's books are.

Its gap: a desk with no Merchant villager has no screen. There, `/enterprise ledger <desk>` prints the same lines.

**What it asks:** one `host.ask(List<Query>)` at open, in a new `adapters/LedgerAdapter`, all from existing variants:
`Cash{desk}`, `CashBook{town, 7}`, `Mint`, `Territory{town}` (name, mood, people), `Stock{town}` (with the adapter's
goods table), `Chests{desk}` (short, free), `Hands{town}`, `Lockers{town}`, `Routes{town}`, `Manifest{route}` for each
wagon out, `Whys{town}`, `Contracts{town}`, `Pallets{town}`, `War` (the phase line only). **No new wire query.** The
answers already hold all of it, and leaving the wire alone keeps the alpha out of the B4c trees' registry and
fixture churn. If one query is ever wanted, it's a `Query::Ledger{town}` bundle, and only to save a round trip.

`LedgerAdapter` turns the answers into a `LedgerView`: six sections of short lines, numbers formatted, negatives under
1e-9 shown as 0. The same lines go to the screen (a `LedgerView` field in `MerchantView`'s codec, empty in Java mode)
and to `/enterprise ledger <desk>`, so a check over RCON reads exactly what a player sees. A line names its source
query in a tooltip ("from CashBook"), for bug reports.

**Compute on touch:** it is asked once when the screen opens, never per tick. Close and reopen to refresh. The ask
waits behind a running look (`settle`); that's milliseconds, or seconds only right after a world starts. The
commands already accept that cost.

```
+--------------------------------------------------------------------------+
| Wheatfell · day 14 · mood 0.62 · 9 people       [Lockers][Caravans][Ledger]|
| Treasury 41.3 em · reserve 0 · chests 3 em, 120 spurs · Banker 800 spurs  |
| +----------+ +----------------------------------------------------------+ |
| | Money  > | | last 7 days             in       out                     | |
| | Goods    | |  sold to Ashby         12.0                              | |
| | People   | |  farm wages                      6.5                     | |
| | Wagons   | |  sold to the mint                20.0  (above reserve 0) | |
| | Papers   | | day 14  +3.0 wheat sold · -1.0 wages                     | |
| | Mint     | | not explained by the book: 0.4                           | |
| +----------+ +----------------------------------------------------------+ |
|  asked at open: reopen to refresh · alpha 0.1.0-alpha.1 · core 3f2a…  ^ v |
+--------------------------------------------------------------------------+
```

The sections:
- **Money:** treasury, reserve, till, and the cash book by reason for 7 days, then by day.
- **Goods:** stock (largest first, with what the chests couldn't take: `short`, unmoved) and free slots.
- **People:** the roster (trade, level, away, child) and the lockers with the offers switched on.
- **Wagons:** each route's status, trips, sold, the wagon out (arrives, crew, stacks, manifest lines), and the top
  "went:/stayed" reasons from `Whys`.
- **Papers:** contracts (state, incoterm, party), rents, pallets held and inbound, received and lost.
- **Mint:** rate, pressure, minted, debased, and the war phase.

## 4. The alpha build

- **Core mode by default.** `Society.parse` defaults to CORE. Game tests stay pinned (`gameTestServer` sets `java`;
  `gameTestServerCore` sets `core`).
- **A world mode marker.** At a fresh world, `world/enterprise_society.txt` records the mode. A world whose marker
  disagrees with the process's mode pauses the host, says why, and joins the paused notice. A world with no marker
  but Java SavedData in `data/` counts as Java. `just server` (studies, lanes) follows the default; a Java-mode study
  needs `ENTERPRISE_SOCIETY=java`.
- **Prism** (`just prism`): `prism_sync.py` also writes `config/enterprise.properties` with `society=core` into the
  instance (belt and braces), and the testbench datapack as now. John starts a **fresh world**.
- **No save compatibility.** Say so in the notes and on the welcome panel: "Alpha worlds may not open in the next
  alpha. Start a new world." FORMAT stays 1, and the save's shape keeps changing under it (every B4 task "rebuilt dev
  worlds"). A newer build refuses an old save (`Load`), which shows as the paused notice.
- **Version:** `mod_version=0.1.0-alpha.1` (gradle.properties) and `pack/pack.toml` `version = "0.1.0-alpha.1"`.
  `/enterprise core version` prints the mod version, `wire::CORE` (the core's commit), `SCHEMA`, FORMAT and the mode.
  The ledger's footer shows the first two.
- **Bug reports:** `/enterprise core report` (op) writes `world/enterprise-reports/<day>-<time>/` with:
  - the version line and `pausedWhy`;
  - every town's ledger lines, its `cashbook 30` and `whys`;
  - a copy of `enterprise_core.dat`.

  It says which folder to zip with `logs/latest.log`. The core is deterministic, so the save plus the steps is a
  reproduction.
- **Player notes** (`docs/alpha.md`; its first lines on the testbench welcome panel), in plain words:
  - **Try:** put a desk by a bell and beds, hire a Merchant, and put emeralds and goods in its chests. Switch trades
    on in the Lockers tab. Open a second desk 300 blocks away and send a caravan from the Caravans tab. Put a
    Numismatics vendor by the desk. Read the Ledger tab after a game day.
  - **Not in the alpha yet:** the commands that say so, crafted crates, piers, pack llamas, settlers' new villages,
    metals sold at the port (unless B4c brought them), and renaming a market.
  - **Expect:** emeralds above the reserve go to the mint; lockers empty into the desk; a villager's own trades differ
    from the Lockers tab.
  - **Report a bug:** `/enterprise core report`, then send the folder and `logs/latest.log`.

## 5. How the integrator verifies it, without John

All headless, on main's dev ports (25599, 25598) after `scripts/server-check`; nothing else compiling meanwhile. Keep
the Mac awake (`pmset -g log | grep Sleep` if a day stalls).

1. `just build`, then `unzip -l core/build/libs/enterprise-*.jar | grep -i jarjar`: binding and Chicory are nested.
   Where the cached NeoForge installer is at hand (memory: reuse the caches), `--installServer` into the scratchpad,
   drop in Create, Numismatics and the jar, and start it once for "The core is running". That is the jar-in-jar's first
   launch outside a dev run. If no installer is cached, say so: John's first Prism launch is then the test.
2. `just server-setup`, `just server` in the background, then `just rcon "enterprise core version"` (mode core) and
   `"enterprise mint"` ("The core's mint").
3. The testbench market: `just rcon "forceload add 19 19 35 35" "function testbench:fixture/switch"`. Then 10 game
   days of `tick sprint 1d`, with `enterprise ledger 27 -60 27` after each, kept to a file.
4. The kept study, for the first time under core: `python3 dev/experiments/network_study.py 30 OUT.csv --stop-when
   any.treasury<5 --save-at 0,10 --label alpha`. It prints each town's cash book and whys.
5. **Pass if:**
   - `logs/latest.log` has none of "The core trapped", "refused a look", "The core is paused", "didn't start", or an
     uncaught exception;
   - each town's cash book is unexplained by no more than a few emeralds;
   - no treasury or stock is below 0 (beyond -1e-9);
   - after a stop and restart (or `server-restore` of the day-10 checkpoint), the ledger reads the same as before;
   - `/enterprise core report` writes its folder.
6. **No Java rule ran:** list `core/run/world/data/enterprise_*.dat`. Allowed are the views' and records' (terrain,
   depots, dials, names). Any other (caravans, contracts, mints, territory, settlements, …) is a Java rule that wrote
   state in core mode: name it and stub it.
7. Findings go into `docs/binding-progress.md` "Now", each with a ledger excerpt.

## 6. Tasks

Briefs paste `docs/binding-plan.md` 3.1's common brief first. CPU rules:
- Game tests one tree at a time, through `python3 scripts/trees.py check NAME --java` (machine-wide slots,
  `trees.py heavy`), never two Gradle servers at once.
- Rust shares ~/.cargo/config.toml's 4 slots.
- No compiling while a headless server or lanes run: they load classes from the build folder.

| # | Task | What | Files | Gates | Model | Waits for |
|---|---|---|---|---|---|---|
| A1 | `alpha-ledger` | `LedgerAdapter` (existing queries into `LedgerView` lines), the Ledger tab, and `/enterprise ledger <desk>`, registered from a new `LedgerCommand` (Brigadier merges it under `enterprise`) so `EnterpriseCommands` is untouched. One game test: a desk with emeralds and goods, a look, the ledger's Money and Goods lines match `core cash`/`core stock`. | new `binding/adapters/LedgerAdapter.java`, `merchant/LedgerView.java`, `LedgerCommand.java`, `test/LedgerTests.java`; `MerchantView`, `MerchantScreen`, `Enterprise.java` (one line), lang file | `just build`; `trees.py check alpha-ledger --java` (gametest and gametest-core no fall); no wire change (`just wire-check` unchanged schema) | sonnet / high | nothing: its files are disjoint from the B4c trees; start now |
| A2 | `alpha-stubs` | Section 2's table: the refusal words, core mint rate on the screen, no `lockersAt` fallback, crate message, `Settlements::onDeath` off in core mode, the paused notice on join (also when the core didn't start). | `CommandsAdapter`, `EnterpriseCommands` (only if a refusal moves), `MerchantView`, `Merchant`, `CrateBlock`, `Enterprise`, `CoreHost` (a getter only) | as A1, plus a game test for the paused notice (a host opened over a refused save) | sonnet / medium | the B4c trees landed (they edit `CommandsAdapter`/`EnterpriseCommands`/`CoreHost`); after A1 (shares `MerchantView`) |
| A3 | `alpha-build` | Core default, world mode marker, `prism_sync.py` config, version strings, `/enterprise core version` and `core report`, `docs/alpha.md`, welcome panel lines; testbench `network/settlement.mcfunction` stops calling `enterprise cottage` (refused in core mode): beds by `setblock`. | `binding/Society.java`, new `binding/WorldMode.java`, `CoreHost.onStarted` (one call), `gradle.properties`, `pack/pack.toml`, `scripts/prism_sync.py`, `LedgerCommand` (adds the two), `dev/testbench/…`, `docs/alpha.md` | `just build`; `trees.py check alpha-build --java`; the jar-in-jar listing; `python3 scripts/prism_sync.py` against a scratch instance dir if the script allows, else read-only review | sonnet / high | A1 (`LedgerCommand`); lands after A2 |
| A4 | `alpha-smoke` | Section 5 end to end; fix what's small and in the binding (a crash, a stub missed, a Java rule found writing); file the rest in "Now"; commit the ledger excerpts and the study's CSV summary to `docs/alpha-smoke.md`. | whatever breaks; `docs/binding-progress.md`, `docs/alpha-smoke.md` | section 5's pass list; any fix re-gated with `trees.py check alpha-smoke --java` | opus / high | A1, A2, A3 landed; no other Java tree running during the 30 days (about 10 to 15 minutes of server) |

Each is a few hours. A1 can start today beside the B4c trees. A2 and A3 follow their landings. A4 is the alpha's gate,
and its findings (likely the field refusal loop, the shops' coin before acceptance, whatever the first long run turns
up) are the next tasks, chosen by what the ledger shows.

## For John to decide

1. **When to start.** On main after the four B4c trees land, before `delete-java-rules`, as planned here. Or now, on
   `be4bf7b`, with fewer features (no freight or exchange vendors, no tolls or mine heads)?
2. **Core as the default for every process**, `just server` and studies included (Java-mode studies then need
   `ENTERPRISE_SOCIETY=java`)? Or only for the Prism instance's config?
3. **The ledger on the Merchant's screen.** A desk without a Merchant has only the command. Or should a ledger item open
   it at any desk? That means a new item, recipe and menu.
4. **No save promise for the alpha**, with a newer build free to refuse an older world. Confirm, and say when the
   promise starts (binding-plan 6.2).
5. The "for John to confirm" rules a player will now meet:
   - lockers emptied into the desk at each look;
   - every emerald above a reserve of 0 sold to the mint at a desk's first count;
   - stopping a route winds it down;
   - the census never taking a lost profession.

### One tree at a time (John, 2026-10-23)

The four fixture trees are parked, not finished first: A1 to A4 run one after another with no other tree beside them.
This replaces "after the B4c trees" below.

### Decided (integrator, 2026-10-23, on John's go-ahead for the alpha and the in-game screen)

1. As planned: A1 now; A2 and A3 after the four B4c trees land; `fixtures-post` and `delete-java-rules` held until
   John has played the alpha.
2. Core is the default for every process (the studies are core-only since `commands-and-studies`); Java mode only by
   `ENTERPRISE_SOCIETY=java` or the game tests' own runs. The world mode marker keeps a world in the mode it began in.
3. The ledger is a Merchant's-screen tab plus `/enterprise ledger <desk>`; a ledger item waits for what the alpha shows.
4. No save promise in the alpha: a newer build may refuse an older world, and says so. When the promise starts is
   John's, after he has played it.
5. The four rules above ship as they are and are listed in `docs/alpha.md` as "judge these while you play".
