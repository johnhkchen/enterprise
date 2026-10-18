# Binding progress: where the Minecraft binding stands

The live to-do list for the Minecraft binding (the plan is `docs/binding-plan.md`; how trees work is
`docs/worktrees.md`). Updated by the integrator at each landing. Last update: 2026-10-18, `chicory-host` landed.

## Now

- **Main:** `d48478f`. 762 tests pass (`cd sim && cargo test --release`); `just core-wasm-replay` matches native;
  `just scenarios-wasm` 3 + 22 under Node's WASI; `just wasm-test` 4/4 with the module required; `just wire-check`
  passes (schema `0c8b594f0b2eba2a`, 24 files); `just wire-javac`: WireCheck passes 215 checks; `just binding-test`:
  87 JVM tests pass (Chicory compiled and interpreted, every golden file, every scenario, a trap at every step). Game
  tests: All 262 required tests passed (`trees.py check --java`: settings.gradle now includes `binding`). `just
  core-fingerprint`: `cb418063bb32364e` (unchanged by this landing).
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
- **Not planned yet:** `content-weights` (a `weight` field at the end of `PackGood`; `Good.weight` is on main, and
  `Content::from_pack` gives every good the default weight) and `pallet-views` (the mod's crates and chests drawing
  pallets from the core's messages). Both are named in `docs/decisions.md` (2026-10-18) but have no row below.
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
| B1.7 | `versioning` | sonnet / high | `chicory-host` | | |
| B2.1 | `proof-in-mod` | opus / high | `versioning` | | |
| B3.1 | `gametest-triage` | sonnet / medium | — | `59b7b77` | `parity.py --binding` writes `docs/binding-tests.md`: 150 fixture, 66 rule API, 46 own; 2 rule-API tests with only a partial twin go to the core first. At landing: helpers followed across test files and a discarded entity counted (5 tests moved to fixture: BuildingTests' three and CountingHouseTests' clerks via `LabourTests.town`, `GrowthTests.childrenGrowUpWhileAway`); rule-API tests that pass the level to a rule marked *level* (27); the plan's section 2 counts updated |
| B3.2 | `core-mode` | sonnet / medium | `proof-in-mod`, `gametest-triage` | | |
| B4.1 | `content-pack` | sonnet / high | `core-mode` | | |
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
