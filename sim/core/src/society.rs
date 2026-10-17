//! A society in a world of its own, for hosts that aren't Bevy apps (a Minecraft mod, a companion program, a test):
//! build it, hand it happenings, read what came out.

use bevy_ecs::prelude::*;

use crate::content::{Content, GoodId};
use crate::contracts::{Book, Contract, Incoterm};
use crate::messages::{Input, Output, RouteId, TownId};
use crate::rules::{look_schedule, touch::catch_up_town};
use crate::state::*;

pub struct Society {
    pub world: World,
    look: Schedule,
    towns: Vec<Entity>,
    routes: Vec<Entity>,
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
        world.insert_resource(Book::default());
        world.insert_resource(crate::bills::Bills::default());
        world.insert_resource(crate::mint::Mint::default());
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
                Town { name: name.to_string(), id: self.towns.len() },
                Money(money),
                Stock(vec![0.0; n]),
                Trades { makes: vec![0.0; n], uses: vec![0.0; n] },
                Touched(now),
                Budget::default(),
                People { last: now, paid_share: 1.0, ..Default::default() },
            ))
            .id();
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

    /// Sets what a town's people make and use of a good a day.
    pub fn set_trade(&mut self, town: TownId, good: GoodId, makes: f64, uses: f64) {
        let mut trades = self.world.get_mut::<Trades>(self.towns[town]).expect("a town");
        trades.makes[good] = makes;
        trades.uses[good] = uses;
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
                let mut town = world.entity_mut(e);
                let trades = town.get::<Trades>().expect("a town").clone();
                let mut touched = *town.get::<Touched>().unwrap();
                let mut money = *town.get::<Money>().unwrap();
                let mut stock = town.get::<Stock>().unwrap().clone();
                catch_up_town(now, &content, &mut ledger, &mut stock, &mut money, &trades, &mut touched);
                stock.0[good] += count;
                ledger.delivered[good] += count;
                town.insert((stock, money, touched));
            });
        });
    }

    /// Money paid a town from outside the society (a player buying at its shops).
    pub fn pay(&mut self, town: TownId, money: f64) {
        let e = self.towns[town];
        self.world.get_mut::<Money>(e).expect("a town").0 += money;
        self.world.resource_mut::<Ledger>().income += money;
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
