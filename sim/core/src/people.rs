//! A town's people, as the mod keeps them (`territory/Territory.java`, `Growth.java`): the households earn a share
//! of what the town makes as wages, and pay for the comforts they use from the warehouse, each villager a ration of
//! each kind a day. Mood heads toward the comforts met, less the town's size, villagers without beds and unpaid
//! wages. A content town grows: a home once its beds are full, a child once a bed is free, each after a few days and
//! for a price; its land is the most beds it can hold (its carrying capacity).
//! A town with a [`Roster`] counts its villagers from it, and a child born there is a baby hand.

use bevy_ecs::prelude::*;

use crate::cash_book::{post, CashBook, Reason, Side};
use crate::content::Content;
use crate::rules::{trading_capital, Step};
use crate::state::*;

/// What a town holds of a kind of comfort, in emeralds' worth.
pub fn comfort_held(content: &Content, stock: &Stock, kind: usize) -> f64 {
    content.goods.iter().enumerate().filter(|(_, g)| g.comfort == Some(kind)).map(|(i, g)| stock.0[i] * g.value).sum()
}

/// What a town aims to hold of each kind of comfort, in emeralds' worth: `comfort_days` of its people's rations.
pub fn comfort_par(content: &Content, people: &People) -> f64 {
    people.villagers * content.dials.ration * content.dials.comfort_days
}

/// What it keeps of each kind when others would buy it: `comfort_keep_days` of rations.
pub fn comfort_keep(content: &Content, people: &People) -> f64 {
    people.villagers * content.dials.ration * content.dials.comfort_days.min(content.dials.comfort_keep_days)
}

/// Food points in a town's warehouse.
pub fn food_points(content: &Content, stock: &Stock) -> f64 {
    content.goods.iter().enumerate().map(|(i, g)| stock.0[i] * g.food).sum()
}

/// Eats `points` of food from the warehouse (any food, in the order of the goods); returns what was eaten.
fn eat(content: &Content, ledger: &mut Ledger, stock: &mut Stock, mut points: f64) -> f64 {
    let mut eaten = 0.0;
    for (i, g) in content.goods.iter().enumerate() {
        if points <= 0.0 || g.food <= 0.0 {
            continue;
        }
        let n = stock.0[i].min((points / g.food).ceil());
        stock.0[i] -= n;
        ledger.used[i] += n;
        points -= n * g.food;
        eaten += n * g.food;
    }
    eaten
}

/// Where a town's mood is heading (`Territory.moodTarget`): the comforts met and its works (a wall, a square, its flower
/// beds), less its size, villagers without beds and unpaid wages.
pub fn mood_target(d: &crate::content::Dials, people: &People, works: Option<&crate::works::Works>) -> f64 {
    people.comfort_met.iter().sum::<f64>()
        + works.map_or(0.0, |w| w.mood())
        - people.villagers / d.villagers_per_want
        - d.crowding_mood * (people.villagers - people.beds).max(0.0)
        - d.unpaid_mood * (1.0 - people.paid_share)
}

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems(live.in_set(Step::Touch).after(crate::rules::touch::catch_up));
}

/// The town's people live the time since they were last counted: wages, comforts, mood, a home, a child.
#[allow(clippy::type_complexity)]
pub fn live(clock: Res<Clock>, content: Res<Content>, mut ledger: ResMut<Ledger>, mut book: ResMut<CashBook>,
        mut towns: Query<(&Town, &mut People, &mut Stock, &mut Money, &Trades, Option<&mut Roster>, Option<&crate::farms::Farm>,
            Option<&crate::works::Works>)>) {
    let d = &content.dials;
    let k = content.comforts.len();
    let (now, day) = (clock.now, content.day_ticks);
    for (town, mut people, mut stock, mut money, trades, mut roster, farm, works) in &mut towns {
        let t = town.id;
        let people = &mut *people;
        let days = clock.now.saturating_sub(people.last) as f64 / content.day_ticks as f64;
        people.last = clock.now;
        if days <= 0.0 || people.villagers <= 0.0 {
            continue;
        }
        if people.comfort_met.len() != k {
            people.comfort_met = vec![0.0; k];
            people.comfort_owed = vec![0.0; k];
        }
        // Wages: a share of the worth of what the town makes, as far as its money goes.
        let due = d.labour_share * trades.makes.iter().zip(&content.goods).map(|(m, g)| m * g.value).sum::<f64>() * days;
        let paid = due.min(money.0.max(0.0));
        post(&mut ledger, &mut book, Side::treasury(t, &mut money.0), Side::households(t, &mut people.purse), paid,
            Reason::Wages, now, day);
        people.paid_share = if due > 0.0 { paid / due } else { 1.0 };
        // And of the Farmers' wages at their last settlement (`farms.rs`).
        if let Some(farm) = farm {
            people.paid_share = people.paid_share.min(farm.paid_share);
        }
        // Comforts: each villager a ration of each kind a day, the cheapest goods of the kind first, whole ones (a big
        // one leaves credit); the households pay the town for what they use, as far as their purse goes.
        for kind in 0..k {
            let due = people.villagers * d.ration * days;
            let mut owed = people.comfort_owed[kind] + due;
            let mut goods: Vec<usize> = (0..content.goods.len()).filter(|&i| content.goods[i].comfort == Some(kind)).collect();
            goods.sort_by(|&a, &b| content.goods[a].value.total_cmp(&content.goods[b].value));
            let mut used = 0.0;
            for i in goods {
                if owed <= 0.0 {
                    break;
                }
                let value = content.goods[i].value.max(1e-9);
                let n = stock.0[i].min((owed / value).ceil());
                stock.0[i] -= n;
                ledger.used[i] += n;
                owed -= n * value;
                used += n * value;
            }
            let pays = used.min(people.purse);
            post(&mut ledger, &mut book, Side::households(t, &mut people.purse), Side::treasury(t, &mut money.0), pays,
                Reason::Comforts, now, day);
            people.comfort_met[kind] = if due > 0.0 { (1.0 - owed.max(0.0) / due).clamp(0.0, 1.0) } else { 0.0 };
            people.comfort_owed[kind] = owed.min(0.0);
        }
        // Mood heads toward the comforts met, less its size, villagers without beds and unpaid wages.
        let target = mood_target(d, people, works);
        people.mood += (target - people.mood) * (1.0 - 0.5f64.powf(days / d.mood_half_life_days));
        // A home, once the beds are full, as far as the land holds: the households pay what they can, the town the rest.
        let spare = money.0 - trading_capital(&content, trades);
        if people.mood >= d.home_mood && people.villagers >= people.beds && people.beds + 2.0 <= people.land {
            people.home_progress = (people.home_progress + days / d.home_days).min(1.0);
        }
        if people.home_progress >= 1.0 && people.purse + spare.max(0.0) >= d.home_cost {
            let theirs = people.purse.min(d.home_cost);
            post(&mut ledger, &mut book, Side::households(t, &mut people.purse), Side::outside(), theirs, Reason::Home,
                now, day);
            post(&mut ledger, &mut book, Side::treasury(t, &mut money.0), Side::outside(), d.home_cost - theirs,
                Reason::Home, now, day);
            people.beds += 2.0;
            people.home_progress = 0.0;
            people.homes_built += 1;
        }
        // A child, once a bed is free: food for it, and the town's money.
        if people.mood >= d.birth_mood && people.villagers < people.beds {
            people.birth_progress = (people.birth_progress + days / d.birth_days).min(1.0);
        }
        let spare = money.0 - trading_capital(&content, trades);
        if people.birth_progress >= 1.0 && people.villagers < people.beds && spare >= d.birth_cost
            && food_points(&content, &stock) >= d.birth_food {
            eat(&content, &mut ledger, &mut stock, d.birth_food);
            post(&mut ledger, &mut book, Side::treasury(t, &mut money.0), Side::outside(), d.birth_cost, Reason::Child,
                now, day);
            // A town with a roster has a baby hand (`Growth.bear`): a resident, not yet a worker.
            if let Some(roster) = roster.as_deref_mut() {
                roster.hands.push(crate::roster::Hand::baby(&content, clock.now));
                people.villagers = roster.residents() as f64;
            } else {
                people.villagers += 1.0;
            }
            people.birth_progress = 0.0;
            people.born += 1;
        }
    }
}
