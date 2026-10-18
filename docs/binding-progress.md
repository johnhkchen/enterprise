# Binding progress: where the Minecraft binding stands

The live to-do list for the Minecraft binding (the plan is `docs/binding-plan.md`; how trees work is
`docs/worktrees.md`). Updated by the integrator at each landing. Last update: 2026-10-18, `wire-types` landed.

## Now

- **Main:** `a91fab3`. 674 tests pass (`cd sim && cargo test --release`); `just core-wasm-replay` matches native.
  Game tests: not run at this landing (core-only; no Java changed). `just core-fingerprint`: `8144f9be7947133b`
  (unchanged).
- **The wire is in** (`sim/core/src/wire/`): `wire::handle(&mut Option<Society>, Request) -> Response` and
  `handle_bytes` (postcard). `SCHEMA` is a hand-bumped `1` until `wire-gen`: bump it by hand at every wire change
  until then. `SaveFile` is checked only for its format (`save::FORMAT` stays 1 while dev worlds are rebuilt).
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
- `just wire-check` and `just binding-test` don't exist yet (`wire-gen`, `chicory-host`).

## Tasks

| Order | Task | Model / effort | Waits for | Landed | Notes |
|---|---|---|---|---|---|
| B1.1 | `wasm-digest` | opus / high | — | `c8fbc0a` | Every ambiguous system pair pinned as it ran natively; `tests/schedule.rs` guards new ones |
| B1.2 | `wire-types` | opus / high | `wasm-digest` | `a91fab3` | `wire/`: Request/Command/Response/Refusal, Input/Output moved there (re-exported from `messages.rs`), Query/Answer (`Stock`, `Points`), ContentPack and `Content::from_pack`, Point, SaveFile; a look is checked whole before any of it applies. Beyond the brief: trade points as core state (`points.rs`, in `saved!`) and four inputs, with a `trade_points` random-worlds slice. At landing: rebased onto pallet-types (digest written again); an `Advance` more than 36,500 days past now refused (`u64::MAX` would have looped without end); `RemovePoint`/`Stocktake` naming another kind than the point there refused. Left: SCHEMA by hand (`wire-gen`), fixtures (`wire-fixtures`), re-placed point counts twice (`stock-adapter`) |
| B1.3 | `wasm-exports` | sonnet / medium | `wire-types` | | |
| B1.4 | `wire-gen` | opus / high | `wire-types` | | |
| B1.5 | `wire-fixtures` | sonnet / medium | `wire-types` | | |
| B1.6 | `chicory-host` | opus / high | `wasm-exports`, `wire-gen`, `wire-fixtures` | | |
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
