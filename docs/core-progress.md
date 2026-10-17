# Core progress: where the parity work stands

The live to-do list for building the Rust core to parity (the plan is `docs/core-plan.md`; how trees work is
`docs/worktrees.md`). Updated by the integrator at each landing. Last update: 2026-10-17, wave 1a landed and the
conflict magnets fixed; waves 1b to 5 starting as one workflow.

## Now

- **Main:** `66af5d0`. 107 tests pass (`just core-test`, a few seconds). Parity: **46 covered, 13 partial, 46
  skipped, 157 left of 262** (`just parity`). `just core-fingerprint`: `98348d1c9b26721a`.
- **Waves 1b to 5 run as one workflow:** each task gets its own tree when everything it waits for has landed, and
  landings are one at a time, the earliest in the plan's order first among those done.

## Waves 1b to 5

Models re-decided before starting (16 Opus, 16 Sonnet, no Haiku): Opus where the work is structural, cross-cutting or
subtle; Sonnet for well-specified ports. The plan's choice is in brackets.

| Order | Wave | Task | Model / effort (plan's) | Waits for | Landed | Notes |
|---|---|---|---|---|---|---|
| 9 | 1b | `bargain-and-par` | opus/high (sonnet/high) | `cash-book` | — | edits sell and plan_sale, the busiest rules, and moves measured ratios |
| 10 | 1b | `advances-and-refunds` | sonnet/medium (sonnet/medium) | `cash-book`, `save-load` | `66af5d0` | 2 ContractTests pinned; CaravanTests advance partial (inputs follow is sourcing's). At landing: refunds before advances (mod's order), the seller's wagon collects at the buyer (was missing), both before bills |
| 11 | 1b | `road-food` | opus/high (opus/medium) | `cash-book`, `bargain-and-par` | — |  |
| 12 | 2 | `lockers` | opus/high (sonnet/high) | `roster`, `cash-book` | — | structural: rostered towns' income; five later tasks build on it |
| 13 | 2 | `squads` | opus/high (sonnet/high) | `roster`, `road-food` | — | threads a crew through Plan, Decide and Turn beside road-food |
| 14 | 2 | `farms-and-wages` | opus/high (sonnet/high) | `roster`, `cash-book` | — | the largest port; wages and the labour market touch money and people |
| 15 | 2 | `jobs` | sonnet/medium (sonnet/medium) | `roster`, `lockers` | — |  |
| 16 | 2 | `territory-growth` | sonnet/medium (sonnet/medium) | `hex-map`, `cash-book` | `4aa28bd` | Claims, culture to two rings, guards, militia, tolls; 7 tags. At landing: food all or nothing, tolls floored in spurs, posts by q then r, a first count a day's culture |
| 17 | 2 | `workshop` | sonnet/medium (sonnet/medium) | `roster` | — |  |
| 18 | 2 | `harvest-and-rents` | sonnet/medium (sonnet/medium) | `advances-and-refunds` | — |  |
| 19 | 2 | `atlas-and-books` | opus/high (sonnet/medium) | `hex-map`, `squads` | — | stocktakes change what every plan reads |
| 20 | 2 | `herds` | sonnet/medium (sonnet/medium) | `roster`, `workshop` | — |  |
| 21 | 3 | `chokepoints` | opus/high (opus/medium) | `territory-growth` | — |  |
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
| 37 | 4 | `settlers-rule` | sonnet/medium (haiku/low) | `roster`, `hex-map` | — | haiku only for trivial edits |
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
