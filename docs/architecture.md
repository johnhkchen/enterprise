# Architecture: core, binding, engines

Enterprise is a society for a factory game: towns, merchants and a crown who need what you make, bid for it, trust
you or don't, grow because of you, and turn to you in a war. It isn't a crafting game. It's meant to plug into any
open enough one (Create in Minecraft first, Factorio next) and give a single-player factory somewhere to send its
goods and someone to care.

## Three layers

| Layer | What it is | Written in |
|---|---|---|
| **Core** | The society: towns, goods, money, contracts, trust, wagons, the post, the crown, war. The rules, the state, the save format, the message protocol | Rust, once (`sim/core`) |
| **Binding** | One game's glue: its happenings into messages for the core, the core's results into things in the game (a shop's stock, a train stop's orders, a contract on a board). Owns that game's content pack: its goods, their worth, its trade points, its recipes | The game's own language: Java for Minecraft, Lua for Factorio |
| **Engine** | The game itself: where code may run, how mods ship, who's in charge in multiplayer, how saves work | Not ours |

A binding stays thin: if it grows rules, they belong in the core.

## Where the core runs

| Way | Engines | How |
|---|---|---|
| **Inside** | Minecraft (Java), Unity and Unreal games, Godot, Bevy | Minecraft: the core compiled to WebAssembly and turned into Java classes (Chicory), one jar, nothing native. Others: a native library, or a Rust crate |
| **Beside** | Factorio (sandboxed Lua: no native code, no network) | A companion program talking through RCON and files. Or, if fast enough, a WebAssembly interpreter in Lua: an open question, to be measured |
| **As a service** | Roblox (Luau; servers can call web services) | A service the game's server calls. A business of its own, only if we want it |

## What that asks of the core

1. **It builds for WebAssembly** (it does: `cargo build -p enterprise-core --target wasm32-wasip1`). So no clock, no files, no threads, no network.
2. **Its whole interface is messages:** happenings in (`Input`), results out (`Output`), the same whether it's inside, beside or remote. Logged, they replay.
3. **Its state is one blob the game saves.** Reload an older save and the core goes back with it.
4. **The game passes time in; the core never asks for it.** The same inputs give the same world, to the last bit.
5. **Content packs are data, checked by the core:** a binding's goods, values and trade points.

## How it's built

On Bevy's ECS (`bevy_ecs`, single-threaded, no rendering): towns and routes are entities, the rules are systems run
in a fixed order each look. `Society` wraps a world of them for hosts that aren't Bevy apps; a Bevy app (a viewer,
or a game of our own) can run the same systems in its own world.

## How we develop it

| Host | For | Speed |
|---|---|---|
| **The Rust host** (`sim/host`, `cargo test`) | The rules: tests that pin each, checks that stop a run (goods and money conserved, nobody in debt), thousands of days, sweeps | The trade loop's tests in 0.01s; 10,000 days of a 9-town map in half a second |
| **NeoForge, run faster** | The binding: the real game, real Create, confirming what the Rust host found | Minutes |

The Rust host is a fast stand-in for the Java server, not a copy of it. Java runs vary a point of mood between runs
of the same code, so the bar is agreeing within that spread; its job is catching problems before they reach the
slow engine.

## Conventions for every binding

1. **Messages only:** a binding never touches the core's insides.
2. **The game passes time in;** the core says what happened.
3. **Trade points are registered things** (a shop, a train stop, a dock): deliveries and pickups are messages
   about them.
4. **The core's state lives in the game's save,** as one blob.
5. **Every binding passes the same scenario scripts:** scripted games ("deliver 400 plates on day 3; the contract
   pays on day 7") that the Rust host runs in seconds and a binding runs in its engine.

## The order

1. The core's trade loop, with its tests and the Rust host (started: `sim/`).
2. The rest of the rules move from the Java mod into the core, a slice at a time: contracts and bills, the
   forwarder and the post, the crown and war, towns' growth and mood. Today's mod is the reference: the same
   scenario in both, any gap a finding.
3. The Minecraft binding: the mod calls the core (WebAssembly through Chicory) for each slice that's moved, its game
   tests green after each.
4. Factorio: inside (a Lua interpreter) or beside (a companion), decided by a measurement.

## Already learned from the core

- **Fewer, fuller wagons need buyers with cash.** A buyer's money goes round: what it pays a wagon comes back as its
  people buy what it bought. Daily wagons meet that money every day; one every four days meets it once. With 20
  emeralds a mill takes as much from four-day wagons as from daily ones (97%); with 2 it takes 61%, however its
  budget is counted. Consolidating freight needs working capital, credit or consignment
  (`a_buyer_short_of_cash_takes_less_from_fewer_wagons`; `cargo run --release --example budget`).
