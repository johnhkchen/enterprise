//! Plan: each idle route starts a plan this look (`Plan`); the modules add to it (owed goods, collections, papers,
//! freight); the sale load fills the rest (`PlanSale`); then the wagon goes or waits (`Decide`).

use bevy_ecs::prelude::*;

use super::{free, in_order, shelf, Step, TownData};
use super::road::provision;
use crate::road::Stay;
use crate::bills::Bills;
use crate::cash_book::{post_legs, CashBook, Reason, Side};
use crate::content::Content;
use crate::contracts::Book;
use crate::messages::Output;
use crate::people::{comfort_keep, comfort_par};
use crate::realms::{OffMap, Realm};
use crate::fixed::Fixed;
use crate::state::*;
use crate::squad::crewed;
use std::collections::BTreeMap;

/// The most of one good a wagon takes for sale, in items (`Caravans.WANT_PER_ITEM`).
pub const WANT_PER_ITEM: f64 = 640.0;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems((
        start.in_set(Step::Plan),
        plan_sale.in_set(Step::PlanSale),
        decide.in_set(Step::Decide),
    ));
}

/// A route waiting for a fuller load, or that couldn't fund its trip, tries again this much later
/// (`Caravans.RECHECK_TICKS`, 6,000 ticks: a quarter day).
pub fn recheck_ticks(content: &Content) -> u64 {
    content.day_ticks / 4
}

/// An idle route starts a plan, a day at least after its last wagon left, once its recheck is due (`Caravans.poll`
/// 173: a route dispatches once `DEPARTURE_INTERVAL`, a day, has passed since its last departure and `now >=
/// route.recheckAt`); a route with its wagon out, whose last wagon left within the day, or waiting out its recheck
/// has none, and nothing plans for it: the mod reckons a load only in `dispatch`. One waiting out its recheck keeps
/// why it stayed (the mod's `route.status`).
pub fn start(clock: Res<Clock>, content: Res<Content>, mut pallets: ResMut<crate::pallet::Pallets>,
        mut routes: Query<&mut RouteState>) {
    // Last look's claims on pallets let go: a wagon that stayed gave its pallets up (`pallet-claims`).
    pallets.clear_claims();
    let n = content.goods.len();
    for mut state in &mut routes {
        let rested = state.last_departure.is_none_or(|t| clock.now.saturating_sub(t) >= content.day_ticks);
        let due = clock.now >= state.recheck_at;
        state.plan = (state.wagon.is_none() && rested && due).then(|| Plan { cargo: vec![0.0; n], ..Default::default() });
        if state.wagon.is_none() && !rested {
            state.stayed = Some(Stay::Home);
        }
    }
}

/// For sale, on top of what's planned already: comforts by kind, the cheapest first, up to the far end's par in worth
/// from what home holds beyond its keep; any other good the far end wants (its people use it, a trade it has switched
/// on takes it, it's an input to what it's contracted to sell, its realm's people or a player's shop there want it:
/// `Caravans.wantsThere`), what home can spare beyond its own shelf up to the room on theirs, no more of one good than
/// its share of the slots left (`pickCargo`); all in the wagon's slots. The load is worth its goods' worth, as the mod
/// weighs a trip against its road food (`saleLoad` 2189: `Prices.emeraldsEach`, the comforts the far end is short of
/// as far as they go); a good the far end wouldn't deal in at any price stays home (`bargain.rs`).
///
/// A home with a roster sizes the room on the far end's shelf by its last stocktake while it's fresh (`atlas.rs`), less
/// what other wagons on their way there carry; with none, by its stock as it is. Why each good went or stayed goes
/// with the route, for the reasons' tally (`Caravans.pickCargo`).
#[allow(clippy::too_many_arguments)]
fn plan_sale(clock: Res<Clock>, content: Res<Content>, book: Res<Book>, stocktakes: Res<crate::atlas::Stocktakes>,
        mut routes: Query<(&Route, &mut RouteState)>, towns: Query<(&Stock, &Trades, &People)>,
        rosters: Query<&crate::roster::Roster>, pallets: Res<crate::pallet::Pallets>, realms: Query<&Realm>,
        orders: Res<crate::orders::Orders>, lockers: Query<&crate::lockers::Lockers>) {
    use crate::atlas::{Want, Why};
    let d = &content.dials;
    let n = content.goods.len();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| content.goods[a].value.total_cmp(&content.goods[b].value));
    // What wagons on their way out carry to each far end: loose, and crated (`rules/cargo.rs::ship` crates a good for
    // sale of two stacks or more as a lot; a contract's pallet is made out to the far end).
    let mut aboard: BTreeMap<usize, Vec<f64>> = BTreeMap::new();
    for (route, state) in routes.iter() {
        if let Some(w) = state.wagon.as_ref().filter(|w| w.leg == Leg::Out) {
            let a = aboard.entry(route.to_id).or_insert_with(|| vec![0.0; n]);
            for (g, c) in w.cargo.iter().enumerate().take(n) {
                a[g] += c;
            }
            for p in pallets.aboard(route.seq, &w.manifest).filter(|p| p.consigned_to(route.to_id)) {
                for (good, count) in p.goods().into_iter().filter(|(g, _)| *g < n) {
                    a[good] += count;
                }
            }
            for l in w.for_sale.iter().filter(|l| l.good() < n) {
                a[l.good()] += l.count();
            }
        }
    }
    for (route, mut state) in &mut routes {
        let state = &mut *state;
        state.whys.clear();
        let Some(plan) = state.plan.as_mut() else { continue };
        let whys = &mut state.whys;
        // The squad's caps (`squad.rs`: packs and porters; slots, weight and pallets since `pallet-caps`), a wagon's
        // without one.
        let caps = plan_caps(&content, plan, rosters.get(route.from).ok(), false, route.carrier);
        let slots = caps.slots;
        let Ok([(home_stock, home_trades, home_people), (far_stock, far_trades, far_people)]) =
            towns.get_many([route.from, route.to]) else { continue };
        // A realm's Merchants haggle at their own level (`realms.rs`), and its people want comforts as goods, by par.
        let far_realm = realms.get(route.to).ok();
        // The far end's lockers, for the trades it has switched on (`Caravans.wantsAt`).
        let far_lockers = lockers.get(route.to).ok().zip(rosters.get(route.to).ok());
        let far_taken = far_lockers.map(|(l, r)| l.taken(&content, r, n));
        let seller = crate::realms::level(&content, realms.get(route.from).ok(), rosters.get(route.from).ok());
        let buyer = crate::realms::level(&content, far_realm, rosters.get(route.to).ok());
        // What the far end holds of a good, as home knows it (`atlas.rs`): a home with a roster reads its fresh
        // stocktake, and counts what's on its way there; else its stock as it is.
        // (A realm's wagons are never crewed from a roster: `realms.rs`.)
        let rostered = realms.get(route.from).is_err() && crewed(rosters.get(route.from).ok());
        let seen = if rostered { stocktakes.fresh(&content, route.to_id, clock.now) } else { None };
        let on_the_way = if rostered { aboard.get(&route.to_id) } else { None };
        let held = |g: usize| -> f64 {
            seen.map_or(far_stock.0[g], |s| s.stock.get(g).copied().unwrap_or(0.0))
                + on_the_way.map_or(0.0, |a| a[g])
        };
        // What the town owes under harvest contracts by the end of the next window, beyond what this plan already
        // carries to the contract's buyer and what's crated for them, isn't for sale (`ForwardSales.setAside`).
        let mut aside = crate::cargo::set_aside_all(&book, &pallets, route.from_id, n, clock.now, content.day_ticks);
        for &(id, taken) in &plan.contracted {
            let c = &book.contracts[id];
            if c.harvest.is_some() {
                aside[c.good] = (aside[c.good] - taken).max(0.0);
            }
        }
        // What's planned so far: the goods owed as they'll go on pallets, home's pallets (`rules/cargo.rs`), what loads first
        // (crates booked on its run, goods sent on through a hub: `freight.rs`); its slots and weight.
        let mut load = plan_load(&content, &book, &pallets, plan);
        let mut stacks: u32 = load.slots;
        // Comforts first, out of what home holds beyond its own par of each kind: half again what the far end is short
        // of (`comfort_margin`), the cheapest goods of a kind first, whole items (`Caravans.dispatch` 468–492,
        // `pickComforts`). What the far end is short of is its par less what its last stocktake held while that's fresh
        // (a home with a roster), less what its people have used since (`comfortWants`); else as it stands.
        if far_realm.is_none() {
            let counted = if rostered { stocktakes.fresh(&content, route.to_id, clock.now) } else { None };
            let arrival = clock.now + route.travel_ticks;
            let far_par = comfort_par(&content, far_people);
            let needs = match counted {
                Some(t) => crate::desires::comfort_wants(&content, far_par, far_people.villagers, &t.stock, t.when, arrival),
                None => crate::desires::comfort_wants(&content, far_par, far_people.villagers, &far_stock.0, None, arrival),
            };
            let keep = comfort_keep(&content, home_people);
            let left: Vec<f64> = (0..n).map(|g| home_stock.0[g] - plan.cargo[g]).collect();
            let mut wants = needs.clone();
            for kind in 0..wants.len() {
                if wants[kind] <= 0.0 {
                    continue;
                }
                let held = crate::desires::held_in(&content, &left, kind);
                if held <= keep {
                    // They're short of it, and we keep our own par of it, or have none (`dispatch` 476–483).
                    whys.push((None, if held > 0.0 { Why::ShortKeepOurPar(kind) } else { Why::ShortHaveNone(kind) }, 0.0));
                }
                wants[kind] = wants[kind].min((held - keep).max(0.0) / d.comfort_margin);
            }
            let avail: Vec<f64> = (0..n).map(|g| (left[g] - aside[g]).max(0.0)).collect();
            for (g, count) in crate::desires::pick_comforts(&content, &wants, &avail) {
                let stack = content.goods[g].stack as f64;
                let going = count.min(caps.less(&load).loose(&content, g)).floor();
                if going <= 0.0 {
                    continue;
                }
                let kind = content.goods[g].comfort.expect("a comfort");
                // What the far end will buy: whole items up to the gap (`sell`), at the price at the middle of the batch.
                let gap = (needs[kind] / content.goods[g].value.max(1e-9)).ceil();
                let held = far_stock.0[g];
                let sells = going.min(gap);
                if crate::bargain::unit_price(&content, g, held, sells, held + gap, seller, buyer).is_none() {
                    continue;
                }
                let each = content.goods[g].value;
                plan.cargo[g] += going;
                stacks += (going / stack).ceil() as u32;
                load.slots = stacks;
                load.weight += going * content.goods[g].weight;
                plan.worth += sells * each;
                plan.sale.push((g, sells, each));
                whys.push((Some(g), Why::SentComfort(kind), going));
            }
        }
        // What the far end needs to make what it has contracted to sell goes first (`Caravans.wantsAt`: its inputs are
        // listed first). Each good it wants that home can send shares the slots left (`pickCargo` 2391–2399): no more
        // of one than `WANT_PER_ITEM`, nor than the slots left over the goods that could go, 64 to a slot, a stack at
        // least.
        let needs_input = |g: usize| far_trades.inputs.get(g).is_some_and(|n| *n > 0.0);
        let mut order = order.clone();
        order.sort_by_key(|&g| !needs_input(g));
        // Each good's standing: why it stays (`Caravans.pickCargo`: they hold their fill, we have none, it's set aside,
        // it's our own shelf), or what it could send, the room there and the far end's par for it.
        #[allow(clippy::type_complexity)]
        let mut standing: Vec<(usize, Result<(f64, f64, f64, Option<Want>, f64), (Why, f64)>)> = Vec::new();
        for &g in &order {
            let planned = plan.cargo[g];
            // A trade it has switched on takes it, and its people don't use it: its par is the warehouse's for a
            // wanted good (`Warehouse.par(to, item, true)`), a stack at least (`wantsAt`, `pickCargo`).
            // Or a raw good its own trades turn into one its lockers take and lack (cane for its Cartographer's paper):
            // as much as makes up the lack (`rules/workshop.rs` makes it there).
            let locker_par = crate::lockers::wanted_par_of(&content, far_trades, far_taken.as_deref(), g)
                .or_else(|| crate::lockers::craft_par(&content, rosters.get(route.to).ok(), far_trades, far_taken.as_deref(), &held, g));
            match content.goods[g].comfort.filter(|_| far_realm.is_none()) {
                Some(_) => continue, // (the comforts went first)
                None if far_trades.uses[g] > 0.0 || needs_input(g) || far_realm.is_some_and(|r| r.wants.contains(&g))
                    || locker_par.is_some() || orders.wanted_of(route.to_id, g) > 0.0 => {
                    // A good the far end wants for its own reasons has its usual shelf, whatever it has ordered. One
                    // wanted only because a player's shop there ordered it has the room `Warehouse.par(to, item, true)`
                    // gives it (a stack at least), less what's held: an order only adds the good to the wanted set
                    // (`wantsThere`, `pickCargo`).
                    let usual = far_trades.uses[g] > 0.0 || needs_input(g) || far_realm.is_some_and(|r| r.wants.contains(&g));
                    let (want, par) = if usual {
                        (Want::of(far_trades, g), shelf(&content, far_trades, g))
                    } else if let Some(par) = locker_par {
                        (Want::Locker, par)
                    } else {
                        (Want::Ordered, crate::bargain::par(&content, 0.0, true))
                    };
                    let room = par - held(g);
                    let have = home_stock.0[g] - planned;
                    let keep = shelf(&content, home_trades, g);
                    standing.push((g, if room <= 0.0 {
                        Err((Why::HoldTheirFill(want), held(g)))
                    } else if have <= 0.0 {
                        Err((Why::HaveNone(want), room))
                    } else if have <= aside[g] {
                        Err((Why::SetAside(want), room))
                    } else if have - aside[g] - keep <= 0.0 {
                        Err((Why::KeepOurShelf(want), room))
                    } else {
                        Ok((have - aside[g] - keep, room, par, Some(want), have - aside[g]))
                    }));
                }
                None => continue,
            }
        }
        let goods = standing.iter().filter(|(_, s)| s.is_ok()).count().max(1) as u32;
        let per_item = WANT_PER_ITEM.min(((slots.saturating_sub(stacks) / goods).max(1) * 64) as f64);
        for (g, standing) in standing {
            let (spare, room, par, want) = match standing {
                Err((why, count)) => {
                    whys.push((Some(g), why, count.max(0.0)));
                    continue;
                }
                Ok((_, room, _, _, sendable)) if stacks >= slots || caps.less(&load).loose(&content, g) < 1.0 => {
                    whys.push((Some(g), Why::NoRoomAboard, room.min(sendable).max(0.0)));
                    continue;
                }
                Ok((spare, room, par, want, _)) => (spare, room, par, want),
            };
            let stack = content.goods[g].stack as f64;
            let going = spare.min(room).min(per_item).min(caps.less(&load).loose(&content, g)).floor();
            if going <= 0.0 {
                continue;
            }
            let far_held = if want.is_some() { held(g) } else { far_stock.0[g] };
            if crate::bargain::unit_price(&content, g, far_held, going, par, seller, buyer).is_none() {
                if let Some(want) = want {
                    whys.push((Some(g), Why::NoDeal(want), going));
                }
                continue;
            }
            let each = content.goods[g].value;
            plan.cargo[g] += going;
            stacks += (going / stack).ceil() as u32;
            load.slots = stacks;
            load.weight += going * content.goods[g].weight;
            plan.worth += going * each;
            plan.sale.push((g, going, each));
            match (content.goods[g].comfort, want) {
                (Some(kind), _) => whys.push((Some(g), Why::SentComfort(kind), going)),
                (None, Some(want)) => whys.push((Some(g), Why::Sent(want), going)),
                _ => {}
            }
        }
        plan.stacks = stacks;
    }
}

/// The wagon goes when something can't wait, or when full or on its day with a load worth its road food, if the town
/// can pay for the road and what it collects (where content has food, the road's food: `rules/road.rs`). Leaving, it
/// takes its cargo, the money for its collections and the bills home holds on the far end.
#[allow(clippy::too_many_arguments)]
pub(crate) fn decide(clock: Res<Clock>, content: Res<Content>, mut book: ResMut<Book>, mut bills: ResMut<Bills>,
        mut ledger: ResMut<Ledger>, mut cash: ResMut<CashBook>, mut outbox: ResMut<Outbox>, mut routes: Query<(Entity, &Route, &mut RouteState)>,
        mut towns: Query<TownData>, mut rosters: Query<&mut crate::roster::Roster>, mut lockers: Query<&mut crate::lockers::Lockers>,
        places: Query<(Option<&OffMap>, Option<&Realm>, Option<&crate::freight::Hub>)>, mut pallets: ResMut<crate::pallet::Pallets>,
        mut herds: Query<&mut crate::herds::Herds>, hiring: Res<crate::jobs::Hiring>) {
    let now = clock.now;
    let today = now / content.day_ticks.max(1);
    let d = &content.dials;
    // Squads (`squad.rs`): each town's routes, and those of them without a wagon (`dispatch`'s `waiting`).
    let (mut routes_at, mut waiting): (BTreeMap<usize, usize>, BTreeMap<usize, usize>) = Default::default();
    for (_, route, state) in routes.iter() {
        *routes_at.entry(route.from_id).or_default() += 1;
        *waiting.entry(route.from_id).or_default() += state.wagon.is_none() as usize;
    }
    // Road food (`road.rs`): what wagons on their way out will eat at each far end, if they packed none.
    let food_mode = crate::road::road_food(&content);
    let mut promised: std::collections::BTreeMap<usize, f64> = std::collections::BTreeMap::new();
    if food_mode {
        // (In the routes' order: a sum's last bit mustn't hang on archetypes, which a load reorders.)
        for e in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
            let (_, route, state) = routes.get(e).unwrap();
            if let Some(w) = state.wagon.as_ref().filter(|w| w.leg == Leg::Out && w.provisions <= 0.0) {
                *promised.entry(route.to_id).or_insert(0.0) += crate::road::food_needed(&content, route.blocks)
                    * crate::road::mouths(Some(w));
            }
        }
    }
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        let route = *route;
        let Some(mut plan) = state.plan.take() else { continue };
        state.tried_at = Some(now);
        // (A realm's wagons are never crewed from a roster, whoever stands in it: `realms.rs`.)
        let foreign = places.get(route.from).is_ok_and(|(_, realm, _)| realm.is_some());
        let rostered = !foreign && rosters.get(route.from).ok().is_some_and(|r| crewed(Some(r)));
        // The stacks of what the far end wants that home holds now (`wantedStacks`), for the squad's packs and porters.
        let stacks_wanted = match (rostered, towns.get(route.from), towns.get(route.to)) {
            (true, Ok((home, ..)), Ok((_, _, far, ..))) => crate::squad::wanted_load(&content, &home.0, far,
                lockers.get(route.to).ok().zip(rosters.get(route.to).ok())),
            _ => (0, 0.0),
        };
        let mut roster = rosters.get_mut(route.from).ok();
        // A town with a roster sends no wagon with no Merchant at its desk (`Caravans.dispatch` 210).
        if rostered && roster.as_ref().is_some_and(|r| r.merchant.is_none()) {
            state.stayed = Some(Stay::NoMerchant);
            for id in &plan.pallets { pallets.release(*id); }
            continue;
        }
        // A llama road goes with a train, of the llamas still home (another road's may have gone with some this look):
        // the train it sized up, else what its load wants (`llamas.rs`); none home, it stays.
        if route.carrier == crate::map::Carrier::Llama {
            let home = herds.get(route.from).map_or(0, |h| h.llamas.home);
            let want = if plan.llamas > 0 { plan.llamas } else { crate::llamas::wanted(&content, plan.stacks, 0.0) };
            plan.llamas = want.min(home);
            if plan.llamas == 0 {
                state.stayed = Some(Stay::NoLlamas);
                for id in &plan.pallets { pallets.release(*id); }
                continue;
            }
        } else {
            plan.llamas = 0;
        }
        // A realm's wagon is its own people's: no squad, two mouths' road food, no team, never home's poverty counted.
        // An off-map market that sells no food feeds no one (`OffMap.sells`): its wagons pack the road home's food.
        let far_sells = places.get(route.to).map_or(true, |(off, ..)| off.map_or(true, |o| o.sells_food));
        let far_food = if food_mode && far_sells {
            towns.get(route.to).map_or(0.0, |(far, ..)| crate::road::held(&content, &far.0))
                - promised.get(&route.to_id).copied().unwrap_or(0.0)
        } else {
            0.0
        };
        let (mut stock, mut money, trades, mut budget, _) = towns.get_mut(route.from).expect("home");
        // Every route planned on the warehouse as it was at the look's start: another wagon from here may have left
        // with some of it since. What's owed first, then the rest, as far as the shelves still hold.
        let mut left = stock.0.clone();
        let mut owed = vec![0.0; left.len()];
        for (id, n) in plan.contracted.iter_mut() {
            let g = book.contracts[*id].good;
            *n = n.min(left[g].max(0.0)).floor();
            left[g] -= *n;
            owed[g] += *n;
        }
        plan.contracted.retain(|&(_, n)| n > 0.0);
        for g in 0..plan.cargo.len() {
            plan.cargo[g] = owed[g] + (plan.cargo[g] - owed[g]).min(left[g].max(0.0)).max(0.0).floor();
        }
        // The load: what's owed as it'll go on pallets, the sale load loose, home's pallets it takes (`rules/cargo.rs`),
        // what loads first at a hub (`freight.rs`).
        let load = plan_load(&content, &book, &pallets, &plan);
        plan.stacks = load.slots;
        let on_its_day = state.last_departure.map_or(true, |t| now - t >= route.every_ticks);
        let waiting_here = waiting.get(&route.from_id).copied().unwrap_or(0);
        let mut porter = false;
        if let Some(roster) = roster.as_deref_mut().filter(|_| rostered) {
            // The squad, mustered of the hands still at home (`squad.rs`); nobody to lead it, a hand taken on, or none.
            let wanted_trades = crate::atlas::trip_wants(&content, &state);
            let routes_here = routes_at.get(&route.from_id).copied().unwrap_or(0);
            let mustered = crate::rules::squad::crew(&content, &mut ledger, &mut cash, now, route.from_id, &mut plan,
                roster, lockers.get_mut(route.from).ok().map(|l| l.into_inner()), stacks_wanted, &wanted_trades,
                waiting_here, routes_here, hiring.left_idle.get(&route.from_id) == Some(&today),
                route.carrier == crate::map::Carrier::Ship, &mut money,
                |m| free(&content, trades, m, &bills, route.from_id, now));
            if let Err(stay) = mustered {
                state.stayed = Some(stay);
                for id in &plan.pallets { pallets.release(*id); }
                continue;
            }
            // A small load goes on a porter's back: one hand, one pack, no team to feed.
            let owed_stacks: u32 = owed.iter().enumerate()
                .map(|(g, n)| (n / content.goods[g].stack as f64).ceil() as u32).sum();
            // (A ship's crew sail it: no porter walks a sea road.)
            porter = route.carrier != crate::map::Carrier::Ship
                && crate::rules::squad::porter_run(&content, &state, &plan, owed_stacks, on_its_day, Some(roster), &load);
            if porter {
                plan.llamas = 0; // a porter on foot: the llamas stay home
                crate::rules::squad::to_porter(&content, &mut plan, roster);
            }
        }
        if food_mode && !crate::road::in_range(&content, route.blocks) {
            state.stayed = Some(Stay::OutOfRange);
            for id in &plan.pallets { pallets.release(*id); }
            continue;
        }
        if let Some(roster) = roster.as_deref().filter(|_| rostered) {
            // Each member eats: one more only while home's food (what the wagons gone before left) feeds it.
            let home_food = food_mode.then(|| crate::road::held(&content, &stock.0));
            crate::rules::squad::trim(&content, roster, &mut plan, home_food, route.blocks, porter);
        }
        let caps = plan_caps(&content, &plan, roster.as_deref().filter(|_| rostered), porter, route.carrier);
        let slots = caps.slots;
        if !caps.holds(&load) {
            // Less room than the plan was made for (slots, weight or pallets): the last of the sale load stays home; then,
            // were it still over (a squad trimmed to home's food), the last of what's owed, as the mod picks what's owed
            // with the slots it leaves with (`pickContracted`'s `room`).
            clip(&content, &book, &pallets, &mut plan, &owed, &caps);
            fit_owed(&content, &book, &pallets, &mut plan, &mut owed, &caps);
        }
        // Full by what the far end could pay for when its last wagon sold there (`saleLoad`): a freight hub's forwarder
        // takes on consignment what it can't pay for, so there any.
        let (far_realm, far_hub) = places.get(route.to)
            .map_or((false, false), |(_, realm, hub)| (realm.is_some(), hub.is_some_and(|h| h.storehouse)));
        let purse = if far_hub { None } else { state.far_purse };
        // Full in slots, or in weight (`pallet-caps`: a heavy good fills a wagon by weight first).
        let full_load = full_load(&content, &pallets, &plan, &owed, purse, far_realm);
        let full = full_load.slots as f64 >= d.wagon_full * slots as f64 || full_load.weight >= d.wagon_full * caps.weight;
        // Each of the crew eats (`squad.rs`); a town without a roster sends one.
        let mouths = if foreign { crate::realms::crew(&content) } else { plan.crew.len().max(1) as f64 };
        // The road's cost: its food where content has food (eaten from home, bought on the way, a purse for the road
        // home's), else its money; a load must be worth both legs' food, and home must pay for the road and the purse.
        let keep_wheat = food_mode && crate::road::keep_wheat(&content, trades, roster.as_deref());
        let provision = food_mode.then(|| {
            let after: Vec<f64> = stock.0.iter().zip(&plan.cargo).map(|(s, c)| s - c).collect();
            let home_free = if foreign { f64::INFINITY } else { free(&content, trades, &money, &bills, route.from_id, now).to_f64() };
            let mut p = provision(&content, route.blocks, mouths, crate::road::held(&content, &after), far_food,
                home_free, |feed| crate::road::fodder(&content, &after, feed, keep_wheat).2);
            if porter || foreign || route.carrier == crate::map::Carrier::Ship {
                crate::rules::squad::no_team(&content, &mut p);
            }
            p
        });
        let (road_money, wagon_purse, cost) = match provision {
            Some(p) => (p.food_coin + p.team_coin, p.road_purse + plan.purse, p.cost),
            None => (route.road_cost, plan.purse, route.road_cost.to_f64()),
        };
        let pays = plan.worth >= cost;
        // (Where content has food, a purse the plan filled to all home has beyond the road may be a few thousandths
        // over it: the road's food is rounded up leg by leg here and as one sum where the plan reckoned it.)
        let slack = if provision.is_some() { Fixed::from_millis(3) } else { Fixed::ZERO };
        // (Would it go, were it funded: what its town saves up for, `rules/farms.rs` `road_reserve`.)
        state.would_go = foreign || plan.pressing || (full || on_its_day) && pays;
        if money.0 + slack < road_money + wagon_purse || money.0 < road_money {
            state.stayed = Some(Stay::CantFund);
            state.recheck_at = now + recheck_ticks(&content); // `dispatch` 374: "the hub can't fund the trip"
            for id in &plan.pallets { pallets.release(*id); }
            continue;
        }
        let wagon_purse = if provision.is_some() { wagon_purse.min(money.0 - road_money).at_least_zero() } else { wagon_purse };
        // A realm's wagon never waits for a load (`dispatch`'s waiting is `!foreign`): a day after the last it goes
        // with anything aboard or any business there; only an empty trip stays home (`no empty trips`).
        let goes = if foreign {
            state.last_departure.map_or(true, |t| now - t >= content.day_ticks)
                && (plan.stacks > 0 || plan.pressing || plan.purse.is_positive() || !plan.collecting.is_empty())
        } else {
            plan.pressing || (full || on_its_day) && pays
        };
        state.last_plan = Some(crate::state::PlanNote { at: now, worth: plan.worth, cost, purse: wagon_purse.to_f64(), full, on_its_day,
            pressing: plan.pressing, went: goes, stacks: plan.stacks, whys: state.whys.clone() });
        if !goes {
            state.stayed = Some(Stay::WaitingForALoad);
            // It looks again in a while (`dispatch` 438); a realm's wagon never waits, and an empty trip sets no
            // recheck (`dispatch` 501–509: "nothing to sell there").
            if !foreign {
                state.recheck_at = now + recheck_ticks(&content);
            }
            for id in &plan.pallets { pallets.release(*id); }
            continue;
        }
        state.stayed = None;
        for (g, n) in plan.cargo.iter().enumerate() {
            stock.0[g] -= n;
        }
        for &(id, n) in &plan.contracted {
            book.ship(id, n);
        }
        if let Some(p) = provision {
            // The team's fodder first, as it was found (the mod eats it after the food: the same, unless wheat is food).
            crate::road::eat_fodder(&content, &mut ledger, &mut stock.0, p.team_feed, keep_wheat);
            // The way out's food and any packed for the road home, from home's warehouse: already paid for.
            crate::road::eat(&content, &mut ledger, &mut stock.0, p.from_home + p.provisions);
            state.food_eaten += p.food; // the way out's (`foodEaten`, as the mod counts it at dispatch)
            if p.provisions <= 0.0 {
                *promised.entry(route.to_id).or_insert(0.0) += p.food;
            }
        }
        // Road food (what it lacks of it, and the team's fodder, bought on the way) and the wagon's purse go out of the
        // treasury as one sum.
        let free_before = free(&content, trades, &money, &bills, route.from_id, now);
        let mut purse = Fixed::ZERO;
        post_legs(&mut ledger, &mut cash, Side::treasury(route.from_id, &mut money.0), vec![
            (Side::outside(), road_money, Reason::RoadFood),
            (Side::wagon(route.seq, &mut purse), wagon_purse, Reason::WagonPurse),
        ], now, content.day_ticks);
        // The crew go away from home (`squad.rs`); a saddle in the warehouse, it rides a third faster.
        let crew = crate::rules::squad::depart(roster.as_deref_mut(), &plan.crew);
        // Its llamas go with it (`llamas.rs`).
        if plan.llamas > 0 {
            if let Ok(mut h) = herds.get_mut(route.from) {
                h.llamas.home -= plan.llamas;
                h.llamas.away += plan.llamas;
            }
        }
        if let Some(w) = waiting.get_mut(&route.from_id) {
            *w -= 1;
        }
        let saddled = crate::rules::squad::saddle(&content, &mut ledger, &mut state, &mut stock.0);
        let arrival = now + crate::squad::leg_ticks(&content, route.travel_ticks, saddled, 1.0);
        state.last_departure = Some(now);
        // What its last wagon could carry and the mouths that ate on the road, team included (`Desires.haulEach`); and
        // the shopping money it took, counted spent off its town's day's budget (the Accountant allowed it at the plan).
        state.last_capacity = slots;
        state.last_crew = (mouths + if porter || foreign || route.carrier == crate::map::Carrier::Ship { 0.0 }
            else { d.wagon_team as f64 }) as u32;
        super::desires::spend(&content, now, free_before, &mut budget, plan.shopping_money);
        state.carried += plan.cargo.iter().sum::<f64>();
        let papers = bills.take_to_present(route.from_id, route.to_id, arrival);
        state.wagon = Some(Wagon {
            cargo: plan.cargo,
            purse,
            leg: Leg::Out,
            arrives: arrival,
            contracted: plan.contracted,
            collecting: plan.collecting,
            bills: papers,
            provisions: provision.map_or(0.0, |p| p.provisions),
            // Home's pallets it planned, on its manifest: loaded as it leaves (`rules/cargo.rs` `ship`).
            manifest: crate::pallet::Manifest { lines: plan.pallets.iter()
                .map(|&pallet| crate::pallet::Line { pallet, ..Default::default() }).collect(), ..Default::default() },
            crew,
            porter,
            slots,
            llamas: plan.llamas,
            saddled,
            shopping: plan.shopping,
            full,
            // It sets out for the far end; a call on its way goes in as it leaves (`rules/freight.rs` `depart`).
            itinerary: crate::freight::Itinerary::direct(route.to_id, arrival - now),
            ..Default::default()
        });
        outbox.0.push(Output::WagonLeft { route: crate::wire::no(route.seq), stacks: plan.stacks, worth: plan.worth });
    }
}

/// The caps a plan is sized to (`pallet-caps`): its squad's (`squad.rs`: a wagon and its packs, a porter run's one pack),
/// as `roster` has the crew it mustered, its slots those the squad sized up (`Plan.slots`, when set); a plain wagon's
/// without a crew; a ship's hold [`crate::map::SHIP_HOLD`] times them.
pub(crate) fn plan_caps(content: &Content, plan: &Plan, roster: Option<&crate::roster::Roster>, porter: bool,
        carrier: crate::map::Carrier) -> crate::pallet::Caps {
    let mut caps = match roster.filter(|_| !plan.crew.is_empty()) {
        Some(r) => crate::pallet::Caps::of(content, plan.crew.iter().filter_map(|&i| r.hands.get(i).map(|h| h.trade)), porter,
            plan.llamas),
        None => crate::pallet::Caps::of(content, [], false, plan.llamas),
    };
    if plan.slots > 0 {
        caps.slots = plan.slots;
    }
    if carrier == crate::map::Carrier::Ship {
        let hold = crate::map::SHIP_HOLD;
        caps = crate::pallet::Caps { slots: caps.slots * hold, weight: caps.weight * hold as f64, pallets: caps.pallets * hold };
    }
    caps
}

/// What a plan owes of each good under its contracts (`Plan.contracted`), in its cargo.
fn owed_of(content: &Content, book: &Book, plan: &Plan) -> Vec<f64> {
    let mut owed = vec![0.0; content.goods.len()];
    for &(id, n) in &plan.contracted {
        owed[book.contracts[id].good] += n;
    }
    owed
}

/// What a plan loads (`pallet-caps`): the goods owed under each contract as packing will put them on pallets as the wagon
/// leaves (`rules/cargo.rs` `ship`: a contract's goods on pallets of their own, a bill each), the rest of its cargo loose,
/// home's pallets it claimed, and what loads first at a hub (`Plan.booked`, and its weight).
pub(crate) fn plan_load(content: &Content, book: &Book, pallets: &crate::pallet::Pallets, plan: &Plan) -> crate::pallet::Load {
    let mut load = crate::pallet::Load::of(content, &[], plan.pallets.iter().filter_map(|id| pallets.get(*id)));
    let owed = owed_of(content, book, plan);
    for &(id, n) in &plan.contracted {
        if n > 0.0 {
            load = load.and(crate::pallet::palleted(content, book.contracts[id].good, n));
        }
    }
    for (g, &n) in plan.cargo.iter().enumerate() {
        let owed = owed[g].min(n).max(0.0);
        let rest = n - owed;
        load.slots += crate::cargo::stacks_of(content, g, rest);
        load.weight += rest.max(0.0) * content.goods[g].weight;
    }
    load.slots += plan.booked;
    load.weight += plan.booked_weight;
    load
}

/// The load that makes a wagon full (`Caravans.saleLoad` 2168–2205, `dispatch` 414): what's owed, home's crates
/// and what loads first, and of the sale load as much as the far end could pay for when a wagon last sold there
/// (`Route.farPurse`, at each good's worth, in the order it went aboard). Comforts count toward a load's worth, not
/// its stacks (a realm's people want theirs as goods). `purse`: `None` for any (never sold there yet, or a freight
/// hub). A load the far end can't pay for waits for its day rather than going full. Its slots, and its weight
/// (`pallet-caps`: a wagon is full when either reaches `wagon_full` of its cap).
fn full_load(content: &Content, pallets: &crate::pallet::Pallets, plan: &Plan, owed: &[f64], purse: Option<Fixed>,
        realm: bool) -> crate::pallet::Load {
    let mut load = crate::pallet::Load::of(content, &[], plan.pallets.iter().filter_map(|id| pallets.get(*id)));
    for (g, &n) in owed.iter().enumerate() {
        if n > 0.0 {
            load = load.and(crate::pallet::palleted(content, g, n));
        }
    }
    load.slots += plan.booked;
    load.weight += plan.booked_weight;
    let mut paid = 0.0;
    for &(g, n, _) in &plan.sale {
        if content.goods[g].comfort.is_some() && !realm {
            continue;
        }
        let n = n.min((plan.cargo[g] - owed[g]).max(0.0));
        let each = content.goods[g].value;
        let sells = match purse {
            Some(p) if each > 0.0 => n.min(((p.to_f64() - paid).max(0.0) / each).floor()),
            _ => n,
        };
        load.slots += crate::cargo::stacks_of(content, g, sells);
        load.weight += sells.max(0.0) * content.goods[g].weight;
        paid += sells * each;
    }
    load
}

/// What's owed cut to `caps` once the sale load is gone (`pallet-caps`): the last contract's first, to what the room left
/// holds of it on pallets; its worth goes with it. Home's pallets stay (`plan_crated` took them within a wagon's caps).
fn fit_owed(content: &Content, book: &Book, pallets: &crate::pallet::Pallets, plan: &mut Plan, owed: &mut [f64],
        caps: &crate::pallet::Caps) {
    for i in (0..plan.contracted.len()).rev() {
        if caps.holds(&plan_load(content, book, pallets, plan)) {
            break;
        }
        let (id, n) = plan.contracted[i];
        let c = &book.contracts[id];
        plan.contracted[i].1 = 0.0;
        plan.cargo[c.good] -= n;
        let room = caps.less(&plan_load(content, book, pallets, plan)).palleted(content, c.good);
        let kept = n.min(room).floor().max(0.0);
        plan.contracted[i].1 = kept;
        plan.cargo[c.good] += kept;
        owed[c.good] -= n - kept;
        plan.worth -= (n - kept) * c.price;
    }
    plan.contracted.retain(|&(_, n)| n > 0.0);
    plan.stacks = plan_load(content, book, pallets, plan).slots;
}

/// The sale load cut to `caps` (`plan_sale` filled the room the squad sized up at the look's start): the last of what it
/// put aboard first, whole stacks, never below what's `owed`, until slots and weight fit (`pallet-caps`: weight too); its
/// worth goes with it. Home's pallets and what's owed stay (the sale is loose: dropping it frees no pallet).
fn clip(content: &Content, book: &Book, pallets: &crate::pallet::Pallets, plan: &mut Plan, owed: &[f64],
        caps: &crate::pallet::Caps) {
    let sale = std::mem::take(&mut plan.sale);
    'sale: for &(g, n, price) in sale.iter().rev() {
        let mut worth_left = n;
        loop {
            let load = plan_load(content, book, pallets, plan);
            plan.stacks = load.slots;
            if load.slots <= caps.slots && load.weight <= caps.weight + 1e-9 {
                break 'sale;
            }
            let stack = content.goods[g].stack as f64;
            let selling = (plan.cargo[g] - owed[g]).max(0.0);
            if selling <= 0.0 {
                break;
            }
            // Stacks over in slots, and in weight (a stack of it weighs `stack * weight`).
            let over_slots = load.slots.saturating_sub(caps.slots);
            let per_stack = stack * content.goods[g].weight;
            let over_weight = if per_stack > 0.0 { ((load.weight - caps.weight) / per_stack - 1e-9).ceil().max(0.0) as u32 } else { 0 };
            let over = over_slots.max(over_weight).max(1);
            let had = (selling / stack).ceil() as u32;
            let kept = selling.min(had.saturating_sub(over) as f64 * stack);
            let dropped = selling - kept;
            plan.cargo[g] = owed[g] + kept;
            plan.worth -= dropped.min(worth_left) * price;
            worth_left = (worth_left - dropped).max(0.0);
        }
    }
    plan.stacks = plan_load(content, book, pallets, plan).slots;
    plan.sale = sale;
}
