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

/// The mod's comfort files, for a pack that sends none: good (bare name), villager-days a piece serves, quality. A book
/// is passed round and read again (ten days, John: "reading is being consumed too aggressively"), a book and quill
/// written in for half that; the rest cover their worth in rations.
pub const MOD_COMFORTS: [(&str, f64, f64); 2] = [("book", 10.0, 1.0), ("writable_book", 5.0, 1.0)];

/// The good a home's or a library's sockets take: a bookshelf (by its bare name).
pub const SHELF: &str = "bookshelf";

/// A town's sockets for shelves still empty: its homes' (a bed each) and, with a library built, the library's.
pub fn empty_sockets(content: &Content, people: &People, works: Option<&crate::works::Works>) -> u32 {
    let library = if works.is_some_and(|w| w.built(crate::works::Kind::Library)) { content.dials.library_sockets } else { 0 };
    (people.beds.max(0.0) as u32).saturating_sub(people.home_shelves) + library.saturating_sub(people.library_shelves)
}

/// The share of its villagers' reading its shelves cover a day, for good: a home's shelf its bed's villager, a library's
/// its share of the town (all of it, full), each `shelf_reading` of a ration.
pub fn passive_reading(content: &Content, people: &People) -> f64 {
    if people.villagers <= 0.0 {
        return 0.0;
    }
    let library = people.library_shelves as f64 / content.dials.library_sockets.max(1) as f64 * people.villagers;
    let covered = (people.home_shelves as f64 + library).min(people.villagers);
    content.dials.shelf_reading.clamp(0.0, 1.0) * covered / people.villagers
}

/// What a town holds of a kind of comfort, in emeralds' worth.
pub fn comfort_held(content: &Content, stock: &Stock, kind: usize) -> f64 {
    content.goods.iter().enumerate().filter(|(_, g)| g.comfort == Some(kind)).map(|(i, _)| stock.0[i] * content.comfort_worth(i)).sum()
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
        + people.comfort_fine.iter().sum::<f64>() // the finer tier's lift (`State::People::fine`)
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
        mut ids: ResMut<crate::roster::Hands>, mut land: ResMut<crate::land::LandBook>, order: Res<crate::state::Towns>,
        mut towns: Query<(&Town, &mut People, &mut Stock, &mut Money, &Trades, Option<&mut Roster>, Option<&crate::farms::Farm>,
            Option<&crate::works::Works>)>) {
    let d = &content.dials;
    let k = content.comforts.len();
    let (now, day) = (clock.now, content.day_ticks);
    // In the towns' order: a child's hand takes the next id, and a query's order isn't a save's (seed 1717's 240 days:
    // two children born on one look took each other's ids after a load, and the run parted from its replay).
    for &e in order.0.iter() {
        let Ok((town, mut people, mut stock, mut money, trades, mut roster, farm, works)) = towns.get_mut(e) else { continue };
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
        if people.comfort_fine.len() != k {
            people.comfort_fine = vec![0.0; k];
        }
        if people.fine_banked.len() != k {
            people.fine_banked = vec![0.0; k];
            people.fine_cover = vec![0.0; k];
        }
        // (The finer pieces the warehouse still holds: its wagons and lockers move them as plain ones.)
        for (&g, n) in people.fine.iter_mut() {
            *n = n.min(stock.0.get(g).copied().unwrap_or(0.0).max(0.0));
        }
        people.fine.retain(|_, n| *n > 0.0);
        // Shelves in its warehouse are set in its empty sockets, the library's first (the town's whole), then its homes'.
        if let Some(shelf) = content.goods.iter().position(|g| g.name.rsplit(':').next() == Some(SHELF)) {
            let library = if works.is_some_and(|w| w.built(crate::works::Kind::Library)) { d.library_sockets } else { 0 };
            while stock.0[shelf] >= 1.0 && empty_sockets(&content, people, works) > 0 {
                if people.library_shelves < library {
                    people.library_shelves += 1;
                } else {
                    people.home_shelves += 1;
                }
                stock.0[shelf] -= 1.0;
                ledger.used[shelf] += 1.0;
            }
        }
        let reading = content.comfort("reading");
        let passive = passive_reading(&content, people);
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
            // (Of reading, what its shelves read for it, for good.)
            let shelved = if Some(kind) == reading { due * passive } else { 0.0 };
            let mut owed = people.comfort_owed[kind] + due - shelved;
            let mut goods: Vec<usize> = (0..content.goods.len()).filter(|&i| content.goods[i].comfort == Some(kind)).collect();
            goods.sort_by(|&a, &b| content.goods[a].value.total_cmp(&content.goods[b].value));
            let mut used = 0.0;
            let mut finer = 0.0;
            let mut cover = 0.0;
            for i in goods {
                if owed <= 0.0 {
                    break;
                }
                // (What a piece covers of the rations, `comfort_worth`, and what it costs, its worth.)
                let (value, covers) = (content.goods[i].value.max(1e-9), content.comfort_worth(i));
                let n = stock.0[i].min((owed / covers).ceil());
                stock.0[i] -= n;
                ledger.used[i] += n;
                owed -= n * covers;
                cover += n * covers;
                // Its quality's lift, and the finer pieces first: each worth `fine_quality` plain ones, and paid for so.
                finer += n * covers * (content.goods[i].quality - 1.0).max(0.0);
                let f = people.fine.get(&i).copied().unwrap_or(0.0).min(n);
                if f > 0.0 {
                    *people.fine.get_mut(&i).unwrap() -= f;
                    finer += f * covers * (d.fine_quality - 1.0).max(0.0);
                }
                used += n * value + f * value * (d.fine_quality - 1.0).max(0.0);
            }
            // The finer pieces' comfort is banked as they're put on, with the rations they cover, and taken as those
            // rations are used: a book of 1.5 read over ten days lifts each of them by half.
            people.fine_banked[kind] += finer;
            people.fine_cover[kind] += cover;
            let met = (due - owed.max(0.0)).max(0.0);
            let share = if people.fine_cover[kind] > 0.0 { (met / people.fine_cover[kind]).min(1.0) } else { 0.0 };
            let lift = people.fine_banked[kind] * share;
            people.fine_banked[kind] -= lift;
            people.fine_cover[kind] -= met.min(people.fine_cover[kind]);
            people.comfort_fine[kind] = if due > 0.0 { (lift / due).min(1.0) } else { 0.0 };
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
        let spare = money.0 - crate::rules::farms::build_reserve(&content, trades, farm) + trades.home_reserve;
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
