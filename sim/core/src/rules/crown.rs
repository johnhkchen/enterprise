//! The crown's steps (`crown.rs`; `Sovereign.onServerTick`, `Requisitions`, `Caravans.raiseLevy`, `hireMercenaries`,
//! `recruitsOwedAt`, `dispatch`'s run to the front, `arriveAtDestination`, `arriveHome`):
//!
//! - **Review**, once a day while the crown mints or stands mobilised ([`review`]): the day's coin into the treasury
//!   ([`mint`]); tenders for the masters the realm lacks ([`offer_training`]); mobilising or at war, requisitions past
//!   their day close, those done taking bids are awarded, and what the stockpile still lacks for the period is posted
//!   ([`procure`]); at war, mercenaries hired when short of the planned army ([`hire`]) and the front's orders from the
//!   capital ([`front_orders`]). (The army campaigns on the front's stock in its own step: `rules/army.rs`.)
//! - **Settle**, at the far end: recruits brought to the front join the army; at a town levied by the wagon's home, its
//!   hands are handed to the wagon, and at a market off the map (across the sea) mercenaries are ([`raise`]).
//! - **Draft**, at the far end: each end's Merchant reads the other's requisitions and bids ([`read_boards`]).
//! - **Turn**, home: the levy's recruits wait in the capital's barracks ([`muster`]).
//! - **PlanBuy**: a trip to fetch recruits owed can't wait, nor one to the front with its orders or recruits waiting
//!   ([`plan`]); **Decide**, as a wagon leaves for the front: the waiting recruits go with it ([`embark`]).

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;

use super::{in_order, Step};
use crate::army::{self, Army, Recruit};
use crate::cash_book::{post, CashBook, Reason, Side};
use crate::content::{Content, GoodId};
use crate::contracts::{Book, Incoterm, State};
use crate::crown::*;
use crate::mint::Mint;
use crate::realms::{Crown, OffMap};
use crate::roster::Roster;
use crate::sourcing::{route_facts, Env, MetalExchange, Phase, Sourcing};
use crate::state::*;
use crate::training::{Tenders, Training};

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems((
        // (before the tenders lapse: the crown sees its own that lapsed untaken, `offerTraining`)
        review.in_set(Step::Review).before(super::training::lapse),
        // (once the copies have come and the goods owed are delivered: `raiseLevy` after `settleOwing`)
        raise.in_set(Step::Settle).after(super::contracts::settle),
        read_boards.in_set(Step::Draft).after(super::arrive::draft),
        muster.in_set(Step::Turn).before(super::arrive::turn),
        plan.in_set(Step::PlanBuy),
        embark.in_set(Step::Decide).after(super::plan::decide),
    ));
}

/// The capital, if one's named and is a town.
fn capital(world: &World) -> Option<usize> {
    world.resource::<Crown>().capital.filter(|&c| c < world.resource::<Towns>().0.len())
}

/// The crown's day (`Sovereign.onServerTick`): nothing with no capital, or at peace with nothing to mint; once a day
/// from the last.
pub fn review(world: &mut World) {
    let Some(capital) = capital(world) else { return };
    let now = world.resource::<Clock>().now;
    let day = world.resource::<Content>().day_ticks.max(1);
    {
        let sov = world.resource::<Sovereign>();
        if sov.phase == Phase::Peace && sov.mint_per_day == 0 || sov.last_day.is_some_and(|l| now < l + day) {
            return;
        }
    }
    world.resource_mut::<Sovereign>().last_day = Some(now);
    mint(world);
    offer_training(world, capital);
    let phase = world.resource::<Sovereign>().phase;
    if phase == Phase::Peace {
        return; // the crown's revenue only: nothing to stockpile
    }
    procure(world, capital);
    if phase == Phase::War {
        hire(world);
        if world.resource::<Army>().front.is_some() {
            front_orders(world, capital);
        }
    }
}

/// A day's coin into the treasury (`Sovereign.mint`): `mint_per_day` spurs, worth what the spur fetches before
/// they're minted, from outside the society; and the mint debased by them.
pub fn mint(world: &mut World) {
    let Some(capital) = capital(world) else { return };
    let spurs = world.resource::<Sovereign>().mint_per_day;
    if spurs == 0 {
        return;
    }
    let now = world.resource::<Clock>().now;
    let day = world.resource::<Content>().day_ticks.max(1);
    let emeralds = spurs as f64 / world.resource::<Mint>().rate(now, day);
    let e = world.resource::<Towns>().0[capital];
    let mut ledger = std::mem::take(&mut *world.resource_mut::<Ledger>());
    let mut cash = std::mem::take(&mut *world.resource_mut::<CashBook>());
    if let Some(mut money) = world.get_mut::<Money>(e) {
        post(&mut ledger, &mut cash, Side::outside(), Side::treasury(capital, &mut money.0), emeralds,
            Reason::Other("minted".into()), now, day);
    }
    *world.resource_mut::<Ledger>() = ledger;
    *world.resource_mut::<CashBook>() = cash;
    world.resource_mut::<Mint>().mint(spurs, now, day);
    world.resource_mut::<Sovereign>().minted += spurs as f64;
}

/// The capital and the towns its roads reach that keep a roster (the mod's settlements), in the roads' order.
fn realm(world: &mut World, capital: usize) -> Vec<usize> {
    let mut out = vec![capital];
    let towns = world.resource::<Towns>().0.clone();
    for r in route_facts(world) {
        let e = towns[r.route.to_id];
        if r.route.from_id == capital && world.get::<Roster>(e).is_some() && world.get::<OffMap>(e).is_none()
            && !out.contains(&r.route.to_id) {
            out.push(r.route.to_id);
        }
    }
    out
}

/// Does a town's people offer meat for emeralds (a villager's selling offer at its level: `Warehouse.offers`)?
fn sells_any(world: &World, town: Entity, goods: &[GoodId]) -> bool {
    let (Some(roster), Some(lockers)) = (world.get::<Roster>(town), world.get::<crate::lockers::Lockers>(town)) else {
        return false;
    };
    roster.hands.iter().zip(&lockers.hands).any(|(h, l)| l.offers.iter()
        .any(|o| o.sells && o.level <= h.level && goods.contains(&o.good)))
}

/// Masters short in the realm, less those being trained under the crown's contracts or already tendered: posted as
/// tenders on the capital's board, a quarter more for each of the trade that lapsed untaken (`offerTraining`).
pub fn offer_training(world: &mut World, capital: usize) {
    let content = world.resource::<Content>().clone();
    let now = world.resource::<Clock>().now;
    let day = content.day_ticks.max(1);
    let mut tenders = std::mem::take(&mut *world.resource_mut::<Tenders>());
    let mut sov = std::mem::take(&mut *world.resource_mut::<Sovereign>());
    for lapsed in tenders.lapse(&content, now) {
        if lapsed.board == capital {
            *sov.raised.entry(lapsed.work.trade).or_default() += 1;
        }
    }
    let realm = realm(world, capital);
    let towns = world.resource::<Towns>().0.clone();
    let premium = sov.premium(&content.dials);
    let meats = meats(&content);
    let book = world.resource::<Book>();
    for wanted in TRAINED {
        let Some(trade) = content.trade(wanted.trade) else { continue };
        let masters = if wanted.meat {
            realm.iter().filter(|&&t| sells_any(world, towns[t], &meats)).count()
        } else {
            realm.iter().filter_map(|&t| world.get::<Roster>(towns[t]))
                .flat_map(|r| r.hands.iter()).filter(|h| h.trade == trade && h.level >= wanted.level).count()
        };
        let training = book.open.iter().map(|&id| &book.contracts[id])
            .filter(|c| c.buyer == capital && matches!(c.state, State::Drafted | State::Active))
            .filter(|c| c.training.is_some_and(|t| t.trade == trade)).count();
        let tendered = tenders.open_for(capital, trade) as usize;
        let short = wanted.count as i64 - masters as i64 - training as i64 - tendered as i64;
        if short > 0 {
            let raised = sov.raised.get(&trade).copied().unwrap_or(0);
            let bonus = content.dials.training_bonus * wanted.level as f64 / 5.0 * premium
                * crate::training::REPOST_RAISE.powi(raised as i32);
            let work = Training::new(trade, wanted.level, bonus, OFFTAKE);
            tenders.post(capital, capital, work, short as u32, now + TRAINING_DAYS * day, now, 0);
        }
    }
    *world.resource_mut::<Tenders>() = tenders;
    *world.resource_mut::<Sovereign>() = sov;
}

/// Requisitions what the capital still needs of these goods (`need` units: items, or food points if `by_points`),
/// beyond what's under contract and already requisitioned; unanswered a whole bidding window, it's posted as wanted
/// too (`Sovereign.requisition`).
#[allow(clippy::too_many_arguments)]
fn requisition(env: &Env, sov: &mut Sovereign, capital: usize, goods: &[GoodId], by_points: bool, need: f64,
        premium: f64, now: u64) {
    let content = env.content;
    let coming: f64 = goods.iter()
        .map(|&g| env.coming(capital, g) * if by_points { content.goods[g].food } else { 1.0 }).sum();
    let short = need - coming - sov.open(capital, goods);
    if short > 0.0 {
        sov.post(content, capital, goods, by_points, short, premium, now);
    }
    if sov.unanswered(capital, goods) {
        let first = goods[0];
        sov.unmet.push((first, if by_points { (need / content.goods[first].food.max(1.0)).ceil() } else { need }));
    }
}

/// The procurement agent's day (`onServerTick`): requisitions past their day close, those done taking bids are
/// awarded to the best offers, and what's still wanted for the period is posted, the meat as one of a kind.
pub fn procure(world: &mut World, capital: usize) {
    let content = world.resource::<Content>().clone();
    let now = world.resource::<Clock>().now;
    let mut sov = std::mem::take(&mut *world.resource_mut::<Sovereign>());
    sov.unmet.clear();
    sov.shortfall.clear();
    let premium = sov.premium(&content.dials);
    sov.lapse(now);
    let routes = route_facts(world);
    let awards = award(&Env::gather(world, routes), &mut sov.requisitions, capital, sov.theatre);
    sov.awarded += awards.len() as u32;
    apply_awards(world, &awards);
    // What it wants on hand, less what its depot holds.
    let towns = world.resource::<Towns>().0.clone();
    let depot = world.get::<Stock>(towns[capital]).map_or_else(Vec::new, |s| s.0.clone());
    let held = |g: GoodId| depot.get(g).copied().unwrap_or(0.0).floor();
    let army = world.resource::<Army>().clone();
    let ration_days = ration_days_of(world, &content, &army, capital);
    let routes = route_facts(world);
    let env = Env::gather(world, routes);
    for (g, n) in targets(&content, sov.phase, &army, ration_days) {
        let need = n as f64 - held(g);
        if need <= 0.0 {
            continue;
        }
        sov.shortfall.push((g, need)); // its own villagers sell to the depot too
        requisition(&env, &mut sov, capital, &[g], false, need, premium, now);
    }
    let meats = meats(&content);
    let points = content.dials.meat_points - meats.iter().map(|&m| held(m) * content.goods[m].food).sum::<f64>();
    if points > 0.0 && !meats.is_empty() {
        sov.shortfall.push((meats[0], (points / content.dials.army_ration.max(1.0)).ceil()));
        requisition(&env, &mut sov, capital, &meats, true, points, premium, now);
    }
    *world.resource_mut::<Sovereign>() = sov;
}

/// Days of rations the front keeps, by its distance from the capital (`Sovereign.rationDays`).
fn ration_days_of(world: &World, content: &Content, army: &Army, capital: usize) -> u32 {
    let towns = &world.resource::<Towns>().0;
    let hex = |t: usize| towns.get(t).and_then(|&e| world.get::<Town>(e)).and_then(|t| t.hex);
    let blocks = army.front.map_or(0.0, |f| army::blocks_between(hex(capital), hex(f)));
    army::ration_days(content, blocks)
}

/// Recruits riding with the wagons (`CaravanLedger.recruitsOnTheRoad`).
fn on_the_road(world: &mut World) -> usize {
    let mut q = world.query::<&RouteState>();
    q.iter(world).filter_map(|s| s.wagon.as_ref()).map(|w| w.recruits.len()).sum()
}

/// Short of its planned army (counting soldiers serving, recruits waiting or on the road, and those levied and not
/// yet raised), the crown hires mercenaries at the market across the sea: one contract open at a time, at most
/// `mercenaries_most`, its copy riding with the capital's wagon (`Sovereign.hireMercenaries`). Returns those hired.
pub fn hire(world: &mut World) -> u32 {
    let Some(capital) = capital(world) else { return 0 };
    let towns = world.resource::<Towns>().0.clone();
    let Some(sea) = world.resource::<MetalExchange>().overseas
        .filter(|&s| towns.get(s).is_some_and(|&e| world.get::<OffMap>(e).is_some())) else { return 0 };
    let (mut coming, mut hiring) = (0u32, false);
    {
        let book = world.resource::<Book>();
        for c in book.open.iter().map(|&id| &book.contracts[id]) {
            let Some(levy) = c.levy else { continue };
            if c.buyer == capital && matches!(c.state, State::Drafted | State::Active) {
                coming += levy.owed();
                hiring |= levy.owed() > 0 && c.seller == sea;
            }
        }
    }
    let road = on_the_road(world);
    let army = world.resource::<Army>();
    let strength = army.size() + army.waiting() + coming as usize + road;
    let d = &world.resource::<Content>().dials;
    let short = d.planned_army as i64 - strength as i64;
    if short <= 0 || hiring {
        return 0;
    }
    let hired = (short as u32).min(d.mercenaries_most);
    let pay = d.mercenary_pay;
    let now = world.resource::<Clock>().now;
    let day = world.resource::<Content>().day_ticks.max(1);
    let id = draft_levy(&mut world.resource_mut::<Book>(), sea, capital, hired, pay, now, day, false);
    world.resource_mut::<Sourcing>().copies.insert(id);
    world.resource_mut::<Sovereign>().hired += hired;
    hired
}

/// The front's quartermaster (`onServerTick`'s front orders): what the army will want at the front, beyond what it
/// holds and what's on its way, ordered from the capital's spare stock: rations in food points, food heartier than
/// bread that the capital holds first, bread the rest; a weapon and a piece of armour for each place of any kind, the
/// best the capital holds first, then what's wanted; shields and arrows. Each order the crown's own: in force at once,
/// at no price, delivered by the capital's wagons when the next can be there.
pub fn front_orders(world: &mut World, capital: usize) {
    let content = world.resource::<Content>().clone();
    let now = world.resource::<Clock>().now;
    let day = content.day_ticks.max(1);
    let army = world.resource::<Army>().clone();
    let Some(front) = army.front.filter(|&f| f != capital && f < world.resource::<Towns>().0.len()) else { return };
    let towns = world.resource::<Towns>().0.clone();
    let lines = world.get::<Stock>(towns[front]).map_or_else(Vec::new, |s| s.0.clone());
    let depot = Stock(world.get::<Stock>(towns[capital]).map_or_else(Vec::new, |s| s.0.clone()));
    let at_front = |g: GoodId| lines.get(g).copied().unwrap_or(0.0).floor();
    let ration_days = ration_days_of(world, &content, &army, capital);
    let routes = route_facts(world);
    let env = Env::gather(world, routes);
    let on_hand = |g: GoodId| env.stocktakes.on_hand(capital, g).floor();
    let mut coming: BTreeMap<GoodId, f64> = BTreeMap::new();
    let mut orders: Vec<(GoodId, f64, u64)> = Vec::new();
    // What's coming to the front of a good, with the orders drafted today.
    let mut ensure = |coming: &mut BTreeMap<GoodId, f64>, good: GoodId, need: f64| {
        let covered = *coming.entry(good).or_insert_with(|| env.coming(front, good));
        if covered >= need {
            return;
        }
        let Some(m) = env.means(front, capital, good) else { return };
        let quantity = (need - covered).min(m.spare).floor();
        if quantity <= 0.0 {
            return;
        }
        // Delivered at the front, due once the capital's next wagon can be there.
        let r = &env.routes[m.route];
        let home = match r.wagon {
            Some((Leg::Out, arrives)) => arrives + r.route.travel_ticks,
            Some((Leg::Home, arrives)) => arrives,
            None => now,
        };
        let start = now.max(home).max(r.last_departure.map_or(0, |t| t + day)) + r.route.travel_ticks;
        orders.push((good, quantity, start));
        *coming.get_mut(&good).unwrap() += quantity;
    };
    let bread = army::good_named(&content, army::BREAD);
    for (want, n) in army.wants(&content, ration_days) {
        if Some(want) == bread {
            // Rations, in food points: any food the lines hold counts.
            let bread_food = content.goods[want].food.max(1e-9);
            let held_points: f64 = (0..lines.len()).map(|g| at_front(g) * content.goods[g].food).sum();
            let mut points = n as f64 * bread_food - held_points;
            for food in army::heartiest_first(&content, &depot) {
                if points <= 0.0 || army::heartiness(&content.goods[food]) < HEARTY_ENOUGH {
                    break;
                }
                let nut = content.goods[food].food;
                let was = *coming.entry(food).or_insert_with(|| env.coming(front, food));
                let n = (points / nut).ceil().min(was + on_hand(food));
                if n > 0.0 {
                    ensure(&mut coming, food, n);
                    points -= n.min(coming[&food]) * nut;
                }
            }
            if points > 0.0 {
                ensure(&mut coming, want, (points / bread_food).ceil());
            }
            continue;
        }
        let name = army::bare(&content.goods[want].name);
        let kinds: Vec<GoodId> = if name == army::WEAPONS[1].name {
            army::WEAPONS.iter().filter(|g| g.name != army::BOW).filter_map(|g| army::good_named(&content, g.name)).collect()
        } else {
            army::ARMOUR.iter().find(|place| place.iter().any(|g| g.name == name))
                .map_or_else(Vec::new, |place| place.iter().filter_map(|g| army::good_named(&content, g.name)).collect())
        };
        if !kinds.is_empty() {
            let mut need = n as f64 - kinds.iter().map(|&g| at_front(g)).sum::<f64>();
            for &kind in &kinds {
                let held = on_hand(kind);
                if need > 0.0 && held > 0.0 && kind != want {
                    ensure(&mut coming, kind, need.min(held));
                    need -= need.min(held);
                }
            }
            if need > 0.0 {
                ensure(&mut coming, want, need);
            }
            continue;
        }
        let need = n as f64 - at_front(want);
        if need > 0.0 {
            ensure(&mut coming, want, need);
        }
    }
    drop(env);
    let mut book = world.resource_mut::<Book>();
    for (good, quantity, start) in orders {
        book.draft(capital, front, good, quantity, 0.0, start, day, 1, Incoterm::Dap, false, true);
    }
}

/// At the far end: recruits the wagon brought to the front join the army there; at a town its home levied, the town
/// hands over its recruits, idle hands first (`raiseLevy`); at a market off the map, its mercenaries (`hireMercenaries`).
/// Raised, a soldier counts as delivered under the levy.
#[allow(clippy::too_many_arguments)]
pub fn raise(clock: Res<Clock>, content: Res<Content>, mut book: ResMut<Book>, mut army: ResMut<Army>,
        mut routes: Query<(Entity, &Route, &mut RouteState)>, mut rosters: Query<&mut Roster>, off: Query<(), With<OffMap>>) {
    let levied = |c: &crate::contracts::Contract| c.in_force() && c.levy.is_some_and(|l| l.owed() > 0);
    if army.front.is_none() && !book.open.iter().any(|&id| levied(&book.contracts[id])) {
        return;
    }
    let now = clock.now;
    let trained = trained_trades(&content);
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        if state.arrived != Some(Leg::Out) {
            continue;
        }
        let route = *route;
        let Some(wagon) = state.wagon.as_mut() else { continue };
        if army.front == Some(route.to_id) {
            for recruit in std::mem::take(&mut wagon.recruits) {
                army.enlist(recruit, now);
            }
        }
        let levies: Vec<usize> = book.open.iter().copied().filter(|&id| {
            let c = &book.contracts[id];
            levied(c) && c.seller == route.to_id && c.buyer == route.from_id
        }).collect();
        if levies.is_empty() {
            continue;
        }
        let abroad = off.get(route.to).is_ok();
        let mut needed = trained.clone();
        needed.extend(book.training_for(route.to_id).map(|(_, t)| t.trade));
        for id in levies {
            let levy = book.contracts[id].levy.unwrap();
            let raised = if abroad {
                // Mercenaries, as many as are owed, ready to go: their home that market.
                for _ in 0..levy.owed() {
                    wagon.recruits.push(Recruit { home: route.to_id, hand: None, mustered: now, levy: Some(id),
                        pay: levy.pay_per_day });
                }
                levy.owed()
            } else {
                let Ok(mut roster) = rosters.get_mut(route.to) else { continue };
                let picked = levy_pick(&content, &roster, &needed, levy.owed() as usize);
                let mut k = 0;
                for i in picked {
                    if let Some(hand) = crate::squad::stamp(&mut roster, i) {
                        wagon.recruits.push(Recruit { home: route.to_id, hand: Some(hand), mustered: now, levy: Some(id),
                            pay: levy.pay_per_day });
                        k += 1;
                    }
                }
                k
            };
            book.contracts[id].levy.as_mut().unwrap().raised += raised;
        }
    }
}

/// Home again: the recruits it brought wait in the capital's barracks for a wagon to the front (`arriveHome`).
pub fn muster(clock: Res<Clock>, mut army: ResMut<Army>, mut routes: Query<(Entity, &Route, &mut RouteState)>) {
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, _, mut state) = routes.get_mut(entity).unwrap();
        if state.arrived != Some(Leg::Home) {
            continue;
        }
        let Some(wagon) = state.wagon.as_mut() else { continue };
        if wagon.recruits.is_empty() {
            continue;
        }
        for recruit in std::mem::take(&mut wagon.recruits) {
            army.barracks.push(Recruit { mustered: clock.now, ..recruit });
        }
    }
}

/// In the plan: a levy (or a hire) at the far end that still owes the wagon's home soldiers is business that can't
/// wait (`recruitsOwedAt`); and a wagon bound for the front goes with whatever it's taking or the recruits waiting
/// (`dispatch`'s "to the front": no waiting for a full load, no empty trips).
pub fn plan(book: Res<Book>, army: Res<Army>, mut routes: Query<(&Route, &mut RouteState)>) {
    let levies: Vec<(usize, usize)> = book.open.iter().map(|&id| &book.contracts[id])
        .filter(|c| c.in_force() && c.levy.is_some_and(|l| l.owed() > 0)).map(|c| (c.seller, c.buyer)).collect();
    if levies.is_empty() && army.front.is_none() {
        return;
    }
    for (route, mut state) in &mut routes {
        let Some(plan) = state.plan.as_mut() else { continue };
        if levies.contains(&(route.to_id, route.from_id)) {
            plan.pressing = true;
        }
        if army.front == Some(route.to_id) && (!plan.contracted.is_empty() || army.waiting() > 0) {
            plan.pressing = true;
        }
    }
}

/// A wagon leaving for the front takes the recruits waiting in the barracks (`dispatch`: `army.embark`).
pub fn embark(clock: Res<Clock>, mut army: ResMut<Army>, mut routes: Query<(Entity, &Route, &mut RouteState)>) {
    let Some(front) = army.front else { return };
    if army.barracks.is_empty() {
        return;
    }
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        if route.to_id != front || state.last_departure != Some(clock.now) {
            continue;
        }
        let Some(wagon) = state.wagon.as_mut().filter(|w| w.leg == Leg::Out && w.arrives > clock.now) else { continue };
        let going = army.embark(usize::MAX);
        wagon.recruits.extend(going);
    }
}

/// At the far end, each end's Merchant reads the other's requisitions and bids what its town can supply
/// (`arriveAtDestination`: `requisitions.read(to, from)`, then `read(from, to)`).
pub fn read_boards(world: &mut World) {
    if world.resource::<Sovereign>().requisitions.iter().all(|r| r.open <= 0.0 || r.awarded) {
        return;
    }
    let mut q = world.query::<(&Route, &RouteState)>();
    let mut arrived: Vec<(usize, usize, usize)> = q.iter(world).filter(|(_, s)| s.arrived == Some(Leg::Out))
        .map(|(r, _)| (r.seq, r.from_id, r.to_id)).collect();
    if arrived.is_empty() {
        return;
    }
    arrived.sort();
    let mut reqs = std::mem::take(&mut world.resource_mut::<Sovereign>().requisitions);
    let routes = route_facts(world);
    {
        let env = Env::gather(world, routes);
        for &(_, from, to) in &arrived {
            read(&env, &mut reqs, to, from);
            read(&env, &mut reqs, from, to);
        }
    }
    world.resource_mut::<Sovereign>().requisitions = reqs;
}
