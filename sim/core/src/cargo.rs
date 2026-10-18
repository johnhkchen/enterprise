//! Crates and their papers, as the mod packs and opens them (`cargo/Crates.java` `pack`, `shipping`, `store`, `open`,
//! `receive`, `consignedTo`, `stacks`, `takeEmpty`, `spareEmpties`, `EMPTIES_KEPT`, `CRATE_EMERALDS`; `cargo/Papers.java`
//! `billOfLading`, `contractCopy`; `cargo/Paperwork.java` `drawn`, `settle`, `book`, `INK_PER_DOCUMENT`,
//! `QUILL_PER_DOCUMENT`, `SCRIBE_EMERALDS`, `COUNTING_STATIONERY`, `CLERK_EMERALDS`; `caravan/ForwardSales.java`
//! `earmark`, `earmarked`, `takeEarmarked`, `setAside`; `caravan/Caravans.java` `pickContracted`, `settleContracts`,
//! `collectExWorks`, `lots`, `arriveAtDestination` (crates made out to the caravan's home fetched, spare empties
//! returned), `settleOwing`; `contract/ContractBook.java` `owe`, `owing`, `paidOwing`).
//!
//! Goods crated under a bill of lading are a [`Pallet`] in the registry ([`Pallets`], `pallet.rs`): one good, so many of
//! it, in crates of nine stacks (the last part-full), no more than its kind's caps (`pallet_slots`, `pallet_weight`: a
//! shipment over them is several pallets, a bill each, since `pallet-caps`), from a shipper, made out to a consignee
//! under a contract, with a
//! bill saying who pays for it ([`Payment`]: owed, paid in coin when crated, or by a bill of exchange drawn on the buyer)
//! and whether it carries a copy of the contract. It takes the cargo space of what's in it, at least a stack a crate
//! ([`Pallet::slots`]). A town's pallets (`At::Town`) are out of its [`Stock`](crate::state::Stock)'s reach (the lockers,
//! the workshop and spot sales see only the stock); its [`Shipments`] are the empty crates it keeps to ship with. Goods
//! for sale go crated as an open [`Lot`] aboard a wagon: no bill, not sealed, sold piecemeal. Packing ([`layout`], then
//! [`execute`]; since `packing`) puts goods on pallets by the grouping key (shipper, consignee, contract, payment, handling:
//! [`PalletKey`](crate::pallet::PalletKey)), packages of each good's own kind (`Good.package`: crate, barrel, bundle, or
//! none), goods of one key sharing a pallet up to its kind's caps. A package is the town's own empty first (a returnable
//! kind), then a new one it pays `crate_emeralds` for, as far as it can spare beyond its reserve; what it can't package
//! rides loose on its pallet under its bill ([`How::OWED`]), or stays in stock where only what can be crated goes
//! ([`How::CRATED`]). A farm's earmark tops up the open pallet of its key first ([`How::TOP_UP`]). Every bill of lading is a
//! document drawn up at the shipper's
//! ([`Paperwork`]): a sheet of paper, an eighth of an ink sac and a thirty-second of a feather from its stock when it's
//! next touched, or a scribe's fee for what it lacks.
//!
//! The rules are in `rules/cargo.rs`: contracted goods load as pallets, the town's own first (`PlanOwed`, then at
//! departure, sealed as they leave), the rest of what's owed packed on pallets then, none of it left in the wagon's cargo;
//! goods for sale go crated a good to a lot when there are two stacks or more; the consignee opens what's made out to it
//! (`Settle`: its goods into the wagon's cargo for `rules/contracts.rs`'s `settle` to deliver); a wagon fetches home what's made out to its home and takes the far
//! end's spare empties, first as it arrives (`Papers`); a farm crates what falls due under its forward sales in lots
//! (`Work`); the buyer's wagon carries what it owes for goods a player brought (`Papers`).
//!
//! **Stood in for:** the crate's slots are Minecraft's; here a warehouse has room for `crate_room` crates of
//! pallets. A pallet delivered under its contract is stored as it came in the mod (plain stock, nine stacks to a
//! slot, papers filed): in the core, stock (it has no slots), its crate kept empty to ship with. Mixed crates, the
//! freight forwarder's tickets and bills on crates, and the counting house's clerks are later slices'.

use std::collections::BTreeMap;

use bevy_ecs::prelude::Component;
use serde::{Deserialize, Serialize};

use crate::cash_book::{post, CashBook, Reason, Side};
use crate::content::{Content, GoodId};
use crate::contracts::Book;
use crate::messages::{RouteId, TownId};
use crate::pallet::{package_of, At, Caps, Handling, Line, Load, Manifest, Package, Pallet, PalletId, Pallets, LOOSE};
pub use crate::pallet::{Lading, Payment};
use crate::state::{Ledger, Wagon};

/// Stacks of goods a crate holds (`Crates.GOODS`).
pub const CRATE_STACKS: u32 = 9;
/// A counting house's clerks use half the stationery a document, and charge this for one they lack it for
/// (`Paperwork.COUNTING_STATIONERY`, `CLERK_EMERALDS`): for `buildings-and-works`.
pub const COUNTING_STATIONERY: f64 = 0.5;
pub const CLERK_EMERALDS: f64 = 0.02;
/// What a document is, for the books (`Paperwork.CONTRACT`, `LADING`, `EXCHANGE`, `LEDGER`).
pub const LADING: &str = "bill of lading";
pub const CONTRACT: &str = "contract";
pub const EXCHANGE: &str = "bill of exchange";
pub const LEDGER: &str = "ledger page";
/// The goods paperwork takes, by the names the mod's are (`Items.PAPER`, `INK_SAC`, `FEATHER`).
pub const PAPER: &str = "paper";
pub const INK: &str = "ink_sac";
pub const QUILL: &str = "feather";

/// Stacks `count` of a good fills, loose.
pub fn stacks_of(content: &Content, good: GoodId, count: f64) -> u32 {
    if count <= 0.0 { 0 } else { (count / content.goods[good].stack.max(1) as f64).ceil() as u32 }
}

/// Stacks loose goods fill, each good its own.
pub fn loose_stacks(content: &Content, loose: &[f64]) -> u32 {
    loose.iter().enumerate().map(|(g, n)| stacks_of(content, g, *n)).sum()
}

/// Stacks of cargo space a wagon's load takes: each loose good its stacks, each pallet aboard and each lot for sale what's
/// in it, at least a stack a crate (`Crates.stacks`).
pub fn stacks(content: &Content, pallets: &Pallets, route: RouteId, wagon: &Wagon) -> u32 {
    loose_stacks(content, &wagon.cargo)
        + pallets.at_place(At::Wagon(route)).map(|p| p.slots(content)).sum::<u32>()
        + wagon.for_sale.iter().map(|l| l.slots(content)).sum::<u32>()
}

/// What a wagon carries crated of each good, on its pallets and in its lots for sale (with its loose cargo, what's
/// aboard), each good's counts summed in the order they're aboard.
pub fn crated_aboard(pallets: &Pallets, route: RouteId, wagon: &Wagon, good: GoodId) -> f64 {
    pallets.at_place(At::Wagon(route)).filter(|p| p.packages.iter().any(|pk| pk.good == good)).map(|p| p.count_of(good))
        .chain(wagon.for_sale.iter().filter(|l| l.good() == good).map(|l| l.count())).sum()
}

/// How many items one package of a good's kind holds: its kind's stacks of the good (a crate's nine); `None` for a good
/// that is not packed (`Good.package` loose).
fn items_per_package(content: &Content, good: GoodId) -> Option<f64> {
    let (kind, per, _) = package_of(content, good);
    (kind != LOOSE).then(|| per as f64 * content.goods[good].stack.max(1) as f64)
}

/// Lays `count` of a good onto a pallet's `packages`, the way its kind packs it (`Good.package`): the last part-full package
/// of that kind and good is filled first (a pallet topped up, a second good of the key beside the first), then new
/// packages of a kind's stacks each, the last part-full; a good that isn't packed lies in one loose package (stacks on the
/// pallet). Returns the place of the first package it added (the packages from there are new, to be had: crates or
/// barrels; the one it filled is not).
pub fn lay(content: &Content, packages: &mut Vec<Package>, good: GoodId, count: f64) -> usize {
    let first_new = packages.len();
    let (kind, ..) = package_of(content, good);
    let mut left = count.max(0.0);
    if left <= 0.0 {
        return first_new;
    }
    match items_per_package(content, good) {
        None => match packages.iter_mut().find(|p| p.kind == LOOSE && p.good == good) {
            Some(p) => p.count += left,
            None => packages.push(Package::new(LOOSE, good, left)),
        },
        Some(per) => {
            if let Some(p) = packages.iter_mut().find(|p| p.kind == kind && p.good == good && p.count < per) {
                let add = left.min(per - p.count);
                p.count += add;
                left -= add;
            }
            while left > 0.0 {
                let lot = left.min(per);
                packages.push(Package::new(kind, good, lot));
                left -= lot;
            }
        }
    }
    first_new
}

/// `count` of a good in packages of its kind (`Good.package`: crates of nine stacks, barrels, bundles), the last part-full
/// (one, empty, for none): how a shipment's goods lie in their packages (`Crates.pack`'s nine stacks a crate). A good that
/// isn't packed lies in one loose package.
pub fn crated(content: &Content, good: GoodId, count: f64) -> Vec<Package> {
    let mut packages = Vec::new();
    if count <= 0.0 {
        packages.push(Package::new(package_of(content, good).0, good, 0.0));
    } else {
        lay(content, &mut packages, good, count);
    }
    packages
}

/// Goods for sale crated aboard a wagon (`Caravans.lots`): an open lot of one good, its crates. No bill, not sealed,
/// sold piecemeal at the far end; what's left comes home.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Lot {
    pub packages: Vec<Package>,
}

impl Lot {
    pub fn good(&self) -> GoodId {
        self.packages.first().map_or(0, |p| p.good)
    }

    /// Its good's count, its crates' summed in order.
    pub fn count(&self) -> f64 {
        self.packages.iter().map(|p| p.count).sum()
    }

    /// Stacks it takes: each crate its stacks, at least one.
    pub fn slots(&self, content: &Content) -> u32 {
        self.packages.iter().map(|p| p.slots(content)).sum()
    }

    /// Crates it's in.
    pub fn crates(&self) -> u32 {
        self.packages.iter().filter(|p| p.kind != LOOSE).count() as u32
    }

    /// The packages that come back empty from it (the returnable kinds).
    pub fn empties(&self, content: &Content) -> u32 {
        crate::pallet::returnables(content, &self.packages)
    }
}

/// A town's empty crates, kept to ship with (its crated goods are its pallets, `At::Town`, in the registry).
#[derive(Component, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Shipments {
    pub empties: u32,
}

impl Shipments {
    /// Takes one empty crate, if it has one (`Crates.takeEmpty`).
    pub fn take_empty(&mut self) -> bool {
        if self.empties > 0 {
            self.empties -= 1;
            true
        } else {
            false
        }
    }

    /// Empty crates beyond the stack it keeps (`empties_kept`), taken to go back with a wagon (`Crates.spareEmpties`).
    pub fn spare_empties(&mut self, kept: u32) -> u32 {
        let spare = self.empties.saturating_sub(kept);
        self.empties -= spare;
        spare
    }
}

/// What a town has crated under a contract, in its warehouse (`ForwardSales.earmarked`).
pub fn earmarked(pallets: &Pallets, town: TownId, contract: usize) -> f64 {
    pallets.at_place(At::Town(town)).filter(|p| p.lading.contract == Some(contract)).map(|p| p.count()).sum()
}

/// The crates a town holds packed, in its warehouse.
pub fn crates_held(pallets: &Pallets, town: TownId) -> u32 {
    pallets.at_place(At::Town(town)).map(|p| p.crates()).sum()
}

/// A town's paperwork (`Paperwork.Desk`): documents drawn up and not yet reckoned, the stationery owed in parts of a
/// sheet, sac and quill, and in all the documents drawn, the paper, ink and quills used and the scribes' fees paid.
#[derive(Component, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Paperwork {
    pub owed: f64,
    pub ink_owed: f64,
    pub quill_owed: f64,
    pub paper_owed: f64,
    pub documents: u64,
    pub paper: u64,
    pub ink: u64,
    pub quills: u64,
    pub scribe_fees: crate::fixed::Fixed,
    /// When it first drew up a document.
    pub since: Option<u64>,
    /// Documents drawn, by what they are.
    pub by_kind: BTreeMap<String, u64>,
}

impl Paperwork {
    /// `count` documents of `kind` drawn up (reckoned when the town is next touched): `Paperwork.drawn`.
    pub fn drawn(&mut self, count: u32, kind: &str, now: u64) {
        self.owed += count as f64;
        self.documents += count as u64;
        *self.by_kind.entry(kind.to_string()).or_insert(0) += count as u64;
        self.since.get_or_insert(now);
    }

    /// {documents, paper, ink, quills, scribe fees in emeralds}: `Paperwork.book`.
    pub fn book(&self) -> [f64; 5] {
        [self.documents as f64, self.paper as f64, self.ink as f64, self.quills as f64, self.scribe_fees.to_f64()]
    }

    /// Documents a day since its first: `Paperwork.perDay`.
    pub fn per_day(&self, now: u64, day: u64) -> f64 {
        self.since.map_or(0.0, |since| self.documents as f64 / ((now.saturating_sub(since)) as f64 / day as f64).max(1.0))
    }
}

/// The money and goods a town's own doings reach: its number, treasury, stock, crates and paperwork, and what it can
/// spare beyond its reserve (counted down as it spends).
pub struct Shipper<'a> {
    pub town: TownId,
    pub money: &'a mut crate::fixed::Fixed,
    pub spare: crate::fixed::Fixed,
    pub store: &'a mut Shipments,
    pub paperwork: &'a mut Paperwork,
}

fn crates_reason() -> Reason {
    Reason::Other("crates".into())
}

fn scribe_reason() -> Reason {
    Reason::Other("scribe".into())
}

/// Has a package of `kind` for a shipment: the town's own empty one first (a returnable kind), else a new one it pays for
/// (`crate_emeralds` a returnable package, the kind's own price a bundle) as far as it can spare beyond its reserve
/// (`Crates.pack`).
fn acquire(content: &Content, ledger: &mut Ledger, cash: &mut CashBook, now: u64, shipper: &mut Shipper, kind: usize) -> bool {
    let returnable = content.packages.get(kind).is_none_or(|k| k.returnable);
    if returnable && shipper.store.take_empty() {
        return true;
    }
    let cost = crate::fixed::Fixed::ceil(if returnable { content.dials.crate_emeralds } else { content.packages[kind].emeralds });
    if shipper.spare >= cost && *shipper.money >= cost {
        if returnable || cost.is_positive() {
            post(ledger, cash, Side::treasury(shipper.town, shipper.money), Side::outside(), cost, crates_reason(), now,
                content.day_ticks);
            shipper.spare -= cost;
        }
        return true;
    }
    false
}

/// Packs `count` of a good for sale in packages of its kind: a lot, no papers and no pallet. The town's own empties first,
/// then new ones as far as it can spare; what can't be had goes loose. None if nothing could be packed. `Crates.pack` for
/// a lot (`Caravans.lots`).
pub fn pack_lot(content: &Content, ledger: &mut Ledger, cash: &mut CashBook, now: u64, shipper: &mut Shipper, good: GoodId,
        count: f64) -> Option<Vec<Package>> {
    if count <= 0.0 || package_of(content, good).0 == LOOSE {
        return None;
    }
    let mut packed = Vec::new();
    for package in crated(content, good, count) {
        if !acquire(content, ledger, cash, now, shipper, package.kind) {
            break; // no crate to be had: the rest goes loose
        }
        packed.push(package);
    }
    (!packed.is_empty()).then_some(packed)
}

/// `count` of a good as packing puts it on pallets of the base kind (`pallet-caps`, `docs/pallets.md` "Weight and
/// volume"): a pallet [`pallet_most`](crate::pallet::pallet_most) at most (its kind's slots, as far as its weight
/// allows), each in packages of the good's kind, the last part-full ([`crated`]). The base content's pallet is one crate.
pub fn pallet_loads(content: &Content, good: GoodId, count: f64) -> Vec<Vec<Package>> {
    let most = crate::pallet::pallet_most(content, good);
    let (mut left, mut loads) = (count.max(0.0), Vec::new());
    loop {
        let lot = left.min(most);
        loads.push(crated(content, good, lot));
        left -= lot;
        if left <= 0.0 {
            return loads;
        }
    }
}

/// What a host hands in as one pallet of a good (`pallet-caps`): `count` of it, at most a pallet's worth
/// ([`pallet_most`](crate::pallet::pallet_most)), crated as packing crates it. A host's pallet is taken as the host made
/// it (`Society::deliver_pallet`, `forward_pallet`, `book`, `hand_over`); this keeps one within its kind's caps.
pub fn one_pallet(content: &Content, good: GoodId, count: f64) -> Vec<Package> {
    crated(content, good, count.min(crate::pallet::pallet_most(content, good)))
}

/// Whether packages are within a pallet of the base kind's caps: its slots and its weight.
fn within_pallet(content: &Content, packages: &[Package]) -> bool {
    let kind = &content.pallet_kinds[crate::pallet::LOAD];
    let slots: u32 = packages.iter().map(|p| p.slots(content)).sum();
    let weight: f64 = packages.iter().map(|p| p.weight(content)).sum();
    slots <= kind.slots(&content.dials) && weight <= kind.weight(&content.dials) + 1e-9
}

/// The most of a good that can still be laid on a pallet already holding `packages`, in whole items, within its kind's
/// caps: a fresh pallet's [`pallet_most`](crate::pallet::pallet_most); a part-full one's by trial (a part-full package of
/// the good's kind is filled before a new one is begun).
pub fn room_for(content: &Content, packages: &[Package], good: GoodId) -> f64 {
    if packages.is_empty() {
        return crate::pallet::pallet_most(content, good);
    }
    let stack = content.goods[good].stack.max(1) as f64;
    let kind = &content.pallet_kinds[crate::pallet::LOAD];
    let (mut lo, mut hi) = (0u64, (kind.slots(&content.dials) as f64 * stack).floor() as u64);
    let fits = |x: u64| {
        let mut trial = packages.to_vec();
        lay(content, &mut trial, good, x as f64);
        within_pallet(content, &trial)
    };
    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        if fits(mid) { lo = mid } else { hi = mid - 1 }
    }
    lo as f64
}

/// How a shipment is packed: whether goods that can't be crated ride loose on their pallet (owed goods under a bill: a
/// pallet is always had), or only what can be crated goes; and whether it tops up the open pallets already at the shipper's
/// that share its key (a farm's earmark).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct How {
    pub loose: bool,
    pub top_up: bool,
}

impl How {
    /// Owed goods: what can't be crated rides loose on its pallet.
    pub const OWED: How = How { loose: true, top_up: false };
    /// Only what can be crated goes on a new pallet (a pallet's goods are the town's to ship, or stay in stock).
    pub const CRATED: How = How { loose: false, top_up: false };
    /// Only what can be crated, topped up into the open pallets of the key first.
    pub const TOP_UP: How = How { loose: false, top_up: true };
}

/// A pallet being made up: a new one, or an open one at the shipper's being topped up. Its packages from `first_new` on are
/// new, to be had; `added` is what of each good the shipment put on it.
#[derive(Clone, Debug)]
pub struct Slot {
    pub id: Option<PalletId>,
    pub handling: Handling,
    pub packages: Vec<Package>,
    first_new: usize,
    pub added: Vec<(GoodId, f64)>,
}

impl Slot {
    fn add(&mut self, good: GoodId, count: f64) {
        match self.added.iter_mut().find(|(g, _)| *g == good) {
            Some((_, n)) => *n += count,
            None => self.added.push((good, count)),
        }
    }

    fn take_back(&mut self, good: GoodId, count: f64) {
        if let Some((_, n)) = self.added.iter_mut().find(|(g, _)| *g == good) {
            *n = (*n - count).max(0.0);
        }
    }
}

/// How a shipment lies on pallets before any package is had: the pallets (`Slot`s) it makes up, by the grouping key, and
/// the packages new to them.
#[derive(Clone, Debug)]
pub struct Layout {
    pub slots: Vec<Slot>,
}

impl Layout {
    /// The packages that must be had (crates and barrels bought or taken from the empties): new, and not loose.
    pub fn crates(&self) -> u32 {
        self.slots.iter().map(|s| s.packages[s.first_new..].iter().filter(|p| p.kind != LOOSE).count() as u32).sum()
    }

    /// New pallets (not tops-up).
    pub fn new_pallets(&self) -> usize {
        self.slots.iter().filter(|s| s.id.is_none()).count()
    }
}

/// Makes up the pallets for `lines` (a good and how many) under `lading`: the grouping key is the bill's (shipper, consignee,
/// contract, payment) with the goods' handling, so goods that need care never share a pallet with goods that don't; goods of
/// one key fill a pallet up to its kind's slots and weight, in packages of each good's own kind, a new pallet when the last
/// is full. With `top_up`, the open pallets at `at` that share the key (not sealed, not claimed by a plan) are filled first,
/// in the order they came.
pub fn layout(content: &Content, pallets: &Pallets, lading: &Lading, lines: &[(GoodId, f64)], at: At, top_up: bool) -> Layout {
    let key = lading.key(crate::pallet::LOAD);
    let mut slots: Vec<Slot> = Vec::new();
    if top_up {
        for p in pallets.at_place(at) {
            if p.seal.is_none() && p.claimed.is_none() && p.kind == key.kind
                && PalletKeyNoHandling::of(&p.lading) == PalletKeyNoHandling::of(lading) {
                slots.push(Slot { id: Some(p.id), handling: p.lading.handling, packages: p.packages.clone(),
                    first_new: p.packages.len(), added: Vec::new() });
            }
        }
    }
    let opened = slots.len();
    for &(good, count) in lines {
        let handling = merge_handling(content.goods[good].handling, lading.handling);
        let mut left = count;
        while left > 0.0 {
            // An open pallet of the key with room, in order; else the last new one of the key; else a new one.
            let candidates: Vec<usize> = (0..opened).chain(slots.iter().rposition(|s| s.id.is_none() && s.handling == handling))
                .filter(|&i| slots[i].handling == handling).collect();
            let mut placed = false;
            for i in candidates {
                let room = room_for(content, &slots[i].packages, good);
                if room <= 0.0 {
                    continue;
                }
                let add = left.min(room);
                lay(content, &mut slots[i].packages, good, add);
                slots[i].add(good, add);
                left -= add;
                placed = true;
                break;
            }
            if !placed {
                slots.push(Slot { id: None, handling, packages: Vec::new(), first_new: 0, added: Vec::new() });
            }
        }
    }
    slots.retain(|s| !s.packages.is_empty() && (s.id.is_none() || s.added.iter().any(|(_, n)| *n > 0.0)));
    Layout { slots }
}

/// A bill's key without its handling: the goods on a pallet decide that.
#[derive(PartialEq, Eq)]
struct PalletKeyNoHandling(TownId, TownId, Option<usize>, crate::pallet::Payment);

impl PalletKeyNoHandling {
    fn of(l: &Lading) -> Self {
        PalletKeyNoHandling(l.shipper, l.consignee, l.contract, l.payment)
    }
}

fn merge_handling(a: Handling, b: Handling) -> Handling {
    Handling { cool: a.cool || b.cool, fragile: a.fragile || b.fragile }
}

/// What a shipment put on pallets: the pallets it made or topped up, in order, and what of each good it packed.
#[derive(Clone, Debug, Default)]
pub struct Packed {
    pub ids: Vec<PalletId>,
    pub goods: Vec<(GoodId, f64)>,
    /// Pallets it made new (a bill of lading each).
    pub new: usize,
}

impl Packed {
    /// What it packed of a good.
    pub fn of(&self, good: GoodId) -> f64 {
        self.goods.iter().filter(|(g, _)| *g == good).map(|(_, n)| n).sum()
    }
}

/// Has the packages of a layout (the town's empties first, then new ones as far as it can spare; a pallet at a time, a
/// package at a time) and puts the pallets in the registry: new ones at `at` with a bill each (drawn at the shipper's), open
/// ones topped up. A package that can't be had goes loose on its pallet when `how.loose` (a pallet is always had), else it and
/// the packages after it stay out (their goods stay in the shipper's stock). Their goods are the caller's to take out of the
/// stock: [`Packed::goods`].
#[allow(clippy::too_many_arguments)]
pub fn execute(content: &Content, ledger: &mut Ledger, cash: &mut CashBook, now: u64, shipper: &mut Shipper,
        pallets: &mut Pallets, lading: &Lading, at: At, how: How, layout: Layout) -> Packed {
    let mut packed = Packed::default();
    let attempted = layout.new_pallets() > 0;
    let mut dry = false;
    for mut slot in layout.slots {
        let tail: Vec<Package> = slot.packages.split_off(slot.first_new);
        for pk in tail {
            if pk.kind == LOOSE {
                merge_loose(&mut slot.packages, pk);
            } else if !dry && acquire(content, ledger, cash, now, shipper, pk.kind) {
                slot.packages.push(pk);
            } else {
                dry = true;
                if how.loose {
                    merge_loose(&mut slot.packages, pk);
                } else {
                    slot.take_back(pk.good, pk.count);
                }
            }
        }
        let added: f64 = slot.added.iter().map(|(_, n)| *n).sum();
        if slot.packages.len() == slot.first_new && slot.id.is_none() || added <= 0.0 && slot.id.is_some() {
            continue;
        }
        for &(g, n) in &slot.added {
            match packed.goods.iter_mut().find(|(good, _)| *good == g) {
                Some((_, total)) => *total += n,
                None => packed.goods.push((g, n)),
            }
        }
        match slot.id {
            Some(id) => {
                pallets.top_up(id, slot.packages);
                packed.ids.push(id);
            }
            None => {
                let bill = Lading { handling: slot.handling, ..lading.clone() };
                packed.ids.push(pallets.pack(bill, slot.packages, at, now));
                packed.new += 1;
            }
        }
    }
    // A bill of lading is drawn up for each pallet made (`Crates.shipping` is drawn before `pack`: one for a shipment
    // however little of it could be crated).
    let drawn = if how.loose { packed.new } else { packed.new.max(usize::from(attempted)) };
    if drawn > 0 {
        shipper.paperwork.drawn(drawn as u32, LADING, now);
    }
    packed
}

fn merge_loose(packages: &mut Vec<Package>, pk: Package) {
    match packages.iter_mut().find(|p| p.kind == LOOSE && p.good == pk.good) {
        Some(p) => p.count += pk.count,
        None => packages.push(Package::new(LOOSE, pk.good, pk.count)),
    }
}

/// Packs goods for a consignee under `lading` at `at` ([`layout`], then [`execute`]): the pallets of each key, within the
/// pallet kind's caps, a bill of lading each, packages of each good's kind (crates first: the town's empties, then new at
/// `crate_emeralds`), what can't be crated loose on its pallet with `how.loose`, else left out. `Crates.pack` and
/// `Crates.shipping`; the pallets are `pallet-caps`' and the key, loose packages and top-up `packing`'s.
#[allow(clippy::too_many_arguments)]
pub fn pack_lines(content: &Content, ledger: &mut Ledger, cash: &mut CashBook, now: u64, shipper: &mut Shipper,
        pallets: &mut Pallets, lading: &Lading, lines: &[(GoodId, f64)], at: At, how: How) -> Packed {
    let layout = layout(content, pallets, lading, lines, at, how.top_up);
    execute(content, ledger, cash, now, shipper, pallets, lading, at, how, layout)
}

/// Packs `count` of a good for a consignee under `lading` at `at`, only what can be crated ([`How::CRATED`]): a pallet each
/// pallet's worth, a bill each. Their ids in order, none if nothing could be crated.
#[allow(clippy::too_many_arguments)]
pub fn pack_pallets(content: &Content, ledger: &mut Ledger, cash: &mut CashBook, now: u64, shipper: &mut Shipper,
        pallets: &mut Pallets, lading: &Lading, good: GoodId, count: f64, at: At) -> Vec<PalletId> {
    pack_lines(content, ledger, cash, now, shipper, pallets, lading, &[(good, count)], at, How::CRATED).ids
}

/// How many of their good the pallets `ids` hold, in all (each pallet's count, summed in order).
pub fn count_of(pallets: &Pallets, ids: &[PalletId]) -> f64 {
    ids.iter().filter_map(|id| pallets.get(*id)).map(|p| p.count()).sum()
}

/// What a wagon holds at most (`pallet-caps`): its crew's caps ([`Caps::of`]: a wagon, its packs, or a porter's one
/// pack), its slots those it was sent out with (`Wagon.slots`, when set: `squad::capacity` as it left or was last
/// trimmed).
pub fn wagon_caps(content: &Content, wagon: &Wagon) -> Caps {
    let mut caps = Caps::of(content, wagon.crew.iter().map(|h| h.trade), wagon.porter);
    if wagon.slots > 0 {
        caps.slots = wagon.slots;
    }
    caps
}

/// What a wagon carries (`pallet-caps`): its loose cargo, its pallets, and its lots for sale (each crate its stacks and
/// weight); its slots are [`stacks`].
pub fn wagon_load(content: &Content, pallets: &Pallets, route: RouteId, wagon: &Wagon) -> Load {
    let mut load = Load::of(content, &wagon.cargo, pallets.at_place(At::Wagon(route)));
    for pk in wagon.for_sale.iter().flat_map(|l| &l.packages) {
        load.slots += pk.slots(content);
        load.weight += pk.weight(content);
    }
    load
}

/// The room left on a wagon: its caps less its load.
pub fn wagon_room(content: &Content, pallets: &Pallets, route: RouteId, wagon: &Wagon) -> Caps {
    wagon_caps(content, wagon).less(&wagon_load(content, pallets, route, wagon))
}

/// Whether a pallet fits on a wagon with what it carries.
pub fn fits(content: &Content, pallets: &Pallets, route: RouteId, wagon: &Wagon, pallet: &Pallet) -> bool {
    wagon_caps(content, wagon).holds(&wagon_load(content, pallets, route, wagon).and(Load::of_pallet(content, pallet)))
}

/// A pallet loaded onto a route's wagon as it leaves `by`'s premises (or a hub's custody): moved aboard (the back of its
/// load), sealed (`docs/pallets.md` "Sealed in transit"), and its line on the wagon's manifest.
pub fn put_aboard(pallets: &mut Pallets, wagon: &mut Wagon, route: RouteId, line: Line, by: TownId, now: u64) {
    pallets.move_to(line.pallet, At::Wagon(route), now);
    pallets.seal_leaving(line.pallet, by, now);
    wagon.manifest.lines.push(line);
}

/// A town's paperwork reckoned when it's touched (`Paperwork.settle`): for the whole documents drawn since, paper, ink
/// and quills from its stock (`stationery` of a sheet each: half with a counting house's clerks), and a fee for those
/// it lacked stationery for (`fee` each: a scribe's, or a clerk's), paid from its treasury as far as it holds.
#[allow(clippy::too_many_arguments)]
pub fn reckon(content: &Content, ledger: &mut Ledger, cash: &mut CashBook, now: u64, town: TownId, money: &mut crate::fixed::Fixed,
        stock: &mut [f64], paperwork: &mut Paperwork, stationery: f64, fee: f64) {
    let d = &content.dials;
    if paperwork.owed < 1.0 {
        return;
    }
    let documents = paperwork.owed.floor();
    paperwork.owed -= documents;
    paperwork.ink_owed += documents * d.ink_per_document * stationery;
    paperwork.quill_owed += documents * d.quill_per_document * stationery;
    paperwork.paper_owed += documents * stationery;
    let mut take = |name: &str, wanted: f64| -> f64 {
        let Some(g) = content.good(name) else { return 0.0 };
        let taken = wanted.min(stock[g].max(0.0).floor()).max(0.0);
        stock[g] -= taken;
        ledger.used[g] += taken;
        taken
    };
    let paper_wanted = paperwork.paper_owed.floor();
    let paper = take(PAPER, paper_wanted);
    paperwork.paper_owed -= paper_wanted;
    let ink_wanted = paperwork.ink_owed.floor();
    let ink = take(INK, ink_wanted);
    let quills_wanted = paperwork.quill_owed.floor();
    let quills = take(QUILL, quills_wanted);
    paperwork.ink_owed -= ink_wanted;
    paperwork.quill_owed -= quills_wanted;
    paperwork.paper += paper as u64;
    paperwork.ink += ink as u64;
    paperwork.quills += quills as u64;
    let lacking = ((paper_wanted - paper) / stationery)
        .max((ink_wanted - ink) / d.ink_per_document / stationery)
        .max((quills_wanted - quills) / d.quill_per_document / stationery)
        .round();
    if lacking > 0.0 {
        let owed = crate::fixed::Fixed::ceil(lacking.min(documents) * fee);
        let paid = owed.min(money.at_least_zero());
        post(ledger, cash, Side::treasury(town, money), Side::outside(), paid, scribe_reason(), now, content.day_ticks);
        paperwork.scribe_fees += owed; // the fee in full, paid or not, as the mod books it (`Paperwork.settle`)
    }
}

/// What the farm keeps back of a crop for its forward sales and rents' coupons, less what's crated for each already
/// (out of the stock's reach): `ForwardSales.setAside`. Its spot sales and its workshop use only the rest.
pub fn set_aside(book: &Book, pallets: &Pallets, farm: TownId, crop: GoodId, now: u64, day: u64) -> f64 {
    book.open.iter().map(|&id| &book.contracts[id])
        .filter(|c| c.seller == farm && c.good == crop && c.in_force())
        .filter_map(|c| c.harvest.map(|h| (c, h)))
        .map(|(c, h)| (c.owed(now + h.window_ticks(day)) - c.shipped - earmarked(pallets, farm, c.id)).max(0.0))
        .sum()
}

/// [`set_aside`] of every crop below `n` at once, by good: one pass over the open contracts, each good's sum in their
/// order as `set_aside` adds it (from -0.0, as `Iterator::sum` starts).
pub fn set_aside_all(book: &Book, pallets: &Pallets, farm: TownId, n: usize, now: u64, day: u64) -> Vec<f64> {
    let mut out = vec![-0.0; n];
    for c in book.open.iter().map(|&id| &book.contracts[id]).filter(|c| c.seller == farm && c.good < n && c.in_force()) {
        if let Some(h) = c.harvest {
            out[c.good] += (c.owed(now + h.window_ticks(day)) - c.shipped - earmarked(pallets, farm, c.id)).max(0.0);
        }
    }
    out
}

/// A pallet received by its consignee, opened (`Crates.receive`): its contract copy filed, its goods into `stock`
/// (stored as they came: in the core, stock), its crates kept empty to ship with; if it wasn't paid for in coin when
/// crated and the consignee is the buyer of a contract in force, the contract's good on it is delivered under it, at most
/// what's owed by the next installment, the payment owed the seller until a wagon between the two carries it
/// (`ContractBook.owe`).
pub fn receive(content: &Content, book: &mut Book, now: u64, town: TownId, stock: &mut [f64], store: &mut Shipments,
        pallet: &Pallet) {
    if let Some(id) = pallet.lading.contract.filter(|_| pallet.lading.copy) {
        book.deliver_copy(id, town);
    }
    for (good, count) in pallet.goods() {
        stock[good] += count;
    }
    store.empties += pallet.empties(content);
    let Some(id) = pallet.lading.contract.filter(|_| !pallet.lading.prepaid()) else { return };
    let c = &book.contracts[id];
    if c.buyer != town || !c.in_force() || !pallet.packages.iter().any(|p| p.good == c.good) {
        return;
    }
    let counted = pallet.count_of(c.good).min(c.owed(now + c.every)).max(0.0);
    let owing = crate::fixed::Fixed::ceil(counted * c.price);
    book.settle(id, 0.0, counted, crate::fixed::Fixed::ZERO, owing);
}

use crate::society::Society;
use crate::state::{RouteState, Stock};

impl Society {
    /// A pallet a player carried into a town's warehouse (`Crates.receive`, called as the Merchant next looks): made out
    /// to the town, it's received (one not paid for in coin delivered under its contract, the money owed the seller till
    /// a wagon between them carries it); made out elsewhere, it waits, crated, to be fetched (its id). One good a pallet.
    pub fn deliver_pallet(&mut self, town: TownId, lading: Lading, packages: Vec<Package>) -> Option<PalletId> {
        let e = self.towns[town];
        let now = self.now();
        let pallet = Pallet { id: 0, kind: crate::pallet::LOAD, lading, packages, at: At::Town(town), since: now, seal: None,
            claimed: None };
        if pallet.consigned_to(town) {
            // Into the warehouse from outside the society, as `deliver` brings goods (the town brought up to date
            // first), then out of the stock as the pallet's, received.
            for (good, count) in pallet.goods() {
                self.deliver(town, good, count);
            }
            let mut store = self.shipments(town);
            let mut stock = self.world.get::<Stock>(e).expect("a town").0.clone();
            for (good, count) in pallet.goods() {
                stock[good] -= count;
            }
            let content = self.content().clone();
            receive(&content, &mut self.world.resource_mut::<Book>(), now, town, &mut stock, &mut store, &pallet);
            self.world.get_mut::<Stock>(e).expect("a town").0 = stock;
            *self.world.get_mut::<Shipments>(e).expect("a town") = store;
            None
        } else {
            for (good, count) in pallet.goods() {
                self.world.resource_mut::<Ledger>().delivered[good] += count;
            }
            Some(self.world.resource_mut::<Pallets>().pack(pallet.lading, pallet.packages, At::Town(town), now))
        }
    }

    /// A town packs goods from its warehouse onto pallets at its premises, as it packs what it ships (`packing`): `lines` (a good and how
    /// many, no more than it holds) under `lading`'s key, the goods of one key sharing pallets within their kind's caps,
    /// packages of each good's kind (the town's empty crates first, else new at `crate_emeralds` as far as it can spare), a
    /// bill of lading drawn for each pallet made, `how` saying whether what can't be crated rides loose on its pallet or stays
    /// in stock, and whether the open pallets of the key already at the town are topped up. The town is brought up to date
    /// first. Returns the pallets made or topped up, in order; the goods packed leave the stock. Refused (nothing packed)
    /// unless the town is `lading`'s shipper, and, under a contract, unless every line is the contract's good: the readers of
    /// a contract's pallets (`collect`, `ship`, set-asides) count a pallet's goods as the contract's.
    pub fn pack_pallets(&mut self, town: TownId, lading: Lading, lines: &[(GoodId, f64)], how: How) -> Vec<PalletId> {
        use bevy_ecs::prelude::Mut;
        use crate::cash_book::CashBook;
        if lading.shipper != town || lading.contract.is_some_and(|id| {
            let good = self.world.resource::<crate::contracts::Book>().contracts.get(id).map(|c| c.good);
            lines.iter().any(|&(g, _)| Some(g) != good)
        }) {
            return Vec::new();
        }
        self.deliver(town, 0, 0.0); // brings the town up to date: nothing delivered
        let (e, now) = (self.towns[town], self.now());
        let content = self.content().clone();
        let reserve = crate::rules::farms::reserve(&content, self.world.get::<crate::state::Trades>(e).expect("a town"),
            self.world.get::<crate::farms::Farm>(e));
        let mut money = self.world.get::<crate::state::Money>(e).expect("a town").0;
        let spare = money - reserve;
        let mut stock = self.world.get::<Stock>(e).expect("a town").0.clone();
        let mut store = self.shipments(town);
        let mut paperwork = self.paperwork(town);
        let mut held = stock.clone();
        let lines: Vec<(GoodId, f64)> = lines.iter().map(|&(g, n)| {
            let n = n.min(held[g].floor()).max(0.0);
            held[g] -= n;
            (g, n)
        }).collect();
        let packed = self.world.resource_scope(|world, mut pallets: Mut<Pallets>| {
            world.resource_scope(|world, mut ledger: Mut<Ledger>| {
                world.resource_scope(|_, mut cash: Mut<CashBook>| {
                    let mut shipper = Shipper { town, money: &mut money, spare, store: &mut store, paperwork: &mut paperwork };
                    pack_lines(&content, &mut ledger, &mut cash, now, &mut shipper, &mut pallets, &lading, &lines,
                        At::Town(town), how)
                })
            })
        });
        for (g, n) in &packed.goods {
            stock[*g] -= n;
        }
        self.world.get_mut::<crate::state::Money>(e).expect("a town").0 = money;
        self.world.get_mut::<Stock>(e).expect("a town").0 = stock;
        *self.world.get_mut::<Shipments>(e).expect("a town") = store;
        *self.world.get_mut::<Paperwork>(e).expect("a town") = paperwork;
        packed.ids
    }

    /// A town's empty crates.
    pub fn shipments(&self, town: TownId) -> Shipments {
        self.world.get::<Shipments>(self.towns[town]).cloned().unwrap_or_default()
    }

    /// Every pallet in the world, where it is.
    pub fn pallets(&self) -> &Pallets {
        self.world.resource::<Pallets>()
    }

    /// The pallets at a place, in arrival order.
    pub fn pallets_at(&self, place: At) -> Vec<Pallet> {
        self.pallets().at_place(place).cloned().collect()
    }

    /// A town's crated goods: its pallets, in its warehouse.
    pub fn town_pallets(&self, town: TownId) -> Vec<Pallet> {
        self.pallets_at(At::Town(town))
    }

    /// A town's paperwork.
    pub fn paperwork(&self, town: TownId) -> Paperwork {
        self.world.get::<Paperwork>(self.towns[town]).cloned().unwrap_or_default()
    }

    /// `count` documents of `kind` drawn up at a town, as a host's own doings draw them (reckoned when it's next
    /// touched).
    pub fn draw_papers(&mut self, town: TownId, count: u32, kind: &str) {
        let now = self.now();
        self.world.get_mut::<Paperwork>(self.towns[town]).expect("a town").drawn(count, kind, now);
    }

    /// The pallets aboard a route's wagon (in load order), its lots for sale, and the empty crates it carries.
    pub fn wagon_pallets(&self, route: RouteId) -> (Vec<Pallet>, Vec<Lot>, u32) {
        self.world.get::<RouteState>(self.routes[route]).and_then(|s| s.wagon.as_ref())
            .map_or((Vec::new(), Vec::new(), 0), |w| (self.pallets_at(At::Wagon(route)), w.for_sale.clone(), w.empties))
    }

    /// What a route's wagon carries crated of a good: on its pallets and in its lots for sale.
    pub fn wagon_crated(&self, route: RouteId, good: GoodId) -> f64 {
        self.world.get::<RouteState>(self.routes[route]).and_then(|s| s.wagon.as_ref())
            .map_or(0.0, |w| crated_aboard(self.pallets(), route, w, good))
    }

    /// A route's wagon's manifest: a line for each pallet aboard.
    pub fn wagon_manifest(&self, route: RouteId) -> Option<Manifest> {
        self.world.get::<RouteState>(self.routes[route]).and_then(|s| s.wagon.as_ref()).map(|w| w.manifest.clone())
    }

    /// What a town has crated under a contract.
    pub fn earmarked(&self, town: TownId, contract: usize) -> f64 {
        earmarked(self.pallets(), town, contract)
    }

    /// What a farm keeps back of a crop for its harvest contracts beyond what it has crated for them.
    pub fn set_aside_beyond_crates(&self, town: TownId, good: GoodId) -> f64 {
        set_aside(self.world.resource::<Book>(), self.pallets(), town, good, self.now(), self.content().day_ticks)
    }

    /// A host's bill of exchange names the pallets it pays for (goods crated for its drawee: their title, as a rule's
    /// bill drawn for crated goods names them: `Bill.pallets`).
    pub fn bill_pallets(&mut self, bill: usize, pallets: &[PalletId]) {
        self.world.resource_mut::<crate::bills::Bills>().bills[bill].pallets.extend_from_slice(pallets);
    }
}
