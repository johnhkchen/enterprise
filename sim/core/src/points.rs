//! Trade points (`docs/architecture.md`, convention 3): the places in the host's game where the society meets it (a
//! Merchant's desk, a locker, a shop, a banker, a depot), each a town's. The core keeps them, so a host keeps no
//! society ids of its own: a desk placed founds a town there ([`Society::found`]), and after a load the host rebuilds
//! its lookup from the points (`Query::Points`, `wire/`).
//!
//! **Stocktakes.** Chests stay physical (`docs/binding-plan.md` 1.5): players, hoppers and Create move items in a
//! point's chests, so the host counts them and the core books the difference. A point keeps what the core takes its
//! chests to hold (`Placed.counted`); a new stocktake ([`Society::take_stock`]) books what's more as goods delivered to
//! its town from outside the society (`Ledger.delivered`, as [`Society::deliver`] does) and what's less as goods taken
//! out of it (`Ledger.used`, as a player's buying at a town's shop is), never more than the town holds: what the chests
//! gave up beyond that is the point's `short` (goods the host's world made, which the core never had). A point's first
//! stocktake counts from nothing. What the core itself moves in and out of a desk's or a depot's chests (a wagon's
//! sale, the town's people eating) goes to the host as `Moved` and into `counted` at once, so the next stocktake
//! doesn't book it again; what the host couldn't move comes back as `Unmoved` (`chests.rs`).
//!
//! **A point broken** takes its chests with it: what the core took them to hold leaves its town (`Ledger.used`, never
//! more than it holds), since the chests are the host's and a point placed there again counts them in afresh. A desk
//! placed where a town's desk stood is that town's desk again (`chests::FormerDesks`), not a new town.
//!
//! The mod keeps a market's desk and its chests in `merchant/Merchant` and `merchant/Warehouse` (the chests within
//! reach of the desk, summed); here a point is a place and a town, and its chests are the host's.

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::cash_book::CashBook;
use crate::content::{Content, GoodId};
use crate::messages::TownId;
use crate::rules::touch::catch_up_town;
use crate::society::Society;
use crate::state::*;

/// Where a point stands: the host's dimension (a world, a surface) and its block.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Spot {
    pub dim: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// What a point is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    /// A Merchant's desk: its chests are its town's warehouse.
    #[default]
    Desk,
    Locker,
    Shop,
    Banker,
    Depot,
    /// A market off the map (`merchant/OffMap.java`): a town no one builds or visits, at a real position. It has no chests
    /// to count; its goods and money are the core's alone.
    Market,
}

/// A point: what it is, its town, and its chests as the core takes them to be.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Placed {
    pub kind: Kind,
    pub town: TownId,
    /// What its chests hold, by good (empty before the first stocktake): what the host counted at the last, with what
    /// the core has moved in and out since (`Moved`) and less what the host said it couldn't (`Unmoved`).
    pub counted: Vec<f64>,
    /// Its chests' empty slots at the last stocktake: room for what the core moves in (`chests.rs`).
    pub free: u32,
    /// Goods its chests gave up that its town didn't hold, by good (empty if none ever): taken from chests that showed
    /// more than the town had, before they were counted again. The host's world made them; the core never had them.
    pub short: Vec<f64>,
}

/// Every trade point, by where it stands.
#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Points(pub BTreeMap<Spot, Placed>);

impl Society {
    /// A new town at its desk: it stands where the desk does (`place_town` at the desk's x and z), with nothing held
    /// and no money, and its desk is a point of its own. A desk placed where a town's desk stood before it was broken
    /// is that town's desk again (its name kept, `name` unused): a player who breaks a desk and puts it back has the
    /// same town. Refused if a point already stands there.
    pub fn found(&mut self, name: &str, desk: Spot) -> Result<TownId, String> {
        if self.points().0.contains_key(&desk) {
            return Err(format!("a point already stands at {desk:?}"));
        }
        if let Some(town) = self.world.resource_mut::<crate::chests::FormerDesks>().0.remove(&desk) {
            self.add_point(Kind::Desk, desk, town)?;
            return Ok(town);
        }
        let town = self.add_town(name, 0);
        self.place_town(town, desk.x as f64, desk.z as f64);
        self.add_point(Kind::Desk, desk, town)?;
        Ok(town)
    }

    /// A trade point of a town's, placed. Refused if the town isn't there or a point already stands there.
    pub fn add_point(&mut self, kind: Kind, at: Spot, town: TownId) -> Result<(), String> {
        if town >= self.town_count() {
            return Err(format!("no town {town}"));
        }
        let mut points = self.world.resource_mut::<Points>();
        if points.0.contains_key(&at) {
            return Err(format!("a point already stands at {at:?}"));
        }
        points.0.insert(at.clone(), Placed { kind, town, ..Placed::default() });
        // The place is taken again: a desk later put there founds a town of its own.
        self.world.resource_mut::<crate::chests::FormerDesks>().0.remove(&at);
        Ok(())
    }

    /// A trade point broken: gone from the points, and what the core took its chests to hold gone from its town with
    /// them (as a player's taking is: never more than the town holds), so a point placed there again counts them in
    /// afresh and nothing is booked twice. The rest of what its town holds stays. A desk's place is remembered: a desk
    /// put there again is that town's ([`Society::found`]).
    pub fn remove_point(&mut self, at: &Spot) -> Result<Placed, String> {
        let placed = self.world.resource_mut::<Points>().0.remove(at).ok_or_else(|| format!("no point at {at:?}"))?;
        if placed.kind == Kind::Locker {
            // A hand whose locker stood there has none now (`locker_host.rs`).
            self.untie_lockers_at(placed.town, at);
        }
        for (good, &n) in placed.counted.iter().enumerate() {
            if n > 0.0 {
                self.withdraw(placed.town, good, n);
            }
        }
        if placed.kind == Kind::Shop {
            // A vendor's shop goes with it (`shop_host.rs`).
            self.forget_shop_point(at);
        }
        if placed.kind == Kind::Desk {
            // Its money goes with its chests (`till.rs`), and its place is remembered.
            self.forget_till(at, placed.town);
            self.world.resource_mut::<crate::chests::FormerDesks>().0.insert(at.clone(), placed.town);
        }
        Ok(placed)
    }

    /// Every trade point.
    pub fn points(&self) -> &Points {
        self.world.resource::<Points>()
    }

    /// The point at a place, if there's one.
    pub fn point(&self, at: &Spot) -> Option<&Placed> {
        self.points().0.get(at)
    }

    /// What a point's chests hold now, every good there (a good not listed holds none; a good listed twice, the sum),
    /// and `free` of their slots empty: what's more than the core took them to hold is delivered to its town, what's
    /// less taken from it (never more than the town holds: the rest is the point's `short`), and this is what they hold
    /// from now on. Returns what was booked by good (+ delivered, - taken).
    pub fn take_stock(&mut self, at: &Spot, counts: &[(GoodId, f64)], free: u32) -> Result<Vec<f64>, String> {
        let n = self.content().goods.len();
        let Some(placed) = self.point(at) else {
            return Err(format!("no point at {at:?}"));
        };
        let (town, mut last) = (placed.town, placed.counted.clone());
        last.resize(n, 0.0);
        let mut now = vec![0.0; n];
        for &(good, count) in counts {
            if good >= n || !count.is_finite() || count < 0.0 {
                return Err(format!("{count} of good {good}"));
            }
            now[good] += count;
        }
        let mut booked = vec![0.0; n];
        let mut short = vec![0.0; n];
        for good in 0..n {
            let more = now[good] - last[good];
            if more > 0.0 {
                self.deliver(town, good, more);
                booked[good] = more;
            } else if more < 0.0 {
                let taken = self.withdraw(town, good, -more);
                booked[good] = -taken;
                short[good] = -more - taken;
            }
        }
        let mut points = self.world.resource_mut::<Points>();
        let placed = points.0.get_mut(at).expect("the point");
        placed.counted = now;
        placed.free = free;
        if short.iter().any(|&s| s > 0.0) {
            placed.short.resize(n, 0.0);
            for (into, s) in placed.short.iter_mut().zip(short) {
                *into += s;
            }
        }
        Ok(booked)
    }

    /// Goods taken out of a town's warehouse to outside the society (a player's taking from its chests): as many of
    /// `count` as it holds, once the town is caught up to now. Returns how many.
    pub fn withdraw(&mut self, town: TownId, good: GoodId, count: f64) -> f64 {
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
                    let taken = count.min(stock.0[good]).max(0.0);
                    stock.0[good] -= taken;
                    ledger.used[good] += taken;
                    town.insert((stock, money, touched));
                    taken
                })
            })
        })
    }

    /// Applies a point's happening (`Input::AddTown`, `AddPoint`, `RemovePoint`, `Stocktake`, `Unmoved`, `Cashcount`, `CashUnmoved`); false for any
    /// other.
    /// Panics on one naming what isn't there: the wire checks them first (`wire::handle`).
    pub(crate) fn apply_points(&mut self, input: &crate::wire::Input) -> bool {
        use crate::wire::Input;
        match input {
            Input::AddTown { name, desk } => {
                let town = self.found(name, desk.spot()).expect("a town's desk where no point stands");
                // A host's town: its lockers are where the host places them (`locker_host.rs`).
                self.host_places_lockers(town);
            }
            Input::AddPoint { point, town } => {
                self.add_point(point.kind(), point.spot(), *town as usize).expect("a point of a town's where none stands");
            }
            Input::RemovePoint { point } => {
                self.remove_point(&point.spot()).expect("a point that stands");
            }
            Input::Stocktake { point, counts, free } => {
                let counts: Vec<(GoodId, f64)> = counts.iter().map(|c| (c.good as GoodId, c.count)).collect();
                self.take_stock(&point.spot(), &counts, *free).expect("a stocktake at a point that stands");
            }
            Input::Unmoved { point, good, count } => {
                self.unmoved(&point.spot(), point.kind(), *good as GoodId, *count);
            }
            Input::Cashcount { point, emeralds, coins, bank, coin_room, writes } => {
                self.count_cash(&point.spot(), *emeralds, *coins, *bank, *coin_room, *writes).expect("a cash count at a desk that stands");
            }
            Input::CashUnmoved { point, emeralds, coins, bank } => {
                self.cash_unmoved(&point.spot(), *emeralds, *coins, *bank);
            }
            _ => return false,
        }
        true
    }
}
