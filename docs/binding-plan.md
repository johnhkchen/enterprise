# Binding plan: the Minecraft mod on the Rust core

The phase after parity: the NeoForge mod (`core/`, Java) stops running its own society and calls the Rust core
(`sim/core`) through WebAssembly in the JVM (Chicory). Divided into tasks an integrator hands to agents in trees
(`docs/worktrees.md`), as `docs/core-plan.md` was.

**The measure of success is how cheaply the binding follows the core.** The core keeps changing (pallets, value over
time and more are next: `handling-units-after-parity`). So the layer comes first and is built to be upgraded: one
boundary, generated from the Rust types; a version on everything that crosses it; one adapter per Minecraft concern;
contract tests that run without Minecraft. Moving the mod's rules over comes after, a concern at a time. Section 5
says how an upgrade goes and how long it should take.

## 0. What the plan rests on (checked 2026-10-18)

| Fact | Checked how | What it means |
|---|---|---|
| **Chicory 1.7.5** (24 Mar 2026) is current: a pure-JVM WebAssembly runtime, no native code, no JNI. It has an interpreter, a runtime compiler and a **build-time compiler** that turns wasm into JVM bytecode (Maven plugin official; Gradle through the community `at.released.wasm2class.plugin` 0.5.0, marked experimental). WASI preview 1 (`com.dylibso.chicory:wasi`) is supported except sockets, `fd_fdstat_set_rights`, and per-process/thread CPU clocks | [releases](https://github.com/dylibso/chicory/releases), [build-time compiler](https://chicory.dev/docs/usage/build-time-compiler), [WASI](https://chicory.dev/docs/usage/wasi), [wasm2class](https://github.com/illarionov/wasm2class-gradle-plugin) | One jar, nothing native, as `architecture.md` says. A wasm function too big for one JVM method (64 KB of bytecode) makes the build-time compiler **fail by default**; `interpreterFallback = WARN` runs those functions in the interpreter. Bevy's schedule and our large `match`es may hit it: the spike lists them |
| Chicory's speed: the project says performance isn't a primary goal and publishes no benchmarks | its README; no third-party numbers found | Measured in task `chicory-host`, not assumed. Estimates below **Measured (section 4): compiled, 38x native on the study world (2.3 ms a look), 17x on a 25-town world; the look runs on a thread of its own** |
| The core builds for `wasm32-wasip1`; the study host does too (`enterprise-sim.wasm`, 6.7 MB; the replay test binary 7.3 MB, release, unstripped) | built here | The cdylib with only the exports will be smaller; measured in `wasm-exports` |
| **Study world: native 1.16 ms a day, Node's V8 on the same wasm 1.79 ms a day (1.54x), identical output** (365 days, 9 towns, 14 routes; main at `37f56f9`) | `enterprise-sim 365` native and under `wasi-run.mjs` | A JIT on this wasm costs about half again. Chicory's build-time compiler goes through the JVM's JIT: estimate 2 to 5x native, 2.5 to 6 ms a day, **0.13 to 0.3 ms a look** (20 looks a day). Its interpreter: estimate 20 to 100x, 1.2 to 6 ms a look. Either fits a game: one look a minute of play |
| **`just core-wasm-replay` fails on main.** At `37f56f9` (plan-on-departure) the WebAssembly digest differs from the native one for seeds 77, 1234 and 2026 (seed 3 and the curves match). `76ea783`, `f92903b` and `cb8b2e3` pass; the commit's message says it matched. The wasm result is the same run to run: one build is deterministic, the two builds disagree | bisected here, both builds | Every contract test below compares native and wasm. Fix first (task `wasm-digest`). Likeliest cause: the new `squad::unmanned` system left an order Bevy resolves differently in the two builds (the lessons in `core-progress.md` met ambiguous orders before); else something that depends on `usize` being 32 bits |
| Messages today: `Input` (8 kinds) and `Output` (5) in `messages.rs`, serde-derived. Everything else a host does is a Rust call: about 330 `pub fn`s in `impl Society` blocks across 28 modules | `messages.rs`, grep | The boundary can't be those calls. It is a few exports carrying versioned messages; the wire adds only what an adapter needs, each through an existing host call |
| The save: `Society::save()` is postcard after `ENTS` and `FORMAT = 1`, not self-describing; a blob of another format is refused. Replay worlds save 170 to 265 KB | `save.rs`, the replay's output | The blob is opaque to Java. Migrations need the old shapes kept (section 1.4) |
| **bincode is unmaintained** (RUSTSEC-2025-0141, Dec 2025: 1.3.3 final). serde-generate (0.34.1, June 2026, the `serde-reflection` family) generates Java but only for bincode and BCS. typeshare has no Java | [advisory](https://rustsec.org/advisories/RUSTSEC-2025-0141), [serde-reflection](https://github.com/zefchain/serde-reflection), [typeshare](https://github.com/1Password/typeshare) | Messages stay postcard. The schema comes from serde-reflection, our generator writes the Java (section 1.2) |
| The mod: 149 files, 31,084 lines (24,700 outside `test/`); 33 `SavedData` classes; 13 server-tick listeners in `Enterprise.java`; Numismatics in 23 files; no direct Create API calls (Create reaches the mod through chests, hoppers and belts) | read | Most of the mod is rules. The views are chests, villagers, blocks, coins and screens |
| The 262 game tests: about 121 build a world (blocks, villagers, chests: `setBlock`/`spawn`/`absolutePos` in the body), about 141 call a Java rule's API on data (`new Mint().sell(...)`, `ContractBook.get(server).review(...)`) | a script over `test/*.java` | Not all 262 can be acceptance tests unchanged. The rule-API ones are pinned by their Rust twins already (`parity.py`) and retire with the Java class; the world ones are the binding's acceptance tests (section 2) |

## 1. The shape

```
Minecraft events ──> adapters (one per concern) ──> Inputs ──┐
                                                             │  ent_call(Request) ─> core.wasm (Chicory, AOT)
Minecraft views  <── adapters <── Outputs, Answers <─────────┘  <─ Response
                     the blob <── Save ── saved with the world (enterprise_core.dat, one file)
```

### 1.1 The ABI: four exports, fixed

A new crate `sim/wasm` (cdylib, `wasm32-wasip1`, `panic = "abort"`) wraps the core. It exports four functions and
never more:

| Export | Does |
|---|---|
| `ent_abi() -> u32` | The ABI's number (1). Bumped only if these four change |
| `ent_alloc(len: u32) -> u32` | A buffer in the module's memory for a request |
| `ent_call(ptr: u32, len: u32) -> u64` | Takes a `Request` (postcard), returns a `Response` at `ptr << 32 \| len` |
| `ent_free(ptr: u32, len: u32)` | Frees a buffer either side made |

One module instance holds one society (one world). Every call is on the server thread; Chicory instances aren't
shared. Imports are only what Rust's std needs on `wasm32-wasip1` for an abort's message (`fd_write`,
`proc_exit`, `environ_*`); a test lists the module's imports and fails on any other, `random_get` and the clocks
above all (they would break determinism).

The envelope:

```rust
struct Request  { abi: u32, schema: u64, command: Command }
enum Command    { Create { pack: ContentPack, seed: u64 }, Load { save: SaveFile }, Save,
                  Apply { inputs: Vec<Input> }, Ask { queries: Vec<Query> } }
enum Response   { Done { outputs: Vec<Output>, answers: Vec<Answer> }, Saved { save: SaveFile },
                  Refused { why: Refusal } }
```

`Apply` takes a look's happenings and its `Advance` in one call: one crossing a look, not one per event. A refusal
(no society yet, a schema or format it doesn't know, a bad pack) is a message. A panic aborts: Chicory throws, the
binding drops the instance, logs the `Request` that did it, and loads the last save in a fresh instance with the
inputs since replayed (`CoreHost` keeps them: the input log, section 1.5). A bug report is the save plus that log.

### 1.2 The messages: Rust types are the source, the Java is generated

**Wire types live in `sim/core/src/wire/`**, apart from the state: `Request`, `Command`, `Response`, `Input`,
`Output`, `Query`, `Answer`, `ContentPack`, `Point`, `SaveFile`, and the small structs they carry. They hold numbers,
strings, ids and vectors of each, never a state type: `TownView` today holds `People`, a state struct, so a state
change would ripple to Java; the wire's `Answer::Town` holds its own fields. A test walks the registry and fails on
any wire type defined outside `wire/`. So the core's state can change shape every day and the Java never knows;
only a change to the wire regenerates it. `messages.rs`'s `Input` and `Output` move into `wire/`.

**Format: postcard**, the save's own format, already a dependency and building for wasm. Compact, varints, no schema
in the bytes (the schema hash travels in the envelope instead).

**Generator: `sim/wire-gen`**, a binary that traces the wire types with **serde-reflection** (it reads a type's
serde shape into a registry; tracing needs the types to derive `Deserialize`, which they do) and emits:

1. `sim/wire/registry.yaml`: the schema, committed, reviewed like code. Its FNV-64 is the **schema hash**.
2. Java 21 into `binding/src/generated/java/dev/b28/enterprise/wire/`: a record per struct, a sealed interface with a
   record per variant per enum, and a postcard codec for each (`encode(PostcardOut)`, `decode(PostcardIn)`), plus
   `Wire.SCHEMA` (the hash). The runtime (`PostcardIn`/`PostcardOut`: varints, zigzag, `f64` little-endian, lengths,
   variant indexes; `Bytes`, `WireException`) is hand-written, about 400 lines in `sim/wire-gen/src/runtime/`, and
   written beside the generated types, so the generated folder compiles alone; `just wire-javac` runs it on the
   core's own bytes at their edges, and the golden fixtures pin it after.

Why this pair, against what was checked:

| Option | For | Against |
|---|---|---|
| **serde-reflection + our Java emitter, postcard** (chosen) | The Rust types stay the source (no schema language beside them); the registry is maintained (serde-reflection 0.6.0, beside its family's serde-generate 0.34.1, June 2026); Java 21 records and sealed interfaces make a new `Output` variant a **compile error** in the dispatcher's exhaustive `switch`, which is what an upgrade wants; one format for saves and messages | The emitter is ours: about 500 lines, kept honest by the fixtures. serde-reflection can't trace `#[serde(flatten)]`, untagged enums or complex map keys: the wire avoids them |
| serde-generate's Java (same registry) | Off the shelf | Bincode or BCS only, and bincode is unmaintained; Java 8 classes, no sealed hierarchy; a second format beside postcard |
| Protobuf (prost + protobuf-java) | Mature, field tags tolerate skew | The schema becomes the source and the Rust types are generated or mapped by hand; skew tolerance buys nothing when one jar ships both sides |
| JSON (serde_json, schemars, jsonschema2pojo) | Readable | Slower and larger; `f64` text round trips are a risk to the digest; the codegen is two tools. JSON stays as the fixtures' readable twin only |
| WIT and the component model | The standard for this | Chicory runs core modules on WASI preview 1; components aren't in its docs |

`just wire-gen` writes both; `just wire-check` regenerates into a temp folder and fails if anything differs, run by
`cargo test` (a test calls the generator) and by Gradle before `compileJava`. A Rust wire change without the Java
regenerated fails both builds; a Java side out of date fails at `Create` too (the schema hash, section 1.4).

### 1.3 What crosses

Ids: towns and routes go by their `TownId`/`RouteId` (stable across saves, `messages.rs`). New: **trade points**
(`architecture.md` convention 3), `Point { kind, dim, x, y, z }` for a desk, a locker, a shop, a banker, a depot; the
core keeps them, so Java keeps no society ids of its own and rebuilds its lookup from `Query::Points` after a load.
And **stable hand ids** in the roster (`HandId`, never an index: `settlers::take_hand` shifts indexes,
`core-progress.md`), each tied to a villager's UUID by the people adapter.

| Happenings in (`Input`) | From |
|---|---|
| `Advance { to }` | the clock adapter, at each look's tick |
| `AddTown`, `AddPoint`, `RemovePoint` | a Merchant's desk, lockers, vendors, bankers, depots placed or broken |
| `Stocktake { point, counts, free }`, `Unmoved`, `Deliver`, `Withdraw` | a desk's chests read when loaded: what players, hoppers and Create put in or took out since the last sync, and their empty slots; what the host couldn't do of a `Moved` |
| `Census { town, beds, residents }`, `AddHand`, `HandLost`, `TagHand` | the census (bells, beds, villagers; a profession taken travels in it, so no `HandTrade`), a villager joining on touch, a villager's death, a villager spawned for a hand |
| `Switch { point, offer, on }` | the Merchant's screen (`ToggleTradePayload`) |
| `Ground { hex, water, relief }`, `Bridge`, `Built` | chunk scans (`Terrain`), blocks placed by works |
| `Order`, `Stall`, `OwnShop`, `Buy`, `Pay` | Numismatics vendors and players buying (exist today) |
| `SetDial` | `/enterprise dial` |

| Outcomes out (`Output`) | To |
|---|---|
| `Moved { point, good, count }` | items put in or taken from a desk's or a depot's chests: what the core moved in or out of the town's stock since they were counted, after a stocktake in the same look (built: nothing queued; an unloaded point is counted as it loads) |
| `WagonLeft`, `WagonHome`, `WagonSold` (exist), `HandAway`, `HandBack` | show the caravan; a hand gone with a wagon (crew, porter, soldier) stowed by hand id, one home let out by its desk with its trade, level and XP (built: the wagon's own three only logged) |
| `Born`, `Settled`, `Departed`, `HandGone`, `HandLevel` | spawn a baby, a party arriving, a party leaving and a hand gone (the villager taken away), a villager's trade, level and XP written to its entity |
| `Trades { point, offers }` | the offers a locker's villager runs, for the screen |
| `Build { kind, hex }` | works the town builds (walls, halls, bridges, fields) |
| `Coins { point, spurs }` | coins minted into a desk's stock or its Blaze Banker |
| `OrderFilled`, `Signal` (exist) | shops, boards |

Queries (`Ask`): a town's stock, cash book, whys, books, contracts, trust, the mint's rate, points: the commands and
screens read only these.

### 1.4 Versions

| What | Where | When it moves | A mismatch |
|---|---|---|---|
| ABI | `ent_abi()`, `Request.abi` | the four exports change (never, in plan) | `CoreHost` refuses to start: a build error in practice |
| Schema | the registry's hash, in `Request.schema` and `Wire.SCHEMA` | any wire type changes | `Refused { SchemaMismatch }`: a jar built from two commits. `wire-check` catches it before |
| Save format | `SaveFile { format, schema, core: git hash, blob }`, `save::FORMAT` | the state's shape changes **after the first release** | migrated forward, or refused |

**Saves and migrations.** Postcard isn't self-describing, so a migration reads the old shape: `save/migrate/vN.rs`
keeps a frozen copy of only the types that changed at format N and a function to N+1; `load` runs the chain from the
blob's format to today's. Each migration lands with a golden save of the old format (`sim/core/tests/saves/vN.blob`,
from a scenario) that must load and run on to a committed digest.

- **An old world in a newer core:** migrated on load; the old blob is kept beside the new one in the world's folder
  (`enterprise_core.prev.dat`) until the next save, so a rollback has it.
- **A newer world in an older core:** refused, with the formats in the message; the world opens with the society
  paused and its blob untouched, nothing lost.
- **Before the first release:** `FORMAT` stays 1 and nothing is migrated: a dev world whose core changed shape is
  rebuilt (the testbench datapack rebuilds its market; study checkpoints are made again). Decided by John (section 6).

**Built (B1.7 `versioning`).** In the core: `save::migrate` (`CHAIN`, empty while `FORMAT` is `OLDEST` = 1; `reads`,
`upgrade`; the steps are payload to payload, the header is the save's), `Load` refusing by number (`SaveFormat`) what
the chain doesn't reach, and a blob whose header disagrees with its `SaveFile` is a `Save` refusal. Its recipe for the
first format bump (golden save before, frozen `vN.rs`, digest after) is in `save/migrate/mod.rs`; `tests/saves.rs`
holds the template (a hand-made format 0 through a chain of one step) and the golden-save test over
`tests/saves/vN.blob` (none yet). In the binding: `Core.start` asks an empty `Ask` first and throws when the module's
ABI or schema isn't the generated wire's (a jar from two commits is said at start; the core checks both on every call
anyway), and `SaveStore` is the world's `enterprise_core.dat` (a `SaveFile`'s bytes): `open()` loads it (`Fresh`,
`Loaded`, or `Refused` with the player's words, which pauses the store: `save()` then writes nothing); the first `save()`
after a migration copies the file it replaces to `enterprise_core.prev.dat`, and later saves leave that copy alone
(read as: "until the next save" is the window in which the old file is the world's file; after it, the copy). The mod's
`CoreHost` (B2.1) is a thin caller of `SaveStore`: 1.5 calls `enterprise_core.dat` a SavedData (NBT), and `SaveStore`
writes a plain file of that name, so B2.1 picks one (`SaveStore` on its own path, or its rules around a SavedData's
byte-array tag), not both in `world/data/`: B2.1 picked `SaveStore`'s plain file, beside `level.dat`. The migrating load path (`save::upgraded_with`) runs end to end only
over the template's chain until the first real step.

### 1.5 The host side, and its cost

`binding/` is a **Gradle subproject of plain Java 21** (Chicory, the generated wire, `Core`, the codec, the
scenario runner): no Minecraft classes, so its tests run in seconds with JUnit, no game. The mod depends on it.

In the mod, `dev.b28.enterprise.binding`:

- `CoreHost`: one per server. Creates the society from the content pack at a new world or loads `enterprise_core.dat`
  (`SaveStore`'s plain file in the world's folder, beside `level.dat`, holding a `SaveFile`'s bytes; not a
  `SavedData`), keeps the input log since the last save, saves when the world saves.
- `CoreClock`: one `ServerTickEvent` listener. At each look's tick (`look_ticks`, 1,200: once a minute) it gathers
  the adapters' happenings, sends `Apply`, and hands each output to its adapter. Nothing runs per tick.
- `Outcomes`: an exhaustive `switch` over `Output`, one line an adapter. A new variant doesn't compile until it's
  routed.
- `adapters/`: one class per concern (section 2), each `happenings()` in, `show(Output)` out, `onLoad(chunk)`.

**Compute on touch, unloaded chunks.** The core runs every town as numbers whether its chunks are loaded or not (the
design rule: save data is the truth for what works unloaded; here, the core's blob). Views catch up when they load:
an adapter's `onLoad` asks the core and writes the result to blocks and entities (a locker's chests, a villager's
level), or counts the point's chests and is given what moved meanwhile (built so in `stock-adapter`: nothing is queued). So `Rounds` (forced chunk tickets) and `LockerCatchUp`
(replaying restocks on load) go: nothing needs loading for the society to run.

**Chests stay physical.** Players, hoppers and Create move items in a desk's chests, so the core can't own them
outright. At each look a loaded desk's adapter reads its chests and sends a `Stocktake`; the core books the difference
from the last one as deliveries or withdrawals; the core's own movements come back as `Moved`. Write-back safety as
the lockers' rule in `decisions.md`: a chest that doesn't match its last snapshot on load (a rollback, a hand edit)
takes a fresh stocktake, never a blind write. Room is a number the adapter reports (slots free).

**Budget.** At most 1 ms a tick on average and 5 ms for the tick a look runs on (a 50 ms tick). The estimate above
gives 0.13 to 0.3 ms a look for the study world compiled, 1.2 to 6 ms interpreted; a crossing is a few KB. Catching up
(a server down for days, a time warp) runs looks in a loop: 30 days is 600 looks, 0.1 to 0.2 s compiled, spread over
ticks at 2 ms each like the mod's catch-up today. A save is 200 to 300 KB at the world's autosave. The spike measures
each of these; if compiled looks exceed 1 ms, the look stays as is and the clock spreads it. **Measured** (section 4):
compiled looks are ten times the estimate, 2.3 ms on the study world and 9 ms on a 25-town one, the slowest (a day's
work in one look) 0.1 to 0.7 s; a look can't be spread over ticks, as it's one call. So the core runs on a thread of its
own (`CoreWorker` in `binding/`): the server thread hands it a look's `Apply` and shows the outputs a tick or so later.
The tick keeps only the crossing (a few KB); a 30-day catch-up is 1.4 s (study) to 5.5 s (25 towns) on the core's thread
while the game runs.

## 2. The migration order

**First the layer, proved on one trivial slice: a desk's stocktake end to end** (Minecraft chests to the core and
back through `/enterprise core stock <desk>`). Then the mod's rules move, a concern at a time, behind a switch.

**The switch.** The core has every rule (parity: 191 covered, 25 partial of 216), but the mod's rules are one web:
`Caravans.poll` reads `ContractBook`, `Territory`, `Bills`, `OffMap` in one pass. Moving them one by one would mean
Java rules reading core state through queries for months. Instead `society = java | core` (a server config line,
default `java`): in core mode the Java rules' tick listeners aren't registered and the adapters run. `just
gametest-core` runs the suite in core mode and prints the count passing per test class: **that count is the
migration's progress.** Java mode stays green until the delete wave, so the game always works.

**The game tests, sorted once** (task `gametest-triage`: `python3 scripts/parity.py --binding`, every test in
`docs/binding-tests.md`). The sort found about twice the fixtures first guessed: shared helpers in other test files
build the world (`CaravanTests.network` behind 47 tests, `LabourTests.town` behind 24), so the B4/B5 acceptance work is
about twice the first estimate:

| Kind | Count | Becomes |
|---|---|---|
| A world fixture: blocks, villagers, chests, then an outcome in the world | 150 | **The binding's acceptance tests.** Rewritten where they call a Java rule to drive the core through the binding; green in core mode is a concern's gate |
| A Java rule's API on data | 66 (27 hand the level to a rule: check it reads no terrain before retiring) | Retired with the class they pin. Their Rust twin stands (`parity.py` shows it); if a test has none, it moves to the core first |
| Minecraft's own (`parity-skip.txt`) | 46 (most of them world fixtures) | The binding's own view tests, kept |

**The concerns, in order** (each a task; "deletes" happens in the delete wave, once its tests are green in core
mode; "view" keeps the Minecraft half and drops the rules in it):

| # | Concern | Java it deletes | Java it shrinks to a view | Acceptance tests (core mode) |
|---|---|---|---|---|
| 1 | Content pack, dials | `Dials` (the catalogue; the command stays), `money/Prices`, `territory/Comforts` (the kinds) | `trade/Wares` | ContractTests' dial test; a pack fixture checked by the core |
| 2 | Points, ids, clock, save | the 33 `SavedData` one by one (as their rules go) | `merchant/Merchant` (desk, profession) | `CoreBindingTests` |
| 3 | Stock: desk chests, shulkers, depots, cold store | `merchant/Warehouse`, `merchant/Depots` (rules), `market/ColdStore` (rules) | `merchant/MerchantStock`, `market/ColdStore` | LogisticsTests, MerchantTests (world) |
| 4 | Money: emeralds, coins, Banker, mint | `money/Mint`, `CashBook`, `Accountant`, `Bargain`, `Treasury` (rules) | `money/MoneyChanger`, `Treasury` | MoneyTests, CountingHouseTests (world) |
| 5 | People: census, villagers, births, settlers | `territory/Jobs`, `Labour`, `Households`, `Growth` (rules), `caravan/Expeditions` (rules) | `settlement/CensusTaker`, `Settlements`, `Names`, `mixin/VillagerMixin` | SettlementTests, GrowthTests, JobsTests, LabourTests, ExpeditionTests |
| 6 | Lockers and the Merchant's screen | `locker/Rounds`, `LockerCatchUp`, `LockerTrading` (rules) | `locker/Lockers`, `Locker`, `merchant/MerchantView`, `ToggleTradePayload`, `client/MerchantScreen` | LockerTests, LockerTradingTests |
| 7 | Land: terrain, hexes, fields, works, herds, mines | `territory/Territory` (2,305 lines), `Chokepoints`, `Crops`, `Husbandry`, `Mines`, `Smithing`, `Hunting`, `Focus`, `Works` (rules), `Workshop` | `territory/Terrain`, `Fields`, `Docks`, `Buildings`, `Works` (placement) | TerritoryTests, TollTests, BuildingTests, HusbandryTests, MetalTests, MoodTests |
| 8 | Wagons, crates, post | `caravan/*` but the stow (`Caravans` 2,834 lines, `Squad`'s rules, `Atlas`, `Route`, `RoutePlanner`, `Desires`, `ForwardSales`, `Commissions`, `CaravanLedger`), `cargo/Crates`, `Papers`, `Paperwork`, `market/Post`, `Forwarder`, `Freight`, `Correspondent`, `service/*` (rules) | `Squad.stow/release`, `cargo/CrateBlock*`, `CrateItem`, `CrateMenu`, `DocumentsItem`, `client/CrateScreen` | CaravanTests, CargoTests, PostTests, ServiceTests, MasterTests (world) |
| 9 | Shops, boards, contracts, banks | `contract/*`, `market/Broker`, `Bank`, `Discounts`, `Exchange`, `Procurement`, `RentDealer`, `Stranded`, `Tenders`, `Apprentices`, `MetalExchange` (rules), `merchant/Books`, `MerchantBook` | `market/Shops`, `ShopBoard`, `RentDeeds`, `mixin/VendorAccess` | MarketTests, BargainTests (world) |
| 10 | Crown, war, realms | `market/Sovereign`, `Requisitions`, `army/Army`, `trade/Realms`, `merchant/OffMap` | Sovereign's gold shop | WarTests |
| 11 | Commands, studies | the rule half of `EnterpriseCommands` (1,131 lines) | `EnterpriseCommands` on `Ask` | the `dev/experiments` scripts read the same numbers |

**Built (B4.11 `commands-and-studies`): the commands and the studies.** In core mode `/enterprise` answers from the core
or says it is still the Java rules'. In the core (`sim/core/src/commands_host.rs`, no new saved state): `Query::CashBook`
(its money in and out and totals by what for are the core's sums, carried on the answer), `Books`, `Whys`, `Trust`,
`Contracts` (each contract with `day N:` history lines and the town's rents with theirs, worked out from the state: the
core keeps no list of events), `Territory { town }` (the settlement report: hexes and culture, farms and idle plots by good
a day, crops sold forward, treasury and what is kept back, mood, villagers and beds, comforts, bridges, wall, halls, toll,
posts, traffic, locker trades), `War` (the sovereign, the capital's treasury, the crown's contracts, the army, each town's
part: `/enterprise war` and `war tally`) and `RouteBooks` (each route's totals for the studies' rows). In the mod
(`adapters/BooksAdapter`, `adapters/CommandsAdapter`, `EnterpriseCommands` only routing on `Society.isCore()`): each command
is one question and its lines out, in the Java commands' words so the studies' regexes read both modes' output.

| Command | In core mode |
|---|---|
| `mint`, `territory <bell>`, `war`, `war tally`, `contracts [desk]`, `trust`, `cashbook`, `books`, `whys`, `routes`, `route add/remove`, `dial`, `lockers`, `stock` | the core's answer (`Mint`, `Territory`, `War`, `Contracts`, `Trust`, `CashBook`, `Books`, `Whys`, `RouteBooks`/`Routes`, ...) |
| `atlas <desk>` | the core's (`Routes`: the markets the town knows) |
| `name` (list), `depot` (list), `autoswitch` | the core's (`Points` and `Territory`'s names; `Lockers` and `Switch`); `depot add/remove` already sync the core |
| `census <bell>` | refused: a census is the core's, each look (`/enterprise core census <desk>` takes one now) |
| `toll`, `hex`, `bridge`, `cottage`, `survey`, `services`, `expeditions`, `settlements`, `switch`, `name <desk> <name>`, `freight`, `post`, `forwarder`, `paperwork` (the crates and papers stay Java mode's: `pallet-views` drew the core's pallets without them) | "Java mode only until B5.1 delete-java-rules" |
| `market <desk>` | the core's (`Shops`: the town's vendors, orders, signals, board listings and tally) |
| `tenders`, `bank`, `bank open` (no core rules shown for them: `shop-adapter` took the vendors only) | "Java mode only until B5.1 delete-java-rules" |
| `metals`, `realms`, `realms open`, `war capital`, `war mobilise`, `war declare`, `war peace` | the core's (`Metals`, `Realms`, `OffMap`, `Capital`, `Mobilise`, `Declare`, `Peace`: `adapters/CrownAdapter`) |

**Studies.** John, 2026-10-18: most Java-server studies retire for `sim/host` (`just study`); one in-game study is kept as a
check. Kept: `network_study.py` with its default flags (and `--save-at`, `--label`, `--resume`, `--stop-when`), the same
script in either mode; under `society=core` every number in its per-day CSV and in `--stop-when` comes from the core (mint
rate, each town's row from `Territory`, each route's from `RouteBooks`, the cash book), and its other flags exit with "Java
mode only (retired for sim/host)". `trace.py` works in either mode (it asks `territory` and `contracts`, the core's under
`society=core`). `forks.py` and `compare_lanes.py` run the kept study and read its CSV: unchanged. Retired (Java mode only; under
`society=core` they exit saying "Java mode only (retired for sim/host)"): `merchant_ramp.py`, `farm_wages.py`,
`caravan_network.py`, `standalone_town.py`, `paper_report.py` (it refuses a checkpoint whose `study.json` says `"core": true`);
and `world_map.py`, which reads the Java rules' saved data and is only noted Java mode only (it reads files, not the server,
so it can't tell the mode: on a core world it draws what the Java rules last saved). Their questions are asked of `sim/host` (`docs/architecture.md`).

## 3. Tasks

### 3.1 The common brief (paste first in every `.tree/TASK.md`)

```
You are building the Minecraft binding of the Enterprise core: the NeoForge mod (core/, Java 21) calling the Rust
core (sim/core) compiled to WebAssembly and run in the JVM by Chicory. Read, in order: docs/worktrees.md (the rules
you work under; this phase lifts its "no Java" rule as your task says), docs/architecture.md, docs/binding-plan.md
(sections 1 and 5, and your task's entry), and the Java and Rust your task names. CLAUDE.md has the game test
gotchas; read them before writing a game test.

The binding stays thin: if you find yourself writing a rule in Java, it belongs in the core (a wire message and a
host call there). The wire types are sim/core/src/wire/: they hold numbers, strings, ids and vectors, never a state
type. After changing them run `just wire-gen` and commit the regenerated Java and sim/wire/registry.yaml with the
change; never edit binding/src/generated by hand. A new Output variant gets one line in binding's `Outcomes` switch
and an adapter in core/src/main/java/dev/b28/enterprise/binding/adapters/; an adapter maps one Minecraft concern, and
nothing outside adapters/ learns a new message. Every new message gets a golden fixture (`just wire-fixtures`) and
appears in a scenario under sim/scenarios/.

The core's rules from docs/worktrees.md still hold for any Rust you touch: determinism (no clock, no HashMap
iteration, the seeded Rng), money through cash_book::post, new state in saved!, no new dependencies (Cargo.toml,
Cargo.lock and the Gradle dependency lists are the integrator's: if you need one, say so in your summary).

Gates, all that apply to your task: `cd sim && cargo test --release`; `just core-wasm-replay`; `just wire-check`;
`just binding-test` (Gradle :binding:test, the JVM contract tests, no Minecraft). If your task says Java runs:
`just build`, then `just gametest` (look for "All N required tests passed" with N as your task says; it runs headless
in core/run/gametest) and, from `core-mode` on, `just gametest-core` (its pass count must not fall). Java is heavy
(a Gradle daemon 2 GB, a game-test server 2 GB): run it under `python3 scripts/trees.py heavy -- CMD` (machine-wide slots), and
stop only your own processes, by pid. A headless server, if your task needs one, is the tree's own (ports from
.tree/env); never point anything at 25565/25575 or 25566/25576. Don't compile while lanes run.

Commit in your tree in the repo's style (what changed and why, measured), ending with the two attribution lines.
Don't push, don't land. When done, add a summary at the end of .tree/TASK.md: what landed, what's partial, the
measurements your task asks for, and how long an upgrade through your part would take.
```

### 3.2 Wave B1: the layer (no Minecraft)

#### B1.1 `wasm-digest`

| | |
|---|---|
| Goal | `just core-wasm-replay` passes on main again, and stays passing: every later test compares native and wasm |
| Files | whichever system's order or width moved (start at `rules/squad.rs::unmanned` and the `Plan` step); `tests/replay.rs` (a per-day digest on failure, so the first diverging day shows) |
| Tests | `just core-wasm-replay`; `just core-fingerprint` unchanged (`8144f9be7947133b`) or the change explained; a test that prints `look_schedule()`'s order and compares it with a committed list, so an ambiguity a landing adds shows natively |
| Waits for | — |
| Java | none |
| Model | **opus / high**: a bisect inside one commit and Bevy's ordering; the fix must not move native behaviour |

#### B1.2 `wire-types`

| | |
|---|---|
| Goal | `sim/core/src/wire/`: the envelope (`Request`, `Command`, `Response`, `Refusal`), `Input`/`Output` moved from `messages.rs` (re-exported there for the host), `Query`/`Answer` with the proof's two (`Stock { town }`, `Points`), `ContentPack` (goods: name, worth, flags, food points) with `Content::from_pack` checking it, `Point`, `SaveFile`; `wire::handle(&mut Option<Society>, Request) -> Response`, natively callable |
| Files | `sim/core/src/wire/*.rs`, `lib.rs` (a line), `messages.rs` (re-export), `sim/host` unchanged |
| Tests | `tests/wire.rs`: create from a pack, deliver, ask the stock; save and load through `handle`; refusals; a test that no wire type holds a state type (by the registry, once `wire-gen` lands; until then by a list) |
| Waits for | `wasm-digest` |
| Java | none |
| Model | **opus / high**: the shape every later message follows |

#### B1.3 `wasm-exports`

| | |
|---|---|
| Goal | `sim/wasm`: the cdylib with the four exports over `wire::handle`, `panic = "abort"`; `just wasm` builds `enterprise_core.wasm` (release, stripped of names but the exports) and prints its size |
| Files | `sim/wasm/` (new crate; the workspace line is the integrator's), `justfile` (`wasm`) |
| Tests | a native test of the exports' framing (alloc, call, free); a test listing the module's imports against the allowed set; the proof's request run under Node's WASI against the native answer (`wasi-run.mjs` grows a reactor mode) |
| Measure | the module's size; its imports |
| Waits for | `wire-types` |
| Java | none |
| Model | **sonnet / medium**: well specified |

#### B1.4 `wire-gen`

| | |
|---|---|
| Goal | `sim/wire-gen`: traces the wire types with serde-reflection, writes `sim/wire/registry.yaml` and the Java 21 records, sealed interfaces and postcard codecs into `binding/src/generated/java`, and `Wire.SCHEMA`; `just wire-gen`, `just wire-check` |
| Files | `sim/wire-gen/` (new crate; serde-reflection is the integrator's to add), `justfile` |
| Tests | a Rust test that runs the generator and diffs against the committed output; the registry rejects `flatten`/untagged; enum variant order is append-only against the committed registry (a reordered variant changes every index: refuse it, or bump the schema loudly) |
| Waits for | `wire-types` |
| Java | the generated code must compile: `javac` on the generated folder alone (no Gradle) is the check here |
| Model | **opus / high**: a code generator others depend on; the codec's edge cases (varints, zigzag, `Option`, nested enums, `u128` refused) |

#### B1.5 `wire-fixtures`

| | |
|---|---|
| Goal | Golden files and scenarios both sides read. `just wire-fixtures` writes, for each wire type, sample values (serde-reflection's samples plus hand-picked edges: 0, negatives, `Fixed::MAX`, NaN refused, empty and long vectors) as `sim/wire/fixtures/NAME.bin` with a `NAME.json` twin. A scenario format, `sim/scenarios/NAME.json`: a list of `Request`s, each with its expected `Response` (or a digest of it), and a runner: `cargo test --test scenarios`. The first scenario: `stocktake.json` (a town created, deliveries, a stocktake, its stock asked, saved, loaded, asked again) |
| Files | `sim/wire/fixtures/`, `sim/scenarios/`, `sim/core/tests/scenarios.rs`, `tests/wire_fixtures.rs` |
| Tests | the bytes Rust writes equal the files; the scenarios pass natively and under Node |
| Waits for | `wire-types` |
| Java | none |
| Model | **sonnet / medium** |

#### B1.6 `chicory-host`

| | |
|---|---|
| Goal | `binding/`: the Gradle subproject (plain Java 21, Chicory runtime and WASI, the build-time compiler through the wasm2class plugin or a `JavaExec` of the compiler if the plugin won't fit ModDevGradle's build), `Core` (loads the compiled module, `call(Request) -> Response`, the trap path: drop, reload, replay), the postcard runtime, the generated wire on its source path, `just binding-test` |
| Files | `binding/`, `settings.gradle` (`include 'binding'`), `justfile` |
| Tests | JUnit: every fixture decodes and re-encodes to the same bytes; every scenario through Chicory gives the expected responses; a trap mid-scenario recovers to the same answers |
| Measure (the spike, inside the proof) | instantiation time (compiled, interpreted); an empty call's round trip; a look on the scenario world and on a study-sized world built through the wire (`Create` + setup inputs), compiled and interpreted, ms a look against native and Node; save and load of a 250 KB blob; the functions the compiler couldn't compile (`interpreterFallback = WARN`'s list) and what they cost; the jar's growth; heap after a 365-day run. Writes them into `docs/binding-plan.md` section 4 |
| Waits for | `wasm-exports`, `wire-gen`, `wire-fixtures` |
| Java | **yes**, `./gradlew :binding:test` only (no NeoForge, no game test server); gate: `just binding-test` |
| Model | **opus / high**: new build tooling, the numbers the plan rests on |

#### B1.7 `versioning`

| | |
|---|---|
| Goal | Section 1.4 built: `abi` and `schema` checked on every call; `SaveFile` wrapping the blob; `save::migrate` with its chain (empty until a format 2) and the way a migration lands (a frozen type module, a golden old save, a digest); refusals for a newer format; `enterprise_core.prev.dat` kept on a migration |
| Files | `sim/core/src/save.rs`, `save/migrate/mod.rs`, `wire/`, `binding/` `Core`, `sim/core/tests/saves/` |
| Tests | Rust and JVM: a request with another schema is refused; a `SaveFile` of format 2 is refused by a format-1 core; a hand-made "format 0" migration test (a renamed field) shows the chain works, then is removed or kept as the template |
| Waits for | `chicory-host` |
| Java | `just binding-test` |
| Model | **sonnet / high** |

### 3.3 Wave B2: the proof in the mod

#### B2.1 `proof-in-mod`

| | |
|---|---|
| Goal | The mod depends on `binding`. `CoreHost`, `CoreClock`, `Outcomes`, `enterprise_core.dat`; one adapter, `adapters/StockAdapter` (a desk registered as a town and a point; its chests read at each look into a `Stocktake`); `/enterprise core stock <desk>` answers from the core. The rest of the mod untouched (Java mode) |
| Files | `core/build.gradle` (the dependency), `core/src/main/java/dev/b28/enterprise/binding/`, `Enterprise.java` (two listeners), `EnterpriseCommands.java` (one subcommand), `test/CoreBindingTests.java` |
| Tests | `CoreBindingTests.aDesksStocktakeComesBackFromTheCore`, `.theCoreSurvivesASaveAndLoad` (save `enterprise_core.dat`, load a fresh host, ask again) |
| Measure | on the tree's headless server (its own ports, `.tree/env`), the tick time with the core running a look against without; the time a world save takes with the blob |
| Waits for | `versioning` |
| Java | **yes**: `just build`, `just gametest`: "All 264 required tests passed" (262 and the two new) |
| Model | **opus / high**: the first contact with NeoForge's lifecycle (server start, world save, stop) |

**Built (B2.1 `proof-in-mod`).** `core/` depends on `binding` (`implementation`, `additionalRuntimeClasspath` so FML
loads it beside the mod in dev runs and game tests, `jarJar` with Chicory's runtime, wasi, wasm and log at
`chicory_version` in `gradle.properties`: the jar grows 2.3 MB). In `dev.b28.enterprise.binding`: `CoreHost` (made at
`ServerStartedEvent`, the world's seed and `ModPack` at a new world, else the save loaded; saved on the overworld's
`LevelEvent.Save`; closed at `ServerStoppedEvent`; the core on a `CoreWorker`, looks sent and their outputs shown a tick
later, questions and registrations waited for), `CoreClock` (the one tick listener: a look every 1,200 ticks of game
time, outputs polled each tick), `Outcomes` (the exhaustive switch; every variant today routed to "unshown" until its
adapter lands), `ModPack` (the goods `Prices` lists, by id, worth and stack and food: B4.1 adds the rest), and
`adapters/StockAdapter` (desks as towns and points, registered when a player places one, forgotten when one is broken,
or registered by `/enterprise core stock <desk>`; touching chests counted at each look; the desks and the goods' numbers
read back from the core, never kept in Java). **The save is `SaveStore`'s plain file**, `enterprise_core.dat` in the
world's folder beside `level.dat`, not a SavedData: the same bytes, with `SaveStore`'s rules (whole through a temporary
file, prev.dat, the refusal that pauses) kept in one place; a world's folder copied copies its society. One addition to
the core: `Query::Goods` (the goods' names by number), since a loaded society keeps the pack it was made with and the
host must map items by that, not by the pack it would build today. Measured in the game-test server (no dev server:
`ENTERPRISE_CORE_MEASURE=1 just gametest`), four desks with 540 full stacks: a look costs the server's tick 0.05 to
0.07 ms mean (p99 0.2, the worst 1.7: counting the chests and handing over the `Apply`; without the core, nothing), the
core's thread 2.4 to 2.6 ms a look (empty towns, the mod's 135 priced goods; a machine at load 11); a save of the 39 KB blob
2.2 ms p50 (8.6 the worst) on the tick that saves the world.

### 3.4 Wave B3: the switch (start beside B1/B2: `gametest-triage` reads only)

#### B3.1 `gametest-triage`

| | |
|---|---|
| Goal | Every game test sorted as section 2's table: `scripts/parity.py --binding` prints world fixture / rule API (with its Rust twin) / Minecraft's own, from the test bodies and `parity-skip.txt`, and flags any rule-API test with no Rust twin (those go to the core first). Writes `docs/binding-tests.md` |
| Files | `scripts/parity.py`, `docs/binding-tests.md` |
| Waits for | — |
| Java | none (reads) |
| Model | **sonnet / medium** |

#### B3.2 `core-mode`

| | |
|---|---|
| Goal | `society = java \| core` in the server config; in core mode the 13 rule tick listeners (`Caravans`, `Expeditions`, `Realms`, `Broker`, `Sovereign`, `Freight`, `MetalExchange`, `Works`, `Rounds`, `LockerCatchUp`, `CensusTaker`'s rule half...) aren't registered and `CoreClock` drives the adapters; `just gametest-core` runs the suite in core mode and prints passes per class, against a committed baseline it must not fall below |
| Files | `Enterprise.java`, `binding/CoreHost.java`, `justfile`, `core/run/gametest-core/` (its own folder) |
| Tests | the suite in both modes; `CoreBindingTests` in core mode |
| Waits for | `proof-in-mod`, `gametest-triage` |
| Java | **yes**: `just gametest` (all passing, Java mode) and `just gametest-core` (the baseline written) |
| Model | **sonnet / medium** |

### 3.5 Wave B4: the concerns (section 2's table; at most two at once: Java slots)

Each: its wire messages and host calls in the core, its adapter, its acceptance tests rewritten to drive the core
where they called a Java rule, green in core mode; Java mode stays green. Gate for every one: the Rust gates,
`just binding-test`, `just build`, `just gametest` (all N passing, Java mode), `just gametest-core` (its classes
passing, nothing else falling).

| Order | Task | Concern (section 2) | Waits for | Model / effort | Why |
|---|---|---|---|---|---|
| B4.1 | `content-pack` | 1 | `core-mode` | sonnet / high | the mod's goods, trades, recipes and dials as one `ContentPack`, checked by the core; `/enterprise dial` routed |
| B4.2 | `points-and-hands` | 2 | `content-pack` | opus / high | trade points; stable `HandId` in the core's roster (a core change through every slice keyed by hand) |
| B4.3 | `stock-adapter` | 3 | `points-and-hands` | opus / xhigh | chests stay physical: stocktakes, `Moved`, write-back safety, shulkers, depots; everything after reads stock |
| B4.4 | `money-adapter` | 4 | `stock-adapter` | sonnet / high | emeralds in stock, coins, the Banker, the mint's rate; spurs convert at the treasury (`core-progress.md`) |
| B4.5 | `people-adapter` | 5 | `points-and-hands` | opus / high | the census as the roster's source, villagers as views of hands, births, deaths, parties |
| B4.6 | `locker-adapter` | 6 | `stock-adapter`, `people-adapter` | opus / high | offers switched on the screen, locker chests moved by the core; `Rounds` and `LockerCatchUp` go |
| B4.7 | `land-adapter` | 7 | `people-adapter` | opus / high | terrain into the map; works, fields and docks placed as outcomes |
| B4.8 | `wagon-adapter` | 8 | `locker-adapter`, `land-adapter`, `money-adapter` | opus / high | squads stowed and released by hand id, crates as items and blocks, the post's papers |
| B4.9 | `shop-adapter` | 9 | `money-adapter`, `wagon-adapter` | sonnet / high | Numismatics vendors, boards, rent deeds |
| B4.10 | `crown-views` | 10 | `wagon-adapter`, `shop-adapter` | sonnet / medium | the sovereign's shops, off-map markets as points |
| B4.11 | `commands-and-studies` | 11 | `wagon-adapter` | sonnet / medium | commands and screens on `Ask`; `dev/experiments` read the same numbers (cash book, whys, trace) |

**Built (B4.2 `points-and-hands`).** In the core: every hand on a roster has a `HandId` (`roster.rs`), a `u32` given from
the world's count (`roster::Hands`, in `saved!`) as it joins a roster in a society (a host's hand, a child born) and kept
through a new trade (`jobs.rs` no longer makes the hand anew), a caravan's trip (`squad::stamp` keeps it rather than
stamping a fresh one each departure), a war and a party that settles another town; ids aren't reused. The host's name
for a hand (a villager's UUID) is kept beside it (`Hands.tags`), so the mod keeps no ids. What the core keeps by hand
follows the id: a `Locker` knows its hand (`Locker.hand`), `Lockers::sync` matches lockers to the roster by id, and the
Merchant's clients are ids (`Lockers.clients`); the managed lockers and `Roster.merchant` stay places, worked out again
at each sync and kept straight by `settlers::take_hand`. `Society::set_roster` keeps the ids at the places the old
roster had (a fixture setting a town's people anew). Host calls in `hands.rs` (`join`, `lose_hand`, `find_hand`,
`hand_tagged`, `hand_views`). On the wire: `Input::AddHand { town, trade, level, tag }` (refused for a trade the core
hasn't, a level outside 1 to 5, a tag another living hand has), `Input::HandLost { hand }` (refused for a hand that's
away: off the host's world), `Query::Hands { town }` and `Answer::Hands { town, lockers, hands: Vec<HandView> }`; schema
`9c9206f91a4ed70f`, FORMAT 1 (the save's shape grew: dev worlds' `enterprise_core.dat` are rebuilt). Behaviour unmoved:
the replay's outputs digest the same with the save left out, and `just core-fingerprint` is `96b225213f5f291c`; the
digest moved for the save's shape and the new random-worlds slice (`hands`: villagers joining and dying, its check that
every hand has an id none other has and every locker is its hand's). In the mod: `adapters/PointsAdapter` (every point
the core keeps, by place, rebuilt from `Query::Points`; desks, lockers, shops, bankers, depots registered there; the
stock adapter's desks are its), `adapters/HandsAdapter` (villager to hand by UUID, rebuilt from `Query::Hands` for every
town with a point; `join`, `lose`, a villager's death heard at LOWEST), `Merchant.lockersAt` (core mode: the Merchant's
screen shows the core's count of the lockers it manages, the Merchant joined as its town's hand on that touch),
`/enterprise core points` and `/enterprise core hands <desk>`.

**Built (B4.5 `people-adapter`).** In the core (`census.rs`, `Sightings` in `saved!`): the host's census is a town's
roster's source. `Input::Census { town, beds, residents: Vec<Resident { tag, trade, level, baby }> }` sets the town's
beds and reconciles its roster by tag, leniently (a count of the world never refuses a look): a villager the core
doesn't know joins (a child as a child), one it knows at home is seen (a profession taken in the world becomes its
trade, never `none` and never the desk's Merchant's; a level earned there its level, never less), one whose hand is
away, another town's or with settlers is left alone, one whose hand is gone comes back as `HandGone`; a tagged hand at
home that no census has seen for `FORGET_AFTER_DAYS` (10, `CensusTaker.FORGET_AFTER_TICKS`) has left. What the rules do
to hands in an `Advance` through the wire comes out for the host's villagers (`Society::hand_outputs`, a diff of every
hand by id around the `Advance`): `Born { town, hand }` and `Settled { town, hand }` (a hand with no villager: the host
spawns one and names it with `Input::TagHand { hand, tag }`), `Departed { town, hand, tag }` (gone with settlers: the
core forgets the tag and keeps it as retired, so the old villager seen again is `HandGone`), `HandGone { hand, tag }`,
`HandLevel { hand, tag, trade, level, xp }`. `AddHand` stays beside `Census`: one villager joining on touch (the
Merchant's screen), where the census is the periodic whole. In the mod: `adapters/PeopleAdapter` (in core mode, each
look counts every town whose desk and settlement are loaded, round the bell nearest its desk, radius 48, no path
checks, sends the `Census` and spawns a villager by the bell for each hand with none; shows the five outputs; the
villagers it spawned are named with the next look), `CensusTaker.beds`/`villagersAround`/`nearestBell` (its counting
half, shared), `HandsAdapter`'s lookup read again after each look, `/enterprise core census <desk>`. Schema
`e0b54576b14c0176`, FORMAT 1 (the save grew: dev worlds' `enterprise_core.dat` are rebuilt).

**Built (B4.3 `stock-adapter`).** Chests stay physical, and a town's stock is shown in them. In the core (`chests.rs`,
`points.rs`): a point keeps what the core takes its chests to hold (`Placed.counted`), their empty slots at the last count
(`free`) and what they gave up beyond what the town held (`short`, which `withdraw` used to drop unrecorded). After a
look's happenings, every desk or depot counted in that look shows its town's stock (`Society::show_stock`, run by
`wire::handle` at the end of each `Apply`): for each good, what the town holds in whole items against what all its
points' chests are taken to hold, the difference put into its counted desks then depots as far as each has room (empty
slots, and what's left of the good's partial stack), or taken out of them as far as each holds it; what has no room waits on
the town's books until a count shows some; fractions stay on the books. A move is worked out only against a stocktake of
the same look and is a number of items, never a chest's contents, so a chest is always counted before it's written and a
hopper filling it between the count and the write keeps what it put in; a point not counted (unloaded) isn't written, and
when counted again what changed in it is booked first: nothing is queued. Lockers', shops' and bankers' stocktakes still
book into the town (their adapters decide what their chests are), and count as shown. A point removed takes what the core
took its chests to hold out of its town (the double count of a point broken and placed again, settled), and a desk put back
where a town's desk stood is that town's again (`chests::FormerDesks`, saved), not a new town. On the wire: `Stocktake`
gained `free`; `Output::Moved { point, good, count: i64 }`; `Input::Unmoved { point, good, count }` (what didn't fit,
above nothing, or wasn't there to take, below; ignored for a point gone); `Query::Chests { point }` and `Answer::Chests
{ point, town, held, free, short }`; schema `bffc72824690b076`, FORMAT 1 (the save's shape grew: dev worlds'
`enterprise_core.dat` are rebuilt). Base worlds' behaviour unmoved (`just core-fingerprint` `cb4390b51df82271`); the replay
digest moved for the new random-worlds slice (`chests`: a host whose chests take part of each move, desks broken and put
back) and the save's shape. In the mod: `adapters/StockChests` (a point's containers: a desk's touching ones, a depot's
own; goods counted loose and in shulker boxes, crates left shut as the pallets' view; put loose then into boxes, taken
loose then out of boxes), `adapters/StockAdapter` (desks and depots counted at each look, a desk or depot gone unseen
removed, `Moved` written in core mode and every one sent back unmoved in Java mode, where the Java rules own the chests;
the chests in a chunk that loads counted at the next tick, and desks placed unseen there taken as towns by the server's
host; depots from a building's floorplan, `/enterprise depot add` and `remove` registering the core's too, and at
each look the floorplan's containers the core doesn't keep, a chest put back or one added, joining as depots), `CoreHost`
(no stocktake while a look's moves are still to be shown; what wasn't moved told the core before the next count, a
question or a save), `/enterprise core chests <point>`, the testbench's desk registered at load. Seven game tests.

**Built (B4.4 `money-adapter`).** A town's money in its desk's chests and Blaze Banker, shown by the core. In the core
(`till.rs`, `Tills` in `saved!`): the treasury stays one number of emerald-worth; a desk's `Till` is what the core takes
its chests and Banker to hold of it (whole emeralds, spurs of coin, the Banker's balance), the two piles of coin each a
`Hoard` (spurs, and the worth they stand for). **Spurs convert at the treasury**: spurs a player pays in come in at the
mint's rate then, spurs the mint pays for emeralds stand for those emeralds, spurs taken out take their share of the
pile's worth: the rate moving makes no money and loses none, and nothing posts for a conversion. At each look the host
counts the till (`Input::Cashcount`, right after the desk's `Stocktake`): what's more than the core took it to hold is
`Reason::PaidIn`, what's less `Reason::Withdrawn` (new; never more than the treasury holds), both through
`cash_book::post`. After the look's counts the core shows its treasury there (`Society::show_cash`, run by
`wire::handle` after `show_stock`; `Output::Cash { point, emeralds, coins, bank }`, deltas): the treasury above the till
put in as whole emeralds as far as the chests have room (fractions stay on the books), the treasury below it taken out
(emeralds first, then coin, then the Banker, then an emerald changed at the mint and the change back as coin:
`Treasury.withdraw`), the Merchant's emeralds topped up to its reserve from its coin at the mint's price
(`MoneyChanger.buyEmeralds`, the 5% spread shown in what a spur stands for, not in the treasury), and emeralds above the
reserve sold to the mint for coin, into the Banker's account if there is one, else coins in the chests if they have room
(`MoneyChanger.settle`). The reserve is 64 emeralds while a switched-on trade of the town's lockers costs emeralds
(`Society::emerald_reserve`), none otherwise. What the host couldn't do comes back as `Input::CashUnmoved`. A host that
doesn't write (Java mode) says so in the count (`writes`): the core then only counts, sells nothing to the mint and
moves nothing. `Query::Cash { point }` / `Answer::Cash` (the treasury, the till, the reserve) and `Query::Mint` /
`Answer::Mint` (the rate, the pressure, the coin minted without backing, the debasement); a desk broken takes its money
with it (`Society::remove_point`) and one put back counts it afresh. Schema `ccb902b478f9a78f`, FORMAT 1 (the save's
shape grew: dev worlds' `enterprise_core.dat` are rebuilt). Base worlds' behaviour unmoved (`just core-fingerprint`
`5e6b6d51a3b4061e`); the replay digest written again for the new random-worlds slice (`till`) and the save's shape. In the
mod: `adapters/MoneyAdapter` (the desk's chests and Banker counted after its stocktake: emeralds loose and in shulker
boxes, coin loose, the Banker's account; `Output.Cash` written: coin minted again into the fewest stacks with the
chests' own, the account's balance set, emeralds put and taken; a sale whose coin has no room keeps its emeralds and is
told back unmoved), `StockChests` (`coinSpurs`, `coinRoom`, `mint`), `CoreHost` (the counts ride the stock adapter's;
unmoved cash told before the next count, a question or a save), `Outcomes` routes `Cash`, `/enterprise core cash <desk>`
and `/enterprise core mint`. `MoneyChanger.bankAt` (a Blaze Banker's account, or an off-map market's) stays Java as the
view onto the account. Six game tests.
**Built (B4.7 `land-adapter`).** In the core (`land.rs`, `LandBook` in `saved!`): the host surveys its land and lays what
the towns build; whose land is whose, what grows and what's built are the core's. `Input::Ground { hex, water, relief,
land }` (a hex's survey into the `Map`: caravans go round water and cliffs by it, hands gather what its `LandKind`
gives, bridges go where it's wet; a finished bridge stays), `Input::Crops { hex, chunk_x, chunk_z, counts }` (the crop
plots counted in one chunk's part of a hex, kept by chunk, so an unloaded chunk keeps its last count, as
`Territory.cropsIn`), `Input::Survey { town, bell_x, bell_z }` (the bell its land grows from; its farm set to the plots
counted in the hexes it holds, its home hex before its first count, with the fields it planted that the host hasn't
laid; the farm set up), and the works shown as outcomes: at the end of each `Apply`, every town surveyed in it is asked
for what it has to lay, `Output::Build { town, work, share }` (a bridge or a hall once finished, the wall a quarter at a
time as `Works.onServerTick` raised it, a field as soon as the rules plant it: noted around a wire `Advance` from the
plots each crop gained), until the host says `Input::Built { town, work, share }`: nothing waits in the host, and a town
unloaded when its work finished is shown it when its ground loads. `Work` is `Bridge { hex, from, to }`, `Wall`, `Hall {
kind }`, `Field { crop }`; `Query::Land { town }` (hexes, culture, the next hex's cost, plots, works with laid, done and
shown) and `Query::Ground { hexes }`. Schema `03941e1d6b77b641` (on `money-adapter`'s), FORMAT 1 (the save grew: dev worlds'
`enterprise_core.dat` are rebuilt); `just core-fingerprint` unmoved (`5e6b6d51a3b4061e`), the replay digest written
again for the save's shape and the new random-worlds slice (`land`). In the mod: `adapters/LandAdapter` (ground in both
modes: a hex whose chunks `Terrain` surveyed again, and at a town's first sight in a run the hexes within 8 of it whose
ground the core has otherwise, at most 64 a look; in core mode each loaded town surveyed every look, its crops counted
every 20 looks through `Crops.countChunk`; `Build` laid with the mod's own placement, `Works.layDeck`, `raiseColumns`,
`Buildings.site`/`build`, `Growth.farmSite`/`Fields.plant`, at the start of the next look once every look sent is
shown, its chunks held loaded for four looks if they aren't, then `Built` with that look, or before a save), `Works`' placement halves made public (`layDeck`, `raiseColumns`), `/enterprise core
land <desk>`; three game tests. Measured in the game-test server: a town's first look of a run 1.1 to 4.1 ms of the tick
(64 hexes' ground read, 35 to 42 chunks of crops counted), a look after it 0.14 to 0.15 ms (fifty averaged, the crops
counted at every twentieth), one hex's crops 0.4 ms. A look the core refuses or traps on has its `Built`s said again
with the next (never laid twice). Left: the wooden pier (`Territory.buildDock`), pastures' fences and mine heads have no core rule or output;
`dock` and `pastures` and the free/confined split aren't surveyed; settlers' founded towns get no desk; town names
still drift from Java's `Names`; the Java `Territory` family stays (Java mode) until `delete-java-rules`. A field is paid
for when the rules plant it, before the host looks for its site (`Fields.consider` finds the site first), so one with
no room is paid for, on the farm and asked for at every survey; every town's crops are counted on the same look.

**Built (B4.6 `locker-adapter`).** In the core (`locker_host.rs`; `Lockers.placed` in the saved `Lockers`): a town founded
through the wire (`AddTown`) has its lockers where its host places them, and a hand has a locker the Merchant can manage
only where one stands (`Lockers::has`, through `order`, `takes`, `restock`); a town the core keeps as numbers has every
hand's, as before (`just core-fingerprint` unmoved). `Input::Locker { hand, at: Option<Point> }` ties a hand's locker to a
`Locker` point of its town's (one hand a point) or unties it; a locker point removed, or a hand gone, takes its tie. A
locker's chests are counted with the desk's and written (`chests.rs`): what the town uses is taken from its counted
lockers first, and what they still hold is carried to its counted desks and depots as far as they have room (`Moved` out
of the locker, into the desk); `Unmoved` is taken at a locker too. The plan's `Switch { point, offer, on }` is
`Input::Switch { hand, offer, good, on }` (lockers are hands' in the core; `good` checks the screen's view isn't stale),
and its `Output::Trades` is a question, `Query::Lockers { town }` / `Answer::Lockers { town, limit, placed, lockers:
Vec<LockerView { hand, managed, client, at, worth, offers: Vec<OfferView> }> }` (screens read `Ask`); schema
`190c9fa413786e45` (on `land-adapter`'s), FORMAT 1 (the save's shape grew: dev worlds' `enterprise_core.dat` are rebuilt). A `locker_host`
random-worlds slice; `tests/locker_host.rs`; `sim/scenarios/lockers.json` (the first scenario with a `HandLevel`). In the
mod: `adapters/LockerAdapter` (each look in core mode, before the stocktakes, every loaded villager's workstation with a
container touching it tied as its hand's locker, the point added, untied points removed, one question for every town;
the Merchant's screen built from `Query::Lockers` with keys the screen sends back, `ToggleTradePayload` sending `Switch`),
`StockAdapter` counting and writing locker points, `VillagerMixin` running the Java locker rules only in Java mode
(`LockerTrading.underJavaRules` for the Java rules' own tests in core mode), `/enterprise core lockers <desk>` (and
`/enterprise lockers` in core mode). `Rounds` and `LockerCatchUp` are not needed in core mode (the core's looks trade
lockers loaded or not; their listeners were already off there); they and `LockerTrading` stay for Java mode until
`delete-java-rules`.

**Built (B4.8 `wagon-adapter`).** The caravans are the core's; what the world shows of a wagon is its crew. In the core
(`wagon_host.rs`, no new saved state): `Input::Route { from, to, send }` starts a town's wagons to another (the
Merchant's screen, `/enterprise route add`): a route opens as a commissioned one does, if the far end is within a leg's
food desk to desk and there's a way, and one wound down opens again on the same terms (its number, books and road
kept); stopped, it's
wound down (`close_route`: its wagon comes home and stays). Refused for a town that isn't there or a town to itself;
too far opens nothing. Around a wire `Advance`, beside `hand_outputs`: `Output::HandAway { town, hand, tag }` for a
named hand that went away (a wagon's crew or porter, a soldier) and `Output::HandBack { town, hand, tag, trade, level,
xp }` for one home again, with what it is now (a hand taken on as a travelling Merchant comes home one: no `HandLevel`
comes while a hand is away), and a hand back counts as seen by the census from then. `Query::Routes { town }` /
`Answer::Routes { routes: Vec<RouteLine>, markets: Vec<MarketLine>, squad }`: its routes from and to it with their wagons
on the road and crews by hand and a word on where each stands, the markets it knows (desk, blocks, settlement, within a
leg's food, routed) and its squad at home. Schema `ac8763985aaf102c`, FORMAT 1 (the save's shape didn't change);
`just core-fingerprint` unmoved (`5e6b6d51a3b4061e`); the replay digest written again for the new random-worlds slice
(`wagon_host`: hosts starting and stopping routes, every wagon's crew away from its home, every routed market an open
route). `sim/scenarios/wagons.json`, the first to show the wagon's three outputs. In the mod: `adapters/WagonAdapter`
(a `HandAway` stows the villager with `Squad.stow`, kept by hand id, one not loaded then stowed at a look once it is; a
`HandBack` lets it out by its town's desk with `Squad.release` and writes its trade, level and XP, a desk not loaded
keeping it until it is; the stowed villagers are the view's own file, `enterprise_core_stowed.dat` beside the core's,
written when the host saves; a hand gone while away forgets its record), the Merchant's screen's caravans and its
"Send caravans"/"Stop" from `Query::Routes` and `Input::Route` in core mode (`MerchantView.of`, the packet's handler;
`MerchantView.TownView.reachable` is the core's word, `Caravans.inRange` only in Java mode), `/enterprise route
add`/`remove`/`routes` on the core in core mode and `/enterprise core routes <desk>`. Three game tests. **Crates and
papers are not here** (John, 2026-10-18: pallets live in the core only): the crates a wagon carries and their bills of
lading are the core's pallets; the mod's crate items, blocks and documents are drawn from the pallet outputs in
`pallet-views`, with no crate or paper rules in Java, so `cargo/Crates`, `Papers` and `Paperwork` stay Java mode's
until `delete-java-rules` and the row's crate and paper views go to `pallet-views`.

**Built (B4.12 `pallet-views`).** The core implements pallets and the mod draws them: no pallet rules in Java. In the
core (`pallet_host.rs`, `Stowage` in `saved!`): what the core keeps of a pallet's showing is where the host put its
packages (a desk) and the pallet as it was then (`since`, its digest), held while it stays there (a package lost keeps
it: the host's other crates are where they were) and gone when it moves, is opened or repacked. `Input::Pack { town,
consignee, goods }` (a player has the Merchant pack goods from stock for another town: `pack_pallets`, under no contract,
only what can be crated, a bill a pallet) and `Input::Stowed { point, pallet }` (the host put a pallet's packages in a
desk's chests); `Query::Stow { point, free }` (a chest takes a pallet whole or not at all: the town's pallets shown
nowhere, in arrival order, each whole if its packages fit the free slots, one slot a package, else passed over),
`Query::Pallets { town }` (held and inbound, with the registry's received, lost and lost goods), `Query::Pallet { id }`,
`Query::Manifest { route }` (a wagon's lines and whether it needs a superbill); `PalletView`, `ManifestLine`; the content
pack's `packages` (`PackPackage`: the kinds of packing, checked by the core, loose and the crate first; none listed, the
core's own four). Schema `b8fe731e20bb5c21`, FORMAT 1 (the save grew: dev worlds' `enterprise_core.dat` are rebuilt);
`just core-fingerprint` unmoved (`5e6b6d51a3b4061e`); the replay digest written again for the save's shape and the new
random-worlds slice (`pallet_host`). `sim/scenarios/pallets.json` is the first scenario with the three pallet outputs.
In the mod: `adapters/PalletAdapter` (core mode, at each look before the counts: crates destroyed told as `PackageLost`
and applied at once; every loaded desk's crates matched to their pallets as the core has them, stale ones taken out,
crates a player carried from another town's warehouse said as `PalletBrought` or `PackageBrought`, after the look's
`Advance` so a pallet brought in parts has a look for the rest; then `Query::Stow` with each desk's empty slots and the
offered pallets put in, `Stowed`; `PalletOpened` and `PalletMoved` off a warehouse take a pallet's crates out of every
loaded desk at once), `cargo/PackageTag` (the `enterprise:package` component: pallet, place, of, kind, consignee,
contract, since, digest, good, count; "1 of 3, to Wheatfell, contract 7"), the bill of lading as a `Papers` document in
the first package's documents slot, `ModPack.PACKAGES` (the pack's kinds of packing, one table: an item registered for
each kind the mod hasn't, `enterprise:barrel` and `enterprise:bundle`), a tagged crate never placed as a block,
`/enterprise core pallets <desk>` and `/enterprise core pack <desk> <to> <item> <count>`. Three game tests (298 both
modes). Measured in the game-test server: a look's pallet happenings with two desks and a pallet shown 0.185 ms of the
tick (two crossings), a pallet of three crates drawn 0.03 ms. Left: the superbill is a query and a command's line, not
yet a paper a player holds; empty crates (`Shipments.empties`) aren't shown; depots and lockers hold no pallets; the
Merchant's screen doesn't list pallets; the kinds of packing are read from the mod's table, not asked of the core after
a load; `cargo/Crates`, `Papers` and `Paperwork` stay Java mode's until `delete-java-rules`.

**Built (B4.9 `shop-adapter`).** The shops' rules were the core's already (a master Merchant's brokering, the board's
listings, rents and a dealer's prices); what the core lacked was which vendor was which. In the core (`shop_host.rs`,
`Vendors` in `saved!`; `Order.at`/`room`, `Stall.at`, `OwnShop.at`/`board`/`shown` in `orders.rs`): `Input::Vendor { point,
read }` is a host's count of one Numismatics vendor (a `Shop` point of a town's: mode, the filter's good or the rent it is
the deed to, lot and price in spurs, the items it holds, the lots it has room for, the bank card's balance, the coin the
host took out of its box, whether it touches a desk), and the core says what the vendor is: a buying vendor a player's
standing order, kept by where it stands so each count updates it in place (a good, a lot or a price changed is a new order and the
old contract closes), with the vendor's room a limit on the lots filled; a selling vendor a stall, its stock as counted; one
touching a desk the town's own shop (a shelf counted lower than the core's was bought from: used; coin collected is its
takings, banked by the master Merchant), or, told by `Input::Board`, one of the shop board's (paid four emeralds, its
takings banked at once and entered in the books as a sale, the listing at its place shown, a listed good the town can spare
beyond its keep and what other markets want on the shelf, a stack at most). `Output::OrderFilled` gained `point`;
`StallBought`, `Shelf { good, lot, price, add, clear }` (only what changed since the vendor was shown, only for a vendor
counted in the look), `Deed { selling, price, fund }`, `DeedPaid` and `BuildShop` are new. A freight hub's rents are its
dealer's: `Query::Deeds` lists them with the ask and bid in spurs and the vendor where each deed is sold (`Input::DeedShop`);
a deed bought (the vendor sold out, its takings paid in) makes the rent a player's and the vendor buys it back at the bid
from a card the core says to fund from the treasury (`fund`); a deed sold back makes it the dealer's again; `Input::
DeedBrought` hands a bearer the crop collected for them as far as the stock goes. `Query::Shops` is what `/enterprise
market` shows. A vendor point removed takes its shop with it (a shelf is used). Schema `dc7793b63586c15f` (on
`commands-and-studies`': Input 31-34, Query/Answer 24-25), FORMAT 1 (the save grew: dev worlds' `enterprise_core.dat` are rebuilt). Base worlds' behaviour unmoved
(`just core-fingerprint`), the replay digest written again for the save's shape and the new random-worlds slice
(`shop_host`). `sim/scenarios/shops.json`, the first to show `OrderFilled`, `StallBought` and `Shelf`. In the mod:
`adapters/ShopAdapter` (in core mode, each look registers the vendors within reach of a loaded desk as `Shop` points,
puts up the board vendors the core asked for and the hubs' deed vendors in the row by the desk, reads every vendor whose
town's desk and own chunk are loaded, and shows the outputs: goods into a vendor and the price off its card, lots out of a
stall and their price into its box, a listing set and a shelf added to or cleared, a deed offered or a dealer's red card set
and funded, the crop put in a player's hands), `RentDeeds` (the deed document as a view; right-clicking a desk with one
asks the core in core mode), `StockAdapter.good(Item)`, `/enterprise core shops <desk>` and `/enterprise market` on the
core in core mode. Five game tests (core-mode twins of MarketTests' order, stall, own shop and deed tests and MoodTests'
shop board, skip-listed as the binding's own); no existing game test changed. 958 tests pass, All 300 required tests
passed, core mode 300 of 300 (baseline written again). Measured in the game-test server: a look's vendors registered (the
desks' reach scanned, the deeds asked) 0.033 ms, a vendor counted 0.005 ms, a town's shops asked 0.099 ms. `Shops.collect`
read Numismatics' cached coin sum, which `clear()` leaves behind, so a box emptied once read as full at every later
collection (the Java board banked phantom takings every look): it sums the coins now (`Shops.box`). Left: enchanted commissions (the vendor
deals in nothing the core knows), the dealer's trades in rents between towns (the core has none), the vendors of the crown,
the metal exchange, the requisitions and the freight hubs (`crown-views`), a deed vendor driven end to end in the world
(the core's rents come from its rules; the game test shows the vendor's half with the outputs the Rust tests pin). At landing: a look deals only with the vendors counted for it (`Vendors.counted`, cleared as each
`Advance` ends), so a vendor whose chunk unloaded isn't filled, bought from or stocked on the books while the world can't
show it; a vendor's box is emptied only where the core says the coin is the town's (touching the desk, or a deed shop in
its `Query::Deeds`), never a player's own vendor reselling a deed; a buying vendor's price changed is a new order, as
`Broker.contractFor` matches a delivery by its price.

**Built (B4.10 `crown-views`).** The crown's rules were the core's already (`crown.rs`, `army.rs`, `realms.rs`,
`sourcing.rs`: mint, stockpile, levies, requisitions, the front, the exchange, realms); what the core lacked was a way for a host to
start them and a place to show them. In the core (`crown_host.rs`, `CrownShop` in `saved!`; `Kind::Market` / `PointKind::Market`
in `points.rs`): `Input::Capital`, `Mobilise`, `Declare` (a theatre by name) and `Peace` are the sovereign's commands
(`Output::WarDeclared { levies }`, `PeaceMade { home }`); `Input::OffMap { name, at, sells, realm, spurs, partners }` opens a
market off the map at a `Market` point (a host's own, a realm "coast" or "march", money paid in at the mint's rate, its roads
to the towns it trades with); the front and the market across the sea the core opens itself are `Market` points too
(`mark_off_map`, beside the capital's desk's dimension), so `Query::Points` lists every market off the map. The gold shop:
mobilising, the core asks for a vendor by the capital's desk (`Output::BuildCrownShop`, asked again at each look that counts the
capital while none stands: nothing waits in the host), the host puts one up and says so (`Input::CrownShop`), and from then each look's `Input::Vendor` count of it is the crown's (`read_crown_shop`): it shows 8 gold at
`gold_premium` times its worth (`Output::CrownShop { point, good, lot, price, fund, taken }`), its card kept funded to two lots'
price from the capital's treasury (`cash_book::post`, "gold shop"), the gold sold into it the crown's (delivered and sent to the
mint: `Ledger`), and at peace it comes down (`CrownShopDown`, said again at each count of it until the host removes its point,
so a vendor out of reach at peace is never taken for a player's). `Query::Realms` and `Query::Metals` are what `/enterprise realms`
and `metals` show. Schema `41c307f6fa0993a8` (Input 35-40, Output 23-27, Query/Answer 26-27, `PointKind::Market`, `RealmLine`,
`MetalLine`), FORMAT 1 (the save grew: dev worlds' `enterprise_core.dat` are rebuilt). Base worlds' behaviour unmoved
(`just core-fingerprint` `d8a270619eff9cdb`), the replay digest written again for the save's shape. `sim/scenarios/crown.json`
(capital, realm, mobilising with the gold shop, war, peace, every refusal), `tests/crown_host.rs` (13), a `crown_host`
random-worlds slice (a capital named, mobilised, the gold shop counted, peace). In the mod: `adapters/CrownAdapter` (the commands,
the realm opened at a block, the gold shop put up on open floor near the capital's desk and shown, the books), `PointsAdapter.Kind.MARKET`;
in core mode `/enterprise war capital|mobilise|declare|peace`, `realms`, `realms open` and `metals` are the core's.
Three game tests (core-mode twins of WarTests' gold shop and Realms.open, skip-listed as the binding's own); no existing game
test changed. 972 Rust tests pass, All 308 required tests passed, core mode 308 of 308 (baseline written again), 411 JVM
tests (`just binding-test`). Measured in the game-test server: the gold shop's place check 0.00005 ms a look, the realms and metals
asked 0.177 ms. The Java `Sovereign`, `Requisitions`, `Army`, `Realms`, `OffMap` and `MetalExchange` are unused in core mode (their ticks
were never registered there) and go with `delete-java-rules`. Left: the metal exchange's vendors and the requisitions' buying vendors
(players selling metals or kit at the capital's port: the core's exchange and requisitions deal with markets' own Merchants only),
the gold shop's leftover card balance when it comes down (money parked on a card, as a deed vendor's), the front and the market
across the sea have no block to show (the war is shown by its soldiers, hands away and back), a realm's roads need the
host to give its partners at opening.

### 3.5c Wave B4c: representative world fixtures (John, 2026-10-23, lean)

Before the Java rules go, each concern gets 3 to 6 in-world game tests that drive the core through the binding (no
call into a class `tree/delete-java-rules` deletes), each a different path through its adapter: the same kind of
blocks, villagers and chests, an outcome looked for in the world. The rest of a concern's world fixtures and its
rule-API tests retire against their Rust twins (`scripts/parity-retired.txt`). The kept features of
`docs/decisions.md` (2026-10-23, lean) get their core rule, wire message and adapter with Rust tests; anything else
dropped goes on its lost list with why. `just gametest-core` falls only by tests named retired or lost.

| Order | Task | Concern (and features it keeps) | Waits for | Model |
|---|---|---|---|---|
| B4c.1 | `fixtures-caravans` | Caravans, freight and logistics: trips and crews, porters, rations, the cleric, hubs and forwarders, expeditions; the freight hubs' vendors | — | opus / high |
| B4c.3 | `fixtures-post` | The post and counting houses: a contract's copy where a wagon runs, `post`, `paperwork` | `fixtures-caravans` | opus / high |
| B4c.4 | `fixtures-market` | Lockers, markets, money and metal: lockers by distance and by switched-on trades, a sale into the till; the metal exchange's and requisitions' vendors, renaming a market (lost: `bank`, `tenders`) | — | opus / high |
| B4c.5 | `fixtures-land` | Land and work: tolls as coin, set by command, `bridge`; stalls built by Jobs, mine heads, pastures; a field, a building | — | opus / high |
| B4c.6 | `fixtures-people` | People, moods, services and war: a census, a mood, a Librarian's books on an unknown stocktake, enchanting and book copying, a levy; the seven Java-only dials (each kept where its rule is, else lost) | — | opus / high |

B5.1 `delete-java-rules` then runs again in its held tree, rebased on all five, keeping every rewritten fixture.

### 3.6 Wave B5: closing

| Order | Task | Goal | Waits for | Java | Model / effort |
|---|---|---|---|---|---|
| B5.1 | `delete-java-rules` | Core mode the only mode: the rule classes of section 2 deleted with their rule-API tests (each with a Rust twin, `parity.py --binding` says so), the 33 `SavedData` down to `enterprise_core.dat` and the views' own (stowed entities' NBT, chest snapshots) | all of B4 | **yes**: `just gametest`, all remaining passing | opus / high |
| B5.2 | `scenarios-in-game` | `architecture.md` convention 5: the scenario scripts run in the real server through the binding (`just binding-scenarios`, a headless server on the tree's ports), the same expected answers | `delete-java-rules` | yes | sonnet / medium |
| B5.3 | `upgrade-drill` | The measure of section 5 taken: a small real core change (a new `Output` from the pallet work, or one made for the drill) carried through the binding by a fresh agent with only this plan; time and files touched written up | `delete-java-rules` | yes | sonnet / medium |

**Model mix:** 24 tasks, 12 opus, 12 sonnet, no haiku (every task here runs a build or a generator that must be right
for every later one). Opus where a shape is set (the wire, the generator, the host, NeoForge's lifecycle, stable
hands, the chests) or where many systems meet (wagons, people, land); Sonnet for the well-specified rest.

**Java runs.** None in B1 but `javac` on the generated code (`wire-gen`) and `:binding:test`, a plain JVM (`chicory-host`, `versioning`). From `proof-in-mod`
on, every task builds the jar and runs the game tests. Two Java trees at once at most on a 24 GB machine (a Gradle
daemon and a game-test server each), through `trees.py heavy`; the B1 tasks run beside them freely.

## 4. Risks, and what to measure first

**Measured first, in `chicory-host`** (the spike is that proof's numbers, not a 200-day replay):

| Measurement | Bar | If it misses |
|---|---|---|
| A look on a study-sized world, compiled | under 1 ms (estimate 0.13 to 0.3) | `CoreClock` spreads looks over ticks; the interpreter only for what wouldn't compile |
| Functions the build-time compiler refuses (JVM method size) | none on the hot path | `interpretedFunctions` for cold ones; split hot ones in Rust (a `match` per module, not one) |
| Instantiation | under 2 s at server start | the runtime compiler's directory cache (Chicory 1.6), or accept it |
| Module size, jar growth | under 10 MB jar growth | strip names; `opt-level = "s"` for the wasm crate only |
| Heap | under 200 MB for a 365-day world | the core's own caches (`Local`s) sized down |
| Save at autosave | under 50 ms | save every few autosaves, keep the input log between |

**Measured** (`chicory-host`, 2026-10-18: `just binding-spike`; its tables in `binding/build/spike.md`). Chicory 1.7.5 on
JDK 21 (Temurin 21.0.7, aarch64, Apple silicon with 10 cores, load average about 4 as each part began: other trees'
tests share it), the module as `just wasm` builds it. A world through the wire: the study host writes the world as a
`Load` of its save, a year of looks (an `Apply` advancing 1,200 ticks: 7,300) and a `Save` (`--wire-looks`, `--world`),
and native (the module's own framing), Node's V8 and the JVM play the same script. The wire has no inputs that make routes
or people yet, so `Create` and setup inputs can't build a study world; a `Load` of one is the same society. **Every response
in every JVM run (and Node's) was the native core's, byte for byte.**

| Measurement | Bar | Measured | |
|---|---|---|---|
| A look, compiled | under 1 ms | **study world** (9 towns, 14 routes, day 30, 75 KB save): 2.34 ms a look (its thread's CPU the same) against native's 0.062 and Node's 0.088: **38x native**, 27x Node; p50 2.2, p99 5.6, the slowest 107 ms; a day 47 ms. **Random world 1234** (25 towns, 80 routes, day 150, 254 KB save): 9.1 ms against 0.52 (17x), the slowest 0.7 s, a day 183 ms. The scenario world (one town): 0.49 ms | **missed**, ten times the estimate (2-5x native) |
| Interpreted, and compiled at load | | interpreted: study 27 ms a look (440x native), random world 0.67 s; Chicory's runtime compiler: study 3.7 ms (its first 600 looks) | out |
| Functions the compiler refuses | none on the hot path | none of the 4,811 fell back to Chicory's interpreter. But **75 compiled methods (1.2 MB of bytecode) are over HotSpot's 8,000-byte limit**, and HotSpot doesn't JIT a method that big (`-XX:+DontCompileHugeMethods`, the default): the JVM interprets them. They're the hot ones: in a profile of the study's year 67% of samples are in JVM-interpreted frames, `people::live`, `rules::plan::decide` and `plan_sale`, `services::plan`, `desires::plan_shopping`, `herds::tend_towns`, `works::review`, `farms::pay` at the top; Chicory's `call_indirect` dispatch is one too. `CompileCore`'s report lists them all (named with `coreWasmNamed`) | **the cost** |
| Instantiation | under 2 s | compiled: 76 ms to read the meta module and load the classes, 89 ms the first instance, **0.7 ms each after** (a trap's recovery); interpreted 0.56 s; compiled at load 1.8 s | met |
| A call's round trip | | 1.9 µs (an `Ask` of nothing), 4.3 µs (one town's stock); interpreted 17 and 54 µs | |
| Save and load, 254 KB | under 50 ms | `Save` 10 ms, `Load` 36 ms (p50; the crossing and the Java decoding in it) | met |
| Jar growth | under 10 MB | **2.3 MB**: binding.jar 1.9 MB (the machine's 6.8 MB of classes compressed, the 162 KB meta module, the wire, `Core`) and chicory's runtime, wasm, wasi and log, 0.43 MB | met |
| Heap | under 200 MB | **12 MB** after the study's year, the core's instance live (7 MB of it the module's memory) | met |

**What was tried for the speed** (the study's year, the thread's CPU a look, three runs each on a busy machine): opt-level
`z` (15 methods over the limit, not 75) and `s`, no better; `-C llvm-args=-inline-threshold=0` about 15% better (within
the spread); Chicory's `ByteArrayMemory`, no better; `-XX:-DontCompileHugeMethods` (a flag the mod can't set on a
player's JVM anyway): C2 compiles the big methods, then deoptimises them again and again, no better. With `z`, memory
accesses through Chicory's paged memory are 40% of the samples: wasm keeps Rust's stack in linear memory, so every spill
is a call the JIT must inline. The responses were native's in every variant.

**What follows.** The look runs on the core's own thread (`CoreWorker`, in `binding/` now, for `proof-in-mod`'s `CoreClock`):
2.3 ms of CPU a game minute is nothing, and the tick only crosses. To make it faster (not needed for the budget, wanted for
catch-ups and bigger worlds), split the hot functions in Rust so each compiles under 8,000 bytes of bytecode (a step's
loop body into functions of their own, a `match` into several): about two thirds of the time would then be JIT-compiled,
an estimated three times faster (0.8 ms a look). That is a core task touching several slices' systems, for the
integrator to place; `CompileCore`'s report is its checklist. Upstream, Chicory splitting a huge function into several JVM
methods would do it for every module.

**Risks:**

- **The wasm and native builds disagree today** (section 0). Until `wasm-digest` lands nothing in this plan can trust
  a contract test. After it, any ambiguous Bevy order a landing adds can split them again: the schedule-order test it
  adds catches that natively.
- **Chests against the ledger.** Two writers (Create, players; the core) on one inventory. The stocktake-and-`Moved`
  design is the lockers' catch-up rule made general; the risk is a look's `Moved` meeting a chest a hopper filled since
  the stocktake. The adapter applies `Moved` against room and reports what didn't fit as a happening; the core books
  it back. `stock-adapter`'s game tests must include a hopper filling a chest during a wagon's sale.
- **The schema churns with every core slice.** Variant indexes are positional: a variant inserted mid-enum renumbers
  the rest. `wire-gen` keeps enums append-only against the committed registry, so old fixtures and logs stay readable.
- **The generator is ours.** Kept small and pinned by fixtures both sides read; if it costs more than it saves,
  serde-generate's Java with a postcard `Serializer` subclass is the fallback.
- **Game tests that drive Java rules.** 66 can't run in core mode as written. Retiring them leans on the Rust
  twins' honesty (`parity:` tags are self-reported, `worktrees.md` pain 5): `gametest-triage` flags the ones with no
  twin, and partials (25) are read against the Java's asserts before their Java test goes.
- **Two societies in the mod during B4.** The switch doubles what a change can break for a while. Mitigation: B4 lands
  concern by concern quickly, and no rule work goes into the Java side after `core-mode` (a fix goes to the core).
- **A trap loses a look.** `panic = "abort"` kills the instance; the input log since the last save replays it. A
  panic on replay is a stuck world: `CoreHost` then pauses the society (views frozen, the game playable) and says so.
- **Compute on touch meets Minecraft's lag.** A view that loads after days catches up in one write (a locker's level,
  a chest's goods). Large writes spread over ticks as `LockerCatchUp` does today (8 a tick).
- **Determinism across JVMs.** Chicory runs wasm `f64` ops with Java's `double`, which is IEEE strict since Java 17:
  the same bits. The JVM scenarios' expected answers are the native ones, so a difference fails `binding-test`.

## 5. How an upgrade goes (the measure of success)

**A core change with no wire change** (a rule, a constant, new state): nothing in Java. `just wasm` (about 30 s),
`just binding-test` (the scenarios' expected answers rewritten if behaviour moved, as `replay_digest.txt` is), the
jar rebuilt. **Target: under 10 minutes, zero Java files touched.** If the state's shape changed after the first
release: a `FORMAT` bump and a migration with its golden save, **under an hour**.

**A new message for an existing view** (a slice reports something new, the Merchant's screen shows it):

1. Add the variant to `sim/core/src/wire/` (appended), its arm in `wire::handle` over the slice's host call.
2. `just wire-gen`; `just wire-fixtures`; extend or add a scenario.
3. Java doesn't compile until `Outcomes` routes the variant: one line, and the adapter's method.
4. `just binding-test` (no Minecraft); a game test only if the view changed.

**Target: under an hour, one adapter file and the generated code touched.**

**A new Minecraft concern** (pallets as blocks: a new point kind, new happenings, new outcomes): the steps above plus
a new adapter class with its view's blocks and items and their game tests. **Target: the binding part under half a
day; the view work is the view's.**

**How it's kept honest.** Each landing after `delete-java-rules` notes in `docs/core-progress.md` the minutes from
core landing to binding green and the Java files touched outside `binding/adapters/` and the generated folder (the
goal: none). `upgrade-drill` takes the first reading. If an upgrade touches more, that's a finding: the boundary is
leaking state, and the fix is in the wire, not in more adapters.

## 6. For John

1. **Chests stay physical?** The plan keeps desk and locker chests as real inventories Create and players use, synced
   with the core each look (stocktakes in, `Moved` out). The other way: the core's warehouse is the only stock and
   chests become a window with a fixed exchange slot. Physical is more Minecraft and more risk.
2. **When do saves start to survive core changes?** Until the first release the plan rebuilds dev worlds on a format
   change (no migrations). Say when that promise starts (a public release, or the first world you'd hate to lose).
3. **New dependencies:** Chicory (runtime, WASI, build-time compiler) in Gradle, with the experimental community
   Gradle plugin or a `JavaExec` instead; `serde-reflection` in `sim/wire-gen` only (the core gains none). And the
   format: postcard with our own Java emitter, rather than serde-generate's Java on bincode (unmaintained).
4. **Retire the 66 rule-API game tests with their Java classes,** their Rust twins standing in? The alternative is
   rewriting them to drive the binding, which tests the core twice and the binding little.
5. **The `java | core` switch for the length of B4,** two societies in the mod for a while, rather than a cut per
   concern that leaves the game half-working between landings.
6. **The Java-server studies** (`network_study.py`, `forks.py`, lanes): rewrite them on the core's answers in
   `commands-and-studies`, or keep the real server only for confirming what `sim/host` finds and retire most of them.
