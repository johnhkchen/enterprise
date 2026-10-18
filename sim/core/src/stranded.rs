//! Stranded paper: a sound town isn't ruined by a closed road (`market/Stranded.java` `reaches` (34), `bill` (44),
//! `crate` (52), `buy` (83), `INSTALLMENTS`; `market/Apprentices.java` `collectable` (125); `market/Discounts.java`
//! `quote`'s stale branch; `contract/Bills.java` `lapse`; `caravan/ForwardSales.java` `earmark`'s stale sweep;
//! `caravan/Caravans.java` `buyStrandedAtTown` (873), `sellStrandedAtHub` (909), `collectForwarding` (the credit
//! branch); `docs/decisions.md` "Stranded paper: a sound town ruined by a closed road").
//!
//! Bills a town holds on a market whose wagons no longer call (its run wound down, say), or past their day, and the
//! goods it crated for that market, paid for by those bills, that no one comes to collect: sound on paper, worth nothing
//! where it lies. A freight hub that runs to the drawee buys them, as a merchant bank would: each bill at its face less
//! the discount desk's fee (`Discounts::quote`: 2%, and `stale_bill_discount` more past its day, more for goods it can't
//! see), less the forwarding of the goods it paid for; paid in coin as far as the hub trusts the drawee and can spare,
//! the rest in the hub's own bills, up to `stranded_installments` of them, a bill term apart, each at most half what the
//! hub's treasury could pay (beyond that it doesn't buy). The bills are endorsed to the hub; the goods go to its forwarder.
//!
//! - **Papers:** the hub's own wagon at a town buys what the town holds ([`crate::rules::stranded::buy_at_town`]); a
//!   town's wagon bound for a hub, or calling at one on its way, carries its stranded paper and sells it there
//!   ([`crate::rules::stranded::load`], `sell_at_stop`, `sell_at_hub`). Never a bill on the hub itself: the town presents
//!   that there, for its face.
//! - **Freight on credit:** the hub's own wagon at a town collects its goods for markets the hub serves; what the town
//!   can't pay of the fare is its bill to the hub, due in `freight_credit_days`, as far as a stack's worth covers the fare
//!   at the lien's share ([`crate::rules::stranded::collect_forwarding`]).
//! - **No offtake without a collector:** the pupil's paid work is crated and billed to the buyer only where a wagon of the
//!   buyer's calls, or the town's wagons reach a hub that runs to the buyer ([`collectable`]).
//! - **Dead paper:** a crate shipped under a contract no longer in force goes back on the shelf unless a bill it was paid
//!   for by is still to be collected (`rules/cargo.rs` `earmark`); a bill no one can carry lapses (`rules/bank.rs`), unless
//!   a hub it reaches would buy it ([`collectable`]).
//! - **Citizens' deposits:** a Merchant short of coin to honour a bill put to it, or to buy stranded paper, borrows its
//!   households' savings (`farms.rs` `borrow`; `rules/bills.rs` `present`).
//!
//! **Where the core differs.** Money is counted in emerald-worth, unfloored, as everywhere in the core; the coin a hub
//! pays is floored to the spur as the mod's (`Math.floor(.. * rate)`), the rest left to its own bills. A bill's paper isn't
//! an item: a town's bills are `Held::Town` and a wagon's `Wagon`, and the mod's `Bills.tidy` (paper no longer the desk's to
//! collect thrown out of its warehouse) has nothing to throw. A wagon carries stranded paper in the room it has beside its
//! cargo (crates only; bills are documents). Not yet: the forwarder's collection on consignment (`Forwarder.collect`), and
//! another hub's wagon calling on its way home collecting for a hub (`collectForwarding`'s cash-only branch).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::bank::Discounts;
use crate::bills::{Bill, BillState, Bills};
use crate::content::Content;
use crate::contracts::Book;
use crate::freight::{forwarding_price, Freight, Network};
use crate::fixed::Fixed;
use crate::messages::TownId;
use crate::pallet::{At, Pallet, PalletId, Pallets};

/// A hub's book of the stranded paper it has bought, and the freight it has carried on credit.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StrandedBook {
    /// Deals made, the bills bought, their face, the coin paid, the hub's own bills promised, and the crates taken.
    pub deals: u32,
    pub bills: u32,
    pub face: Fixed,
    pub coin: Fixed,
    pub promised: Fixed,
    pub crates: u32,
    /// Freight on credit: the stacks carried, and the fares left as the town's bill.
    pub credit_stacks: u32,
    pub credit: Fixed,
}

impl Freight {
    /// A hub's book of stranded paper (kept with the freight rules' own state: a resource of its own would be another entity in
    /// the world, and main's worlds would not stay put).
    pub fn stranded(&mut self, hub: TownId) -> &mut StrandedBook {
        self.stranded.entry(hub).or_default()
    }
}

/// Can the hub reach this market: a run of its own there, or it's the hub (`Stranded.reaches`)?
pub fn reaches(net: &Network, hub: TownId, market: TownId) -> bool {
    market == hub || !net.legs(hub, market).is_empty()
}

/// Does a wagon of `from`'s still call at `to` (`Stranded.calls`)? A route wound down no longer does.
pub fn calls(net: &Network, from: TownId, to: TownId) -> bool {
    net.route_between(from, to).is_some()
}

/// Can the buyer get the work, and the town its money: a wagon of the buyer's calls at the town, or the town's wagons
/// reach a freight hub (the far end of a run of theirs, or one it calls at on its way) that runs to the buyer
/// (`Apprentices.collectable`).
pub fn collectable(net: &Network, desk: TownId, buyer: TownId) -> bool {
    if calls(net, buyer, desk) {
        return true;
    }
    net.runs.values().filter(|r| r.from == desk).any(|r| {
        let hub = if net.is_hub(r.to) { Some(r.to) } else { r.stop.map(|(h, _)| h) };
        hub.is_some_and(|h| reaches(net, h, buyer))
    })
}

/// What a hub deals with: the world as it stands, the hub and the town it buys from, whether the hub's market has a
/// counting house, and the hour.
pub struct Terms<'a> {
    pub content: &'a Content,
    pub net: &'a Network,
    pub book: &'a Book,
    pub hub: TownId,
    pub town: TownId,
    pub counting: bool,
    pub now: u64,
    pub rate: f64,
}

/// A bill the town holds that the hub would buy: one it can carry to its drawee, whose own wagons don't call (or past its
/// day) (`Stranded.bill`). Not a bill on the hub itself: the town presents that there, for its face.
pub fn bill_ok(t: &Terms, bill: &Bill) -> bool {
    bill.state == BillState::Outstanding && bill.payee == t.town && bill.drawee != t.town && bill.drawee != t.hub
        && reaches(t.net, t.hub, bill.drawee) && (!calls(t.net, bill.drawee, t.town) || bill.due < t.now)
}

/// A pallet the town shipped, paid for, that its consignee doesn't come for (`Stranded.crate`).
pub fn crate_ok(t: &Terms, s: &Pallet) -> bool {
    let to = s.lading.consignee;
    s.lading.prepaid() && s.lading.shipper == t.town && to != t.town && reaches(t.net, t.hub, to) && !calls(t.net, to, t.town)
}

/// What the hub pays: coin now (in spurs), its own bills for the rest (in installments), and the paper it bought.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Deal {
    pub spurs: u64,
    pub hub_bills: Vec<usize>,
    /// The pallets bought with the bills: those the bills name.
    pub crates: Vec<PalletId>,
    pub bills: Vec<usize>,
    pub face: Fixed,
    /// The price, before the coin: what the hub promised in all.
    pub price: f64,
}

impl Deal {
    /// The coin, in emeralds at the mint's `rate`, rounded down to the thousandth (what the hub pays out).
    pub fn coin(&self, rate: f64) -> Fixed {
        Fixed::floor(self.spurs as f64 / rate)
    }

    pub fn hub_bill(&self) -> Option<usize> {
        self.hub_bills.first().copied()
    }
}

/// The hub buys the stranded paper among `crates` (pallets, by id) and `offered` (bill numbers; the rest is left in them):
/// the bills whose goods it can forward, and the goods they pay for among the crates (the pallets they name, and any other
/// for their drawees). Pays coin up to `coin` emeralds; the rest in its bills on itself,
/// due a bill term apart. Returns nothing if nothing was worth buying. It promises no more in its own bills than its
/// treasury (`means`, emeralds) could pay: installments every bill term, each at most half of it; beyond that it doesn't
/// buy (`Stranded.buy`).
#[allow(clippy::too_many_arguments)]
pub fn buy(t: &Terms, bills: &mut Bills, pallets: &Pallets, crates: &mut Vec<PalletId>, offered: &mut Vec<usize>, coin: f64,
        means: f64) -> Option<Deal> {
    let d = &t.content.dials;
    let day = t.content.day_ticks;
    // At least one: a dial set to nothing would leave the hub no way to pay the part it can't in coin.
    let installments = d.stranded_installments.max(1);
    let mut bought: BTreeMap<TownId, Vec<usize>> = BTreeMap::new();
    let (mut face, mut price) = (Fixed::ZERO, 0.0);
    let mut least: f64 = 1.0; // the trust the hub has in the least trusted drawee bought
    // Paper on goods in hand: a pallet for the drawee among the crates.
    let crated: BTreeSet<TownId> = crates.iter().filter_map(|id| pallets.get(*id)).filter(|c| crate_ok(t, c))
        .map(|c| c.lading.consignee).collect();
    let quote_means = if means == f64::MAX { coin } else { means };
    let mut kept = Vec::new();
    for id in std::mem::take(offered) {
        let bill = &bills.bills[id];
        if !bill_ok(t, bill) {
            kept.push(id);
            continue;
        }
        // Priced and capped at the hub's discount desk, as any paper it buys (its book, the drawee's share).
        let quote = Discounts::quote(bills, t.book, t.hub, t.counting, bill, crated.contains(&bill.drawee), face, 0.0,
            Fixed::floor(quote_means.min(1e15)), t.now, day, d);
        let paying = bill.amount.to_f64() * quote.share;
        if !quote.taken() || price + paying - coin.max(0.0) > installments as f64 * means / 2.0 {
            kept.push(id); // more than it could pay in its own bills: it leaves the rest
            continue;
        }
        face += bill.amount;
        price += paying;
        least = least.min(t.book.trust(t.hub, bill.drawee, t.now, day));
        bought.entry(bill.drawee).or_default().push(id);
    }
    *offered = kept;
    if bought.is_empty() {
        return None;
    }
    // The goods its paper pays for go with it: the pallets the bills it buys name (`Bill.pallets`), by id; and, as the
    // core had it before bills named their goods (`Stranded.buy`: crates for the drawee), any other paid-for pallet among
    // them made out to a drawee whose paper it bought. Their forwarding comes off the price. (By id alone, a pallet whose
    // own bill the hub refused would stay with the town's wagon, and go to the forwarder for its fee: that moves outcomes,
    // so it waits for the integrator: `docs/pallets.md` divergence 7.)
    let named: BTreeSet<PalletId> = bought.values().flatten().flat_map(|&b| bills.bills[b].pallets.iter().copied()).collect();
    let mut goods = Vec::new();
    let mut left = Vec::new();
    for id in std::mem::take(crates) {
        match pallets.get(id).filter(|c| (named.contains(&id) || bought.contains_key(&c.lading.consignee)) && crate_ok(t, c)) {
            Some(c) => {
                let to = c.lading.consignee;
                let forwarding = if to == t.hub { 0.0 } else {
                    t.net.legs(t.hub, to).iter().map(|l| forwarding_price(t.content, t.net, l)).fold(f64::INFINITY, f64::min)
                };
                price -= if forwarding.is_finite() { forwarding } else { 0.0 } * c.slots(t.content) as f64;
                goods.push(id);
            }
            None => left.push(id),
        }
    }
    *crates = left;
    price = price.max(0.0);
    let spurs = (coin.max(0.0).min(price * least) * t.rate).floor() as u64;
    let rest = price - spurs as f64 / t.rate;
    // The rest in installments, a bill term apart, each no more than half what it could pay.
    let pieces = if rest <= 0.01 { 0 } else {
        (installments as f64).min(1f64.max((rest / 1f64.max(rest.min(means / 2.0))).ceil())) as u32
    };
    let term = (crate::bills::TERM_DAYS * day as f64) as u64;
    let hub_bills = (0..pieces).map(|i| {
        bills.draw_term(t.hub, t.town, Fixed::ceil(rest / pieces as f64 / (1.0 + crate::bills::INTEREST)), t.now, day,
            term * (i as u64 + 1), None)
    }).collect();
    let bills_bought: Vec<usize> = bought.into_values().flatten().collect();
    for &id in &bills_bought {
        bills.endorse(id, t.hub);
    }
    Some(Deal { spurs, hub_bills, crates: goods, bills: bills_bought, face, price })
}

use crate::society::Society;

impl Society {
    /// The hub buys stranded paper a town holds, among `crates` (the town's pallets, by id) and `bills` (bill numbers),
    /// now (`Stranded.buy`, as the mod's test calls it): coin up to `coin` emeralds from nowhere in the society (the host's
    /// purse; the caller posts it), the rest in the hub's bills. What it takes leaves `crates` and `bills`: the pallets go
    /// into its forwarder's custody, to be carted in; its own bills are held by the town (filed). Returns the deal.
    pub fn buy_stranded(&mut self, hub: TownId, town: TownId, crates: &mut Vec<PalletId>, bills: &mut Vec<usize>, coin: f64)
            -> Option<Deal> {
        self.buy_stranded_within(hub, town, crates, bills, coin, f64::MAX)
    }

    /// As [`Society::buy_stranded`], promising no more in the hub's own bills than its treasury (`means`, emeralds) could pay
    /// (`Stranded.buy`'s `means`).
    pub fn buy_stranded_within(&mut self, hub: TownId, town: TownId, crates: &mut Vec<PalletId>, bills: &mut Vec<usize>,
            coin: f64, means: f64) -> Option<Deal> {
        let net = self.network();
        let content = self.content().clone();
        let (now, rate) = (self.now(), self.mint_rate());
        let counting = self.world.get::<crate::state::Trades>(self.towns[hub]).is_some_and(|t| t.counting_house);
        let book = self.world.resource::<Book>().clone();
        let pallets = self.world.resource::<Pallets>().clone();
        let mut ledger = self.world.resource_mut::<Bills>();
        let terms = Terms { content: &content, net: &net, book: &book, hub, town, counting, now, rate };
        let deal = buy(&terms, &mut ledger, &pallets, crates, bills, coin, means)?;
        let mut promised = Fixed::ZERO;
        for &id in &deal.hub_bills {
            ledger.bills[id].held = crate::bills::Held::Town(town);
            promised += ledger.bills[id].amount;
        }
        for &id in &deal.bills {
            ledger.bills[id].held = crate::bills::Held::Town(hub);
        }
        let mut freight = self.world.resource_mut::<Freight>();
        let book = freight.stranded(hub);
        book.deals += 1;
        book.bills += deal.bills.len() as u32;
        book.face += deal.face;
        book.coin += deal.coin(rate);
        book.promised += promised;
        book.crates += deal.crates.len() as u32;
        for &id in &deal.crates {
            let mut pallets = self.world.resource_mut::<Pallets>();
            pallets.move_to(id, At::Custody(hub), now);
            pallets.seal_leaving(id, town, now);
            self.world.get_mut::<crate::freight::Hub>(self.towns[hub]).expect("a town").custody
                .insert(id, crate::freight::Custody { carted: false, booked: None });
        }
        Some(deal)
    }

    /// A hub's book of stranded paper bought and freight on credit.
    pub fn stranded_book(&self, hub: TownId) -> StrandedBook {
        self.world.resource::<Freight>().stranded.get(&hub).cloned().unwrap_or_default()
    }

    /// Closes the road from `from` to `to`: its wagons call no more (the route is wound down, as its owner does for a run
    /// by a hub). Returns whether such a route was open.
    pub fn close_route(&mut self, from: TownId, to: TownId) -> bool {
        let now = self.now();
        let mut closed = false;
        for &e in &self.routes {
            let Some(route) = self.world.get::<crate::state::Route>(e).copied() else { continue };
            if route.from_id == from && route.to_id == to {
                if let Some(mut state) = self.world.get_mut::<crate::state::RouteState>(e) {
                    if state.freight.wound_down.is_none() {
                        state.freight.wound_down = Some(now);
                        closed = true;
                    }
                }
            }
        }
        closed
    }

    /// Can the buyer get the work from the desk, and the desk its money (`Apprentices.collectable`)?
    pub fn collectable(&self, desk: TownId, buyer: TownId) -> bool {
        collectable(&self.network(), desk, buyer)
    }
}
