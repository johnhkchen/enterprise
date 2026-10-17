//! A town's people as a roster of hands, and trades as content (decision 1 of `docs/core-plan.md` section 2).
//!
//! The mod keeps its towns' people in a census roster (`settlement/Settlements.java` `Resident`: trade, level, baby,
//! away); the core keeps the same as a [`Roster`] of [`Hand`]s, and `People.villagers` follows it when a town has one
//! (towns without a roster count villagers as a number, as before). Ported from:
//!
//! - `caravan/Squad.java` `LEADS`, `TRADES`: the trades that lead a squad, and every trade that can go with one.
//! - `territory/Labour.java` `NONE`, `NITWIT`, `idle`, `hands` (116): the idle adults at home, Nitwits first.
//! - `locker/LockerTrading.java` `rewardTradeXp` (402): a villager levels at vanilla's thresholds, at once;
//!   `rewardMerchantXp` (412): the Merchant gets what a player would pick up (3 to 6 XP, 5 more when the villager
//!   levelled) and levels at a player's thresholds, `MERCHANT_LEVEL_XP`; `merchantXpForNextLevel` (423).
//! - `merchant/Merchant.java` `lockerLimit` (80): 2 lockers a Merchant level.
//! - `settlement/Census.java` and `docs/decisions.md` "The census keeps a roster": babies are residents (they need
//!   beds), not workers.
//! - `territory/Growth.java` `bear` (219), `growUp` (252): a child is marked with its birth time and grows up after
//!   `BABY_TICKS` of elapsed time, whether or not it was loaded.

use serde::{Deserialize, Serialize};

use bevy_ecs::prelude::Component;

use crate::content::Content;

/// A trade, by its place in [`Content::trades`].
pub type TradeId = usize;

/// The Merchant's trade: the desk's own hand, and a lead for caravans.
pub const MERCHANT: &str = "merchant";
/// An adult with no trade (`Labour.NONE`), and one that can't take one (`Labour.NITWIT`): the town's idle hands.
pub const NONE: &str = "none";
pub const NITWIT: &str = "nitwit";

/// The trades that can lead a squad, and that towns train for their caravans (`Squad.LEADS`).
pub const LEADS: [&str; 6] = [MERCHANT, "cartographer", "librarian", "shepherd", "leatherworker", "fisherman"];
/// Every trade that can go with a caravan, in the squad's order: the leads, and those who go only when their service
/// is wanted there (`Squad.TRADES`).
pub const SQUAD: [&str; 14] = [MERCHANT, "cartographer", "librarian", "shepherd", "leatherworker", "fisherman",
    "cleric", "farmer", "toolsmith", "armorer", "weaponsmith", "mason", "fletcher", "butcher"];

/// The XP a villager needs, in all, to leave each level from Novice to Expert: vanilla's `VillagerData`
/// thresholds. A level earned comes at once (`LockerTrading.rewardTradeXp`).
pub const VILLAGER_LEVEL_XP: [u32; 4] = [10, 70, 150, 250];
/// The XP the Merchant needs, in all, to leave each level: the points a player has at levels 15, 30, 45 and 55
/// (`LockerTrading.MERCHANT_LEVEL_XP`).
pub const MERCHANT_LEVEL_XP: [u32; 4] = [315, 1395, 4020, 6895];
/// The top level, Master.
pub const MAX_LEVEL: u8 = 5;
/// A child grows up this many ticks after it's born (`AgeableMob.BABY_START_AGE`, -24000).
pub const BABY_TICKS: u64 = 24_000;
/// The XP a locker trade gives the Merchant: at least this, plus a roll of up to `MERCHANT_TRADE_XP_ROLL - 1`, plus
/// `MERCHANT_LEVEL_UP_XP` when the villager levelled up on it (`LockerTrading.rewardMerchantXp`).
pub const MERCHANT_TRADE_XP: u32 = 3;
pub const MERCHANT_TRADE_XP_ROLL: u32 = 4;
pub const MERCHANT_LEVEL_UP_XP: u32 = 5;
/// Lockers a Merchant manages a level (`Merchant.lockerLimit`).
pub const LOCKERS_PER_LEVEL: u32 = 2;

/// A trade a hand can have: its name in the core (a host maps its own ids, `minecraft:farmer`, onto these), whether
/// it can lead a squad, go with one, is the Merchant's, or is no trade at all (`none`, `nitwit`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Trade {
    pub name: String,
    pub leads: bool,
    pub squad: bool,
    pub merchant: bool,
    pub none: bool,
    pub nitwit: bool,
    /// The trades its hands offer, each from the level it's offered at (`lockers.rs`): the mod's defaults for the
    /// host's goods by name (`lockers::default_offers`), every one switched off.
    #[serde(default)]
    pub offers: Vec<crate::lockers::Offer>,
}

impl Trade {
    /// A trade with no work of its own: its hands are the town's idle ones.
    pub fn idle(&self) -> bool {
        self.none || self.nitwit
    }
}

/// The mod's sixteen: the fourteen that go with caravans in the squad's order, then `none` and `nitwit`.
pub fn trades() -> Vec<Trade> {
    let mut out: Vec<Trade> = SQUAD
        .iter()
        .map(|&name| Trade { name: name.to_string(), leads: LEADS.contains(&name), squad: true,
            merchant: name == MERCHANT, ..Default::default() })
        .collect();
    out.push(Trade { name: NONE.to_string(), none: true, ..Default::default() });
    out.push(Trade { name: NITWIT.to_string(), nitwit: true, ..Default::default() });
    out
}

/// Total XP a villager needs to leave `level` (1 = Novice); 0 at Master.
pub fn villager_xp_for_next_level(level: u8) -> u32 {
    if (1..=4).contains(&level) { VILLAGER_LEVEL_XP[level as usize - 1] } else { 0 }
}

/// Total Merchant XP needed to leave `level` (1 = Novice); 0 at Master (`LockerTrading.merchantXpForNextLevel`).
pub fn merchant_xp_for_next_level(level: u8) -> u32 {
    if (1..=4).contains(&level) { MERCHANT_LEVEL_XP[level as usize - 1] } else { 0 }
}

/// The XP a hand of `level` has at least: what it took to get there.
pub fn xp_floor(merchant: bool, level: u8) -> u32 {
    match level {
        0 | 1 => 0,
        l if merchant => MERCHANT_LEVEL_XP[(l.min(MAX_LEVEL) - 2) as usize],
        l => VILLAGER_LEVEL_XP[(l.min(MAX_LEVEL) - 2) as usize],
    }
}

/// How many lockers a Merchant of this level manages: 2 at Novice, up to 10 at Master (`Merchant.lockerLimit`).
pub fn locker_limit(level: i32) -> u32 {
    LOCKERS_PER_LEVEL * level.clamp(1, MAX_LEVEL as i32) as u32
}

/// The XP a locker trade gives the Merchant, from a roll the caller draws from the world's generator (any number:
/// it's taken modulo `MERCHANT_TRADE_XP_ROLL`): 3 to 6, plus 5 when the villager levelled up on it.
pub fn merchant_trade_xp(roll: u32, villager_levelled_up: bool) -> u32 {
    MERCHANT_TRADE_XP + roll % MERCHANT_TRADE_XP_ROLL + if villager_levelled_up { MERCHANT_LEVEL_UP_XP } else { 0 }
}

/// One of a town's people: its trade, level (1 to 5) and XP (in all, as vanilla counts it), when it grows up if it's
/// a child, and whether it's away (out with a caravan).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hand {
    pub trade: TradeId,
    pub level: u8,
    pub xp: u32,
    pub baby_until: Option<u64>,
    pub away: bool,
}

impl Default for Hand {
    fn default() -> Self {
        Hand { trade: 0, level: 1, xp: 0, baby_until: None, away: false }
    }
}

impl Hand {
    /// An adult of `trade` at `level` (clamped to 1..=5), with the XP it took to get there.
    pub fn new(content: &Content, trade: TradeId, level: u8) -> Self {
        let level = level.clamp(1, MAX_LEVEL);
        let merchant = content.trades.get(trade).is_some_and(|t| t.merchant);
        Hand { trade, level, xp: xp_floor(merchant, level), ..Default::default() }
    }

    /// A child born at `born`, with no trade (`Growth.bear`): grown up `BABY_TICKS` later.
    pub fn baby(content: &Content, born: u64) -> Self {
        Hand { trade: content.trade(NONE).unwrap_or(0), baby_until: Some(born + BABY_TICKS), ..Default::default() }
    }

    pub fn is_baby(&self) -> bool {
        self.baby_until.is_some()
    }

    /// Its age at `now` as vanilla counts it: below 0 a child (ticks to go), 0 grown.
    pub fn age(&self, now: u64) -> i64 {
        self.baby_until.map_or(0, |until| -(until.saturating_sub(now) as i64))
    }

    /// A child grows up by the time elapsed, loaded or not (`Growth.growUp`). Returns true if it grew up now.
    pub fn grow_up(&mut self, now: u64) -> bool {
        match self.baby_until {
            Some(until) if now >= until => {
                self.baby_until = None;
                true
            }
            _ => false,
        }
    }

    /// Vanilla's trade XP: a villager earns `xp`, and a level it has earned comes at once, one a trade
    /// (`LockerTrading.rewardTradeXp`). Returns true if this trade earned it its next level.
    pub fn reward_trade_xp(&mut self, xp: u32) -> bool {
        self.xp += xp;
        if self.level < MAX_LEVEL && self.xp >= villager_xp_for_next_level(self.level) {
            self.level += 1;
            return true;
        }
        false
    }

    /// The Merchant's XP from a locker trade (see [`merchant_trade_xp`]): it levels at once at a player's
    /// thresholds, one level a trade (`LockerTrading.rewardMerchantXp`). Returns true if it levelled.
    pub fn reward_merchant_xp(&mut self, xp: u32) -> bool {
        self.xp += xp;
        if self.level < MAX_LEVEL && self.xp >= merchant_xp_for_next_level(self.level) {
            self.level += 1;
            return true;
        }
        false
    }
}

/// A town's people, hand by hand, and which of them is the desk's Merchant (a hand that never travels). Every hand is
/// a resident (a child too: it needs a bed); the adults are its workers.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Roster {
    pub hands: Vec<Hand>,
    pub merchant: Option<usize>,
}

impl Roster {
    /// A roster of `hands`; the desk's Merchant is the most experienced Merchant among them, if any.
    pub fn new(content: &Content, hands: Vec<Hand>) -> Self {
        let mut roster = Roster { hands, merchant: None };
        if let Some(trade) = content.trade(MERCHANT) {
            roster.merchant = roster.best(trade);
        }
        roster
    }

    /// Adds a hand; a Merchant becomes the desk's if it has none. Returns its place.
    pub fn add(&mut self, content: &Content, hand: Hand) -> usize {
        let merchant = content.trades.get(hand.trade).is_some_and(|t| t.merchant);
        self.hands.push(hand);
        let i = self.hands.len() - 1;
        if merchant && self.merchant.is_none() {
            self.merchant = Some(i);
        }
        i
    }

    /// Everyone: adults and children, at home or away.
    pub fn residents(&self) -> usize {
        self.hands.len()
    }

    /// The children.
    pub fn babies(&self) -> usize {
        self.hands.iter().filter(|h| h.is_baby()).count()
    }

    /// The adults: the town's workers.
    pub fn workers(&self) -> usize {
        self.residents() - self.babies()
    }

    /// Adults of `trade`.
    pub fn count(&self, trade: TradeId) -> usize {
        self.hands.iter().filter(|h| h.trade == trade && !h.is_baby()).count()
    }

    /// The levels of the adults of `trade`, in the roster's order.
    pub fn levels(&self, trade: TradeId) -> Vec<u8> {
        self.hands.iter().filter(|h| h.trade == trade && !h.is_baby()).map(|h| h.level).collect()
    }

    /// The most experienced adult of `trade` (highest level, then most XP, then first): who a squad takes first.
    pub fn best(&self, trade: TradeId) -> Option<usize> {
        let mut best: Option<usize> = None;
        for (i, h) in self.hands.iter().enumerate() {
            if h.trade != trade || h.is_baby() {
                continue;
            }
            if best.is_none_or(|b| (h.level, h.xp) > (self.hands[b].level, self.hands[b].xp)) {
                best = Some(i);
            }
        }
        best
    }

    /// The idle adults at home (no trade, or Nitwits), Nitwits first, each in the roster's order (`Labour.hands`).
    pub fn idle(&self, content: &Content) -> Vec<usize> {
        let idle = |i: &usize| {
            let h = &self.hands[*i];
            !h.away && !h.is_baby() && content.trades.get(h.trade).is_some_and(|t| t.idle())
        };
        let nitwit = |i: &usize| content.trades[self.hands[*i].trade].nitwit;
        let (mut first, rest): (Vec<usize>, Vec<usize>) = (0..self.hands.len()).filter(idle).partition(nitwit);
        first.extend(rest);
        first
    }

    /// Whether a hand can go with a caravan: an adult, and not the desk's Merchant.
    pub fn travels(&self, i: usize) -> bool {
        self.merchant != Some(i) && self.hands.get(i).is_some_and(|h| !h.is_baby())
    }

    /// The desk's Merchant's level, if it has one.
    pub fn merchant_level(&self) -> Option<u8> {
        self.merchant.and_then(|i| self.hands.get(i)).map(|h| h.level)
    }

    /// How many lockers the desk's Merchant manages: `2 * clamp(level, 1, 5)`; none without a Merchant.
    pub fn locker_limit(&self) -> u32 {
        self.merchant_level().map_or(0, |l| locker_limit(l as i32))
    }

    /// Children grow up by the time elapsed, loaded or not. Returns how many grew up.
    pub fn grow_up(&mut self, now: u64) -> usize {
        self.hands.iter_mut().map(|h| h.grow_up(now)).filter(|g| *g).count()
    }
}
