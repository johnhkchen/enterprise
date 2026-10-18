//! The crown: a sovereign shifting the region onto a war footing through the region's own institutions
//! (`docs/decisions.md` "War footing", "Trust, advances and the capital's port", "Why training broke", "Feeding the
//! front", "Mercenaries, meat, and the front's supply", "The crown's procurement agent: requisitions").
//!
//! Ported from the mod:
//! - `market/Sovereign.java`: `Phase`, the premiums (dials), `TRAINED`, `TRAINING_BONUS`, `OFFTAKE`, `TRAINING_TICKS`,
//!   `FRONT_DISTANCE`, `SOLDIER_PAY`, `MERCENARY_PAY`, `MERCENARIES_MOST`, `LEVY_SHARE`, `LEVY_MOST`, `PLANNED_ARMY`,
//!   `RATION_STORE_DAYS`, `STOCKPILE`/`stockpile` (110–135), `MEAT_POINTS`, `bonuses` (140), `foodReserve` (193),
//!   `establish` (230: [`Society::name_capital`]), `mobilise` (260), `declare` (294), `requisition` (335),
//!   `hireMercenaries` (355), `openFront` (392), `peace` (416), `mint` (435), `targets` (457), `rationDays` (466),
//!   `onServerTick` (476: the daily loop and the front's orders, `rules/crown.rs`), `offerTraining` (590).
//! - `market/Requisitions.java`: `PERIOD_DAYS`, `BID_DAYS`, `MAKE_MARKUP`, `BID_SHARE`, `CEILING_CARRIAGE` (dials),
//!   `Bid`, `Requisition` (`unit`, `ceiling` 102), `open`, `unanswered`, `post` (172), `lapse`, `read` (288), `bid`
//!   (318), `award` (362).
//! - `contract/Bonus.java` `met` (23); `contract/Contract.java` `priceOf` (204, [`crate::Contract::price_of`]);
//!   `contract/Levy.java`; `army/Army.java` `Theatre`.
//! - `caravan/Caravans.java` `raiseLevy` (1662), `hireMercenaries` (1721), `recruitsOwedAt` (2606), `dispatch`'s
//!   recruits to the front (539) and its front runs ("to the front"), `arriveAtDestination`'s recruits enlisted at the
//!   front (657) and its board read (667), `arriveHome`'s muster (1201): `rules/crown.rs`.
//!
//! **The capital and its treasury.** The capital is `realms::Crown`'s (the one capital every slice reads); its
//! treasury is the capital town's money. Each day the crown mints `mint_per_day` spurs into it: coin with nothing
//! behind it, which debases the spur (`Mint::mint`), entering the society from outside (`Reason::Other("minted")`) at
//! the rate before the day's minting. Named in peacetime ([`Society::name_capital`]) it mints its peacetime revenue,
//! opens its port's metal exchange (with the market across the sea) and its bank; mobilising ([`Society::mobilise`])
//! it mints what it's told; at peace ([`Society::peace`]) its revenue again.
//!
//! **The stockpile** ([`stockpile`], [`targets`]): kit for a planned army of 24, the best a third of it, and its food
//! put by (half bread, half meat in food points: [`food_reserve`]); at war a set of kit for each soldier and the front's
//! rations besides (`army::Army::wants`). **Requisitions** ([`Requisition`]): each day the agent posts what the stockpile
//! lacks beyond what's under contract and already requisitioned, per good (meat as one requisition of a kind, in food
//! points), for `period_days` at the crown's price; a town's Merchant reading the board (a wagon between the two) bids
//! what its town can spare and its people make by the day, at its landed cost (a quarter more for what's made) and half
//! the way to the crown's price; after `bid_days` the cheapest offers are awarded, each a contract for its whole lot,
//! with the crown's bonus clauses and a third up front where they trust each other. **Tenders** for masters (and
//! Butchers selling meat) the realm lacks, on the capital's board, a quarter more for each that lapsed untaken.
//!
//! **War** ([`Society::declare`]): the front opens, an off-map market `front_distance` north of the capital with a road
//! from it; each town the capital's wagons reach is levied by contract (a [`Levy`]: its idle hands and a fifth of the
//! rest, one to four), the copy riding with the capital's wagon, which takes the recruits home to its barracks
//! (`rules/crown.rs`). Short of its planned army the crown hires mercenaries at the market across the sea, one contract
//! open at a time. Each day the front's quartermaster orders from the capital what the army will want there (food
//! heartier than bread that the capital holds first, best kit first), carried by the capital's wagons with the waiting
//! recruits; the army campaigns on what reaches the front (`rules/army.rs`).
//!
//! **Where the core differs.** The mod's shops (the gold vendor, the requisitions' buying vendors, their sweep into the
//! depot) are Numismatics blocks: the binding's. A requisition read off a freight hub's board isn't (no hub posts the
//! requisitions of the markets it runs freight to). The front's orders are the crown's own office: drafted with both
//! copies (in force at once) and at no price (the front's account is the treasury's, so what it paid its capital's wagon
//! would come back to it: the core's front holds no money of its own). The bonus clauses ride on the contracts and
//! [`crate::Contract::price_of`] prices a piece by them, but deliveries are paid at the locked price: the core's cargo
//! doesn't carry a piece's enchantments from the seller's `Kits` (the binding pays the bonus for the pieces it sees).
//! Recruits riding with a wagon at peace still muster at the capital (as the mod's); a levy not yet raised is closed at
//! peace (the mod leaves it in force, and the capital's wagons would fetch recruits for no war). The road to the front
//! stays at peace (the mod removes it once its caravan is home), idle with nothing owed there. The capital's ration
//! store isn't yet kept from its own wagons' road food ([`food_reserve`] is for the host and the road food's later
//! slice); and the capital's own stock counts as its depot (the mod's `MerchantStock`).

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::army::{self, Army};
use crate::contracts::{Book, Incoterm, State};
use crate::content::{Content, GoodId};
use crate::messages::TownId;
use crate::realms::{Crown, OffMap};
use crate::roster::{Roster, TradeId};
use crate::society::Society;
use crate::sourcing::{Env, MetalExchange, Phase, Sourcing};
use crate::state::*;

/// Bread's food points, as the stockpile's loaves are counted (`STOCKPILE`'s `/ 5`: `Army.nutrition(BREAD)`).
pub const BREAD_POINTS: u32 = 5;
/// The crown takes the apprentices' work too, at its worth (`Sovereign.OFFTAKE`); a tender is due in so many days
/// (`TRAINING_TICKS`).
pub const OFFTAKE: f64 = 1.0;
pub const TRAINING_DAYS: u64 = 15;
/// Food is sent the front for its heartiness only above this (`onServerTick`'s `heartiness(food) < 0.05`).
pub const HEARTY_ENOUGH: f64 = 0.05;

/// A trade the crown wants in its realm, so many at the level; `meat`: counted by the towns whose villagers offer
/// meat for emeralds (`Sovereign.Trained`, its `sells` the army's meats).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trained {
    pub trade: &'static str,
    pub count: u32,
    pub level: u8,
    pub meat: bool,
}

/// Master smiths (diamond swords and armour are theirs to sell) and Butchers selling meat (`Sovereign.TRAINED`).
pub const TRAINED: [Trained; 4] = [
    Trained { trade: "weaponsmith", count: 2, level: 5, meat: false },
    Trained { trade: "armorer", count: 2, level: 5, meat: false },
    Trained { trade: "toolsmith", count: 1, level: 5, meat: false },
    Trained { trade: "butcher", count: 2, level: 2, meat: true },
];

/// Who the war is against (`Army.Theatre`): it names the enchantments suited to the enemy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theatre {
    #[default]
    Faction,
    Undead,
    Nether,
}

impl Theatre {
    /// The weapon's enchantment suited to the enemy (`weaponEnchant`).
    pub fn weapon_enchant(self) -> Enchant {
        match self {
            Theatre::Faction => Enchant::Sharpness,
            Theatre::Undead => Enchant::Smite,
            Theatre::Nether => Enchant::FireAspect,
        }
    }

    /// The armour's (`armourEnchant`).
    pub fn armour_enchant(self) -> Enchant {
        match self {
            Theatre::Nether => Enchant::FireProtection,
            _ => Enchant::Protection,
        }
    }
}

/// An enchantment a bonus clause can name.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Enchant {
    #[default]
    Sharpness,
    Smite,
    FireAspect,
    Power,
    Protection,
    FireProtection,
    Unbreaking,
}

/// A bonus clause (`contract/Bonus.java`): a piece delivered with each enchantment at least at its level is paid
/// `percent` more. The goods count whatever their enchantments; the bonus only adds to the pay. Clauses add up.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Bonus {
    pub at_least: Vec<(Enchant, u8)>,
    pub percent: f64,
}

impl Bonus {
    pub fn new(at_least: &[(Enchant, u8)], percent: f64) -> Self {
        Bonus { at_least: at_least.to_vec(), percent }
    }

    /// Does a piece with these enchantments meet the clause (`Bonus.met`)? Each named at its level or better.
    pub fn met(&self, piece: &[(Enchant, u8)]) -> bool {
        self.at_least.iter().all(|&(want, level)| {
            piece.iter().filter(|(e, _)| *e == want).map(|(_, l)| *l).next_back().unwrap_or(0) >= level
        })
    }
}

/// A levy (`contract/Levy.java`): the seller sends `soldiers` to the sovereign's army, who serve until peace, paid
/// `pay_per_day` a soldier-day, owed to the town. `raised`: those it has sent (the mod's `delivered`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Levy {
    pub soldiers: u32,
    pub pay_per_day: f64,
    pub raised: u32,
}

impl Levy {
    /// Soldiers it still owes.
    pub fn owed(&self) -> u32 {
        self.soldiers.saturating_sub(self.raised)
    }
}

/// A seller's offer on a requisition (`Requisitions.Bid`): so many of a good at so much each, filed then.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Bid {
    pub seller: TownId,
    pub good: GoodId,
    pub count: f64,
    pub each: f64,
    pub filed: u64,
}

/// A posted want (`Requisitions.Requisition`): its buyer, the goods any of which will do (in food points if
/// `by_points`, else items), how many are wanted and still open, at the phase's premium, posted and due, its bids, and
/// whether its bidding is over.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Requisition {
    pub id: usize,
    pub buyer: TownId,
    pub goods: Vec<GoodId>,
    pub by_points: bool,
    pub wanted: f64,
    pub open: f64,
    pub premium: f64,
    pub posted: u64,
    pub due: u64,
    pub bids: Vec<Bid>,
    pub awarded: bool,
}

impl Requisition {
    /// What one of a good counts toward it: its food points, or one (`unit`).
    pub fn unit(&self, content: &Content, good: GoodId) -> f64 {
        if self.by_points { content.goods[good].food.max(1.0) } else { 1.0 }
    }

    /// The crown's price for one: no bid above it is taken (`ceiling`).
    pub fn ceiling(&self, content: &Content, good: GoodId) -> f64 {
        ceiling(content, good, self.premium)
    }
}

/// The crown's price for a good: its worth, the suppliers' margin and carriage, times the phase's premium
/// (`Requisition.ceiling`).
pub fn ceiling(content: &Content, good: GoodId, premium: f64) -> f64 {
    let d = &content.dials;
    content.goods[good].value * (1.0 + d.supplier_margin) * d.ceiling_carriage * premium
}

/// The sovereign (`Sovereign`, `Requisitions`): its phase and theatre, the spurs it mints a day (and in peacetime), the
/// day it last ran and since when it's been in its phase, its front, the requisitions on its boards, what tenders it
/// raised (lapsed untaken, by trade), what it lacks today and what no town answered, and its record: spurs minted,
/// levies drafted, mercenaries hired, requisitions' contracts awarded.
#[derive(Resource, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Sovereign {
    pub phase: Phase,
    pub theatre: Theatre,
    pub mint_per_day: u64,
    pub peace_revenue: u64,
    pub last_day: Option<u64>,
    pub phase_since: Option<u64>,
    pub front: Option<TownId>,
    pub requisitions: Vec<Requisition>,
    pub next_id: usize,
    pub raised: BTreeMap<TradeId, u32>,
    pub shortfall: Vec<(GoodId, f64)>,
    pub unmet: Vec<(GoodId, f64)>,
    pub minted: f64,
    pub levies: u32,
    pub hired: u32,
    pub awarded: u32,
}

impl Sovereign {
    /// The phase's premium on what it buys: half again mobilising, two and a half at war (1 at peace, for tenders).
    pub fn premium(&self, d: &crate::Dials) -> f64 {
        match self.phase {
            Phase::Peace => 1.0,
            Phase::Mobilising => d.mobilising_premium,
            Phase::War => d.war_premium,
        }
    }

    /// What's still open, in units, of a buyer's requisitions for these goods (`open`).
    pub fn open(&self, buyer: TownId, goods: &[GoodId]) -> f64 {
        self.requisitions.iter().filter(|r| r.buyer == buyer && r.goods == goods).map(|r| r.open).sum()
    }

    /// Has the buyer's want of these goods gone a whole bidding window without an offer (`unanswered`)?
    pub fn unanswered(&self, buyer: TownId, goods: &[GoodId]) -> bool {
        self.requisitions.iter().any(|r| r.buyer == buyer && r.goods == goods && r.awarded && r.open == r.wanted)
    }

    /// Posts a want of `units` of the goods for the coming period, at `premium`; tops up one still taking bids instead
    /// of posting another (`Requisitions.post`). Returns its id.
    #[allow(clippy::too_many_arguments)]
    pub fn post(&mut self, content: &Content, buyer: TownId, goods: &[GoodId], by_points: bool, units: f64, premium: f64,
            now: u64) -> usize {
        if let Some(r) = self.requisitions.iter_mut().find(|r| r.buyer == buyer && r.goods == goods && !r.awarded) {
            r.wanted += units;
            r.open += units;
            return r.id;
        }
        self.next_id += 1;
        let due = now + content.dials.period_days as u64 * content.day_ticks;
        self.requisitions.push(Requisition { id: self.next_id, buyer, goods: goods.to_vec(), by_points, wanted: units,
            open: units, premium, posted: now, due, bids: Vec::new(), awarded: false });
        self.next_id
    }

    /// Requisitions past their day, or met once awarded, close (`lapse`). Returns how many.
    pub fn lapse(&mut self, now: u64) -> usize {
        let before = self.requisitions.len();
        self.requisitions.retain(|r| !(now >= r.due || r.open <= 0.0 && r.awarded));
        before - self.requisitions.len()
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// What the crown wants

/// The stockpile (`STOCKPILE`, `stockpile`): of the host's goods, kit for the planned army, the best a third of it
/// (diamond swords for two thirds, iron for the rest; diamond armour for a third, iron for two thirds; chainmail and
/// leather chestplates, shields for half, bows, arrows), and the bread of its food put by.
pub fn stockpile(content: &Content) -> Vec<(GoodId, u32)> {
    let d = &content.dials;
    let army = d.planned_army;
    let best = army * 2 / 3;
    let mut m: Vec<(&str, u32)> = vec![("diamond_sword", best), ("iron_sword", army - best)];
    for piece in ["diamond_helmet", "diamond_chestplate", "diamond_leggings", "diamond_boots"] {
        m.push((piece, army / 3));
    }
    for piece in ["iron_helmet", "iron_chestplate", "iron_leggings", "iron_boots"] {
        m.push((piece, army * 2 / 3));
    }
    m.extend([("chainmail_chestplate", 6), ("leather_chestplate", 6), ("shield", army / 2), ("bow", 8), ("arrow", 256)]);
    let ration = d.army_ration.round() as u32;
    m.push(("bread", army * ration * d.ration_store_days / 2 / BREAD_POINTS));
    m.into_iter().filter_map(|(name, n)| army::good_named(content, name).map(|g| (g, n))).collect()
}

/// What the crown wants on hand at the capital now (`targets`): the stockpile, and at war what the army will want
/// besides (`Army.wants` for `ration_days`), merged by good in the stockpile's order.
pub fn targets(content: &Content, phase: Phase, army: &Army, ration_days: u32) -> Vec<(GoodId, u32)> {
    let mut out = stockpile(content);
    if phase == Phase::War {
        for (g, n) in army.wants(content, ration_days) {
            match out.iter_mut().find(|(o, _)| *o == g) {
                Some((_, m)) => *m += n,
                None => out.push((g, n)),
            }
        }
    }
    out
}

/// Food points the crown keeps at its capital for the army while mobilising and at war, its ration store
/// (`foodReserve`): the stockpile's bread and its meat; none elsewhere, and none at peace.
pub fn food_reserve(content: &Content, phase: Phase, capital: Option<TownId>, town: TownId) -> f64 {
    if capital != Some(town) || phase == Phase::Peace {
        return 0.0;
    }
    let bread = army::good_named(content, army::BREAD);
    let loaves = bread.and_then(|b| stockpile(content).into_iter().find(|(g, _)| *g == b)).map_or(0, |(_, n)| n);
    loaves as f64 * bread.map_or(0.0, |b| content.goods[b].food) + content.dials.meat_points
}

/// The army's meats among the host's goods: any meat is hearty (`Army.MEATS`).
pub fn meats(content: &Content) -> Vec<GoodId> {
    (0..content.goods.len()).filter(|&g| content.goods[g].meat && content.goods[g].food > 0.0).collect()
}

/// The bonus clauses on the crown's contracts for a good (`bonuses`): quality is paid for, never required. A sword:
/// Unbreaking 1 and Sharpness 2, 25% (and against the undead Smite 3, 30%); armour: the theatre's protection 2, 25%,
/// and Unbreaking 2, 15%; a bow: Power 2, 25%, Unbreaking 1, 10%; a shield: Unbreaking 1, 10%.
pub fn bonuses(content: &Content, good: GoodId, theatre: Theatre) -> Vec<Bonus> {
    use Enchant::*;
    let name = army::bare(&content.goods[good].name);
    let mut terms = Vec::new();
    if name.ends_with("_sword") {
        terms.push(Bonus::new(&[(Unbreaking, 1), (Sharpness, 2)], 25.0));
        if theatre == Theatre::Undead {
            terms.push(Bonus::new(&[(Smite, 3)], 30.0));
        }
    } else if ["_helmet", "_chestplate", "_leggings", "_boots"].iter().any(|p| name.ends_with(p)) {
        terms.push(Bonus::new(&[(theatre.armour_enchant(), 2)], 25.0));
        terms.push(Bonus::new(&[(Unbreaking, 2)], 15.0));
    } else if name == army::BOW {
        terms.push(Bonus::new(&[(Power, 2)], 25.0));
        terms.push(Bonus::new(&[(Unbreaking, 1)], 10.0));
    } else if name == army::SHIELD.name {
        terms.push(Bonus::new(&[(Unbreaking, 1)], 10.0));
    }
    terms
}

/// A piece's enchantments as the core's kit holds them (`kit::Kit`): its levels of the enchantment suited to the enemy
/// (a bow's Power, a weapon's the theatre's, armour's the theatre's protection) and its Unbreaking.
pub fn enchantments(content: &Content, good: GoodId, enchant_level: f64, unbreaking: u8, theatre: Theatre)
        -> Vec<(Enchant, u8)> {
    let name = army::bare(&content.goods[good].name);
    let suited = if name == army::BOW {
        Some(Enchant::Power)
    } else if army::weapon(name).is_some() {
        Some(theatre.weapon_enchant())
    } else if army::armour(name).is_some() {
        Some(theatre.armour_enchant())
    } else {
        None
    };
    let mut out = Vec::new();
    if let Some(e) = suited.filter(|_| enchant_level > 0.0) {
        out.push((e, enchant_level.round().min(255.0) as u8));
    }
    if unbreaking > 0 {
        out.push((Enchant::Unbreaking, unbreaking));
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------------
// Levies

/// How many a town's levy asks (`declare`): its idle hands and `levy_share` of the rest, at least one and at most
/// `levy_most` (the mod's barracks beds add to it: the core's towns have none).
pub fn levy_size(content: &Content, roster: &Roster) -> u32 {
    let d = &content.dials;
    let grown: Vec<_> = roster.hands.iter().filter(|h| !h.is_baby()).collect();
    let idle = grown.iter().filter(|h| content.trades.get(h.trade).is_some_and(|t| t.idle())).count() as f64;
    let rest = grown.len() as f64 - idle;
    let most = d.levy_most as f64;
    (idle + (rest * d.levy_share + 0.5).floor()).min(most).max(1.0) as u32
}

/// The trades a levied town keeps last (`raiseLevy`'s `needed`): those its training contracts are for, those that sell
/// or make a good it delivers under a contract in force (`Apprentices.neededTrades`, `Procurement.suppliers`: every
/// seller and every maker), and the ones the crown wants trained (`Sovereign.TRAINED`).
pub fn needed_trades(content: &Content, book: &Book, town: TownId) -> Vec<TradeId> {
    let mut needed = trained_trades(content);
    needed.extend(book.training_for(town).map(|(_, t)| t.trade));
    let recipes = crate::services::recipes(content);
    for c in book.open.iter().map(|&id| &book.contracts[id]) {
        if !c.in_force() || c.seller != town || c.training.is_some() || c.levy.is_some() || c.good >= content.goods.len() {
            continue;
        }
        needed.extend((0..content.trades.len()).filter(|&t| content.trades[t].offers.iter().any(|o| o.sells && o.good == c.good)));
        needed.extend(recipes.iter().filter(|r| r.output == c.good).map(|r| r.trade));
    }
    needed.sort_unstable();
    needed.dedup();
    needed
}

/// Who a town sends of `n` (`Caravans.raiseLevy`): its grown hands at home but the Merchant at its desk, idle hands
/// first, then those of a trade it has others of, then whoever is least missed, and last the only one of a trade its
/// contracts need or the crown wants trained; the least skilled first of each, equals in the roster's order.
pub fn levy_pick(content: &Content, roster: &Roster, needed: &[TradeId], n: usize) -> Vec<usize> {
    let at_home: Vec<usize> = (0..roster.hands.len())
        .filter(|&i| !roster.hands[i].is_baby() && !roster.hands[i].away)
        .collect();
    let of_trade = |t| at_home.iter().filter(|&&i| roster.hands[i].trade == t).count();
    let mut able: Vec<usize> = at_home.iter().copied().filter(|&i| roster.merchant != Some(i)).collect();
    able.sort_by_key(|&i| {
        let h = &roster.hands[i];
        let rank = if content.trades.get(h.trade).is_some_and(|t| t.idle()) {
            0
        } else if of_trade(h.trade) > 1 {
            1
        } else if needed.contains(&h.trade) {
            3
        } else {
            2
        };
        (rank, h.level)
    });
    able.truncate(n);
    able
}

/// The trades the crown wants trained, as the host has them.
pub fn trained_trades(content: &Content) -> Vec<TradeId> {
    TRAINED.iter().filter_map(|t| content.trade(t.trade)).collect()
}

/// Drafts a levy at the buyer's (`book.draft(.., List.of(new Levy(..)), EXW, now, a day, 1, now, buyer, ..)`): a
/// contract for soldiers, no goods. Returns its id.
pub fn draft_levy(book: &mut Book, seller: TownId, buyer: TownId, soldiers: u32, pay: f64, now: u64, day: u64, both: bool)
        -> usize {
    let id = book.draft(seller, buyer, 0, 0.0, 0.0, now, day, 1, Incoterm::Exw, false, both);
    // (Its `drafted` stays unset: a levy has no installment before its first to date, and sourcing reads a drafted day as
    // a lot of goods it sourced.)
    let _ = now;
    book.contracts[id].levy = Some(Levy { soldiers, pay_per_day: pay, raised: 0 });
    id
}

// ---------------------------------------------------------------------------------------------------------------------
// Bids and awards

/// What a town's people offer of a good a restock: its hands' selling offers at their levels, so many a trade, so many
/// trades (`Warehouse.offered`).
pub fn offered(world: &World, town: Entity, good: GoodId) -> f64 {
    let (Some(roster), Some(lockers)) = (world.get::<Roster>(town), world.get::<crate::lockers::Lockers>(town)) else {
        return 0.0;
    };
    if lockers.hands.len() != roster.hands.len() {
        return 0.0;
    }
    roster.hands.iter().zip(&lockers.hands).filter(|(h, _)| !h.is_baby())
        .flat_map(|(h, l)| l.offers.iter().filter(move |o| o.sells && o.good == good && o.level <= h.level))
        .map(|o| o.count as f64 * o.uses as f64).sum()
}

/// What `seller`'s Merchant offers on a requisition (`Requisitions.bid`): of the goods its town can supply most of,
/// what it can spare and what its people make before the day (less the road), at its landed cost (a quarter more for
/// what's made) and half the way to the crown's price; none if it can supply none, or not under the crown's price.
pub fn bid(env: &Env, seller: TownId, r: &Requisition) -> Option<Bid> {
    let content = env.content;
    let d = &content.dials;
    let day = content.day_ticks.max(1) as f64;
    let mut best: Option<Bid> = None;
    for &good in &r.goods {
        let Some(m) = env.means(r.buyer, seller, good) else { continue };
        let road = env.routes[m.route].route.travel_ticks as f64 / day;
        let days = ((r.due as f64 - env.now as f64) / day - road - 1.0).max(0.0);
        let per_day = if m.makes {
            offered(env.world, env.towns[seller], good).max((env.maker_lot(good) / 2.0).floor())
        } else {
            0.0
        };
        let made = (per_day * days).floor();
        let unit = r.unit(content, good);
        let mut count = (r.open / unit).ceil().min((m.spare + made).floor());
        if !m.trusted {
            count = count.min(d.trial_lot as f64); // a trial lot, to rebuild the record
        }
        if count <= 0.0 {
            continue;
        }
        let made_share = (count - m.spare).max(0.0) / count;
        let cost = env.landed(good, m.route, m.incoterm, 1.0) * (1.0 + d.make_markup * made_share);
        let ceiling = r.ceiling(content, good);
        if cost > ceiling {
            continue;
        }
        let each = cost + d.bid_share * (ceiling - cost);
        let bid = Bid { seller, good, count, each, filed: env.now };
        if best.as_ref().is_none_or(|b| bid.count * unit > b.count * r.unit(content, b.good)) {
            best = Some(bid);
        }
    }
    best
}

/// `reader`'s Merchant reads `board`'s requisitions and bids on those its town can supply, once each (`read`). Returns
/// the bids filed.
pub fn read(env: &Env, requisitions: &mut [Requisition], reader: TownId, board: TownId) -> u32 {
    let mut filed = 0;
    for r in requisitions.iter_mut().filter(|r| r.open > 0.0 && r.buyer == board) {
        if r.awarded || r.buyer == reader || r.bids.iter().any(|b| b.seller == reader) {
            continue;
        }
        if let Some(b) = bid(env, reader, r) {
            r.bids.push(b);
            filed += 1;
        }
    }
    filed
}

/// A contract an award drafts: the lot, at the bid's price, due on the requisition's day, on the way's terms, with the
/// crown's bonus clauses, a third up front where they trust each other.
#[derive(Clone, Debug, PartialEq)]
pub struct Award {
    pub seller: TownId,
    pub buyer: TownId,
    pub good: GoodId,
    pub count: f64,
    pub each: f64,
    pub due: u64,
    pub incoterm: Incoterm,
    pub bonuses: Vec<Bonus>,
    pub advance: bool,
}

/// The bidding over on `buyer`'s requisitions (`award`): the agent takes the best offers, cheapest a unit first (the
/// more trusted first at a price), each a contract for its lot (as much as is still wanted). Returns the contracts to
/// draft; the requisitions' open counts are taken down already.
pub fn award(env: &Env, requisitions: &mut [Requisition], buyer: TownId, theatre: Theatre) -> Vec<Award> {
    let content = env.content;
    let day = content.day_ticks.max(1);
    let mut out = Vec::new();
    for r in requisitions.iter_mut() {
        if r.buyer != buyer || r.awarded || env.now < r.posted + content.dials.bid_days as u64 * day {
            continue;
        }
        r.awarded = true;
        let mut offers = r.bids.clone();
        let trust = |b: &Bid| env.book.trust(b.seller, buyer, env.now, day);
        offers.sort_by(|a, b| (a.each / r.unit(content, a.good)).total_cmp(&(b.each / r.unit(content, b.good)))
            .then(trust(b).total_cmp(&trust(a))));
        for b in offers {
            let unit = r.unit(content, b.good);
            let n = b.count.min((r.open / unit).ceil());
            let Some(m) = env.means(buyer, b.seller, b.good) else { continue };
            if n <= 0.0 {
                continue;
            }
            out.push(Award { seller: b.seller, buyer, good: b.good, count: n, each: b.each, due: r.due,
                incoterm: m.incoterm, bonuses: bonuses(content, b.good, theatre), advance: m.trusted });
            r.open = (r.open - n * unit).max(0.0);
        }
    }
    out
}

/// Drafts the awards at the buyer's, their copies riding with the wagons between the two (`Sourcing::copies`).
pub fn apply_awards(world: &mut World, awards: &[Award]) -> Vec<usize> {
    let now = world.resource::<Clock>().now;
    let (day, share) = {
        let c = world.resource::<Content>();
        (c.day_ticks.max(1), c.dials.advance_share)
    };
    let mut ids = Vec::new();
    for a in awards {
        let mut book = world.resource_mut::<Book>();
        let id = book.draft_with_advance(a.seller, a.buyer, a.good, a.count, a.each, a.due, day, 1, a.incoterm, false,
            false, if a.advance { share } else { 0.0 });
        book.contracts[id].bonuses = a.bonuses.clone();
        book.contracts[id].drafted = now;
        world.resource_mut::<Sourcing>().copies.insert(id);
        ids.push(id);
    }
    ids
}

// ---------------------------------------------------------------------------------------------------------------------
// The tally

/// What a town gave the war (`/enterprise war tally`): soldiers raised, fallen and deserted, soldier-days (and hungry
/// ones), strength, the pay its soldiers earned, the kit it delivered the crown and the emeralds it was paid for it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TownTally {
    pub town: TownId,
    pub raised: u32,
    pub fallen: u32,
    pub deserted: u32,
    pub days: u32,
    pub hungry: u32,
    pub strength: f64,
    pub pay: crate::fixed::Fixed,
    pub kit: f64,
    pub earned: crate::fixed::Fixed,
}

/// The war in all: each town's part; the kit issued (by kind and whether enchanted), food points eaten and how hearty;
/// coin minted and the spur's rate; levies, mercenaries hired and the requisitions' contracts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tally {
    pub towns: Vec<TownTally>,
    pub issued: BTreeMap<String, u32>,
    pub eaten: f64,
    pub heartiness: f64,
    pub minted: f64,
    pub rate: f64,
    pub levies: u32,
    pub hired: u32,
    pub awarded: u32,
}

/// Is the good kit the army wears or shoots?
pub fn is_kit(content: &Content, good: GoodId) -> bool {
    let name = army::bare(&content.goods[good].name);
    army::weapon(name).is_some() || army::armour(name).is_some() || name == army::SHIELD.name || name == army::ARROW
}

// ---------------------------------------------------------------------------------------------------------------------
// Host calls

impl Society {
    /// The sovereign as it stands.
    pub fn sovereign(&self) -> &Sovereign {
        self.world.resource::<Sovereign>()
    }

    /// The capital, if one's named.
    pub fn capital(&self) -> Option<TownId> {
        self.world.resource::<Crown>().capital.filter(|&c| c < self.towns.len())
    }

    fn set_phase_all(&mut self, phase: Phase) {
        let now = self.now();
        let mut sov = self.world.resource_mut::<Sovereign>();
        sov.phase = phase;
        sov.phase_since = Some(now);
        self.world.resource_mut::<MetalExchange>().phase = phase;
    }

    /// Names the capital in peacetime (`Sovereign.establish`): its treasury takes `spurs_per_day` a day (the crown's
    /// revenue, minted for now; today's is due), its port's metal exchange opens (with the market across the sea, if
    /// the capital is placed and there's none yet) and its bank.
    pub fn name_capital(&mut self, town: TownId, spurs_per_day: u64) {
        let now = self.now();
        self.world.resource_mut::<Crown>().capital = Some(town);
        {
            let mut sov = self.world.resource_mut::<Sovereign>();
            sov.peace_revenue = spurs_per_day;
            if sov.phase == Phase::Peace {
                sov.mint_per_day = spurs_per_day;
                sov.phase_since = Some(now);
            }
            sov.last_day = None; // today's revenue is due
        }
        let ex = self.world.resource::<MetalExchange>().clone();
        if ex.desk != Some(town) {
            if ex.overseas.is_none() && self.town_hex(town).is_some() {
                self.open_exchange_overseas(town, 0.0);
            } else {
                self.open_exchange(town);
            }
        }
        self.world.resource_mut::<crate::bank::Banks>().open(town);
    }

    /// Mobilisation (`Sovereign.mobilise`): peace first (any war ended), then the capital mints `spurs_per_day` a day
    /// into its treasury, today's at once, and stockpiles. Returns the shops it puts up (the binding's gold shop: 0).
    pub fn mobilise(&mut self, town: TownId, spurs_per_day: u64) -> usize {
        self.peace();
        let now = self.now();
        self.world.resource_mut::<Crown>().capital = Some(town);
        self.set_phase_all(Phase::Mobilising);
        {
            let mut sov = self.world.resource_mut::<Sovereign>();
            sov.mint_per_day = spurs_per_day;
            sov.last_day = Some(now);
        }
        crate::rules::crown::mint(&mut self.world);
        0
    }

    /// War (`Sovereign.declare`): the front opens and each town the capital's wagons reach is levied by contract, its
    /// copy riding with them. Returns the levies drafted (none with no capital).
    pub fn declare(&mut self, theatre: Theatre) -> usize {
        let Some(capital) = self.capital() else { return 0 };
        self.set_phase_all(Phase::War);
        self.world.resource_mut::<Sovereign>().theatre = theatre;
        self.open_front(capital);
        let now = self.now();
        let content = self.content().clone();
        let mut drafted = Vec::new();
        let mut routes: Vec<Route> = self.routes.iter().filter_map(|&r| self.world.get::<Route>(r).copied()).collect();
        routes.sort_by_key(|r| r.seq);
        for route in routes.iter().filter(|r| r.from_id == capital) {
            let e = self.towns[route.to_id];
            if self.world.get::<OffMap>(e).is_some() {
                continue;
            }
            let Some(roster) = self.world.get::<Roster>(e) else { continue };
            drafted.push((route.to_id, levy_size(&content, roster)));
        }
        let pay = content.dials.soldier_pay;
        for &(town, soldiers) in &drafted {
            let id = draft_levy(&mut self.world.resource_mut::<Book>(), town, capital, soldiers, pay, now,
                content.day_ticks.max(1), false);
            self.world.resource_mut::<Sourcing>().copies.insert(id);
        }
        self.world.resource_mut::<Sovereign>().levies += drafted.len() as u32;
        drafted.len()
    }

    /// The front (`Sovereign.openFront`): an off-map market `front_distance` blocks north of the capital that keeps all
    /// it gets for the army, on a road from the capital, each knowing the other; the army's front. The same market
    /// each war.
    fn open_front(&mut self, capital: TownId) {
        let front = match self.world.resource::<Sovereign>().front.filter(|&f| f < self.towns.len()) {
            Some(f) => f,
            None => {
                let (x, z) = self.town_hex(capital).map_or((0.0, 0.0), |h| (h.center_x(), h.center_z()));
                let distance = self.content().dials.front_distance;
                let f = self.add_off_map("the front", x, z - distance, 0.0);
                self.world.resource_mut::<Sovereign>().front = Some(f);
                f
            }
        };
        let has_road = self.routes.iter().filter_map(|&r| self.world.get::<Route>(r))
            .any(|r| r.from_id == capital && r.to_id == front);
        if !has_road {
            let blocks = self.places().distance(capital, front).map_or(self.content().dials.front_distance, |b| b as f64);
            let days = blocks.max(1.0) / crate::map::BLOCKS_PER_DAY;
            let every = self.content().dials.wagon_wait_days;
            let cost = self.per_hex() * (blocks / crate::hex::ACROSS).ceil().max(1.0);
            self.add_route(capital, front, days, cost, every);
        }
        let now = self.now();
        self.world.resource_mut::<crate::atlas::Atlas>().learn_both(capital, front, now);
        self.world.resource_mut::<Army>().front = Some(front);
    }

    /// Peace (`Sovereign.peace`): the army walks home, the front closes to the army, the minting falls back to the
    /// crown's revenue, and every levy still drafted or in force is closed, raised or not (the mod leaves them in force;
    /// pay still owed survives, `Book::tidy` keeps a contract owing). Returns the soldiers and recruits sent home.
    pub fn peace(&mut self) -> usize {
        let home = if self.capital().is_some() { self.demobilise() } else { 0 };
        self.world.resource_mut::<Army>().front = None;
        self.set_phase_all(Phase::Peace);
        {
            let mut sov = self.world.resource_mut::<Sovereign>();
            sov.mint_per_day = sov.peace_revenue;
            sov.unmet.clear();
            sov.shortfall.clear();
        }
        let mut book = self.world.resource_mut::<Book>();
        let levies: Vec<usize> = book.open.iter().copied()
            .filter(|&id| book.contracts[id].levy.is_some() && matches!(book.contracts[id].state, State::Drafted | State::Active))
            .collect();
        for id in levies {
            book.close(id);
        }
        home
    }

    /// What the crown wants on hand at the capital now (`Sovereign.targets`).
    pub fn targets(&self) -> Vec<(GoodId, u32)> {
        let sov = self.sovereign();
        targets(self.content(), sov.phase, self.army(), self.front_ration_days())
    }

    /// Days of rations the front keeps, by its distance from the capital (`Sovereign.rationDays`).
    pub fn front_ration_days(&self) -> u32 {
        let army = self.army();
        let blocks = match (army.front, self.capital()) {
            (Some(f), Some(c)) => army::blocks_between(self.town_hex(c), self.town_hex(f)),
            _ => 0.0,
        };
        army::ration_days(self.content(), blocks)
    }

    /// Food points the crown keeps at a town for the army (`Sovereign.foodReserve`): its ration store, at the capital
    /// while mobilising or at war.
    pub fn food_reserve(&self, town: TownId) -> f64 {
        food_reserve(self.content(), self.sovereign().phase, self.capital(), town)
    }

    /// The crown's bonus clauses for a good, in this war's theatre (`Sovereign.bonuses`).
    pub fn bonuses(&self, good: GoodId) -> Vec<Bonus> {
        bonuses(self.content(), good, self.sovereign().theatre)
    }

    /// Posts a requisition on `buyer`'s board (`Requisitions.post`). Returns its id.
    pub fn post_requisition(&mut self, buyer: TownId, goods: &[GoodId], by_points: bool, units: f64, premium: f64) -> usize {
        let now = self.now();
        let content = self.content().clone();
        self.world.resource_mut::<Sovereign>().post(&content, buyer, goods, by_points, units, premium, now)
    }

    /// The requisitions posted, open or not.
    pub fn requisitions(&self) -> Vec<Requisition> {
        self.sovereign().requisitions.clone()
    }

    /// `reader`'s Merchant reads `board`'s requisitions and bids, as a wagon between them lets it (`Requisitions.read`).
    /// Returns the bids filed.
    pub fn read_requisitions(&mut self, reader: TownId, board: TownId) -> u32 {
        let mut reqs = std::mem::take(&mut self.world.resource_mut::<Sovereign>().requisitions);
        let routes = crate::sourcing::route_facts(&mut self.world);
        let filed = read(&Env::gather(&self.world, routes), &mut reqs, reader, board);
        self.world.resource_mut::<Sovereign>().requisitions = reqs;
        filed
    }

    /// The bidding over on `buyer`'s requisitions taking bids `bid_days` or more: the best offers awarded
    /// (`Requisitions.award`). Returns the contracts drafted.
    pub fn award_requisitions(&mut self, buyer: TownId) -> Vec<usize> {
        let theatre = self.sovereign().theatre;
        let mut reqs = std::mem::take(&mut self.world.resource_mut::<Sovereign>().requisitions);
        let routes = crate::sourcing::route_facts(&mut self.world);
        let awards = award(&Env::gather(&self.world, routes), &mut reqs, buyer, theatre);
        let mut sov = self.world.resource_mut::<Sovereign>();
        sov.requisitions = reqs;
        sov.awarded += awards.len() as u32;
        apply_awards(&mut self.world, &awards)
    }

    /// A levy drafted at the buyer's (`both`: both hold copies, in force at once): `soldiers` from the seller, at `pay`
    /// a soldier-day. Its copy rides with the wagons between the two. Returns its id.
    pub fn draft_levy(&mut self, seller: TownId, buyer: TownId, soldiers: u32, pay: f64, both: bool) -> usize {
        let (now, day) = (self.now(), self.content().day_ticks.max(1));
        let id = draft_levy(&mut self.world.resource_mut::<Book>(), seller, buyer, soldiers, pay, now, day, both);
        if !both {
            self.world.resource_mut::<Sourcing>().copies.insert(id);
        }
        id
    }

    /// At war, short of its planned army, the crown hires mercenaries at the market across the sea
    /// (`Sovereign.hireMercenaries`). Returns those contracted.
    pub fn hire_mercenaries(&mut self) -> u32 {
        crate::rules::crown::hire(&mut self.world)
    }

    /// The recruits riding with the wagons, route by route (`CaravanLedger.recruitsOnTheRoad`).
    pub fn recruits_on_the_road(&self) -> Vec<crate::army::Recruit> {
        self.routes.iter().filter_map(|&r| self.world.get::<RouteState>(r))
            .filter_map(|s| s.wagon.as_ref()).flat_map(|w| w.recruits.iter().cloned()).collect()
    }

    /// The war's tally (`/enterprise war tally`).
    pub fn war_tally(&self) -> Tally {
        let content = self.content();
        let army = self.army();
        let sov = self.sovereign();
        let book = self.world.resource::<Book>();
        let capital = self.capital();
        let crown = |t: TownId| Some(t) == capital || Some(t) == sov.front;
        let mut towns = Vec::new();
        for t in 0..self.town_count() {
            let r = army.record(t);
            let mut kit = 0.0;
            let mut earned = crate::fixed::Fixed::ZERO;
            for c in book.contracts.iter().filter(|c| c.seller == t && crown(c.buyer)) {
                if c.levy.is_none() && c.training.is_none() && is_kit(content, c.good) {
                    kit += c.delivered;
                }
                earned += c.paid;
            }
            if r.raised > 0 || kit > 0.0 || earned.is_positive() {
                towns.push(TownTally { town: t, raised: r.raised, fallen: r.fallen, deserted: r.deserted, days: r.days,
                    hungry: r.hungry, strength: r.strength, pay: r.pay, kit, earned });
            }
        }
        Tally { towns, issued: army.issued.clone(), eaten: army.eaten, heartiness: army.heartiness(), minted: sov.minted,
            rate: self.mint_rate(), levies: sov.levies, hired: sov.hired, awarded: sov.awarded }
    }
}
