# Pallets: the bill of lading on a handling unit

The first work after parity (John's decision of 2026-10-17, after a talk with a logistics expert). Freight today is a
loose mix of stacks, crates, shulkers and bills, and the bill of lading sits on the crate: a bill a crate, one good a
crate, as `crates-and-papers` ported it from the mod. That was right for parity. Here the core changes to a nested
chain of handling units, each moved whole: a wagon (or a hub's consolidation) holds **pallets**; a pallet holds
**packages** (crates, barrels, bundles); a package holds **units** of one good. The pallet carries the bill of
lading, one bill to a pallet.

This is a design and a task plan, not code. John's decisions are settled (the memory note "handling units after
parity"); this file decides the rest and says why. What only John can decide is in "For John" at the end.

Read with: `docs/decisions.md` "Crates and bills of lading", "Bills of exchange", "Freight: carriers, forwarders,
drayage", "Stranded paper"; `docs/core-plan.md` 3.5, 3.6, 3.9, 4.1, 4.7; `sim/core/src/cargo.rs`, `freight.rs`,
`stranded.rs`, `rules/cargo.rs`, `rules/freight.rs`, `rules/plan.rs`.

## Why

- **Whole units end the races.** Today a plan clones the crates it means to take, and the wagon finds them again by
  equality as it leaves ("another wagon may have taken one since"). A hub's booked crates ride as copies in
  `Aboard.booked` and `Aboard.forwarding`, matched to `Wagon.shipments` with `==`: two crates alike in good, count and
  consignee can't be told apart. A pallet has an id, one place, and one mover. Nothing is found again by its contents.
- **The bill belongs on the unit a worker handles.** Handling instructions are the bill's. Odd freight becomes small
  pallets with their own bills, not loose items a carrier has to remember.
- **Limits are reckoned per unit.** A pallet has a weight and a volume; a carrier holds so many pallets within its
  caps. A carrier reasons about pallets, never their contents.
- **It's the standard.** Pallets, consolidations and master bills are how freight has been handled for a century, so
  the next steps (lots, value over time, itineraries, insurance) have well-trodden shapes to follow.

## 1. The model

### The chain

| Level | What it is | Moved by | Paper |
|---|---|---|---|
| **Container** | A wagon on a trip, or a hub's consolidation of pallets for one run | Its carrier | A **superbill** (manifest) naming its pallets' bills, when it holds two or more |
| **Pallet** | The packages one shipper sends one consignee under one bill, about what one worker moves at a time | Whole, by any carrier that takes its kind | Its **bill of lading** (1:1) |
| **Package** | A crate, a barrel, a bundle, or loose stacks: one good each for now | Only inside its pallet while sealed | None of its own |
| **Units** | Items of a good | Only at a holder's premises | None |

### Data in the core

A new module `pallet.rs`. Everything is saved (`saved!`), deterministic (`BTreeMap`, `Vec`), and has a `Default`.

```rust
pub type PalletId = u64;

/// The packing units (content, like goods): a crate holds 9 stacks, as today.
pub struct PackageKind { name: String, slots: u32, tare: f64, emeralds: f64, returnable: bool }

pub struct Package {
    pub kind: PackageKindId,   // crate, barrel, bundle, or `LOOSE` (no package: stacks on the pallet)
    pub good: GoodId,
    pub count: f64,
    pub batch: Option<Batch>,  // reserved for value over time (step 2); None now
}

/// Who pays for the goods on the bill: replaces `Shipment.settled` and the mod's "billed" lading.
pub enum Payment { Owed, Paid, ByBill(usize) }

/// The bill of lading: the receipt for the goods and the title to them.
pub struct Lading {
    pub shipper: TownId,
    pub consignee: TownId,
    pub contract: Option<usize>,
    pub payment: Payment,
    pub copy: bool,            // carries a copy of the contract
    pub handling: Handling,    // the instructions: plain for now (see below)
    pub issued: u64,
}

pub struct Seal { pub at: u64, pub by: TownId, pub digest: u64 }

pub enum At { Town(TownId), Custody(TownId), Wagon(RouteId), Container(ContainerId) }

pub struct Pallet {
    pub id: PalletId,
    pub kind: PalletKindId,
    pub lading: Lading,
    pub packages: Vec<Package>,
    pub at: At,
    pub since: u64,            // when it came to `at` (dwell times; exposure later)
    pub seal: Option<Seal>,
    pub claimed: Option<RouteId>, // a plan's claim this look (task `pallet-claims`)
}

/// Every pallet in the world, where it is, and the one way to move it.
#[derive(Resource)]
pub struct Pallets {
    pub next: PalletId,
    pub pallets: BTreeMap<PalletId, Pallet>,
    pub by_place: BTreeMap<At, Vec<PalletId>>, // in arrival order: loading order is deterministic
    pub containers: BTreeMap<ContainerId, Container>,
    pub received: u64, pub lost: u64,          // tallies for the books
}
```

- **One registry, not pallets inline.** Today crates live in three places (a town's `Shipments.held`, a wagon's
  `shipments`, a hub's `held`), and the hub's are copied into a fourth (`Aboard`). The registry is the one place a
  pallet is; `by_place` is an index kept by `Pallets::move_to`, the only way a pallet changes place. Holders keep
  nothing but what's theirs to say about it: a hub's `Custody { carted, booked }` by pallet id, a wagon's `Manifest`.
  `check.rs` counts goods on pallets once. This follows the design rule that save data is the truth for anything that
  must work unloaded: a pallet in a wagon is a record; a crate in a chest, a view of it.
- **Not entities.** Pallets come and go by the thousand; entities would need their links rebuilt on load (as routes'
  `from`/`to` are) and Bevy's iteration order kept in mind. A resource with `BTreeMap`s is simpler and plainly
  deterministic.
- **Received pallets leave the registry.** Opened by the consignee, a pallet's goods go into stock, its returnable
  packages into the empties, and it's gone. The registry holds only what's still packed.
- **`Shipments` stays as the town's empties** (by package kind) and nothing else. `Paperwork` is unchanged and gains
  the document kind `"superbill"`.

### One contract to a bill

A bill names at most one contract. A pallet's grouping key is (shipper, consignee, contract, payment, handling, pallet
kind): packages that differ in any of these go on separate pallets. Why: receiving delivers a pallet under its
contract, a bank reads a bill of exchange's goods by contract, and stranded paper goes with its goods by contract; all
three stay as simple as today. Two contracts to one buyer still travel together, on two pallets under one superbill.
A bill of several lines is for negotiation's bundles of terms (step 5), when a deal may be one bill for several uses.

### Weight and volume

- **Volume is slots**, as today: a package takes the stacks of what's in it, at least one (`Crates.stacks`); a pallet
  the sum of its packages'. Slots stay whole numbers: they're Minecraft's slots and Factorio's stacks.
- **Weight** is new: `Good.weight`, per item, defaulting to `1 / stack` (a stack weighs 1), plus each package kind's
  `tare` (0 for now). With that default, weight equals slots and never binds first: nothing changes until a content
  pack says a good is heavy. A pallet's weight is the sum of its packages'.
- **A pallet kind** (`PalletKind { name, slots, weight }`) caps a pallet: the base content's one kind, the **load**, is
  at most `pallet_slots` 9 and `pallet_weight` 9.0. Nine stacks is a crate, a porter's pack and a Leatherworker's:
  what one worker moves at a time. A shipment bigger than that is several pallets, a bill each.
- **A carrier's caps** (`Caps { slots, weight, pallets }`, from `squad::capacity` extended): a wagon `wagon_slots` 27,
  `wagon_weight` 27.0, `wagon_pallets` 27; each pack (Leatherworker, porter in a squad) adds `pack_stacks` 9,
  `pack_weight` 9.0, one pallet; a porter run is one pack. A drayman's cart stays 4 slots (`CART`): a pallet of 9 is
  three carts, as a 9-stack crate is today. `wagon_pallets` doesn't bind (a pallet takes at least a slot); it's there
  for carriers that will (a ship of 12 containers).
- **Each carrier takes its own kinds:** a `PalletKind` lists the carriers that take it. The base content has one kind
  every carrier takes. A rail binding or a later era adds its own (a container a ship takes and a porter doesn't);
  nothing in the rules names a kind.

### Superbills

- A **container** is a wagon on a trip, or a hub's consolidation: the pallets its forwarder loads on one run for one
  next stop. Its **manifest** lists each pallet's id, its bill, where it drops, and (on freight) its booking.
- Its superbill is drawn as a document (kind `"superbill"`, stationery or a scribe, as any) when it holds two pallets
  or more. One pallet's bill is its own manifest, so a wagon with one crate of paper draws one document, as today.
- **The carrier works by the manifest.** It loads, drops and hands over whole lines; a consolidation is one line on
  the wagon's manifest and comes off whole at its stop. What's aboard always matches the manifest's open lines: the
  sweep checks it, and it's the carrier's report that insurance claims will need (step 6).
- A hub's consolidation: at `load_booked`, the pallets booked on one run with the same next stop become a container
  with the hub as issuer. At that stop it's handed out whole and dissolves; its pallets go on, still sealed.

### Sealed in transit

- A pallet is **sealed** as it leaves a holder's premises onto a carrier (a wagon leaving, a buyer's wagon collecting
  Ex Works, a wagon fetching from the far end). The seal is a digest of its bill and packages (FNV-1a over their
  `postcard` bytes: no new dependency).
- While sealed, a pallet is moved, booked, carted, consolidated and handed over, whole. Its packages and bill don't
  change: never broken up, topped up or re-billed on the way. A wagon with several stops drops whole pallets.
- **Sealing ends at delivery.** The consignee opens what's made out to it, at its premises, and owns what it gets.
- **Who may open a pallet,** and only at their own premises: its consignee; or its shipper, when the bill is void (the
  contract no longer in force and no bill of exchange still to collect on it: today's stale sweep in `earmark`). A hub
  holding pallets for others is in custody: it never opens them. A hub opens only pallets made out to itself.
- **Before it's sealed,** at the shipper's premises, a pallet is open: the shipper may top it up with more of the same
  key (a farm's earmark adding the next lot) up to its kind's caps.

### Breaking bulk and re-palleting

- Only at a holder's premises: the consignee's warehouse, a granary or root cellar (the farm's store), a hub's
  storehouse for its own goods. Never in transit.
- Commodities are fungible. A pallet opened goes into stock as plain goods; which wheat went in doesn't matter. New
  pallets are made from stock by destination, each with a new bill. The core already works this way: `receive` puts a
  crate into stock, `earmark` and `ship` pack from stock. Nothing new is needed but the rule.
- An Ex Works collection smaller than the earmarked pallet takes whole pallets first; the last is opened at the
  seller's (its premises), what's due packed as a new pallet for the buyer, the rest re-palleted for the contract with
  a new bill. Today's `take_earmarked` takes part of a crate, which is the same thing without the paper.

### Open lots: the other kind of cargo

A wagon carries sealed pallets (promised to a consignee) and **open lots** (the Merchant's own goods). Today's loose
sale cargo (`Wagon.cargo`) and its crated lots for sale (`Shipment { consignee: None }`, `Wagon.lots`) are open lots:
no bill, not sealed, sold piecemeal. They stay as they are now. Every package is on exactly one pallet or in exactly
one open lot. The to-order paper a lot will travel under, and the delivery orders that split it, are step 3 (below).

### Handling

`Handling` is a small set of flags from the goods (`Good.handling`, plain by default): it's in the pallet's grouping
key, so goods that need care never share a pallet with goods that don't. Nothing reads it yet; value over time
(perishables, keep cool) and insurance will.

## 2. How today's code maps onto it

| Today | After | What changes |
|---|---|---|
| `cargo::Shipment` (a crate's goods and its bill) | `Pallet` (bill and packages) | 1:1 at first (task `pallets-for-crates`): a shipment is a pallet of its crates. `settled: bool` becomes `Payment` |
| `Shipments.held` | `Pallets` at `At::Town(t)` | The town's crated goods; `Shipments` keeps only empties |
| `Wagon.shipments` | `Pallets` at `At::Wagon(route)` and the wagon's `Manifest` | Lots for sale move to `Wagon.lots` as packages |
| `Hub.held: Vec<Held>` | `Pallets` at `At::Custody(hub)`, `Hub.custody: BTreeMap<PalletId, Custody>` | `carted`, `booked` by id |
| `Aboard.booked`, `Aboard.forwarding` (copies) | Manifest lines with `booking` and `for_forwarder` | No copies, no matching by `==` |
| `Plan.shipments` (clones) | `Plan.pallets: Vec<PalletId>`, claimed | A pallet counted in one plan only |
| `cargo::stacks(loose, shipments)` | `Load::of(cargo, pallets)`: slots, weight, pallets | Compared with the carrier's `Caps` |
| `Bill.contract` | `Bill.contract` and `Bill.pallets: Vec<PalletId>` | The goods a bill of exchange pays for are named, not guessed |

What each slice reads and writes afterwards:

- **Plan and load sizing** (`rules/plan.rs`: `plan_sale`, `full_stacks`, `clip`, `decide`; the `pickCargo` and
  `saleLoad` ports). The plan counts its pallets' slots and weight with its loose sale load and fits them to the
  squad's `Caps`, not to slots alone. The sale load is still chosen good by good in slots; `clip` drops the last of
  it until slots, weight and pallets all fit. `full` is judged on slots as now (and on weight once a good is heavy:
  a wagon is full when either cap is reached). `plan_crated` claims home's pallets for each contract, in route order.
- **Leaving** (`rules/cargo.rs` `ship`, `rules/freight.rs` `depart`, `pick_forwarded`). Claimed pallets move to the
  wagon and are sealed. Owed goods are packed into pallets by the grouping key, split at the pallet kind's caps, a
  bill each. Goods for sale go crated into open lots as today. The manifest is written; a superbill drawn if two or
  more pallets.
- **Arriving and unloading** (`fetch`, `open`, `unlot`/`relot`, `unload`, `deliver_freight`, `deliver_home`). The far
  end opens pallets consigned to it (copy filed, goods to the wagon's cargo for `contracts::settle` or to stock, empties
  kept), fetches whole pallets consigned to the wagon's home within the wagon's caps, and the wagon drops its manifest
  lines for that stop. Home receives what's consigned home; a pallet consigned elsewhere goes back to `At::Town(home)`,
  still sealed.
- **Hubs' booking and forwarding** (`freight.rs` `hand_over`, `forward`, `buy_blocks`, `run_block`, `load_booked`).
  Same rules and prices, by pallet id: a pallet's slots are what a crate's stacks were. `load_booked` makes a
  consolidation for each next stop with two pallets or more. A pallet booked on a run that's gone is booked again, as
  now.
- **The post** (`post.rs`, `rules/post.rs`). Letters, copies and bills by post are untouched: a bill of lading rides
  on its pallet and never goes by post (sending title ahead of the goods is step 3's). A town's booked goods
  go to the hub as pallets made out to it (`consignment-pallets`: paid, no contract, crated at the town's cost or loose
  where it can't pay, sealed aboard), so a collection run carries a superbill. The hub opens them into its stock,
  and the forwarder's lots are sold from there.
- **The bank discounting a bill** (`bank.rs` `Discounts::backed`, `quote`). A bill with pallets is backed while they
  exist or were received; one whose pallets were lost isn't. A bill without pallets reads its contract, as now.
- **Stranded paper** (`stranded.rs`, `rules/stranded.rs`). The goods that go with a bill to the hub are the bill's
  pallets, by id, not "crates for the same contract and drawee". The hub takes them into custody, sealed.
- **Earmarks** (`rules/cargo.rs` `earmark`, `cargo::set_aside`). The farm's earmark is an open pallet at its premises,
  topped up lot by lot until its caps, then a new one. `set_aside` subtracts what's on its pallets. The stale sweep is
  the shipper opening a void pallet.
- **Ex works** (`rules/contracts.rs` `settle`'s collection, `take_earmarked`, `pack_collected`). The buyer's wagon takes
  the seller's earmarked pallets whole, then packs the rest paid for (`Payment::Paid`), sealed as it loads; a part
  pallet is re-palleted at the seller's, as above.
- **Training's offtake** (`training.rs`): the pupil's work packed as a pallet its bill names (`Bill.pallets`), but
  `Payment::Paid`, not `ByBill`: as `ByBill` the stale-crate sweep and stranded paper would treat it as unpaid when
  crated and outcomes would move (`docs/decisions.md`, pallets-for-crates).
- **Everything that counts what's aboard** (`rules/atlas.rs`, `rules/desires.rs`, `rules/squad.rs`, `rules/kit.rs`,
  `rules/workshop.rs`): asks `Pallets` instead of iterating `shipments`.
- **Stays:** prices, fees, fares and drayage (by slots); the paperwork's stationery and scribes; contracts' delivery
  and owing; `crate_room` (counted in packages: crates are still the room a warehouse has); lots for sale; the
  consignee's empties and the returnable crates; `deliver_shipment`'s behaviour (renamed `deliver_pallet`).

## 3. Milk runs and itineraries, as far as pallets need them

- **A leg's itinerary** is its ordered calls: today at most a hub stop and the far end out, a hub stop and home back
  (`Stop`, `Route.stops`). Task `itinerary-drops` writes it as `Itinerary { calls: Vec<Call { town, ticks } > }` with
  no change in what happens.
- **Each pallet's drop** is set as it loads: its consignee if a call on the itinerary is the consignee, else the first
  hub on the itinerary that serves it. A wagon loads a pallet only if it has a drop.
- **At each call** the wagon drops the lines for that call (delivered, or handed to the forwarder), then picks up what's
  booked or made out for a later call. One function does both for every call, the far end included.
- **The fit is per stretch:** the load between two calls must fit the caps. Drops free room for later pickups. With
  today's three calls this is the check that already happens; it's written for any number.
- **The run never looks inside a pallet:** only its drop, slots, weight and kind.
- **Built** (`itinerary-drops`): `freight::Itinerary { calls: Vec<Call { town, ticks }>, next }` on `Wagon.itinerary`
  replaces `Aboard.stop`: a wagon sets out with its leg's end (`plan.rs`), `depart` and `turn_stop` put the call on the
  way in (`call_on_way`, today's `setStop` arithmetic), `call_at_stop` makes it (`made`). Drops are chosen by
  `Itinerary::drop_for` over the calls ahead and, going out, the way back: the consignee where a leg ends there (a call
  on the way receives nothing, as `callAtStop` never has), else the first hub ahead that forwards it. Calls drop through
  `cargo::drop_off` (freight and the far end's goods by line, the forwarder's hand-over by line, home everything left)
  and pick up through `cargo::pick_up`/`room_to`, which fit each stretch (`stretch_load`, `stretch_peak`: with nothing
  spoken for on a later stretch, the first binds, so the numbers are today's). Two rules keep today's outcomes where the
  network moves under a wagon: a hub that has come to forward a line since it loaded takes it at its call, and a line a
  call couldn't take drops at the next that will, else home (`drop_again`, at the hand-over, `close_call` and the turn).
  The stranded sale buys by its terms, not by drop (it's the paper's sale, title to the hub). The steps of a call stay
  in their places in the look (freight in `Mail`, fetching in `Papers`, opening in `Settle`, the forwarder in
  `Freight`): one function for dropping and one for picking up, used by each.

The rest is sketched only, so this design leaves room for it. John's order after this work:

1. **Pallets are bills** (this file).
2. **Value over time.** A package carries its batch (`Package.batch`: made, and a few exposure numbers such as
   heat-days), reckoned on touch from each good's curve. A pallet's `since` and `at` give its exposure: on a wagon in
   summer, in a cave. Stock is fungible, so a store keeps a weighted age per good; opening a pallet merges its batch in.
3. **Lots and delivery orders.** An open lot gets a to-order paper (a lading whose consignee is "to order of" its
   holder: `Consignee::ToOrder(TownId)`, a variant added then). Selling part of a lot issues a delivery order; at the
   buyer's premises the part becomes a pallet with the buyer's bill. Endorsing a bill passes title (stranded paper's
   endorsement already does this for bills of exchange). A deposit in a cave is a warehouse receipt on pallets
   `At::Town(depot)`.
4. **Itineraries and multi-stop routes with price news.** A route with several calls chosen from its pallets' pickups
   and drops (milk runs: any size, any source and destination, dropped all at once or over many stops), and depots
   for one's own goods (deposit on the way out, pick up on the backhaul).
5. **Negotiation**, then evolved strategies; bills with several lines as bundles of terms.
6. **Contingent claims:** insurance on pallets and lots (a policy is paper attached to the bill), "pays X if E before
   T" in general. The manifest is the carrier's report a claim needs.
7. **Crown tools.**

## 4. The Minecraft binding

The binding doesn't run the core yet (architecture step 3). When it does:

- **A pallet is invisible.** No block, no entity. It's the grouping of the packages one bill covers: two barrels, six
  crates, four bundles.
- **Each package item carries its pallet** (a data component: pallet id, n of m). Its tooltip says where it's bound
  and under what: "1 of 3, to Wheatfell, contract 7". The bill of lading is a paper in the documents slot of the
  pallet's first package; the others carry only their tag.
- **Pallets move whole.** Putting a pallet into chests reserves room for all its packages first; if they don't all fit,
  none moves. This is the "can this go in here? someone grabbed it" race gone: it's the reason for the work.
- **Wagons are records** in the mod already: a pallet aboard is data, nothing to show but the Caravans tab's manifest.
- **What the binding shows:** the Merchant's desk lists pallets held (to whom, contract, packages, slots, weight,
  sealed since), what's made out to it, and what's waiting to be fetched; the Caravans tab shows each wagon's manifest;
  `/enterprise pallets <desk>` prints them. Weight comes from the content pack (Minecraft has none of its own).
- **What it maps, both ways** (task `pallet-messages`): outputs `PalletPacked { id, at, packages }`, `PalletMoved { id,
  to }`, `PalletOpened { id, at }`, so crates appear, move and empty in the world; inputs `PalletBrought { town, id }`
  (a player carried a whole pallet to a desk: today's `deliver_shipment`), `PackageBrought { town, pallet, package }`
  (part of one) and `PackageLost { pallet, package }` (raided, broken, thrown in lava).
- **Built** (`pallet-messages`, `pallet_messages.rs`): the outputs are queued by `Pallets::pack`, `move_to`, `open` and
  `unpack` once a host has applied a happening (`Society::apply` turns `Pallets.report` on; they are not state, and
  `advance` called directly returns none, so the fingerprint and replay hold). `PalletOpened` also tells of a pallet
  taken apart by its holder. `PackageBrought` marks a package of a pallet made out to the town (`Pallets.arriving`):
  complete, it is received at once with its seal; short, at the town's next look (`rules/pallet_messages.rs`). A pallet
  names a package by its place on it, and the places after a lost package move up one. Inputs naming a pallet that isn't
  in a town's warehouse (moved on, on a wagon, in custody, gone) are a stale view: ignored, not refused.
- **A short pallet** (a package lost, or brought apart): its seal is broken. The consignee takes what came; the rest is
  a loss on the bill (in the books as lost, the carrier's report shows it). A player is a carrier: one who brings every
  package keeps the seal.
- **The mod's crates stay as they are** until the binding replaces its cargo code (see "For John").

## 5. Behaviour and tests

### What changes in outcomes

| Task | Fingerprint | What changes |
|---|---|---|
| `pallet-types`, `pallets-for-crates`, `pallet-sweep`, `itinerary-drops`, `pallet-messages` | Unchanged (`8144f9be7947133b` or main's at the time) | Nothing: re-representation. Each shows `just core-fingerprint` equal to main's |
| `pallet-claims` | Changed (`cb418063bb32364e`) | A town's crates are counted in one route's plan, not in every route's plan that look; a plan that would have counted a crate another route took no longer goes for it. Fixes a race; rare, as expected: in the 300 worlds without the claims slice, trips 139846 to 139843 and broken contracts 12708 to 12719; study 365's books unchanged |
| `pallet-caps` | Changes | A shipment over 9 slots goes as several pallets, a bill each: more documents, more paper and ink, a counting house sooner. Base content weight equals slots, so loads don't shrink. A content pack's heavy goods fill a wagon by weight |
| `packing` | Changes | Owed goods that can't be crated ride on their pallet as loose stacks under its bill, out of the sale's reach; a farm's earmark tops up its open pallet (fewer bills) |
| `superbills` | Changes | A document for each container of two pallets or more; consolidations load and drop whole |
| `consignment-pallets` | Changes | A town's goods booked with the forwarder go crated and billed to the hub: crates and paper cost it a little |

Each changing task measures what changed: `just study 365` (trips, documents drawn, crates bought, scribes' fees),
`cargo run --release -p enterprise-sim -- --sweep 300`, and ms a day on the study world. The budget: pallets may cost
at most a tenth more a day than main's (1.26 ms).

### Parity tags

The crates-and-papers tags were ported faithfully. Each stays honest (same numbers, renamed types) or is listed as a
deliberate divergence for `docs/decisions.md`.

| Tag | Touched by | Stays honest because / diverges how |
|---|---|---|
| `CargoTests.aCrateTakesTheSpaceOfWhatsInIt` (partial) | `pallets-for-crates`, `pallet-caps` | A package takes the slots of what's in it, at least one: the same rule. Stays partial (one good a package) |
| `CargoTests.documentsUseStationeryOrAScribe` | none | Papers drawn directly |
| `CaravanTests.contractedGoodsShipInCratesWithTheirPapers` | all | 64 paper is one pallet, one bill, no superbill (one pallet): documents 1, a crate 0.125 |
| `CaravanTests.whatAPlayerBroughtIsPaidForByTheNextCaravan` | `pallets-for-crates`, `pallet-messages` | `deliver_pallet` (then `PalletBrought`) with `Payment::Owed`: delivered 16, owing 4 |
| `CaravanTests.aCaravanFetchesHomeCratesMadeOutToIt` | `pallets-for-crates`, `itinerary-drops` | A whole pallet fetched within the caps |
| `CaravanTests.goodsForSaleTravelCratedAndAreStoredAsTheyCome` (partial) | none in effect | Lots for sale are open lots, as today |
| `MarketTests.aFarmCratesOwedCropInLotsNotAsItTrickles` | `packing` | 40 at once, one bill: topping up is only of an open pallet with the next lot |
| `MarketTests.aFarmWithNoRoomForCratesDrawsNoPapers` (partial) | `pallets-for-crates` | `crate_room` counts packages, as it counted crates |
| `MarketTests.aFarmCratesWhatItOwesOutOfReachUntilItShips` | `packing` | Set aside drops to 0 once on a pallet; a void pallet opened back to stock |
| `CaravanTests.aForwarderTakesCratesBoundOnAndTheirRunLoadsThemFirst` | `pallets-for-crates`, `superbills` | One pallet: no consolidation; booked pallets load first |
| `CaravanTests.theForwarderBuysABlockOfSpaceItFillsAndPaysForEachRun` | `pallets-for-crates` | Blocks in slots; a pallet's slots are the crate's stacks |
| `ContractTests.aHubBuysATownsStrandedPaperForCoinAndItsOwnBill` | `pallets-for-crates` | The bill's pallets go with it: the same crates, by id |
| `LockerTradingTests.underAnOfftakeThePupilsWorkIsCratedForTheBuyerWithABill` | `pallets-for-crates` | One pallet its bill names, `Payment::Paid` (not `ByBill`: see section 2) |
| `CountingHouseTests.theDiscountDeskPricesByGoodsAndCapsItsBook` | `pallets-for-crates` | Bills in the test have no pallets: backed reads the contract, as now |
| `CountingHouseTests.aCountingHousesClerksDrawUpPapersCheaply` (partial) | none | Papers drawn directly |
| `PostTests.*` (6) | `consignment-pallets` | Letters and bills untouched. The two consignment tests (`aTownsBookingsFillingAWagonBringTheForwarderToCollect`, `goodsOnConsignmentSellAndTheMoneyComesBackByPost`) keep their numbers for what sells and is remitted; the crates and paper the town now pays are a divergence |

**Deliberate divergences** for the integrator's write-up in `docs/decisions.md` ("Pallets: the bill on the handling
unit", built on the landing date), one line each as the tasks land:

1. A bill of lading covers a pallet of packages, not a crate; a shipment over a pallet's caps is several bills
   (`pallet-caps`): landed 2026-10-19, with a porter run of one pallet, stranded loads and a hub's buying at a
   town by room (a bill only with all its pallets), shopping capped by room, owed and for-sale goods counted apart
   (`docs/decisions.md`).
2. A plan claims the pallets it counts; another route's plan that look doesn't count them (`pallet-claims`). Routes
   stuck last time (no money, Merchant, squad or range) plan last, and a stay lets the claims go.
3. Contracted goods that can't be crated travel loose on their pallet, under its bill (`packing`). The mod's go loose
   without papers in the wagon's cargo.
4. A farm's earmark tops up its open pallet (`packing`); the mod crates each lot apart.
5. A container of two pallets or more draws a superbill (`superbills`).
6. A town's goods on consignment go to the hub on pallets made out to it (`consignment-pallets`); the mod's forwarder
   collects them as a lot.
7. A bill of exchange names the pallets it pays for; a discount desk counts a bill whose pallets were lost as unbacked
   (`pallets-for-crates`, `pallet-messages`).

### Invariants for the random-worlds sweep

A new slice file `tests/common/slices/pallets.rs`, checked every day across 300 seeds:

- **Every package on exactly one pallet** (or in one open lot): no pallet empty, every package's count positive and
  finite, one good each.
- **Every pallet on one bill:** its consignee a town, its contract (if any) in the book, a `Payment::ByBill` bill
  existing and naming this pallet; no two bills of exchange naming the same pallet.
- **Every pallet in one place:** `by_place` matches each pallet's `at`, and nothing else; a pallet `At::Wagon(r)` only
  while route `r` has a wagon; `At::Custody(h)` only at a hub, with a custody record; no pallet at its own consignee
  unopened.
- **Sealed pallets unchanged between premises:** a sealed pallet's digest equals its seal's; a pallet on a carrier or
  in custody is sealed.
- **Within the caps:** every pallet within its kind's slots and weight; every wagon's load (pallets and loose lots)
  within its carrier's slots, weight and pallets.
- **The manifest is the load:** a wagon's open manifest lines are exactly the pallets and containers aboard; a
  consolidation's pallets share one next stop.
- **Goods conserved:** `check.rs` counts pallets once, from the registry.

Host doings for that slice (`daily`): a player brings a whole pallet (owed, paid, made out elsewhere), part of one, or
loses a package from a wagon; a content pack's heavy good in some worlds (weight 4 a stack) so the weight cap binds.

## 6. Task plan

Each task is one agent in one tree, an hour or two, briefed as in `docs/core-plan.md` section 3 (the common brief,
with "port the Java" read as "keep the ported rules"; there's no new Java here). The decisions above bind every task.
Every task: tests in `sim/core/tests/pallets.rs` (or its own file named below), `cd sim && cargo test --release`
green, the parity tags above still passing, its divergences (if any) listed in its `.tree/TASK.md` summary for the
integrator's `docs/decisions.md` entry.

Order: `pallet-types` → `pallets-for-crates` → (`pallet-sweep` ∥ `pallet-claims` ∥ `pallet-messages`) →
`pallet-caps` → `packing` → `superbills` → (`itinerary-drops` ∥ `consignment-pallets`). Tasks that edit
`rules/plan.rs`, `rules/cargo.rs` or `rules/freight.rs` land one after another.

| Order | Task | Model / effort | Waits for | Fingerprint |
|---|---|---|---|---|
| 1 | `pallet-types` | sonnet / medium | — | unchanged |
| 2 | `pallets-for-crates` | opus / xhigh | `pallet-types` | unchanged |
| 3 | `pallet-sweep` | sonnet / high | `pallets-for-crates` | unchanged |
| 3 | `pallet-claims` | sonnet / medium | `pallets-for-crates` | may change |
| 3 | `pallet-messages` | sonnet / medium | `pallets-for-crates` | unchanged |
| 4 | `pallet-caps` | opus / high | `pallet-claims` | changes |
| 5 | `packing` | sonnet / high | `pallet-caps` | changes |
| 6 | `superbills` | opus / high | `packing`, `pallet-sweep` | changes |
| 7 | `itinerary-drops` | opus / high | `superbills` | unchanged |
| 7 | `consignment-pallets` | sonnet / medium | `superbills` | changes |

### 1 `pallet-types`

| | |
|---|---|
| Goal | The types and the registry, unused by the rules: `Package`, `PackageKind`, `Lading`, `Payment`, `Handling`, `Seal`, `At`, `Pallet`, `PalletKind`, `Caps`, `Load`, `Container`, `Manifest`, and `Pallets` with its one mover |
| Files | adds `sim/core/src/pallet.rs`; `content.rs` (`Good.weight` default `1/stack`, `Good.package`, `Good.handling`, `Content.packages` with the crate, barrel, bundle and `LOOSE`, `Content.pallet_kinds` with the load); `dials.rs` (`pallet_slots 9`, `pallet_weight 9.0`, `wagon_weight 27.0`, `wagon_pallets 27`, `pack_weight 9.0`); `slices.rs` (`Pallets` in `saved!`); `lib.rs` |
| Tests | `tests/pallets.rs`: `move_to` keeps `by_place` in step and in arrival order; the digest changes with any package or the bill and not with `at` or `since`; a package's slots are its stacks at least one (`aCrateTakesTheSpaceOfWhatsInIt`'s numbers); a load's slots, weight and pallets; caps for a wagon, a squad with two packs, a porter run; save and load round trip with pallets in it |
| Model | **sonnet / medium**: well-specified types, nothing reads them yet |

Brief:
```
# pallet-types
Read docs/pallets.md (all of section 1). Add sim/core/src/pallet.rs with the types of section 1's "Data in the core".
Pallets::pack(lading, packages, at, now) -> PalletId (next id, indexed); move_to(id, at, now) the only mover (index
kept in arrival order, `since` set); open(id) -> Pallet (removed, `received` counted); seal(id, by, now); digest
(FNV-1a 64 over postcard bytes of lading and packages); at(place) -> &[PalletId]. Load::of(content, cargo, pallets)
gives slots (each package its stacks, at least one; loose cargo ceil per good), weight (count x Good.weight + tare) and
pallets; Caps::of(content, crew, porter_run) extends squad::capacity with weight and pallets. Content: Good.weight
(default 1/stack), Good.package (crate), Good.handling (plain), the package kinds and the one pallet kind; dials as
listed. Nothing in rules/ reads them: the fingerprint must equal main's. Tests in tests/pallets.rs as listed.
```

### 2 `pallets-for-crates`

| | |
|---|---|
| Goal | Every use of `Shipment` moved to `Pallets`, one shipment one pallet, no change in behaviour: towns', wagons' and hubs' crates in the registry; `Payment` for `settled` and the billed lading; `Aboard.booked` and `forwarding` replaced by manifest lines; `Bill.pallets`; seals set as pallets leave premises; `check.rs` counting the registry |
| Files | `cargo.rs`, `rules/cargo.rs`, `freight.rs`, `rules/freight.rs`, `stranded.rs`, `rules/stranded.rs`, `training.rs`, `rules/training.rs`, `rules/contracts.rs`, `rules/plan.rs`, `rules/desires.rs`, `rules/atlas.rs`, `rules/squad.rs`, `rules/kit.rs`, `rules/workshop.rs`, `rules/post.rs`, `bills.rs`, `bank.rs`, `harvest.rs`, `check.rs`, `state.rs` (`Wagon.manifest`, `Plan.pallets`), `society.rs`; tests that build a `Shipment` (`tests/cargo.rs`, `freight.rs`, `stranded.rs`, `training.rs`, `common/slices/crates_and_papers.rs`) |
| Tests | every existing test green with the same numbers; new: two alike crates for one consignee booked at a hub go to the right runs (the `==` match gone); a stranded bill's goods are exactly its pallets; `just core-fingerprint` equal to main's (the acceptance) |
| Model | **opus / xhigh**: cross-cutting, every freight slice at once, and it must change nothing; the copies in `Aboard` and the plan's clones need judgment to replace without moving an outcome |

Brief:
```
# pallets-for-crates
Read docs/pallets.md sections 1 and 2, then cargo.rs, rules/cargo.rs, freight.rs, rules/freight.rs, stranded.rs,
rules/stranded.rs and every other file grep finds using Shipment or shipments. Replace Shipment with Pallet one for
one: a shipment's crates are its packages (a crate to nine stacks, the last part-full), its bill is the lading,
settled false/true/the billed branch of freight::receive become Payment::Owed/Paid/ByBill(bill). Towns' held crates are
At::Town, wagons' At::Wagon, hubs' At::Custody with Hub.custody by id; Aboard.booked and Aboard.forwarding become
Wagon.manifest lines (booking, for_forwarder); Plan.shipments becomes Plan.pallets (ids). Bills of exchange drawn for
crated goods name their pallets (Bill.pallets); stranded paper's goods go by those ids; Discounts::backed reads them
when a bill has any. Seal a pallet as it leaves premises (ship, Ex Works collection, fetch); open only where section 1
allows. check.rs counts goods on pallets from the registry. Society::deliver_shipment becomes deliver_pallet (keep the
old name as a thin wrapper for the tests that use it, or update them). Keep every order of iteration as it is:
pallets at a place in arrival order where a Vec was. Acceptance: all tests green with their numbers, and
`just core-fingerprint` equal to main's. If a number must move, stop and say why in the summary.
```

### 3 `pallet-sweep`

| | |
|---|---|
| Goal | The random worlds check pallets every day: section 5's invariants, and host doings that bring, split and lose pallets |
| Files | adds `tests/common/slices/pallets.rs`; `tests/common/slices.rs` (its name); `pallet.rs` (read-only views the checks need, if any) |
| Tests | the 300 seeds green; a test that a deliberately broken registry (a pallet indexed twice, a sealed pallet changed) fails each check with a clear message |
| Model | **sonnet / high**: well-specified checks, but they must hold across every slice's doings |

Brief:
```
# pallet-sweep
Read docs/pallets.md section 5 ("Invariants") and pallet.rs as landed. Add tests/common/slices/pallets.rs: check()
with every invariant listed (caps and manifest checks for what exists now; leave a line for what later tasks add);
daily() bringing a whole pallet (each Payment), a pallet made out elsewhere, and losing nothing yet (PackageLost is
pallet-messages'). Don't change rules; if a check fails on main's behaviour, report the seed and the finding.
```

### 3 `pallet-claims`

| | |
|---|---|
| Goal | A plan claims the pallets it counts; a later route's plan that look skips claimed ones; claims cleared at the look's start; `ship` takes claimed pallets by id (no "it's not aboard" case left) |
| Files | `rules/cargo.rs` (`plan_crated`, `ship`), `rules/plan.rs` (`start` clears claims), `pallet.rs` (`claim`, `release`) |
| Tests | two routes from one town to one buyer, one pallet: only the first route's plan counts it, and only its wagon presses for it; a wagon that stays releases its claim for the next look |
| Model | **sonnet / medium**: a small rule with a clear test; the fingerprint may move and the summary says by how much |

Brief:
```
# pallet-claims
Read docs/pallets.md (sections 2 and 5, divergence 2). In plan_crated, claim each pallet a plan counts (route order:
the first route wins), skipping pallets claimed this look. Claims clear in plan::start. ship takes the plan's
claimed pallets by id. Measure: core-fingerprint before and after, just study 365 trips; say what moved and why.
```

### 3 `pallet-messages`

| | |
|---|---|
| Goal | The binding's protocol for pallets: outputs `PalletPacked`, `PalletMoved`, `PalletOpened`; inputs `PalletBrought`, `PackageBrought`, `PackageLost`; a short pallet's broken seal and loss |
| Files | `messages.rs` (new variants), `pallet.rs` (events queued by the mover), `society.rs` (handling the inputs), `ledger` (lost goods as used, reason "lost") |
| Tests | a replay of inputs gives the same outputs; `PalletBrought` reproduces `whatAPlayerBroughtIsPaidForByTheNextCaravan`; a pallet missing a package arrives unsealed, the consignee takes what came, the loss in the books and conserved; a bill of exchange on a lost pallet is unbacked |
| Model | **sonnet / medium**: messages and a small rule; must keep the fingerprint (outputs aren't state) |

Brief:
```
# pallet-messages
Read docs/pallets.md section 4. Add the three outputs (queued by Pallets::pack, move_to and open) and three inputs.
PackageBrought adds to a pallet made out to the town as it arrives in parts (unsealed, received when complete or at
the town's next look). PackageLost removes a package (Ledger used, reason lost), breaks the seal, and the pallet goes
on short. Fingerprint unchanged: the base worlds send none of the new inputs. Add PackageLost to the pallets slice's
daily doings.
```

### 4 `pallet-caps`

| | |
|---|---|
| Goal | Pallets within their kind's caps and loads within their carrier's: packing splits at `pallet_slots` and `pallet_weight`; plan, `clip`, `full`, `fetch`, porters, `hand_over` and `load_booked` fit slots, weight and pallets |
| Files | `cargo.rs` (`pack` splits into pallets), `rules/cargo.rs`, `rules/plan.rs` (`plan_sale`, `full_stacks`, `clip`, `decide`), `squad.rs` (`porters_needed` by the larger of slots and weight), `freight.rs` (`hand_over`, `load_booked`), `rules/contracts.rs` |
| Tests | 20 stacks owed: three pallets, three bills, all delivered; a heavy good (weight 4 a stack): a wagon takes 6 stacks of it, and porters are sized by weight; a 9-slot porter run takes one pallet; the pallets slice adds the caps checks and some worlds' heavy good |
| Model | **opus / high**: changes what fits on every carrier through the plan's sizing (the `pickCargo`/`saleLoad` ports); outcomes move and must be explained |

Brief:
```
# pallet-caps
Read docs/pallets.md sections 1 (weight and volume) and 2 (plan and load sizing). Split packing at the pallet kind's
caps, a bill each. Fit every load to Caps (slots, weight, pallets): the plan's sale load clipped last-in first-out
until all three fit; full when slots or weight reach wagon_full of the cap; fetch, hand_over and load_booked take a
pallet only if it fits; porters_needed by the larger of slots and weight beyond the wagon. Base content (weight equals
slots) should move only the paper. Measure: just study 365 (documents, scribes' fees, trips), the sweep, ms a day.
Divergence 1.
```

### 5 `packing`

| | |
|---|---|
| Goal | Packages of several kinds on a pallet; owed goods that can't be crated ride loose on their pallet under its bill (out of `Wagon.cargo`); a farm's earmark tops up its open pallet; the grouping key with handling |
| Files | `cargo.rs` (`pack` by `Good.package`, `LOOSE` packages, the grouping key, top-up while unsealed), `rules/cargo.rs` (`ship`, `earmark`, `open`), `rules/contracts.rs` (`settle` delivers from opened pallets; the loose `contracted` path folded in), `rules/plan.rs` (owed goods counted on pallets) |
| Tests | a shipper with no crates and no money: its owed goods still go on a pallet, one bill, delivered; a barrel good and a crate good for one buyer and contract share a pallet; a farm's second lot tops up its open pallet (one bill), and once full a new one; `aFarmCratesOwedCropInLotsNotAsItTrickles` and `aFarmCratesWhatItOwesOutOfReachUntilItShips` unchanged |
| Model | **sonnet / high**: the rules are written here; the care is in folding the loose owed path into pallets without double counting |

Brief:
```
# packing
Read docs/pallets.md sections 1 (the model, one contract to a bill, handling) and 2 (earmarks, ex works). Make pack
build pallets by the grouping key from packages of each good's kind, crates first (empties, then new at
crate_emeralds), LOOSE for what can't be crated. Wagon.contracted's loose goods move onto their pallets; contracts::
settle delivers what the far end opened. earmark tops up an unsealed pallet of the same key at the farm to its caps.
Ex Works collection re-pallets a part pallet at the seller (a new bill). Divergences 3 and 4; measure as pallet-caps.
```

### 6 `superbills`

| | |
|---|---|
| Goal | Containers and their manifests: a wagon's manifest written as it leaves, a superbill drawn for two pallets or more; a hub's consolidation per run and next stop, loaded, carried and handed out whole; drops by manifest line |
| Files | `pallet.rs` (`Container`, `Manifest`), `rules/cargo.rs` (`ship`), `freight.rs` (`load_booked` consolidates), `rules/freight.rs` (`call_at_stop`, `at_hub`, `deliver_freight`, `deliver_home` drop by line), `cargo.rs` (`"superbill"` papers) |
| Tests | a wagon with one pallet draws one document, with three draws four; a hub loading three pallets for one next stop makes one consolidation that comes off whole there and dissolves; `aForwarderTakesCratesBoundOnAndTheirRunLoadsThemFirst` unchanged; the pallets slice checks the manifest against the load |
| Model | **opus / high**: the container level threads through the hubs' stops and arrivals, where the old copies were |

Brief:
```
# superbills
Read docs/pallets.md section 1 (superbills) and section 3. Write Wagon.manifest at departure (lines: pallet or
container, drop, booking); draw a "superbill" document at the issuer for a container of two pallets or more.
load_booked groups a run's booked pallets by next stop into a hub consolidation (At::Container), loaded and handed out
whole. Every drop and pickup at a call goes by manifest line. Divergence 5; measure documents and ms a day.
```

### 7 `itinerary-drops`

| | |
|---|---|
| Goal | A leg's calls as an `Itinerary`; each pallet's drop chosen as it loads; one drop-and-pickup function at every call (the far end included); the per-stretch fit. No behaviour change on today's routes |
| Files | `freight.rs` (`Stop` folded into `Itinerary`), `rules/freight.rs` (`depart`, `set_stop`, `call_at_stop`, `at_hub`, `turn_stop`), `rules/cargo.rs` (`fetch`, `open`, `unload` through the call function), `state.rs` (`Wagon.itinerary`) |
| Tests | today's freight, cargo and stranded tests unchanged; an itinerary of four calls built by hand in a test: pallets for the second and fourth calls dropped there, a pickup at the third fitting only because the second's drop freed room; `just core-fingerprint` equal to main's |
| Model | **opus / high**: restructures the stop logic that freight-hubs and stranded-paper share, and must change nothing while making room for multi-stop routes |

Brief:
```
# itinerary-drops
Read docs/pallets.md section 3. Replace Wagon.freight.stop with Wagon.itinerary (calls in order, ticks between);
set each manifest line's drop as the pallet loads (its consignee's call, else the first hub on the itinerary that
serves it; no drop, no load). One function at each call drops that call's lines, then picks up booked or made-out
pallets for later calls, fitting each stretch. Today's routes give today's calls: fingerprint equal to main's.
```

### 7 `consignment-pallets`

| | |
|---|---|
| Goal | The post's forwarder collects a town's booked goods as pallets made out to the hub on consignment (packed and billed at the town, sealed on the collection run, a superbill for two or more); the hub opens them into the consignment lots it keeps today |
| Files | `rules/post.rs` (`collect`, `consign`, `consign_host`), `post.rs` (`Lot` from an opened pallet), `cargo.rs` (a `Purpose::Consignment` on the lading, or the contract-less bill to the hub) |
| Tests | `aTownsBookingsFillingAWagonBringTheForwarderToCollect` and `goodsOnConsignmentSellAndTheMoneyComesBackByPost` keep what sells and what's remitted; the town's crates and papers counted; a town that can't pay for crates still consigns (loose packages) |
| Model | **sonnet / medium**: one slice's collection, rules written here |

Brief:
```
# consignment-pallets
Read docs/pallets.md (section 2, the post; divergence 6) and post.rs, rules/post.rs as landed. The forwarder's
collection packs the town's booked goods into pallets made out to the hub (no contract, Payment::Paid, consignment),
sealed as the hub's run loads them; at the hub they're opened into Forwarder lots as today. Measure the post tests'
numbers and the study's consignments.
```

The write-up (`docs/decisions.md`, `docs/core-progress.md`, `docs/parity.md`'s notes) is the integrator's at each
landing, from the tasks' summaries, as for the parity waves.

## For John

1. **Do Minecraft's goods get weights?** The base content's weight equals slots, so nothing changes. Should the mod's
   content pack make ore, ingots, stone and blocks heavy (a wagon of iron carries fewer stacks than a wagon of wool)?
   It's game feel, and it changes how much a mine's town can send.
2. **The Java mod's crates:** freeze them as the reference they are (bills on crates) until the binding replaces the
   mod's cargo code with the core's, or port pallets into the mod first? Freezing is cheaper; the parity tags above
   would then read against the mod's old behaviour, with the divergences listed.
