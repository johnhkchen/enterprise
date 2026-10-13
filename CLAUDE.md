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

## Commands (`just`)
- `just build`: build the core jar.
- `just run`: dev client with only the core mod.
- `just prism`: build, then sync the Prism instance with the full pack. Launching from Prism re-copies the latest jar.
- `just add <modrinth-slug>`: add a mod to the pack.

## Toolchain
- Gradle runs on the repo's mise Java (23); Gradle's foojay resolver downloads JDK 21 for compiling.
- packwiz is installed via `go install github.com/packwiz/packwiz@latest` (lands in `~/go/bin`).

## Design rules (from the decisions doc)
- Compute on touch, never per tick. Store a few numbers per batch, node or population.
- Nothing crosses distance without a carrier: goods, coins and price news all travel.
- Save data is the source of truth for anything that must work while unloaded (lockers, merchants, caravans); blocks and entities are views onto it.
