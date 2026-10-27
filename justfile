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

# The gates for what changed since origin/main (scripts/check.py): `just check`, `--only A,B`, `--full` before pushing
check *flags:
    python3 scripts/check.py {{flags}}

# Run game tests headless; `just gametest LedgerTests,LockerTests` runs only those classes
gametest only="":
    rm -rf core/run/gametest/world  # a fresh world each run: saved data (the mint, trust, contracts) would carry over
    ./gradlew :core:runGameTestServer -PwasmProfile=wasm-dev {{ if only != "" { "-Ponly=" + only } else { "" } }}

# The same game tests with the Rust core running the society (society = core): passes per class against
# scripts/gametest-core-baseline.txt, failing if one that passed there falls. Its own folder, core/run/gametest-core.
gametest-core only="":
    rm -rf core/run/gametest-core/world core/run/gametest-core/report.tsv  # no last run's passes if this one never starts
    -./gradlew :core:runGameTestServerCore -PwasmProfile=wasm-dev {{ if only != "" { "-Ponly=" + only } else { "" } }}
    python3 scripts/gametest_core.py core/run/gametest-core/report.tsv {{ if only != "" { "--only=" + only } else { "" } }}

# Rewrite the core-mode baseline from a fresh run (when tests newly pass; never to excuse a fall)
gametest-core-baseline:
    rm -rf core/run/gametest-core/world core/run/gametest-core/report.tsv  # no last run's passes if this one never starts
    -./gradlew :core:runGameTestServerCore
    python3 scripts/gametest_core.py core/run/gametest-core/report.tsv --write-baseline

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

# A survival world for the dev server: normally generated from SEED, no testbench (its market is the flat world's), the
# same ports. Then `just server` and `dev/experiments/survival_study.py`.
server-setup-survival seed:
    @scripts/server-check
    mkdir -p core/run
    echo "eula=true" > core/run/eula.txt
    printf '%s\n' 'level-type=minecraft\:normal' 'level-seed={{seed}}' 'enable-rcon=true' 'rcon.password=dev' 'rcon.port=25598' 'online-mode=false' 'spawn-protection=0' 'difficulty=normal' 'server-ip=127.0.0.1' 'server-port=25599' > core/run/server.properties
    rm -rf core/run/world
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

# The surveyor's map of the dev server's world, block by block, with the run's wagons (core mode; after the server stops):
# TRACE is the run's `enterprise-sim --trace` database, LOG the server's log (its forums' bells)
render trace log out="world_survey.html":
    python3 dev/experiments/world_render.py core/run/world {{trace}} {{log}} {{out}}

# A town's plan as GIS data for the asset studio (docs/assets/README.md, "GIS export"); --region adds its region files
gis trace log town out *flags:
    python3 dev/experiments/gis_export.py core/run/world {{trace}} {{log}} {{town}} {{out}} {{flags}}

# A film of a survey page's caravans, each trip on its own days, from the first (an MP4 Discord plays inline);
# needs Pillow (run with a venv's python)
film survey out="world_survey.mp4" python="python3":
    {{python}} dev/experiments/world_video.py {{survey}} {{out}}

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
    cd sim && cargo test --release -q -p enterprise-core --test all -- fingerprint:: --ignored --nocapture 2>&1 | grep '^fingerprint'

# The 200-day replay built for WebAssembly (wasm32-wasip1, under Node's WASI): its digest must be the native one's
core-wasm-replay:
    cd sim && CARGO_TARGET_WASM32_WASIP1_RUNNER="node $PWD/wasi-run.mjs" cargo test --release -q -p enterprise-core --target wasm32-wasip1 --test replay

# The core as a WebAssembly module for the Minecraft binding: enterprise_core.wasm in sim/target, size and imports told
wasm:
    cd sim && cargo build --profile wasm -q -p enterprise-wasm --target wasm32-wasip1 --target-dir target
    cp sim/target/wasm32-wasip1/wasm/enterprise_wasm.wasm sim/target/enterprise_core.wasm
    @ls -l sim/target/enterprise_core.wasm | awk '{print "enterprise_core.wasm: " $5 " bytes"}'
    @node -e 'const m=new WebAssembly.Module(require("fs").readFileSync("sim/target/enterprise_core.wasm")); console.log("imports:", WebAssembly.Module.imports(m).map(i=>i.module+"."+i.name).join(" ")); console.log("exports:", WebAssembly.Module.exports(m).map(e=>e.name).join(" "))'

# The module's tests where the target and node are required, not skipped: imports, and the proof's requests in Node's WASI
wasm-test:
    cd sim && ENT_WASM_REQUIRED=1 cargo test --release -q -p enterprise-wasm

# The wire's golden files (sim/wire/fixtures/) and scenarios (sim/scenarios/) written anew: run it after a wire type or a rule moved, commit the diff
wire-fixtures:
    cd sim && WIRE_FIXTURES=write WIRE_SCENARIOS=write cargo test --release -q -p enterprise-core --test wire_fixtures --test scenarios
    cd sim && cargo test --release -q -p enterprise-core --test wire_fixtures --test scenarios

# The scenarios and the golden files built for WebAssembly, under Node's WASI: the same answers as native
scenarios-wasm:
    cd sim/core && CARGO_TARGET_WASM32_WASIP1_RUNNER="node $PWD/../wasi-run.mjs" cargo test --release -q -p enterprise-core --target wasm32-wasip1 --test scenarios --test wire_fixtures

# The wire's registry and Java, generated from the core's wire types (sim/core/src/wire): run after changing one. `--renumber` lets an enum's variants move
wire-gen *args:
    cd sim && cargo run --release -q -p enterprise-wire-gen -- {{args}}

# Fails while sim/wire/registry.yaml or binding/src/generated aren't what the core's wire types generate
wire-check:
    cd sim && cargo run --release -q -p enterprise-wire-gen -- --check

# The generated Java compiled alone (javac --release 21, no Gradle), then its codec run on the core's own bytes
wire-javac:
    cd sim && cargo test --release -q -p enterprise-wire-gen --test java -- --ignored --nocapture

# The binding's JVM tests, no Minecraft (Gradle :binding:test, docs/binding-plan.md 1.5): wire-check and the codec check first, then every golden file through the generated Java, every scenario through Chicory (compiled and interpreted) and a trap at every step
binding-test:
    ./gradlew :binding:test --console=plain

# The binding's spike (docs/binding-plan.md section 4): look scripts from the study host, played natively, under Node and in the JVM (the same bytes back from each); tables in binding/build/spike.md
binding-spike:
    cd sim && cargo run --release -q -p enterprise-sim -- 30 --wire-looks target/spike/study.looks | tail -1
    cd sim && cargo run --release -q -p enterprise-sim -- --world 1234 150 target/spike/world.looks | tail -1
    just wasm
    cd sim && for w in study world; do cargo run --release -q -p enterprise-wasm --example looks -- target/spike/$w.looks target/spike/$w.native.out target/spike/$w.native.times && ENT_TIMES=target/spike/$w.node.times node wasi-run.mjs --reactor target/enterprise_core.wasm target/spike/$w.looks target/spike/$w.node.out && cmp target/spike/$w.node.out target/spike/$w.native.out && echo "$w: Node's responses are native's" || exit 1; done
    ./gradlew :binding:spike -Pspike=heap --console=plain -q
    ./gradlew :binding:spike --console=plain -q

# Sections of the spike again, on the scripts the last `just binding-spike` wrote (a busy machine stretches wall times): `just binding-spike-again looks,saveload`
binding-spike-again sections:
    ./gradlew :binding:spike -Pspike={{sections}} --console=plain -q

# The network study's world in the fast host, no game: `just study 365 --csv /tmp/base.csv --river --people-use`
study *args:
    cd sim && cargo run --release -q -p enterprise-sim -- {{args}}

# How far the core matches the mod's game tests: `just parity --left` lists what's not covered yet
parity *flags:
    python3 scripts/parity.py {{flags}}
