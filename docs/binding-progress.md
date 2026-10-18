# Binding progress: where the Minecraft binding stands

The live to-do list for the Minecraft binding (the plan is `docs/binding-plan.md`; how trees work is
`docs/worktrees.md`). Updated by the integrator at each landing. Last update: 2026-10-19, `stock-adapter` landed.

## Now

- **Main:** `d19bb85`. 852 tests pass (`cd sim && cargo test --release`); `just core-wasm-replay` matches native;
  `just wire-check` passes (schema `a7833bce12ce3273`, 31 files); `just scenarios-wasm` and `just binding-test` pass
  (190 JVM tests). Game tests: "All 280 required tests passed" in Java mode, and `just gametest-core`: 280 of 280
  against the baseline of 280. `just core-fingerprint`: `5e6b6d51a3b4061e` (unmoved by `stock-adapter`).
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
  look until the roster shows the name taken. Five outputs around an `Advance`: `Born`, `Settled`, `Departed`,
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
  free/confined split isn't counted (`land-adapter`); a mode switch would spawn the core's births beside Java
  `Growth`'s; vanilla-bred babies join as the core's children (two sources of births, as in Java mode); GrowthTests,
  JobsTests, LabourTests and ExpeditionTests still call the Java rules (`Jobs`, `Labour`, `Households`, `Growth`,
  `Expeditions` stay until `delete-java-rules`); two parity-partial tags added (GrowthTests.aContentTownWithASpareBedHasAChild,
  ExpeditionTests.aMerchantSettlesAVillageNobodyTradesWith). The save grew at FORMAT 1: dev worlds'
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
| B4.4 | `money-adapter` | sonnet / high | `stock-adapter` | | |
| B4.5 | `people-adapter` | opus / high | `points-and-hands` | `2874c6b` | The census as a town's roster's source (`census.rs`, `Sightings` saved): `Census`, `TagHand`, `Resident`; outputs `Born`, `Settled`, `Departed`, `HandGone`, `HandLevel`; `PeopleAdapter` counts loaded towns (48 blocks round the bell, no path checks), spawns villagers for hands with none and writes profession, level and XP; `CensusTaker`'s counting half shared; `/enterprise core census <desk>`; the first Output scenario (`people`); 3 game tests (272 both modes), fingerprint unchanged, schema `e0b54576b14c0176`. At landing: a villager not loaded and last seen where nothing is loaded sent as the core has it (it was aged out and came back a new hand, breaking lockers and clients); names sent with every look until the roster takes them (a refused look lost them and hands were spawned twice); `/enterprise census` refuses in core mode; rebased onto consignment-pallets (digest written again); decisions.md "People and the binding". Left: settlers' views and founded towns' desks (`land-adapter`), away hands' outputs (`wagon-adapter`), census beds overwriting the core's homes, no free/confined split, births across a mode switch, the Java people rules and their tests until `delete-java-rules`, John to confirm the census never takes `none` or the Merchant's trade |
| B4.6 | `locker-adapter` | opus / high | `stock-adapter`, `people-adapter` | | |
| B4.7 | `land-adapter` | opus / high | `people-adapter` | | |
| B4.8 | `wagon-adapter` | opus / high | `locker-adapter`, `land-adapter`, `money-adapter` | | |
| B4.9 | `shop-adapter` | sonnet / high | `money-adapter`, `wagon-adapter` | | |
| B4.10 | `crown-views` | sonnet / medium | `wagon-adapter`, `shop-adapter` | | |
| B4.11 | `commands-and-studies` | sonnet / medium | `wagon-adapter` | | |
| B4.12 | `pallet-views` | opus / high | `stock-adapter`, `content-weights` | | Placed at `points-and-hands`' landing: the mod's crates and chests drawn from the core's pallet outputs, no pallet rules in Java; `PackPallets`, `Good.package`/`.handling` and the manifest on the wire |
| B5.1 | `delete-java-rules` | opus / high | all of B4 | | |
| B5.2 | `scenarios-in-game` | sonnet / medium | `delete-java-rules` | | |
| B5.3 | `upgrade-drill` | sonnet / medium | `delete-java-rules` | | |
