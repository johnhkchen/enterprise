# Core math: a checklist

Tricks to steal for the Rust core (`sim/core`), each with where it fits and how far along it is. Pick them up as a
slice needs them; tick them here when they land.

**Key:** ✅ in use · 🚧 started · 🔜 planned, with the slice it goes in · 💭 maybe, if a need shows up

## Representation (what the world is)

| | Trick | Where it fits | Status |
|---|---|---|---|
| 🚧 | **Leontief input-output matrix:** recipes as a linear map, inputs to outputs; shocks propagate by matrix algebra | Workshops and recipes slice: a town's conversions as a matrix; what a shortage of one input does downstream, computed, not simulated | Workshops. Started: `workshop::input_demand` is the two-step back-substitution `A d`, `A (A d)` over the conversions; the full inverse and shock propagation aren't done |
| ✅ | **Double-entry bookkeeping (REA ledger):** every transfer a balanced entry; conservation by construction | `cash_book.rs`: every transfer (sale, contract payment, bill, road food, wages, comforts, homes, children) is a `post` between accounts (a town's treasury, its households, a wagon's purse, the outside), noted on both sides by day and reason; `check.rs` reconciles every account's balance and the world's money from the postings, and the ledger's running totals against the outside's | In use |
| 🔜 | **Bill of materials as a DAG:** recipe chains as a directed acyclic graph; depth and dependencies precomputed | Recipes from mods (Create's chains): order production, find what a good ultimately needs, spot cycles | Workshops |
| ✅ | **Contracts as state machines:** posted, accepted, in transit, delivered, failed, with deadlines and collateral | `contracts.rs`: drafted → active → completed / broken; installments judged at their deadlines. Collateral (advances, escrow) still to come | Contracts (advances later) |

## Flow and prices (what moves and where it settles)

| | Trick | Where it fits | Status |
|---|---|---|---|
| ✅ | **Spatial price equilibrium:** the price gap between two places settles at transport cost plus risk | Tolls: `chokepoints.rs` asks a route at most 80% of what going around its posts costs, so a toll settles at the detour's price; the random worlds check no town's plan asks a route more than its detour (toll and fee together). Still a test target for prices: once they move, the gap between two towns should approach the road's cost a unit | Tolls in use; prices |
| 💭 | **Min-cost network flow:** optimal routing and allocation | Deciding what many haulers do at once (the forwarder's blocks, a hub's runs) without simulating each; or as a yardstick for how close wagons get to optimal | Freight hub, maybe. A yardstick only (`tests/freight.rs`, `a_hubs_routing_of_three_towns_shipments_against_the_direct_roads`): three towns round a hub, two stacks a trip to each other, the optimum found by trying each pair's way (its own road, or the hub); the hub's routing as the forwarder prices it is within 6% of that optimum (51.8 against 49.1 a trip) and 62% of the direct roads (83.4). The rules route by price (`freight.rs`: the soonest leg, blocks first; long runs wound down at 3/4), not by solving a flow |
| 💭 | **Contraction hierarchies:** fast shortest paths on big graphs | Real terrain with 50+ villages: route costs over the hex graph | Maps from real worlds |
| 🔜 | **Tâtonnement:** price moves in proportion to excess demand | A good's worth is a fixed value (seeded from the mod's table, `prices.rs`), but a per-town price now exists at the point of sale: the far end pays between half and all of it by its shortage below par at the middle of the batch, split by the Merchants' levels (`bargain.rs`, the mod's `Bargain`). Still to come: a per-good, per-town price that moves with excess demand each day, simple and stable | Prices |
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
| ✅ | **Bullwhip effect / order-up-to policy:** small demand swings amplify upstream; a one-line policy per node | Shelves are order-up-to: a town's par is `shelf_days` of what its lockers can take a day (`Trades.capacity`, from `lockers.rs`: price × uses for its managed lockers' switched-on trades, one restock's worth counted as a day's as in the mod), a level that doesn't move with the day's swing, and a locker's restock takes what the warehouse holds up to its uses, never ahead of it. `tests/lockers.rs` shows a daily swing in what comes in (100 and 300 paper on alternate days) passes through no larger than the two whole lots a restock can leave over. Still to show along a chain of towns | In use (one node) |
| 🔜 | **Reorder point and safety stock (s, S):** near-optimal inventory for NPC firms | The departure rule's "full or on its day" is a crude (s, S); a buyer reordering at s up to S is the cleaner version, and sets shelf depth from the delivery interval (the cash finding) | Wagons in full |
| 🚧 | **Logistic growth with carrying capacity:** settlement growth and saturation | A town's growth bounded by its land and beds, with births slowing as it nears the bound | People. Started: `herds::tend` grows a fed herd by `breed_per_day` of itself, never beyond the room its pastures hold (`pasture_animals` each), and a Butcher culls only above `cull_share` of that room |
| 💭 | **Lotka-Volterra predator-prey:** raiders and traffic, with over-raiding collapse | Bandits on busy roads: traffic feeds raiders, raiders drive traffic off | Raids, maybe |
| 💭 | **Cascading failure / percolation:** when losing a node or road splits the economy | A check on maps: which hub's loss fragments the network; worth a report per world | Maps, maybe |
| 🔜 | **Hazard rates / survival models:** per-road loss as a rate, updated by observed losses | Risk in prices (spatial equilibrium's "plus risk") and in which road a wagon takes | War / raids |

## Agents and strategy

| | Trick | Where it fits | Status |
|---|---|---|---|
| ✅ | **Utility or expected-profit decision rules:** one line per agent type | A wagon goes when its load is worth its road food (`departures`); a town buys within its budget. More of the same as slices land | In use |
| ✅ | **Lanchester equations:** attrition combat as supply-scaled strength | `army.rs`: the front's losses as a rate against a fixed foe, `dN = Σ risk` a day (each soldier's by its kit, its hunger and how hearty the meal), and desertion as the hungry's summed chance; the expected losses carried as a fraction and taken a whole soldier at a time (the one who has run the most risk), never a roll a head, so the same army in the same kit loses the same soldiers on the same days whatever the dice (`tests/army.rs`, `losses_are_the_expected_value_carried_not_dice`). Still to come: an enemy whose strength the army's (the tally's `strength`) wears down in turn, the square law | In use (attrition; an enemy to fight, later) |
| ✅ in use: advances | **Principal-agent / collateral contracts:** escrow and deposits make risky shipments possible | Advances against contracts: a share paid up front by the buyer's wagon, worked off at the same share of each delivery, owed back on a break (`advances.rs`, `rules/advances.rs`); deposits on consignment still to come | Contracts (advances) / forwarder |
| 💭 | **Replicator dynamics:** strategies that earn more get copied | Towns' choices (what to make, which routes to run) drift toward what pays, without learning code | Maybe, later |

## Validation

| | Trick | Where it fits | Status |
|---|---|---|---|
| ✅ | **Stock-flow consistency checks:** totals reconcile every step | `check.rs`: goods and money conserved and no town in debt, every day of every test and host run | In use |
| ✅ | **Property-based testing:** invariants across thousands of random seeds | `tests/random_worlds.rs`: 300 seeds × 120 days, each slice turned on by seed through its own generator (`tests/common/slices/`), every day checked for conservation, solvency, every slice's invariants and the sweep's (`random_worlds_sweep.rs`: mood above its floor, no wagon out a month, no idle route without a word why, no contract left unjudged, wages paid or credited), and every 30 days saved, loaded and run on to the bit. The seeds run across the cores (`common::each_seed`); `sim/host --sweep N` prints what the worlds did | In use |
