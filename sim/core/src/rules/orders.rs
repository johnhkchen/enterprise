//! A master Merchant and the players' shops through the look (`orders.rs`; `market/Broker.java` `broker`):
//!
//! - **Review** ([`review`]), before sourcing's: a town whose Merchant is a master (level 5) fills the buying shops'
//!   orders at its desk under their contracts from what the warehouse can spare ([`fill`]) and leaves what it can't as
//!   open orders, which sourcing reads and the wagons work on. A desk with no master has nothing open.
//! - **Broker** ([`broker`]), after sourcing's: the rest of `Broker.broker`'s order. It posts what the desk wants and no
//!   market holds as signals ([`signals`]; a desk with no shops still posts its lockers' shortfalls and sourcing's unmet
//!   wants), buys from the selling shops what its orders and lockers want ([`buy`]), and stocks the town's own shops with
//!   its biggest surplus and banks their takings ([`sell_surplus`]).
//! - **Marbles** ([`post_signals`]), after both: the signals the markets a town knows have posted are marbles for its
//!   idle hands (`Jobs.signals`).
//!
//! **One signal stream.** The mod's `Broker.signals` is one list per desk: its open orders, its lockers' wants and the
//! war's unmet wants, kept only if no market it knows holds the good and no stall here sells it. The core has that one
//! list ([`Signals`], built in [`signals`]) and it is the only thing that reaches `jobs::Signals.trades`: sourcing's
//! review (`rules/sourcing.rs`) still finds what its master desks want and no market with a road supplies
//! (`Sourcing::unmet`) but no longer posts it itself; that is folded into this list. A good in both the desk's own wants
//! and sourcing's unmet is one want of the one contract (a shop's order is a seller contract of the desk's, which
//! sourcing reads), so the larger of the two counts, not both; what sourcing judged unmet is kept without asking again
//! whether some market holds it (it holds it with no road to it), as sourcing has always posted it. Sourcing reads the
//! open orders in turn (`Orders::wanted`): the mod's `fill` calls `source` for what the desk can't spare, and the
//! contract sourcing drafts for it is a supplier's, not the shop's. The marbles are made again when a desk's list
//! changes or sourcing has run (its unmet wants move then), as sourcing made them when it was the only poster, so a
//! world with no shops and no lockers' shortfalls has the jobs' marbles it had before this slice.
//!
//! The open orders also reach the wagons: a wagon bound for the desk brings them (`rules/plan.rs`, `rules/arrive.rs`
//! read [`crate::orders::Orders::wanted_of`]) and the desk's own wagons buy them up at the far end (`rules/desires.rs`).

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;

use super::farms::reserve;
use super::Step;
use crate::atlas::{reach, Atlas, Places, Stocktakes};
use crate::bargain;
use crate::cash_book::{post, CashBook, Reason, Side};
use crate::contracts::{self, Book, Incoterm};
use crate::content::{Content, GoodId};
use crate::desires::want_per_day;
use crate::farms::Farm;
use crate::messages::{Output, TownId};
use crate::mint::Mint;
use crate::orders::*;
use crate::roster::Roster;
use crate::sourcing::Sourcing;
use crate::state::*;
use crate::workshop::Workshop;
use crate::works::Works;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems((
        // Pinned so that registering it leaves every other system where main ran it (the look's order without this
        // slice is the same list with these two left out): after the reviews that write what it reads and writes
        // (`works::review`, `squad::train` both write Money, Ledger and the cash book; `atlas::consider`), after the
        // jobs' draw and the mine's dig, and right before sourcing's review and the shops' keeping (`keep_shops` reads
        // Stock), which read what it leaves.
        review.in_set(Step::Review).after(super::works::review).after(super::squad::train).after(super::atlas::consider)
            .after(super::jobs::review).after(super::kit::dig).before(super::sourcing::review),
        // After the day's sourcing, whose unmet wants it folds into the desk's signals (so the marbles are as fresh as
        // sourcing's own were), and before the shops' keeping.
        broker.in_set(Step::Review).after(review).after(super::sourcing::review).before(super::desires::keep_shops),
        // Writes `jobs::Signals` as `works::review` does.
        post_signals.in_set(Step::Review).after(broker).after(super::sourcing::review).after(super::works::review)
            .before(super::desires::keep_shops),
    ));
}


/// Why a desk wants what sourcing found no market with a road supplies.
const UNMET: &str = "a contract no road fills";

fn reason(what: &str) -> Reason {
    Reason::Other(what.to_string())
}

/// What a desk's warehouse aims to hold of a good if a locker takes it (`Warehouse.par`): none if none does.
fn par_of(content: &Content, trades: &Trades, good: GoodId) -> f64 {
    let per_day = want_per_day(trades, good);
    bargain::par(content, per_day, per_day > 0.0)
}

/// What the warehouse can spare of a good: beyond its par, and what's set aside for a forward sale (`Broker.spare`).
fn spare(content: &Content, trades: &Trades, workshop: &Workshop, stock: &Stock, good: GoodId) -> f64 {
    (stock.0[good].floor() - workshop.aside_of(good) - par_of(content, trades, good)).max(0.0)
}

/// What the world looks like to the atlas: where each town stands, how far word of it carries, the routes' lengths.
fn places_of(content: &Content, sites: &Query<(&Town, Option<&Works>)>, routes: &Query<&Route>) -> Places {
    let n = sites.iter().map(|(t, _)| t.id + 1).max().unwrap_or(0);
    let mut places = Places { hexes: vec![None; n], reach: vec![content.dials.word_of_mouth; n], ..Default::default() };
    for (town, works) in sites.iter() {
        places.hexes[town.id] = town.hex;
        places.reach[town.id] = reach(content, works);
    }
    for r in routes.iter() {
        places.roads.entry((r.from_id, r.to_id)).and_modify(|b| *b = (*b).min(r.blocks)).or_insert(r.blocks);
    }
    places
}

/// A desk's master Merchant fills its shops' orders, before sourcing looks at what is open (`Broker.broker` to
/// `fill`); a desk that has none has nothing open and posts nothing.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn review(clock: Res<Clock>, content: Res<Content>, mint: Res<Mint>, order: Res<Towns>, mut orders: ResMut<Orders>,
        mut posted: ResMut<Signals>, mut book: ResMut<Book>, mut ledger: ResMut<Ledger>, mut cash: ResMut<CashBook>,
        mut outbox: ResMut<Outbox>,
        mut towns: Query<(&Town, &mut Stock, &mut Money, &Trades, &Workshop, Option<&Roster>)>) {
    let (now, day) = (clock.now, content.day_ticks);
    let rate = mint.rate(now, day);
    for &e in &order.0 {
        let Ok((town, mut stock, mut money, trades, workshop, roster)) = towns.get_mut(e) else { continue };
        let t = town.id;
        if roster.and_then(|r| r.merchant_level()).unwrap_or(0) < MASTER {
            // Only a master Merchant brokers: a desk that has none has nothing open and posts nothing.
            if orders.wanted.contains_key(&t) || orders.orders.iter().any(|o| o.town == t && o.open > 0.0) {
                orders.wanted.remove(&t);
                orders.orders.iter_mut().filter(|o| o.town == t).for_each(|o| o.open = 0.0);
            }
            if posted.desks.contains_key(&t) {
                posted.desks.remove(&t);
                posted.changed = true;
            }
            continue;
        }
        let Orders { orders: list, wanted, tally, .. } = &mut *orders;
        let tally = tally.entry(t).or_default();

        // Fill the buying shops' orders under their contracts; what's left open is asked of the region.
        let mut open: BTreeMap<GoodId, f64> = BTreeMap::new();
        for o in list.iter_mut().filter(|o| o.town == t) {
            let filled = fill(&content, &mut book, &mut ledger, &mut cash, rate, now, trades, workshop, &mut stock,
                &mut money, o, tally);
            if filled.lots > 0 {
                outbox.0.push(Output::OrderFilled { town: t, order: o.id, good: o.good, lots: filled.lots,
                    count: filled.count, spurs: filled.spurs });
            }
            if o.open > 0.0 {
                *open.entry(o.good).or_insert(0.0) += o.open;
            }
        }
        if open.is_empty() {
            wanted.remove(&t);
        } else {
            wanted.insert(t, open.clone());
        }

    }
}

/// A desk's master Merchant, once sourcing has been, posts what it wants that nothing supplies, buys from the selling
/// shops and stocks the town's own (`Broker.broker`, from `signals` on).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn broker(clock: Res<Clock>, content: Res<Content>, mint: Res<Mint>, atlas: Res<Atlas>, stocktakes: Res<Stocktakes>,
        order: Res<Towns>, routes: Query<&Route>, sites: Query<(&Town, Option<&Works>)>, mut orders: ResMut<Orders>,
        mut posted: ResMut<Signals>, mut ledger: ResMut<Ledger>, mut cash: ResMut<CashBook>, mut outbox: ResMut<Outbox>,
        sourcing: Res<Sourcing>,
        mut towns: Query<(&Town, &mut Stock, &mut Money, &Trades, &Workshop, Option<&Roster>, Option<&Farm>)>) {
    let (now, day) = (clock.now, content.day_ticks);
    let rate = mint.rate(now, day);
    let n = content.goods.len();
    let mut places: Option<Places> = None;
    for &e in &order.0 {
        let Ok((town, mut stock, mut money, trades, workshop, roster, farm)) = towns.get_mut(e) else { continue };
        let t = town.id;
        if roster.and_then(|r| r.merchant_level()).unwrap_or(0) < MASTER {
            continue; // (review has cleared what it had posted)
        }
        let Orders { stalls, shops, wanted, tally, .. } = &mut *orders;
        let tally = tally.entry(t).or_default();
        let open: BTreeMap<GoodId, f64> = wanted.get(&t).cloned().unwrap_or_default();

        // Signals: what it wants that no market it knows holds, nor a stall here sells.
        let unmet = sourcing.unmet.get(&t).map_or(&[][..], |u| u.as_slice());
        let markets = if open.is_empty() && unmet.is_empty() && (0..n).all(|g| want_per_day(trades, g) <= 0.0) {
            Vec::new()
        } else {
            let places = places.get_or_insert_with(|| places_of(&content, &sites, &routes));
            places.known(&content, &atlas, t)
        };
        let signalled = signals(&content, trades, &stock, &open, unmet, &markets, &stocktakes, stalls, t, rate);
        let before = posted.desks.get(&t).cloned().unwrap_or_default();
        // (A want only sourcing found unmet is not told to the host: it never was.)
        for s in signalled.iter().filter(|s| s.why != UNMET && !before.iter().any(|b| b.good == s.good)) {
            outbox.0.push(Output::Signal { town: t, good: s.good, count: s.count, spurs_each: s.spurs_each });
        }
        // (The jobs' marbles are in emeralds: a price moving with the mint's rate alone changes nothing for them.)
        let wants = |l: &[Signal]| l.iter().map(|s| (s.good, s.count)).collect::<Vec<_>>();
        if wants(&signalled) != wants(&before) {
            posted.changed = true;
        }
        if signalled.is_empty() {
            posted.desks.remove(&t);
        } else {
            posted.desks.insert(t, signalled.clone());
        }

        // Buy from the selling shops what its orders and its lockers want.
        for stall in stalls.iter_mut().filter(|s| s.town == t) {
            buy(&content, rate, now, trades, farm, &mut ledger, &mut cash, &mut stock, &mut money, stall, &open,
                &signalled, tally);
        }

        // The town's own shops.
        let mine: Vec<usize> = (0..shops.len()).filter(|&i| shops[i].town == t).collect();
        if !mine.is_empty() {
            sell_surplus(&content, rate, now, trades, workshop, &mut ledger, &mut cash, &mut stock, &mut money,
                shops, &mine, &open, tally);
        }
    }
}

/// What a look delivered to one shop.
struct Filled {
    lots: u32,
    count: f64,
    spurs: f64,
}

/// Takes in how the shop's contract has been judged since the last look (installments kept, and missed) into the shop's
/// own record: `Broker.contractFor` keys trust by the desk and the shop, so one shop's misses never cost another its
/// contract.
fn take_in(content: &Content, book: &Book, now: u64, o: &mut Order) {
    let Some(id) = o.contract else { return };
    let c = &book.contracts[id];
    let (judged, missed) = (c.judged, c.buyer_missed + c.missed);
    let new_missed = missed.saturating_sub(o.seen.1);
    let new_kept = judged.saturating_sub(o.seen.0).saturating_sub(new_missed);
    if new_kept > 0 || new_missed > 0 {
        o.record.add(new_kept as f64, new_missed as f64, now, content.day_ticks);
    }
    o.seen = (judged, missed);
}

/// The contract with a player's shop for what it orders (`Broker.contractFor`): drafted in person at the shop, so in
/// force at once; Ex Works; as many daily lots as its purse would pay for, up to `shop_installments`, at the shop's
/// price, which is then locked. One in force is kept only while the shop still asks the lot and price it was drafted
/// at: relisted, it gets a new one (`book.open(desk, buyer, delivery)` matches the delivery). None if the shop offers
/// less than `agent_floor` of the goods' worth, can't pay a lot, or its own record has run out of trust.
fn contract_for(content: &Content, book: &mut Book, rate: f64, now: u64, o: &mut Order) -> Option<usize> {
    take_in(content, book, now, o);
    if let Some(id) = o.contract.filter(|&id| book.contracts[id].in_force()) {
        if o.drafted_at == (o.lot, o.price) {
            return Some(id);
        }
        book.close(id);
    }
    let day = content.day_ticks;
    let worth = worth_of(content, o.good, o.lot, rate);
    let lots = if o.price > 0.0 { (o.purse / o.price).floor() } else { 0.0 };
    if o.price < content.dials.agent_floor * worth || lots < 1.0 || o.record.trust_at(now, day) < contracts::MIN_TRUST {
        return None;
    }
    let installments = lots.min(content.dials.shop_installments as f64) as u32;
    let id = book.draft(o.town, o.town, o.good, o.lot, o.price / o.lot / rate, now, day, installments, Incoterm::Exw, true, true);
    o.contract = Some(id);
    o.drafted_at = (o.lot, o.price);
    o.seen = (0, 0);
    Some(id)
}

/// Fills a player's order under its contract (`Broker.fill`, `deliver`): a lot as each falls due, from what the
/// warehouse can spare, at the locked price but never above the shop's own, paid from its purse. What the contract
/// still wants beyond the warehouse stays open for the region.
#[allow(clippy::too_many_arguments)]
fn fill(content: &Content, book: &mut Book, ledger: &mut Ledger, cash: &mut CashBook, rate: f64, now: u64, trades: &Trades,
        workshop: &Workshop, stock: &mut Stock, money: &mut Money, o: &mut Order, tally: &mut Tally) -> Filled {
    let day = content.day_ticks;
    let mut filled = Filled { lots: 0, count: 0.0, spurs: 0.0 };
    o.open = 0.0;
    if o.lot < 1.0 {
        return filled;
    }
    let Some(id) = contract_for(content, book, rate, now, o) else {
        return filled; // priced under its worth, nothing to pay with, or no trust left: the Merchant won't take it on
    };
    let (g, lot) = (o.good, o.lot);
    loop {
        let c = &book.contracts[id];
        if !(c.owed(now) >= lot && spare(content, trades, workshop, stock, g) >= lot) {
            break;
        }
        // The locked price at today's rate, but never more than the shop now asks: the Merchant won't charge a player
        // above its own posted price.
        let price = o.price.min((c.price * lot * rate).round());
        if o.purse < price {
            unpaid(content, book, id, now);
            break;
        }
        stock.0[g] -= lot;
        ledger.used[g] += lot;
        o.purse -= price;
        o.delivered += lot;
        o.paid += price;
        post(ledger, cash, Side::outside(), Side::treasury(o.town, &mut money.0), price / rate, reason("order"), now, day);
        book.settle(id, 0.0, lot, price / rate, 0.0);
        tally.lots_sold += 1.0;
        tally.spurs_from_sales += price;
        filled.lots += 1;
        filled.count += lot;
        filled.spurs += price;
    }
    let c = &book.contracts[id];
    let remaining = c.installments as f64 * lot - c.delivered;
    o.open = (remaining - spare(content, trades, workshop, stock, g)).max(0.0);
    filled
}

/// The buyer couldn't pay for goods it was handed: its miss (`ContractBook.unpaid`).
fn unpaid(content: &Content, book: &mut Book, id: usize, now: u64) {
    let (town, day) = (book.contracts[id].seller, content.day_ticks);
    let c = &mut book.contracts[id];
    c.buyer_missed += 1;
    let broken = c.buyer_missed >= contracts::MISSES_TO_BREAK;
    if broken {
        c.state = contracts::State::Broken;
    }
    book.missed(town, town, 1.0, now, day);
    if broken {
        book.broken(id);
        book.tidy();
    }
}

/// What the desk wants that no market it knows holds, nor a stall here sells, with the premium it will pay
/// (`Broker.signals`): its open orders and its lockers' wants, and what sourcing found no market with a road supplies
/// (`unmet`: a contract's wants, standing in for the war's). One list, a good once.
#[allow(clippy::too_many_arguments)]
fn signals(content: &Content, trades: &Trades, stock: &Stock, open: &BTreeMap<GoodId, f64>, unmet: &[(GoodId, f64)],
        markets: &[TownId], stocktakes: &Stocktakes, stalls: &[Stall], t: TownId, rate: f64) -> Vec<Signal> {
    let mut wants: Vec<(GoodId, f64, &'static str)> = Vec::new();
    let mut want = |g: GoodId, count: f64, why: &'static str| match wants.iter_mut().find(|w| w.0 == g) {
        Some(w) => w.1 += count,
        None => wants.push((g, count, why)),
    };
    for (&g, &count) in open {
        want(g, (count - stock.0[g].floor()).max(0.0), "a player's order");
    }
    for g in 0..content.goods.len() {
        if want_per_day(trades, g) > 0.0 {
            let short = bargain::par(content, want_per_day(trades, g), true) - stock.0[g].floor();
            if short > 0.0 {
                want(g, short, "its lockers' trade");
            }
        }
    }
    let sold_here = |g: GoodId| stalls.iter().any(|s| s.town == t && s.good == g && s.lots() > 0.0);
    let mut out: Vec<Signal> = Vec::new();
    let each = |g: GoodId| (content.goods[g].value * rate * content.dials.signal_premium).ceil().max(1.0);
    for (g, count, why) in wants {
        if count > 0.0 && !(markets.iter().any(|&m| stocktakes.on_hand(m, g) > 0.0) || sold_here(g)) {
            out.push(Signal { good: g, count, spurs_each: each(g), why: why.to_string() });
        }
    }
    // What sourcing found nobody with a road supplies is the same want as the desk's own for that good when it has one
    // (an order is a contract sourcing reads): the larger counts.
    for &(g, count) in unmet {
        if count <= 0.0 || sold_here(g) {
            continue;
        }
        match out.iter_mut().find(|s| s.good == g) {
            Some(s) => s.count = s.count.max(count),
            None => out.push(Signal { good: g, count, spurs_each: each(g), why: UNMET.to_string() }),
        }
    }
    out.sort_by(|a, b| (b.count * b.spurs_each).total_cmp(&(a.count * a.spurs_each)).then(a.good.cmp(&b.good)));
    out
}

/// Buys from a player's selling shop what the desk's orders and lockers want, at up to the goods' worth, and what a
/// signal offers for a good it has posted (`Broker.buy`), as far as the treasury goes beyond the town's reserve.
#[allow(clippy::too_many_arguments)]
fn buy(content: &Content, rate: f64, now: u64, trades: &Trades, farm: Option<&Farm>, ledger: &mut Ledger,
        cash: &mut CashBook, stock: &mut Stock, money: &mut Money, stall: &mut Stall, open: &BTreeMap<GoodId, f64>,
        signalled: &[Signal], tally: &mut Tally) {
    let (g, lot, day) = (stall.good, stall.lot, content.day_ticks);
    let signal = signalled.iter().find(|s| s.good == g);
    let par = if want_per_day(trades, g) > 0.0 { bargain::par(content, want_per_day(trades, g), true) } else { 0.0 };
    let held = stock.0[g].floor();
    let mut want = (open.get(&g).copied().unwrap_or(0.0) + par - held).max(0.0) + signal.map_or(0.0, |s| s.count);
    let ceiling = signal.map_or(worth_of(content, g, lot, rate) * content.dials.maker_ceiling, |s| s.spurs_each * lot);
    let price = stall.price;
    let reserve = reserve(content, trades, farm);
    while want > 0.0 && price > 0.0 && price <= ceiling && stall.lots() > 0.0
        && money.0 - reserve >= price / rate {
        stall.stock -= lot;
        post(ledger, cash, Side::treasury(stall.town, &mut money.0), Side::outside(), price / rate, reason("stall"), now, day);
        stock.0[g] += lot;
        ledger.delivered[g] += lot;
        want -= lot;
        tally.lots_bought += 1.0;
        tally.spurs_on_purchases += price;
    }
}

/// The town's own shops sell its biggest surpluses (beyond par, no food, nothing ordered) a little over their worth,
/// and the Merchant banks their takings (`Broker.sellSurplus`).
#[allow(clippy::too_many_arguments)]
fn sell_surplus(content: &Content, rate: f64, now: u64, trades: &Trades, workshop: &Workshop, ledger: &mut Ledger,
        cash: &mut CashBook, stock: &mut Stock, money: &mut Money, shops: &mut [OwnShop], mine: &[usize],
        open: &BTreeMap<GoodId, f64>, tally: &mut Tally) {
    let day = content.day_ticks;
    for &i in mine {
        let takings = shops[i].takings;
        if takings > 0.0 {
            shops[i].takings = 0.0;
            post(ledger, cash, Side::outside(), Side::treasury(shops[i].town, &mut money.0), takings / rate,
                reason("own shop"), now, day);
            tally.spurs_from_own_shops += takings;
        }
    }
    let mut surplus: Vec<(GoodId, f64)> = Vec::new();
    for g in 0..content.goods.len() {
        let good = &content.goods[g];
        if good.food > 0.0 || open.contains_key(&g) {
            continue;
        }
        let spare = spare(content, trades, workshop, stock, g);
        if spare >= (content.dials.own_lot as f64).min(good.stack as f64) {
            surplus.push((g, spare));
        }
    }
    surplus.sort_by(|a, b| (b.1 * content.goods[b.0].value).total_cmp(&(a.1 * content.goods[a.0].value)));
    for (k, &i) in mine.iter().enumerate() {
        let Some(&(g, spare)) = surplus.get(k) else { break };
        let stack = content.goods[g].stack as f64;
        let lot = (content.dials.own_lot as f64).min(stack);
        let shop = &mut shops[i];
        let price = (worth_of(content, g, lot, rate) * content.dials.own_markup).ceil().max(1.0);
        if shop.good != Some(g) {
            if let Some(old) = shop.good {
                stock.0[old] += shop.shelf; // what's on the shelf of the old listing goes back to the warehouse
            }
            *shop = OwnShop { good: Some(g), lot, price, shelf: 0.0, ..shop.clone() };
        } else if shop.lot != lot || shop.price != price {
            // The same good relisted at another lot or price: its shelf stays.
            shop.lot = lot;
            shop.price = price;
        }
        // Keep a stack's worth on its shelf.
        let top = (stack - shop.shelf).min(spare);
        if top > 0.0 {
            stock.0[g] -= top;
            shop.shelf += top;
        }
    }
}

/// What the markets a town knows have posted as wanted are marbles for its idle hands, by the trade that would supply
/// them, in emeralds (`Jobs.signals`). The one stream into `jobs::Signals.trades`: what was added last time
/// (`Sourcing::signalled`) comes off and what is posted now goes on. Made again when a desk's list changed or sourcing
/// ran this look (its unmet wants moved), as sourcing made them when it was the only poster.
#[allow(clippy::too_many_arguments)]
pub fn post_signals(clock: Res<Clock>, content: Res<Content>, atlas: Res<Atlas>, mut posted: ResMut<Signals>,
        order: Res<Towns>, routes: Query<&Route>, sites: Query<(&Town, Option<&Works>)>,
        rosters: Query<&Town>, mut jobs: ResMut<crate::jobs::Signals>, mut sourcing: ResMut<Sourcing>) {
    if !(posted.changed || sourcing.ran == Some(clock.now)) {
        return;
    }
    if posted.changed {
        posted.changed = false;
    }
    let mut now: BTreeMap<(usize, usize), f64> = BTreeMap::new();
    if !posted.desks.is_empty() {
        // Desk by desk, a good at a time, each town that knows the desk hears of it: the trade that would supply it,
        // at the good's worth and the premium, in emeralds.
        let places = places_of(&content, &sites, &routes);
        let readers: Vec<usize> = order.0.iter().filter_map(|&e| rosters.get(e).ok().map(|t| t.id)).collect();
        for (&desk, list) in &posted.desks {
            let mut list: Vec<&Signal> = list.iter().collect();
            list.sort_by_key(|s| s.good);
            for s in list {
                let Some(trade) = crate::rules::sourcing::supplier_trade(&content, s.good) else { continue };
                for &reader in &readers {
                    if reader != desk && places.known(&content, &atlas, reader).contains(&desk) {
                        *now.entry((reader, trade)).or_insert(0.0) += s.count * content.goods[s.good].value
                            * content.dials.signal_premium;
                    }
                }
            }
        }
    }
    if now == sourcing.signalled {
        return;
    }
    let old = std::mem::replace(&mut sourcing.signalled, now.clone());
    crate::sourcing::post_trades(&mut jobs, &old, &now);
}
