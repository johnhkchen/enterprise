//! A society in a world of its own, for hosts that aren't Bevy apps (a Minecraft mod, a companion program, a test):
//! build it, hand it happenings, read what came out.

use bevy_ecs::prelude::*;

use crate::cash_book::{self, Account, Accountant, CashBook, Reason, Side};
use crate::content::{Content, GoodId};
use crate::contracts::{Book, Contract, Incoterm};
use crate::messages::{Input, Output, RouteId, TownId};
use crate::rules::{look_schedule, touch::catch_up_town};
use crate::state::*;

/// A society and its look. A slice's own host calls go in an `impl Society` block in its own module, not here.
pub struct Society {
    pub world: World,
    pub(crate) look: Schedule,
    pub(crate) towns: Vec<Entity>,
    pub(crate) routes: Vec<Entity>,
}

/// A town as a host sees it.
#[derive(Clone, Debug)]
pub struct TownView {
    pub name: String,
    pub money: f64,
    pub stock: Vec<f64>,
    pub people: People,
}

/// A route as a host sees it.
#[derive(Clone, Debug)]
pub struct RouteView {
    pub trips: u32,
    pub sold: f64,
    pub carried: f64,
    pub on_the_road: bool,
}

impl Society {
    pub fn new(content: Content) -> Self {
        let n = content.goods.len();
        let mut world = World::new();
        world.insert_resource(Ledger { made: vec![0.0; n], used: vec![0.0; n], delivered: vec![0.0; n], ..Default::default() });
        world.insert_resource(content);
        world.insert_resource(Clock::default());
        world.insert_resource(Outbox::default());
        world.insert_resource(Towns::default());
        // The rest a new society starts with, as `slices.rs` lists them.
        crate::slices::Resources::seed(&mut world);
        Society { world, look: look_schedule(), towns: Vec::new(), routes: Vec::new() }
    }

    pub fn content(&self) -> &Content {
        self.world.resource::<Content>()
    }

    pub fn content_mut(&mut self) -> Mut<'_, Content> {
        self.world.resource_mut::<Content>()
    }

    pub fn now(&self) -> u64 {
        self.world.resource::<Clock>().now
    }

    /// A town with `money` and nothing made, used or held yet.
    pub fn add_town(&mut self, name: &str, money: f64) -> TownId {
        let n = self.content().goods.len();
        let now = self.now();
        let e = self
            .world
            .spawn((
                Town { name: name.to_string(), id: self.towns.len(), hex: None },
                Money(money),
                Stock(vec![0.0; n]),
                Trades { makes: vec![0.0; n], uses: vec![0.0; n], capacity: vec![0.0; n] },
                Touched(now),
                Budget::default(),
                People { last: now, paid_share: 1.0, ..Default::default() },
                crate::bargain::LockerCapacity::default(),
            ))
            .id();
        // Every town has a workshop's hooks, so towns share one archetype whether or not a hook is set.
        self.world.entity_mut(e).insert(crate::workshop::Workshop::default());
        self.towns.push(e);
        self.world.resource_mut::<Towns>().0.push(e);
        self.towns.len() - 1
    }

    /// A town's people: how many villagers, how many beds, and the most beds its land holds.
    pub fn set_people(&mut self, town: TownId, villagers: f64, beds: f64, land: f64) {
        let mut p = self.world.get_mut::<People>(self.towns[town]).expect("a town");
        p.villagers = villagers;
        p.beds = beds;
        p.land = land;
    }

    /// A town's people as a roster, replacing any it had: a hand of each `(trade, level)` (the trade by name, as
    /// [`Content::trade`] reads it), with the XP its level took. Its villagers follow the roster from now on; the
    /// desk's Merchant is its most experienced Merchant.
    pub fn set_roster(&mut self, town: TownId, hands: &[(&str, u8)]) {
        let content = self.content();
        let hands: Vec<crate::roster::Hand> = hands
            .iter()
            .map(|&(trade, level)| {
                let t = content.trade(trade).unwrap_or_else(|| panic!("no trade called {trade}"));
                crate::roster::Hand::new(content, t, level)
            })
            .collect();
        let roster = crate::roster::Roster::new(content, hands);
        self.put_roster(town, roster);
    }

    /// One more hand for a town (a town without a roster starts one). Returns its place in the roster.
    pub fn add_hand(&mut self, town: TownId, trade: &str, level: u8) -> usize {
        let e = self.towns[town];
        let mut roster = self.world.get::<crate::roster::Roster>(e).cloned().unwrap_or_default();
        let content = self.content();
        let t = content.trade(trade).unwrap_or_else(|| panic!("no trade called {trade}"));
        let i = roster.add(content, crate::roster::Hand::new(content, t, level));
        self.put_roster(town, roster);
        i
    }

    /// A town's roster as it is now (its children grown by the time elapsed); empty if it keeps none.
    pub fn roster(&self, town: TownId) -> crate::roster::Roster {
        let mut roster = self.world.get::<crate::roster::Roster>(self.towns[town]).cloned().unwrap_or_default();
        roster.grow_up(self.now());
        roster
    }

    fn put_roster(&mut self, town: TownId, roster: crate::roster::Roster) {
        let e = self.towns[town];
        self.world.get_mut::<People>(e).expect("a town").villagers = roster.residents() as f64;
        self.world.entity_mut(e).insert(roster);
    }

    /// Sets what a town's people make and use of a good a day; until its lockers record what they can take, that use
    /// is its capacity too (once they have, their figures stand).
    pub fn set_trade(&mut self, town: TownId, good: GoodId, makes: f64, uses: f64) {
        let e = self.towns[town];
        let recorded = !self.world.get::<crate::bargain::LockerCapacity>(e).expect("a town").0.is_empty();
        let mut trades = self.world.get_mut::<Trades>(e).expect("a town");
        trades.makes[good] = makes;
        trades.uses[good] = uses;
        if !recorded {
            trades.capacity[good] = uses;
        }
    }

    /// A run from one town to another, `days` each way, `road_cost` a round trip, every `every_days` on its timetable.
    pub fn add_route(&mut self, from: TownId, to: TownId, days: f64, road_cost: f64, every_days: f64) -> RouteId {
        let day = self.content().day_ticks as f64;
        let route = Route {
            seq: self.routes.len(),
            from: self.towns[from],
            to: self.towns[to],
            from_id: from,
            to_id: to,
            travel_ticks: (days * day).round().max(1.0) as u64,
            road_cost,
            every_ticks: (every_days * day).round() as u64,
        };
        let e = self.world.spawn((route, RouteState::default())).id();
        self.routes.push(e);
        self.routes.len() - 1
    }

    /// Places a town at the point (x, z) in blocks: it stands in that point's hex.
    pub fn place_town(&mut self, town: TownId, x: f64, z: f64) {
        self.world.get_mut::<Town>(self.towns[town]).expect("a town").hex = Some(crate::hex::Hex::at(x, z));
    }

    /// The hex a town stands in, if it's been placed.
    pub fn town_hex(&self, town: TownId) -> Option<crate::hex::Hex> {
        self.world.get::<Town>(self.towns[town]).expect("a town").hex
    }

    /// A hex's ground: its share under water (0 to 1) and its relief in blocks. Its bridge, if any, stays.
    pub fn set_ground(&mut self, hex: crate::hex::Hex, water_share: f64, relief: f64) {
        self.world.resource_mut::<crate::map::Map>().set_ground(hex, water_share, relief);
    }

    /// A finished bridge on a hex: if it's mostly water, caravans cross it as open ground.
    pub fn bridge(&mut self, hex: crate::hex::Hex) {
        self.world.resource_mut::<crate::map::Map>().bridge(hex);
    }

    pub fn map(&self) -> &crate::map::Map {
        self.world.resource::<crate::map::Map>()
    }

    /// Road food both ways across a hex of open ground (`Caravans.perHex`): 16 food points, at bread's worth a point
    /// (the mod's 1/30 of an emerald when the host has no bread with food points).
    pub fn per_hex(&self) -> f64 {
        let content = self.content();
        let point = content
            .good("bread")
            .map(|b| &content.goods[b])
            .filter(|b| b.food > 0.0)
            .map_or(crate::map::FOOD_POINT_VALUE, |b| b.value / b.food);
        crate::map::per_hex(point)
    }

    /// The cheapest way between two placed towns over the map (no tolls yet), or `None` if either isn't placed or
    /// there is no way through.
    pub fn plan_road(&self, from: TownId, to: TownId) -> Option<crate::map::Path> {
        let (a, b) = (self.town_hex(from)?, self.town_hex(to)?);
        let free = [from, to].into_iter().collect();
        self.map().plan(a, b, self.per_hex(), 0.0, &Default::default(), &free, false, &Default::default())
    }

    /// A road from one placed town to another over the map, a wagon every `every_days` on its timetable: its path is
    /// the cheapest way (`plan_road`), its time the path's blocks (80 a hex) at 1,200 blocks a day (20 ticks a block),
    /// its road food the path's cost. `None` when there's no way through (or a town isn't placed).
    pub fn add_road(&mut self, from: TownId, to: TownId, every_days: f64) -> Option<RouteId> {
        let path = self.plan_road(from, to)?;
        let days = path.blocks().max(1.0) / crate::map::BLOCKS_PER_DAY;
        let id = self.add_route(from, to, days, path.cost, every_days);
        self.world.entity_mut(self.routes[id]).insert(RoutePath(path.hexes));
        Some(id)
    }

    /// The hexes a route's wagons cross, if it's a road over the map.
    pub fn route_path(&self, route: RouteId) -> Option<Vec<crate::hex::Hex>> {
        self.world.get::<RoutePath>(self.routes[route]).map(|p| p.0.clone())
    }

    /// The ticks a route's wagons take one way, and its road food a round trip.
    pub fn route_road(&self, route: RouteId) -> (u64, f64) {
        let r = self.world.get::<Route>(self.routes[route]).expect("a route");
        (r.travel_ticks, r.road_cost)
    }

    /// Moves the society on to `to` (in ticks), looking at it every `look_ticks`. Returns what happened.
    pub fn advance(&mut self, to: u64) -> Vec<Output> {
        let look = self.content().dials.look_ticks.max(1);
        while self.now() < to {
            let next = ((self.now() / look + 1) * look).min(to);
            self.world.resource_mut::<Clock>().now = next;
            self.look.run(&mut self.world);
        }
        std::mem::take(&mut self.world.resource_mut::<Outbox>().0)
    }

    /// Goods handed to a town from outside the society (a player's delivery): into its warehouse.
    pub fn deliver(&mut self, town: TownId, good: GoodId, count: f64) {
        let e = self.towns[town];
        let now = self.now();
        self.world.resource_scope(|world, content: Mut<Content>| {
            world.resource_scope(|world, mut ledger: Mut<Ledger>| {
              world.resource_scope(|world, mut book: Mut<CashBook>| {
                let mut town = world.entity_mut(e);
                let id = town.get::<Town>().expect("a town").id;
                let trades = town.get::<Trades>().expect("a town").clone();
                let mut touched = *town.get::<Touched>().unwrap();
                let mut money = *town.get::<Money>().unwrap();
                let mut stock = town.get::<Stock>().unwrap().clone();
                catch_up_town(now, &content, &mut ledger, &mut book, id, &mut stock, &mut money, &trades, &mut touched);
                stock.0[good] += count;
                ledger.delivered[good] += count;
                town.insert((stock, money, touched));
              });
            });
        });
    }

    /// Money paid a town from outside the society (a player buying at its shops).
    pub fn pay(&mut self, town: TownId, money: f64) {
        self.post(Account::Outside, Account::Treasury(town), money, Reason::PaidIn);
    }

    /// Moves `amount` from one account to another for `reason`, noted in both books (a host's own transfers, and
    /// tests). A wagon's purse moves only while its wagon is on the road.
    pub fn post(&mut self, from: Account, to: Account, amount: f64, reason: Reason) {
        if from == to {
            return;
        }
        let now = self.now();
        let day = self.content().day_ticks;
        let mut balances: Vec<(Account, f64)> = [from, to].into_iter().filter(|a| *a != Account::Outside)
            .map(|a| (a, self.balance(a).expect("an account that holds money"))).collect();
        let mut ledger = std::mem::take(&mut *self.world.resource_mut::<Ledger>());
        let mut book = std::mem::take(&mut *self.world.resource_mut::<CashBook>());
        {
            let mut sides = balances.iter_mut();
            let mut side = |a: Account| -> Side<'_> {
                if a == Account::Outside { Side::outside() } else {
                    let (_, b) = sides.next().unwrap();
                    Side { account: a, balance: Some(b) }
                }
            };
            let (f, t) = (side(from), side(to));
            cash_book::post(&mut ledger, &mut book, f, t, amount, reason, now, day);
        }
        *self.world.resource_mut::<Ledger>() = ledger;
        *self.world.resource_mut::<CashBook>() = book;
        for (a, b) in balances {
            self.set_balance(a, b);
        }
    }

    /// What an account holds now (the outside holds no count; a wagon's purse, none while it's home).
    pub fn balance(&self, account: Account) -> Option<f64> {
        match account {
            Account::Treasury(t) => self.world.get::<Money>(self.towns[t]).map(|m| m.0),
            Account::Households(t) => self.world.get::<People>(self.towns[t]).map(|p| p.purse),
            Account::Wagon(r) => self.world.get::<RouteState>(self.routes[r]).and_then(|s| s.wagon.as_ref()).map(|w| w.purse),
            Account::Outside => None,
        }
    }

    fn set_balance(&mut self, account: Account, value: f64) {
        match account {
            Account::Treasury(t) => self.world.get_mut::<Money>(self.towns[t]).unwrap().0 = value,
            Account::Households(t) => self.world.get_mut::<People>(self.towns[t]).unwrap().purse = value,
            Account::Wagon(r) => self.world.get_mut::<RouteState>(self.routes[r]).unwrap().wagon.as_mut().unwrap().purse = value,
            Account::Outside => {}
        }
    }

    /// A town's cash book for the last `days` days, today's included: by day, then what for (in words), the money
    /// in (+) or out (-) of its treasury.
    pub fn cash_book(&self, town: TownId, days: u64) -> Vec<(u64, String, f64)> {
        self.account_book(Account::Treasury(town), days)
    }

    /// Any account's book for the last `days` days, as [`Society::cash_book`].
    pub fn account_book(&self, account: Account, days: u64) -> Vec<(u64, String, f64)> {
        let today = self.now() / self.content().day_ticks;
        let from = (today + 1).saturating_sub(days);
        self.world.resource::<CashBook>().entries(account, from, today).into_iter()
            .map(|(d, r, v)| (d, r.label(self.content()), v)).collect()
    }

    /// What the Accountant held back from a town's spenders, by day and what for, the last `days` days.
    pub fn held_back(&self, town: TownId, days: u64) -> Vec<(u64, String, f64)> {
        let today = self.now() / self.content().day_ticks;
        let from = (today + 1).saturating_sub(days);
        self.world.resource::<CashBook>().held.get(&Account::Treasury(town)).map_or_else(Vec::new, |h| {
            h.range((from, Reason::Sold(0))..).take_while(|((d, _), _)| *d <= today)
                .map(|((d, r), v)| (*d, r.label(self.content()), *v)).collect()
        })
    }

    /// A town's free money now: beyond its trading capital, less the bills on it falling due within `budget_days`.
    pub fn free(&self, town: TownId) -> f64 {
        let e = self.world.entity(self.towns[town]);
        crate::rules::free(self.content(), e.get::<Trades>().unwrap(), e.get::<Money>().unwrap(),
            self.world.resource::<crate::bills::Bills>(), town, self.now())
    }

    /// What's left of a town's day's budget.
    pub fn budget_left(&mut self, town: TownId) -> f64 {
        let free = self.free(town);
        let now = self.now();
        self.world.resource_scope(|world, content: Mut<Content>| {
            let mut budget = world.get_mut::<Budget>(self.towns[town]).unwrap();
            Accountant::left(&content, now, free, &mut budget)
        })
    }

    /// May the town spend `amount` on `what` today? If so it's taken off its day's budget; if not, held back.
    pub fn approve(&mut self, town: TownId, what: &str, amount: f64) -> bool {
        self.accountant(town, what, |content, book, now, free, budget, what| {
            if Accountant::approve(content, book, town, now, free, budget, what, amount) { 1.0 } else { 0.0 }
        }) > 0.0
    }

    /// As much of `amount` for `what` as the town's day's budget has left, taken off it.
    pub fn allow(&mut self, town: TownId, what: &str, amount: f64) -> f64 {
        self.accountant(town, what, |content, book, now, free, budget, what| {
            Accountant::allow(content, book, town, now, free, budget, what, amount)
        })
    }

    fn accountant(&mut self, town: TownId, what: &str,
            f: impl FnOnce(&Content, &mut CashBook, u64, f64, &mut Budget, Reason) -> f64) -> f64 {
        let free = self.free(town);
        let now = self.now();
        let what = Reason::parse(self.content(), what);
        let e = self.towns[town];
        self.world.resource_scope(|world, content: Mut<Content>| {
            world.resource_scope(|world, mut book: Mut<CashBook>| {
                let mut budget = world.get_mut::<Budget>(e).unwrap();
                f(&content, &mut book, now, free, &mut budget, what)
            })
        })
    }

    /// The mint as it stands (its pressure and debasement, touched as of their last change).
    pub fn mint(&self) -> &crate::mint::Mint {
        self.world.resource::<crate::mint::Mint>()
    }

    /// Spurs per emerald now: the rate the host and prices read.
    pub fn mint_rate(&self) -> f64 {
        self.mint().rate(self.now(), self.content().day_ticks)
    }

    /// Sells `emeralds` to the mint and returns the spurs paid. Money-neutral in the core's books: the core's money is
    /// emerald-worth, so the coin paid for emeralds stands for the same worth. What the mint changes is the rate
    /// (selling pressure) and so what coin buys, which the host reads through [`Society::mint_rate`].
    pub fn sell_emeralds(&mut self, emeralds: u64) -> u64 {
        let now = self.now();
        let day = self.content().day_ticks;
        self.world.resource_mut::<crate::mint::Mint>().sell(emeralds, now, day)
    }

    /// Applies one happening and returns what came of it.
    pub fn apply(&mut self, input: &Input) -> Vec<Output> {
        match *input {
            Input::Advance { to } => self.advance(to),
            Input::Deliver { town, good, count } => {
                self.deliver(town, good, count);
                Vec::new()
            }
            Input::Pay { town, money } => {
                self.pay(town, money);
                Vec::new()
            }
        }
    }

    pub fn town(&self, town: TownId) -> TownView {
        let e = self.world.entity(self.towns[town]);
        TownView {
            name: e.get::<Town>().unwrap().name.clone(),
            money: e.get::<Money>().unwrap().0,
            stock: e.get::<Stock>().unwrap().0.clone(),
            people: e.get::<People>().unwrap().clone(),
        }
    }

    pub fn route(&self, route: RouteId) -> RouteView {
        let s = self.world.get::<RouteState>(self.routes[route]).unwrap();
        RouteView { trips: s.trips, sold: s.sold, carried: s.carried, on_the_road: s.wagon.is_some() }
    }

    /// A contract drafted at one party's (`at_seller`: the seller's; `both`: both hold copies, in force at once):
    /// `quantity` of `good` an installment, from `start`, every `every` ticks, `installments` times.
    #[allow(clippy::too_many_arguments)]
    pub fn draft(&mut self, seller: TownId, buyer: TownId, good: GoodId, quantity: f64, price: f64, start: u64,
            every: u64, installments: u32, incoterm: Incoterm, both: bool) -> usize {
        self.world.resource_mut::<Book>().draft(seller, buyer, good, quantity, price, start, every, installments,
            incoterm, true, both)
    }

    pub fn contract(&self, id: usize) -> Contract {
        self.world.resource::<Book>().contracts[id].clone()
    }

    pub fn bills(&self) -> Vec<crate::bills::Bill> {
        self.world.resource::<crate::bills::Bills>().bills.clone()
    }

    pub fn contracts(&self) -> Vec<Contract> {
        self.world.resource::<Book>().contracts.clone()
    }

    /// Trust between two towns now.
    pub fn trust(&self, a: TownId, b: TownId) -> f64 {
        self.world.resource::<Book>().trust(a, b, self.now(), self.content().day_ticks)
    }

    /// Judges installments now (as each day's first look does).
    pub fn review(&mut self) {
        let now = self.now();
        let day = self.content().day_ticks;
        self.world.resource_mut::<Book>().review(now, day, |_| false);
    }

    pub fn town_count(&self) -> usize {
        self.towns.len()
    }

    pub fn route_count(&self) -> usize {
        self.routes.len()
    }

    /// The society as one blob, for the game to keep with its save (see [`crate::save`]).
    pub fn save(&mut self) -> Vec<u8> {
        crate::save::save(&mut self.world)
    }

    /// A society from a blob [`Society::save`] made: run on, it comes out as the saved one would have.
    /// What in the world a save would lose (see [`crate::save::unsaved`]): empty when every piece of state is in
    /// `saved!`.
    pub fn unsaved(&self) -> Vec<String> {
        crate::save::unsaved(&self.world)
    }

    pub fn load(bytes: &[u8]) -> Result<Society, String> {
        let (world, towns, routes) = crate::save::load(bytes)?;
        Ok(Society { world, look: look_schedule(), towns, routes })
    }

    /// Sets a dial for this world by its name (saved with it), as the mod's `/enterprise dial <name> <value>`.
    pub fn set_dial(&mut self, name: &str, value: f64) -> Result<(), String> {
        self.content_mut().dials.set(name, value)
    }

    /// Puts a dial back to the mod's default.
    pub fn reset_dial(&mut self, name: &str) -> Result<(), String> {
        self.content_mut().dials.reset(name)
    }

    /// A dial's value now, by its name.
    pub fn dial(&self, name: &str) -> Option<f64> {
        self.content().dials.get(name)
    }
}
