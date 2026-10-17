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
4. **The checks hold every day:** a new mechanism keeps goods and money conserved (`check.rs`): money into or out of the world goes through the ledger (`income`, `road`, `spent`). Add your mechanism to the random worlds (`tests/random_worlds.rs`) so 300 seeds exercise it.
5. **Determinism:** no clock, no randomness except a seeded generator in the world's state, no `HashMap` iteration in a rule (use `BTreeMap`, or a `Vec`). The same inputs give the same world to the bit.
6. **Join a step from your own file.** A slice's systems go in `sim/core/src/rules/SLICE.rs` with a `register` function, and its data in `sim/core/src/SLICE.rs`. The shared files (below) take additions only, in their own lines, so landings merge cleanly.
7. **Read the mod, don't run it.** The Java under `core/src/main/java/dev/b28/enterprise/` is the reference; `docs/decisions.md` says why it is as it is. Port the rule and its constants; cite the Java file in your module's doc comment.
8. **Commit messages** follow the repo's style (what changed and why, measured), ending with the two attribution lines.

## Shared files: additions only

These are where landings meet. Add in your own lines; don't reorder or reformat others':

| File | What goes in |
|---|---|
| `sim/core/src/rules/mod.rs` | A `pub mod`, a `register` call. A new `Step` only if no existing one fits |
| `sim/core/src/state.rs` | New components and fields (with `Default`) |
| `sim/core/src/content.rs` | New dials, with their defaults from the mod |
| `sim/core/src/society.rs` | New host calls (`Society::...`) |
| `sim/core/src/lib.rs` | A `pub mod` |
| `docs/decisions.md` | Not edited in trees: the integrator writes it up at landing, from the commit messages |

## Ergonomic pains, and what's been done

**Fixed:**
1. **Tracked Python bytecode** (`__pycache__/*.pyc`) dirtied every tree on every run and would have conflicted. Untracked.
2. **One rules file every slice edited** (`rules.rs`: arrivals, departures, the schedule). Split into steps a module joins from its own file (`rules/`). A route's wagon plan is built step by step, and `Decide` trims it to what's still in the warehouse.
3. **Heavy Java gates.** A Gradle daemon (2 GB), a game-test server (about 2 GB) and lanes (1.5 GB each) on a 24 GB machine. Off in this phase; `trees.py check --java` and `heavy` (machine-wide slots) for when they're back.
4. **Trees outside the project's folder** would have had their own Claude memory (it's keyed by path) and permissions. Trees now live in `.trees/` inside the repo, ignored by git and by ripgrep.
5. **Checkpoints are 10 GB,** and trees don't need them now. Linked from the main tree only for `--java` trees.

**Still there:**
1. **Shared files still collect additions:** `state.rs`, `content.rs` (one `Dials` for all), `society.rs`. Two trees adding a dial each will merge, but by hand if they touch the same lines. Next step if it bites: a dials struct per module.
2. **A tree branches from the last commit:** work not committed on main isn't in it.
3. **A cold Rust build per tree:** 13 seconds the first time (bevy_ecs), 3 after; 150 MB of `target/` per tree. `sccache` would share it; not installed.
4. **The reference is big:** `Caravans.java` alone is 2,600 lines. Briefs should name the Java methods to read, not whole files, or agents spend their context reading.
5. **Parity is self-reported:** a `parity:` tag is the agent's claim. The integrator checks a tag's test really pins the Java test's behaviour before landing.
6. **`f64` money** replays to the bit on one machine, not necessarily across hosts (WebAssembly in Java against native). Fixed-point is planned before the Minecraft binding (`docs/core-math.md`).
7. **Landing is one at a time,** and a rebase can stop on a conflict in a shared file. Land small and often.
8. **Agent tool worktrees:** Claude Code's own `isolation: "worktree"` makes throwaway worktrees too. Use `.trees/` for named work that lands; the tool's for quick, disposable tries.
