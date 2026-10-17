//! How a town works out whether it holds a chokepoint, and what to ask there. Each wagon that crosses its land shows
//! it what guarding that land would do: a post where the wagon crossed, the way it would go instead, a post there too,
//! and so on, noting each time what going around the posts would cost the wagon (road food and other towns' tolls).
//! Land nobody can go around cheaply is a chokepoint: a bridge, a pass, the one dry hex. Every few days the town picks
//! the toll and posts that bring in the most after the guards' pay, asking no more than a share of each route's detour
//! so wagons keep coming; and it can ask what a bridge on a hex would bring in.
//!
//! Ported from the mod:
//! - `territory/Chokepoints.java`: `DETOUR_SHARE`, `MIN_RATE`, `MAX_POSTS_PER_ROUTE`, `Option`, `Crossing`
//!   (`bearableRate` 58, `fewestPostsFor` 63, `incomePerDay`), `Plan`, `bearableFee` (85), `bestPlan` (105: each rate
//!   some route would bear, the routes richest first, each route's fewest posts that bear it where its toll and fee beat
//!   the new guards' pay; the rate leaving the most after the guards).
//! - `caravan/Caravans.java`: `payTolls` (1912: each town crossed sees the wagon with what it still carries, before its
//!   own toll), `postOptions` (1981), `better` (2022: dearer way around, then more impassable neighbours, then nearer
//!   home), `impassableNeighbours`, `bridgeValue` (2083).
//! - `territory/Territory.java`: `observeCrossing` (1838: the route's value averaged with the last, its options
//!   replaced), `recordTraffic` (1829), `guardCostPerDay` (1854), `reviewTolls` (1939: every `TOLL_REVIEW_TICKS`, trips
//!   a day, routes quiet for `FORGET_AFTER_REVIEWS` forgotten, the plan; posts it no longer wants let go at once, one
//!   new guard a review), `PASSAGE_FEE_CAP` (a share of the mint's rate, in spurs).
//! - `territory/Smithing.java` `ARMED_DETOUR_SHARE`, `ARMOURED_TOLL_RATE`; `caravan/Route.java` `expectedValue` (218:
//!   spurs collected a trip, 300 before the first).
//!
//! Money here is in spurs, as the mod's coins are: a wagon's purse at the mint's rate, road food at bread's worth.
//!
//! **Where the core differs.** The mod's plan keeps its posts in an unordered set, so which new post a review hires
//! first isn't fixed there; here it's the first in the plan's order (the richest route's posts, in the order tried).
//! A wagon on a bare route (no road over the map) is seen by nobody, as it pays no tolls (`rules/territory.rs`);
//! [`Society::pass`] takes the straight line for a bare route, as the mod's `Route.path` does. With the
//! `toll_reviews` dial off, towns keep the posts and tolls the host gives them.

use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::content::Dials;
use crate::hex::Hex;
use crate::map::{Ground, Map, Path, Posts};
use crate::messages::{RouteId, TownId};
use crate::society::Society;
use crate::territory::{Holding, Territory};

/// Tolls below this aren't worth posting a guard for (`Chokepoints.MIN_RATE`).
pub const MIN_RATE: f64 = 0.005;
/// Armed guards: a town asks up to this share of the detour (`Smithing.ARMED_DETOUR_SHARE`).
pub const ARMED_DETOUR_SHARE: f64 = 0.95;
/// Armoured guards: a town's toll may go up to this (`Smithing.ARMOURED_TOLL_RATE`).
pub const ARMOURED_TOLL_RATE: f64 = 0.35;
/// Spurs a wagon on a route is guessed to carry home before its first trip (`Route.expectedValue`).
pub const FIRST_VALUE: f64 = 300.0;

/// Guards on these hexes would make a wagon go around at this extra cost, in spurs (infinite: no way at all).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PostOption {
    pub posts: Vec<Hex>,
    pub detour: f64,
}

/// What a town has seen of one route crossing its land: ever larger sets of posts (each the last one's hexes and one
/// more), the coins carried past a trip (averaged, in spurs), trips since the last review and a day over the last
/// period, and reviews in a row with no trip.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Crossing {
    pub route: RouteId,
    pub options: Vec<PostOption>,
    pub value: f64,
    pub trips_since: f64,
    pub trips_per_day: f64,
    pub quiet_reviews: u32,
}

impl Crossing {
    /// The most this route would pay at a set of posts, as a share of what it carries: `share` of the detour, at most
    /// `cap` (`Crossing.bearableRate`).
    pub fn bearable_rate(&self, option: &PostOption, share: f64, cap: f64) -> f64 {
        if self.value <= 0.0 {
            return 0.0;
        }
        cap.min(share * option.detour / self.value)
    }

    /// The fewest posts that make this route bear `rate`, if any (`Crossing.fewestPostsFor`).
    pub fn fewest_posts_for(&self, rate: f64, share: f64, cap: f64) -> Option<&PostOption> {
        self.options.iter().find(|o| self.bearable_rate(o, share, cap) >= rate - 1e-9)
    }

    pub fn income_per_day(&self, rate: f64) -> f64 {
        rate * self.value * self.trips_per_day
    }
}

/// The fee a wagon on this route would bear at these posts beyond the share of its coins: what's left of `share` of
/// the detour, at most `fee_cap` spurs (`Chokepoints.bearableFee`).
pub fn bearable_fee(crossing: &Crossing, option: &PostOption, rate: f64, share: f64, fee_cap: f64) -> f64 {
    let detour_share = if option.detour.is_infinite() { f64::INFINITY } else { share * option.detour };
    fee_cap.min(detour_share - rate * crossing.value).max(0.0)
}

/// What a plan asks of a route it counts on: the route, the coins it carries a trip, the detour of the posts it's
/// asked at (spurs), and the share of that detour the town may ask.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Asked {
    pub route: RouteId,
    pub value: f64,
    pub detour: f64,
    pub share: f64,
}

/// The toll and posts a town settles on (`Chokepoints.Plan`): a share of the coins each wagon carries, the posts in the
/// order the plan took them, the tolls a day it expects before the guards' pay (spurs), the passage fee a wagon
/// (spurs), and the routes it counts on.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TollPlan {
    pub rate: f64,
    pub posts: Vec<Hex>,
    pub income: f64,
    pub fee: f64,
    pub asked: Vec<Asked>,
}

/// One guard's daily cost in spurs: its pay plus its food at bread prices, a food point a thirtieth of an emerald
/// (`Territory.guardCostPerDay`).
pub fn guard_cost_per_day(dials: &Dials, spurs: f64) -> f64 {
    (dials.guard_pay + dials.guard_food / 30.0) * spurs
}

/// Tries each toll some route would bear (`Chokepoints.bestPlan`). At each, takes the routes richest first, posting
/// the guards each needs where its toll and fee beat the new guards' pay. Keeps the toll that leaves the most after
/// the guards' pay; no posts if none pays.
pub fn best_plan(crossings: &[Crossing], guard_cost: f64, share: f64, cap: f64, fee_cap: f64) -> TollPlan {
    let mut rates: Vec<f64> = crossings.iter().flat_map(|c| c.options.iter().map(|o| c.bearable_rate(o, share, cap))).collect();
    rates.sort_by(f64::total_cmp);
    rates.dedup();
    let mut richest: Vec<&Crossing> = crossings.iter().collect();
    richest.sort_by(|a, b| (b.value * b.trips_per_day).total_cmp(&(a.value * a.trips_per_day)));
    let mut best = TollPlan::default();
    let mut best_profit = 0.0;
    for rate in rates {
        if rate < MIN_RATE {
            continue;
        }
        let mut posts: Vec<Hex> = Vec::new();
        let mut income = 0.0;
        let mut fee = f64::INFINITY;
        let mut trips = 0.0;
        let mut asked = Vec::new();
        for crossing in &richest {
            let Some(option) = crossing.fewest_posts_for(rate, share, cap) else { continue };
            let added = option.posts.iter().filter(|x| !posts.contains(x)).count();
            let bearable = bearable_fee(crossing, option, rate, share, fee_cap);
            let gain = crossing.income_per_day(rate) + bearable * crossing.trips_per_day;
            if gain > added as f64 * guard_cost {
                for &x in &option.posts {
                    if !posts.contains(&x) {
                        posts.push(x);
                    }
                }
                income += crossing.income_per_day(rate);
                fee = fee.min(bearable); // what every wagon through will still bear
                trips += crossing.trips_per_day;
                asked.push(Asked { route: crossing.route, value: crossing.value, detour: option.detour, share });
            }
        }
        if fee.is_infinite() {
            fee = 0.0;
        }
        income += fee * trips;
        let profit = income - posts.len() as f64 * guard_cost;
        if profit > best_profit + 1e-9 {
            best = TollPlan { rate, posts, income, fee, asked };
            best_profit = profit;
        }
    }
    best
}

/// Hexes around `hex` no wagon can cross.
pub fn impassable_neighbours(map: &Map, hex: Hex) -> usize {
    hex.neighbors().iter().filter(|n| map.crossing_factor(**n).is_infinite()).count()
}

fn cost(path: &Option<Path>) -> f64 {
    path.as_ref().map_or(f64::INFINITY, |p| p.cost)
}

/// Is posting `hex` (leaving `around`) better for the town than posting `than` (leaving `around_than`)? A dearer way
/// around, then more impassable neighbours (narrower), then nearer home (`Caravans.better`).
fn better(map: &Map, around: &Option<Path>, hex: Hex, around_than: &Option<Path>, than: Hex, home: Hex) -> bool {
    let (a, b) = (cost(around), cost(around_than));
    if (a - b).abs() > 1e-6 {
        return a > b;
    }
    let (narrow, narrow_than) = (impassable_neighbours(map, hex), impassable_neighbours(map, than));
    if narrow != narrow_than {
        return narrow > narrow_than;
    }
    hex.distance(home) < than.distance(home)
}

/// What guarding `town`'s land would do to a wagon from `start` to `goal` expecting to carry `value` spurs home, at
/// `per_hex` spurs of road food a hex (`Caravans.postOptions`). The town tries a post on each of its hexes on the
/// wagon's cheapest path and keeps the one that makes going around dearest; then does the same on the cheapest path
/// avoiding the posts so far, until the path keeps off its land, there's no way at all, or `max_posts` posts. Costs
/// are with the town and the route's `ends` charging nothing; other towns' `posts` count.
#[allow(clippy::too_many_arguments)]
pub fn post_options(map: &Map, territory: &Territory, posts: &Posts, town: TownId, ends: &[TownId], start: Hex,
        goal: Hex, home: Hex, per_hex: f64, value: f64, max_posts: usize) -> Vec<PostOption> {
    let mut free: BTreeSet<TownId> = ends.iter().copied().collect();
    free.insert(town);
    let none = BTreeSet::new();
    let Some(through) = map.plan(start, goal, per_hex, value, posts, &free, false, &none) else { return Vec::new() };
    let mut options = Vec::new();
    let mut tried: Vec<Hex> = Vec::new();
    let mut path = Some(through.clone());
    while let Some(p) = &path {
        if tried.len() >= max_posts {
            break;
        }
        let land: Vec<Hex> = p.hexes.iter().copied()
            .filter(|&x| x != start && x != goal && territory.owner(x) == Some(town))
            .collect();
        if land.is_empty() {
            break; // this path keeps off the town's land
        }
        let mut pick: Option<(Hex, Option<Path>)> = None;
        for hex in land {
            let mut trial: BTreeSet<Hex> = tried.iter().copied().collect();
            trial.insert(hex);
            let around = map.plan(start, goal, per_hex, value, posts, &free, false, &trial);
            let take = match &pick {
                None => true,
                Some((than, around_than)) => better(map, &around, hex, around_than, *than, home),
            };
            if take {
                pick = Some((hex, around));
            }
        }
        let (hex, around) = pick.expect("land on the path");
        tried.push(hex);
        let detour = around.as_ref().map_or(f64::INFINITY, |a| (a.cost - through.cost).max(0.0));
        options.push(PostOption { posts: tried.clone(), detour });
        path = around;
    }
    options
}

impl Holding {
    /// A wagon on `route` carrying `spurs` crossed this town's land; `options` is what going around ever larger sets of
    /// posts on its land would have cost it (`Territory.observeCrossing`, `recordTraffic`).
    pub fn observe_crossing(&mut self, route: RouteId, spurs: f64, options: Vec<PostOption>) {
        self.traffic_since += spurs;
        let at = match self.crossings.iter().position(|c| c.route == route) {
            Some(i) => i,
            None => {
                self.crossings.push(Crossing { route, ..Default::default() });
                self.crossings.len() - 1
            }
        };
        let c = &mut self.crossings[at];
        c.value = if c.value == 0.0 { spurs } else { (c.value + spurs) / 2.0 };
        c.trips_since += 1.0;
        c.options = options;
    }

    /// Every `toll_review_days` the town looks at the routes that crossed its land and settles its toll and the hexes
    /// to guard (`Territory.reviewTolls`): posts it no longer wants are let go at once, one new guard a review; with no
    /// post worth a guard it keeps its toll for later. The first call only starts the clock. `spurs`: the mint's rate.
    /// True if it reviewed.
    pub fn review_tolls(&mut self, now: u64, day_ticks: u64, spurs: f64, dials: &Dials) -> bool {
        let Some(last) = self.last_toll_review else {
            self.last_toll_review = Some(now);
            return false;
        };
        let elapsed = now.saturating_sub(last) as f64;
        if elapsed < dials.toll_review_days * day_ticks as f64 || elapsed <= 0.0 {
            return false;
        }
        let days = elapsed / day_ticks as f64;
        self.traffic_per_day = self.traffic_since / days;
        self.traffic_since = 0.0;
        for c in &mut self.crossings {
            c.trips_per_day = c.trips_since / days;
            c.quiet_reviews = if c.trips_since > 0.0 { 0 } else { c.quiet_reviews + 1 };
            c.trips_since = 0.0;
        }
        self.crossings.retain(|c| c.quiet_reviews < dials.forget_after_reviews);
        // Armed guards ask more of the detour, armoured ones may ask a higher toll.
        let share = dials.detour_share + (ARMED_DETOUR_SHARE - dials.detour_share) * self.armed;
        let cap = dials.max_toll_rate + (ARMOURED_TOLL_RATE - dials.max_toll_rate) * self.armoured;
        let plan = best_plan(&self.crossings, guard_cost_per_day(dials, spurs), share, cap, dials.passage_fee_cap * spurs);
        self.posts.retain(|x| plan.posts.contains(x));
        if let Some(&new) = plan.posts.iter().find(|x| !self.posts.contains(x)) {
            self.posts.push(new);
        }
        if !plan.posts.is_empty() {
            self.toll_rate = Some(plan.rate);
            self.passage_fee = plan.fee;
        }
        self.toll_plan = plan;
        self.last_toll_review = Some(now);
        true
    }
}

/// Spurs each route's wagons have brought home, and their trips: what a wagon on it is expected to carry
/// (`Route.spursCollected`, `trips`).
#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Takings(pub BTreeMap<RouteId, (u32, f64)>);

impl Takings {
    /// Spurs a wagon on the route has carried home a trip, on average; [`FIRST_VALUE`] before its first
    /// (`Route.expectedValue`).
    pub fn expected_value(&self, route: RouteId) -> f64 {
        match self.0.get(&route) {
            Some(&(trips, spurs)) if trips > 0 => spurs / trips as f64,
            _ => FIRST_VALUE,
        }
    }

    pub fn add(&mut self, route: RouteId, spurs: f64) {
        let t = self.0.entry(route).or_default();
        t.0 += 1;
        t.1 += spurs;
    }
}

/// A wagon on route `route` between `ends`, carrying `carried` spurs over `path` from `start` to `goal`: each town
/// whose land it crosses (the ends excepted), in path order, sees it go by with what it still carries after the tolls
/// before, and what going around ever larger sets of posts on its land would cost it (`Caravans.payTolls`). Returns
/// the tolls due, as [`Territory::tolls_due`]. `home`: a town's home hex.
#[allow(clippy::too_many_arguments)]
pub fn observe(territory: &mut Territory, map: &Map, dials: &Dials, route: RouteId, ends: [TownId; 2], start: Hex,
        goal: Hex, path: &[Hex], carried: f64, per_hex: f64, value: f64, home: &dyn Fn(TownId) -> Option<Hex>)
        -> Vec<(TownId, f64)> {
    let due = territory.tolls_due(&ends, path, carried, dials.default_toll_rate);
    let posts = territory.toll_posts(dials.default_toll_rate);
    let mut crossed: Vec<TownId> = Vec::new();
    for &hex in path {
        if let Some(owner) = territory.owner(hex).filter(|o| !ends.contains(o) && !crossed.contains(o)) {
            crossed.push(owner);
        }
    }
    let mut paid = 0.0;
    for owner in crossed {
        let left = carried - paid;
        let home = home(owner).or_else(|| territory.holding(owner).and_then(|h| h.hexes.first().copied())).unwrap_or(start);
        let options = post_options(map, territory, &posts, owner, &ends, start, goal, home, per_hex, value,
            dials.max_posts_per_route as usize);
        territory.holdings.entry(owner).or_default().observe_crossing(route, left, options);
        paid += due.iter().filter(|(o, _)| *o == owner).map(|(_, t)| t).sum::<f64>();
    }
    due
}

/// What a bridge on a hex would bring its town a day (`Caravans.BridgeValue`): tolls a day in spurs, and the
/// neighbours the busiest route would cross between (for the deck).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BridgeValue {
    pub hex: Hex,
    pub tolls_per_day: f64,
    pub from: Option<Hex>,
    pub to: Option<Hex>,
}

/// A route as a bridge's worth reads it: its two towns, where it starts and ends, the spurs a wagon carries home.
#[derive(Clone, Copy, Debug)]
pub struct RouteEnds {
    pub ends: [TownId; 2],
    pub start: Hex,
    pub goal: Hex,
    pub value: f64,
}

/// What a bridge on `hex` would bring `town` a day (`Caravans.bridgeValue`): on each route it would carry (not the
/// town's own), the lesser of the most it may ask (`max_rate` of the wagon's coins) and `share` of what the bridge
/// saves over the cheapest way without it, the town's own tolls left out (a route with no way at all would save its
/// whole value), one trip a day. A second bridge beside the first is worth only the shorter way it opens.
#[allow(clippy::too_many_arguments)]
pub fn bridge_value(map: &Map, posts: &Posts, routes: &[RouteEnds], town: TownId, hex: Hex, per_hex: f64,
        max_rate: f64, share: f64) -> BridgeValue {
    let mut bridged = map.clone();
    bridged.ground.insert(hex, Ground::default()); // open ground, whatever it was
    let none = BTreeSet::new();
    let mut out = BridgeValue { hex, ..Default::default() };
    let mut best = 0.0;
    for r in routes {
        if r.ends.contains(&town) || hex == r.start || hex == r.goal {
            continue;
        }
        // The town's own posts count for nothing: a bridge that lets wagons dodge its other posts gains it nothing.
        let free: BTreeSet<TownId> = [r.ends[0], r.ends[1], town].into_iter().collect();
        let without = map.plan(r.start, r.goal, per_hex, r.value, posts, &free, false, &none);
        let Some(with) = bridged.plan(r.start, r.goal, per_hex, r.value, posts, &free, false, &none) else { continue };
        let Some(at) = with.hexes.iter().position(|&x| x == hex) else { continue };
        if at == 0 || at + 1 >= with.hexes.len() {
            continue; // the bridge wouldn't be on this route's way
        }
        let saves = without.map_or(r.value, |w| w.cost - with.cost);
        let toll = (max_rate * r.value).min(share * saves);
        out.tolls_per_day += toll.max(0.0);
        if toll > best {
            best = toll;
            out.from = Some(with.hexes[at - 1]);
            out.to = Some(with.hexes[at + 1]);
        }
    }
    out
}

/// Host calls.
impl Society {
    /// The toll posts as a wagon's planner sees them: each town's guarded hexes, at its rate and passage fee.
    pub fn toll_posts(&self) -> Posts {
        self.territory().toll_posts(self.content().dials.default_toll_rate)
    }

    /// Spurs a wagon on the route is expected to carry home: its takings a trip, 300 before its first.
    pub fn expected_value(&self, route: RouteId) -> f64 {
        self.world.resource::<Takings>().expected_value(route)
    }

    /// A wagon on the route carrying `carried` spurs passes over its road (the straight line, on a bare route), at
    /// `per_hex` spurs of road food a hex: each town whose land it crosses sees it go by, and what going around posts
    /// there would cost it (`Caravans.payTolls`). Returns the tolls the posted towns would take, in spurs; a wagon
    /// coming home pays them out of its purse (`rules/territory.rs`). Nothing if an end isn't placed.
    pub fn pass(&mut self, route: RouteId, carried: f64, per_hex: f64) -> Vec<(TownId, f64)> {
        let r = *self.world.get::<crate::state::Route>(self.routes[route]).expect("a route");
        let (Some(start), Some(goal)) = (self.town_hex(r.from_id), self.town_hex(r.to_id)) else { return Vec::new() };
        let path = self.route_path(route).unwrap_or_else(|| Hex::line(start, goal));
        let value = self.expected_value(route);
        let homes: Vec<Option<Hex>> = (0..self.town_count()).map(|t| self.town_hex(t)).collect();
        let dials = self.content().dials.clone();
        self.world.resource_scope(|world, mut territory: bevy_ecs::prelude::Mut<Territory>| {
            let map = world.resource::<Map>();
            observe(&mut territory, map, &dials, r.seq, [r.from_id, r.to_id], start, goal, &path, carried, per_hex, value,
                &|t| homes.get(t).copied().flatten())
        })
    }

    /// The town reviews its tolls now if `toll_review_days` have passed since its last review (the first call only
    /// starts the clock), at the mint's rate. True if it reviewed.
    pub fn review_tolls(&mut self, town: TownId) -> bool {
        let (now, day) = (self.now(), self.content().day_ticks);
        let spurs = self.mint_rate();
        let dials = self.content().dials.clone();
        self.world.resource_mut::<Territory>().holdings.entry(town).or_default().review_tolls(now, day, spurs, &dials)
    }

    /// What the town has seen of the routes crossing its land.
    pub fn crossings(&self, town: TownId) -> Vec<Crossing> {
        self.holding(town).crossings
    }

    /// The toll plan the town settled on at its last review.
    pub fn toll_plan(&self, town: TownId) -> TollPlan {
        self.holding(town).toll_plan
    }

    /// The town's passage fee at its posts, in spurs (none without a post: `Territory.passageFee`).
    pub fn passage_fee(&self, town: TownId) -> f64 {
        let h = self.holding(town);
        if h.posts.is_empty() { 0.0 } else { h.passage_fee }
    }

    /// Shares of the town's guards armed and armoured, 0 to 1 (a host's, until `mines-and-kit` keeps them).
    pub fn set_guard_kit(&mut self, town: TownId, armed: f64, armoured: f64) {
        let mut t = self.world.resource_mut::<Territory>();
        let h = t.holdings.entry(town).or_default();
        h.armed = armed.clamp(0.0, 1.0);
        h.armoured = armoured.clamp(0.0, 1.0);
    }

    /// What a bridge on `hex` would bring `town` a day in spurs, over the routes between placed towns, at the mint's
    /// rate (`Caravans.bridgeValue`).
    pub fn bridge_value(&self, town: TownId, hex: Hex) -> BridgeValue {
        let spurs = self.mint_rate();
        let takings = self.world.resource::<Takings>();
        let routes: Vec<RouteEnds> = self.routes.iter().filter_map(|&e| {
            let r = self.world.get::<crate::state::Route>(e)?;
            Some(RouteEnds { ends: [r.from_id, r.to_id], start: self.town_hex(r.from_id)?, goal: self.town_hex(r.to_id)?,
                value: takings.expected_value(r.seq) })
        }).collect();
        let d = &self.content().dials;
        bridge_value(self.map(), &self.toll_posts(), &routes, town, hex, crate::map::per_hex_at_bread(self.content()) * spurs, d.max_toll_rate,
            d.detour_share)
    }
}
