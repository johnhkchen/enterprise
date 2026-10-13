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
- `dev/testbench/`: datapack synced into the instance's `testing` world. On load it builds a standing test market near spawn if missing (desk at 27 -60 27, bell at 27 -60 24 with a bed per villager so it's a settlement, 18 shulkers, Blaze Banker, selling trades switched on once villagers settle) and shows a welcome panel on every join. `/function testbench:help` has the rest (market build/clear anywhere, time warps). Headless servers build the same fixture; spawn chunks don't tick without a player, so `forceload add 19 19 35 35` to run it.
- `dev/experiments/`: measurement scripts that drive the headless server over RCON: `merchant_ramp.py` (one market's ramp), `caravan_network.py` (hub warehouse supplying three settlements by caravan).

## Commands (`just`)
- `just build`: build the core jar.
- `just run`: dev client with only the core mod.
- `just prism`: build, then sync the Prism instance with the full pack. Launching from Prism re-copies the latest jar.
- `just add <modrinth-slug>`: add a mod to the pack.
- `just gametest`: run the in-game tests headless (`core/src/main/java/dev/b28/enterprise/test/`).
- Headless dev server: `just server-setup` (flat world, testbench, wipes `core/run/world`), then `just server` in the
  background, then `just rcon "<command>" ...` to drive it. It listens on localhost only: game 25599, RCON 25598.
  Other local servers use 25565/25575 (claude_plays_mc_create) and 25566/25576 (redshelf): never point RCON at those.

## Game test gotchas
- Mock server players (`makeMockServerPlayerInLevel`) break on Numismatics' login packets; make logic take positions or plain `makeMockPlayer`.
- In dev environments Numismatics trusts only players wearing golden boots; its real trust rule can't be tested here.
- Test coordinates start at the structure block, one below the arena: the `floor` template's floor is y=1.
- Tests run side by side: keep world searches inside the 7x7 arena (range 4 from the centre).
- `helper.relativePos` doesn't invert `absolutePos` for rotated tests: compare positions in world space.

## Toolchain
- Gradle runs on the repo's mise Java (23); Gradle's foojay resolver downloads JDK 21 for compiling.
- packwiz is installed via `go install github.com/packwiz/packwiz@latest` (lands in `~/go/bin`).

## Design rules (from the decisions doc)
- Compute on touch, never per tick. Store a few numbers per batch, node or population.
- Nothing crosses distance without a carrier: goods, coins and price news all travel.
- Save data is the source of truth for anything that must work while unloaded (lockers, merchants, caravans); blocks and entities are views onto it.
