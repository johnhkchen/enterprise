//! Review: a town's idle hands find work (`territory/Jobs.java` `employ`; see `jobs.rs`). Once a day, towns in their
//! order, after the Merchant's review of its lockers (which counts the days a trade has done nothing) and the
//! settlers' (trained adults go with caravans first): one unemployed adult, or a slack one, takes a trade drawn from
//! the town's bag of marbles when the town can pay for its stall. Not while the town is raising a party of settlers
//! (`Territory.grow`: its idle hands wait for that), nor on the day one left (the mod's `grow` hires before `consider`
//! sends, so the party's day hires nobody). An idle adult it leaves without work is noted ([`Hiring::left_idle`]): the
//! squad trains only such a hand, after this (`rules/squad.rs` `train`; the mod's `Territory.grow` trained the squad
//! before anything, and a town's first day turned the villager who'd have farmed into a second Merchant).

use bevy_ecs::prelude::*;

use super::Step;
use crate::bills::Bills;
use crate::cash_book::{post, CashBook, Reason, Side};
use crate::content::Content;
use crate::farms::{Deposits, Farm};
use crate::jobs::{employ, Hiring, Place, Signals};
use crate::lockers::{Books, Lockers};
use crate::settlers::{raising, Settlers};
use crate::state::*;

pub fn register(schedule: &mut Schedule) {
    // After the census and the farm's review, as the mod's `grow` hires after `afterCensus` counts the town.
    schedule.add_systems(review.in_set(Step::Review).after(super::lockers::review).after(super::settlers::review)
        .after(super::farms::review).after(super::territory::review));
    schedule.add_systems(relocate.in_set(Step::Review).after(super::lockers::review).after(super::settlers::review)
        .after(super::farms::review).after(super::territory::review).after(super::works::review)
        .after(super::atlas::consider).before(review));
}

/// A town short of a comfort it holds the makings of, with no hand of its maker, none idle and a bed free, hires one from
/// a town one or two roads away (John: "Allow a long cooldown retraining or hire-relocated-worker mechanism"; "Two
/// roads": the idle sit in villages a road leaves only for a neighbour): one of two or more of that trade at home there,
/// the least trained, its trade and level kept; else an idle adult of a town that left one idle at its last review (no work there for it), to learn the trade. Paid its stall, on the road its way's
/// travel ticks (`jobs::Mover`, landed by `Society::land_movers`). Once in `comfort_retrain_days`, as a retraining is
/// (the nearest such town first, a trade's spare before an idle hand; with none, its review may retrain one of its
/// own, `jobs::employ`). Seed 1717's 60 days had a town with two of a comfort's trade once (two Librarians, day 60),
/// while settlers' villages kept three idle each.
#[allow(clippy::type_complexity)]
pub fn relocate(clock: Res<Clock>, content: Res<Content>, order: Res<Towns>, mut hiring: ResMut<Hiring>,
        mut ledger: ResMut<Ledger>, mut book: ResMut<CashBook>, routes: Query<&Route>,
        mut towns: Query<(&Town, &mut Money, &mut People, &Stock, Option<&mut Roster>, Option<&mut Lockers>)>) {
    let (now, today) = (clock.now, clock.now / content.day_ticks.max(1));
    let stall = crate::fixed::Fixed::ceil(content.dials.stall_emeralds);
    let mut roads: Vec<&Route> = routes.iter().collect();
    roads.sort_by_key(|r| r.seq);
    for &e in &order.0 {
        let want = {
            let Ok((town, money, people, stock, Some(roster), _)) = towns.get(e) else { continue };
            let cooled = roster.comfort_retrained.is_none_or(|d| {
                now.saturating_sub(d) as f64 >= content.dials.comfort_retrain_days * content.day_ticks as f64
            });
            if roster.merchant.is_none() || !roster.idle(&content).is_empty() || !cooled
                || people.beds - people.villagers < 1.0 || money.0 < stall || hiring.movers.iter().any(|m| m.to == town.id) {
                continue;
            }
            crate::jobs::for_comfort(&content, stock, &people.comfort_met, roster)
        };
        let Some(trade) = want else { continue };
        // The towns one road away or two, each by its quickest way (in the roads' order, so a tie keeps to it).
        let next = |at: Entity| roads.iter().filter_map(move |r| {
            if r.from == at { Some((r.to, r.travel_ticks)) } else if r.to == at { Some((r.from, r.travel_ticks)) } else { None }
        });
        let mut near: Vec<(Entity, u64)> = Vec::new();
        for (one, a) in next(e) {
            for (other, ticks) in std::iter::once((one, a)).chain(next(one).map(|(two, b)| (two, a + b))) {
                if other == e {
                    continue;
                }
                match near.iter_mut().find(|(t, _)| *t == other) {
                    Some(seen) => seen.1 = seen.1.min(ticks),
                    None => near.push((other, ticks)),
                }
            }
        }
        // The nearest of them with a hand to spare: (ticks, a spare of the trade 0 or an idle hand 1, town, hand).
        let mut best: Option<(u64, u8, Entity, usize)> = None;
        for &(other, ticks) in &near {
            let Ok((far, _, _, _, Some(roster), _)) = towns.get(other) else { continue };
            let at_home: Vec<usize> = roster.hands.iter().enumerate()
                .filter(|&(i, h)| h.trade == trade && !h.away && !h.is_baby() && Some(i) != roster.merchant).map(|(i, _)| i).collect();
            let spare = (at_home.len() >= 2)
                .then(|| *at_home.iter().min_by_key(|&&i| (roster.hands[i].level, std::cmp::Reverse(i))).unwrap());
            // An idle adult with no trade before a Nitwit, as `jobs::employ` takes them.
            let left_idle = hiring.left_idle.get(&far.id).is_some_and(|&d| d + 1 >= today);
            let idle = roster.idle(&content);
            let idle = left_idle.then(|| idle.iter().copied().find(|&i| content.trades[roster.hands[i].trade].none)
                .or(idle.first().copied())).flatten();
            let Some(found) = spare.map(|i| (0, i)).or(idle.map(|i| (1, i))) else { continue };
            if best.is_none_or(|(t, k, ..)| (ticks, found.0) < (t, k)) {
                best = Some((ticks, found.0, other, found.1));
            }
        }
        let Some((ticks, kind, donor, i)) = best else { continue };
        let Ok([(town, mut money, _, _, Some(mut roster), _), (from, _, mut from_people, _, Some(mut from_roster), mut from_lockers)]) =
            towns.get_many_mut([e, donor]) else { continue };
        // Its locker goes with it, as a settler's does (`rules/settlers.rs`).
        if let Some(l) = from_lockers.as_deref_mut() {
            l.take_hand(i);
        }
        let mut hand = crate::settlers::take_hand(&mut from_roster, i);
        from_people.villagers = from_roster.residents() as f64;
        if kind == 1 {
            hand = crate::roster::Hand { id: hand.id, ..crate::roster::Hand::new(&content, trade, 1) };
        }
        post(&mut ledger, &mut book, Side::treasury(town.id, &mut money.0), Side::outside(), stall,
            Reason::Other("relocation".into()), now, content.day_ticks);
        roster.comfort_retrained = Some(now);
        hiring.movers.push(crate::jobs::Mover { hand, from: from.id, to: town.id, arrives: now + ticks.max(1) });
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
/// The most hands a party of settlers takes (`rules/settlers.rs`: a Merchant, and a settler to found a town).
const PARTY_MOST: usize = 2;

pub fn review(clock: Res<Clock>, content: Res<Content>, bills: Res<Bills>, signals: Res<Signals>, order: Res<Towns>,
        settlers: Res<Settlers>, hands: Res<crate::roster::Hands>, mut outbox: ResMut<Outbox>, mut hiring: ResMut<Hiring>, mut ledger: ResMut<Ledger>, mut book: ResMut<CashBook>, seed: Res<Rng>,
        mut dice: ResMut<Dice>, mut recipes: Local<Option<Vec<crate::services::Recipe>>>,
        (routes, map, territory, security): (Query<(&Route, &RouteState)>, Res<crate::map::Map>, Res<crate::territory::Territory>,
            Res<crate::security::Security>),
        mut towns: Query<(&Town, &mut Money, &mut People, &Stock, &Trades, &Farm, &mut Deposits, &mut Budget,
            Option<&mut Roster>, &mut Lockers, Option<&crate::herds::Herds>, Option<&crate::works::Works>)>) {
    let (now, today) = (clock.now, clock.now / content.day_ticks.max(1));
    // What each town's villagers buy a day at its managed lockers: its neighbours make it for them.
    let demand: std::collections::BTreeMap<usize, std::collections::BTreeMap<crate::content::GoodId, f64>> = towns.iter()
        .filter_map(|(town, .., roster, lockers, _, _)| Some((town.id, crate::lockers::census_demand(roster?, lockers))))
        .collect();
    if content.is_changed() {
        *recipes = None;
    }
    let recipes = recipes.get_or_insert_with(|| crate::services::recipes(&content));
    // Each town's roads that stayed home for want of a squad, at what their wagons could take: a caravan's lead in its
    // job draw, as likely as that's worth (John: "crew as a job choice"). (In the routes' order: a sum's last bit
    // mustn't hang on archetypes.)
    let mut stuck: Vec<&Route> = routes.iter().filter(|(_, st)| st.stayed == Some(crate::road::Stay::NoSquad))
        .map(|(r, _)| r).collect();
    stuck.sort_by_key(|r| r.seq);
    let mut unmanned: std::collections::BTreeMap<usize, f64> = std::collections::BTreeMap::new();
    for r in stuck {
        let (Ok(home), Ok(far)) = (towns.get(r.from), towns.get(r.to)) else { continue };
        *unmanned.entry(r.from_id).or_default() += crate::squad::wanted_worth(&content, &home.3 .0, far.4,
            far.8.map(|r| (&*far.9, &*r)));
    }
    for &e in order.0.iter() {
        let Ok((town, mut money, mut people, stock, trades, farm, mut deposits, mut budget, roster, mut lockers, herds, works)) =
            towns.get_mut(e) else { continue };
        let Some(mut roster) = roster else { continue };
        if hiring.reviewed.get(&town.id) == Some(&today) {
            continue;
        }
        hiring.reviewed.insert(town.id, today);
        let sent_today = settlers.last_sent.get(&town.id).is_some_and(|&t| t / content.day_ticks.max(1) == today);
        // A town raising settlers keeps the hands a party takes (a Merchant and a settler) and hires the rest: seed
        // 8675309's Meadowfell, with no clear ground in reach, held eight idle from day 48 for a party that never went.
        let waiting = roster.idle(&content).len();
        if sent_today || waiting <= PARTY_MOST && raising(&content, &settlers, town.id, &*people, stock, now) {
            continue;
        }
        // A road counts while it carries: a wagon on it, or one gone down it within `slack_days`. Roads that have
        // carried nothing that long are no reason to keep a trade that does nothing (John: "fix that": seed
        // 8675309's Meadowfell, its roads unfunded, kept two Leatherworkers with no leather to buy and earned nothing
        // in 90 days).
        let roads = routes.iter()
            .any(|(r, st)| (r.from_id == town.id || r.to_id == town.id) && crate::jobs::carries(&content, st, now));
        let routed = routes.iter().any(|(r, _)| r.from_id == town.id || r.to_id == town.id);
        let mut far: Vec<usize> = routes.iter().map(|(r, _)| r)
            .filter_map(|r| if r.from_id == town.id { Some(r.to_id) } else if r.to_id == town.id { Some(r.from_id) } else { None })
            .collect();
        far.sort_unstable();
        far.dedup();
        let mut far = crate::jobs::far_marbles(&content, recipes, &roster, far.iter().filter_map(|t| demand.get(t)));
        if let (Some(&worth), Some(merchant)) = (unmanned.get(&town.id), content.trade(crate::roster::MERCHANT)) {
            far.push((merchant, worth));
        }
        // Its fishing grounds unfished, a Fisherman's, as untended plots are a Farmer's.
        if let Some(marbles) = crate::jobs::fishing_marbles(&content, works, territory.holding(town.id), &map, &roster) {
            far.push(marbles);
        }
        // Its woods unfelled while its trades want wood, a Lumberjack's.
        if let Some(marbles) = crate::jobs::woodland_marbles(&content, territory.holding(town.id), town.hex, &map, &roster, stock,
                people.timber_wait + works.map_or(0.0, |w| w.timber_wait)) {
            far.push(marbles);
        }
        // Its watch short of its kit, a smith's or a Fletcher's (`security.rs`).
        let held = territory.holding(town.id);
        far.extend(crate::jobs::security_marbles(&content, &roster, security.watchmen(town.id),
            held.map_or(0.0, |h| h.armed), held.map_or(0.0, |h| h.armoured)));
        let people = &mut *people;
        let timber_wait = people.timber_wait + works.map_or(0.0, |w| w.timber_wait);
        let mut place = Place { town: town.id, money: &mut money.0, purse: &mut people.purse,
            deposits: &mut deposits, budget: &mut budget, stock, trades, farm, met: &people.comfort_met,
            roster: &mut roster, lockers: &mut lockers, roads, routed, far, herd: herds.map_or(&[][..], |h| &h.count[..]),
            timber_wait };
        let mut books = Books { ledger: &mut ledger, book: &mut book, rng: dice.of(town.id, &seed) };
        let idle = place.roster.hands.iter().any(|h| !h.is_baby() && !h.away && content.trades.get(h.trade).is_some_and(|t| t.none));
        match employ(&content, &mut books, &bills, &signals, now, &mut place) {
            None if idle => {
                hiring.left_idle.insert(town.id, today);
            }
            // A stall paid for, for the host to put up (a caravan's lead has none: `employ`).
            Some(hired) if !content.trades[hired.trade].merchant => {
                let id = place.roster.hands[hired.hand].id;
                outbox.0.push(crate::messages::Output::Stall { town: crate::wire::no(town.id), hand: id,
                    tag: hands.tag(id).unwrap_or_default().to_string(), trade: content.trades[hired.trade].name.clone() });
            }
            _ => {}
        }
    }
}
