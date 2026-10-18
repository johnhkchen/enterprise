//! Settlers: a town grown past growing sends a party out to start a new market (`caravan/Expeditions.java`; the
//! decision is `rules/settlers.rs`).
//!
//! - `Expeditions.raising` (118): a town of [`GROWN`] villagers or more with food of at least twice the caravans' par
//!   (`Focus.of`, `Warehouse.FOOD_PAR_POINTS`), none of its parties on the road, and its last gone `days_between` (8)
//!   days ago, is raising a party.
//! - `consider` (130): the hands waiting for work, money to spare, a village nobody trades with (`unclaimedVillage`,
//!   284) or else open ground (`openGround`, 242); the road planned over the hexes; the party leaves.
//! - `capital` (218): `max(least, min(jumpstart, spare / 2))`, the least 10 for a village and 16 for a new town, the
//!   jumpstart the `settling_capital` / `founding_capital` dial. A village costs 14 emeralds in all at the least (the
//!   desk, 4, is spent), a new town 34 (18 spent).
//! - `homeAgain` (398): no place to settle, and the party comes home with the capital and the bread.
//!
//! The party is off the world on the road: its hands are taken off the old town's roll (`Settlements.remove`), its
//! capital and bread carried. The capital is outside the society's books while it travels (posted out at the
//! departure, in at the arrival, `settling` in both), as a squad is off the world; so conservation holds on the road.
//! A village is a placed town with a roster and no Merchant and no route ([`Society::add_village`]); the party gives
//! it its Merchant, the capital and a road home. The trip takes the road's blocks (80 a hex) at 20 ticks a block.
//! The bell, desk, chest and cottages the mod's settlers build are Minecraft's; the core counts their cost only.

use std::collections::BTreeMap;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::cash_book::{Account, Reason};
use crate::content::Content;
use crate::hex::Hex;
use crate::messages::TownId;
use crate::people::food_points;
use crate::roster::{Hand, Roster, MERCHANT};
use crate::society::Society;
use crate::state::*;

/// Villagers a town grows to before it sends settlers (`Focus.GROWN`).
pub const GROWN: f64 = 8.0;
/// Food points the caravans keep at home (`Warehouse.FOOD_PAR_POINTS`); a town fed at twice it is past growing.
pub const FOOD_PAR_POINTS: f64 = 320.0;
/// What settling a village costs the town in all, and the capital in it at the least (`SETTLING_EMERALDS`, `SETTLING_CAPITAL`).
pub const SETTLING_EMERALDS: f64 = 14.0;
pub const SETTLING_CAPITAL: f64 = 10.0;
/// What founding a town costs in all, and the capital in it at the least (`FOUNDING_EMERALDS`, `FOUNDING_CAPITAL`).
pub const FOUNDING_EMERALDS: f64 = 34.0;
pub const FOUNDING_CAPITAL: f64 = 16.0;
/// The beds a village's or a new town's ground holds: the mod builds cottages (5 by 5 with a block round) wherever they
/// fit 8 to 32 blocks from the bell (`Growth.homeSite`), room for some sixty; with fields and works, the core counts
/// twenty cottages of two beds (its own estimate: the mod has no cap).
pub const OPEN_LAND: f64 = 40.0;
/// Bread a party carries (`BREAD`).
pub const BREAD: f64 = 64.0;
/// How often a wagon runs on the road home from the new market (the core's own choice: the mod opens a route and its
/// caravans keep their own timetable).
pub const ROAD_EVERY_DAYS: f64 = 4.0;
/// What a party's money and cost are posted for.
pub fn reason() -> Reason {
    Reason::Other("settling".into())
}

/// What a party is bound to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    /// A travelling Merchant to a village nobody trades with.
    Village,
    /// A Merchant and a settler to found a town on open ground.
    Found,
}

/// A party on the road: where from, where to, who (their hands, off every roster), and the capital and bread it carries.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Party {
    pub id: u32,
    pub from: TownId,
    pub kind: Kind,
    /// The hex it is bound for, and for a village the town standing there.
    pub target: Hex,
    pub village: Option<TownId>,
    pub hands: Vec<Hand>,
    pub capital: crate::fixed::Fixed,
    pub bread: f64,
    pub departs: u64,
    pub arrives: u64,
}

/// The parties on the road, when each town last sent one, and what came of those that arrived.
#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Settlers {
    pub parties: Vec<Party>,
    pub last_sent: BTreeMap<TownId, u64>,
    pub next_id: u32,
    /// The day the town reviews last ran (once a day).
    pub reviewed: Option<u64>,
    /// A host asked for this town to be looked at now ([`Society::settle`]).
    pub force: Option<TownId>,
    /// Towns that settlers made a market of or founded, in order.
    pub settled: Vec<TownId>,
    /// The villages among them: the search for the nearest village still finds them (and, a market now, passes on).
    pub villages: Vec<TownId>,
    pub came_home: u32,
}

/// What a party on the road looks like to a host.
#[derive(Clone, Debug, PartialEq)]
pub struct PartyView {
    pub from: TownId,
    pub kind: Kind,
    pub target: Hex,
    pub hands: usize,
    pub capital: crate::fixed::Fixed,
    pub bread: f64,
    pub arrives: u64,
}

/// The capital a party carries: half what the town can spare beyond the set-up, up to the dial's jumpstart, and never
/// less than the least it takes (`Expeditions.capital`). `spare` is what's left after the set-up. Rounded down to the
/// thousandth (a share of the spare).
pub fn capital(kind: Kind, jumpstart: f64, spare: crate::fixed::Fixed) -> crate::fixed::Fixed {
    let least = if kind == Kind::Village { SETTLING_CAPITAL } else { FOUNDING_CAPITAL };
    crate::fixed::Fixed::floor(least.max(jumpstart.min(spare.to_f64() / 2.0)))
}

/// What the set-up costs beyond the capital (the desk, stalls and homes): spent.
pub fn set_up(kind: Kind) -> crate::fixed::Fixed {
    crate::fixed::Fixed::ceil(match kind {
        Kind::Village => SETTLING_EMERALDS - SETTLING_CAPITAL,
        Kind::Found => FOUNDING_EMERALDS - FOUNDING_CAPITAL,
    })
}

/// Is the town raising a party (`Expeditions.raising`): past growing, none of its parties on the road, its last
/// long gone.
pub fn raising(content: &Content, settlers: &Settlers, town: TownId, people: &People, stock: &Stock, now: u64) -> bool {
    people.villagers >= GROWN
        && food_points(content, stock) >= 2.0 * FOOD_PAR_POINTS
        && !settlers.parties.iter().any(|p| p.from == town)
        // Counted in calendar days (the review runs once a day): a party sent on day 0 is followed on day 8.
        && settlers.last_sent.get(&town).is_none_or(|&t| {
            (now / content.day_ticks).saturating_sub(t / content.day_ticks) as f64 >= content.dials.days_between
        })
}

/// Takes the hand at `i` off a roster (`Settlements.remove`), keeping the desk's Merchant straight.
pub fn take_hand(roster: &mut Roster, i: usize) -> Hand {
    let hand = roster.hands.remove(i);
    roster.merchant = match roster.merchant {
        Some(m) if m == i => None,
        Some(m) if m > i => Some(m - 1),
        other => other,
    };
    hand
}

/// Road food both ways across a hex of open ground, as [`Society::per_hex`].
pub fn per_hex(content: &Content) -> f64 {
    let point = content
        .good("bread")
        .map(|b| &content.goods[b])
        .filter(|b| b.food > 0.0)
        .map_or(crate::map::FOOD_POINT_VALUE, |b| b.value / b.food);
    crate::map::per_hex(point)
}

/// The ticks a party takes over `blocks` of road (`Caravans.travelTicks`).
pub fn travel_ticks(blocks: f64) -> u64 {
    ((blocks * crate::map::TICKS_PER_BLOCK).round() as u64).max(20)
}

impl Society {
    /// A village nobody trades with, at the point (x, z) in blocks: a placed town of three idle villagers with no
    /// Merchant and no route, which a town's settlers may make a market of.
    pub fn add_village(&mut self, x: f64, z: f64) -> TownId {
        let n = self.town_count();
        let t = self.add_town(&format!("Village {n}"), 0.0);
        self.place_town(t, x, z);
        self.set_roster(t, &[("none", 1), ("none", 1), ("none", 1)]);
        self.set_people(t, 3.0, 3.0, OPEN_LAND);
        t
    }

    /// Is the town raising a party of settlers now (so its Merchant keeps its idle hands for it)?
    pub fn raising(&self, town: TownId) -> bool {
        let e = self.towns[town];
        match (self.world.get::<People>(e), self.world.get::<Stock>(e)) {
            (Some(people), Some(stock)) => {
                raising(self.content(), self.world.resource::<Settlers>(), town, people, stock, self.now())
            }
            _ => false,
        }
    }

    /// Looks at the town for settlers now, as the daily review would: true if a party left it.
    pub fn settle(&mut self, town: TownId) -> bool {
        let before = self.world.resource::<Settlers>().parties.len();
        self.world.resource_mut::<Settlers>().force = Some(town);
        let mut schedule = bevy_ecs::schedule::Schedule::default();
        crate::rules::settlers::register(&mut schedule);
        schedule.run(&mut self.world);
        self.world.resource_mut::<Settlers>().force = None;
        self.world.resource::<Settlers>().parties.len() > before
    }

    /// The parties on the road.
    pub fn parties(&self) -> Vec<PartyView> {
        self.world.resource::<Settlers>().parties.iter()
            .map(|p| PartyView { from: p.from, kind: p.kind, target: p.target, hands: p.hands.len(),
                capital: p.capital, bread: p.bread, arrives: p.arrives })
            .collect()
    }

    /// The towns settlers have made a market of or founded, in order.
    pub fn settled(&self) -> Vec<TownId> {
        self.world.resource::<Settlers>().settled.clone()
    }

    /// When the town last sent a party, if it has.
    pub fn last_party(&self, town: TownId) -> Option<u64> {
        self.world.resource::<Settlers>().last_sent.get(&town).copied()
    }

    /// Settles the parties that have reached where they were going (called after each look): the new market opens
    /// with its Merchant, the capital and a road home; or, with no place to settle, they come home.
    pub(crate) fn land_parties(&mut self) {
        let now = self.now();
        let arrived: Vec<Party> = {
            let mut s = self.world.resource_mut::<Settlers>();
            let (here, rest): (Vec<Party>, Vec<Party>) = std::mem::take(&mut s.parties).into_iter().partition(|p| p.arrives <= now);
            s.parties = rest;
            here
        };
        for party in arrived {
            self.land(party);
        }
    }

    fn land(&mut self, mut party: Party) {
        let content = self.content().clone();
        let merchant = content.trade(MERCHANT).expect("the Merchant's trade");
        // The village must still be nobody's market; the ground still open and the way still there.
        let ok = match party.kind {
            Kind::Village => party.village.is_some_and(|v| {
                self.world.get::<Roster>(self.towns[v]).is_some_and(|r| r.merchant.is_none())
                    && !self.world.query::<&Route>().iter(&self.world).any(|r| r.from_id == v || r.to_id == v)
            }),
            Kind::Found => {
                let near = (0..self.town_count())
                    .any(|t| self.town_hex(t).is_some_and(|h| h.distance(party.target) < content.dials.min_hexes as i32));
                let way = self.town_hex(party.from).and_then(|h| self.map().plan(h, party.target, per_hex(&content), 0.0,
                    &Default::default(), &Default::default(), false, &Default::default()));
                !near && way.is_some()
            }
        };
        if !ok {
            return self.home_again(party);
        }
        let town = match party.village {
            Some(v) if party.kind == Kind::Village => v,
            _ => {
                let t = self.add_town(&format!("Newfound {}", party.id), 0.0);
                self.place_town(t, party.target.center_x(), party.target.center_z());
                t
            }
        };
        // Its people: the Merchant (the first hand) and, for a new town, its settler and their cottages.
        let e = self.towns[town];
        let mut roster = self.world.get::<Roster>(e).cloned().unwrap_or_default();
        for hand in party.hands.drain(..) {
            roster.add(&content, hand);
        }
        if roster.merchant.is_none() {
            roster.merchant = roster.best(merchant);
        }
        let residents = roster.residents() as f64;
        self.world.entity_mut(e).insert(roster);
        {
            let mut people = self.world.get_mut::<People>(e).expect("a town");
            people.villagers = residents;
            if party.kind == Kind::Found {
                people.beds = 4.0; // two family cottages
                people.land = OPEN_LAND;
            }
        }
        if let Some(bread) = content.good("bread") {
            self.world.resource_mut::<Ledger>().delivered[bread] += party.bread;
            self.world.get_mut::<Stock>(e).expect("a town").0[bread] += party.bread;
        }
        self.post(Account::Outside, Account::Treasury(town), party.capital, reason());
        self.add_road(party.from, town, ROAD_EVERY_DAYS);
        let mut settlers = self.world.resource_mut::<Settlers>();
        settlers.settled.push(town);
        if party.kind == Kind::Village {
            settlers.villages.push(town);
        }
    }

    /// No place to settle: they come home with the capital and the bread (`Expeditions.homeAgain`).
    fn home_again(&mut self, mut party: Party) {
        let content = self.content().clone();
        let e = self.towns[party.from];
        let mut roster = self.world.get::<Roster>(e).cloned().unwrap_or_default();
        for hand in party.hands.drain(..) {
            roster.add(&content, hand);
        }
        let residents = roster.residents() as f64;
        self.world.entity_mut(e).insert(roster);
        self.world.get_mut::<People>(e).expect("a town").villagers = residents;
        if let Some(bread) = content.good("bread") {
            self.world.resource_mut::<Ledger>().delivered[bread] += party.bread;
            self.world.get_mut::<Stock>(e).expect("a town").0[bread] += party.bread;
        }
        self.post(Account::Outside, Account::Treasury(party.from), party.capital, reason());
        self.world.resource_mut::<Settlers>().came_home += 1;
    }
}

