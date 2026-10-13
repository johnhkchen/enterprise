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

# Configure the headless dev server: flat world, localhost only (game 25566, RCON 25576), testbench installed (wipes core/run/world)
server-setup:
    mkdir -p core/run
    echo "eula=true" > core/run/eula.txt
    printf '%s\n' 'level-type=minecraft\:flat' 'enable-rcon=true' 'rcon.password=dev' 'rcon.port=25576' 'online-mode=false' 'spawn-protection=0' 'difficulty=peaceful' 'server-ip=127.0.0.1' 'server-port=25566' > core/run/server.properties
    rm -rf core/run/world
    mkdir -p core/run/world/datapacks
    cp -R dev/testbench core/run/world/datapacks/enterprise-testbench

# Send commands to the running dev server, e.g. `just rcon "time set day"`
rcon +commands:
    python3 scripts/rcon.py {{commands}}
