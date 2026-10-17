//! Road food's rules (`road.rs`; `Caravans.dispatch` and `arriveAtDestination`): what a wagon leaving takes for the
//! road ([`provision`], called by `plan::decide` before the departure check), and at the far end, the road home's food
//! eaten, bought, or waited for, then home on short rations ([`feed`], at `Turn`, before the wagon turns).

use bevy_ecs::prelude::*;

use super::{in_order, Step};
use crate::cash_book::{post, CashBook, Reason, Side};
use crate::content::Content;
use crate::road::{eat, food_needed, mouths};
use crate::state::*;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems(feed.in_set(Step::Turn).before(super::arrive::turn));
}

/// What a wagon leaving takes for the road, in food points (a leg's, for all mouths) and money.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Provision {
    /// A leg's food.
    pub food: f64,
    /// Eaten from home's warehouse for the way out.
    pub from_home: f64,
    /// Packed for the road home: the far end may not feed it, or home is too poor to pay it but rich in food.
    pub provisions: f64,
    /// The way out's food home hasn't got, bought on the way.
    pub food_coin: f64,
    /// The purse for the road home's food at the far end (none if it packed its own).
    pub road_purse: f64,
    /// The team's fodder there and back (`teamFeed`: a leg's food per animal of `wagon_team`, both ways), from home's
    /// (`fed`, `eatFodder`), and what it lacks bought on the road (`teamCoin`).
    pub team_feed: f64,
    pub fed: f64,
    pub team_coin: f64,
    /// Both legs' food and the team's fodder at `purse_per_food_point`: what a load must be worth for the trip.
    pub cost: f64,
}

/// A wagon's road food as it leaves (`dispatch`): `home_food` is the food points home holds (beyond its cargo),
/// `far_food` what the far end is known to hold less what wagons already bound there will eat (`foodKnownAt` less
/// `foodPromisedAt`), `home_free` home's money beyond its reserve, `fodder` the points of the team's fodder home has
/// (`fodderOnHand`, as [`crate::road::fodder`] finds them of the team's feed).
pub fn provision(content: &Content, blocks: u32, mouths: f64, home_food: f64, far_food: f64, home_free: f64,
        fodder: impl FnOnce(f64) -> f64) -> Provision {
    let d = &content.dials;
    let per_mouth = food_needed(content, blocks);
    let food = per_mouth * mouths;
    let ppfp = d.purse_per_food_point;
    // Where the far end won't or can't feed it, it takes that food along too, if a wagon can carry both legs'.
    let fed_there = far_food >= food;
    // A town too poor to pay the far end for the road home's food packs that too, from food it can spare.
    let own_food = home_food - crate::road::FOOD_PAR_POINTS >= 2.0 * food && home_free < food * ppfp;
    let provisions = if (!fed_there || own_food) && 2.0 * per_mouth <= d.food_capacity as f64 && home_food >= 2.0 * food {
        food
    } else {
        0.0
    };
    let road_purse = if provisions > 0.0 { 0.0 } else { food * ppfp };
    let from_home = food.min(home_food - provisions).max(0.0);
    let team_feed = 2.0 * per_mouth * d.wagon_team as f64;
    let fed = fodder(team_feed).min(team_feed);
    Provision { food, from_home, provisions, food_coin: (food - from_home) * ppfp, road_purse, team_feed, fed,
        team_coin: (team_feed - fed) * ppfp, cost: (2.0 * food + team_feed) * ppfp }
}

/// At the far end, before it turns for home: a wagon that packed the road home's food eats it; one that didn't eats
/// the far end's, paying its treasury the food's worth (in whole spurs, as far as its purse goes); none there, it
/// waits, and after `max_wait_ticks` goes home on short rations, at half pace. A wagon waiting tries again each look.
#[allow(clippy::too_many_arguments)]
pub fn feed(clock: Res<Clock>, content: Res<Content>, mint: Res<crate::mint::Mint>, mut ledger: ResMut<Ledger>,
        mut cash: ResMut<CashBook>, mut routes: Query<(Entity, &Route, &mut RouteState)>,
        mut towns: Query<(&mut Stock, &mut Money)>) {
    if !crate::road::road_food(&content) {
        return;
    }
    let (now, day) = (clock.now, content.day_ticks);
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        let fresh = state.arrived == Some(Leg::Out);
        let state = &mut *state;
        let Some(wagon) = state.wagon.as_mut() else { continue };
        if wagon.leg != Leg::Out || !(fresh || wagon.waiting_since.is_some()) {
            continue;
        }
        let food = food_needed(&content, route.blocks) * mouths(Some(wagon));
        let pace = if wagon.provisions >= food {
            // It brought its food for the road home.
            wagon.provisions = 0.0;
            state.food_eaten += food;
            1.0
        } else {
            let (mut stock, mut money) = towns.get_mut(route.to).expect("the far end");
            let meal = eat(&content, &mut ledger, &mut stock.0, food);
            if !meal.is_empty() {
                let cost: f64 = meal.iter().map(|&(g, n)| n * content.goods[g].value).sum();
                let spurs = mint.rate(now, day);
                let price = ((cost * spurs).ceil() / spurs).min(wagon.purse.max(0.0));
                post(&mut ledger, &mut cash, Side::wagon(route.seq, &mut wagon.purse), Side::treasury(route.to_id, &mut money.0),
                    price, Reason::RoadFood, now, day);
                state.food_paid += price;
                state.food_eaten += food;
                wagon.waiting_since = None;
                1.0
            } else {
                let since = *wagon.waiting_since.get_or_insert(now);
                if now - since < content.dials.max_wait_ticks {
                    continue; // waiting for food at the far end
                }
                // It won't sit there for days: home on short rations, foraging, at half the pace.
                wagon.waiting_since = None;
                state.short_rations += 1;
                2.0
            }
        };
        wagon.leg = Leg::Home;
        // At its pace (a saddled wagon a third faster: `squad.rs`), and half that on short rations.
        wagon.arrives = now + crate::squad::leg_ticks(&content, route.travel_ticks, wagon.saddled, pace);
    }
}
