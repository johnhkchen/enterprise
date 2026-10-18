//! Pallets: the bill of lading on a handling unit (`docs/pallets.md` section 1, task `pallet-types`).
//!
//! The types and the registry, unused by the rules yet: a wagon (or a hub's consolidation) holds pallets, a pallet holds
//! packages (crates, barrels, bundles, loose stacks) of one good each, and the pallet carries one bill of lading. Every
//! pallet is a record in [`Pallets`], at one place, moved only by [`Pallets::move_to`]; `by_place` is an index in
//! arrival order, so loading is deterministic. Reference for the rules already ported: `cargo/Crates.java` `stacks`
//! (a package takes the stacks of what's in it, at least one: [`Package::slots`]), `caravan/Caravan.java` `capacity`
//! (the carrier's caps: [`Caps::of`], extending `squad::capacity` with weight and pallets).
//!
//! Weight is new: `Good.weight` per item (default a stack weighs 1) plus each package kind's tare. With the default,
//! weight equals slots, so nothing binds differently until a content pack says a good is heavy.

use std::collections::BTreeMap;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::cargo::stacks_of;
use crate::content::{Content, GoodId};
use crate::messages::{RouteId, TownId};
use crate::roster::TradeId;

pub type PalletId = u64;
pub type ContainerId = u64;
/// A package kind, by its place in [`Content::packages`].
pub type PackageKindId = usize;
/// A pallet kind, by its place in [`Content::pallet_kinds`].
pub type PalletKindId = usize;

/// No package: stacks lying on the pallet.
pub const LOOSE: PackageKindId = 0;
/// The crate: nine stacks, returnable (`Crates.GOODS`).
pub const CRATE: PackageKindId = 1;
pub const BARREL: PackageKindId = 2;
pub const BUNDLE: PackageKindId = 3;
/// The base content's one pallet kind: what one worker moves at a time.
pub const LOAD: PalletKindId = 0;

/// A packing unit (content, like goods).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PackageKind {
    pub name: String,
    /// Stacks it holds at most.
    pub slots: u32,
    /// Weight of the empty package.
    pub tare: f64,
    /// What a new one costs, in emeralds.
    pub emeralds: f64,
    /// It comes back empty to the consignee.
    pub returnable: bool,
}

impl PackageKind {
    pub fn new(name: &str, slots: u32, emeralds: f64, returnable: bool) -> Self {
        PackageKind { name: name.to_string(), slots, tare: 0.0, emeralds, returnable }
    }
}

/// A pallet kind: its caps, and the carriers that take it (by name; none listed: every carrier). A cap left `None`
/// follows the pallet dials (`pallet_slots`, `pallet_weight`), read when asked, so setting a dial moves it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PalletKind {
    pub name: String,
    pub slots: Option<u32>,
    pub weight: Option<f64>,
    pub carriers: Vec<String>,
}

impl PalletKind {
    /// Slots it holds at most.
    pub fn slots(&self, dials: &crate::dials::Dials) -> u32 {
        self.slots.unwrap_or(dials.pallet_slots)
    }

    /// Weight it holds at most.
    pub fn weight(&self, dials: &crate::dials::Dials) -> f64 {
        self.weight.unwrap_or(dials.pallet_weight)
    }
}

/// Reserved for value over time (step 2 of the pallets plan): when a lot was made.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Batch {
    pub made: u64,
}

/// A package: so many of one good in one kind of packing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Package {
    pub kind: PackageKindId,
    pub good: GoodId,
    pub count: f64,
    pub batch: Option<Batch>,
}

impl Package {
    pub fn new(kind: PackageKindId, good: GoodId, count: f64) -> Self {
        Package { kind, good, count, batch: None }
    }

    /// Stacks of cargo space it takes: what's in it, at least one if packed, and loose cargo ceil per good
    /// (`Crates.stacks`).
    pub fn slots(&self, content: &Content) -> u32 {
        let stacks = stacks_of(content, self.good, self.count);
        if self.kind == LOOSE { stacks } else { stacks.max(1) }
    }

    /// Its goods' weight and its kind's tare.
    pub fn weight(&self, content: &Content) -> f64 {
        let tare = content.packages.get(self.kind).map_or(0.0, |k| k.tare);
        self.count.max(0.0) * content.goods[self.good].weight + tare
    }
}

/// Who pays for the goods on the bill: replaces `Shipment.settled`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Payment {
    #[default]
    Owed,
    Paid,
    ByBill(usize),
}

/// The handling instructions from the goods: plain for now (nothing reads them yet).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Handling {
    pub cool: bool,
    pub fragile: bool,
}

/// The bill of lading: the receipt for the goods and the title to them.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Lading {
    pub shipper: TownId,
    pub consignee: TownId,
    pub contract: Option<usize>,
    pub payment: Payment,
    /// Carries a copy of the contract.
    pub copy: bool,
    pub handling: Handling,
    pub issued: u64,
}

/// A seal on a pallet in transit: a digest of its bill and packages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seal {
    pub at: u64,
    pub by: TownId,
    pub digest: u64,
}

/// Where a pallet is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum At {
    /// In a town's own premises.
    Town(TownId),
    /// Held in a hub's custody for others.
    Custody(TownId),
    /// On a route's wagon.
    Wagon(RouteId),
    /// In a consolidation.
    Container(ContainerId),
}

impl Default for At {
    fn default() -> Self {
        At::Town(0)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pallet {
    pub id: PalletId,
    pub kind: PalletKindId,
    pub lading: Lading,
    pub packages: Vec<Package>,
    pub at: At,
    /// When it came to `at` (dwell times).
    pub since: u64,
    pub seal: Option<Seal>,
    /// A plan's claim this look (task `pallet-claims`).
    pub claimed: Option<RouteId>,
}

/// The base content's packing units: loose, the crate (nine stacks, returnable), the barrel, the bundle.
pub fn base_packages() -> Vec<PackageKind> {
    vec![
        PackageKind::new("loose", 0, 0.0, false),
        PackageKind::new("crate", crate::cargo::CRATE_STACKS, 0.5, true),
        PackageKind::new("barrel", crate::cargo::CRATE_STACKS, 0.5, true),
        PackageKind::new("bundle", 3, 0.0, false),
    ]
}

/// The base content's one pallet kind, the load, capped by the pallet dials; every carrier takes it.
pub fn base_pallet_kinds() -> Vec<PalletKind> {
    vec![PalletKind { name: "load".to_string(), slots: None, weight: None, carriers: Vec::new() }]
}

/// FNV-1a 64.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

impl Pallet {
    /// FNV-1a over the `postcard` bytes of its bill and packages: not of where it is or since when.
    pub fn digest(&self) -> u64 {
        let mut bytes = postcard::to_allocvec(&self.lading).expect("a bill serializes");
        bytes.extend(postcard::to_allocvec(&self.packages).expect("packages serialize"));
        fnv1a(&bytes)
    }

    /// Sealed, and unchanged since.
    pub fn intact(&self) -> bool {
        self.seal.is_none_or(|s| s.digest == self.digest())
    }
}

/// One line of a manifest: a pallet, where it drops, and its booking on freight.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Line {
    pub pallet: PalletId,
    pub drop: Option<TownId>,
    pub booking: Option<usize>,
}

/// What a container carries, by line.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub lines: Vec<Line>,
}

/// A wagon on a trip, or a hub's consolidation of pallets for one run.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Container {
    pub issuer: TownId,
    pub manifest: Manifest,
}

/// Every pallet in the world, where it is, and the one way to move it.
#[derive(Clone, Debug, Default, Resource, Serialize, Deserialize)]
pub struct Pallets {
    pub next: PalletId,
    pub pallets: BTreeMap<PalletId, Pallet>,
    /// In arrival order: loading order is deterministic.
    pub by_place: BTreeMap<At, Vec<PalletId>>,
    pub containers: BTreeMap<ContainerId, Container>,
    pub next_container: ContainerId,
    /// Tallies for the books: pallets opened by their consignees, and lost.
    pub received: u64,
    pub lost: u64,
}

impl Pallets {
    /// Packs a new pallet of the base kind at `at`; its id, indexed.
    pub fn pack(&mut self, lading: Lading, packages: Vec<Package>, at: At, now: u64) -> PalletId {
        let id = self.next;
        self.next += 1;
        self.pallets.insert(id, Pallet { id, kind: LOAD, lading, packages, at, since: now, seal: None, claimed: None });
        self.by_place.entry(at).or_default().push(id);
        id
    }

    /// The only mover: takes `id` to `at` (the back of its arrival order) and notes when.
    pub fn move_to(&mut self, id: PalletId, at: At, now: u64) {
        let Some(p) = self.pallets.get_mut(&id) else { return };
        let from = p.at;
        p.at = at;
        p.since = now;
        self.unindex(from, id);
        self.by_place.entry(at).or_default().push(id);
    }

    fn unindex(&mut self, at: At, id: PalletId) {
        if let Some(list) = self.by_place.get_mut(&at) {
            list.retain(|&i| i != id);
            if list.is_empty() {
                self.by_place.remove(&at);
            }
        }
    }

    /// Opens a pallet at its consignee's: it leaves the registry and is counted received.
    pub fn open(&mut self, id: PalletId) -> Option<Pallet> {
        let p = self.pallets.remove(&id)?;
        self.unindex(p.at, id);
        self.received += 1;
        Some(p)
    }

    /// Seals a pallet as it leaves `by`'s premises, over its bill and packages as they are.
    pub fn seal(&mut self, id: PalletId, by: TownId, now: u64) {
        if let Some(p) = self.pallets.get_mut(&id) {
            p.seal = Some(Seal { at: now, by, digest: p.digest() });
        }
    }

    /// A pallet's digest.
    pub fn digest(&self, id: PalletId) -> Option<u64> {
        self.pallets.get(&id).map(Pallet::digest)
    }

    /// The pallets at a place, in arrival order.
    pub fn at(&self, place: At) -> &[PalletId] {
        self.by_place.get(&place).map_or(&[], |v| v.as_slice())
    }

    pub fn get(&self, id: PalletId) -> Option<&Pallet> {
        self.pallets.get(&id)
    }
}

/// What a carrier is asked to hold: slots, weight, pallets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Load {
    pub slots: u32,
    pub weight: f64,
    pub pallets: u32,
}

impl Load {
    /// Loose `cargo` (items by good) ceil to stacks a good; each pallet its packages' slots and weight.
    pub fn of<'a>(content: &Content, cargo: &[f64], pallets: impl IntoIterator<Item = &'a Pallet>) -> Load {
        let mut load = Load::default();
        for (g, &n) in cargo.iter().enumerate() {
            load.slots += stacks_of(content, g, n);
            load.weight += n.max(0.0) * content.goods[g].weight;
        }
        for p in pallets {
            load.pallets += 1;
            for pk in &p.packages {
                load.slots += pk.slots(content);
                load.weight += pk.weight(content);
            }
        }
        load
    }
}

/// What a carrier holds at most.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Caps {
    pub slots: u32,
    pub weight: f64,
    pub pallets: u32,
}

impl Caps {
    /// `squad::capacity` extended (its slots, the one source): a wagon its dials' slots, weight and pallets; each pack
    /// (Leatherworker, porter) adds `pack_stacks`, `pack_weight` and one pallet; a porter run is one pack.
    pub fn of(content: &Content, crew: impl IntoIterator<Item = TradeId>, porter_run: bool) -> Caps {
        let d = &content.dials;
        let crew: Vec<TradeId> = crew.into_iter().collect();
        let slots = crate::squad::capacity(content, crew.iter().copied(), porter_run);
        if porter_run {
            return Caps { slots, weight: d.pack_weight, pallets: 1 };
        }
        let packs = crew.iter().filter(|&&t| crate::squad::carries(content, t)).count() as u32;
        Caps {
            slots,
            weight: d.wagon_weight + d.pack_weight * packs as f64,
            pallets: d.wagon_pallets + packs,
        }
    }

    /// Whether `load` fits.
    pub fn holds(&self, load: &Load) -> bool {
        load.slots <= self.slots && load.weight <= self.weight + 1e-9 && load.pallets <= self.pallets
    }
}
