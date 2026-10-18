# Enterprise: Create-based trading modpack.

# packwiz installs to ~/go/bin via `go install`
export PATH := env_var('HOME') + "/go/bin:" + env_var('PATH')

default:
    @just --list

# Build the core mod jar (core/build/libs)
build:
    ./gradlew :core:build

# Launch a dev client with only the core mod (fast iteration)
run:
    ./gradlew :core:runClient

# Launch a dev server
server:
    ./gradlew :core:runServer

# Run game tests headless
gametest:
    rm -rf core/run/gametest/world  # a fresh world each run: saved data (the mint, trust, contracts) would carry over
    ./gradlew :core:runGameTestServer

# Build, then create/update the "Enterprise (dev)" Prism instance with the full pack
prism: build
    python3 scripts/prism_sync.py

# Add a Modrinth mod to the pack, e.g. `just add jei`
add slug:
    cd pack && packwiz modrinth add {{slug}} -y

# Refresh the pack index after hand-editing pack/
refresh:
    cd pack && packwiz refresh

# Configure the headless dev server: flat world, localhost only (game 25599, RCON 25598), testbench installed (wipes core/run/world)
server-setup:
    # A previous dev server must finish saving first (it holds the world's session lock until then), or its world
    # bleeds into the new one. Not the RCON port: other apps on this machine may hold it. Fails after 60s, not never.
    @scripts/server-check
    mkdir -p core/run
    echo "eula=true" > core/run/eula.txt
    printf '%s\n' 'level-type=minecraft\:flat' 'enable-rcon=true' 'rcon.password=dev' 'rcon.port=25598' 'online-mode=false' 'spawn-protection=0' 'difficulty=peaceful' 'server-ip=127.0.0.1' 'server-port=25599' > core/run/server.properties
    rm -rf core/run/world
    mkdir -p core/run/world/datapacks
    cp -R dev/testbench core/run/world/datapacks/enterprise-testbench
    @date +%s > core/run/.last-setup

# The dev server's world put back as a study checkpoint left it (`network_study.py --save-at`), to fork a run from there
server-restore checkpoint:
    @scripts/server-check
    @test -d "{{checkpoint}}/world" || (echo "no world in {{checkpoint}}: checkpoints are core/run/checkpoints/LABEL/dayN" >&2; exit 1)
    rm -rf core/run/world
    cp -R "{{checkpoint}}/world" core/run/world
    @date +%s > core/run/.last-setup
    @echo "restored {{checkpoint}}: start the server, then run the study with --resume {{checkpoint}}"

# A lane is a dev server of its own in core/run/lanes/N, on game port 25599+2N and RCON 25600+2N, so runs go side by
# side. Start it with `just lane N`; drive it with ENTERPRISE_LANE=N set (rcon.py, network_study.py).
# Lane N's world put back as a study checkpoint left it
lane-restore lane checkpoint:
    @test -d "{{checkpoint}}/world" || (echo "no world in {{checkpoint}}" >&2; exit 1)
    @! lsof core/run/lanes/{{lane}}/world/session.lock >/dev/null 2>&1 || (echo "lane {{lane}}'s server is still running: ENTERPRISE_LANE={{lane}} just rcon stop" >&2; exit 1)
    mkdir -p core/run/lanes/{{lane}}
    rm -rf core/run/lanes/{{lane}}/world core/run/lanes/{{lane}}/config
    cp -R core/run/config core/run/lanes/{{lane}}/config
    echo "eula=true" > core/run/lanes/{{lane}}/eula.txt
    printf '%s\n' 'level-type=minecraft\:flat' 'enable-rcon=true' 'rcon.password=dev' "rcon.port=$((25600 + 2 * {{lane}}))" 'online-mode=false' 'spawn-protection=0' 'difficulty=peaceful' 'server-ip=127.0.0.1' "server-port=$((25599 + 2 * {{lane}}))" > core/run/lanes/{{lane}}/server.properties
    cp -R "{{checkpoint}}/world" core/run/lanes/{{lane}}/world

# Run lane N's server (see lane-restore)
lane lane:
    ./gradlew :core:runServer -Plane={{lane}}

# Send commands to the running dev server, e.g. `just rcon "time set day"`
rcon +commands:
    python3 scripts/rcon.py {{commands}}

# The cartographer's map of the dev server's world: towns, hubs, and routes by trips (after the server stops).
map out="world_map.html" title="The realm, by its cartographer":
    python3 dev/experiments/world_map.py core/run/world/data {{out}} "{{title}}"

# Worktrees for agents side by side (docs/worktrees.md): `just tree-new NAME "brief"`, then work in .trees/NAME
tree-new name brief="":
    python3 scripts/trees.py new {{name}} --brief "{{brief}}"

# Every tree: ahead/behind main, uncommitted changes, last commit
trees:
    python3 scripts/trees.py list

# A tree's gates (the Rust core's tests; the Java mod isn't built in this phase)
tree-check name="":
    python3 scripts/trees.py check {{name}}

# Rebase a tree onto main, check it, fast-forward main and push
tree-land name:
    python3 scripts/trees.py land {{name}}

# Remove a landed tree (--force drops unlanded work)
tree-rm name *flags:
    python3 scripts/trees.py rm {{name}} {{flags}}

# The Rust core's tests (seconds)
core-test:
    cd sim && cargo test --release -q

# One hash of 300 random worlds' 120 days: a refactor that mustn't change behaviour compares it before and after.
core-fingerprint:
    cd sim && cargo test --release -q --test fingerprint -- --ignored --nocapture 2>&1 | grep '^fingerprint'

# The 200-day replay built for WebAssembly (wasm32-wasip1, under Node's WASI): its digest must be the native one's
core-wasm-replay:
    cd sim && CARGO_TARGET_WASM32_WASIP1_RUNNER="node $PWD/wasi-run.mjs" cargo test --release -q -p enterprise-core --target wasm32-wasip1 --test replay

# The network study's world in the fast host, no game: `just study 365 --csv /tmp/base.csv --river --people-use`
study *args:
    cd sim && cargo run --release -q -p enterprise-sim -- {{args}}

# How far the core matches the mod's game tests: `just parity --left` lists what's not covered yet
parity *flags:
    python3 scripts/parity.py {{flags}}
