# Core progress: where the parity work stands

The live to-do list for building the Rust core to parity (the plan is `docs/core-plan.md`; how trees work is
`docs/worktrees.md`). Updated by the integrator at each landing. Last update: 2026-10-17, after wave 1a's seventh
landing.

## Now

- **Main:** `46d39b2`. 74 tests pass (`just core-test`, a few seconds). Parity: **35 covered, 13 partial, 46
  skipped, 168 left of 262** (`just parity`).
- **One tree open: `cash-book`** (`.trees/cash-book`, branch `tree/cash-book`, commit `9df807c`). Done and passing
  in its tree, but 10 commits behind main. **Land it next**, expecting conflicts (it migrates every money move in
  every rule file to `cash_book::post`):
  - `sim/Cargo.toml`/`Cargo.lock`: take main's.
  - `sim/core/tests/random_worlds.rs`: apply its additions to main's version (towns spending through the Accountant
    and direct posts; each town's book explaining its treasury over days 90 to 120). The generator now lives in
    `sim/core/tests/common/mod.rs`.
  - The rule files: `people.rs` and the new `rules/roster.rs` (roster changed births), `society.rs`, `check.rs`.
    Re-run the fingerprint idea: behaviour must not change; every money change must go through `post`.
  - `save.rs`: add `cash_book: crate::cash_book::CashBook` to the `saved!` resources.

## Wave 1a: 7 of 8 landed

| Task | Model | Landed | Notes |
|---|---|---|---|
| `look-steps` | haiku | `769ce90` | Steps Work, Pay, Review, Mail. Bevy chains at most 20 sets: two chains joined at Sell |
| `parity-triage` | haiku | `aa7270e` | Skip list (46 lines, 47 tests); two trade-loop tests (one tagged partial: no crates yet) |
| `contract-tags` | haiku | `793a47e` | Five ContractTests; `Incoterm::price` (no caller yet) |
| `mint-rate` | haiku | `1e8465f` | `Mint` resource; fixed the same-seed test (it compared nothing) |
| `save-load` | opus | `69241f1` | One blob (postcard); dials by name; random worlds save and load at day 60. The mint added to the save at landing |
| `roster` | opus | `8d621f9` | Hands with trade, level, XP; babies; villagers follow it. Roster added to the save at landing |
| `hex-map` | opus | `46d39b2` | Hexes, ground, A* roads, tolls as in `postToll`. Map and `RoutePath` added to the save at landing |
| `cash-book` | opus | — | Done in its tree; land next (above) |

## Next

1. **Land `cash-book`** (above).
2. **Fix the conflict magnets before wave 1b** (each landing so far conflicted in the same three places):
   - `sim/core/tests/random_worlds.rs` and `tests/common/mod.rs`: every slice extends the one generator. Give each
     slice its own file in `tests/common/` (a `fn extend(s: &mut Society, r: &mut Rng, ...)`) called from `world()`,
     and its own checks in a list, so a slice adds a file and one line.
   - `sim/core/Cargo.toml` / `Cargo.lock`: briefs should say "serde and postcard are already there; don't add
     dependencies without the integrator".
   - `sim/core/src/save.rs`'s `saved!` list: agents must add their state there (three slices forgot, because
     save-load landed after they branched). Add a test that fails when a component or resource in the world isn't
     in `saved!` (spawn a world with every module, save, load, compare component counts).
   - Update `docs/worktrees.md`'s rules and the common brief in `docs/core-plan.md` section 3 to match.
3. **Wave 1b** (all wait for `cash-book`; `advances-and-refunds` also for `save-load`, done):
   `bargain-and-par` (sonnet/high), `advances-and-refunds` (sonnet/medium), then `road-food` (opus/medium, after
   `bargain-and-par`: both edit `arrive.rs`). Briefs: extract from `docs/core-plan.md` (the script used: each task's
   "Brief:" code block, after section 3's common brief), `python3 scripts/trees.py new NAME --brief-file FILE`.
4. **Waves 2 to 5** as `docs/core-plan.md` section 5's table: start a task when its "waits for" have landed.
5. **Integrator checks at each landing:** read the diff; check `parity:` tags really pin the Java test (mark
   `parity-partial:` otherwise); check new state is in `saved!`; `just tree-land NAME` (rebases, gates, pushes);
   `just tree-rm NAME`; update this file.

## Parked

- `park/warp` (branch): a fast-forward mode for the NeoForge server, untested. For after parity, when the binding
  work brings the Java server back.

## Lessons from wave 1a

- Haiku did the mechanical tasks well and fast (1 to 3 minutes each); Opus the structural ones in 6 to 8 minutes.
- Agents flagged their own weak spots honestly (a tag that claimed too much; a float-sensitive test); read their
  summaries before landing.
- Slices branched before a structural one landed miss its conventions (the save list): land structural tasks first,
  and check new state against `saved!` at landing.
