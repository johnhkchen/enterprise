# Enterprise

A trade-oriented modpack for Minecraft 1.21.1 (NeoForge), built around Create and Numismatics.
Monorepo: our mods, the pack definition, and design docs.

## Read first
- `docs/decisions.md`: settled design decisions. Wins over the brief where they disagree.
- `docs/brief.md`: the original design brief (written under the old name, Staples).

## Layout
- `core/`: the core mod (`mod_id=enterprise`, package `dev.b28.enterprise`). NeoForge ModDevGradle.
- `pack/`: packwiz pack definition, the source of truth for which mods ship.
- `scripts/prism_sync.py`: mirrors `pack/` plus the built core jar into the Prism instance "Enterprise (dev)".
- Agents side by side: worktrees in `.trees/NAME` (`just tree-new`, `just trees`, `just tree-land`), each with a brief in `.tree/TASK.md`; how they work, the shared files and the known pains: `docs/worktrees.md`. While the core comes to parity it's built and vetted without Java: the mod is read, not run; `just parity` counts which of its game tests the core's tests cover.
- `sim/`: the Enterprise core in Rust (`sim/core`, on Bevy's ECS) and its fast host (`sim/host`): the society with no game in it, for any engine to host; see `docs/architecture.md`. `cd sim && cargo test --release` (seconds: the core's tests are one binary, `--test all -- lockers::` for one file's; the random-worlds sweeps run a tenth of their worlds unless `SWEEP=full`); `cargo run --release -p enterprise-sim -- 10000` runs 10,000 days and checks the books every day.
- `dev/testbench/`: datapack synced into the instance's `testing` world. On load it builds a standing test market near spawn if missing (desk at 27 -60 27, bell at 27 -60 24 with a bed per villager so it's a settlement, 18 shulkers, Blaze Banker, selling trades switched on once villagers settle) and shows a welcome panel on every join. `/function testbench:help` has the rest (market build/clear anywhere, time warps). Headless servers build the same fixture; spawn chunks don't tick without a player, so `forceload add -21 -24 75 72` to run it: a census counts a town only with everything within 48 blocks of its bell loaded (a smaller forceload runs the market with 0 people).
- `dev/experiments/`: measurement scripts that drive the headless server over RCON: `merchant_ramp.py` (one market's ramp), `caravan_network.py` (hub warehouse supplying three settlements by caravan), `network_study.py` (days of the whole network to CSV; in core mode it switches on offers villagers unlock every 4 days, as a player would; `--comforts`, `--river`, `--river-bridge`, `--fleet`, `--services`, `--metal`, `--trades`, `--master`, `--war M,W,P`, `--inject DAY`, `--add-hand TOWN:TRADE`: a villager put in a town on the first day, `TOWN:none` the fair side to fork against; core mode takes `--realms` too: the Coast's tobacco by sea to the hub), `standalone_town.py` (one settlement left to itself).
- Forking a study: `--save-at 0,30 --label NAME` copies the world after those days (0: after setup) to `core/run/checkpoints/NAME/dayN`; `just server-restore core/run/checkpoints/NAME/dayN`, start the server, then the same flags with `--resume core/run/checkpoints/NAME/dayN` carry on from day N+1. A ridge study's stages: boot 5s, setup 90s, then 4s a day rising to 8s by day 40. A checkpoint holds for code changes that touch nothing before its day.
- Runs side by side: in Java mode one run is noise (moods swing a point between runs of the same code), so compare three. Under the core a fork from a checkpoint is deterministic (three lanes come out identical to the item): one lane is the run, and two builds compare exactly. Each town rolls its own dice (`state::Dice`), so a change in one town doesn't re-roll another's draws (the jobs draw, the Merchant's XP). `python3 dev/experiments/forks.py CHECKPOINT LABEL 3 OUTDIR -- 120 [study flags]` runs the fork on lanes 1-3, each a server of its own in `core/run/lanes/N` (game 25599+2N, RCON 25600+2N; `just lane-restore N CHECKPOINT`, `just lane N`, `ENTERPRISE_LANE=N` for rcon.py and the study). Three lanes take about 25 minutes for 120 days. Don't compile while lanes run: their servers load classes from the build folder.
- Tuning from a fork: settings a study tunes are dials (`core/.../Dials.java`, one catalogue): `/enterprise dial` lists them, `/enterprise dial <name> <value>` (or `reset`) sets one for that world, saved with it. `forks.py ... OUTDIR --dial 2:harvest_window_days=6 --dial '*:dealer_spread=0.2' -- DAYS ...` sets them per lane before its study begins (each lane's settings in OUTDIR/LABEL-N.dials), so lanes from one build try different values with no recompiling. DAYS is the day to run to: from a day-120 checkpoint, `-- 140` runs 20 days (about 5 minutes for three lanes).
- How the paper did: `python3 dev/experiments/paper_report.py OUTDIR LABEL [--base CHECKPOINT]` reads forward sales, rents' coupons and rents from each lane's last checkpoint (no server); `--base` counts only what a fork wrote after it started.
- Why a town went broke: every emerald in and out of a market's treasury is in its cash book (`/enterprise cashbook <desk> [days]`), by what for and by day, reconciled against its opening balance (what the book can't explain is printed: a few emeralds is normal). `--stop-when any.treasury<5` (or `E2.treasury<5`, `S3.mood<0`, `E3.villagers>=12`) ends a study at the first day a town meets it, saves that day's checkpoint and prints the town's cash book: a broke town in two minutes, not a 120-day run.
- Following one town while a lane runs: `python3 dev/experiments/trace.py LANE TOWN --every 3 --out FILE` (read-only; start it once forks.py says the lane's server is up): its farm plan, idle plots, crops sold forward, mood, and its contracts and rents with their latest history, every few game days.
- Each Merchant's reasons: `/enterprise whys <desk>` (and at the end of `enterprise books`) tallies why goods were sent or not, why the far end wouldn't buy, why no caravan left (and why each one went: `went: full`, `on its day`, `goods owed, by porter`...), and what its workshop could make but is short of. The study prints them for every market. Each route also keeps how full its caravans really ran (`loadOut`/`loadHome`: full stacks of goods, all trips told; `slotsOut`/`slotsHome`: slots taken) in `enterprise_caravans.dat`.
- Tracing a run (core mode): the server writes every request its core took to `world/enterprise-journal/<time>.ejl`
  (one a server run, the last 10 kept; `journal=off` in `config/enterprise.properties` or `ENTERPRISE_JOURNAL=off` for
  none; `/enterprise core journal` says where). The core is deterministic, so `cd sim && cargo run --release -p
  enterprise-sim -- --trace FILE run.db` replays it exactly and writes SQLite: `requests`, `inputs`, `outputs`, each
  day's `towns` (with mood's target, comforts met, paid share, land), `comforts` (met by kind), `herds`, `hands` (each town's roster), `works`, `farms`, `stock`, `routes`, `lockers`, `whys`, every decided `plans` (load worth vs road cost, purse, went or
  stayed) with `plan_whys` (why each good went or stayed, stays too), each wagon's `trips` (out: stacks, worth, purse;
  there: sold, takings, bought, paid; pallet items either way), and `cash`. A trip's worth counts only what it took to
  sell: a buying trip's goods are in `bought` and `pallet_items`. Then `sqlite3 run.db`. The tables:
  `sim/host/src/trace.rs`. A journal replays only on the core of its wire schema.
- Long runs stall when the Mac sleeps: a closed lid on battery sleeps it even under `caffeinate`, and the server freezes mid-tick. The study reports a day that takes six times the usual; `pmset -g log | grep Sleep` confirms it.

## Commands (`just`)
- `just build`: build the core jar.
- `just run`: dev client with only the core mod.
- `just prism`: build, then sync the Prism instance with the full pack. Launching from Prism re-copies the latest jar.
- `just add <modrinth-slug>`: add a mod to the pack.
- `just check`: the gates for what changed since origin/main (scripts/check.py): the touched Rust test targets, a compile, the changed game test classes (`--only A,B` for more). `just check --full` runs everything; once, before pushing, not while building.
- `just gametest [A,B]`: run the in-game tests headless (only those classes when named) (`core/src/main/java/dev/b28/enterprise/test/`).
- Headless dev server: `just server-setup` (flat world, testbench, wipes `core/run/world`; first says when it was last set up and whether a server still holds the world, and fails after 60s rather than waiting on one), then `just server` in the
  background, then `just rcon "<command>" ...` to drive it. It runs the core (the default since the alpha;
  `ENTERPRISE_SOCIETY=java` for the Java rules), and a world keeps the mode it began in (`world/enterprise_society.txt`):
  opened in the other mode, the core pauses and says why. It listens on localhost only: game 25599, RCON 25598.
  Other local servers use 25565/25575 (claude_plays_mc_create) and 25566/25576 (redshelf): never point RCON at those.

## Game test gotchas
- Game tests run in their own folder, `core/run/gametest`: before, they opened the dev server's `core/run/world` and inherited its ledgers (a war's debased mint failed two market tests). Look for "All N required tests passed". `just gametest` wipes its world first: saved data (the mint's debasement, trust, contracts) would otherwise carry over between runs.
- Mock server players (`makeMockServerPlayerInLevel`) break on Numismatics' login packets; make logic take positions or plain `makeMockPlayer`.
- In dev environments Numismatics trusts only players wearing golden boots; its real trust rule can't be tested here.
- Test coordinates start at the structure block, one below the arena: the `floor` template's floor is y=1.
- Tests run side by side: keep world searches inside the 7x7 arena (range 4 from the centre).
- Don't leave unemployed or deskless trained villagers in an arena (discard them): they wander off to claim neighbouring tests' workstations. Don't plant crops outside your arena: farm tests count every crop in their hex.
- `helper.relativePos` doesn't invert `absolutePos` for rotated tests: compare positions in world space.

## Toolchain
- Gradle runs on the repo's mise Java (23); Gradle's foojay resolver downloads JDK 21 for compiling.
- packwiz is installed via `go install github.com/packwiz/packwiz@latest` (lands in `~/go/bin`).

## Design rules (from the decisions doc)
- Compute on touch, never per tick. Store a few numbers per batch, node or population.
- Nothing crosses distance without a carrier: goods, coins and price news all travel.
- Save data is the source of truth for anything that must work while unloaded (lockers, merchants, caravans); blocks and entities are views onto it.
