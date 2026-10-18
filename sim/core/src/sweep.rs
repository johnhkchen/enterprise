//! The random worlds' sweep (`docs/core-plan.md` 5.1): what a host reads to say what a world did and to check it never
//! stalls. Nothing here is a rule or state the save keeps: a route's whole state as a host may read it, and a tally of
//! worlds (trips, contracts by how they stand, mood's range, wages paid and taken in kind, route-days idle by why,
//! the mechanisms switched on) that `sim/host --sweep N` adds up over seeds and prints.
//!
//! No Java: it hardens every slice's claims. The mood floor ([`mood_floor`]) is `Territory.moodTarget` (1587) with
//! every term that can lift it at nothing: the lowest a town's mood can head for, given its people now.

use std::collections::BTreeMap;

use crate::contracts::State;
use crate::messages::{RouteId, TownId};
use crate::society::Society;
use crate::sourcing::Phase;
use crate::state::{Route, RouteState};

impl Society {
    /// A route and everything it has done, as they stand.
    pub fn route_state(&self, route: RouteId) -> (&Route, &RouteState) {
        let e = self.world.entity(self.routes[route]);
        (e.get::<Route>().expect("a route"), e.get::<RouteState>().expect("a route"))
    }

    /// The lowest a town's mood can head for, given its people now ([`mood_floor`]).
    pub fn mood_floor(&self, town: TownId) -> f64 {
        mood_floor(&self.content().dials, &self.town(town).people)
    }
}

/// The lowest mood target a town's people can have (`Territory.moodTarget` with no comforts met, no works, no
/// blessing, its wages unpaid and its grief at the most): its size and its villagers without beds. Mood heads toward
/// its target, so it could fall below this only while the town has just shrunk (a lag of days, not a slide).
pub fn mood_floor(d: &crate::content::Dials, people: &crate::state::People) -> f64 {
    -people.villagers / d.villagers_per_want
        - d.crowding_mood * (people.villagers - people.beds).max(0.0)
        - d.unpaid_mood
        - d.grief_most
}

/// Why a route home sat idle a day, as the sweep counts it.
pub fn idle_label(state: &RouteState, route: &Route) -> &'static str {
    if state.freight.wound_down.is_some() {
        return "wound down for a hub";
    }
    if route.on_call {
        return "on call";
    }
    match state.stayed {
        Some(stay) => crate::atlas::stay_label(stay),
        None => "not planned yet",
    }
}

/// What a sweep of worlds did, added up day by day ([`Sweep::day`]) and at each world's end ([`Sweep::end`]).
#[derive(Clone, Debug, Default)]
pub struct Sweep {
    pub worlds: u32,
    pub days: u64,
    /// Towns, routes (with a road over the map among them) and trips at the worlds' ends.
    pub towns: u32,
    pub routes: u32,
    pub roads: u32,
    pub trips: u64,
    /// Items carried out and sold at the far ends, in all.
    pub carried: f64,
    pub sold: f64,
    /// Contracts at the worlds' ends: drafted, in force, completed, broken; and of those in force, the ones past their
    /// last installment, every installment judged (the mod leaves a contract short of its total in force: see
    /// `ContractBook.review`).
    pub contracts: [u32; 4],
    pub standing_short: u32,
    /// Contracts with a harvest term, a training term, a levy (at the ends).
    pub harvests: u32,
    pub trainings: u32,
    pub levies: u32,
    /// Mood: the lowest and highest any town had on any day, and each town's at its world's end.
    pub mood_lowest: f64,
    pub mood_highest: f64,
    pub moods: Vec<f64>,
    /// Town-days with mood below -8, and of those the ones crowding (villagers without beds) explains.
    pub low_mood_days: u64,
    pub crowded_low_days: u64,
    /// Farm wages paid in emeralds and owed past credit (taken in crops), at the ends.
    pub wages_paid: f64,
    pub wages_unpaid: f64,
    /// Route-days home by why the route stayed ([`idle_label`]), and route-days on the road.
    pub idle: BTreeMap<&'static str, u64>,
    pub road_days: u64,
    /// The longest a wagon had been out (days since it left) on any day.
    pub longest_out: f64,
    /// Mechanisms on at the ends: towns with a farm, banks, hubs with a storehouse, off-map places; worlds whose crown
    /// was at war (world-days); soldiers serving.
    pub farms: u32,
    pub banks: u32,
    pub hubs: u32,
    pub off_map: u32,
    pub war_days: u64,
    pub soldiers: u32,
    /// The paper at the ends (`cargo.rs`): documents drawn, of them bills of lading, scribes' and clerks' fees in
    /// emeralds, and pallets packed (`pallet.rs`: the registry's ids handed out).
    pub documents: u64,
    pub ladings: u64,
    pub scribe_fees: f64,
    pub pallets: u64,
}

impl Sweep {
    pub fn new() -> Self {
        Sweep { mood_lowest: f64::INFINITY, mood_highest: f64::NEG_INFINITY, ..Default::default() }
    }

    /// A day of a world, at its end.
    pub fn day(&mut self, s: &Society) {
        self.days += 1;
        let d = &s.content().dials;
        for t in 0..s.town_count() {
            let p = s.town(t).people;
            self.mood_lowest = self.mood_lowest.min(p.mood);
            self.mood_highest = self.mood_highest.max(p.mood);
            if p.mood < -8.0 {
                self.low_mood_days += 1;
                // Crowding explains it where without the villagers who have no beds the floor is above -8.
                self.crowded_low_days += (mood_floor(d, &p) + d.crowding_mood * (p.villagers - p.beds).max(0.0)
                    > -8.0) as u64;
            }
        }
        let now = s.now();
        let day = s.content().day_ticks;
        for r in 0..s.route_count() {
            let (route, state) = s.route_state(r);
            if state.wagon.is_some() {
                self.road_days += 1;
                let out = now.saturating_sub(state.last_departure.unwrap_or(0)) as f64 / day as f64;
                self.longest_out = self.longest_out.max(out);
            } else {
                *self.idle.entry(idle_label(state, route)).or_insert(0) += 1;
            }
        }
        self.war_days += (s.sovereign().phase == Phase::War) as u64;
    }

    /// A world's end.
    pub fn end(&mut self, s: &Society) {
        self.worlds += 1;
        self.towns += s.town_count() as u32;
        self.routes += s.route_count() as u32;
        let now = s.now();
        let grace = (crate::contracts::GRACE_DAYS * s.content().day_ticks as f64) as u64;
        for r in 0..s.route_count() {
            let v = s.route(r);
            self.trips += v.trips as u64;
            self.carried += v.carried;
            self.sold += v.sold;
            self.roads += s.route_path(r).is_some() as u32;
        }
        for c in s.contracts() {
            self.contracts[match c.state {
                State::Drafted => 0,
                State::Active => 1,
                State::Completed => 2,
                State::Broken => 3,
            }] += 1;
            let last = c.start + c.every * c.installments.saturating_sub(1) as u64 + c.wait_ticks(grace, s.content().day_ticks);
            self.standing_short += (c.in_force() && now > last && c.judged >= c.installments) as u32;
            self.harvests += c.harvest.is_some() as u32;
            self.trainings += c.training.is_some() as u32;
            self.levies += c.levy.is_some() as u32;
        }
        for t in 0..s.town_count() {
            self.moods.push(s.town(t).people.mood);
            if let Some(f) = s.farm(t) {
                self.farms += 1;
                self.wages_paid += f.farm.wages_paid.to_f64();
                self.wages_unpaid += f.farm.wages_unpaid.to_f64();
            }
            self.banks += s.is_bank(t) as u32;
            self.hubs += s.hub(t).storehouse as u32;
            self.off_map += s.is_off_map(t) as u32;
        }
        self.soldiers += s.army().soldiers.len() as u32;
        for t in 0..s.town_count() {
            let p = s.paperwork(t);
            self.documents += p.documents;
            self.ladings += p.by_kind.get(crate::cargo::LADING).copied().unwrap_or(0);
            self.scribe_fees += p.scribe_fees.to_f64();
        }
        self.pallets += s.pallets().next;
    }

    /// Two sweeps' tallies as one (seeds run apart).
    pub fn merge(&mut self, o: Sweep) {
        self.worlds += o.worlds;
        self.days += o.days;
        self.towns += o.towns;
        self.routes += o.routes;
        self.roads += o.roads;
        self.trips += o.trips;
        self.carried += o.carried;
        self.sold += o.sold;
        for i in 0..4 {
            self.contracts[i] += o.contracts[i];
        }
        self.standing_short += o.standing_short;
        self.harvests += o.harvests;
        self.trainings += o.trainings;
        self.levies += o.levies;
        self.mood_lowest = self.mood_lowest.min(o.mood_lowest);
        self.mood_highest = self.mood_highest.max(o.mood_highest);
        self.moods.extend(o.moods);
        self.low_mood_days += o.low_mood_days;
        self.crowded_low_days += o.crowded_low_days;
        self.wages_paid += o.wages_paid;
        self.wages_unpaid += o.wages_unpaid;
        for (k, v) in o.idle {
            *self.idle.entry(k).or_insert(0) += v;
        }
        self.road_days += o.road_days;
        self.longest_out = self.longest_out.max(o.longest_out);
        self.farms += o.farms;
        self.banks += o.banks;
        self.hubs += o.hubs;
        self.off_map += o.off_map;
        self.war_days += o.war_days;
        self.soldiers += o.soldiers;
        self.documents += o.documents;
        self.ladings += o.ladings;
        self.scribe_fees += o.scribe_fees;
        self.pallets += o.pallets;
    }

    /// The tally in words, a line a topic.
    pub fn report(&self) -> String {
        let mut moods = self.moods.clone();
        moods.sort_by(f64::total_cmp);
        let q = |f: f64| moods.get(((moods.len() as f64 - 1.0) * f).round().max(0.0) as usize).copied().unwrap_or(0.0);
        let route_days: u64 = self.idle.values().sum::<u64>() + self.road_days;
        let mut out = String::new();
        out += &format!("{} worlds, {} days: {} towns, {} routes ({} on roads over the map)\n", self.worlds, self.days,
            self.towns, self.routes, self.roads);
        out += &format!("trips: {} ({:.1} a route), {:.0} items carried, {:.0} sold at the far ends\n", self.trips,
            self.trips as f64 / self.routes.max(1) as f64, self.carried, self.sold);
        out += &format!("contracts at the end: {} drafted, {} in force ({} past their last installment, all judged, \
            short), {} completed, {} broken; {} harvests, {} trainings, {} levies\n", self.contracts[0], self.contracts[1],
            self.standing_short, self.contracts[2], self.contracts[3], self.harvests, self.trainings, self.levies);
        out += &format!("mood: {:.2} to {:.2} on any day; at the end {:.2} / {:.2} / {:.2} / {:.2} / {:.2} (min, \
            quartiles, max); {} town-days below -8 ({} of them crowding explains)\n", self.mood_lowest, self.mood_highest,
            q(0.0), q(0.25), q(0.5), q(0.75), q(1.0), self.low_mood_days, self.crowded_low_days);
        out += &format!("farm wages: {:.0} paid, {:.0} taken in crops\n", self.wages_paid, self.wages_unpaid);
        out += &format!("route-days: {} on the road ({:.0}%; the longest out {:.1} days)", self.road_days,
            100.0 * self.road_days as f64 / route_days.max(1) as f64, self.longest_out);
        let mut idle: Vec<_> = self.idle.iter().collect();
        idle.sort_by(|a, b| b.1.cmp(a.1));
        for (why, n) in idle {
            out += &format!(", {n} {why}");
        }
        out += "\n";
        out += &format!("on at the end: {} farms, {} banks, {} hubs, {} off-map places, {} soldiers serving; {} \
            world-days at war\n", self.farms, self.banks, self.hubs, self.off_map, self.soldiers, self.war_days);
        out += &format!("paper: {} documents drawn ({} bills of lading), {:.1} emeralds of scribes' fees, {} pallets packed\n",
            self.documents, self.ladings, self.scribe_fees, self.pallets);
        out
    }
}
