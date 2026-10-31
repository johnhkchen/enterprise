//! The market through the look (`market.rs`): each town's bids reviewed once a day, before its farm's review reads
//! them (Review); a wagon at the far end reads the book there (Settle).

use bevy_ecs::prelude::*;

use super::{in_order, shelf, Step};
use crate::content::Content;
use crate::desires::{held_in, will_pay};
use crate::market::{rebid, uses, Market, Use};
use crate::people::comfort_par;
use crate::roster::Roster;
use crate::state::*;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems((
        // After the settlers left and the crown's review, before the farm's review reads the book.
        review.in_set(Step::Review).after(super::settlers::review).after(super::crown::review).before(super::farms::review),
        // (After the papers open, the contracts settle and the crown raises, as they ran before the market joined.)
        sight.in_set(Step::Settle).after(super::cargo::open).after(super::contracts::settle).after(super::crown::raise),
    ));
}

/// Each town's bids, once a day (`market.rs`): what it wants of each good and at most what for, the bid creeping
/// while unfilled.
#[allow(clippy::type_complexity)]
pub fn review(clock: Res<Clock>, content: Res<Content>, order: Res<Towns>, mut market: ResMut<Market>,
        towns: Query<(&Town, &Stock, &People, &Trades, &Money, Option<&Roster>, Option<&crate::farms::Farm>)>) {
    if !content.dials.market {
        return;
    }
    let today = clock.now / content.day_ticks.max(1);
    let n = content.goods.len();
    for &e in &order.0 {
        let Ok((town, stock, people, trades, money, roster, farm)) = towns.get(e) else { continue };
        let t = town.id;
        let last = market.reviewed.get(&t).copied();
        if last == Some(today) {
            continue;
        }
        market.reviewed.insert(t, today);
        let facts = super::desires::facts_of(&content, stock, money, trades, people, farm);
        let cash = facts.spare.max(0.0) + facts.purse;
        let par = comfort_par(&content, people);
        // What it wants for itself: a comfort its par less what it holds of the kind, at what its people would pay; else
        // its shelf, at the good's worth.
        let direct: Vec<Use> = (0..n).map(|g| {
            let value = content.goods[g].value;
            match content.goods[g].comfort {
                Some(kind) => {
                    let short = par - held_in(&content, &stock.0, kind);
                    let met = people.comfort_met.get(kind).copied().unwrap_or(0.0);
                    Use { want: crate::workshop::comfort_wanted(&content, people, stock, kind, g),
                          ceiling: will_pay(&content, value, met, short, cash) }
                }
                None => Use { want: (shelf(&content, trades, g) - stock.0[g]).max(0.0).ceil(), ceiling: value },
            }
        }).collect();
        // And what its trades make of those (its roster's conversions).
        let conversions: Vec<_> = roster.map(|r| content.conversions.iter().filter(|c| crate::workshop::can_do(c, r))
            .map(|c| (c.inputs.clone(), c.output, c.out)).collect()).unwrap_or_default();
        let uses = uses(direct, &conversions, &stock.0, |h| market.mark(&content, t, h));
        let creep = content.dials.bid_creep;
        let book = market.books.entry(t).or_default();
        for (g, use_) in uses.into_iter().enumerate() {
            if use_.want <= 0.0 && !book.contains_key(&g) {
                continue;
            }
            let q = book.entry(g).or_default();
            // Filled since its last review (or today): it holds; else it creeps.
            let filled = q.filled.is_some_and(|d| last.is_none_or(|l| d >= l));
            q.bid = rebid(use_, q.bid, filled, content.goods[g].value, creep);
            q.want = if q.bid > 0.0 { use_.want } else { 0.0 };
            q.ceiling = use_.ceiling;
        }
        book.retain(|_, q| q.bid > 0.0 || q.mark.is_some());
    }
}

/// A wagon at the far end reads its book (`Market::sighted`).
pub fn sight(clock: Res<Clock>, content: Res<Content>, mut market: ResMut<Market>,
        routes: Query<(Entity, &Route, &RouteState)>) {
    let today = clock.now / content.day_ticks.max(1);
    for e in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, state) = routes.get(e).unwrap();
        if state.arrived == Some(Leg::Out) {
            market.sighted(route.to_id, today);
        }
    }
}
