//! A town's people, as the mod keeps them (`territory/Territory.java`, `Growth.java`): the households earn a share
//! of what the town makes as wages, and pay for the comforts they use from the warehouse, each villager a ration of
//! each kind a day. Mood heads toward the comforts met, less the town's size, villagers without beds and unpaid
//! wages. A content town grows: a home once its beds are full (a town with villagers short of beds builds at any mood),
//! a child once a bed is free, each after a few days and
//! for a price; its land is the most beds it can hold (its carrying capacity).
//! A town with a [`Roster`] counts its villagers from it, and a child born there is a baby hand.

use bevy_ecs::prelude::*;

use crate::cash_book::{post, CashBook, Reason, Side};
use crate::content::Content;
use crate::fixed::Fixed;
use crate::rules::Step;
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

/// How tasty a food is, a point of it: the saturation it gives a point (the game's: steak 1.6, bread and carrots 1.2, a
/// raw potato 0.6), 1 where the host gives none.
pub fn taste(good: &crate::content::Good) -> f64 {
    if good.saturation > 0.0 && good.food > 0.0 { good.saturation / good.food } else { 1.0 }
}

/// Eats `points` of food from the warehouse, whole items; returns what was eaten. A bag of marbles (John: "a bag of
/// marbles eating approach with tastiness weight"): each food puts in its points held times its [`taste`], and each takes
/// its share of the meal, as the draw would come out on the whole; one that runs out leaves the rest to the others. Not
/// as the mod, which ate in the goods' order: its cows' beef, cooked, sat behind the town's bread. A comfort that is
/// food (cookies) goes in only once nothing else is left.
fn eat(content: &Content, ledger: &mut Ledger, stock: &mut Stock, mut points: f64) -> f64 {
    let mut eaten = 0.0;
    for comforts in [false, true] {
        loop {
            let bag: Vec<(usize, f64)> = content.goods.iter().enumerate()
                .filter(|(i, g)| g.food > 0.0 && g.comfort.is_some() == comforts && stock.0[*i] >= 1.0)
                .map(|(i, g)| (i, stock.0[i].floor() * g.food * taste(g)))
                .collect();
            let total: f64 = bag.iter().map(|(_, m)| m).sum();
            if points <= 0.0 || total <= 0.0 {
                break;
            }
            let want = points;
            for (i, m) in bag {
                let g = &content.goods[i];
                let n = stock.0[i].floor().min((want * m / total / g.food).ceil()).min((points / g.food).ceil());
                if n <= 0.0 || points <= 0.0 {
                    continue;
                }
                stock.0[i] -= n;
                ledger.used[i] += n;
                points -= n * g.food;
                eaten += n * g.food;
            }
        }
    }
    eaten
}

/// A town's meals since its last look (John: "a bag of marbles eating approach with tastiness weight"): `points` shared
/// among the foods it holds as the bag's marbles come out on the whole, each food's points held times its [`taste`]; a
/// comfort that is food (cookies) only once nothing else is left. Each food runs a tab (`owed`, points by good) and
/// whole items come off it as it fills, so a town looked at often eats no more than one looked at once a day; a food
/// that runs out drops its tab (no hunger). Returns how varied the meal was: the number of foods it was as good as made
/// of, each an equal share (the inverse of the sum of the shares squared: two half and half, 2); 0 with none to eat.
pub fn dine(content: &Content, ledger: &mut Ledger, stock: &mut Stock, owed: &mut Vec<f64>, points: f64) -> f64 {
    owed.resize(content.goods.len(), 0.0);
    let held = |comforts: bool, stock: &Stock| -> Vec<(usize, f64)> {
        content.goods.iter().enumerate()
            .filter(|(i, g)| g.food > 0.0 && g.comfort.is_some() == comforts && stock.0[*i] >= 1.0)
            .map(|(i, g)| (i, stock.0[i].floor() * g.food * taste(g))).collect()
    };
    let mut bag = held(false, stock);
    if bag.is_empty() {
        bag = held(true, stock);
    }
    let total: f64 = bag.iter().map(|(_, m)| m).sum();
    for (i, o) in owed.iter_mut().enumerate() {
        if !bag.iter().any(|(b, _)| *b == i) {
            *o = 0.0;
        }
    }
    if total <= 0.0 || points <= 0.0 {
        return 0.0;
    }
    let mut squares = 0.0;
    for &(i, m) in &bag {
        let share = m / total;
        squares += share * share;
        let g = &content.goods[i];
        owed[i] += points * share;
        let n = (owed[i] / g.food + 1e-9).floor().min(stock.0[i].floor());
        if n > 0.0 {
            stock.0[i] -= n;
            ledger.used[i] += n;
            owed[i] = (owed[i] - n * g.food).max(0.0);
        }
    }
    1.0 / squares
}

/// Where a town's mood is heading (`Territory.moodTarget`): the comforts met and its works (a wall, a square, its flower
/// beds), less its size, villagers without beds and unpaid wages.
/// Less its grief for its soldiers fallen (`army.rs`).
pub fn mood_target(d: &crate::content::Dials, people: &People, works: Option<&crate::works::Works>) -> f64 {
    people.comfort_met.iter().sum::<f64>()
        + works.map_or(0.0, |w| w.mood())
        + d.variety_mood * (people.variety.min(d.variety_most) - 1.0).max(0.0) // its meals' variety
        + crate::services::blessing_mood(d, people, works) // a Cleric's blessing (`services.rs`)
        - crate::army::grief(d, people) // its soldiers fallen (`army.rs`)
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
        mut ids: ResMut<crate::roster::Hands>, mut land: ResMut<crate::land::LandBook>,
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
        // Wages: a share of the worth of what the town makes (rounded down to the thousandth: a share paid out), as far
        // as its money goes. The part of a thousandth rounded off isn't carried to the next look (the comforts below,
        // rounded up, likewise): at most a thousandth a look each way stays with the treasury. And the same share of
        // what its lockers took for goods other than crops since (the Farmers are paid from the crops'): a town that
        // makes nothing, selling its Cartographers paper, pays its people too (John: "pay its people"), so they buy
        // their comforts as other towns' do rather than the treasury giving them away.
        let sold = crate::farms::locker_sales(&content, &book, t, (&mut people.sales_day, &mut people.sales_seen), now, false);
        let due = Fixed::floor(d.labour_share * (trades.makes.iter().zip(&content.goods).map(|(m, g)| m * g.value).sum::<f64>()
            * days + sold));
        let paid = due.min(money.0.at_least_zero());
        post(&mut ledger, &mut book, Side::treasury(t, &mut money.0), Side::households(t, &mut people.purse), paid,
            Reason::Wages, now, day);
        people.paid_share = if due.is_positive() { paid.ratio(due) } else { 1.0 };
        // And what the grown villagers earn by their work (the dial `hand_wage`): new money, into their purses, which
        // they spend on the town's comforts (John: "Villagers earn wages").
        let grown = roster.as_deref().map_or(people.villagers, |r| r.hands.iter()
            .filter(|h| h.baby_until.is_none_or(|b| b <= now)).count() as f64);
        let earned = Fixed::floor(d.hand_wage * grown * days);
        post(&mut ledger, &mut book, Side::outside(), Side::households(t, &mut people.purse), earned, Reason::Earned,
            now, day);
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
            // Their worth rounded up to the thousandth (a price the households pay), as far as their purse goes.
            let pays = Fixed::ceil(used).min(people.purse.at_least_zero());
            post(&mut ledger, &mut book, Side::households(t, &mut people.purse), Side::treasury(t, &mut money.0), pays,
                Reason::Comforts, now, day);
            people.comfort_met[kind] = if due > 0.0 { (1.0 - owed.max(0.0) / due).clamp(0.0, 1.0) } else { 0.0 };
            people.comfort_owed[kind] = owed.min(0.0);
        }
        // Meals: each villager `meal_points` a day from the bag of what the town holds, the tastiest likeliest (not as the
        // mod, whose villagers ate only to be born). Short, it eats what there is: no hunger, but no variety either
        // (John: "daily meals, no hunger"; "a variety of food happiness weight").
        let meal = people.villagers * d.meal_points * days;
        people.variety = dine(&content, &mut ledger, &mut stock, &mut people.meal_owed, meal);
        // Mood heads toward the comforts met, less its size, villagers without beds and unpaid wages.
        let target = mood_target(d, people, works);
        people.mood += (target - people.mood) * (1.0 - crate::fixed::halved(days, d.mood_half_life_days));
        // A home, once the beds are full, as far as the land holds: the households pay what they can, the town the rest.
        // Villagers without a bed get one whatever the mood (not as the mod's `Territory`, whose homeless town, its
        // mood sunk by them, never builds); content with every bed full, the town grows.
        // (What the treasury holds beyond its reserve: its trading capital, wage float and wagons' road money; not the
        // part kept back for this home.)
        let spare = money.0 - crate::rules::farms::reserve(&content, trades, farm) + trades.home_reserve;
        let homeless = people.villagers > people.beds;
        if (homeless || people.mood >= d.home_mood && people.villagers >= people.beds) && people.beds + 2.0 <= people.land {
            people.home_progress = (people.home_progress + days / d.home_days).min(1.0);
        }
        let home_cost = Fixed::ceil(d.home_cost);
        if people.home_progress >= 1.0 && people.purse + spare.at_least_zero() >= home_cost {
            let theirs = people.purse.min(home_cost);
            post(&mut ledger, &mut book, Side::households(t, &mut people.purse), Side::outside(), theirs, Reason::Home,
                now, day);
            post(&mut ledger, &mut book, Side::treasury(t, &mut money.0), Side::outside(), home_cost - theirs,
                Reason::Home, now, day);
            people.beds += 2.0;
            people.home_progress = 0.0;
            people.homes_built += 1;
            *land.homes.entry(t).or_default() += 1; // for the host to lay (`land.rs`)
        }
        // A child, once a bed is free: food for it, and the town's money.
        if people.mood >= d.birth_mood && people.villagers < people.beds {
            people.birth_progress = (people.birth_progress + days / d.birth_days).min(1.0);
        }
        let spare = money.0 - crate::rules::farms::reserve(&content, trades, farm);
        let birth_cost = Fixed::ceil(d.birth_cost);
        if people.birth_progress >= 1.0 && people.villagers < people.beds && spare >= birth_cost
            && food_points(&content, &stock) >= d.birth_food {
            eat(&content, &mut ledger, &mut stock, d.birth_food);
            post(&mut ledger, &mut book, Side::treasury(t, &mut money.0), Side::outside(), birth_cost, Reason::Child,
                now, day);
            // A town with a roster has a baby hand (`Growth.bear`): a resident, not yet a worker.
            if let Some(roster) = roster.as_deref_mut() {
                roster.hands.push(crate::roster::Hand { id: ids.next(), ..crate::roster::Hand::baby(&content, clock.now) });
                people.villagers = roster.residents() as f64;
            } else {
                people.villagers += 1.0;
            }
            people.birth_progress = 0.0;
            people.born += 1;
        }
    }
}
