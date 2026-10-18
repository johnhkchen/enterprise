# Binding progress: where the Minecraft binding stands

The live to-do list for the Minecraft binding (the plan is `docs/binding-plan.md`; how trees work is
`docs/worktrees.md`). Updated by the integrator at each landing. Last update: 2026-10-19, `content-pack` landed.

## Now

- **Main:** `f48f053`. 788 tests pass (`cd sim && cargo test --release`); `just core-wasm-replay` matches native;
  `just scenarios-wasm` passes; `just wire-check` passes (schema `e66ad7c365d84fd0`, 29 files); `just binding-test`
  passes (122). Game tests: "All 267 required tests passed" in Java mode, and `just gametest-core`: 267 of 267 against
  the baseline of 267. `just core-fingerprint`: `96b225213f5f291c` (unchanged).
- **The content pack** (B4.1, `binding/ModPack`): the mod's goods, comfort kinds, trades, recipes (`PackRecipe`),
  herds (`PackAnimal`), each good's stack weight (`PackGood.weight`, which was `content-weights`) and dials
  (`PackDial`) cross as one `ContentPack`; `Content::from_pack` names everything wrong at once. The pack's dials are
  the world's start (`Content.dial_defaults`, saved; `reset` returns to them). `Input::SetDial`, `Query::Dials`.
  `/enterprise dial` in core mode lists and sets the core's (`adapters/DialAdapter`); in Java mode it sets the Java
  dial and the core follows (and at start, for what the world had set). `Content::good` now matches a name with or
  without the host's namespace, so in core mode the core's bread, wheat and hay_block rules and the vanilla
  `default_offers` find the mod's goods (they found nothing before), and pallet caps bind on the pack's weights.
  **For John to confirm:** the weights (`ModPack.WEIGHTS`: ore, ingots and logs 2 a stack, stone and sand 3, the rest
  1); the mod's `birth_mood` 0.5 goes in the pack and the core's default stays 1.0. **Open:** 7 mod dials have no core
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
  in one mode and then the other advances the Java SavedData and `enterprise_core.dat` apart, unsaid. Open: the whole
  `CensusTaker` poller is off in core mode, its counting half (villagers, beds, workstations around a bell) included,
  which `people-adapter` needs as the roster's source; no JVM test covers `Society.parse`.
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
  drift (for `people-adapter`). `CoreHost.of` ignores its server (one host a JVM).
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
- **Open for `stock-adapter`:** a point broken and placed again counts its chests from nothing at its first
  stocktake, so what they held is delivered to the town twice (free goods for a player; the books still balance,
  because it comes from outside). Decide it there, with `Moved` and write-back. Also: chests that lose more than the
  town holds drop the excess with no record (`withdraw`); `Query::Stock` answers as last touched, not caught up.
- **Not planned yet:** `pallet-views` (the mod's crates and chests drawing pallets from the core's messages; the
  weights it needs are in the pack now, `content-weights` having folded into `content-pack`). Named in
  `docs/decisions.md` (2026-10-18) but has no row below.
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
| B4.2 | `points-and-hands` | opus / high | `content-pack` | | |
| B4.3 | `stock-adapter` | opus / xhigh | `points-and-hands` | | |
| B4.4 | `money-adapter` | sonnet / high | `stock-adapter` | | |
| B4.5 | `people-adapter` | opus / high | `points-and-hands` | | |
| B4.6 | `locker-adapter` | opus / high | `stock-adapter`, `people-adapter` | | |
| B4.7 | `land-adapter` | opus / high | `people-adapter` | | |
| B4.8 | `wagon-adapter` | opus / high | `locker-adapter`, `land-adapter`, `money-adapter` | | |
| B4.9 | `shop-adapter` | sonnet / high | `money-adapter`, `wagon-adapter` | | |
| B4.10 | `crown-views` | sonnet / medium | `wagon-adapter`, `shop-adapter` | | |
| B4.11 | `commands-and-studies` | sonnet / medium | `wagon-adapter` | | |
| B5.1 | `delete-java-rules` | opus / high | all of B4 | | |
| B5.2 | `scenarios-in-game` | sonnet / medium | `delete-java-rules` | | |
| B5.3 | `upgrade-drill` | sonnet / medium | `delete-java-rules` | | |
