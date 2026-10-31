//! Review: a town grown past growing raises a party of settlers (`caravan/Expeditions.java` `consider`, 130; see
//! `settlers.rs`). Once a day, towns in their order: one that is raising, with idle hands, money to spare and
//! somewhere to go, sends one hand as a travelling Merchant to the nearest village nobody trades with (4 to 8 hexes
//! out), or two (a Merchant and a settler) to found a town on the best open ground there; out from home, or from a town
//! its caravans go to, where the party resupplies. The set-up and the capital
//! leave the treasury, the households lending what they would on call first (the Accountant approves the rest), the
//! bread leaves the warehouse, the hands leave the roster.

use bevy_ecs::prelude::*;

use super::Step;
use crate::bills::Bills;
use crate::cash_book::{post, Accountant, CashBook, Side};
use crate::content::Content;
use crate::fixed::Fixed;
use crate::hex::Hex;
use crate::map::Map;
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

/// What a hex's land gives a day, in emeralds (its worth to a town that holds it): a hand's gathering there
/// (`kit::hauls`), a Toolsmith's ore of a level on the hills, a Fisherman's cod on the water.
fn land_worth(content: &Content, land: crate::map::Land) -> f64 {
    let value = |name: &str| content.good(name).map_or(0.0, |g| content.goods[g].value);
    let d = &content.dials;
    let mut worth: f64 = crate::kit::hauls(land).iter().map(|&(name, n)| n * value(name)).sum();
    match land {
        crate::map::Land::Hills => worth += d.raw_iron_per_level_day * value("raw_iron") + d.coal_per_level_day * value("coal")
            + d.raw_gold_per_level_day * value("raw_gold"),
        crate::map::Land::Water => worth += crate::works::COD_PER_WATER_HEX * value("cod"),
        _ => {}
    }
    worth
}

/// What settling round `site` would add to the town's portfolio (John: "seaside towns start founding inland colonies
/// to gain a complementary portfolio of things to trade"): the worth of the land of the site's hex and those round it,
/// a land the town already has (its holding, its own hex and those round it) a quarter as much.
fn complement(content: &Content, map: &Map, has: &[crate::map::Land], site: Hex) -> f64 {
    std::iter::once(site).chain(site.neighbors()).map(|h| {
        let land = map.land(h);
        land_worth(content, land) * if has.contains(&land) { COMPLEMENT_HAD } else { 1.0 }
    }).sum()
}

/// What a land the town already has counts toward a site's complement.
const COMPLEMENT_HAD: f64 = 0.25;

/// The village in reach that would add most to the town's portfolio (`complement`; the nearest on a tie), still
/// nobody's market, with no other town within `min_hexes` hexes and no party already bound there: its town, hex and
/// complement.
fn unclaimed_village(sites: &[Site], settlers: &Settlers, home: Hex, min: i32, max: i32,
        worth: impl Fn(Hex) -> f64) -> Option<(usize, Hex, f64)> {
    sites.iter()
        .filter(|s| s.village && s.hex.distance(home) <= max)
        .filter(|s| !settlers.parties.iter().any(|p| p.village == Some(s.id)))
        .filter(|s| clear(sites, settlers, min, s.hex, Some(s.id)))
        .map(|s| (s.id, s.hex, worth(s.hex)))
        .max_by(|a, b| a.2.total_cmp(&b.2).then((b.1.distance(home), b.0).cmp(&(a.1.distance(home), a.0))))
}

/// The best open ground in reach (`openGround`): along the six directions between `min` and `max` hexes, clear,
/// unclaimed, dry and flat enough; the one that would add most to the town's portfolio (`complement`), then the
/// flattest, driest and nearest. Its hex and complement.
fn open_ground(sites: &[Site], settlers: &Settlers, territory: &Territory, map: &Map, home: Hex, min: i32, max: i32,
        worth: impl Fn(Hex) -> f64) -> Option<(Hex, f64)> {
    let mut best: Option<((f64, f64), Hex)> = None;
    // Every hex of the rings between (seed 1717: six spokes' two hexes each left a town with savings and hands no ground),
    // the spokes first, each way in turn, so a tie goes as it did.
    let spokes = crate::hex::DIRECTIONS.iter().flat_map(|&(dq, dr)| (min + 1..max).map(move |d| (d * dq, d * dr)));
    let between = (-max..=max).flat_map(|dq| (-max..=max).map(move |dr| (dq, dr)))
        .filter(|&(dq, dr)| dq != 0 && dr != 0 && dq != -dr);
    for (dq, dr) in spokes.chain(between) {
        let hex = Hex::new(home.q + dq, home.r + dr);
        let d = home.distance(hex);
        if d <= min || d >= max || !clear(sites, settlers, min, hex, None) || territory.owner(hex).is_some() {
            continue;
        }
        let ground = map.ground(hex);
        let (wet, relief) = (ground.water_share, ground.relief);
        if wet > OPEN_WET || relief >= ROUGH {
            continue;
        }
        let score = (worth(hex), -relief - 20.0 * wet + if wet > 0.02 { 4.0 } else { 0.0 } - 0.5 * d as f64);
        if best.is_none_or(|(b, _)| score.0 > b.0 || score.0 == b.0 && score.1 > b.1) {
            best = Some((score, hex));
        }
    }
    best.map(|((w, _), h)| (h, w))
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn review(clock: Res<Clock>, content: Res<Content>, bills: Res<Bills>, mut map: ResMut<Map>, territory: Res<Territory>,
        mut outbox: ResMut<Outbox>,
        order: Res<Towns>,
        mut settlers: ResMut<Settlers>, mut ledger: ResMut<Ledger>, mut book: ResMut<CashBook>, routes: Query<&Route>,
        mut towns: Query<(&Town, &mut Money, &mut Stock, &mut People, &Trades, &mut Budget, Option<&mut Roster>,
            Option<&mut crate::lockers::Lockers>, Option<&mut crate::farms::Deposits>)>) {
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
        .filter_map(|(town, _, _, _, _, _, roster, _, _)| {
            let village = roster.is_some_and(|r| r.merchant.is_none()) && !roads.contains(&town.id);
            town.hex.map(|hex| Site { id: town.id, hex, village,
                structure: village || settlers.villages.contains(&town.id) })
        })
        .collect();
    let merchant = content.trade(MERCHANT);
    let farmer = content.trade("farmer");
    // The markets the world has, and those its parties will make: at `max_towns`, no party sets out (John, 2026-10-31:
    // "close to a town limit for a server that aims to keep resource footprint low"; seed 1717's 37 towns at day 240
    // ran a server out of a 6 GB heap).
    let markets = towns.iter().filter(|(_, _, _, _, _, _, roster, _, _)| roster.is_some_and(|r| r.merchant.is_some()))
        .count();
    for &e in order.0.iter() {
        if markets + settlers.parties.len() >= d.max_towns as usize {
            break;
        }
        let Ok((town, mut money, mut stock, mut people, trades, mut budget, roster, mut lockers, deposits)) = towns.get_mut(e)
            else { continue };
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
        let free = crate::rules::free(&content, trades, &money, &bills, t, now);
        // The households' savings stand behind a party as behind a stall: what they'd lend on call (`jobs.rs`).
        let savings = deposits.as_ref().map_or(Fixed::ZERO, |d| crate::farms::on_call(&content, people.purse, d.owed));
        let spare = free + savings;
        // The hands it can spare: those waiting for work, else workers it has others of (`party_hands`).
        let hands = party_hands(&content, &roster);
        if hands.is_empty() || spare < SETTLING_EMERALDS {
            continue;
        }
        // The lands the town has: its holding's, its own hex's and those round it.
        let has: Vec<crate::map::Land> = territory.holding(t).map(|h| h.hexes.clone()).unwrap_or_default()
            .into_iter().chain(std::iter::once(home)).chain(home.neighbors()).map(|h| map.land(h)).collect();
        let worth = |hex: Hex| complement(&content, &map, &has, hex);
        // Where a party may set out from: home, and each town its caravans go to, where it resupplies on the way (John,
        // 2026-10-31: "letting them use the colonies as resupply points to extend their settlement range"; seed 1717's
        // towns, their country within 10 hexes settled by day 50, raised parties with nowhere to go for 190 days). The
        // reach is counted from the stop; the best site of all of them, then the shortest trip.
        // By the far town's number: a query's order isn't a save's, and a tie goes to the first stop.
        let mut far: Vec<usize> = routes.iter().filter(|r| r.from_id == t).map(|r| r.to_id).collect();
        far.sort_unstable();
        far.dedup();
        let mut stops = vec![home];
        for to in far {
            if let Some(s) = sites.iter().find(|s| s.id == to).map(|s| s.hex).filter(|h| !stops.contains(h)) {
                stops.push(s);
            }
        }
        let trip = |stop: Hex, hex: Hex| home.distance(stop) + stop.distance(hex);
        let better = |w: f64, n: i32, best: Option<(f64, i32)>| best.is_none_or(|(bw, bn)| w > bw || w == bw && n < bn);
        // A village (its people come with it) where it adds as much as the best open ground; else that ground.
        let mut village: Option<(usize, Hex, f64, Hex)> = None;
        let mut ground: Option<(Hex, f64, Hex)> = None;
        for &stop in &stops {
            if let Some((v, hex, w)) = unclaimed_village(&sites, &settlers, stop, min, max, worth) {
                if better(w, trip(stop, hex), village.map(|(_, h, bw, bs)| (bw, trip(bs, h)))) {
                    village = Some((v, hex, w, stop));
                }
            }
            if hands.len() >= 2 && spare >= FOUNDING_EMERALDS {
                if let Some((hex, w)) = open_ground(&sites, &settlers, &territory, &map, stop, min, max, worth) {
                    if better(w, trip(stop, hex), ground.map(|(h, bw, bs)| (bw, trip(bs, h)))) {
                        ground = Some((hex, w, stop));
                    }
                }
            }
        }
        let (kind, target, target_hex, stop) = match (village, ground) {
            (Some((v, hex, w, stop)), g) if g.is_none_or(|(_, gw, _)| w >= gw) => (Kind::Village, Some(v), hex, stop),
            (_, Some((hex, _, stop))) => (Kind::Found, None, hex, stop),
            _ => continue,
        };
        // The road (`way`), by the stop: no way there, nobody goes.
        let path = if stop == home {
            way(&map, &content, home, target_hex)
        } else {
            way(&map, &content, home, stop).zip(way(&map, &content, stop, target_hex)).map(|(a, b)| crate::map::Path {
                hexes: a.hexes.iter().chain(b.hexes.iter().skip(1)).copied().collect(),
                cost: a.cost + b.cost,
            })
        };
        let Some(path) = path else { continue };
        // Its way and where it's bound, explored while it travels (`Output::Explore`): ground the host never loaded is
        // the core's guess, and a party sent to found a town on it finds it as it is (seed 1717's Lakeshire, founded on
        // guessed ground the host found no open ground on, asked of the host 1,400 times).
        let new: Vec<crate::wire::HexAt> = path.hexes.iter().filter(|&&h| map.explored.insert(h))
            .map(|&h| crate::wire::HexAt::of(h)).collect();
        if !new.is_empty() {
            outbox.0.push(crate::wire::Output::Explore { hexes: new });
        }
        let jumpstart = if kind == Kind::Village { d.settling_capital } else { d.founding_capital };
        let set_up = set_up(kind);
        let carried = capital(kind, jumpstart, spare - set_up);
        // The households put in first; the treasury's share is the Accountant's to approve.
        let lent = savings.min(set_up + carried);
        let what = crate::cash_book::Reason::Other("settlers".into());
        if !Accountant::approve(&content, &mut book, t, now, free, &mut budget, what, set_up + carried - lent) {
            continue;
        }
        if let Some(mut deposits) = deposits {
            crate::farms::borrow(&content, &mut ledger, &mut book, t, &mut money.0, &mut people.purse, &mut deposits, lent,
                now);
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
