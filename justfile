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
