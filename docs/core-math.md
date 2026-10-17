# Core math: a checklist

Tricks to steal for the Rust core (`sim/core`), each with where it fits and how far along it is. Pick them up as a
slice needs them; tick them here when they land.

**Key:** ✅ in use · 🔜 planned, with the slice it goes in · 💭 maybe, if a need shows up

## Representation (what the world is)

| | Trick | Where it fits | Status |
|---|---|---|---|
| 🔜 | **Leontief input-output matrix:** recipes as a linear map, inputs to outputs; shocks propagate by matrix algebra | Workshops and recipes slice: a town's conversions as a matrix; what a shortage of one input does downstream, computed, not simulated | Workshops |
| 🔜 | **Double-entry bookkeeping (REA ledger):** every transfer a balanced entry; conservation by construction | Today the ledger counts what enters and leaves the world, and `check.rs` reconciles. Next: every transfer (sale, contract payment, bill, road food) as a posting between accounts, so the books balance by construction and a cash book per town falls out | Before the post slice |
| 🔜 | **Bill of materials as a DAG:** recipe chains as a directed acyclic graph; depth and dependencies precomputed | Recipes from mods (Create's chains): order production, find what a good ultimately needs, spot cycles | Workshops |
| ✅ | **Contracts as state machines:** posted, accepted, in transit, delivered, failed, with deadlines and collateral | `contracts.rs`: drafted → active → completed / broken; installments judged at their deadlines. Collateral (advances, escrow) still to come | Contracts (advances later) |

## Flow and prices (what moves and where it settles)

| | Trick | Where it fits | Status |
|---|---|---|---|
| 🔜 | **Spatial price equilibrium:** the price gap between two places settles at transport cost plus risk | A test target: once prices move, the gap between two towns should approach the road's cost a unit. A check that the economy prices distance right | Prices |
| 💭 | **Min-cost network flow:** optimal routing and allocation | Deciding what many haulers do at once (the forwarder's blocks, a hub's runs) without simulating each; or as a yardstick for how close wagons get to optimal | Freight hub, maybe |
| 💭 | **Contraction hierarchies:** fast shortest paths on big graphs | Real terrain with 50+ villages: route costs over the hex graph | Maps from real worlds |
| 🔜 | **Tâtonnement:** price moves in proportion to excess demand | Prices today are fixed values; the mod bargains on shortage (`Bargain`). A per-good, per-town price that moves with excess demand each day, simple and stable | Prices |
| 🔜 | **Square-root price impact:** a big order moves the price by about √(size / depth) | Keeps a dominant player honest when selling a whole factory's output into one town | Prices |
| 💭 | **Constant-product market maker (x·y = k):** a closed-form market with price impact, no order book | A player-facing exchange at a hub, cheap per trade | Maybe, for player markets |

## Time and speed (how to fast-forward)

| | Trick | Where it fits | Status |
|---|---|---|---|
| 🔜 | **Discrete-event simulation with a priority queue:** jump to the next event instead of ticking | Today the core looks every 1,200 ticks. Arrivals, deadlines, post days and departures are all known times: a queue would jump straight between them. Needed for 50+ towns over 10,000 days | When the looks get dear |
| 🔜 | **Poisson / exponential arrivals:** sample *when* the next event happens | Raids, losses on the road, births, deaths: one sample for when, not a roll every look | People (births), war (raids) |
| 💭 | **Gillespie algorithm:** exact event-driven stochastic simulation | If many random processes interact (raids, births, desertions), so rare events cost nothing while idle | Maybe |
| 💭 | **Tau-leaping / fluid approximation:** many small events as one batch of expected flows, back to discrete when detail matters | Far-off towns as flows, the ones near a player in detail: a natural fit for "compute on touch" | Maybe, at scale |
| 🔜 | **Fixed-point arithmetic:** deterministic across platforms | Today `f64`: deterministic on one machine (the replay test checks to the bit), but WebAssembly in Java against native could differ in the last bits. Money and counts as fixed-point integers before replays cross hosts | Before the Minecraft binding |

## Risk, shock, and collapse

| | Trick | Where it fits | Status |
|---|---|---|---|
| 💭 | **ARIO (adaptive regional input-output):** capacity loss from a shock, propagated through supply chains, slow recovery | War, a closed road, a lost hub: how far the damage spreads and how fast towns recover | War, maybe |
| 🔜 | **Bullwhip effect / order-up-to policy:** small demand swings amplify upstream; a one-line policy per node | Shelves are already order-up-to (each town restocks to `shelf_days`). A test should show whether swings amplify along a chain, and the policy should damp them | People / workshops |
| 🔜 | **Reorder point and safety stock (s, S):** near-optimal inventory for NPC firms | The departure rule's "full or on its day" is a crude (s, S); a buyer reordering at s up to S is the cleaner version, and sets shelf depth from the delivery interval (the cash finding) | Wagons in full |
| 🔜 | **Logistic growth with carrying capacity:** settlement growth and saturation | A town's growth bounded by its land and beds, with births slowing as it nears the bound | People |
| 💭 | **Lotka-Volterra predator-prey:** raiders and traffic, with over-raiding collapse | Bandits on busy roads: traffic feeds raiders, raiders drive traffic off | Raids, maybe |
| 💭 | **Cascading failure / percolation:** when losing a node or road splits the economy | A check on maps: which hub's loss fragments the network; worth a report per world | Maps, maybe |
| 🔜 | **Hazard rates / survival models:** per-road loss as a rate, updated by observed losses | Risk in prices (spatial equilibrium's "plus risk") and in which road a wagon takes | War / raids |

## Agents and strategy

| | Trick | Where it fits | Status |
|---|---|---|---|
| ✅ | **Utility or expected-profit decision rules:** one line per agent type | A wagon goes when its load is worth its road food (`departures`); a town buys within its budget. More of the same as slices land | In use |
| 🔜 | **Lanchester equations:** attrition combat as supply-scaled strength | The war's outcome from soldiers, kit and food, no per-unit simulation | War |
| 🔜 | **Principal-agent / collateral contracts:** escrow and deposits make risky shipments possible | Advances against contracts (the mod has them), deposits on consignment | Contracts (advances) / forwarder |
| 💭 | **Replicator dynamics:** strategies that earn more get copied | Towns' choices (what to make, which routes to run) drift toward what pays, without learning code | Maybe, later |

## Validation

| | Trick | Where it fits | Status |
|---|---|---|---|
| ✅ | **Stock-flow consistency checks:** totals reconcile every step | `check.rs`: goods and money conserved and no town in debt, every day of every test and host run | In use |
| 🔜 | **Property-based testing:** invariants across thousands of random seeds | Random worlds (towns, trades, routes, contracts) checked for conservation, solvency and no stalls, in seconds | Next, alongside people |
