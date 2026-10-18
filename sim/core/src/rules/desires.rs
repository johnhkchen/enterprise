//! Desires and backhauls through the look (`desires.rs`; `Caravans.dispatch`, `arriveAtDestination`, `sell`;
//! `ShopBoard.keep`):
//!
//! - **Review:** a town keeps its shop, its people having lived the time since: what it's after, at what it would
//!   pay ([`keep_shops`]; the mod's `Territory.touch` calls `ShopBoard.keep` at its end).
//! - **Settle:** a wagon calling at the far end reads its shop: the prices it lists are news the Merchant carries home
//!   ([`sight`], `ShopBoard.sighted`). (`Sell`, in `arrive.rs`, then pays by desire and buys for resale.)
//! - **PlanBuy:** a town sending its own wagon puts money in its purse to bring home the comforts it lacks, when it's
//!   worth twice the road food and the Accountant allows, before the bank's float ([`plan_shopping`], `dispatch`
//!   326–337).
//! - **Decide:** a wagon gone, its shopping money is counted spent off its town's day's budget (in `plan::decide`:
//!   the Accountant allowed it at the plan).
//! - **Buy**, in turn after the sale, keeping the road home's food money ([`buy_for_home`], [`buy_surplus`],
//!   [`buy_backhaul`]): the town's shopping, whole items of what the far end holds beyond its own keep, cheapest
//!   first, at a price bargained up to full worth; the far end's surplus, only goods another town on the Merchant's
//!   routes wants (or a comfort it can sell on at a margin), at most the resale ceiling of its worth, never below the
//!   far end's 320 food points; and a glut the Merchant's home can use, at a discount for each par it holds beyond.

use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::prelude::*;

use super::farms::reserve;
use super::{free, in_order, Step};
use crate::atlas::{keeps, Books, Entry, Kind, Stocktakes, Whys};
use crate::bills::Bills;
use crate::cash_book::{post, Accountant, CashBook, Reason, Side};
use crate::content::Content;
use crate::desires::*;
use crate::farms::Farm;
use crate::people::{comfort_keep, comfort_par};
use crate::realms::{OffMap, Realm};
use crate::roster::Roster;
use crate::fixed::Fixed;
use crate::state::*;
use crate::works::Works;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems((
        // (last of the reviews, as the mod's `Territory.touch` keeps the shop at its end)
        keep_shops.in_set(Step::Review).after(super::works::review).after(super::chokepoints::review)
            .after(super::jobs::review).after(super::atlas::consider)
            // (before the hubs' review, as main ran them before sourcing's review joined the step)
            .before(super::freight::review),
        sight.in_set(Step::Settle),
        plan_shopping.in_set(Step::PlanBuy).before(super::bank::plan_float),
        // (porters gone home first, as the mod's `sendPortersHome` before the buying: the mouths the road home feeds)
        buy_for_home.in_set(Step::Buy).after(super::squad::send_porters_home),
        buy_surplus.in_set(Step::Buy).after(buy_for_home),
        buy_backhaul.in_set(Step::Buy).after(buy_surplus),
    ));
}

/// A town's facts from its components (`Desires.reckon`'s inputs).
pub(crate) fn facts_of<'a>(content: &Content, stock: &'a Stock, money: &Money, trades: &Trades, people: &'a People,
        farm: Option<&Farm>) -> Facts<'a> {
    Facts {
        villagers: people.villagers,
        par: comfort_par(content, people),
        met: &people.comfort_met,
        purse: people.purse.to_f64(),
        spare: (money.0 - reserve(content, trades, farm)).to_f64(),
        stock: &stock.0,
    }
}

fn reason(what: &str) -> Reason {
    Reason::Other(what.to_string())
}

/// The routes as the Merchants see them, in the routes' order.
pub(crate) fn infos(routes: &Query<(Entity, &Route, &mut RouteState)>) -> Vec<RouteInfo> {
    let mut all: Vec<(usize, RouteInfo)> = routes.iter().map(|(_, r, s)| (r.seq, RouteInfo::of(r, s))).collect();
    all.sort_by_key(|(seq, _)| *seq);
    all.into_iter().map(|(_, i)| i).collect()
}

/// Each town keeps its shop (`ShopBoard.keep`): what it's after and holds none of, at what it would pay, while it has a
/// road for it and money for a lot; a town with no people, or no road, lists nothing.
#[allow(clippy::type_complexity)]
pub fn keep_shops(clock: Res<Clock>, content: Res<Content>, order: Res<Towns>, mut listings: ResMut<Listings>,
        routes: Query<&Route>,
        towns: Query<(&Town, &Money, &Stock, &Trades, &People, Option<&Farm>)>) {
    let roads: BTreeSet<usize> = routes.iter().flat_map(|r| [r.from_id, r.to_id]).collect();
    for &e in &order.0 {
        let Ok((town, money, stock, trades, people, farm)) = towns.get(e) else { continue };
        let mut shop = Vec::new();
        if people.villagers > 0.0 && roads.contains(&town.id) {
            let facts = facts_of(&content, stock, money, trades, people, farm);
            let cash = facts.spare.max(0.0) + facts.purse;
            shop = shop_wants(&content, &facts, trades, cash, true, |g| reckon(&content, g, &facts, None, clock.now));
        }
        if shop.is_empty() {
            listings.shops.remove(&town.id);
        } else if listings.shops.get(&town.id) != Some(&shop) {
            listings.shops.insert(town.id, shop);
        }
    }
}

/// A wagon at the far end reads the shop there (`ShopBoard.sighted`).
pub fn sight(mut listings: ResMut<Listings>, routes: Query<(Entity, &Route, &RouteState)>) {
    for e in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, state) = routes.get(e).unwrap();
        if state.arrived == Some(Leg::Out) {
            listings.sighted(route.to_id);
        }
    }
}

/// A town sending its own wagon gives it money to bring home the comforts it lacks (`Caravans.dispatch` 326–337): the
/// kinds it holds less than its par of, as much as it can spare beyond its reserve, the road and what every plan from
/// it carries already, if that's worth twice the road food (and at least half an emerald), as far as its Accountant
/// allows today. The money goes in the purse and counts toward the trip's worth.
#[allow(clippy::type_complexity)]
pub fn plan_shopping(clock: Res<Clock>, content: Res<Content>, bills: Res<Bills>, stocktakes: Res<Stocktakes>,
        mut routes: Query<(Entity, &Route, &mut RouteState)>, realms: Query<&Realm>,
        towns: Query<(&Money, &Stock, &Trades, &Budget, &People, Option<&Roster>, Option<&Farm>)>) {
    let now = clock.now;
    // What every plan from a town carries and its road costs this look.
    let mut committed: BTreeMap<usize, Fixed> = BTreeMap::new();
    // (In the routes' order: a sum's last bit mustn't hang on archetypes, which a load reorders.)
    for e in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, state) = routes.get(e).unwrap();
        let Some(plan) = state.plan.as_ref() else { continue };
        let Ok((_, stock, trades, _, _, roster, _)) = towns.get(route.from) else { continue };
        let keep_wheat = crate::road::keep_wheat(&content, trades, roster);
        *committed.entry(route.from_id).or_default() +=
            plan.purse + crate::road::road_money(&content, route, &stock.0, keep_wheat, plan.crew.len().max(1) as f64);
    }
    let mut budgets: BTreeMap<usize, Budget> = BTreeMap::new();
    let mut planned: BTreeMap<usize, Fixed> = BTreeMap::new();
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        let route = *route;
        let Some(plan) = state.plan.as_mut() else { continue };
        // (A realm's wagons shop for nobody.)
        if realms.get(route.from).is_ok() {
            continue;
        }
        let Ok((money, stock, trades, budget, people, roster, farm)) = towns.get(route.from) else { continue };
        let list = shopping_list(&content, comfort_par(&content, people), &stock.0);
        let wanted: f64 = list.iter().sum();
        if wanted <= 0.0 {
            continue;
        }
        // Is there anything to shop for there (`hasShoppingAt`): the far end's last stocktake holds some of a kind on the
        // list (a home with a roster reads it while it's fresh; else the far end as it stands).
        let seen = if crate::squad::crewed(roster) { stocktakes.fresh(&content, route.to_id, now) } else { None };
        let there = match (seen, towns.get(route.to)) {
            (Some(t), _) => held_goods(&content, &t.stock, &list),
            (None, Ok((_, far, ..))) => held_goods(&content, &far.0, &list),
            _ => false,
        };
        // Worth twice the road food: a round trip's, or the road's cost where content has no food.
        let mouths = plan.crew.len().max(1) as f64;
        let least = if crate::road::road_food(&content) {
            Fixed::ceil(2.0 * crate::road::food_needed(&content, route.blocks) * mouths * content.dials.purse_per_food_point)
        } else {
            route.road_cost
        }
        .max(Fixed::ceil(SHOPPING_LEAST));
        let already = planned.get(&route.from_id).copied().unwrap_or_default();
        let spare = (money.0 - reserve(&content, trades, farm) - committed.get(&route.from_id).copied().unwrap_or_default()
            - already).at_least_zero();
        let mut shopping = spare.min(Fixed::floor(wanted));
        if shopping >= least {
            let budget = budgets.entry(route.from_id).or_insert(*budget);
            let free_money = free(&content, trades, money, &bills, route.from_id, now);
            shopping = shopping.min(Accountant::left(&content, now, free_money, budget));
            if shopping >= least {
                budget.spent += shopping;
            }
        }
        if shopping < least {
            continue;
        }
        *planned.entry(route.from_id).or_default() += shopping;
        plan.shopping = list;
        plan.shopping_money = shopping;
        plan.purse += shopping;
        // (A trip with nothing there to shop for isn't worth the road for the money alone: no empty trips.)
        if there {
            plan.worth += shopping.to_f64();
        }
    }
}

/// Does a market hold some of a good of a kind on the shopping list?
fn held_goods(content: &Content, stock: &[f64], list: &[f64]) -> bool {
    (0..content.goods.len()).any(|g| content.goods[g].comfort.is_some_and(|k| list[k] > 0.0)
        && stock.get(g).copied().unwrap_or(0.0) > 0.0)
}

/// A wagon gone with shopping money: counted spent off its town's day's budget (the Accountant allowed it at the plan).
pub fn spend(content: &Content, now: u64, free_money: Fixed, budget: &mut Budget, amount: Fixed) {
    if amount.is_positive() {
        super::left_for(content, now, free_money, budget, 1.0); // (the day's budget is set before it's spent from)
        budget.spent += amount;
    }
}

/// What a far end's Merchant and the wagon's keep of the wagon's purse for the road home's food (`keepForFood`): none
/// where it packed its own.
fn keep_for_food(content: &Content, route: &Route, wagon: &Wagon) -> Fixed {
    let mouths = crate::road::mouths(Some(wagon));
    if wagon.provisions >= crate::road::food_needed(content, route.blocks) * mouths && crate::road::road_food(content) {
        Fixed::ZERO
    } else {
        crate::road::purse_keep(content, route, mouths)
    }
}

/// The price two Merchants settle between a floor and a ceiling, the seller at one level and the buyer at another;
/// with bargaining off, the ceiling.
fn deal(content: &Content, floor: f64, ceiling: f64, seller: u8, buyer: u8) -> Option<f64> {
    if content.dials.bargain {
        crate::bargain::between(floor, ceiling, seller, buyer)
    } else {
        Some(ceiling)
    }
}

/// What the wagon paid the far end for goods, the wagon's log and the two Merchants' books of it
/// (`Caravans.boughtThere`): the far end's Merchant sold, the wagon's log says bought.
#[allow(clippy::too_many_arguments)]
fn bought_there(content: &Content, now: u64, books: &mut Books, whys: &mut Whys, route: &Route, far_keeps: u64,
        wagon: &mut Wagon, good: usize, count: f64, paid: Fixed) {
    let today = now / content.day_ticks.max(1);
    books.enter(whys, route.to_id, today, far_keeps, Kind::Sold, route.from_id, good, count, paid);
    wagon.log.push(Entry::new(today, Kind::Bought, route.to_id, good, count, paid));
}

type BuyTown<'a> = (&'a mut Stock, &'a mut Money, &'a Trades, &'a People, Option<&'a Roster>, Option<&'a Works>);

/// The far end sells no food and keeps what it has for the army: nothing is bought there (`OffMap.sells`).
fn doesnt_sell(offmap: &Query<&OffMap>, route: &Route) -> bool {
    offmap.get(route.to).is_ok_and(|o| !o.sells_food)
}

/// The town's own shopping, at the far end (`Caravans.buyForHome`): comforts the far end holds beyond its own keep,
/// cheapest first, up to the worth on the list, whole items at a price bargained up to their full value (it's for home,
/// not for resale), keeping the road home's food money and within the wagon's space.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn buy_for_home(clock: Res<Clock>, content: Res<Content>, mut ledger: ResMut<Ledger>, mut cash: ResMut<CashBook>,
        mut books: ResMut<Books>, mut whys: ResMut<Whys>, mut routes: Query<(Entity, &Route, &mut RouteState)>,
        mut towns: Query<BuyTown>, rosters: Query<&Roster>, realms: Query<&Realm>, offmap: Query<&OffMap>) {
    let (now, day) = (clock.now, content.day_ticks);
    let n = content.goods.len();
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        let route = *route;
        if state.arrived != Some(Leg::Out) {
            continue;
        }
        let state = &mut *state;
        let Some(wagon) = state.wagon.as_mut() else { continue };
        if wagon.shopping.iter().all(|w| *w <= 0.0) || doesnt_sell(&offmap, &route) {
            continue;
        }
        // The travelling Merchant is its home's Merchant; the seller, the far end's.
        let merchant = crate::realms::level(&content, realms.get(route.from).ok(), rosters.get(route.from).ok());
        let resident = crate::realms::level(&content, realms.get(route.to).ok(), rosters.get(route.to).ok());
        let keep = keep_for_food(&content, &route, wagon);
        let slots = if wagon.slots > 0 { wagon.slots } else { content.dials.wagon_slots };
        let (mut stock, mut money, _, people, roster, works) = towns.get_mut(route.to).expect("the far end");
        let far_keeps = keeps(&content, roster, works);
        let their_keep = comfort_keep(&content, people);
        let shopping = std::mem::take(&mut wagon.shopping);
        for (kind, &want) in shopping.iter().enumerate() {
            let mut budget = want.min(held_in(&content, &stock.0, kind) - their_keep);
            let mut goods: Vec<usize> = (0..n).filter(|&g| content.goods[g].comfort == Some(kind)).collect();
            goods.sort_by(|&a, &b| content.goods[a].value.total_cmp(&content.goods[b].value));
            for g in goods {
                if budget <= 0.0 || crate::cargo::stacks(&content, &wagon.cargo, &wagon.shipments) >= slots {
                    break;
                }
                let value = content.goods[g].value;
                let Some(each) = deal(&content, crate::bargain::seller_floor(value), value, resident, merchant) else { continue };
                let quantity = (budget / value).ceil().min(((wagon.purse - keep).to_f64() / each).floor());
                let taken = quantity.min(stock.0[g].floor().max(0.0));
                if taken <= 0.0 {
                    continue;
                }
                // Rounded up to the thousandth, as the mod's `Math.ceil(taken * each * rate)`.
                let paid = Fixed::ceil(taken * each).min((wagon.purse - keep).at_least_zero());
                stock.0[g] -= taken;
                wagon.cargo[g] += taken;
                post(&mut ledger, &mut cash, Side::wagon(route.seq, &mut wagon.purse),
                    Side::treasury(route.to_id, &mut money.0), paid, reason("shopping"), now, day);
                budget -= taken * value;
                state.bought += taken;
                state.purchases += paid;
                bought_there(&content, now, &mut books, &mut whys, &route, far_keeps, wagon, g, taken, paid);
            }
        }
    }
}

/// The routes' far ends as the Merchants see them, read from the towns' components (`Desires.outlet`'s markets).
#[allow(clippy::type_complexity)]
fn market<'a>(content: &Content, towns: &'a Query<BuyTown>, farms: &'a Query<&mut Farm>, e: Entity) -> Option<Facts<'a>> {
    let (stock, money, trades, people, _, _) = towns.get(e).ok()?;
    Some(facts_of(content, stock, money, trades, people, farms.get(e).ok()))
}

/// The far end's surplus, bought for the trip home (`Caravans.buySurplus`), but only goods another town on the home's
/// routes wants and the home lacks (`networkWants`: the sum of their pars, less the home's stock at its last stocktake
/// and what's aboard), or a comfort the towns it supplies are short of, where they'd pay more than this costs and the
/// road: goods above the far end's par (and food beyond the 320 points it keeps for wagons), at the price the two
/// Merchants settle, the travelling Merchant paying at most the resale ceiling of the worth since it has to resell.
/// Keeps the road home's food money and stays within its space. Crop sales count toward the town's farmers' share.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn buy_surplus(clock: Res<Clock>, content: Res<Content>, stocktakes: Res<Stocktakes>, listings: Res<Listings>,
        mut ledger: ResMut<Ledger>, mut cash: ResMut<CashBook>, mut books: ResMut<Books>, mut whys: ResMut<Whys>,
        mut routes: Query<(Entity, &Route, &mut RouteState)>, mut towns: Query<BuyTown>, mut farms: Query<&mut Farm>,
        rosters: Query<&Roster>, realms: Query<&Realm>, offmap: Query<&OffMap>, orders: Res<crate::orders::Orders>) {
    let (now, day) = (clock.now, content.day_ticks);
    let n = content.goods.len();
    let all = infos(&routes);
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        let route = *route;
        if state.arrived != Some(Leg::Out) || state.wagon.is_none() || doesnt_sell(&offmap, &route) {
            continue;
        }
        let state = &mut *state;
        let wagon = state.wagon.as_mut().unwrap();
        // What the home's other routes' far ends want of each good, and where a comfort sells on.
        let mut network = vec![0.0; n];
        let mut outlets: Vec<Option<Outlet>> = vec![None; n];
        {
            let known = Knowledge { content: &content, now, stocktakes: &stocktakes, books: &books, listings: &listings };
            for other in all.iter().filter(|o| o.from == route.from_id && o.to != route.to_id) {
                let Ok((_, _, trades, ..)) = towns.get(other.to_entity) else { continue };
                for (g, want) in network.iter_mut().enumerate() {
                    let per_day = want_per_day(trades, g);
                    if per_day > 0.0 {
                        *want += crate::bargain::par(&content, per_day, true);
                    }
                }
            }
            for g in 0..n {
                let aboard = wagon.cargo[g] + wagon.shipments.iter().filter(|s| s.good == g).map(|s| s.count).sum::<f64>();
                // And what players' shops at home have ordered: the caravan buys it up on its way (`networkWants`).
                network[g] += orders.wanted_of(route.from_id, g);
                network[g] = (network[g] - stocktakes.on_hand(route.from_id, g) - aboard).max(0.0);
                outlets[g] = known.outlet(&all, route.from_id, Some(route.to_id), g, &|e| market(&content, &towns, &farms, e));
            }
        }
        let merchant = crate::realms::level(&content, realms.get(route.from).ok(), rosters.get(route.from).ok());
        let resident = crate::realms::level(&content, realms.get(route.to).ok(), rosters.get(route.to).ok());
        let keep = keep_for_food(&content, &route, wagon);
        let slots = if wagon.slots > 0 { wagon.slots } else { content.dials.wagon_slots };
        let (mut stock, mut money, trades, people, roster, works) = towns.get_mut(route.to).expect("the far end");
        let far_keeps = keeps(&content, roster, works);
        let mut spare_food = crate::road::held(&content, &stock.0) - crate::road::FOOD_PAR_POINTS;
        for g in 0..n {
            let stacks = crate::cargo::stacks(&content, &wagon.cargo, &wagon.shipments);
            if stacks >= slots {
                break;
            }
            let held = stock.0[g].floor().max(0.0);
            if held <= 0.0 {
                continue;
            }
            let value = content.goods[g].value;
            let per_day = want_per_day(trades, g);
            let mut wanted = network[g];
            let mut their_par = crate::bargain::par(&content, per_day, per_day > 0.0);
            let outlet = outlets[g];
            if let Some(o) = outlet {
                // Comforts the towns it supplies are short of: they keep their own.
                let aboard = wagon.cargo[g] + wagon.shipments.iter().filter(|s| s.good == g).map(|s| s.count).sum::<f64>();
                wanted = wanted.max(o.units - stocktakes.on_hand(route.from_id, g) - aboard);
                their_par = (comfort_keep(&content, people) / value.max(1e-9)).ceil();
            }
            let mut surplus = (held - their_par).min(wanted);
            let points = crate::road::points(&content, g);
            if points > 0.0 {
                surplus = surplus.min((spare_food.max(0.0) / points).floor());
            }
            let room = (slots - stacks) as f64 * content.goods[g].stack as f64;
            let Some(each) = deal(&content, crate::bargain::seller_floor(value), value * content.dials.resale_ceiling,
                resident, merchant) else { continue };
            let mut quantity = surplus.min(room).floor();
            if outlet.is_some_and(|o| !worth_buying(&content, each, &o)) {
                continue; // no margin in it
            }
            if each > 0.0 {
                quantity = quantity.min(((wagon.purse - keep).to_f64() / each).floor());
            }
            if each <= 0.0 || quantity <= 0.0 {
                continue;
            }
            let taken = quantity.min(held);
            let paid = Fixed::ceil(taken * each).min((wagon.purse - keep).at_least_zero());
            stock.0[g] -= taken;
            wagon.cargo[g] += taken;
            post(&mut ledger, &mut cash, Side::wagon(route.seq, &mut wagon.purse), Side::treasury(route.to_id, &mut money.0),
                paid, reason("backhaul"), now, day);
            spare_food -= taken * points;
            if content.goods[g].crop {
                if let Ok(mut farm) = farms.get_mut(route.to) {
                    farm.sales_since += paid.to_f64(); // the farmers' share counts what was sold of their crops
                }
            }
            state.bought += taken;
            state.purchases += paid;
            bought_there(&content, now, &mut books, &mut whys, &route, far_keeps, wagon, g, taken, paid);
        }
    }
}

/// A backhaul (`Caravans.buyBackhaul`): the wagon goes home anyway, so what it carries back costs nothing more, and any
/// margin is profit. With the space left after the rest, it buys what the far end holds a glut of, at the glut's
/// discount (the seller would otherwise pay to store it), wherever home has a use for it: an input its lockers or crafts
/// take, food while its own runs short, a comfort its people are short of. Home sells it on, or uses it, at its worth.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn buy_backhaul(clock: Res<Clock>, content: Res<Content>, stocktakes: Res<Stocktakes>, mut ledger: ResMut<Ledger>,
        mut cash: ResMut<CashBook>, mut books: ResMut<Books>, mut whys: ResMut<Whys>,
        mut routes: Query<(Entity, &Route, &mut RouteState)>, mut towns: Query<BuyTown>, offmap: Query<&OffMap>) {
    let (now, day) = (clock.now, content.day_ticks);
    let n = content.goods.len();
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        let route = *route;
        if state.arrived != Some(Leg::Out) || state.wagon.is_none() || doesnt_sell(&offmap, &route) {
            continue;
        }
        let state = &mut *state;
        let wagon = state.wagon.as_mut().unwrap();
        // What home uses, how short of food it is and of which comforts. Its food as known (`foodKnownAt`): an off-map
        // market's as it stands, else a stocktake taken at a known time (a Librarian's); one a wagon took as it left
        // has none, and with none the Merchant reckons plenty.
        let (home_trades, home_food, short_at_home) = match towns.get(route.from) {
            Ok((stock, _, trades, people, ..)) => {
                let food = if offmap.get(route.from).is_ok() {
                    crate::road::held(&content, &stock.0)
                } else {
                    stocktakes.0.get(&route.from_id).filter(|s| s.when.is_some())
                        .map_or(f64::INFINITY, |s| crate::road::held(&content, &s.stock))
                };
                let short: Vec<bool> = (0..content.comforts.len())
                    .map(|k| people.villagers > 0.0 && people.comfort_met.get(k).copied().unwrap_or(0.0) < 1.0).collect();
                (trades.clone(), food, short)
            }
            Err(_) => continue,
        };
        let keep = keep_for_food(&content, &route, wagon);
        let slots = if wagon.slots > 0 { wagon.slots } else { content.dials.wagon_slots };
        let (mut stock, mut money, trades, _, roster, works) = towns.get_mut(route.to).expect("the far end");
        let far_keeps = keeps(&content, roster, works);
        for g in 0..n {
            let room = slots as i64 - crate::cargo::stacks(&content, &wagon.cargo, &wagon.shipments) as i64;
            if room <= 0 {
                break;
            }
            let held = stock.0[g].floor().max(0.0);
            let points = crate::road::points(&content, g);
            let is_food = points > 0.0;
            let used_at_home = want_per_day(&home_trades, g) > 0.0
                || is_food && home_food < 2.0 * crate::road::FOOD_PAR_POINTS
                || content.goods[g].comfort.is_some_and(|k| short_at_home[k]);
            if held <= 0.0 || !used_at_home {
                continue;
            }
            let per_day = want_per_day(trades, g);
            let stack = content.goods[g].stack as f64;
            let mut par = stack.max(crate::bargain::par(&content, per_day, per_day > 0.0));
            if is_food {
                par = par.max((2.0 * crate::road::FOOD_PAR_POINTS / points).floor()); // the far end's own food, and callers'
            }
            let glut = held - (GLUT_PARS * par).floor();
            if glut <= 0.0 {
                continue;
            }
            let discount = MOST_GLUT_DISCOUNT.min(GLUT_DISCOUNT * (held / par - GLUT_PARS));
            let each = content.goods[g].value * (1.0 - discount);
            let mut quantity = glut.min(room as f64 * stack);
            quantity = quantity.min(((wagon.purse - keep).to_f64() / each.max(1e-9)).floor());
            if quantity <= 0.0 {
                continue;
            }
            let taken = quantity.min(held);
            let paid = Fixed::ceil(taken * each).min((wagon.purse - keep).at_least_zero());
            stock.0[g] -= taken;
            wagon.cargo[g] += taken;
            post(&mut ledger, &mut cash, Side::wagon(route.seq, &mut wagon.purse), Side::treasury(route.to_id, &mut money.0),
                paid, reason("glut"), now, day);
            state.bought += taken;
            state.purchases += paid;
            state.backhauled += taken;
            bought_there(&content, now, &mut books, &mut whys, &route, far_keeps, wagon, g, taken, paid);
        }
    }
}
