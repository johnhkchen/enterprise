//! What the land gives and what kit does: a Toolsmith's mine, hands gathering what their hex's ground gives, Fletchers
//! hunting, kit wearing out, the shares of its wearers a town's kit covers, and armed and armoured guards.
//!
//! Ported from the mod:
//! - `territory/Mines.java`: `RAW_IRON_PER_LEVEL_DAY` (2), `COAL_PER_LEVEL_DAY` (0.5), `RAW_GOLD_PER_LEVEL_DAY` (0.25),
//!   `LAPIS_PER_LEVEL_DAY` (0.5), `DEEP_EMERALDS` (8), `MINE_EMERALDS` (6): the dials of the same names.
//! - `territory/Territory.java`: `touch` (1328-1365: the mine's ore, half again with pickaxes and with a smithy; the
//!   hands' gathering with tools to hand, wearing out in `HAND_TOOL_DAYS`; the Fletchers' hunt, a quarter without bows,
//!   twice with a Master; the kit's wear) and its end (1419-1426: the kit shares, `hunting`, `armed`, a watchtower's
//!   floor of a half), `digMine` (508: a Toolsmith, a spare `MINE_EMERALDS`), `sinkDeepMine` (434), `miners` (498),
//!   `giveTools` (676); `chokepoints`' use of `armed` and the armour share is `chokepoints.rs`'s.
//! - `territory/Labour.java`: `hauls` (54) by land, `hands` (116: Nitwits first, each `NITWIT_FACTOR`), `work` (130:
//!   hand `i` works hex `i` of the town's, round again; the last `militia` stand watch), `room` (95: a good is
//!   gathered while the town holds under `KEEP_WANTED` of what it uses and `KEEP` of the rest, and the warehouse has
//!   `SPARE_SLOTS` free beyond those it keeps); `territory/Hunting.java` `game` (19).
//! - `territory/Smithing.java`: `KITS` (`services.rs`), `TOOL_BONUS`, `SNARES`, `quality` (86), `wearRate` (97),
//!   `share` (111), `hunting` (65), `armed` (73), `wear` (125-140, with its three overloads).
//! - `market/ColdStore.java` `equipHands` (240): hands working the land without a tool are given an axe or shovel
//!   from the warehouse. (Giving idle tools to the hands, `giveTools`, is the host's: [`Society::give_tools`].)
//! - `service/MasterServices.java` `investWanted` (114), `invest` (119), `enterprise:deep_mine` (68): the deep mine only.
//!
//! **Where the core differs.** The mod's biomes are one [`Land`] a hex (`map.rs`), and the mod's items are goods by
//! name: a good the host hasn't is never gathered. Kit is goods in the warehouse; a piece with enchantments is a
//! [`Kit`] entry in the town's [`Kits`], counted in the warehouse's stock of that good (the stock has no stacks to hold
//! an enchantment): `count` of the pieces held are enchanted so. Work is done once a day on the days since the last
//! (not at each touch), so the figures are those of whole days; the kit shares are measured every look.
//! Worn-out pieces come from the front of the enchanted list, as the first copies in use. A host with none of the
//! guards' kit goods (sword, bow, chestplate) keeps its own `armed` and `armoured` (`Society::set_guard_kit`). The
//! mine has no site and no shaft: a town with a spare `mine_emeralds` and a Toolsmith digs one on its land. Hoes give
//! a Farm its `hands` (half again the plots tended); the mending service, the armoury and the Cleric's enchanting that
//! make `mended_until`, `armoury_until` and the enchanted pieces are later slices': hosts set them.

use std::collections::BTreeMap;

use bevy_ecs::prelude::Component;
use serde::{Deserialize, Serialize};

use crate::content::{Content, GoodId};
use crate::map::Land;
use crate::roster::Roster;
use crate::services::{Wearer, BEST, KITS};
use crate::state::Trades;
use crate::territory::{Holding, Territory};

/// A Nitwit gathers this much more than another hand (`Labour.NITWIT_FACTOR`).
pub const NITWIT_FACTOR: f64 = 1.5;
/// A good is gathered while the warehouse holds fewer than this of it, or [`KEEP_WANTED`] of what the town uses
/// (`Labour.KEEP`, `KEEP_WANTED`), and while it has [`SPARE_SLOTS`] free beyond those it keeps (`SPARE_SLOTS`).
pub const KEEP: f64 = 16.0;
pub const KEEP_WANTED: f64 = 64.0;
pub const SPARE_SLOTS: i64 = 4;
/// A Fletcher without a bow still hunts with snares, at this share (`Smithing.SNARES`).
pub const SNARES: f64 = 0.25;
/// Each level of enchantment but Unbreaking adds this much quality, up to [`BEST`] (`Smithing.QUALITY_PER_LEVEL`).
pub const QUALITY_PER_LEVEL: f64 = 0.15;
/// Tools wear this much as fast while a Toolsmith mends them, kit this much as fast with a smithy, and the guards' kit
/// with an armoury (`Territory.touch`'s arguments to `Smithing.wear`).
pub const MENDED_WEAR: f64 = 0.5;
pub const SMITHY_WEAR: f64 = 0.8;
pub const ARMOURY_WEAR: f64 = 0.5;
/// A smithy brings up half as much ore again (`Territory.touch`).
pub const SMITHY_ORE: f64 = 1.5;
/// A Master Fletcher leads a hunting party: twice the game; a watchtower's archers arm at least half the guards.
pub const MASTER_LEVEL: u8 = 5;
pub const HUNTING_PARTY: f64 = 2.0;
pub const WATCHTOWER_ARMED: f64 = 0.5;
/// No more than this many days are worked at once (`Territory.MAX_DAYS`).
pub const MAX_DAYS: f64 = 60.0;
/// The Toolsmith's partner in sinking a mine deep, and the level it needs (`MasterServices`: a Mason of 3).
pub const MASON_LEVEL: u8 = 3;
/// The tools a hand without one is given, in order (`ColdStore.equipHands`).
pub const HAND_TOOLS: [&str; 4] = ["iron_axe", "stone_axe", "iron_shovel", "stone_shovel"];

/// What a hand gathers a day on land of this kind (`Labour.hauls`), by good name: the mod's biome tags as the land
/// kinds `map.rs` has (woods take the forest's, taiga the taiga's, desert the desert's, shore the beach's, hills the
/// mountains' and hills', water the ocean's and rivers', plains the open ground's).
pub fn hauls(land: Land) -> &'static [(&'static str, f64)] {
    match land {
        Land::Water => &[("clay_ball", 4.0), ("sugar_cane", 2.0), ("ink_sac", 1.0)],
        Land::Shore => &[("sand", 12.0), ("clay_ball", 2.0)],
        Land::Desert => &[("sand", 16.0), ("cactus", 2.0), ("stick", 2.0)],
        Land::Hills => &[("cobblestone", 16.0), ("flint", 3.0)],
        Land::Taiga => &[("spruce_log", 6.0), ("sweet_berries", 3.0), ("stick", 2.0)],
        Land::Woods => &[("oak_log", 6.0), ("stick", 4.0), ("apple", 1.0), ("string", 0.5)],
        Land::Plains => &[("wheat_seeds", 4.0), ("dandelion", 2.0), ("poppy", 2.0), ("stick", 2.0), ("flint", 1.0),
            ("oak_log", 1.0), ("string", 0.5)],
    }
}

/// What a Fletcher of level 1 with a bow brings in a day on land of this kind (`Hunting.game`): feathers by water;
/// rabbits, hides and feathers where it's cold or dry; cattle, pigs and fowl, feathers and leather elsewhere.
pub fn game(land: Land) -> &'static [(&'static str, f64)] {
    match land {
        Land::Water => &[("feather", 1.0)],
        Land::Desert | Land::Taiga => &[("rabbit", 2.0), ("rabbit_hide", 1.0), ("feather", 1.0)],
        _ => &[("beef", 1.0), ("porkchop", 1.0), ("chicken", 1.0), ("feather", 2.0), ("leather", 0.5)],
    }
}

/// A town's mine (`Territory.mine`, `deepMine`): dug by a Toolsmith, sunk deep by a Master with a Mason's help.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mine {
    pub deep: bool,
}

/// Pieces of kit held with enchantments (`Smithing.quality`, `wearRate`): `count` of the town's pieces of `good`,
/// each with `enchant_level` levels of enchantment but Unbreaking and Unbreaking `unbreaking`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Kit {
    pub good: GoodId,
    pub count: u32,
    pub enchant_level: f64,
    pub unbreaking: u8,
}

impl Kit {
    /// How good one piece is: 1 plain, more enchanted, up to [`BEST`] (`Smithing.quality`).
    pub fn quality(&self) -> f64 {
        (1.0 + QUALITY_PER_LEVEL * self.enchant_level).min(BEST)
    }

    /// How fast one piece wears: Unbreaking makes it last (1 + its level) times as long (`Smithing.wearRate`).
    pub fn wear_rate(&self) -> f64 {
        1.0 / (1 + self.unbreaking) as f64
    }
}

/// A town's kit and what its land gives it that isn't yet in the warehouse: the pieces with enchantments, the wear
/// owed on each kind (a fraction of a piece), the ore, game and goods gathered and not yet brought in, the tools its
/// hands hold, the share of each kind's wearers its kit covers (by [`KITS`]), and when it last worked.
#[derive(Component, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Kits {
    pub pieces: Vec<Kit>,
    pub wear: BTreeMap<GoodId, f64>,
    pub pending: BTreeMap<GoodId, f64>,
    pub hand_tools: f64,
    pub shares: [f64; 5],
    /// When it last worked (the day it was made, to begin with).
    pub last: u64,
    /// A Toolsmith mends its tools until then; an armoury keeps its guards' kit until then.
    pub mended_until: Option<u64>,
    pub armoury_until: Option<u64>,
}

/// Whole units in `x`, forgiving the dust floats leave.
fn whole(x: f64) -> f64 {
    (x + 1e-9).floor().max(0.0)
}

/// Who wears kit, by the number of each (`Smithing.wearers`' arguments).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Crew {
    pub farmers: usize,
    pub miners: usize,
    pub guards: usize,
    pub hunters: usize,
}

impl Crew {
    pub fn wearers(&self, wearer: Wearer) -> usize {
        crate::services::wearers(wearer, self.farmers, self.miners, self.guards, self.hunters)
    }
}

/// How fast kit wears: tools at this share of their rate (mended), all kit at `kit` (a smithy keeps it), the guards'
/// kit at `guards` (an armoury keeps it): `Smithing.wear`'s `toolWear`, `kitWear` and `guardWear`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wear {
    pub tools: f64,
    pub kit: f64,
    pub guards: f64,
}

impl Default for Wear {
    fn default() -> Self {
        Wear { tools: 1.0, kit: 1.0, guards: 1.0 }
    }
}

impl Wear {
    /// Plain wear, with tools mended or not (`Smithing.wear(..., mended)`).
    pub fn mended(mended: bool) -> Self {
        Wear { tools: if mended { MENDED_WEAR } else { 1.0 }, ..Default::default() }
    }
}

impl Kits {
    /// The enchanted pieces of a good, clipped to the `held` pieces there are (the stock may have lost some to a sale
    /// or a caravan since): `(quality, wear rate, count)` in the order held.
    fn enchanted(&self, good: GoodId, held: u32) -> Vec<(f64, f64, u32)> {
        let mut left = held;
        let mut out = Vec::new();
        for k in self.pieces.iter().filter(|k| k.good == good) {
            let n = k.count.min(left);
            if n > 0 {
                out.push((k.quality(), k.wear_rate(), n));
                left -= n;
            }
        }
        out
    }

    /// Takes away the enchanted pieces in excess of what the warehouse holds, the last first.
    pub fn fit(&mut self, content: &Content, stock: &[f64]) {
        for g in 0..content.goods.len().min(stock.len()) {
            let held = whole(stock[g]) as u32;
            let mut total: u32 = self.pieces.iter().filter(|k| k.good == g).map(|k| k.count).sum();
            while total > held {
                let Some(i) = self.pieces.iter().rposition(|k| k.good == g && k.count > 0) else { break };
                let cut = (total - held).min(self.pieces[i].count);
                self.pieces[i].count -= cut;
                total -= cut;
            }
        }
        self.pieces.retain(|k| k.count > 0);
    }

    /// Adds `count` pieces of a good already in the warehouse as enchanted so (`Smithing.quality`, `wearRate`).
    pub fn enchant(&mut self, good: GoodId, count: u32, enchant_level: f64, unbreaking: u8) {
        if count == 0 || enchant_level <= 0.0 && unbreaking == 0 {
            return;
        }
        match self.pieces.iter_mut().find(|k| k.good == good && k.enchant_level == enchant_level && k.unbreaking == unbreaking) {
            Some(k) => k.count += count,
            None => self.pieces.push(Kit { good, count, enchant_level, unbreaking }),
        }
    }

    /// The share of `wearers` a kind of kit covers: the best pieces first, enchanted ones counting for more, 0 to
    /// [`BEST`] (0 with no wearers) (`Smithing.share`).
    pub fn share(&self, good: GoodId, held: f64, wearers: usize) -> f64 {
        if wearers == 0 {
            return 0.0;
        }
        let held = whole(held) as u32;
        let mut groups: Vec<(f64, u32)> = self.enchanted(good, held).into_iter().map(|(q, _, n)| (q, n)).collect();
        let enchanted: u32 = groups.iter().map(|g| g.1).sum();
        groups.push((1.0, held - enchanted));
        groups.sort_by(|a, b| b.0.total_cmp(&a.0));
        let (mut left, mut sum) = (wearers as u32, 0.0);
        for (q, n) in groups {
            let take = n.min(left);
            sum += q * take as f64;
            left -= take;
        }
        (sum / wearers as f64).min(BEST)
    }

    /// Kit wears out over `days`: each wearer with kit uses up a share of one, by kind, carried in `wear`; whole ones
    /// are taken from the warehouse. Returns what was used up, by good (`Smithing.wear`).
    pub fn wear(&mut self, content: &Content, stock: &mut [f64], days: f64, crew: Crew, how: Wear) -> Vec<(GoodId, u32)> {
        let mut used = Vec::new();
        for kind in KITS.iter() {
            let Some(g) = content.good(kind.good) else { continue };
            let held = whole(stock[g]) as u32;
            let equipped = held.min(crew.wearers(kind.wearer) as u32);
            if equipped == 0 {
                self.wear.remove(&g); // nothing in use: nothing wearing out
                continue;
            }
            // The first copies in use, enchanted pieces first, then plain: the rate they wear at, on average.
            let mut groups: Vec<(f64, u32)> = self.enchanted(g, held).into_iter().map(|(_, r, n)| (r, n)).collect();
            let enchanted: u32 = groups.iter().map(|g| g.1).sum();
            groups.push((1.0, held - enchanted));
            let (mut left, mut sum) = (equipped, 0.0);
            for (rate, n) in groups {
                let take = n.min(left);
                sum += rate * take as f64;
                left -= take;
            }
            let rate = sum / equipped as f64;
            let tool = matches!(kind.wearer, Wearer::Farmers | Wearer::Miners);
            let owed = self.wear.get(&g).copied().unwrap_or(0.0)
                + equipped as f64 * days / kind.days * rate * if tool { how.tools } else { how.guards } * how.kit;
            let gone = (held as f64).min(owed.floor()) as u32;
            if gone > 0 {
                stock[g] -= gone as f64;
                self.take(g, gone);
                used.push((g, gone));
            }
            self.wear.insert(g, owed - gone as f64);
        }
        used
    }

    /// `n` pieces of a good gone from the front of the enchanted list (the first copies, those in use).
    fn take(&mut self, good: GoodId, mut n: u32) {
        for k in self.pieces.iter_mut().filter(|k| k.good == good) {
            let cut = k.count.min(n);
            k.count -= cut;
            n -= cut;
        }
        self.pieces.retain(|k| k.count > 0);
    }

    /// The share of its wearers each kind of kit covers, from the warehouse (`Territory.touch`'s end).
    pub fn measure(&mut self, content: &Content, stock: &[f64], crew: Crew) {
        for (i, kind) in KITS.iter().enumerate() {
            self.shares[i] = content.good(kind.good).map_or(0.0, |g| self.share(g, stock[g], crew.wearers(kind.wearer)));
        }
    }

    /// The share of one kind of kit's wearers covered, by the kit's good.
    pub fn share_of(&self, good: &str) -> f64 {
        KITS.iter().position(|k| k.good == good).map_or(0.0, |i| self.shares[i])
    }
}

/// The share of the hunters with a bow: bows go to the hunters first (`Smithing.hunting`).
pub fn hunting(bows: usize, hunters: usize) -> f64 {
    if hunters == 0 { 0.0 } else { (bows as f64 / hunters as f64).min(1.0) }
}

/// The share of the guards armed: those with swords, and with the bows the hunters don't need; at least `floor`
/// (archers on a watchtower) (`Smithing.armed`).
pub fn armed(swords: usize, bows: usize, guards: usize, hunters: usize, floor: f64) -> f64 {
    if guards == 0 {
        return 0.0;
    }
    let spare = bows.saturating_sub(hunters);
    floor.max(((swords + spare) as f64 / guards as f64).min(1.0))
}

/// A town's grown residents at home (`Territory.workers`) by trade name, and the levels of one trade's.
pub fn at_home<'a>(roster: &'a Roster, trade: Option<usize>) -> impl Iterator<Item = &'a crate::roster::Hand> {
    roster.hands.iter().filter(move |h| Some(h.trade) == trade && !h.away && !h.is_baby())
}

/// The levels of a trade's grown residents at home, summed, a Novice's at the least (`Territory.levels`).
pub fn levels(roster: &Roster, trade: Option<usize>) -> f64 {
    at_home(roster, trade).map(|h| h.level.max(1) as f64).sum()
}

/// A town's miners: its Toolsmiths at home, once it has a mine (`Territory.miners`).
pub fn miners(content: &Content, territory: &Territory, town: usize, roster: Option<&Roster>) -> usize {
    match (territory.holdings.get(&town), roster) {
        (Some(Holding { mine: Some(_), .. }), Some(r)) => at_home(r, content.trade("toolsmith")).count(),
        _ => 0,
    }
}

/// A town's crew of kit wearers (`Territory.touch`): its Farmers, miners, guards (posts held) and hunters (Fletchers).
pub fn crew(content: &Content, territory: &Territory, town: usize, roster: Option<&Roster>) -> Crew {
    let count = |name: &str| roster.map_or(0, |r| at_home(r, content.trade(name)).count());
    Crew {
        farmers: count("farmer"),
        miners: miners(content, territory, town, roster),
        guards: territory.holdings.get(&town).map_or(0, |h| h.posts.len()),
        hunters: count("fletcher"),
    }
}

/// Is there room to bring in more of a good: below what the town keeps of it (with what's owed), and slots to spare
/// (`Labour.room`)? A good is wanted when it's food, a comfort, its lockers can take it, or (`far`) the towns down its
/// roads buy it: its hands keep it for them as for itself.
pub fn room(content: &Content, trades: &Trades, stock: &[f64], crates: u32, good: GoodId, owed: f64, far: bool) -> bool {
    if free_slots(content, stock, crates) <= content.dials.free_slots as i64 + SPARE_SLOTS {
        return false;
    }
    let g = &content.goods[good];
    let wanted = far || g.food > 0.0 || g.comfort.is_some() || trades.capacity.get(good).is_some_and(|&c| c > 0.0);
    stock[good] + owed < if wanted { KEEP_WANTED } else { KEEP }
}

/// Free warehouse slots: what the stock's stacks and the crates leave (`MerchantStock.freeSlots`).
pub fn free_slots(content: &Content, stock: &[f64], crates: u32) -> i64 {
    let used: i64 = stock.iter().enumerate().map(|(g, n)| crate::cargo::stacks_of(content, g, *n) as i64).sum::<i64>()
        + crates as i64;
    content.dials.warehouse_slots as i64 - used
}

/// Brings in the whole items of a good that are owed, a stack at a time while the warehouse has room for the stack
/// (`MerchantStock.hasRoomFor`: it keeps the free slots for locker results and coins); the rest stays owed. Returns
/// how many came in.
pub fn bring_in(content: &Content, stock: &mut [f64], crates: u32, good: GoodId, owed: &mut f64) -> f64 {
    let mut left = whole(*owed);
    let mut put_all = 0.0;
    let stack = content.goods[good].stack.max(1) as f64;
    while left > 0.0 {
        let put = left.min(stack);
        let more = crate::cargo::stacks_of(content, good, stock[good] + put) as i64
            - crate::cargo::stacks_of(content, good, stock[good]) as i64;
        if (free_slots(content, stock, crates) - content.dials.free_slots as i64).max(0) < more {
            break;
        }
        stock[good] += put;
        left -= put;
        put_all += put;
    }
    *owed -= put_all;
    put_all
}

impl crate::society::Society {
    /// What a hex's land gives the hands working it (plains if never set).
    pub fn set_land(&mut self, hex: crate::hex::Hex, land: Land) {
        self.world.resource_mut::<crate::map::Map>().set_land(hex, land);
    }

    /// A town's kit as it is now: its enchanted pieces, the wear owed, what's gathered and owed it, its hand tools,
    /// the shares of each kind's wearers its kit covers (by [`KITS`]).
    pub fn kit(&self, town: usize) -> Kits {
        self.world.get::<Kits>(self.towns[town]).cloned().unwrap_or_default()
    }

    /// The share of one kind of kit's wearers a town's kit covers, as at its last look (`Territory.kitShare`).
    pub fn kit_share(&self, town: usize, good: &str) -> f64 {
        self.kit(town).share_of(good)
    }

    /// The share of its hunters with a bow, and of its guards armed and armoured, as at its last look.
    pub fn hunting(&self, town: usize) -> f64 {
        self.world.resource::<Territory>().holding(town).map_or(0.0, |h| h.hunting)
    }

    pub fn armed(&self, town: usize) -> f64 {
        self.world.resource::<Territory>().holding(town).map_or(0.0, |h| h.armed)
    }

    pub fn armoured(&self, town: usize) -> f64 {
        self.world.resource::<Territory>().holding(town).map_or(0.0, |h| h.armoured)
    }

    /// A town's mine, if it has dug one.
    pub fn mine(&self, town: usize) -> Option<Mine> {
        self.world.resource::<Territory>().holding(town).and_then(|h| h.mine)
    }

    /// A mine already dug (the host's own; `Territory.setMineForTest`), plain or deep.
    pub fn set_mine(&mut self, town: usize, deep: bool) {
        self.world.resource_mut::<Territory>().holdings.entry(town).or_default().mine = Some(Mine { deep });
    }

    /// Puts `count` pieces of kit in a town's warehouse as the host's own delivery, `enchant_level` levels of
    /// enchantment (but Unbreaking) and Unbreaking `unbreaking` on each. False if the host has no such good.
    pub fn add_kit(&mut self, town: usize, good: &str, count: u32, enchant_level: f64, unbreaking: u8) -> bool {
        let Some(g) = self.content().good(good) else { return false };
        self.deliver(town, g, count as f64);
        self.world.get_mut::<Kits>(self.towns[town]).expect("a town has its kit").enchant(g, count, enchant_level, unbreaking);
        true
    }

    /// Tools given to the hands working a town's land (`Territory.giveTools`): they gather half again as much while
    /// they last.
    pub fn give_tools(&mut self, town: usize, tools: u32) {
        self.world.get_mut::<Kits>(self.towns[town]).expect("a town has its kit").hand_tools += tools as f64;
    }

    /// A Toolsmith mends the town's tools until `until`: they wear half as fast (`Territory.mend`).
    pub fn mend(&mut self, town: usize, until: u64) {
        let mut kits = self.world.get_mut::<Kits>(self.towns[town]).expect("a town has its kit");
        kits.mended_until = Some(kits.mended_until.map_or(until, |u| u.max(until)));
    }

    /// An armoury keeps the town's guards' kit until `until`: it wears half as fast (`Territory.keepArmoury`).
    pub fn keep_armoury(&mut self, town: usize, until: u64) {
        let mut kits = self.world.get_mut::<Kits>(self.towns[town]).expect("a town has its kit");
        kits.armoury_until = Some(kits.armoury_until.map_or(until, |u| u.max(until)));
    }
}
