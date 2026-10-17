//! Kit's steps (`kit.rs`; `territory/Territory.java` `touch`, `digMine`; `service/MasterServices.java` `deep_mine`):
//!
//! - **Review:** a town with a Toolsmith and no mine digs one when it has `mine_emeralds` beyond its reserve (after the
//!   count, as `digMine` runs in `afterCensus`).
//! - **Work,** once a day, after the herds are tended and before the workshop (the mod's order in `touch`): hands
//!   without a tool are given one; the Toolsmiths bring up ore; the idle hands gather what their hexes' land gives
//!   (the militia excepted) and the Fletchers hunt; the kit wears out; and what was gathered is brought in as whole
//!   items while the warehouse has room.
//! - **Work,** after the home services: a Master Toolsmith with a Mason sinks the mine deep; then the kit shares
//!   are measured from the warehouse (the farm's hands, the hunting share, the guards armed and armoured), for the
//!   next day's work and the next count's plan; both before the town's Merchant buys of its lockers and trades there.
//!
//! Gaps: the deep mine is spent for after every home service, not in the services' registration order as the mod's
//! `enterprise:deep_mine`; what was gathered, hunted and mined is owed in `Kits.pending`, apart from the farm's (the
//! mod's one `holding.pending`), so `Labour.room` doesn't count crops owed of the same good and gathered food skips
//! `Territory.storeFor` (granary, root cellar).

use bevy_ecs::prelude::*;

use super::Step;
use crate::cargo::Shipments;
use crate::cash_book::{post, CashBook, Reason, Side};
use crate::content::Content;
use crate::farms::Farm;
use crate::kit::*;
use crate::map::{Land, Map};
use crate::realms::{OffMap, Realm};
use crate::roster::Roster;
use crate::state::*;
use crate::territory::Territory;
use crate::works::{Kind, Works};

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems((
        // (after the stalls' review and before sourcing's: Bevy's free choice pinned as it ran)
        dig.in_set(Step::Review).after(super::territory::review).after(super::jobs::review)
            .before(super::sourcing::review),
        // (before the earmarks: Bevy's free choice pinned as it ran)
        work.in_set(Step::Work).after(crate::herds::tend_towns).before(super::cargo::earmark).before(super::workshop::work),
        // The mine sunk and the shares measured in the town's touch, before its Merchant buys of its lockers, teaches
        // and trades there (`LockerTrading.trade` touches the territory before its trades).
        deep_mine.in_set(Step::Work).after(super::services::home).before(super::sourcing::procure_lockers)
            .before(super::training::lessons).before(super::lockers::restock),
        measure.in_set(Step::Work).after(deep_mine).before(super::sourcing::procure_lockers)
            .before(super::training::lessons).before(super::lockers::restock),
    ));
}

/// A town with a Toolsmith and no mine digs one (`Territory.digMine`) for `mine_emeralds` beyond its reserve.
#[allow(clippy::type_complexity)]
pub fn dig(clock: Res<Clock>, content: Res<Content>, order: Res<Towns>, mut territory: ResMut<Territory>,
        mut ledger: ResMut<Ledger>, mut book: ResMut<CashBook>,
        mut towns: Query<(&Town, &mut Money, &Trades, Option<&Roster>, Option<&Farm>, Has<OffMap>, Has<Realm>)>) {
    let (now, day) = (clock.now, content.day_ticks);
    let Some(toolsmith) = content.trade("toolsmith") else { return };
    for &e in &order.0 {
        let Ok((town, mut money, trades, roster, farm, off_map, realm)) = towns.get_mut(e) else { continue };
        let Some(roster) = roster else { continue };
        if off_map || realm || !roster.hands.iter().any(|h| h.trade == toolsmith && !h.is_baby()) {
            continue;
        }
        let Some(holding) = territory.holdings.get_mut(&town.id).filter(|h| h.mine.is_none()) else { continue };
        let cost = content.dials.mine_emeralds;
        if money.0 - super::farms::reserve(&content, trades, farm) < cost {
            continue;
        }
        post(&mut ledger, &mut book, Side::treasury(town.id, &mut money.0), Side::outside(), cost,
            Reason::Other("a mine".into()), now, day);
        holding.mine = Some(Mine::default());
    }
}

/// Adds `amount` of a good (by name, if the host has it) to what's owed the warehouse.
fn owe(content: &Content, kits: &mut Kits, name: &str, amount: f64) {
    if let (Some(g), true) = (content.good(name), amount > 0.0) {
        *kits.pending.entry(g).or_insert(0.0) += amount;
    }
}

/// A day's work, for the days since the last (`Territory.touch`): see the module.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn work(clock: Res<Clock>, content: Res<Content>, map: Res<Map>, territory: Res<Territory>, order: Res<Towns>,
        mut ledger: ResMut<Ledger>,
        mut towns: Query<(&Town, &mut Kits, &mut Stock, &Trades, Option<&Roster>, Option<&Works>, Option<&Shipments>)>) {
    let (now, day) = (clock.now, content.day_ticks);
    let d = &content.dials;
    for &e in &order.0 {
        let Ok((town, mut kits, mut stock, trades, roster, works, shipments)) = towns.get_mut(e) else { continue };
        if now < kits.last + day {
            continue;
        }
        let days = ((now - kits.last) as f64 / day as f64).min(MAX_DAYS);
        kits.last = now;
        let Some(roster) = roster else { continue };
        let (kits, stock) = (&mut *kits, &mut stock.0);
        let crates = shipments.map_or(0, |s| s.crates(&content));
        let holding = territory.holdings.get(&town.id);
        let smithy = works.is_some_and(|w| w.built(Kind::Smithy));
        kits.fit(&content, stock);
        // Hands working the land without a tool are given one, an axe first (`ColdStore.equipHands`).
        let idle = roster.idle(&content);
        let lacking = (idle.len() as f64 - kits.hand_tools).floor();
        if lacking > 0.0 {
            let mut given = 0.0;
            for name in HAND_TOOLS {
                let Some(g) = content.good(name) else { continue };
                let take = stock[g].floor().min(lacking - given);
                if take > 0.0 {
                    stock[g] -= take;
                    ledger.used[g] += take;
                    given += take;
                }
            }
            kits.hand_tools += given;
        }
        // The Toolsmiths' ore: so much a day a level each, half again with pickaxes and with a smithy.
        if holding.is_some_and(|h| h.mine.is_some()) {
            let levels = levels(roster, content.trade("toolsmith"))
                * (1.0 + d.tool_bonus * kits.share_of("iron_pickaxe")) * if smithy { SMITHY_ORE } else { 1.0 };
            owe(&content, kits, "raw_iron", levels * d.raw_iron_per_level_day * days);
            owe(&content, kits, "coal", levels * d.coal_per_level_day * days);
            owe(&content, kits, "raw_gold", levels * d.raw_gold_per_level_day * days);
            if holding.is_some_and(|h| h.mine.is_some_and(|m| m.deep)) {
                owe(&content, kits, "lapis_lazuli", levels * d.lapis_per_level_day * days);
            }
        }
        // Its hands gather, more with tools to hand, which wear out; the militia stand watch instead.
        let hands: Vec<f64> = idle.iter()
            .map(|&i| if content.trades[roster.hands[i].trade].nitwit { NITWIT_FACTOR } else { 1.0 }).collect();
        let tooled = (hands.len() as f64).min(kits.hand_tools);
        let with_tools = if hands.is_empty() { 0.0 } else { tooled / hands.len() as f64 };
        if let Some(h) = holding.filter(|h| !h.hexes.is_empty()) {
            let militia = (h.militia as usize).min(h.posts.len());
            // Each hand works the next of the town's hexes by q, then r (`Territory.hexes`), not as they were claimed.
            let mut hexes = h.hexes.clone();
            hexes.sort_by_key(|x| (x.q, x.r));
            for (i, weight) in hands.iter().enumerate().take(hands.len().saturating_sub(militia)) {
                let land = map.land(hexes[i % hexes.len()]);
                for &(name, per_day) in hauls(land) {
                    let Some(g) = content.good(name) else { continue };
                    let owed = kits.pending.get(&g).copied().unwrap_or(0.0);
                    if room(&content, trades, stock, crates, g, owed) {
                        owe(&content, kits, name, per_day * weight * (1.0 + d.tool_bonus * with_tools) * days);
                    }
                }
            }
        }
        kits.hand_tools = (kits.hand_tools - tooled * days / d.hand_tool_days).max(0.0);
        // Its Fletchers hunt: so much game a day a level each, a quarter of it without bows, twice with a Master.
        let fletcher = content.trade("fletcher");
        let hunting = holding.map_or(0.0, |h| h.hunting);
        let master = at_home(roster, fletcher).any(|h| h.level >= MASTER_LEVEL);
        let hunters = levels(roster, fletcher) * (SNARES + (1.0 - SNARES) * hunting) * if master { HUNTING_PARTY } else { 1.0 };
        if hunters > 0.0 {
            let land = town.hex.map_or(Land::Plains, |hex| map.land(hex));
            for &(name, per_day) in game(land) {
                let Some(g) = content.good(name) else { continue };
                let owed = kits.pending.get(&g).copied().unwrap_or(0.0);
                if room(&content, trades, stock, crates, g, owed) {
                    owe(&content, kits, name, per_day * hunters * days);
                }
            }
        }
        // Its kit wears out.
        let how = Wear {
            tools: if kits.mended_until.is_some_and(|u| u >= now) { MENDED_WEAR } else { 1.0 },
            kit: if smithy { SMITHY_WEAR } else { 1.0 },
            guards: if kits.armoury_until.is_some_and(|u| u >= now) { ARMOURY_WEAR } else { 1.0 },
        };
        let crew = crew(&content, &territory, town.id, Some(roster));
        for (g, n) in kits.wear(&content, stock, days, crew, how) {
            ledger.used[g] += n as f64;
        }
        // What was gathered is brought in, whole items, while the warehouse has room.
        let goods: Vec<usize> = kits.pending.keys().copied().collect();
        for g in goods {
            let mut owed = kits.pending[&g];
            ledger.made[g] += bring_in(&content, stock, crates, g, &mut owed);
            kits.pending.insert(g, owed);
        }
        kits.pending.retain(|_, owed| *owed > 0.0);
    }
}

/// A Master Toolsmith with a Mason of 3 sinks the mine deep for `deep_mine_emeralds` beyond the reserve
/// (`enterprise:deep_mine`, `MasterServices.investWanted`): lapis too from then on.
#[allow(clippy::type_complexity)]
pub fn deep_mine(clock: Res<Clock>, content: Res<Content>, order: Res<Towns>, mut territory: ResMut<Territory>,
        mut ledger: ResMut<Ledger>, mut book: ResMut<CashBook>,
        mut towns: Query<(&Town, &mut Money, &Trades, Option<&Roster>, Option<&Farm>, Has<OffMap>, Has<Realm>)>) {
    if !content.dials.services {
        return;
    }
    let (now, day) = (clock.now, content.day_ticks);
    let (Some(toolsmith), Some(mason)) = (content.trade("toolsmith"), content.trade("mason")) else { return };
    for &e in &order.0 {
        let Ok((town, mut money, trades, roster, farm, off_map, realm)) = towns.get_mut(e) else { continue };
        let Some(roster) = roster else { continue };
        if off_map || realm {
            continue;
        }
        let Some(mine) = territory.holdings.get_mut(&town.id).and_then(|h| h.mine.as_mut()).filter(|m| !m.deep) else {
            continue;
        };
        let skills = crate::services::residents(Some(roster));
        if skills.get(&toolsmith).copied().unwrap_or(0) < MASTER_LEVEL || skills.get(&mason).copied().unwrap_or(0) < MASON_LEVEL {
            continue;
        }
        let cost = content.dials.deep_mine_emeralds;
        if money.0 - super::farms::reserve(&content, trades, farm) < cost {
            continue;
        }
        post(&mut ledger, &mut book, Side::treasury(town.id, &mut money.0), Side::outside(), cost,
            Reason::Other("a deep mine".into()), now, day);
        mine.deep = true;
    }
}

/// The share of each kind's wearers the kit in the warehouse covers, and what follows from it: the farm's hands, the
/// hunting share, the guards armed and armoured (`Territory.touch`'s end).
#[allow(clippy::type_complexity)]
pub fn measure(content: Res<Content>, order: Res<Towns>, mut territory: ResMut<Territory>,
        mut towns: Query<(&Town, &mut Kits, &Stock, Option<&Roster>, Option<&Works>, Option<&mut Farm>)>) {
    let guards_kit = ["iron_sword", "bow", "iron_chestplate"].iter().any(|n| content.good(n).is_some());
    for &e in &order.0 {
        let Ok((town, mut kits, stock, roster, works, farm)) = towns.get_mut(e) else { continue };
        kits.fit(&content, &stock.0);
        let crew = crew(&content, &territory, town.id, roster);
        kits.measure(&content, &stock.0, crew);
        if let Some(mut farm) = farm {
            let hands = 1.0 + content.dials.tool_bonus * kits.share_of("iron_hoe");
            if farm.hands != hands {
                farm.hands = hands;
            }
        }
        let held = |name: &str| content.good(name).map_or(0, |g| (stock.0[g] + 1e-9).floor().max(0.0) as usize);
        let Some(h) = territory.holdings.get_mut(&town.id) else { continue };
        h.hunting = hunting(held("bow"), crew.hunters);
        if guards_kit {
            let tower = works.is_some_and(|w| w.built(Kind::Watchtower));
            h.armed = armed(held("iron_sword"), held("bow"), crew.guards, crew.hunters, if tower { WATCHTOWER_ARMED } else { 0.0 });
            h.armoured = kits.share_of("iron_chestplate");
        }
    }
}
