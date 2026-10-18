//! Road food: food is the road's cost (decision 6 of `docs/core-plan.md`), as the mod's caravans eat
//! (`caravan/Caravans.java`: `foodNeeded`, `inRange`, `travelTicks`; `dispatch`'s crew against food, `fedThere` and
//! `provisions`, `roadPurse`, the funding check, `fromHome` and `foodCoin` and the withdrawals; `foodKnownAt`;
//! `arriveAtDestination`'s food tail: `stock.eat`, `waitingSince`, `MAX_WAIT_TICKS`, half pace; `CaravanLedger.
//! foodPromisedAt`; `merchant/MerchantStock.java`: `eat`, `foodNutrition`, `nutrition`, 0 for comforts).
//!
//! A wagon eats a food point every `blocks_per_food` blocks per mouth, a leg at a time, and carries at most
//! `food_capacity` points (a stack of bread: a leg is at most 3,200 blocks). It eats the way out's food from home's
//! warehouse as it leaves (bought on the way, at `purse_per_food_point`, what home hasn't got) and carries a purse to
//! buy the road home's at the far end, which sells it at its worth into the far end's treasury. Where the far end may
//! not feed it (its food, less what wagons already bound there will eat, is short), or home is too poor to pay for it
//! but rich in food, it packs the road home's food too. No food at the far end: it waits, and after `max_wait_ticks`
//! sets off home on short rations at half pace. Home that can't pay for the purse and the food it lacks: it stays.
//!
//! Only for content that has food (a good with food points that isn't a comfort, [`road_food`]): without, the road's
//! cost is the route's `road_cost` in money, as before.
//!
//! The wagon's team eats a mouth's food in fodder each leg, from home's hay and wheat it doesn't keep, else bought on the
//! road (`teamFeed`, `fodderOnHand`, `eatFodder`, `teamCoin`). Every one of a wagon's crew is a mouth (`squad.rs`; one
//! for a town without a roster). Not yet: the wheat kept for a contract's inputs or a realm (`wantedAt`: here only its
//! trades and its workshop), the crown's food reserve (`spareFoodAt`), buying food at the far end's shops
//! (`buyFoodAtShops`), stocktakes (the far end's food is read off its warehouse now; the mod's unknown stocktake counts
//! as plenty).

use serde::{Deserialize, Serialize};

use crate::content::{Content, GoodId};
use crate::messages::{RouteId, TownId};
use crate::society::Society;
use crate::state::*;

/// A town keeps this many food points for wagons' road food before another mouth goes (`Warehouse.FOOD_PAR_POINTS`);
/// a town that packs its own road home's food spares it beyond this.
pub const FOOD_PAR_POINTS: f64 = 320.0;
/// Ticks a wagon takes a block (`Caravans.TICKS_PER_BLOCK`): 1,200 blocks a 24,000-tick day.
pub const TICKS_PER_BLOCK: u64 = 20;

/// Fodder points a bale of hay is worth to a wagon's team (`Caravans.HAY_POINTS`); a wheat is one.
pub const HAY_POINTS: f64 = 9.0;

/// Does home keep its wheat from the team (`keepWheat`: `wantedAt` holds it, its trades' or its workshop's)?
pub fn keep_wheat(content: &Content, trades: &Trades, roster: Option<&crate::roster::Roster>) -> bool {
    let Some(wheat) = content.good("wheat") else { return false };
    trades.uses[wheat] > 0.0 || content.conversions.iter().any(|c| c.inputs.iter().any(|&(g, _)| g == wheat)
        && roster.is_some_and(|r| crate::workshop::can_do(c, r)))
}

/// What the team finds of `points` of fodder at home, hay first, whole bales and whole wheat (`eatFodder`): bales and
/// wheat taken, and the points.
pub fn fodder(content: &Content, stock: &[f64], points: f64, keep_wheat: bool) -> (f64, f64, f64) {
    let whole = |g: Option<GoodId>| g.map_or(0.0, |g| stock[g].floor().max(0.0));
    let bales = (points / HAY_POINTS).floor().min(whole(content.good("hay_block")));
    let wheat = if keep_wheat { 0.0 } else { (points - bales * HAY_POINTS).min(whole(content.good("wheat"))).max(0.0) };
    (bales, wheat, bales * HAY_POINTS + wheat)
}

/// The team eats its fodder from home's warehouse (as [`fodder`] found it), into the ledger's `used`.
pub fn eat_fodder(content: &Content, ledger: &mut Ledger, stock: &mut [f64], points: f64, keep_wheat: bool) {
    let (bales, wheat, _) = fodder(content, stock, points, keep_wheat);
    for (g, n) in [(content.good("hay_block"), bales), (content.good("wheat"), wheat)] {
        if let (Some(g), true) = (g, n > 0.0) {
            stock[g] -= n;
            ledger.used[g] += n;
        }
    }
}

/// Why a wagon stayed home this look.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stay {
    /// The leg is longer than a wagon's food will carry it ("the cheapest path is too long for one leg's food").
    OutOfRange,
    /// Home can't pay for the road and the purse ("the hub can't fund the trip").
    CantFund,
    /// Not full nor on its day, or its load isn't worth its road food ("waiting for a fuller load").
    WaitingForALoad,
    /// Nobody at home to lead it, and no hand to take on (`squad.rs`: "no squad at home").
    NoSquad,
    /// A town with a roster and no Merchant at its desk sends no wagon (`squad.rs`: "no Merchant at the hub").
    NoMerchant,
}

/// The food points one of a good is worth on the road: none for a comfort (`MerchantStock.nutrition`: treats aren't
/// road food).
pub fn points(content: &Content, good: GoodId) -> f64 {
    let g = &content.goods[good];
    if g.comfort.is_some() { 0.0 } else { g.food.max(0.0) }
}

/// Does the host's content have road food? If not, the road's cost is money (`Route.road_cost`).
pub fn road_food(content: &Content) -> bool {
    (0..content.goods.len()).any(|g| points(content, g) > 0.0)
}

/// Food points a mouth eats on a leg of `blocks` (`Caravans.foodNeeded`): one per `blocks_per_food`, at least one.
pub fn food_needed(content: &Content, blocks: u32) -> f64 {
    let per = content.dials.blocks_per_food.max(1);
    blocks.div_ceil(per).max(1) as f64
}

/// Can a wagon carry a leg's food (`Caravans.inRange`)? 3,200 blocks it can, 3,210 it can't.
pub fn in_range(content: &Content, blocks: u32) -> bool {
    food_needed(content, blocks) <= content.dials.food_capacity as f64
}

/// Ticks a leg of `blocks` takes (`Caravans.travelTicks`): 20 a block, at least 20.
pub fn travel_ticks(blocks: u32) -> u64 {
    (blocks as u64 * TICKS_PER_BLOCK).max(20)
}

/// Mouths on a wagon: its crew, at least one (`Caravan.mouths`; a town without a roster sends one).
pub fn mouths(wagon: Option<&Wagon>) -> f64 {
    wagon.map_or(1, |w| w.crew.len().max(1)) as f64
}

/// Food points in a warehouse, whole items of road food (`MerchantStock.foodNutrition`).
pub fn held(content: &Content, stock: &[f64]) -> f64 {
    (0..content.goods.len()).map(|g| stock[g].floor().max(0.0) * points(content, g)).sum()
}

/// Eats at least `points` of road food from a warehouse, whole items, the cheapest a point first (`MerchantStock.eat`):
/// what was eaten of each good, into the ledger's `used`. Nothing at all if it holds too little.
pub fn eat(content: &Content, ledger: &mut Ledger, stock: &mut [f64], points: f64) -> Vec<(GoodId, f64)> {
    let mut eaten = Vec::new();
    if points <= 0.0 || held(content, stock) < points {
        return eaten;
    }
    let mut order: Vec<GoodId> = (0..content.goods.len()).filter(|&g| self::points(content, g) > 0.0).collect();
    order.sort_by(|&a, &b| {
        let each = |g: GoodId| content.goods[g].value / self::points(content, g);
        each(a).total_cmp(&each(b)).then(a.cmp(&b))
    });
    let mut got = 0.0;
    for g in order {
        if got >= points {
            break;
        }
        let each = self::points(content, g);
        let n = stock[g].floor().max(0.0).min(((points - got) / each).ceil());
        if n <= 0.0 {
            continue;
        }
        stock[g] -= n;
        ledger.used[g] += n;
        got += n * each;
        eaten.push((g, n));
    }
    eaten
}

/// What a wagon's road takes from home's money as it leaves, for the plan to leave room for it (the mod's `spare`
/// less `roadPurse`; and the food and fodder bought on the road, which the mod's leaves out, so a plan that fills the
/// purse never keeps the wagon home): where content has food, the road home's purse, the way out's food home hasn't
/// got and the team's fodder it hasn't; else the route's `road_cost`.
/// `mouths`: the crew it plans to send (`squad.rs`), one without.
pub fn road_money(content: &Content, route: &Route, home_stock: &[f64], keep_wheat: bool, mouths: f64) -> crate::fixed::Fixed {
    if !road_food(content) {
        return route.road_cost;
    }
    let ppfp = content.dials.purse_per_food_point;
    let food = food_needed(content, route.blocks) * mouths;
    let team = 2.0 * food_needed(content, route.blocks) * content.dials.wagon_team as f64;
    let fed = fodder(content, home_stock, team, keep_wheat).2;
    // Rounded up to the thousandth: a cost (`fixed.rs`).
    crate::fixed::Fixed::ceil((food + (food - held(content, home_stock)).max(0.0) + team - fed) * ppfp)
}

/// What a wagon at the far end keeps of its purse for the road home's food, paying advances there (`payAdvances`'s
/// `keep`): none where content has no road food.
/// `mouths`: its crew's.
pub fn purse_keep(content: &Content, route: &Route, mouths: f64) -> crate::fixed::Fixed {
    if road_food(content) {
        // Rounded up, as the mod's `Math.ceil(... * rate)`.
        crate::fixed::Fixed::ceil(food_needed(content, route.blocks) * mouths * content.dials.purse_per_food_point)
    } else {
        crate::fixed::Fixed::ZERO
    }
}

/// A route's road food as a host sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct RoadFoodView {
    /// The road's length in blocks, and the food a leg takes (all mouths).
    pub blocks: u32,
    pub food_a_leg: f64,
    /// What its wagon on the road carries for the road home, and since when it has waited at the far end for food.
    pub provisions: f64,
    pub waiting_since: Option<u64>,
    /// When its wagon gets home, if it's on the way.
    pub home_at: Option<u64>,
    pub short_rations: u32,
    pub food_eaten: f64,
    pub food_paid: crate::fixed::Fixed,
    pub stayed: Option<Stay>,
}

impl Society {
    /// A run from one town to another as [`Society::add_route`], refused where content has road food and a leg of
    /// `days` (at 1,200 blocks a day) is more than a wagon's food will carry it.
    pub fn try_add_route(&mut self, from: TownId, to: TownId, days: f64, road_cost: impl Into<crate::fixed::Fixed>, every_days: f64)
            -> Result<RouteId, String> {
        let blocks = (days * crate::map::BLOCKS_PER_DAY).round().max(0.0) as u32;
        let content = self.content();
        if road_food(content) && !in_range(content, blocks) {
            return Err(format!("{blocks} blocks is too long for one leg's food ({} points, {} at most)",
                food_needed(content, blocks), content.dials.food_capacity));
        }
        Ok(self.add_route(from, to, days, road_cost, every_days))
    }

    /// A route's road food: its leg's food, its wagon's provisions and wait, and what it has eaten and paid.
    pub fn road_food(&self, route: RouteId) -> RoadFoodView {
        let e = self.world.entity(self.routes[route]);
        let r = e.get::<Route>().expect("a route");
        let s = e.get::<RouteState>().expect("a route");
        let w = s.wagon.as_ref();
        RoadFoodView {
            blocks: r.blocks,
            food_a_leg: food_needed(self.content(), r.blocks) * mouths(w),
            provisions: w.map_or(0.0, |w| w.provisions),
            waiting_since: w.and_then(|w| w.waiting_since),
            home_at: w.filter(|w| w.leg == Leg::Home).map(|w| w.arrives),
            short_rations: s.short_rations,
            food_eaten: s.food_eaten,
            food_paid: s.food_paid,
            stayed: s.stayed,
        }
    }

    /// Road food points in a town's warehouse (whole items, comforts not counted).
    pub fn food_points(&self, town: TownId) -> f64 {
        held(self.content(), &self.town(town).stock)
    }
}
