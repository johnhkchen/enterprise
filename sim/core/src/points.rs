//! Trade points (`docs/architecture.md`, convention 3): the places in the host's game where the society meets it (a
//! Merchant's desk, a locker, a shop, a banker, a depot), each a town's. The core keeps them, so a host keeps no
//! society ids of its own: a desk placed founds a town there ([`Society::found`]), and after a load the host rebuilds
//! its lookup from the points (`Query::Points`, `wire/`).
//!
//! **Stocktakes.** Chests stay physical (`docs/binding-plan.md` 1.5): players, hoppers and Create move items in a
//! point's chests, so the host counts them and the core books the difference. A point keeps what its chests held at
//! its last stocktake; a new one ([`Society::stocktake`]) books what's more as goods delivered to its town from
//! outside the society (`Ledger.delivered`, as [`Society::deliver`] does) and what's less as goods taken out of it
//! (`Ledger.used`, as a player's buying at a town's shop is), never more than the town holds. A point's first
//! stocktake counts from nothing. What the core itself moves in and out of a point's chests (a wagon's sale, a
//! locker's trade) will come back to the host as `Moved` and be counted into the point's last stocktake, so it isn't
//! booked again (the stock adapter's task, `stock-adapter`); until then a point's last stocktake is only what the host
//! counted.
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
}

/// A point: what it is, its town, and what its chests held at its last stocktake (by good; empty before the first).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Placed {
    pub kind: Kind,
    pub town: TownId,
    pub counted: Vec<f64>,
}

/// Every trade point, by where it stands.
#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Points(pub BTreeMap<Spot, Placed>);

impl Society {
    /// A new town at its desk: it stands where the desk does (`place_town` at the desk's x and z), with nothing held
    /// and no money, and its desk is a point of its own. Refused if a point already stands there.
    pub fn found(&mut self, name: &str, desk: Spot) -> Result<TownId, String> {
        if self.points().0.contains_key(&desk) {
            return Err(format!("a point already stands at {desk:?}"));
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
        points.0.insert(at, Placed { kind, town, counted: Vec::new() });
        Ok(())
    }

    /// A trade point broken: gone from the points, its town and what its town holds untouched.
    pub fn remove_point(&mut self, at: &Spot) -> Result<Placed, String> {
        self.world.resource_mut::<Points>().0.remove(at).ok_or_else(|| format!("no point at {at:?}"))
    }

    /// Every trade point.
    pub fn points(&self) -> &Points {
        self.world.resource::<Points>()
    }

    /// The point at a place, if there's one.
    pub fn point(&self, at: &Spot) -> Option<&Placed> {
        self.points().0.get(at)
    }

    /// What a point's chests hold now, every good there (a good not listed holds none; a good listed twice, the sum):
    /// what's more than at its last stocktake is delivered to its town, what's less taken from it (never more than
    /// the town holds), and this is its last stocktake from now on. Returns what was booked by good (+ delivered,
    /// - taken).
    pub fn take_stock(&mut self, at: &Spot, counts: &[(GoodId, f64)]) -> Result<Vec<f64>, String> {
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
        for good in 0..n {
            let more = now[good] - last[good];
            if more > 0.0 {
                self.deliver(town, good, more);
                booked[good] = more;
            } else if more < 0.0 {
                booked[good] = -self.withdraw(town, good, -more);
            }
        }
        self.world.resource_mut::<Points>().0.get_mut(at).expect("the point").counted = now;
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

    /// Applies a point's happening (`Input::AddTown`, `AddPoint`, `RemovePoint`, `Stocktake`); false for any other.
    /// Panics on one naming what isn't there: the wire checks them first (`wire::handle`).
    pub(crate) fn apply_points(&mut self, input: &crate::wire::Input) -> bool {
        use crate::wire::Input;
        match input {
            Input::AddTown { name, desk } => {
                self.found(name, desk.spot()).expect("a town's desk where no point stands");
            }
            Input::AddPoint { point, town } => {
                self.add_point(point.kind(), point.spot(), *town as usize).expect("a point of a town's where none stands");
            }
            Input::RemovePoint { point } => {
                self.remove_point(&point.spot()).expect("a point that stands");
            }
            Input::Stocktake { point, counts } => {
                let counts: Vec<(GoodId, f64)> = counts.iter().map(|c| (c.good as GoodId, c.count)).collect();
                self.take_stock(&point.spot(), &counts).expect("a stocktake at a point that stands");
            }
            _ => return false,
        }
        true
    }
}
