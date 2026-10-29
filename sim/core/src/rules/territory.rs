//! Territory's steps: a town's culture counted and its hexes claimed once a day (`Review`), its guards fed and paid
//! once a day (`Pay`), and the tolls a wagon pays on the land it crosses, coming home (before `Turn` banks its purse).
//! See `territory.rs`.

use bevy_ecs::prelude::*;

use super::{in_order, Step};
use crate::cash_book::{post, CashBook, Reason, Side};
use crate::content::Content;
use crate::state::*;
use crate::territory::*;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems((
        review.in_set(Step::Review),
        upkeep.in_set(Step::Pay),
        tolls.in_set(Step::Services),
    ));
}

/// A town's count, once a day (`Territory.afterCensus`): it claims its home hex, then its culture grows by its
/// people's worth (and its works') times the days since and its mood's factor, and buys the hexes it can. A first
/// count counts as one day (`CensusTaker`).
pub fn review(clock: Res<Clock>, content: Res<Content>, towns: Res<Towns>, mut territory: ResMut<Territory>,
        q: Query<(&Town, &People)>) {
    let (now, day) = (clock.now, content.day_ticks);
    let d = &content.dials;
    for &e in &towns.0 {
        let Ok((town, people)) = q.get(e) else { continue };
        let Some(home) = town.hex else { continue };
        let days = match territory.holdings.get(&town.id).and_then(|h| h.reviewed) {
            None => 1.0,
            Some(last) if now >= last + day => (now - last) as f64 / day as f64,
            Some(_) => continue,
        };
        territory.count(town.id, home, people.villagers, people.mood, days, now, d.culture_per_hex, d.max_rings as i32);
    }
}

/// Eats at least `points` of food from the warehouse, whole items in the order of the goods; returns the points
/// eaten. Nothing at all if it holds too little (`MerchantStock.eat`).
fn eat(content: &Content, ledger: &mut Ledger, stock: &mut Stock, mut points: f64) -> f64 {
    let whole = |x: f64| x.floor().max(0.0);
    let held: f64 = content.goods.iter().enumerate().filter(|(_, g)| g.food > 0.0).map(|(i, g)| whole(stock.0[i]) * g.food).sum();
    if points <= 0.0 || held < points {
        return 0.0;
    }
    let mut eaten = 0.0;
    for (i, g) in content.goods.iter().enumerate() {
        if points <= 0.0 || g.food <= 0.0 {
            continue;
        }
        let n = whole(stock.0[i]).min((points / g.food).ceil());
        stock.0[i] -= n;
        ledger.used[i] += n;
        points -= n * g.food;
        eaten += n * g.food;
    }
    eaten
}

/// Once a day a town's guards eat from its warehouse and are paid from its money, for the days since (`upkeepGuards`):
/// food first, pay for those the idle hands don't stand in for; guards on posts that took tolls wait for their pay up
/// to `guard_credit_days`; unfed or unpaid guards desert in proportion, the earliest posted staying.
#[allow(clippy::too_many_arguments)]
pub fn upkeep(clock: Res<Clock>, content: Res<Content>, towns: Res<Towns>, mut territory: ResMut<Territory>,
        mut ledger: ResMut<Ledger>, mut book: ResMut<CashBook>, mint: Res<crate::mint::Mint>,
        mut q: Query<(&Town, &mut Stock, &mut Money, Option<&Roster>)>) {
    let (now, day) = (clock.now, content.day_ticks);
    let d = &content.dials;
    for &e in &towns.0 {
        let Ok((town, mut stock, mut money, roster)) = q.get_mut(e) else { continue };
        let Some(h) = territory.holdings.get_mut(&town.id) else { continue };
        if h.posts.is_empty() {
            h.upkept = Some(now);
            continue;
        }
        let last = match h.upkept {
            Some(t) if now >= t + day => t,
            Some(_) => continue,
            None => {
                h.upkept = Some(now);
                continue;
            }
        };
        h.upkept = Some(now);
        let days = ((now - last) as f64 / day as f64).min(MAX_DAYS);
        let posts = h.posts.len();
        // Hands without a trade stand watch first, as militia: for their food, without pay.
        let militia = roster.map_or(0, |r| r.idle(&content).len()).min(posts);
        h.militia = militia as u32;
        h.food_owed += posts as f64 * d.guard_food * days;
        h.pay_owed += (posts - militia) as f64 * d.guard_pay * days;
        let (food, pay) = (h.food_owed.floor(), h.pay_owed.floor());
        let mut fed = 1.0;
        if food > 0.0 {
            let eaten = eat(&content, &mut ledger, &mut stock, food);
            fed = (eaten / food).min(1.0);
            h.food_owed -= eaten.max(food);
        }
        let mut paid = 1.0;
        if pay > 0.0 {
            // Whole emeralds owed (fractions carried), as far as the treasury goes.
            let owed = crate::fixed::Fixed::emeralds(pay as i64);
            let taken = owed.min(money.0.at_least_zero());
            post(&mut ledger, &mut book, Side::treasury(town.id, &mut money.0), Side::outside(), taken,
                Reason::Other("guards".into()), now, day);
            paid = taken.ratio(owed).min(1.0);
            h.pay_owed -= pay;
            // Guards on posts that took tolls wait for their pay, up to a few days' worth: it comes out of the tolls
            // to come. Past that, unpaid, they go.
            let unpaid = pay * (1.0 - paid);
            let credit = d.guard_credit_days * posts as f64 * d.guard_pay;
            // (tolls taken lately, or a toll plan that expects them: new guards wait for the first; `chokepoints.rs`)
            let earning = (h.toll_today + h.toll_before).is_positive()
                || h.toll_plan.income > posts as f64 * crate::chokepoints::guard_cost_per_day(d, mint.rate(now, day));
            if unpaid > 0.0 && earning && h.pay_owed + unpaid <= credit {
                h.pay_owed += unpaid;
                paid = 1.0;
            }
        }
        h.toll_before = h.toll_today;
        h.toll_today = crate::fixed::Fixed::ZERO;
        let kept = fed.min(paid);
        if kept < 1.0 {
            let stay = (posts as f64 * kept).floor() as usize;
            h.deserted += (posts - stay) as u32;
            h.posts.truncate(stay);
        }
    }
}

/// A wagon that has got home pays each owner of posted land its path crosses its toll out of its purse, before the
/// purse is banked (`Caravans.payTolls`): reckoned in spurs at the mint's rate, as the mod's coins are.
#[allow(clippy::too_many_arguments)]
pub fn tolls(clock: Res<Clock>, content: Res<Content>, towns: Res<Towns>, mut territory: ResMut<Territory>,
        mint: Res<crate::mint::Mint>, mut ledger: ResMut<Ledger>, mut book: ResMut<CashBook>, mut money: Query<&mut Money>,
        mut routes: Query<(Entity, &Route, &mut RouteState, Option<&RoutePath>)>) {
    let (now, day) = (clock.now, content.day_ticks);
    let spurs = mint.rate(now, day);
    let order = in_order(routes.iter().map(|(e, r, _, _)| (e, r)));
    for entity in order {
        let (_, route, mut state, path) = routes.get_mut(entity).unwrap();
        let Some(path) = path else { continue };
        // A ship slips past toll posts on land (`map.rs` `Carrier::Ship`), as a Fisherman's boat does.
        if state.arrived != Some(Leg::Home) || route.carrier == crate::map::Carrier::Ship {
            continue;
        }
        let Some(wagon) = state.wagon.as_mut() else { continue };
        let carried = wagon.purse.to_f64() * spurs;
        let due = territory.tolls_due(&[route.from_id, route.to_id], &path.0, carried, content.dials.default_toll_rate);
        for (owner, toll) in due {
            // The toll in emeralds, rounded down to the thousandth (`Caravans.payTolls`' `Math.floor`).
            let toll = crate::fixed::Fixed::floor(toll / spurs).min(wagon.purse.at_least_zero());
            let Ok(mut m) = money.get_mut(towns.0[owner]) else { continue };
            post(&mut ledger, &mut book, Side::wagon(route.seq, &mut wagon.purse), Side::treasury(owner, &mut m.0), toll,
                Reason::Other("toll".into()), now, day);
            let h = territory.holdings.entry(owner).or_default();
            h.toll_today += toll;
            h.tolls_taken += toll;
        }
    }
}
