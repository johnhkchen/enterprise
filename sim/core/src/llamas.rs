//! A town's llamas: the trains its roads use where a wagon can't go (`map.rs` `Carrier::Llama`: a river with no bridge,
//! steeper flanks, rougher ground). Not in the mod.
//!
//! - **The Merchant brings two**, its class's animals, once (`merchant_llamas`), when the town first has a Merchant at
//!   its desk.
//! - **A train is a wagon's stand-in**: as many llamas as the load wants, a llama to `llama_slots` stacks (and
//!   `llama_weight`, a pallet each), no more than `llama_train` nor than are home (`train`). Its crew and their packs
//!   go as with a wagon. The llamas are away with the train and home with it; a road whose town has none home stays
//!   ("no llamas at home").
//! - **Farmers breed more as needed**: while the town's llama roads want more llamas (their last loads' trains, all
//!   told) than it has, home and away, a Farmer breeds one every `llama_breed_days` from `llama_breed_wheat` of the
//!   warehouse's wheat (a hay bale's: llamas breed on hay). Llamas eat on the road as a wagon's team does, and nothing
//!   at home: they graze.

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::content::Content;
use crate::herds::Herds;
use crate::roster::Roster;
use crate::state::{Clock, Ledger, Stock, Town};

/// A Farmer, who breeds the llamas.
pub const FARMER: &str = "farmer";

/// A town's llamas: how many are home and away with its trains, whether its Merchant has brought its two, what each
/// llama road (by its route's `seq`) wanted for its last load, and when a llama was last bred.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Llamas {
    pub home: u32,
    pub away: u32,
    pub seeded: bool,
    pub wanted: BTreeMap<usize, u32>,
    pub bred: Option<u64>,
}

impl Llamas {
    /// The llamas the town's roads want, all told.
    pub fn want(&self) -> u32 {
        self.wanted.values().sum()
    }
}

/// The llamas a load of `stacks` weighing `weight` wants: enough for its stacks and its weight, at least one, no more
/// than a train.
pub fn wanted(content: &Content, stacks: u32, weight: f64) -> u32 {
    let d = &content.dials;
    let by_slots = stacks.div_ceil(d.llama_slots.max(1));
    let by_weight = if d.llama_weight > 0.0 { (weight / d.llama_weight - 1e-9).ceil().max(0.0) as u32 } else { 0 };
    by_slots.max(by_weight).clamp(1, d.llama_train.max(1))
}

/// The train a load wants of `home` llamas: as many as it wants, as far as are home (none: no train).
pub fn train(content: &Content, stacks: u32, weight: f64, home: u32) -> u32 {
    wanted(content, stacks, weight).min(home)
}

/// What a carrier holds before its crew's packs: a wagon's `wagon_slots`, `wagon_weight` and `wagon_pallets`, or a
/// train of `llamas` (0: a wagon) at `llama_slots`, `llama_weight` and a pallet each.
pub fn team_caps(content: &Content, llamas: u32) -> (u32, f64, u32) {
    let d = &content.dials;
    if llamas == 0 {
        (d.wagon_slots, d.wagon_weight, d.wagon_pallets)
    } else {
        (d.llama_slots * llamas, d.llama_weight * llamas as f64, llamas)
    }
}

/// A town's llamas tended at `now`: its Merchant's two brought, once; a llama bred by its Farmer while its roads want
/// more than it has, every `llama_breed_days`, if the warehouse holds the wheat.
pub fn tend(content: &Content, llamas: &mut Llamas, roster: Option<&Roster>, stock: &mut [f64], ledger: &mut Ledger,
        now: u64) {
    let d = &content.dials;
    let Some(roster) = roster else { return };
    if !llamas.seeded && roster.merchant.is_some() {
        llamas.seeded = true;
        llamas.home += d.merchant_llamas;
    }
    let farmer = content.trade(FARMER).is_some_and(|t| roster.count(t) > 0);
    if !farmer || llamas.home + llamas.away >= llamas.want() {
        return;
    }
    let every = (d.llama_breed_days * content.day_ticks as f64) as u64;
    if llamas.bred.is_some_and(|t| now < t + every) {
        return;
    }
    let Some(wheat) = content.good("wheat") else { return };
    if stock[wheat] < d.llama_breed_wheat - 1e-9 {
        return;
    }
    stock[wheat] -= d.llama_breed_wheat;
    ledger.used[wheat] += d.llama_breed_wheat;
    llamas.home += 1;
    llamas.bred = Some(now);
}

/// Each town's llamas tended, in town order.
pub fn tend_towns(clock: Res<Clock>, content: Res<Content>, mut ledger: ResMut<Ledger>,
        mut towns: Query<(&Town, &mut Herds, &mut Stock, Option<&Roster>)>) {
    let mut order: Vec<_> = towns.iter_mut().collect();
    order.sort_by_key(|(town, ..)| town.id);
    for (_, mut herds, mut stock, roster) in order {
        if roster.is_none() {
            continue;
        }
        let mut llamas = herds.llamas.clone();
        tend(&content, &mut llamas, roster, &mut stock.0, &mut ledger, clock.now);
        if llamas != herds.llamas {
            herds.llamas = llamas;
        }
    }
}

impl crate::society::Society {
    /// A town's llamas.
    pub fn llamas(&self, town: usize) -> Llamas {
        self.world.get::<Herds>(self.towns[town]).map(|h| h.llamas.clone()).unwrap_or_default()
    }

    /// Sets how many llamas a town has at home (its Merchant's two counted brought).
    pub fn set_llamas(&mut self, town: usize, home: u32) {
        let mut herds = self.world.get_mut::<Herds>(self.towns[town]).expect("a town");
        herds.llamas.home = home;
        herds.llamas.seeded = true;
    }
}
