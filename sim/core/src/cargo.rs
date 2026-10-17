//! Crates and their papers, as the mod packs and opens them (`cargo/Crates.java` `pack`, `shipping`, `store`, `open`,
//! `receive`, `consignedTo`, `stacks`, `takeEmpty`, `spareEmpties`, `EMPTIES_KEPT`, `CRATE_EMERALDS`; `cargo/Papers.java`
//! `billOfLading`, `contractCopy`; `cargo/Paperwork.java` `drawn`, `settle`, `book`, `INK_PER_DOCUMENT`,
//! `QUILL_PER_DOCUMENT`, `SCRIBE_EMERALDS`, `COUNTING_STATIONERY`, `CLERK_EMERALDS`; `caravan/ForwardSales.java`
//! `earmark`, `earmarked`, `takeEarmarked`, `setAside`; `caravan/Caravans.java` `pickContracted`, `settleContracts`,
//! `collectExWorks`, `lots`, `arriveAtDestination` (crates made out to the caravan's home fetched, spare empties
//! returned), `settleOwing`; `contract/ContractBook.java` `owe`, `owing`, `paidOwing`).
//!
//! A [`Shipment`] is goods crated as a unit: one good, so many of it, from a shipper, made out to a consignee under a
//! contract, with a bill of lading saying whether it was paid for when crated (`settled`) and a copy of the contract.
//! It takes the cargo space of what's in it, at least a stack ([`Shipment::stacks`]); a crate holds nine stacks. A
//! town's [`Shipments`] are its crated goods, out of its [`Stock`](crate::state::Stock)'s reach (the lockers, the
//! workshop and spot sales see only the stock), and the empty crates it keeps to ship with. Packing a shipment takes
//! the town's own empty crates first, then new ones it pays `crate_emeralds` each for, as far as it can spare beyond
//! its reserve; what it can't crate goes loose, without papers. Every bill of lading is a document drawn up at the
//! shipper's ([`Paperwork`]): a sheet of paper, an eighth of an ink sac and a thirty-second of a feather from its stock
//! when it's next touched, or a scribe's fee for what it lacks.
//!
//! The rules are in `rules/cargo.rs`: contracted goods load as shipments, crated ones first (`PlanOwed`, then at
//! departure); goods for sale go crated a good to a crate when there are two stacks or more; the consignee opens what's
//! made out to it (`Settle`); a wagon fetches home what's made out to its home and takes the far end's spare empties,
//! first as it arrives (`Papers`); a farm crates what falls due under its forward sales in lots (`Work`); the buyer's wagon carries what
//! it owes for goods a player brought (`Papers`).
//!
//! **Stood in for:** the crate's slots are Minecraft's; here a warehouse has room for `crate_room` crates of
//! shipments. A shipment delivered under its contract is stored as it came in the mod (plain stock, nine stacks to a
//! slot, papers filed): in the core, stock (it has no slots), its crate kept empty to ship with. Mixed crates, the
//! freight forwarder's tickets and bills on crates, and the counting house's clerks are later slices'.

use std::collections::BTreeMap;

use bevy_ecs::prelude::Component;
use serde::{Deserialize, Serialize};

use crate::cash_book::{post, CashBook, Reason, Side};
use crate::content::{Content, GoodId};
use crate::contracts::Book;
use crate::messages::{RouteId, TownId};
use crate::state::Ledger;

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

/// Goods crated as a unit, with their papers.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Shipment {
    pub good: GoodId,
    pub count: f64,
    pub shipper: TownId,
    /// Who the bill of lading makes it out to; none for a lot packed for sale, which has no papers.
    pub consignee: Option<TownId>,
    /// The contract it ships under, if any.
    pub contract: Option<usize>,
    /// Paid for when it was crated: received, it's stored, not booked as owed.
    pub settled: bool,
    /// It carries a copy of the contract, filed by the consignee.
    pub copy: bool,
}

impl Shipment {
    /// Stacks of cargo space it takes: what's in it, at least one (`Crates.stacks`).
    pub fn stacks(&self, content: &Content) -> u32 {
        stacks_of(content, self.good, self.count).max(1)
    }

    /// Crates it's packed in, nine stacks to a crate.
    pub fn crates(&self, content: &Content) -> u32 {
        self.stacks(content).div_ceil(CRATE_STACKS).max(1)
    }

    /// Made out to `town`: its bill of lading names it the consignee (`Crates.consignedTo`).
    pub fn consigned_to(&self, town: TownId) -> bool {
        self.consignee == Some(town)
    }
}

/// Stacks `count` of a good fills, loose.
pub fn stacks_of(content: &Content, good: GoodId, count: f64) -> u32 {
    if count <= 0.0 { 0 } else { (count / content.goods[good].stack.max(1) as f64).ceil() as u32 }
}

/// Stacks of cargo space a load takes: each loose good its stacks, each shipment what's in it, at least one
/// (`Crates.stacks`).
pub fn stacks(content: &Content, loose: &[f64], shipments: &[Shipment]) -> u32 {
    loose.iter().enumerate().map(|(g, n)| stacks_of(content, g, *n)).sum::<u32>()
        + shipments.iter().map(|s| s.stacks(content)).sum::<u32>()
}

/// A town's crated goods (out of its stock's reach), and the empty crates it keeps to ship with.
#[derive(Component, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Shipments {
    pub held: Vec<Shipment>,
    pub empties: u32,
}

impl Shipments {
    /// Of a good, what's crated under a contract (`ForwardSales.earmarked`).
    pub fn earmarked(&self, contract: usize) -> f64 {
        self.held.iter().filter(|s| s.contract == Some(contract)).map(|s| s.count).sum()
    }

    /// Crates held, packed.
    pub fn crates(&self, content: &Content) -> u32 {
        self.held.iter().map(|s| s.crates(content)).sum()
    }

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
    pub scribe_fees: f64,
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
        [self.documents as f64, self.paper as f64, self.ink as f64, self.quills as f64, self.scribe_fees]
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
    pub money: &'a mut f64,
    pub spare: f64,
    pub store: &'a mut Shipments,
    pub paperwork: &'a mut Paperwork,
}

fn crates_reason() -> Reason {
    Reason::Other("crates".into())
}

fn scribe_reason() -> Reason {
    Reason::Other("scribe".into())
}

/// Packs `count` of a good as a shipment like `like` (its consignee, contract, settled, copy; the shipper the town):
/// a crate a nine stacks, the town's own empties first, then new ones at `crate_emeralds` each as far as it can spare.
/// A shipment with a consignee draws a bill of lading at the shipper's, crated or not (`Crates.shipping` is drawn
/// before `pack`); a lot for sale has no papers. Returns the crated part (none if nothing could be crated): the rest
/// goes loose, without papers. `Crates.pack`.
pub fn pack(content: &Content, ledger: &mut Ledger, cash: &mut CashBook, now: u64, shipper: &mut Shipper,
        good: GoodId, count: f64, like: &Shipment) -> Option<Shipment> {
    if count <= 0.0 {
        return None;
    }
    if like.consignee.is_some() {
        shipper.paperwork.drawn(1, LADING, now);
    }
    let per_crate = CRATE_STACKS as f64 * content.goods[good].stack.max(1) as f64;
    let cost = content.dials.crate_emeralds;
    let (mut left, mut crated) = (count, 0.0);
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
        crated += lot;
        left -= lot;
    }
    (crated > 0.0).then(|| Shipment { good, count: crated, shipper: shipper.town, ..like.clone() })
}

/// A town's paperwork reckoned when it's touched (`Paperwork.settle`): for the whole documents drawn since, paper, ink
/// and quills from its stock (`stationery` of a sheet each: half with a counting house's clerks), and a fee for those
/// it lacked stationery for (`fee` each: a scribe's, or a clerk's), paid from its treasury as far as it holds.
#[allow(clippy::too_many_arguments)]
pub fn reckon(content: &Content, ledger: &mut Ledger, cash: &mut CashBook, now: u64, town: TownId, money: &mut f64,
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
        let owed = lacking.min(documents) * fee;
        let paid = owed.min(money.max(0.0));
        post(ledger, cash, Side::treasury(town, money), Side::outside(), paid, scribe_reason(), now, content.day_ticks);
        paperwork.scribe_fees += owed; // the fee in full, paid or not, as the mod books it (`Paperwork.settle`)
    }
}

/// What the farm keeps back of a crop for its forward sales and rents' coupons, less what's crated for each already
/// (out of the stock's reach): `ForwardSales.setAside`. Its spot sales and its workshop use only the rest.
pub fn set_aside(book: &Book, store: Option<&Shipments>, farm: TownId, crop: GoodId, now: u64, day: u64) -> f64 {
    book.open.iter().map(|&id| &book.contracts[id])
        .filter(|c| c.seller == farm && c.good == crop && c.in_force())
        .filter_map(|c| c.harvest.map(|h| (c, h)))
        .map(|(c, h)| (c.owed(now + h.window_ticks(day)) - c.shipped - store.map_or(0.0, |s| s.earmarked(c.id))).max(0.0))
        .sum()
}

/// A shipment received by its consignee (`Crates.receive`): its contract copy filed, its goods into `stock` (stored as
/// they came: in the core, stock), its crates kept empty to ship with; if it wasn't settled when crated and the
/// consignee is the buyer of a contract in force, it's delivered under it, at most what's owed by the next
/// installment, the payment owed the seller until a wagon between the two carries it (`ContractBook.owe`).
pub fn receive(content: &Content, book: &mut Book, now: u64, town: TownId, stock: &mut [f64], store: &mut Shipments,
        shipment: Shipment) {
    if let Some(id) = shipment.contract.filter(|_| shipment.copy) {
        book.deliver_copy(id, town);
    }
    stock[shipment.good] += shipment.count;
    store.empties += shipment.crates(content);
    let Some(id) = shipment.contract.filter(|_| !shipment.settled) else { return };
    let c = &book.contracts[id];
    if c.buyer != town || !c.in_force() || c.good != shipment.good {
        return;
    }
    let counted = shipment.count.min(c.owed(now + c.every)).max(0.0);
    let owing = counted * c.price;
    book.settle(id, 0.0, counted, 0.0, owing);
}

use crate::society::Society;
use crate::state::{RouteState, Stock};

impl Society {
    /// A crate a player carried into a town's warehouse (`Crates.receive`, called as the Merchant next looks): made
    /// out to the town, it's received (an unsettled one delivered under its contract, the money owed the seller till
    /// a wagon between them carries it); made out elsewhere, it waits, crated, to be fetched.
    pub fn deliver_shipment(&mut self, town: TownId, shipment: Shipment) {
        let e = self.towns[town];
        if shipment.consigned_to(town) {
            // Into the warehouse from outside the society, as `deliver` brings goods (the town brought up to date
            // first), then out of the stock as the shipment's, received.
            self.deliver(town, shipment.good, shipment.count);
            let now = self.now();
            let content = self.content().clone();
            let mut store = self.shipments(town);
            let mut stock = self.world.get::<Stock>(e).expect("a town").0.clone();
            stock[shipment.good] -= shipment.count;
            receive(&content, &mut self.world.resource_mut::<Book>(), now, town, &mut stock, &mut store, shipment);
            self.world.get_mut::<Stock>(e).expect("a town").0 = stock;
            *self.world.get_mut::<Shipments>(e).expect("a town") = store;
        } else {
            self.world.resource_mut::<Ledger>().delivered[shipment.good] += shipment.count;
            self.world.get_mut::<Shipments>(e).expect("a town").held.push(shipment);
        }
    }

    /// A town's crated goods and empty crates.
    pub fn shipments(&self, town: TownId) -> Shipments {
        self.world.get::<Shipments>(self.towns[town]).cloned().unwrap_or_default()
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

    /// The shipments aboard a route's wagon, and the empty crates it carries.
    pub fn wagon_shipments(&self, route: RouteId) -> (Vec<Shipment>, u32) {
        self.world.get::<RouteState>(self.routes[route]).and_then(|s| s.wagon.as_ref())
            .map_or((Vec::new(), 0), |w| (w.shipments.clone(), w.empties))
    }

    /// What a town has crated under a contract.
    pub fn earmarked(&self, town: TownId, contract: usize) -> f64 {
        self.shipments(town).earmarked(contract)
    }

    /// What a farm keeps back of a crop for its harvest contracts beyond what it has crated for them.
    pub fn set_aside_beyond_crates(&self, town: TownId, good: GoodId) -> f64 {
        let store = self.world.get::<Shipments>(self.towns[town]);
        set_aside(self.world.resource::<Book>(), store, town, good, self.now(), self.content().day_ticks)
    }
}
