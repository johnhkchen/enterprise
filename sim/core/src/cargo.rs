//! Crates and their papers, as the mod packs and opens them (`cargo/Crates.java` `pack`, `shipping`, `store`, `open`,
//! `receive`, `consignedTo`, `stacks`, `takeEmpty`, `spareEmpties`, `EMPTIES_KEPT`, `CRATE_EMERALDS`; `cargo/Papers.java`
//! `billOfLading`, `contractCopy`; `cargo/Paperwork.java` `drawn`, `settle`, `book`, `INK_PER_DOCUMENT`,
//! `QUILL_PER_DOCUMENT`, `SCRIBE_EMERALDS`, `COUNTING_STATIONERY`, `CLERK_EMERALDS`; `caravan/ForwardSales.java`
//! `earmark`, `earmarked`, `takeEarmarked`, `setAside`; `caravan/Caravans.java` `pickContracted`, `settleContracts`,
//! `collectExWorks`, `lots`, `arriveAtDestination` (crates made out to the caravan's home fetched, spare empties
//! returned), `settleOwing`; `contract/ContractBook.java` `owe`, `owing`, `paidOwing`).
//!
//! Goods crated under a bill of lading are a [`Pallet`] in the registry ([`Pallets`], `pallet.rs`): one good, so many of
//! it, in crates of nine stacks (the last part-full), from a shipper, made out to a consignee under a contract, with a
//! bill saying who pays for it ([`Payment`]: owed, paid in coin when crated, or by a bill of exchange drawn on the buyer)
//! and whether it carries a copy of the contract. It takes the cargo space of what's in it, at least a stack a crate
//! ([`Pallet::slots`]). A town's pallets (`At::Town`) are out of its [`Stock`](crate::state::Stock)'s reach (the lockers,
//! the workshop and spot sales see only the stock); its [`Shipments`] are the empty crates it keeps to ship with. Goods
//! for sale go crated as an open [`Lot`] aboard a wagon: no bill, not sealed, sold piecemeal. Packing takes the town's own
//! empty crates first, then new ones it pays `crate_emeralds` each for, as far as it can spare beyond its reserve; what
//! it can't crate goes loose, without papers. Every bill of lading is a document drawn up at the shipper's
//! ([`Paperwork`]): a sheet of paper, an eighth of an ink sac and a thirty-second of a feather from its stock when it's
//! next touched, or a scribe's fee for what it lacks.
//!
//! The rules are in `rules/cargo.rs`: contracted goods load as pallets, the town's own first (`PlanOwed`, then at
//! departure, sealed as they leave); goods for sale go crated a good to a lot when there are two stacks or more; the
//! consignee opens what's made out to it (`Settle`); a wagon fetches home what's made out to its home and takes the far
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
use crate::pallet::{At, Line, Manifest, Package, Pallet, PalletId, Pallets, CRATE};
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
    pallets.at_place(At::Wagon(route)).filter(|p| p.good() == good).map(|p| p.count())
        .chain(wagon.for_sale.iter().filter(|l| l.good() == good).map(|l| l.count())).sum()
}

/// `count` of a good in crates of nine stacks, the last part-full (one crate, empty, for none): how a shipment's goods
/// lie in their crates (`Crates.pack`'s nine stacks a crate).
pub fn crated(content: &Content, good: GoodId, count: f64) -> Vec<Package> {
    let per_crate = CRATE_STACKS as f64 * content.goods[good].stack.max(1) as f64;
    let (mut left, mut packages) = (count.max(0.0), Vec::new());
    loop {
        let lot = left.min(per_crate);
        packages.push(Package::new(CRATE, good, lot));
        left -= lot;
        if left <= 0.0 {
            return packages;
        }
    }
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
        self.packages.len() as u32
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

/// Packs `count` of a good in crates: a crate a nine stacks, the town's own empties first, then new ones at
/// `crate_emeralds` each as far as it can spare. Goods with a consignee (`papers`) draw a bill of lading at the shipper's,
/// crated or not (`Crates.shipping` is drawn before `pack`); a lot for sale has no papers. Returns the crates packed (none
/// if nothing could be crated): the rest goes loose, without papers. `Crates.pack`.
#[allow(clippy::too_many_arguments)]
pub fn pack(content: &Content, ledger: &mut Ledger, cash: &mut CashBook, now: u64, shipper: &mut Shipper,
        good: GoodId, count: f64, papers: bool) -> Option<Vec<Package>> {
    if count <= 0.0 {
        return None;
    }
    if papers {
        shipper.paperwork.drawn(1, LADING, now);
    }
    let per_crate = CRATE_STACKS as f64 * content.goods[good].stack.max(1) as f64;
    let cost = crate::fixed::Fixed::ceil(content.dials.crate_emeralds);
    let (mut left, mut packages) = (count, Vec::new());
    while left > 0.0 {
        let lot = left.min(per_crate);
        let had = shipper.store.take_empty() || (shipper.spare >= cost && *shipper.money >= cost && {
            post(ledger, cash, Side::treasury(shipper.town, shipper.money), Side::outside(), cost, crates_reason(), now,
                content.day_ticks);
            shipper.spare -= cost;
            true
        });
        if !had {
            break; // no crate to be had: the rest goes loose
        }
        packages.push(Package::new(CRATE, good, lot));
        left -= lot;
    }
    (!packages.is_empty()).then_some(packages)
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
/// crated and the consignee is the buyer of a contract in force, it's delivered under it, at most what's owed by the next
/// installment, the payment owed the seller until a wagon between the two carries it (`ContractBook.owe`).
pub fn receive(book: &mut Book, now: u64, town: TownId, stock: &mut [f64], store: &mut Shipments, pallet: &Pallet) {
    let (good, count) = (pallet.good(), pallet.count());
    if let Some(id) = pallet.lading.contract.filter(|_| pallet.lading.copy) {
        book.deliver_copy(id, town);
    }
    stock[good] += count;
    store.empties += pallet.crates();
    let Some(id) = pallet.lading.contract.filter(|_| !pallet.lading.prepaid()) else { return };
    let c = &book.contracts[id];
    if c.buyer != town || !c.in_force() || c.good != good {
        return;
    }
    let counted = count.min(c.owed(now + c.every)).max(0.0);
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
        let (good, count) = (pallet.good(), pallet.count());
        if pallet.consigned_to(town) {
            // Into the warehouse from outside the society, as `deliver` brings goods (the town brought up to date
            // first), then out of the stock as the pallet's, received.
            self.deliver(town, good, count);
            let mut store = self.shipments(town);
            let mut stock = self.world.get::<Stock>(e).expect("a town").0.clone();
            stock[good] -= count;
            receive(&mut self.world.resource_mut::<Book>(), now, town, &mut stock, &mut store, &pallet);
            self.world.get_mut::<Stock>(e).expect("a town").0 = stock;
            *self.world.get_mut::<Shipments>(e).expect("a town") = store;
            None
        } else {
            self.world.resource_mut::<Ledger>().delivered[good] += count;
            Some(self.world.resource_mut::<Pallets>().pack(pallet.lading, pallet.packages, At::Town(town), now))
        }
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
