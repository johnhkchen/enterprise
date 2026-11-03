//! Security's steps (`security.rs`): once a day each town reckons its defence and protection, sets and pays its watch,
//! and suffers what its protection doesn't keep off: a raid if it's rich, theft, an attack in the night. What's taken
//! leaves the world. A wagon home again by land meets the bandits of the lonely road it crossed.

use bevy_ecs::prelude::*;

use super::Step;
use crate::cash_book::{post, CashBook, Reason, Side};
use crate::content::Content;
use crate::fixed::Fixed;
use crate::security::*;
use crate::state::*;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems((keep.in_set(Step::Review).after(super::taxes::assess).after(super::works::review)
        .before(super::jobs::relocate).before(super::jobs::review),
        bandits.in_set(Step::Services).after(super::taxes::congestion)));
}

/// A share of every good in the warehouse taken out of the world; what it was worth.
fn take_goods(content: &Content, ledger: &mut Ledger, stock: &mut Stock, share: f64) -> f64 {
    let mut worth = 0.0;
    for (g, n) in stock.0.iter_mut().enumerate() {
        if *n <= 0.0 {
            continue;
        }
        let taken = *n * share;
        *n -= taken;
        ledger.used[g] += taken;
        worth += taken * content.goods[g].value;
    }
    worth
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn keep(clock: Res<Clock>, content: Res<Content>, towns: Res<Towns>, territory: Res<crate::territory::Territory>,
        mut security: ResMut<Security>, mut ledger: ResMut<Ledger>, mut book: ResMut<CashBook>, seed: Res<Rng>,
        mut dice: ResMut<Dice>,
        mut q: Query<(&Town, &mut People, &mut Stock, &mut Money, &Trades, Option<&crate::farms::Farm>,
            Option<&crate::roster::Roster>, Option<&crate::works::Works>)>) {
    let (now, day) = (clock.now, content.day_ticks.max(1));
    let d = &content.dials;
    let lantern = content.good("lantern");
    for &e in &towns.0 {
        let Ok((town, mut people, mut stock, mut money, trades, farm, roster, works)) = q.get_mut(e) else { continue };
        let t = town.id;
        // A hamlet, where everyone knows everyone, keeps no watch and draws no raiders or thieves.
        if people.villagers < d.secure_villagers {
            if let Some(w) = security.towns.get_mut(&t) {
                w.watchmen = 0;
                w.kept_at = None;
            }
            continue;
        }
        let watch = security.towns.entry(t).or_default();
        let last = match watch.kept_at {
            Some(k) if now >= k + day => k,
            Some(_) => continue,
            None => {
                watch.kept_at = Some(now);
                continue;
            }
        };
        watch.kept_at = Some(now);
        let days = ((now - last) as f64 / day as f64).min(crate::territory::MAX_DAYS);
        let rng = dice.of(t, &seed);
        // Its watch: one for each `watch_per_villagers`, twice that while it fears raiders, as far as it can pay
        // `watch_days` of them beyond its reserve; by one a day.
        let afraid = watch.raided.is_some_and(|r| now < r + (d.fear_days * day as f64) as u64);
        let want = (people.villagers / d.watch_per_villagers.max(1.0) * if afraid { 2.0 } else { 1.0 }).ceil() as u32;
        let spare = money.0 - super::farms::build_reserve(&content, trades, farm);
        let afford = (spare.to_f64() / (d.guard_pay * d.watch_days).max(1e-9)).floor().max(0.0) as u32;
        let target = want.min(afford);
        if target > watch.watchmen {
            watch.watchmen += 1;
        } else if target < watch.watchmen {
            watch.watchmen -= 1;
        }
        let pay = Fixed::floor(watch.watchmen as f64 * d.guard_pay * days);
        if pay > Fixed::ZERO {
            let paid = pay.min(money.0.at_least_zero());
            post(&mut ledger, &mut book, Side::treasury(t, &mut money.0), Side::outside(), paid, Reason::Other("watch".into()),
                now, day);
            if paid < pay {
                watch.watchmen = watch.watchmen.saturating_sub(1);
            }
        }
        // Its defence and protection.
        let h = territory.holding(t);
        let militia = roster.map_or(0, |r| r.idle(&content).len());
        let walled = works.map_or(0.0, |w| w.walled());
        let tower = works.is_some_and(|w| w.built(crate::works::Kind::Watchtower));
        watch.defence = defence(d, people.villagers, watch.watchmen as usize, militia, h.map_or(0.0, |h| h.armed),
            h.map_or(0.0, |h| h.armoured), walled, tower);
        watch.protection = protection(watch.defence, needed(d, people.villagers));
        let open = 1.0 - watch.protection;
        // Theft from within: of its warehouse and its households' purses.
        let share = (d.theft_rate * open * days).min(1.0);
        let goods = take_goods(&content, &mut ledger, &mut stock, share);
        let purses = Fixed::floor(people.purse.at_least_zero().to_f64() * share);
        if purses > Fixed::ZERO {
            post(&mut ledger, &mut book, Side::households(t, &mut people.purse), Side::outside(), purses,
                Reason::Other("stolen".into()), now, day);
        }
        watch.stolen += Fixed::floor(goods) + purses;
        // A raid on a rich town: its treasury and its goods.
        let wealth = money.0.at_least_zero().to_f64() + stock.0.iter().zip(&content.goods).map(|(n, g)| n.max(0.0) * g.value).sum::<f64>();
        let chance = d.raid_chance * (wealth / d.raid_wealth.max(1.0)).min(1.0) * open * days;
        if (rng.below(1_000_000) as f64) < chance * 1e6 {
            let take = d.raid_take * open;
            let goods = take_goods(&content, &mut ledger, &mut stock, take);
            let coin = Fixed::floor(money.0.at_least_zero().to_f64() * take);
            post(&mut ledger, &mut book, Side::treasury(t, &mut money.0), Side::outside(), coin,
                Reason::Other("raided".into()), now, day);
            watch.raided = Some(now);
            watch.raids += 1;
            watch.raided_of += coin + Fixed::floor(goods);
            frighten(&content, &mut people, now, d.raid_mood, d.fear_days);
        }
        // Monsters at night, less its walls and its lanterns' light.
        let lit = lantern.map_or(0.0, |g| (stock.0[g] * PER_LANTERN / people.villagers).min(1.0));
        let night = d.night_chance * (1.0 - walled) * (1.0 - lit) * open * days;
        if (rng.below(1_000_000) as f64) < night * 1e6 {
            watch.nights += 1;
            frighten(&content, &mut people, now, d.night_mood, d.fear_days / 2.0);
        }
    }
}

/// A wagon home again by land, robbed on the lonely hexes of its road: each beyond a hex of either end, held by no town
/// that keeps a watch or posts, with no stop within 2 hexes, a chance of `bandit_chance` there and back, less its escort (its home's watch, by how
/// well armed: half at best). Robbers take `bandit_take` of its purse and its load, out of the world.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn bandits(clock: Res<Clock>, content: Res<Content>, territory: Res<crate::territory::Territory>,
        ways: Res<crate::waystations::Waystations>, mut security: ResMut<Security>, mut ledger: ResMut<Ledger>,
        mut book: ResMut<CashBook>, seed: Res<Rng>, mut dice: ResMut<Dice>,
        mut routes: Query<(Entity, &Route, &mut RouteState, Option<&RoutePath>)>) {
    let (now, day) = (clock.now, content.day_ticks);
    let d = &content.dials;
    if d.bandit_chance <= 0.0 {
        return;
    }
    let guarded = |town: usize, sec: &Security| sec.watchmen(town) > 0
        || territory.holding(town).is_some_and(|h| !h.posts.is_empty());
    for entity in super::in_order(routes.iter().map(|(e, r, _, _)| (e, r))) {
        let (_, route, mut state, path) = routes.get_mut(entity).unwrap();
        let Some(path) = path else { continue };
        if state.arrived != Some(Leg::Home) || route.carrier == crate::map::Carrier::Ship {
            continue;
        }
        let route = *route;
        let (start, goal) = (path.0.first().copied(), path.0.last().copied());
        let doorstep = |h: crate::hex::Hex| [start, goal].into_iter().flatten().any(|e| e.distance(h) <= 1);
        let lonely = path.0.iter().filter(|&&h| {
            !doorstep(h) && !territory.owner(h).is_some_and(|o| guarded(o, &security))
                && !ways.stations.iter().any(|s| s.hex.distance(h) <= 2)
        }).count() as f64;
        if lonely == 0.0 {
            continue;
        }
        let escort = 0.5 * (security.watchmen(route.from_id) as f64).min(1.0)
            * territory.holding(route.from_id).map_or(0.0, |h| h.armed);
        let chance = (2.0 * lonely * d.bandit_chance * (1.0 - escort)).min(1.0);
        if (dice.of(route.from_id, &seed).below(1_000_000) as f64) >= chance * 1e6 {
            continue;
        }
        let Some(wagon) = state.wagon.as_mut() else { continue };
        let coin = Fixed::floor(wagon.purse.at_least_zero().to_f64() * d.bandit_take);
        post(&mut ledger, &mut book, Side::wagon(route.seq, &mut wagon.purse), Side::outside(), coin,
            Reason::Other("robbed".into()), now, day);
        let mut worth = 0.0;
        for (g, n) in wagon.cargo.iter_mut().enumerate() {
            if *n > 0.0 {
                let taken = (*n * d.bandit_take).floor();
                *n -= taken;
                ledger.used[g] += taken;
                worth += taken * content.goods[g].value;
            }
        }
        let w = security.towns.entry(route.from_id).or_default();
        w.robbed += 1;
        w.robbed_of += coin + Fixed::floor(worth);
    }
}
