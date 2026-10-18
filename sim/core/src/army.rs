//! The army at the front, as a population: recruits muster at the capital and join at the front; the quartermaster
//! wants kit sets, arrows and rations for the round trips a delivery takes; soldiers eat 8 food points a day, the
//! heartiest food first; each day each draws what kit it lacks from the front's warehouse, the best there is, wears it
//! out, and runs a risk by its kit and its hunger; the fallen are gone from their town for good, and it grieves; the
//! hungry desert, walking home. At peace the rest walk home.
//!
//! Ported from the mod:
//! - `army/Army.java`: `DEATH_BASE`, `RATION`, `HUNGRY_RISK`, `DESERTION` (dials), `BREAD_SATURATION`,
//!   `HEARTY_STRENGTH`, `HEARTY_RISK`, `MEATS`, `MEAT_HEARTINESS`, `heartiness` (81, 87), `ARROWS_PER_DAY` (dial), the
//!   `WEAPONS` and `ARMOUR` tables and `SHIELD` (95–125), `UNARMED_RISK`, `UNARMOURED_RISK`, `FULL_SET_POINTS`,
//!   `armourRisk` (128), `mess` (218), `muster` (273), `embark` (279), `enlist` (287), `wants` (306),
//!   `RATION_SPARE_DAYS` (dial), `rationDays` (328), `foodPoints`, `heartiestFirst` (341), `risk` (351–366), `points`
//!   (367), `draw`, `campaign` (419–545), `demobilise`; the tally (`Record`, `issued`, `eaten`, `heartiness`).
//! - `market/Sovereign.java` `SOLDIER_PAY` (0.25 a soldier-day, a dial); `contract/Levy.java` (pay under a levy).
//! - `caravan/Caravans.java` `raiseLevy` (1662): who a town sends, idle hands first, then a trade it has others of,
//!   then the least skilled; never a child, nor the Merchant at its desk ([`levy_order`]). `travelTicks` (138).
//! - `territory/Territory.java` `mourn` (423) and the grief in `moodTarget` (1593): half a point of mood each fallen
//!   soldier for 10 days, at most 2 ([`grief`], hooked into `people::mood_target`).
//! - `docs/decisions.md` "War footing" (campaigning, the fallen, an army eats), "Feeding the front", "Mercenaries, meat,
//!   and the front's supply" (heartiness, any meat), "Armour comes in sets".
//!
//! **Attrition as a rate, not dice** (Lanchester's attrition against a fixed foe, `docs/core-math.md`; decision 8 of
//! `docs/core-plan.md`). The mod rolls each soldier each day: it falls if `random.nextDouble() < risk`, and a hungry
//! one deserts if a roll comes under its chance. The core counts the day's expected losses instead: the army's
//! summed risk, `dN = Σ risk`, carried as a fraction in [`Army::falling`] (and the hungry's summed chance of
//! deserting in [`Army::deserting`]), and a whole soldier goes when the carry reaches one. So the same army in the
//! same kit and on the same rations loses the same soldiers on the same days whatever the world's dice, a small
//! army's losses don't swing by a whole soldier on a lucky day, and two runs of a study differ only by what they
//! changed. Who goes is the soldier who has run the most risk (its [`Soldier::exposure`], the risk of every day it
//! served, summed: the one most likely to have fallen by now), the first enlisted of equals; who deserts is the
//! hungriest (most hungry days), the last of equals, as the mod's hungry are the last of its list.
//!
//! **Where the core differs.** Soldiers are the hands their towns sent: each stays in its town's roster, away, under
//! the id its town stamped it with (`squad::stamp`, as the mod knows a villager by its UUID), and comes off the roster
//! when it falls. The front is any town ([`Society::set_front`]; an off-map market as the mod's, `realms.rs`): the army
//! eats and draws from its warehouse. Kit is goods by name, the tables' names with or without the host's namespace;
//! a piece's enchantments are the front's [`Kits`] (`kit.rs`): its `enchant_level` counts as the enchantment suited to
//! the enemy (Sharpness or Power on a weapon, Protection on armour: the mod's `Theatre.FACTION`), as the core's kit
//! doesn't name its enchantments, so there's no theatre to choose; Unbreaking is Unbreaking. Kit drawn and food eaten
//! leave the world's goods (`Ledger.used`): the soldier's worn kit is the army's, not a warehouse's. Pay (0.25 a
//! soldier-day) is owed through the soldier's levy contract while it's in force and is its home's with the capital
//! (`Book::owe`: the capital's wagons carry it home, `rules/cargo.rs`), as the mod's; a soldier with none (the core has no levy contracts yet: the crown
//! slice drafts them) has it owed in the army's own pay ledger ([`Army::pay_owed`]), which the crown settles. Recruits
//! wait at the capital (`realms::Crown`'s) until a host or the crown's wagons send them on ([`Society::embark`]); the
//! levy's "the only one of a trade its contracts need or the crown wants trained, last" is the crown's to add. Walking
//! home takes the road's time at a wagon's pace, by the distance between the towns' hexes (none for a town not placed).

use std::collections::BTreeMap;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::content::{Content, Dials, Good, GoodId};
use crate::contracts::Book;
use crate::kit::Kits;
use crate::messages::TownId;
use crate::roster::{Hand, Roster};
use crate::society::Society;
use crate::state::{Ledger, People, Stock};

/// Plain bread's saturation a point of food: a food's heartiness is how far above it its own is (`Army.BREAD_SATURATION`).
pub const BREAD_SATURATION: f64 = 1.2;
/// A hearty army fights a quarter harder and falls 15% less (`HEARTY_STRENGTH`, `HEARTY_RISK`); it deserts half as
/// readily when short (`campaign`).
pub const HEARTY_STRENGTH: f64 = 0.25;
pub const HEARTY_RISK: f64 = 0.15;
pub const HEARTY_DESERTION: f64 = 0.5;
/// Meat of any kind is at least as hearty as a pork chop (`MEATS`, `MEAT_HEARTINESS`).
pub const MEATS: [&str; 6] = ["cooked_porkchop", "cooked_beef", "cooked_mutton", "cooked_chicken", "cooked_rabbit",
    "rabbit_stew"];
pub const MEAT_HEARTINESS: f64 = 1.0 / 3.0;
/// Unarmed, a soldier runs twice the risk; unarmoured, 1.6 times, down to half in a full set of 20 points
/// (`UNARMED_RISK`, `UNARMOURED_RISK`, `FULL_SET_POINTS`).
pub const UNARMED_RISK: f64 = 2.0;
pub const UNARMOURED_RISK: f64 = 1.6;
pub const FULL_SET_POINTS: f64 = 20.0;
/// An edge (Sharpness, Power) cuts a soldier's risk 5% a level, to no less than 0.6 of it; a guard (Protection) 6% a
/// level over all its pieces, to no less than a half (`risk`).
pub const EDGE_PER_LEVEL: f64 = 0.05;
pub const EDGE_FLOOR: f64 = 0.6;
pub const GUARD_PER_LEVEL: f64 = 0.06;
pub const GUARD_FLOOR: f64 = 0.5;
/// An archer drawing a bow, or out of arrows, takes this many days' arrows (`ARROWS_PER_DAY * 4`).
pub const ARROW_DRAW_DAYS: u32 = 4;
/// The goods the army knows by name: arrows for its bows, and bread, what the quartermaster counts its rations in.
pub const ARROW: &str = "arrow";
pub const BREAD: &str = "bread";
/// Ticks a block of road takes, as a caravan's pace (`Caravans.TICKS_PER_BLOCK`), and the least a trip takes.
pub const TICKS_PER_BLOCK: f64 = 20.0;
pub const LEAST_TRIP_TICKS: f64 = 20.0;

/// A piece of kit (`Army.Gear`): its name (the mod's item), its power (attack, or armour points), the risk factor a
/// soldier carrying it runs (armour: 1, its points tell), and the days of campaigning it lasts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gear {
    pub name: &'static str,
    pub power: u32,
    pub risk: f64,
    pub days: u32,
}

const fn gear(name: &'static str, power: u32, risk: f64, days: u32) -> Gear {
    Gear { name, power, risk, days }
}

/// Weapons, in the order a soldier draws them, best first (`Army.WEAPONS`): axes are lesser weapons, a bow last.
pub const WEAPONS: [Gear; 8] = [gear("diamond_sword", 7, 0.8, 12), gear("iron_sword", 6, 1.0, 10),
    gear("diamond_axe", 6, 0.95, 12), gear("iron_axe", 5, 1.15, 10), gear("stone_sword", 5, 1.25, 6),
    gear("stone_axe", 4, 1.35, 6), gear("wooden_sword", 4, 1.4, 4), gear("bow", 6, 1.1, 10)];
/// The bow, which shoots arrows (and leaves a hand with no shield).
pub const BOW: &str = "bow";
/// Armour, a piece for each of four places (helmet, chestplate, leggings, boots), best first, with its armour points
/// (a full diamond set is 20, iron 15, chainmail 12, leather 7) and the days it lasts (`Army.ARMOUR`).
pub const ARMOUR: [[Gear; 4]; 4] = [
    [gear("diamond_helmet", 3, 1.0, 30), gear("iron_helmet", 2, 1.0, 20), gear("chainmail_helmet", 2, 1.0, 16),
        gear("leather_helmet", 1, 1.0, 10)],
    [gear("diamond_chestplate", 8, 1.0, 30), gear("iron_chestplate", 6, 1.0, 20), gear("chainmail_chestplate", 5, 1.0, 16),
        gear("leather_chestplate", 3, 1.0, 10)],
    [gear("diamond_leggings", 6, 1.0, 30), gear("iron_leggings", 5, 1.0, 20), gear("chainmail_leggings", 4, 1.0, 16),
        gear("leather_leggings", 2, 1.0, 10)],
    [gear("diamond_boots", 3, 1.0, 30), gear("iron_boots", 2, 1.0, 20), gear("chainmail_boots", 1, 1.0, 16),
        gear("leather_boots", 1, 1.0, 10)],
];
/// A shield: no power, 0.85 of the risk, 12 days (`Army.SHIELD`).
pub const SHIELD: Gear = gear("shield", 0, 0.85, 12);

/// The vanilla foods a host may have, by item: food points (nutrition) and saturation as the item's food component
/// holds them (`Army.nutrition`, `saturation`), for [`food_good`].
pub const FOODS: [(&str, f64, f64); 26] = [("bread", 5.0, 6.0), ("cooked_beef", 8.0, 12.8), ("cooked_porkchop", 8.0, 12.8),
    ("cooked_mutton", 6.0, 9.6), ("cooked_chicken", 6.0, 7.2), ("cooked_rabbit", 5.0, 6.0), ("rabbit_stew", 10.0, 12.0),
    ("golden_carrot", 6.0, 14.4), ("golden_apple", 4.0, 9.6), ("baked_potato", 5.0, 6.0), ("carrot", 3.0, 3.6),
    ("potato", 1.0, 0.6), ("apple", 4.0, 2.4), ("beetroot", 1.0, 1.2), ("cooked_cod", 5.0, 6.0),
    ("cooked_salmon", 6.0, 9.6), ("mushroom_stew", 6.0, 7.2), ("beetroot_soup", 6.0, 7.2), ("pumpkin_pie", 8.0, 4.8),
    ("cookie", 2.0, 0.4), ("sweet_berries", 2.0, 0.4), ("melon_slice", 2.0, 1.2), ("honey_bottle", 6.0, 1.2),
    ("beef", 3.0, 1.8), ("porkchop", 3.0, 1.8), ("chicken", 2.0, 1.2)];

/// A name without the host's namespace (`minecraft:iron_sword` is `iron_sword`).
pub fn bare(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

/// A weapon by its good's name.
pub fn weapon(name: &str) -> Option<&'static Gear> {
    WEAPONS.iter().find(|g| g.name == bare(name))
}

/// A piece of armour by its good's name, with the place it's worn (0 helmet to 3 boots).
pub fn armour(name: &str) -> Option<(usize, &'static Gear)> {
    ARMOUR.iter().enumerate().find_map(|(place, kinds)| kinds.iter().find(|g| g.name == bare(name)).map(|g| (place, g)))
}

/// The host's good of this name (with or without its namespace).
pub fn good_named(content: &Content, name: &str) -> Option<GoodId> {
    content.goods.iter().position(|g| bare(&g.name) == bare(name))
}

/// A vanilla good, worth what the mod's table says (`Good::priced`), with its food points, saturation and whether
/// it's meat as the game has them (a good not in [`FOODS`] is no food).
pub fn food_good(id: &str, stack: u32) -> Good {
    let good = Good::priced(id, stack);
    match FOODS.iter().find(|(n, ..)| *n == bare(id)) {
        Some(&(_, points, saturation)) => {
            let good = good.food(points).saturation(saturation);
            if MEATS.contains(&bare(id)) { good.meat() } else { good }
        }
        None => good,
    }
}

/// The name of the good a piece is.
fn name_of<'c>(content: &'c Content, w: &Worn) -> &'c str {
    content.goods[w.good].name.as_str()
}

/// Whole units in `x`, forgiving the dust floats leave.
fn whole(x: f64) -> f64 {
    (x + 1e-9).floor().max(0.0)
}

/// How hearty food of this saturation a point is: how far above plain bread's, as a share of bread's, 0 to 1 (a float's
/// rounding is no feast): `Army.heartiness(double)`.
pub fn heartiness_of(saturation_per_point: f64) -> f64 {
    let above = saturation_per_point - BREAD_SATURATION;
    if above < 1e-4 { 0.0 } else { (above / BREAD_SATURATION).min(1.0) }
}

/// How hearty a food is: by its saturation, and meat at least as a pork chop (`Army.heartiness(Item)`).
pub fn heartiness(good: &Good) -> f64 {
    let by_saturation = if good.food > 0.0 { heartiness_of(good.saturation / good.food) } else { 0.0 };
    if good.meat { by_saturation.max(MEAT_HEARTINESS) } else { by_saturation }
}

/// The foods a warehouse holds, the heartiest first, then by saturation a point, bread and the like last
/// (`Army.heartiestFirst`); equals by name, as the mod's stock lists its items by id.
pub fn heartiest_first(content: &Content, stock: &Stock) -> Vec<GoodId> {
    let mut foods: Vec<GoodId> = (0..content.goods.len())
        .filter(|&g| content.goods[g].food > 0.0 && whole(stock.0[g]) >= 1.0)
        .collect();
    let per_point = |g: GoodId| content.goods[g].saturation / content.goods[g].food;
    foods.sort_by(|&a, &b| {
        heartiness(&content.goods[b]).total_cmp(&heartiness(&content.goods[a]))
            .then(per_point(b).total_cmp(&per_point(a)))
            .then(bare(&content.goods[a].name).cmp(bare(&content.goods[b].name)))
    });
    foods
}

/// What a meal was: the food points eaten, and their heartiness summed by point.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Meal {
    pub eaten: f64,
    pub hearty: f64,
}

/// Eats `points` of food from the warehouse, the heartiest first, whole items (`Army.mess`): what was eaten leaves the
/// world's goods.
pub fn mess(content: &Content, ledger: &mut Ledger, stock: &mut Stock, points: f64) -> Meal {
    let mut meal = Meal::default();
    if points <= 0.0 {
        return meal;
    }
    for g in heartiest_first(content, stock) {
        let left = points - meal.eaten;
        if left <= 1e-9 {
            break;
        }
        let food = &content.goods[g];
        let taken = (left / food.food - 1e-9).ceil().min(whole(stock.0[g]));
        stock.0[g] -= taken;
        ledger.used[g] += taken;
        meal.eaten += taken * food.food;
        meal.hearty += taken * food.food * heartiness(food);
    }
    meal
}

/// The risk factor of wearing so many armour points: 1.6 bare, down to a half at 20, in a line (`Army.armourRisk`).
pub fn armour_risk(points: f64) -> f64 {
    UNARMOURED_RISK * (1.0 - (1.0 - 0.5 / UNARMOURED_RISK) * (points / FULL_SET_POINTS).min(1.0))
}

/// Armour points worn, every piece together (`Army.points`): what isn't armour counts none.
pub fn points<'a>(armour: impl IntoIterator<Item = &'a str>) -> f64 {
    armour.into_iter().filter_map(self::armour).map(|(_, g)| g.power as f64).sum()
}

/// A campaign day's chance of falling (`Army.risk`): a soldier with `weapon` (by name, and its edge: the levels of the
/// enchantment suited to the enemy, Sharpness or for a bow Power), wearing `armour` (each piece by name, and its
/// guard: Protection's levels), with a shield or not. `death_base`, times the weapon's risk (unarmed: twice), less
/// 5% a level of edge (to 0.6), times the armour's by its points, less 6% a level of guard (to a half), times the
/// shield's.
pub fn risk(d: &Dials, weapon: Option<(&str, f64)>, armour: &[(&str, f64)], shield: bool) -> f64 {
    let w = weapon.and_then(|(name, edge)| self::weapon(name).map(|g| (g, edge)));
    let edge = w.map_or(0.0, |(_, e)| e);
    let guard: f64 = armour.iter().map(|(_, g)| g).sum();
    d.death_base
        * w.map_or(UNARMED_RISK, |(g, _)| g.risk) * (1.0 - EDGE_PER_LEVEL * edge).max(EDGE_FLOOR)
        * armour_risk(points(armour.iter().map(|(n, _)| *n))) * (1.0 - GUARD_PER_LEVEL * guard).max(GUARD_FLOOR)
        * if shield { SHIELD.risk } else { 1.0 }
}

/// The ticks a party takes over `blocks` of road (`Caravans.travelTicks`): 20 a block, 20 at least.
pub fn travel_ticks(blocks: f64) -> f64 {
    (blocks * TICKS_PER_BLOCK).max(LEAST_TRIP_TICKS)
}

/// Days of rations the front keeps, `blocks` from the capital (`Army.rationDays`): what it eats while the next order
/// goes and its delivery comes (the order rides back with a caravan, one sets out with the goods, and a caravan may be
/// on the road meanwhile: three round trips), and `ration_spare_days` spare.
pub fn ration_days(content: &Content, blocks: f64) -> u32 {
    let round_trip = (2.0 * travel_ticks(blocks) / content.day_ticks.max(1) as f64).ceil() as u32;
    3 * round_trip + content.dials.ration_spare_days
}

/// Blocks between two towns' hexes, as the crow flies (none if either isn't placed).
pub fn blocks_between(a: Option<crate::hex::Hex>, b: Option<crate::hex::Hex>) -> f64 {
    match (a, b) {
        (Some(a), Some(b)) => crate::fixed::hypot(a.center_x() - b.center_x(), a.center_z() - b.center_z()),
        _ => 0.0,
    }
}

/// Who a town sends of `n` (`Caravans.raiseLevy`): its grown hands at home but the Merchant at its desk, idle hands
/// first, then those of a trade it has others of, then the rest, the least skilled first of each; equals in the
/// roster's order. Places in the roster.
pub fn levy_order(content: &Content, roster: &Roster, n: usize) -> Vec<usize> {
    let at_home: Vec<usize> = (0..roster.hands.len())
        .filter(|&i| !roster.hands[i].is_baby() && !roster.hands[i].away)
        .collect();
    let of_trade = |t| at_home.iter().filter(|&&i| roster.hands[i].trade == t).count();
    let mut able: Vec<usize> = at_home.iter().copied().filter(|&i| roster.merchant != Some(i)).collect();
    able.sort_by_key(|&i| {
        let h = &roster.hands[i];
        let rank = if content.trades.get(h.trade).is_some_and(|t| t.idle()) { 0 } else if of_trade(h.trade) > 1 { 1 } else { 2 };
        (rank, h.level)
    });
    able.truncate(n);
    able
}

/// A piece a soldier wears: its good, its enchantment suited to the enemy and its Unbreaking (from the front's
/// [`Kits`], 0 for a plain piece), and how worn it is (gone at 1).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Worn {
    pub good: GoodId,
    pub enchant: f64,
    pub unbreaking: u8,
    pub worn: f64,
}

impl Worn {
    /// A day's wear of a piece lasting `days` (Unbreaking makes it last its level more times as long): true once it's
    /// worn out. The mod's running sum against 1, dust and all: summed in doubles, 1/6, 1/10, 1/24 and 1/30 fall just
    /// short of 1 on their last day, so there a stone weapon lasts 7 days, an iron sword or bow 11, diamond armour 31,
    /// and the core's the same.
    fn wear(&mut self, days: u32) -> bool {
        self.worn += 1.0 / (days.max(1) as f64 * (1 + self.unbreaking as u32) as f64);
        self.worn >= 1.0
    }
}

/// A serving soldier (`Army.Soldier`): its town, the hand it is (as it left home, by its id; none for one with no
/// roster), when it joined, the kit it wears, its arrows, the days it went hungry, the risk it has run (summed by day,
/// for who falls), and its pay a day, owed under its levy contract if it came by one.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Soldier {
    pub home: TownId,
    pub hand: Option<Hand>,
    pub enlisted: u64,
    pub weapon: Option<Worn>,
    pub armour: [Option<Worn>; 4],
    pub shield: Option<Worn>,
    pub arrows: u32,
    pub hungry_days: u32,
    pub exposure: f64,
    pub levy: Option<usize>,
    pub pay: f64,
}

/// A recruit waiting at the capital for the front (`Army.barracks`): its town, the hand, when it came, its levy and pay.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Recruit {
    pub home: TownId,
    pub hand: Option<Hand>,
    pub mustered: u64,
    pub levy: Option<usize>,
    pub pay: f64,
}

/// A soldier or recruit walking home (`CaravanLedger.walkHome`): at home again, in its town's roster, when it arrives.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Walker {
    pub home: TownId,
    pub hand: Option<Hand>,
    pub arrives: u64,
}

/// What a town gave the war (`Army.Record`): soldiers raised, fallen and deserted; soldier-days served, of which
/// hungry; its strength (soldier-days weighted by kit and meals); and the pay its soldiers earned.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub raised: u32,
    pub fallen: u32,
    pub deserted: u32,
    pub days: u32,
    pub hungry: u32,
    pub strength: f64,
    pub pay: crate::fixed::Fixed,
}

/// The day's losses, for their towns: the soldiers fallen and the ones who deserted.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Losses {
    pub fallen: Vec<Soldier>,
    pub deserted: Vec<Soldier>,
}

/// The sovereign's army (`Army.java`): its soldiers at the front, the recruits waiting at the capital, the front
/// (whose warehouse it eats and draws from), each town's record, the kit issued (by good, and whether enchanted),
/// food points eaten and their heartiness by soldier-day, the campaign's clock, the expected losses carried (fallen
/// and deserted, a fraction of a soldier), those walking home, and the pay owed towns outside a levy contract.
#[derive(Resource, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Army {
    pub soldiers: Vec<Soldier>,
    pub barracks: Vec<Recruit>,
    pub front: Option<TownId>,
    pub records: BTreeMap<TownId, Record>,
    pub issued: BTreeMap<String, u32>,
    pub eaten: f64,
    pub hearty_sum: f64,
    pub meal_days: f64,
    pub last_day: Option<u64>,
    pub falling: f64,
    pub deserting: f64,
    pub walking: Vec<Walker>,
    pub pay_owed: BTreeMap<TownId, crate::fixed::Fixed>,
}

impl Army {
    pub fn size(&self) -> usize {
        self.soldiers.len()
    }

    /// Recruits waiting at the capital for the front (`Army.waiting`).
    pub fn waiting(&self) -> usize {
        self.barracks.len()
    }

    /// Soldiers serving from a town (`Army.servingFrom`).
    pub fn serving_from(&self, home: TownId) -> usize {
        self.soldiers.iter().filter(|s| s.home == home).count()
    }

    /// A town's record (empty if it gave nothing).
    pub fn record(&self, home: TownId) -> Record {
        self.records.get(&home).cloned().unwrap_or_default()
    }

    /// How hearty its meals were, on average over soldier-days: 0 plain bread, 1 the best (`Army.heartiness`).
    pub fn heartiness(&self) -> f64 {
        if self.meal_days == 0.0 { 0.0 } else { self.hearty_sum / self.meal_days }
    }

    /// A recruit joins the army (`Army.enlist`): its town counts it raised.
    pub fn enlist(&mut self, recruit: Recruit, now: u64) {
        self.records.entry(recruit.home).or_default().raised += 1;
        self.soldiers.push(Soldier { home: recruit.home, hand: recruit.hand, enlisted: now, levy: recruit.levy,
            pay: recruit.pay, ..Default::default() });
    }

    /// Up to `n` waiting recruits leave the barracks for the front, the first come first (`Army.embark`).
    pub fn embark(&mut self, n: usize) -> Vec<Recruit> {
        let n = n.min(self.barracks.len());
        self.barracks.drain(..n).collect()
    }

    /// Food points a day's rations take for the soldiers and the recruits on their way, `ration_days` of them.
    pub fn food_wanted(&self, d: &Dials, ration_days: u32) -> f64 {
        (self.soldiers.len() + self.barracks.len()) as f64 * d.army_ration * ration_days as f64
    }

    /// What kit and food the army will want on hand (`Army.wants`), of the goods the host has: an iron sword and an
    /// iron set each, shields for half, two days' arrows each, and `ration_days` of food for each soldier and each
    /// recruit waiting to join, counted as bread (the quartermaster sends heartier food in its place where the capital
    /// has it). Every kind is listed, none wanted or not.
    pub fn wants(&self, content: &Content, ration_days: u32) -> Vec<(GoodId, u32)> {
        let d = &content.dials;
        let men = self.soldiers.len() as u32;
        let mut wants: Vec<(&str, u32)> = vec![(WEAPONS[1].name, men)];
        wants.extend(ARMOUR.iter().map(|place| (place[1].name, men))); // an iron set each
        wants.push((SHIELD.name, men / 2));
        wants.push((ARROW, men * d.arrows_per_day * 2));
        let mut out: Vec<(GoodId, u32)> = wants.into_iter().filter_map(|(n, c)| good_named(content, n).map(|g| (g, c)))
            .collect();
        if let Some(bread) = good_named(content, BREAD).filter(|&b| content.goods[b].food > 0.0) {
            let loaves = (self.food_wanted(d, ration_days) / content.goods[bread].food - 1e-9).ceil().max(0.0);
            out.push((bread, loaves as u32));
        }
        out
    }

    /// The best piece of these kinds the warehouse has, enchanted ones first (`Army.draw`): taken from it (and from its
    /// enchanted pieces), out of the world's goods, and tallied as issued.
    fn draw(&mut self, content: &Content, ledger: &mut Ledger, stock: &mut Stock, kits: &mut Kits,
            kinds: impl IntoIterator<Item = &'static str>) -> Option<Worn> {
        for name in kinds {
            let Some(g) = good_named(content, name) else { continue };
            if whole(stock.0[g]) < 1.0 {
                continue;
            }
            // The piece there with the most enchantments (the mod's `getEnchantments().size()`: its enchantment and its
            // Unbreaking, whatever their levels), the first of equals.
            let enchantments = |k: &crate::kit::Kit| (k.enchant_level > 0.0) as u8 + (k.unbreaking > 0) as u8;
            let mut best: Option<usize> = None;
            for (i, k) in kits.pieces.iter().enumerate() {
                if k.good != g || k.count == 0 {
                    continue;
                }
                if best.is_none_or(|b| enchantments(k) > enchantments(&kits.pieces[b])) {
                    best = Some(i);
                }
            }
            let piece = match best {
                Some(i) => {
                    let k = &mut kits.pieces[i];
                    k.count -= 1;
                    let worn = Worn { good: g, enchant: k.enchant_level, unbreaking: k.unbreaking, worn: 0.0 };
                    kits.pieces.retain(|k| k.count > 0);
                    worn
                }
                None => Worn { good: g, ..Default::default() },
            };
            stock.0[g] -= 1.0;
            ledger.used[g] += 1.0;
            let enchanted = piece.enchant > 0.0 || piece.unbreaking > 0;
            *self.issued.entry(format!("{}{}", bare(&content.goods[g].name), if enchanted { " (enchanted)" } else { "" }))
                .or_default() += 1;
            return Some(piece);
        }
        None
    }

    /// Arrows from the warehouse, up to `n`: out of the world's goods.
    fn take_arrows(content: &Content, ledger: &mut Ledger, stock: &mut Stock, n: u32) -> u32 {
        let Some(g) = good_named(content, ARROW) else { return 0 };
        let taken = whole(stock.0[g]).min(n as f64);
        stock.0[g] -= taken;
        ledger.used[g] += taken;
        taken as u32
    }

    /// One campaign day on the front's warehouse (`Army.campaign`'s day): the army eats, its heartiest food first, as
    /// far as its rations go; the hungry desert at the expected rate, carried; the rest draw what kit they lack, the
    /// best there is, serve (their towns' records and pay), wear their kit, and run their risk, whose sum is the day's
    /// expected fallen, carried. Returns the day's losses, already off the army.
    pub fn day(&mut self, content: &Content, ledger: &mut Ledger, book: &mut Book, stock: &mut Stock, kits: &mut Kits,
            capital: Option<TownId>) -> Losses {
        let d = &content.dials;
        let mut losses = Losses::default();
        // The day's rations, the best food first: as far as they go, the army is fed, and the heartier the meal, the
        // better it fights and the less it falls or deserts.
        let n = self.soldiers.len();
        let wanted = n as f64 * d.army_ration;
        let meal = mess(content, ledger, stock, wanted);
        self.eaten += meal.eaten;
        let fed = if wanted <= 0.0 { 1.0 } else { (meal.eaten / wanted).min(1.0) };
        let hearty = if meal.eaten <= 0.0 { 0.0 } else { meal.hearty / meal.eaten };
        self.hearty_sum += hearty * n as f64;
        self.meal_days += n as f64;
        let fed_soldiers = (n as f64 * fed).floor() as usize; // (the mod's floor, dust and all)
        let mut hungry: Vec<bool> = (0..n).map(|i| i >= fed_soldiers).collect();
        // Deserted: the hungry's chance, summed and carried; whole deserters walk home, their kit lost to the army.
        let hungry_now = hungry.iter().filter(|h| **h).count();
        self.deserting += hungry_now as f64 * d.desertion * (1.0 - HEARTY_DESERTION * hearty);
        let going = (whole(self.deserting) as usize).min(hungry_now);
        if going > 0 {
            self.deserting = (self.deserting - going as f64).max(0.0); // (a float's dust is no deserter)
            let mut hungriest: Vec<usize> = (0..n).filter(|&i| hungry[i]).collect();
            hungriest.sort_by(|&a, &b| self.soldiers[b].hungry_days.cmp(&self.soldiers[a].hungry_days).then(b.cmp(&a)));
            let mut gone: Vec<usize> = hungriest[..going].to_vec();
            gone.sort_unstable_by(|a, b| b.cmp(a));
            for i in gone {
                hungry.remove(i);
                let soldier = self.soldiers.remove(i);
                self.records.entry(soldier.home).or_default().deserted += 1;
                losses.deserted.push(soldier);
            }
            losses.deserted.reverse(); // in the army's order
        }
        // The rest serve the day.
        for i in 0..self.soldiers.len() {
            let hungry = hungry[i];
            let mut soldier = std::mem::take(&mut self.soldiers[i]);
            // What it lacks, the best there is: a weapon (and arrows for a bow), each piece of armour, a shield.
            let bow = |s: &Soldier| s.weapon.as_ref().is_some_and(|w| bare(&content.goods[w.good].name) == BOW);
            if soldier.weapon.is_none() || bow(&soldier) && soldier.arrows == 0 {
                if bow(&soldier) {
                    soldier.arrows += Self::take_arrows(content, ledger, stock, d.arrows_per_day * ARROW_DRAW_DAYS);
                }
                if soldier.weapon.is_none() {
                    soldier.weapon = self.draw(content, ledger, stock, kits, WEAPONS.iter().map(|g| g.name));
                    if bow(&soldier) {
                        soldier.arrows += Self::take_arrows(content, ledger, stock, d.arrows_per_day * ARROW_DRAW_DAYS);
                    }
                }
            }
            for (place, kinds) in ARMOUR.iter().enumerate() {
                if soldier.armour[place].is_none() {
                    soldier.armour[place] = self.draw(content, ledger, stock, kits, kinds.iter().map(|g| g.name));
                }
            }
            if soldier.shield.is_none() && !bow(&soldier) {
                soldier.shield = self.draw(content, ledger, stock, kits, [SHIELD.name]);
            }
            // How it serves today, and the risk it runs: a bow with no arrows is no weapon.
            let name = |w: &Worn| name_of(content, w);
            let arms = soldier.weapon.as_ref().filter(|_| !bow(&soldier) || soldier.arrows > 0);
            let edge = arms.map_or(0.0, |w| w.enchant);
            let worn: Vec<(&str, f64)> = soldier.armour.iter().flatten().map(|w| (name(w), w.enchant)).collect();
            let risk = risk(d, arms.map(|w| (name(w), w.enchant)), &worn, soldier.shield.is_some())
                * if hungry { d.hungry_risk } else { 1.0 - HEARTY_RISK * hearty };
            let power = arms.and_then(|w| weapon(name(w))).map_or(1.0, |g| g.power as f64);
            let armour_points = points(worn.iter().map(|(n, _)| *n));
            let record = self.records.entry(soldier.home).or_default();
            record.days += 1;
            if hungry {
                record.hungry += 1;
                soldier.hungry_days += 1;
            }
            record.strength += power * (1.0 + 0.1 * edge) * (armour_points * 0.4 + 4.0) / 10.0
                * if hungry { 0.5 } else { 1.0 + HEARTY_STRENGTH * hearty };
            // Its pay: owed under the levy it came by, while that's in force and is its home's with the capital (the
            // mod's `seller == home`, `buyer == capital`); else in the army's own pay ledger. (The mod pays through
            // the home's first such levy in force; the core's contracts have no levy term yet to find one by: the
            // crown's, with `Contract.levy`.)
            // (A soldier-day's pay, to the nearest thousandth: the dial's 0.25 is exact.)
            let pay = crate::fixed::Fixed::round(soldier.pay);
            record.pay += pay;
            let levied = |c: &crate::contracts::Contract| c.in_force() && c.seller == soldier.home && Some(c.buyer) == capital;
            match soldier.levy.filter(|&c| book.contracts.get(c).is_some_and(levied)) {
                Some(c) => book.owe(c, pay),
                None => *self.pay_owed.entry(soldier.home).or_default() += pay,
            }
            // Wear: Unbreaking makes it last.
            if arms.is_some() {
                if bow(&soldier) {
                    soldier.arrows = soldier.arrows.saturating_sub(d.arrows_per_day);
                }
                let days = soldier.weapon.as_ref().and_then(|w| weapon(name(w))).map_or(1, |g| g.days);
                if soldier.weapon.as_mut().is_some_and(|w| w.wear(days)) {
                    soldier.weapon = None;
                }
            }
            for place in 0..ARMOUR.len() {
                let days = soldier.armour[place].as_ref().and_then(|w| armour(name(w))).map_or(1, |(_, g)| g.days);
                if soldier.armour[place].as_mut().is_some_and(|w| w.wear(days)) {
                    soldier.armour[place] = None;
                }
            }
            if soldier.shield.as_mut().is_some_and(|w| w.wear(SHIELD.days)) {
                soldier.shield = None;
            }
            soldier.exposure += risk;
            self.falling += risk;
            self.soldiers[i] = soldier;
        }
        // Fallen: the day's expected losses, carried; whole soldiers fall, those who have run the most risk first.
        let falling = (whole(self.falling) as usize).min(self.soldiers.len());
        if falling > 0 {
            self.falling = (self.falling - falling as f64).max(0.0);
            let mut most: Vec<usize> = (0..self.soldiers.len()).collect();
            most.sort_by(|&a, &b| self.soldiers[b].exposure.total_cmp(&self.soldiers[a].exposure).then(a.cmp(&b)));
            let mut gone: Vec<usize> = most[..falling].to_vec();
            gone.sort_unstable_by(|a, b| b.cmp(a));
            for i in gone {
                let soldier = self.soldiers.remove(i);
                self.records.entry(soldier.home).or_default().fallen += 1;
                losses.fallen.push(soldier);
            }
            losses.fallen.reverse();
        }
        losses
    }
}

/// How much a town's grief takes from its mood now (`Territory.moodTarget`): `grief_mood` for each of its soldiers
/// fallen within `grief_days` of its last count, at most `grief_most`.
pub fn grief(d: &Dials, people: &People) -> f64 {
    let fallen: f64 = people.grief.iter().filter(|(until, _)| people.last < *until).map(|(_, n)| n).sum();
    (d.grief_mood * fallen).min(d.grief_most)
}

/// One of a town's soldiers fell at `now`: it grieves for `grief_days` (`Territory.mourn`). Griefs past are dropped.
pub fn mourn(content: &Content, people: &mut People, now: u64) {
    let until = now + (content.dials.grief_days * content.day_ticks as f64).round() as u64;
    people.grief.retain(|(u, _)| *u > now);
    match people.grief.iter_mut().find(|(u, _)| *u == until) {
        Some((_, n)) => *n += 1.0,
        None => people.grief.push((until, 1.0)),
    }
}

/// The hand that's a soldier off its town's roster, for good (`Settlements.remove`): its locker goes with it.
pub fn bury(roster: &mut Roster, lockers: Option<&mut crate::lockers::Lockers>, hand: &Hand) {
    if let Some(i) = roster.hands.iter().position(|h| h.away && h.id == hand.id) {
        if let Some(l) = lockers {
            l.take_hand(i);
        }
        crate::settlers::take_hand(roster, i);
    }
}

impl Society {
    /// The army as it stands: its soldiers, recruits, the front, the towns' records and the tally.
    pub fn army(&self) -> &Army {
        self.world.resource::<Army>()
    }

    /// The front, where the war is fought (`Army.setFront`): the army eats and draws its kit from this town's
    /// warehouse, a day at a time from the first look after.
    pub fn set_front(&mut self, town: TownId) {
        self.world.resource_mut::<Army>().front = Some(town);
    }

    /// The town's hands for the war: up to `n` (as a levy picks them, [`levy_order`]) leave home and wait at the
    /// capital for the front (`Army.muster`), away from home on its roster. Returns how many went (none from a town
    /// with no roster).
    pub fn muster(&mut self, town: TownId, n: usize) -> usize {
        self.muster_levied(town, n, None)
    }

    /// As [`Society::muster`], the recruits sent under a levy contract: their pay is owed through it while it's in
    /// force (`Levy.payPerDay`, the dial `soldier_pay`).
    pub fn muster_levied(&mut self, town: TownId, n: usize, levy: Option<usize>) -> usize {
        let e = self.towns[town];
        let now = self.now();
        let pay = self.content().dials.soldier_pay;
        let Some(roster) = self.world.get::<Roster>(e) else { return 0 };
        let picked = levy_order(self.content(), roster, n);
        let mut roster = self.world.get_mut::<Roster>(e).expect("a roster");
        let recruits: Vec<Recruit> = picked.into_iter()
            .filter_map(|i| crate::squad::stamp(&mut roster, i))
            .map(|hand| Recruit { home: town, hand: Some(hand), mustered: now, levy, pay })
            .collect();
        let k = recruits.len();
        self.world.resource_mut::<Army>().barracks.extend(recruits);
        k
    }

    /// Up to `n` recruits waiting at the capital reach the front and join the army, the first come first
    /// (`Army.embark`, then `enlist` where the caravan arrives). Returns how many joined.
    pub fn embark(&mut self, n: usize) -> usize {
        let now = self.now();
        let mut army = self.world.resource_mut::<Army>();
        let going = army.embark(n);
        let k = going.len();
        for recruit in going {
            army.enlist(recruit, now);
        }
        k
    }

    /// Up to `n` of a town's hands join the army at once (mustered and at the front: `Army.enlist`). Returns how many.
    pub fn enlist(&mut self, town: TownId, n: usize) -> usize {
        let k = self.muster(town, n);
        let now = self.now();
        let mut army = self.world.resource_mut::<Army>();
        let from = army.barracks.len() - k;
        let joining: Vec<Recruit> = army.barracks.drain(from..).collect();
        for recruit in joining {
            army.enlist(recruit, now);
        }
        k
    }

    /// Peace (`Army.demobilise`): the soldiers walk home from the front, and the recruits from the capital, as long as
    /// the road takes; their kit stays with the army. The campaign's clock stops. Returns how many went home.
    pub fn demobilise(&mut self) -> usize {
        let now = self.now();
        let capital = self.world.resource::<crate::realms::Crown>().capital;
        let hex = |s: &Society, t: Option<TownId>| t.filter(|&t| t < s.towns.len()).and_then(|t| s.town_hex(t));
        let army = self.world.resource::<Army>().clone();
        let from = army.front.or(capital);
        let mut walking: Vec<Walker> = Vec::new();
        for s in &army.soldiers {
            let ticks = travel_ticks(blocks_between(hex(self, Some(s.home)), hex(self, from)));
            walking.push(Walker { home: s.home, hand: s.hand.clone(), arrives: now + ticks as u64 });
        }
        for r in &army.barracks {
            let ticks = travel_ticks(blocks_between(hex(self, Some(r.home)), hex(self, capital)));
            walking.push(Walker { home: r.home, hand: r.hand.clone(), arrives: now + ticks as u64 });
        }
        let home = walking.len();
        let mut army = self.world.resource_mut::<Army>();
        army.walking.extend(walking);
        army.soldiers.clear();
        army.barracks.clear();
        army.last_day = None;
        army.falling = 0.0;
        army.deserting = 0.0;
        home
    }

    /// What the front's quartermaster wants on hand (`Army.wants` with `rationDays` of the road from the capital to the
    /// front): nothing until a front is set.
    pub fn front_wants(&self) -> Vec<(GoodId, u32)> {
        let army = self.army();
        let Some(front) = army.front else { return Vec::new() };
        let capital = self.world.resource::<crate::realms::Crown>().capital;
        let blocks = blocks_between(capital.and_then(|c| self.town_hex(c)), self.town_hex(front));
        army.wants(self.content(), ration_days(self.content(), blocks))
    }
}
