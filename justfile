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

# Send commands to the running dev server, e.g. `just rcon "time set day"`
rcon +commands:
    python3 scripts/rcon.py {{commands}}

# The cartographer's map of the dev server's world: towns, hubs, and routes by trips (after the server stops).
map out="world_map.html" title="The realm, by its cartographer":
    python3 dev/experiments/world_map.py core/run/world/data {{out}} "{{title}}"
