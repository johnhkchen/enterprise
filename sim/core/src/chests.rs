//! Chests stay physical (`docs/binding-plan.md` 1.5, task `stock-adapter`): a town's stock shown in its desk's and
//! depots' chests. Players, hoppers and Create move items in those chests, and the host counts them into a stocktake
//! (`points.rs`); the core's own rules move the town's stock too (a wagon's sale, its people eating, its workshop), and
//! the chests must follow. So after a look's happenings, every desk or depot counted in that look is given what moved:
//! for each good, what the town holds in whole items against what all its points' chests are taken to hold, the
//! difference put in or taken out ([`Society::show_stock`], `Output::Moved` on the wire). The core takes the chests to
//! hold it at once; what the host couldn't do comes back as `Input::Unmoved` ([`Society::unmoved`]).
//!
//! **Never a blind write.** A move is worked out only against a stocktake of the same look, so a chest is always
//! counted before it's written: one that changed while it wasn't counted (unloaded, a hand edit, a rollback) has its
//! changes booked first, and only then is the town's stock shown. A point not counted in a look (its chunks unloaded)
//! is left alone, and what its town moved meanwhile waits for its next stocktake: nothing is queued. A move is a
//! number of items, never a chest's whole contents, so a hopper filling it between the count and the write keeps what
//! it put in (the next stocktake books it).
//!
//! **Where it goes.** Into the town's counted desks first, then its counted depots (each in its places' order), as far
//! as each has room by its last stocktake (its empty slots, and what's left of a stack of the good it holds); what has
//! no room waits on the town's books until a stocktake shows some. Out of its counted desks first, then depots, as far
//! as each holds the good; what's held in chests not counted now waits for them. Whole items only: a town's fractions
//! (a third of a loaf) stay on its books. Shops and bankers aren't written to (their adapters say what their chests
//! are), but what their stocktakes counted is the town's and so isn't shown again in the desk's. **A locker** (a
//! workstation's chest, `locker_host.rs`) is a shelf the Merchant works from, not a store: what the town uses is taken
//! from its counted lockers first, and what they still hold is carried to its desks and depots as far as those have
//! room (`LockerTrading.collect`), so a player's goods left in a locker reach the warehouse and a locker's trades,
//! which the core runs against the warehouse, leave nothing behind there.
//!
//! The mod's warehouse is the chests themselves: `merchant/MerchantStock` (`insert`: a building meant for the goods
//! first, then the desk's chests and storehouses, loose slots then shulker boxes; `take`: loose first, then boxes) and
//! `merchant/Depots` (buildings beyond the desk). Here a depot is one container of a town's warehouse away from its
//! desk, a point of its own; what a building is for (an armoury's war goods, a cold store's crates) isn't here yet.

use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::content::GoodId;
use crate::messages::TownId;
use crate::points::{Kind, Points, Spot};
use crate::society::Society;
use crate::state::Stock;

/// Where towns' desks stood before they were broken: a desk placed there again is that town's ([`Society::found`]).
/// A place another point takes is forgotten.
#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
pub struct FormerDesks(pub BTreeMap<Spot, TownId>);

/// Items the core moves in (`count > 0`) or out of a point's chests, of one good.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Move {
    pub at: Spot,
    pub kind: Kind,
    pub good: GoodId,
    pub count: i64,
}

/// A point's chests as the core takes them to be.
#[derive(Clone, Debug, PartialEq)]
pub struct Chests {
    pub kind: Kind,
    pub town: TownId,
    pub held: Vec<f64>,
    pub free: u32,
    pub short: Vec<f64>,
}

/// The dust floats leave: a count this near a whole one is that one.
const DUST: f64 = 1e-9;

/// Is this kind of point a town's warehouse, written to as its stock moves?
pub fn holds_stock(kind: Kind) -> bool {
    matches!(kind, Kind::Desk | Kind::Depot)
}

/// Is this kind of point's chests written to by the core: a town's warehouse, and its lockers, which the Merchant
/// empties into it (`show_stock`).
pub fn shown(kind: Kind) -> bool {
    holds_stock(kind) || kind == Kind::Locker
}

impl Society {
    /// The stock of every town with a desk, a depot or a locker among `counted` (points the host counted in this look)
    /// shown in those chests: what it holds in whole items against what its points' chests are taken to hold, the
    /// difference moved in (to desks, then depots, as far as each has room) or out (lockers first, then desks, then
    /// depots, as far as each holds it); then what its counted lockers still hold carried to its desks and depots, as
    /// far as they have room (the Merchant collecting, `LockerTrading.collect`: a locker is a workstation's shelf, not
    /// the town's store). Each move is in the points' `counted` from now on. In towns' order, then goods', then points'.
    pub fn show_stock(&mut self, counted: &[Spot]) -> Vec<Move> {
        let present: BTreeSet<&Spot> = counted.iter().collect();
        let n = self.content().goods.len();
        let stacks: Vec<f64> = self.content().goods.iter().map(|g| g.stack.max(1) as f64).collect();
        let points = self.world.resource::<Points>();
        let towns: BTreeSet<TownId> = present.iter()
            .filter_map(|at| points.0.get(*at).filter(|p| shown(p.kind)).map(|p| p.town))
            .collect();
        let mut moves = Vec::new();
        for town in towns {
            let stock = self.world.get::<Stock>(self.towns[town]).expect("a town").0.clone();
            let mut points = self.world.resource_mut::<Points>();
            // Every chest of the town's, counted now or not: what they're all taken to hold is what's shown.
            let mut held = vec![0.0; n];
            for p in points.0.values().filter(|p| p.town == town) {
                for (g, c) in p.counted.iter().enumerate().take(n) {
                    held[g] += c;
                }
            }
            // Its desks counted now, then its depots, each in its places' order; its lockers counted now.
            let counted_of = |points: &Points, want: Kind| -> Vec<Spot> {
                points.0.iter().filter(|(at, p)| p.town == town && p.kind == want && present.contains(at))
                    .map(|(at, _)| at.clone()).collect()
            };
            let mut order: Vec<Spot> = counted_of(&points, Kind::Desk);
            order.extend(counted_of(&points, Kind::Depot));
            let lockers = counted_of(&points, Kind::Locker);
            for p in order.iter().chain(&lockers) {
                points.0.get_mut(p).expect("a counted point").counted.resize(n, 0.0);
            }
            let mut free: BTreeMap<Spot, i64> = order.iter().map(|at| (at.clone(), points.0[at].free as i64)).collect();
            // The stores' room for `k` more of a good, put in order: (where, how many), as far as `k` goes.
            let put_in = |points: &mut Points, free: &mut BTreeMap<Spot, i64>, good: usize, mut k: f64| {
                let mut put = Vec::new();
                for at in &order {
                    if k <= 0.0 {
                        break;
                    }
                    let p = points.0.get_mut(at).expect("a counted point");
                    let has = p.counted[good];
                    let stack = stacks[good];
                    let partial = has - (has / stack).floor() * stack;
                    let merge = if partial > DUST { stack - partial } else { 0.0 };
                    let slots = free[at].max(0) as f64;
                    let room = k.min(merge + slots * stack).floor();
                    if room < 1.0 {
                        continue;
                    }
                    let new_stacks = ((has + room) / stack).ceil() - (has / stack).ceil();
                    *free.get_mut(at).expect("a counted point") -= new_stacks as i64;
                    p.counted[good] = has + room;
                    k -= room;
                    put.push((at.clone(), p.kind, room as i64));
                }
                put
            };
            for good in 0..n {
                let more = stock[good] - held[good];
                if more >= 1.0 - DUST {
                    for (at, kind, k) in put_in(&mut points, &mut free, good, (more + DUST).floor()) {
                        moves.push(Move { at, kind, good, count: k });
                    }
                } else if more < -DUST {
                    let mut take = (-more - DUST).ceil();
                    for at in lockers.iter().chain(&order) {
                        if take <= 0.0 {
                            break;
                        }
                        let p = points.0.get_mut(at).expect("a counted point");
                        let k = take.min((p.counted[good] + DUST).floor());
                        if k < 1.0 {
                            continue;
                        }
                        p.counted[good] -= k;
                        take -= k;
                        moves.push(Move { at: at.clone(), kind: p.kind, good, count: -(k as i64) });
                    }
                }
            }
            // What the lockers still hold, carried to the stores with room: the town holds the same, in other chests.
            for at in &lockers {
                for good in 0..n {
                    let has = (points.0[at].counted[good] + DUST).floor();
                    if has < 1.0 {
                        continue;
                    }
                    let put = put_in(&mut points, &mut free, good, has);
                    let carried: i64 = put.iter().map(|(_, _, k)| k).sum();
                    if carried == 0 {
                        continue;
                    }
                    points.0.get_mut(at).expect("a counted locker").counted[good] -= carried as f64;
                    moves.push(Move { at: at.clone(), kind: Kind::Locker, good, count: -carried });
                    for (to, kind, k) in put {
                        moves.push(Move { at: to, kind, good, count: k });
                    }
                }
            }
        }
        moves
    }

    /// What the host couldn't do of a move at the point at `at` (if it's still there, of `kind`, a desk, a depot or a
    /// locker): `count` of `good`
    /// didn't fit (above nothing) or wasn't there to take (below), so its chests hold that much less (or more) than
    /// the core took them to (never below nothing). The goods that didn't fit are still the town's, moved in again when
    /// a stocktake shows room. False if there's no such point (a stale view, ignored).
    pub fn unmoved(&mut self, at: &Spot, kind: Kind, good: GoodId, count: i64) -> bool {
        let n = self.content().goods.len();
        let mut points = self.world.resource_mut::<Points>();
        let Some(p) = points.0.get_mut(at).filter(|p| p.kind == kind && shown(p.kind)) else {
            return false;
        };
        p.counted.resize(n, 0.0);
        p.counted[good] = (p.counted[good] - count as f64).max(0.0);
        true
    }

    /// A point's chests as the core takes them to be, if a point stands at `at`.
    pub fn chests(&self, at: &Spot) -> Option<Chests> {
        self.point(at).map(|p| Chests { kind: p.kind, town: p.town, held: p.counted.clone(), free: p.free, short: p.short.clone() })
    }

    /// Where towns' desks stood before they were broken, and whose (a desk put there again is that town's).
    pub fn former_desks(&self) -> &FormerDesks {
        self.world.resource::<FormerDesks>()
    }
}
