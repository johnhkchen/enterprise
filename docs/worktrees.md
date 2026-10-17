# Worktrees: agents side by side

Several agents build the Rust core at once, each in a worktree of its own, landed on main one at a time. In this
phase the core is built and vetted without Java: nobody builds or runs the mod. Its source is read as the
reference, and its game tests are the spec.

## The loop

| Who | Does | With |
|---|---|---|
| The integrator | Makes a tree with a brief | `just tree-new NAME "brief"` (or `scripts/trees.py new NAME --brief-file FILE`) |
| The agent | Works only in `.trees/NAME`, commits there | `cd .trees/NAME`; the brief is `.tree/TASK.md` |
| The agent | Checks its own work | `just core-test` in the tree (seconds); `just parity` |
| The integrator | Lands it | `just tree-land NAME`: rebase onto main, the gates, fast-forward, push |
| The integrator | Clears it | `just tree-rm NAME` |
| Anyone | Sees every tree | `just trees` |

## Rules for an agent in a tree

1. **Stay in your tree.** Edit only under `.trees/NAME`. Never commit on main, never push, never land yourself.
2. **Gates before you call it done:** `cd sim && cargo test --release` passes; nothing else is a gate in this phase.
3. **Every behaviour gets a test, and a test that matches a mod game test says so:** `// parity: CaravanTests.aTownPromisesOnlyWhatItSells` above it (`parity-partial:` if only in part). `just parity` counts them. A game test that's Minecraft's own (blocks, entities, the client) goes in `scripts/parity-skip.txt` with its reason.
4. **The checks hold every day:** a new mechanism keeps goods and money conserved (`check.rs`): every money transfer goes through `cash_book::post` (or `post_legs` for one payment in several legs), never `money.0 -=`. Add your mechanism to the random worlds through a file of your own, `sim/core/tests/common/slices/SLICE.rs` (`extend` for a new world, `daily` for a host's doings, `check` for your invariants), and one line in `tests/common/slices.rs`, so 300 seeds exercise it. Don't edit `random_worlds.rs` or the base generator in `tests/common/mod.rs`.
5. **Determinism:** no clock, no randomness except a seeded generator in the world's state, no `HashMap` iteration in a rule (use `BTreeMap`, or a `Vec`). The same inputs give the same world to the bit. A refactor that mustn't change behaviour shows `just core-fingerprint` the same before and after.
6. **Your slice in your own files.** Its data and components in `sim/core/src/SLICE.rs`, its systems in `sim/core/src/rules/SLICE.rs` with a `register` function, its host calls in an `impl Society` block in its own module (the `Society` fields are `pub(crate)`), its tests in `sim/core/tests/SLICE.rs`. Then a line each in the shared lists (below).
7. **Read the mod, don't run it.** The Java under `core/src/main/java/dev/b28/enterprise/` is the reference; `docs/decisions.md` says why it is as it is. Port the rule and its constants; cite the Java file in your module's doc comment.
8. **Everything that's state is saved:** every new component or resource derives `serde::{Serialize, Deserialize}` and `Default` and gets a line in `saved!` in `sim/core/src/slices.rs` (a resource a new society starts with as `name: Type = Type::default(),`). `tests/save.rs` fails while a world holds anything that isn't listed.
9. **No new dependencies.** `sim/Cargo.toml`, `sim/core/Cargo.toml` and `Cargo.lock` are the integrator's: serde, postcard and bevy_ecs are there. If you need a crate, say so in your summary and work without it.
10. **Commit messages** follow the repo's style (what changed and why, measured), ending with the two attribution lines.

## Shared files

**Lists merged by union** (`.gitattributes`). Two trees adding a line each never conflict, so add lines only, one per thing. A line changed or taken out would come back beside its new self at landing: changes to lines already there are the integrator's.

| File | What goes in |
|---|---|
| `sim/core/src/slices.rs` | Your state in `saved!` (a resource, a town's or a route's component); your `register` call |
| `sim/core/src/dials.rs` | Your dials, one line each (`name: type = the mod's default,` under a doc comment), as a block under a comment naming your slice, at the end of the list |
| `sim/core/src/lib.rs` | A `pub mod`, a `pub use` |
| `sim/core/tests/common/slices.rs` | Your slice's name, for its generator file `tests/common/slices/SLICE.rs` |
| `scripts/parity-skip.txt` | A Java test that's Minecraft's own, with its reason |

**Edited by hand, additions only,** in your own lines; don't reorder or reformat others':

| File | What goes in |
|---|---|
| `sim/core/src/rules/mod.rs` | A `pub mod`. A new `Step` only if no existing one fits |
| `sim/core/src/state.rs` | Fields on shared components (with `Default`). Your own components go in your module |
| `sim/core/src/society.rs` | Only what every slice shares; your host calls go in your module's `impl Society` |
| `sim/core/src/rules/arrive.rs`, `plan.rs` | A wagon's steps: the plan says which tasks edit them, landed one after another |

**Not edited in trees:** `docs/decisions.md` and `docs/core-progress.md` (the integrator writes them up at landing), `Cargo.toml`/`Cargo.lock`, `tests/random_worlds.rs`.

## Ergonomic pains, and what's been done

**Fixed:**
1. **Tracked Python bytecode** (`__pycache__/*.pyc`) dirtied every tree on every run and would have conflicted. Untracked.
2. **One rules file every slice edited** (`rules.rs`: arrivals, departures, the schedule). Split into steps a module joins from its own file (`rules/`). A route's wagon plan is built step by step, and `Decide` trims it to what's still in the warehouse.
3. **Heavy Java gates.** A Gradle daemon (2 GB), a game-test server (about 2 GB) and lanes (1.5 GB each) on a 24 GB machine. Off in this phase; `trees.py check --java` and `heavy` (machine-wide slots) for when they're back.
4. **Trees outside the project's folder** would have had their own Claude memory (it's keyed by path) and permissions. Trees now live in `.trees/` inside the repo, ignored by git and by ripgrep.
5. **Checkpoints are 10 GB,** and trees don't need them now. Linked from the main tree only for `--java` trees.
6. **The same four conflicts at every landing in wave 1a** (`Cargo.toml`/`Cargo.lock`, the random worlds' one generator, `society.rs`'s new resources, the `saved!` list, which three slices forgot). Now: the lists slices add to are files of their own merged by union (`slices.rs`, `dials.rs`, `lib.rs`, the tests' `slices.rs`); each slice's part of the random worlds is a file of its own with a random stream of its own; host calls go in the slice's module; a test fails while state isn't saved; dependencies are the integrator's.

**Still there:**
1. **Hand-edited shared files remain:** `state.rs` (fields on shared components), `rules/mod.rs` (a new step), `arrive.rs` and `plan.rs` (a wagon's steps, several tasks). Union merging is only for pure lists: a slice that edits code there conflicts as before.
2. **A tree branches from the last commit:** work not committed on main isn't in it.
3. **A cold Rust build per tree:** 13 seconds the first time (bevy_ecs), 3 after; 150 MB of `target/` per tree. `sccache` would share it; not installed.
4. **The reference is big:** `Caravans.java` alone is 2,600 lines. Briefs should name the Java methods to read, not whole files, or agents spend their context reading.
5. **Parity is self-reported:** a `parity:` tag is the agent's claim. The integrator checks a tag's test really pins the Java test's behaviour before landing.
6. **`f64` money** replays to the bit on one machine, not necessarily across hosts (WebAssembly in Java against native). Fixed-point is planned before the Minecraft binding (`docs/core-math.md`).
7. **Landing is one at a time,** and a rebase can stop on a conflict in a shared file. Land small and often.
8. **Agent tool worktrees:** Claude Code's own `isolation: "worktree"` makes throwaway worktrees too. Use `.trees/` for named work that lands; the tool's for quick, disposable tries.
