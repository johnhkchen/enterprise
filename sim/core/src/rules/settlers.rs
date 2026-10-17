//! Review: a town grown past growing raises a party of settlers (`caravan/Expeditions.java` `consider`, 130; see
//! `settlers.rs`). Once a day, towns in their order: one that is raising, with idle hands, money to spare and
//! somewhere to go, sends one hand as a travelling Merchant to the nearest village nobody trades with (4 to 8 hexes
//! out), or two (a Merchant and a settler) to found a town on the best open ground there. The set-up and the capital
//! leave the treasury (the Accountant must approve them), the bread leaves the warehouse, the hands leave the roster.

use bevy_ecs::prelude::*;

use super::Step;
use crate::bills::Bills;
use crate::cash_book::{post, Accountant, CashBook, Side};
use crate::content::Content;
use crate::hex::Hex;
use crate::map::{Map, RELIEF_PER_FACTOR};
use crate::roster::{Roster, MERCHANT};
use crate::settlers::*;
use crate::state::*;
use crate::territory::Territory;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems(review.in_set(Step::Review));
}

/// A town's place for the review: where it stands, whether it is a village nobody trades with, and whether it stands
/// on a village (one now, or one settlers made a market of: the mod's village structure stays).
struct Site {
    id: usize,
    hex: Hex,
    village: bool,
    structure: bool,
}

/// No other town (a market, a settlement or a village) and no party's goal within `min_hexes` hexes of this one
/// (`Expeditions.clear`); `except` is the village itself.
fn clear(sites: &[Site], settlers: &Settlers, min_hexes: i32, hex: Hex, except: Option<usize>) -> bool {
    sites.iter().all(|s| Some(s.id) == except || s.hex.distance(hex) >= min_hexes)
        && settlers.parties.iter().all(|p| p.target.distance(hex) >= min_hexes)
}

/// The nearest village, if it is in reach, still nobody's market, with no other town within `min_hexes` hexes and no
/// party already bound there (`unclaimedVillage`): its town and hex. Only the nearest, as the mod's
/// `findNearestMapStructure`: once a town's settlers have made a market of it, the town founds towns instead.
fn unclaimed_village(sites: &[Site], settlers: &Settlers, home: Hex, min: i32, max: i32) -> Option<(usize, Hex)> {
    sites.iter()
        .filter(|s| s.structure)
        .min_by_key(|s| (s.hex.distance(home), s.id))
        .filter(|s| s.village && s.hex.distance(home) <= max)
        .filter(|s| !settlers.parties.iter().any(|p| p.village == Some(s.id)))
        .filter(|s| clear(sites, settlers, min, s.hex, Some(s.id)))
        .map(|s| (s.id, s.hex))
}

/// The best open ground 4 to 8 hexes out (`openGround`): along the six directions at 5 and 7 hexes, clear, unclaimed,
/// dry and flat enough, the flattest, driest and nearest.
fn open_ground(sites: &[Site], settlers: &Settlers, territory: &Territory, map: &Map, home: Hex, min: i32, max: i32)
        -> Option<Hex> {
    let mut best: Option<(f64, Hex)> = None;
    for (dq, dr) in crate::hex::DIRECTIONS {
        for d in (min + 1..=max - 1).step_by(2) {
            let hex = Hex::new(home.q + d * dq, home.r + d * dr);
            if !clear(sites, settlers, min, hex, None) || territory.owner(hex).is_some() {
                continue;
            }
            let ground = map.ground(hex);
            let (wet, relief) = (ground.water_share, ground.relief);
            if wet > 0.3 || relief >= RELIEF_PER_FACTOR {
                continue;
            }
            let score = -relief - 20.0 * wet + if wet > 0.02 { 4.0 } else { 0.0 } - 0.5 * d as f64;
            if best.is_none_or(|(b, _)| score > b) {
                best = Some((score, hex));
            }
        }
    }
    best.map(|(_, h)| h)
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn review(clock: Res<Clock>, content: Res<Content>, bills: Res<Bills>, map: Res<Map>, territory: Res<Territory>,
        order: Res<Towns>,
        mut settlers: ResMut<Settlers>, mut ledger: ResMut<Ledger>, mut book: ResMut<CashBook>, routes: Query<&Route>,
        mut towns: Query<(&Town, &mut Money, &mut Stock, &mut People, &Trades, &mut Budget, Option<&mut Roster>,
            Option<&mut crate::lockers::Lockers>)>) {
    let (now, day) = (clock.now, content.day_ticks);
    let today = now / day;
    let forced = settlers.force;
    if forced.is_none() {
        if settlers.reviewed == Some(today) {
            return;
        }
        settlers.reviewed = Some(today);
    }
    let d = &content.dials;
    let (min, max) = (d.min_hexes as i32, d.max_hexes as i32);
    let roads: std::collections::BTreeSet<usize> = routes.iter().flat_map(|r| [r.from_id, r.to_id]).collect();
    let sites: Vec<Site> = towns.iter()
        .filter_map(|(town, _, _, _, _, _, roster, _)| {
            let village = roster.is_some_and(|r| r.merchant.is_none()) && !roads.contains(&town.id);
            town.hex.map(|hex| Site { id: town.id, hex, village,
                structure: village || settlers.villages.contains(&town.id) })
        })
        .collect();
    let merchant = content.trade(MERCHANT);
    let farmer = content.trade("farmer");
    for &e in order.0.iter() {
        let Ok((town, mut money, mut stock, mut people, trades, mut budget, roster, mut lockers)) = towns.get_mut(e) else { continue };
        let t = town.id;
        if forced.is_some_and(|f| f != t) {
            continue;
        }
        let (Some(home), Some(mut roster), Some(merchant)) = (town.hex, roster, merchant) else { continue };
        // Only a market sends settlers (`consider` runs for a Merchant's desk): a village has no Merchant to send.
        if roster.merchant.is_none() {
            continue;
        }
        if !raising(&content, &settlers, t, &people, &stock, now) {
            continue;
        }
        let spare = crate::rules::free(&content, trades, &money, &bills, t, now);
        // The hands waiting for work: no trade at all (`Labour.NONE`); Nitwits never go.
        let hands: Vec<usize> =
            roster.idle(&content).into_iter().filter(|&i| !content.trades[roster.hands[i].trade].nitwit).collect();
        if hands.is_empty() || spare < SETTLING_EMERALDS {
            continue;
        }
        let village = unclaimed_village(&sites, &settlers, home, min, max);
        let (kind, target, target_hex) = if let Some((v, hex)) = village {
            (Kind::Village, Some(v), hex)
        } else {
            let site = if hands.len() >= 2 && spare >= FOUNDING_EMERALDS {
                open_ground(&sites, &settlers, &territory, &map, home, min, max)
            } else {
                None
            };
            let Some(hex) = site else { continue };
            (Kind::Found, None, hex)
        };
        // The road: planned over the hexes as a caravan's would be; no way there, nobody goes.
        let Some(path) = map.plan(home, target_hex, per_hex(&content), 0.0, &Default::default(), &Default::default(), false,
            &Default::default()) else { continue };
        let jumpstart = if kind == Kind::Village { d.settling_capital } else { d.founding_capital };
        let set_up = set_up(kind);
        let carried = capital(kind, jumpstart, spare - set_up);
        let what = crate::cash_book::Reason::Other("settlers".into());
        if !Accountant::approve(&content, &mut book, t, now, spare, &mut budget, what, set_up + carried) {
            continue;
        }
        // The hands: one signs on as a travelling Merchant (trained: it keeps the trade on the road); for a new
        // town the settler goes as its Farmer.
        let going = if kind == Kind::Village { 1 } else { 2 };
        let picked: Vec<usize> = hands[..going].to_vec();
        let mut high_first = picked.clone();
        high_first.sort_unstable_by(|a, b| b.cmp(a));
        let mut got: std::collections::BTreeMap<usize, crate::roster::Hand> =
            high_first.into_iter().map(|i| {
                // Its locker goes with it, so the hands after it keep theirs (`lockers.rs`).
                if let Some(l) = lockers.as_deref_mut() {
                    l.take_hand(i);
                }
                (i, take_hand(&mut roster, i))
            }).collect();
        let mut party_hands: Vec<crate::roster::Hand> = picked.iter().map(|i| got.remove(i).expect("a hand")).collect();
        party_hands[0].trade = merchant;
        party_hands[0].level = 1;
        party_hands[0].xp = 1;
        if kind == Kind::Found {
            if let Some(farmer) = farmer {
                party_hands[1].trade = farmer;
                party_hands[1].level = 1;
                party_hands[1].xp = 1;
            }
        }
        // The set-up is spent; the capital rides with the party, off the society's books until it arrives.
        let reason = reason();
        post(&mut ledger, &mut book, Side::treasury(t, &mut money.0), Side::outside(), set_up + carried, reason, now, day);
        let bread = content.good("bread").map_or(0.0, |b| {
            let n = stock.0[b].min(BREAD);
            stock.0[b] -= n;
            ledger.used[b] += n;
            n
        });
        people.villagers = roster.residents() as f64;
        let id = settlers.next_id;
        settlers.next_id += 1;
        let arrives = now + travel_ticks(path.blocks());
        settlers.parties.push(Party { id, from: t, kind, target: target_hex, village: target, hands: party_hands,
            capital: carried, bread, departs: now, arrives });
        settlers.last_sent.insert(t, now);
    }
}
