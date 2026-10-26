//! Farms, as the mod keeps them (`territory/Territory.java`: `plan`, `usesOf`, `plots`, `rotate`, `accrue`,
//! `wagePerDay`, `valuePerDay`, `payWages`, `touch` (delivery and share-cropping), `bank`, `loose`, `wantsStore`,
//! `storeFor`, `buyLand`, `depositsOnCall`, `borrowDeposits`, `settleDeposits`, `yieldFactor`, `cultureFactor`,
//! `recordCropSale`, `afterCensus`; `territory/Households.java`: `tightness`, `share`; `territory/Fields.java`:
//! `choose`, `allowed`, `wanted`, `consider`, `freehold`, `plant`; `territory/Focus.java`: `of`;
//! `territory/Buildings.java`: `STORES`), and `docs/decisions.md` ("Territory, farms and tolls", "Wages stay in the
//! world", "Sharecroppers", "The Merchant tells its Farmers what sells", "Towns grow before they trade").
//!
//! A town's Farmers tend [`PLOTS_PER_FARMER_LEVEL`] plots a level each; a tended plot yields
//! [`YIELD_PER_PLOT_PER_DAY`] a day, more or less with the town's mood ([`yield_factor`]). The plots a town has of
//! each crop are the host's (fields in the ground, [`Farm::plots`]) and the books' (fields turned over to another
//! crop, [`Farm::turned`]); which its Farmers work is [`plan`]: the herds' feed and what's sold forward first, then
//! each use of a crop where an item earns most, then the rest. Produce accrues on touch and lands in the warehouse as
//! whole items (`Ledger.made`), or in its store where it has one (a granary, a root cellar) beyond what the warehouse
//! keeps loose.
//!
//! The Farmers are paid a share of what the harvest is worth, from the treasury into their households' purse (money
//! in new hands, which comes back for comforts). The share follows the labour market: toward the share of the harvest
//! that sells, times how scarce hands are. Short of money the town pays in credit, up to [`CREDIT_DAYS`] days of a
//! customary wage; beyond that the Farmers take their wage in kind, as sharecroppers: their share of the crop is
//! theirs, the town gets the rest. The households' savings are on call to the treasury ([`Deposits`]): it borrows up
//! to its share of them, with interest, and repays beyond its reserve.
//!
//! The core's looks are short (twenty a day) where the mod touches a market every few days, so: wages are settled at
//! every look where a whole emerald is owed, and the share paid at the last settlement stands until the next (the mod
//! reads 1 at a touch with nothing whole owed); share-cropping takes its share of each look's crop by it.

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::content::{Content, GoodId};
use crate::fixed::Fixed;

/// Plots a Farmer tends a profession level (`Territory.PLOTS_PER_FARMER_LEVEL`).
pub const PLOTS_PER_FARMER_LEVEL: u32 = 32;
/// Items a worked plot yields a day (`Territory.YIELD_PER_PLOT_PER_DAY`).
pub const YIELD_PER_PLOT_PER_DAY: f64 = 0.5;
/// The Farmers' starting share of what they grow is worth, and the customary wage credit is reckoned at
/// (`Territory.LABOR_SHARE`).
pub const LABOR_SHARE: f64 = 0.5;
/// The share floats between these (`Territory.MIN_LABOR_SHARE`, `MAX_LABOR_SHARE`); the mod's floor, a quarter, is the
/// dial `min_labor_share`'s, now none: the Farmers are paid on what sells (John: "farm wages only on what sells").
pub const MIN_LABOR_SHARE: f64 = 0.25;
pub const MAX_LABOR_SHARE: f64 = 1.0;
/// Days for the share to close half its gap, and for crop sales and the harvest's worth to be averaged over.
pub const SHARE_HALF_LIFE_DAYS: f64 = 2.0;
pub const SALES_HALF_LIFE_DAYS: f64 = 3.0;
/// At most this many days of produce accrue between touches (`Territory.MAX_DAYS`).
pub const MAX_DAYS: f64 = 60.0;
/// Days of wages the town keeps back for its Farmers (`Territory.WAGE_FLOAT_DAYS`).
pub const WAGE_FLOAT_DAYS: f64 = 3.0;
/// Days of a customary wage Farmers will wait for as credit (`Households.CREDIT_DAYS`).
pub const CREDIT_DAYS: f64 = 10.0;
/// What a field of its own costs a household (`Households.LAND_EMERALDS`).
pub const LAND_EMERALDS: f64 = 8.0;
/// On freehold plots the Farmers take at least this share (`Households.FREEHOLD_SHARE`).
pub const FREEHOLD_SHARE: f64 = 0.75;
/// Hands scarce raise the wage up to this factor, hands idle lower it this far (`Households.TIGHTEST`, `SLACKEST`).
pub const TIGHTEST: f64 = 1.5;
pub const SLACKEST: f64 = 0.5;
/// A field is this many blocks square (`Fields.SIZE`).
pub const FIELD_SIZE: u32 = 9;
/// A field's seed and labour (`Fields.FIELD_EMERALDS`).
pub const FIELD_EMERALDS: f64 = 4.0;
/// Farmers with room for this many more plots get a field (`Fields.SPARE_PLOTS`).
pub const SPARE_PLOTS: u32 = 54;
/// Food points a warehouse aims to hold for its caravans (`Warehouse.FOOD_PAR_POINTS`): twice it, a town is fed.
pub const FOOD_PAR_POINTS: f64 = 320.0;
/// Villagers a town grows to before it farms for trade (`Focus.GROWN`).
pub const GROWN: f64 = 8.0;
/// Fields go where the town has room: within [`FARTHEST`] blocks of its bell and no nearer than [`NEAREST`]
/// (`Growth.farmSite`, a field 8 out, `Growth.FARTHEST`). The mod searches the blocks for a site; the core counts the
/// ring's area, [`farmland`], and plants no field beyond it.
pub const FARTHEST: f64 = 32.0;
pub const NEAREST: f64 = 8.0;

/// The blocks of ground a town's fields can take: the ring between [`NEAREST`] and [`FARTHEST`] from its bell.
pub fn farmland() -> f64 {
    std::f64::consts::PI * (FARTHEST * FARTHEST - NEAREST * NEAREST)
}

/// Is there room for another field: its 81 blocks beside the plots in the ground?
pub fn room_for_a_field(farm: &Farm) -> bool {
    farm.plots.values().map(|n| *n as f64).sum::<f64>() + (FIELD_SIZE * FIELD_SIZE) as f64 <= farmland()
}

/// What the warehouse keeps loose of a crop beside its store, at the least: two stacks (`Territory.loose`).
pub const LOOSE_AT_LEAST: f64 = 128.0;

/// The crops a field can be planted with (`Fields.CROPS`), by the host's names.
pub const CROPS: [&str; 6] = ["wheat", "carrot", "potato", "beetroot", "sugar_cane", "sweet_berries"];
/// Field crops that share farmland, so a field can be turned over from one to another (`Territory.ROTATION`).
pub const ROTATION: [&str; 4] = ["wheat", "carrot", "potato", "beetroot"];
/// What a crop sells as at a locker, where not as itself (`Fields.SOLD_AS`): cane is pressed into paper first.
pub const SOLD_AS: [(&str, &str); 1] = [("sugar_cane", "paper")];
/// What a trade makes of a crop alone (`Workshop.CONVERSIONS` with one input): so many of the crop into so many of
/// the good.
pub const CONVERSIONS: [(&str, f64, &str, f64); 2] = [("sugar_cane", 1.0, "paper", 1.0), ("wheat", 3.0, "bread", 1.0)];

/// Farm output for a mood: a tenth more a point, between half and one and a half (`Territory.yieldFactor`).
pub fn yield_factor(mood: f64) -> f64 {
    (1.0 + 0.1 * mood).clamp(0.5, 1.5)
}

/// Culture for a mood (`Territory.cultureFactor`): territory's, where culture is counted.
pub use crate::territory::culture_factor;

/// How scarce farm hands are, as a factor on the wage (`Households.tightness`): above 1 with plots going untended (to
/// [`TIGHTEST`] when as many go untended as are worked), below 1 with hands idle (to [`SLACKEST`] when half are).
pub fn tightness(capacity: f64, tended: f64, untended: f64) -> f64 {
    if capacity <= 0.0 {
        return 1.0;
    }
    if untended > 0.0 {
        return TIGHTEST.min(1.0 + untended / capacity);
    }
    SLACKEST.max((tended / capacity).min(1.0))
}

/// The Farmers' share on the estate's plots and their own, by the share of plots that are theirs (`Households.share`).
pub fn share(labor_share: f64, freehold_share: f64) -> f64 {
    let f = freehold_share.clamp(0.0, 1.0);
    (1.0 - f) * labor_share + f * labor_share.max(FREEHOLD_SHARE)
}

/// What a town puts first, from how it stands (`Focus`): growing (short of food or people), building (every bed
/// taken), or trading.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Focus {
    #[default]
    Grow,
    Build,
    Trade,
}

impl Focus {
    /// `Focus.of`: short of food (under twice the caravans' par) or people (under [`GROWN`]), it grows; fed and
    /// peopled with every bed taken, it builds; else it trades.
    pub fn of(villagers: f64, beds: f64, food_points: f64) -> Focus {
        if food_points < 2.0 * FOOD_PAR_POINTS || villagers < GROWN {
            return Focus::Grow;
        }
        if villagers >= beds { Focus::Build } else { Focus::Trade }
    }

    /// Does it farm only for food (and the herds' feed)?
    pub fn food_only(self) -> bool {
        self != Focus::Trade
    }
}

/// A hall that banks crops (`Buildings.STORES`): the granary (wheat, as hay bales) and the root cellar (carrots,
/// potatoes, beetroot, in barrels).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Store {
    #[default]
    Granary,
    RootCellar,
}

impl Store {
    pub const ALL: [Store; 2] = [Store::Granary, Store::RootCellar];

    /// The crops it keeps, by the host's names.
    pub fn items(self) -> &'static [&'static str] {
        match self {
            Store::Granary => &["wheat"],
            Store::RootCellar => &["carrot", "potato", "beetroot"],
        }
    }

    /// How much it holds in all (`Buildings.GRANARY_WHEAT`, `ROOT_CELLAR_ROOTS`).
    pub fn holds(self) -> f64 {
        match self {
            Store::Granary => (64 * 64 * 9) as f64,
            Store::RootCellar => (16 * 27 * 64) as f64,
        }
    }

    /// Its materials, in emeralds, and the Farmer's level it wants at home (`Buildings.GRANARY`, `ROOT_CELLAR`).
    pub fn emeralds(self) -> f64 {
        match self {
            Store::Granary => 12.0,
            Store::RootCellar => 8.0,
        }
    }
    pub const FARMER_LEVEL: u8 = 2;

    /// The goods it keeps in this host's content.
    pub fn goods(self, content: &Content) -> Vec<GoodId> {
        self.items().iter().filter_map(|n| content.good(n)).collect()
    }
}

/// A town's farm: its plots, the plan its Farmers work, what they've grown and not yet handed over, what they're owed
/// and paid, and what the town wants of its crops.
#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct Farm {
    /// Has the host set the farm up (plots, demand, a store)? Every town has a farm from the start, so towns share one
    /// archetype (the order queries visit them in, as a loaded world has it); until it's set up it farms nothing.
    pub set_up: bool,
    /// Plots in the ground, by crop: the host's fields, and those the town plants.
    pub plots: BTreeMap<GoodId, u32>,
    /// Plots turned over to another crop on the books, by crop (below 0: turned away from it).
    pub turned: BTreeMap<GoodId, i64>,
    /// The plan: items a day of each crop the Farmers grow.
    pub per_day: BTreeMap<GoodId, f64>,
    /// What its untended plots would yield, items a day, by crop.
    pub idle: BTreeMap<GoodId, f64>,
    /// Plots lying fallow under the plan: no one wants their crop, and the town has its food.
    pub fallow: u32,
    /// Plots its Farmers could tend beyond those they work.
    pub spare_plots: u32,
    /// Produce grown and not yet handed over, fractions carried.
    pub pending: BTreeMap<GoodId, f64>,
    /// What this look grew, for the share-croppers' share of it.
    pub fresh: BTreeMap<GoodId, f64>,
    /// Wages owed the Farmers, fractions carried (a claim accruing each look, paid in whole emeralds); pay the town owes
    /// them as credit.
    pub wage_owed: f64,
    pub credit: Fixed,
    /// The Farmers' share of what the harvest is worth.
    pub labor_share: f64,
    /// Crop sales since the last accrual, and averaged a day; the harvest's worth a day, averaged.
    pub sales_since: f64,
    pub sales_per_day: f64,
    pub value_per_day: f64,
    /// How scarce farm hands are, from the last review.
    pub tightness: f64,
    /// Plots the Farmers own.
    pub freeholds: u32,
    /// Share of the wage paid (or credited) at the last settlement.
    pub paid_share: f64,
    /// Emeralds paid, and owed that couldn't be (the Farmers took it in crops), in all; items handed over in all.
    pub wages_paid: Fixed,
    pub wages_unpaid: Fixed,
    pub harvested: f64,
    /// Fields the town has planted, its households' among them.
    pub fields_planted: u32,
    pub focus: Focus,
    /// What its villagers would buy of each crop, items a day, beyond what its trades use (`recordDemand`).
    pub demand: BTreeMap<GoodId, f64>,
    /// What must be grown first, items a day: what's sold forward (other slices fill it); the herds' feed is `feed`.
    pub needs: BTreeMap<GoodId, f64>,
    /// What the herds eat, items a day (`herds` fills it): part of the villagers' demand and grown first
    /// (`recordDemand`, `afterCensus`'s needs).
    pub feed: BTreeMap<GoodId, f64>,
    /// What its villagers at the lockers its Merchant manages would buy of each good a day, as its last census counted
    /// them (`CensusTaker.noteDemand`, `recordDemand`: the review sets it, `lockers::census_demand`): part of its demand.
    pub census: BTreeMap<GoodId, f64>,
    /// Other uses of a crop: {items a day, emeralds an item} (the workshop's input, the markets its caravans reach).
    pub draws: BTreeMap<GoodId, (f64, f64)>,
    /// Stores built, and stores it would build (its crops pile up).
    pub stores: Vec<Store>,
    pub stores_wanted: Vec<Store>,
    /// What's banked in its stores, by crop.
    pub banked: BTreeMap<GoodId, f64>,
    /// When it last accrued; the day it was last reviewed.
    pub last: Option<u64>,
    pub reviewed: Option<u64>,
    /// The day of the cash book's crop sales last read, and what of them was read.
    pub sales_day: u64,
    pub sales_seen: f64,
    /// How many plots a Farmer tends as many as its level gives: half again with hoes for all (`kit.rs` sets it).
    pub hands: f64,
}

impl Default for Farm {
    fn default() -> Self {
        Farm {
            set_up: false,
            plots: BTreeMap::new(),
            turned: BTreeMap::new(),
            per_day: BTreeMap::new(),
            idle: BTreeMap::new(),
            fallow: 0,
            spare_plots: 0,
            pending: BTreeMap::new(),
            fresh: BTreeMap::new(),
            wage_owed: 0.0,
            credit: Fixed::ZERO,
            labor_share: LABOR_SHARE,
            sales_since: 0.0,
            sales_per_day: 0.0,
            value_per_day: 0.0,
            tightness: 1.0,
            freeholds: 0,
            paid_share: 1.0,
            wages_paid: Fixed::ZERO,
            wages_unpaid: Fixed::ZERO,
            harvested: 0.0,
            fields_planted: 0,
            focus: Focus::Grow,
            demand: BTreeMap::new(),
            needs: BTreeMap::new(),
            feed: BTreeMap::new(),
            census: BTreeMap::new(),
            draws: BTreeMap::new(),
            stores: Vec::new(),
            stores_wanted: Vec::new(),
            banked: BTreeMap::new(),
            last: None,
            reviewed: None,
            sales_day: 0,
            sales_seen: 0.0,
            hands: 1.0,
        }
    }
}

/// What the households have lent the treasury of their savings, with interest: owed them (`Holding.deposits`). A claim
/// that grows by its interest each look (a fraction of a thousandth on a small debt), so it's kept in `f64` and the
/// money borrowed and repaid moves in thousandths (`fixed.rs`).
#[derive(Component, Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Deposits {
    pub owed: f64,
    /// When interest on them was last reckoned.
    pub last: u64,
}

/// What a crop is wanted for: what the town would buy (its trades' use and its villagers' demand), other uses, and
/// what must be grown first.
#[derive(Clone, Debug, Default)]
pub struct Wants {
    pub demand: BTreeMap<GoodId, f64>,
    pub draws: BTreeMap<GoodId, (f64, f64)>,
    pub needs: BTreeMap<GoodId, f64>,
}

impl Wants {
    /// A town's wants: its trades' use of each good and its villagers' demand (the host's and its census's), its
    /// farm's draws and needs, and the herds' feed in both demand and needs.
    pub fn of(farm: &Farm, uses: &[f64]) -> Wants {
        let mut demand = farm.demand.clone();
        for (&g, &n) in &farm.census {
            *demand.entry(g).or_insert(0.0) += n;
        }
        for (g, u) in uses.iter().enumerate() {
            if *u > 0.0 {
                *demand.entry(g).or_insert(0.0) += u;
            }
        }
        let mut needs = farm.needs.clone();
        for (&g, &n) in &farm.feed {
            *demand.entry(g).or_insert(0.0) += n;
            *needs.entry(g).or_insert(0.0) += n;
        }
        Wants { demand, draws: farm.draws.clone(), needs }
    }

    /// What the town would buy of a good, items a day.
    pub fn demand(&self, g: GoodId) -> f64 {
        self.demand.get(&g).copied().unwrap_or(0.0)
    }

    /// What must be grown of a good first, items a day.
    pub fn need(&self, g: GoodId) -> f64 {
        self.needs.get(&g).copied().unwrap_or(0.0)
    }

    /// Everything the town would take of each good, items a day: its demand and its other draws (`afterCensus`'s
    /// `wants`, what a new field is chosen by).
    pub fn all(&self) -> Wants {
        let mut w = self.clone();
        for (g, (n, _)) in &self.draws {
            *w.demand.entry(*g).or_insert(0.0) += n;
        }
        w
    }
}

/// One use of a crop: so many items a day, at what an item earns there (`Territory.Use`).
#[derive(Clone, Copy, Debug)]
struct Use {
    crop: GoodId,
    per_day: f64,
    each: f64,
}

/// A crop's uses (`Territory.usesOf`): as itself at its worth; as what a trade makes of it alone (paper of cane, bread
/// of wheat) at that good's worth an item of the crop; and its other draws.
fn uses_of(content: &Content, crop: GoodId, wants: &Wants) -> Vec<Use> {
    let mut uses = Vec::new();
    if wants.demand(crop) > 0.0 {
        uses.push(Use { crop, per_day: wants.demand(crop), each: content.goods[crop].value });
    }
    let name = content.goods[crop].name.as_str();
    for (input, n_in, output, n_out) in CONVERSIONS {
        if input != name {
            continue;
        }
        if let Some(made) = content.good(output).filter(|&m| wants.demand(m) > 0.0) {
            uses.push(Use { crop, per_day: wants.demand(made) * n_in / n_out, each: content.goods[made].value * n_out / n_in });
        }
    }
    if let Some(&(per_day, each)) = wants.draws.get(&crop) {
        if per_day > 0.0 {
            uses.push(Use { crop, per_day, each });
        }
    }
    uses
}

/// A farm's plots by crop: those in the ground and those turned over on the books, never fewer than none
/// (`Territory.plots`).
pub fn plots(farm: &Farm) -> BTreeMap<GoodId, u32> {
    let mut out: BTreeMap<GoodId, i64> = farm.plots.iter().map(|(g, n)| (*g, *n as i64)).collect();
    for (g, n) in &farm.turned {
        *out.entry(*g).or_insert(0) += n;
    }
    out.into_iter().map(|(g, n)| (g, n.max(0) as u32)).collect()
}

/// Plots the Farmers can tend: [`PLOTS_PER_FARMER_LEVEL`] a level each (a Novice's at the least), by `hands` (tools).
pub fn capacity(farmers: &[u8], hands: f64) -> f64 {
    (hands * farmers.iter().map(|l| PLOTS_PER_FARMER_LEVEL * (*l).max(1) as u32).sum::<u32>() as f64).round()
}

/// The Farmers' plan (`Territory.plan`): the plots the feed and what's sold forward need first; then plot by plot
/// where the next item earns most, each use of each crop at what an item earns there, the dearest first, as many plots
/// as that use takes; then the rest of each crop's plots, the best-earning crop first, then the most planted. Once the
/// town is `fed`, plots of a crop no one wants lie fallow. What its plots would yield untended goes in `idle`; plots
/// lying fallow are returned beside the plan.
pub fn plan(content: &Content, plots: &BTreeMap<GoodId, u32>, farmers: &[u8], wants: &Wants, hands: f64, fed: bool,
        idle: &mut BTreeMap<GoodId, f64>) -> (BTreeMap<GoodId, f64>, u32) {
    let mut per_day: BTreeMap<GoodId, f64> = BTreeMap::new();
    let mut capacity = capacity(farmers, hands) as i64;
    let mut left: BTreeMap<GoodId, i64> = plots.iter().map(|(g, n)| (*g, *n as i64)).collect();
    let work = |per_day: &mut BTreeMap<GoodId, f64>, left: &mut BTreeMap<GoodId, i64>, crop: GoodId, n: i64| {
        *per_day.entry(crop).or_insert(0.0) += n as f64 * YIELD_PER_PLOT_PER_DAY;
        *left.get_mut(&crop).unwrap() -= n;
    };
    // Feed first: as many plots of each feed crop as the herds eat (crops in the order of their names).
    let mut crops: Vec<GoodId> = plots.keys().copied().collect();
    crops.sort_by(|a, b| content.goods[*a].name.cmp(&content.goods[*b].name));
    for &crop in &crops {
        let need = wants.need(crop);
        let worked = capacity.min(left[&crop].min((need / YIELD_PER_PLOT_PER_DAY).ceil() as i64));
        if worked > 0 {
            work(&mut per_day, &mut left, crop, worked);
            capacity -= worked;
        }
    }
    // Then each use, where an item earns most.
    let mut uses: Vec<Use> = crops.iter().flat_map(|&c| uses_of(content, c, wants)).collect();
    uses.sort_by(|a, b| b.each.total_cmp(&a.each));
    let mut best: BTreeMap<GoodId, f64> = BTreeMap::new();
    for u in &uses {
        let b = best.entry(u.crop).or_insert(u.each);
        *b = b.max(u.each);
        let worked = capacity.min(left[&u.crop].min((u.per_day / YIELD_PER_PLOT_PER_DAY).ceil() as i64));
        if worked > 0 {
            work(&mut per_day, &mut left, u.crop, worked);
            capacity -= worked;
        }
    }
    // The rest: the best-earning crop first, then the most planted, then by name.
    let best_of = |c: &GoodId| best.get(c).copied().unwrap_or(0.0);
    crops.sort_by(|a, b| best_of(b).total_cmp(&best_of(a)).then(plots[b].cmp(&plots[a]))
        .then(content.goods[*a].name.cmp(&content.goods[*b].name)));
    let mut fallow = 0;
    for &crop in &crops {
        if fed && best_of(&crop) <= 0.0 && wants.need(crop) <= 0.0 {
            fallow += left[&crop].max(0) as u32;
            continue;
        }
        let worked = capacity.min(left[&crop]);
        if worked > 0 {
            work(&mut per_day, &mut left, crop, worked);
            capacity -= worked;
        }
        if left[&crop] > 0 {
            *idle.entry(crop).or_insert(0.0) += left[&crop] as f64 * YIELD_PER_PLOT_PER_DAY;
        }
    }
    (per_day, fallow)
}

/// What a day's harvest is worth, in emeralds (`Territory.valuePerDay`).
pub fn value_per_day(content: &Content, per_day: &BTreeMap<GoodId, f64>) -> f64 {
    per_day.iter().map(|(g, n)| n * content.goods[*g].value).sum()
}

/// The Farmers' wages a day (`Territory.wagePerDay`): their share of what the harvest is worth, at least
/// [`FREEHOLD_SHARE`] on their own plots.
pub fn wage_per_day(content: &Content, farm: &Farm) -> f64 {
    let plots = farm.per_day.values().sum::<f64>() / YIELD_PER_PLOT_PER_DAY;
    let own = if plots <= 0.0 { 0.0 } else { farm.freeholds as f64 / plots };
    value_per_day(content, &farm.per_day) * share(farm.labor_share, own)
}

/// Emeralds the town keeps back for its Farmers' wages (`Territory.wageFloat`): whole ones.
pub fn wage_float(content: &Content, farm: &Farm) -> Fixed {
    Fixed::ceil((wage_per_day(content, farm) * WAGE_FLOAT_DAYS).ceil())
}

/// Crop rotation (`Territory.rotate`): plots of a crop grown beyond all its uses are turned over on the books, a
/// field's worth at most a review, to the crop owed (what's sold forward, the feed) that's shortest of plots, from the
/// crop with most plots lying untended under the last plan; else, once the town is fed, to the crop whose unmet use
/// earns most.
pub fn rotate(content: &Content, farm: &mut Farm, wants: &Wants, fed: bool) {
    let plots = plots(farm);
    let have = |g: GoodId| plots.get(&g).copied().unwrap_or(0) as i64;
    let rotation: Vec<GoodId> = ROTATION.iter().filter_map(|n| content.good(n)).collect();
    let (mut from, mut spare): (Option<GoodId>, i64) = (None, 0);
    let (mut to, mut to_each, mut short): (Option<GoodId>, f64, i64) = (None, 0.0, 0);
    let (mut owed_to, mut owed_short): (Option<GoodId>, i64) = (None, 0);
    for &crop in &rotation {
        let uses = uses_of(content, crop, wants);
        let wanted: f64 = uses.iter().map(|u| u.per_day).sum();
        let each = uses.iter().map(|u| u.each).fold(0.0, f64::max);
        let needed = (wanted / YIELD_PER_PLOT_PER_DAY).ceil() as i64;
        if have(crop) - needed > spare {
            from = Some(crop);
            spare = have(crop) - needed;
        }
        if needed > have(crop) && each > to_each {
            to = Some(crop);
            to_each = each;
            short = needed - have(crop);
        }
        let owed = (wants.need(crop) / YIELD_PER_PLOT_PER_DAY).ceil() as i64 - have(crop);
        if owed > owed_short {
            owed_to = Some(crop);
            owed_short = owed;
        }
    }
    if let Some(target) = owed_to {
        to = Some(target);
        short = owed_short;
        let idle = |c: GoodId| farm.idle.get(&c).copied().unwrap_or(0.0);
        let mut idlest: Option<GoodId> = None;
        for &c in rotation.iter().filter(|&&c| c != target) {
            let better = idlest.is_none_or(|i| idle(c).total_cmp(&idle(i)).then(have(c).cmp(&have(i))).is_gt());
            if better {
                idlest = Some(c);
            }
        }
        if let Some(i) = idlest.filter(|&i| have(i) > 0) {
            from = Some(i);
            spare = have(i);
        }
    } else if !fed {
        return; // nothing owed, and food first
    }
    let (Some(from), Some(to)) = (from, to) else { return };
    if from == to {
        return;
    }
    let per = content.dials.rotate_plots as i64;
    let most = if owed_to.is_some() { content.dials.rotate_owed_fields as i64 * per } else { per };
    let n = most.min(spare.min(short));
    if n > 0 {
        *farm.turned.entry(from).or_insert(0) -= n;
        *farm.turned.entry(to).or_insert(0) += n;
    }
}

/// Rotation for the workshop, where [`rotate`] turned nothing over because no crop is grown beyond its uses (the town's
/// own Farmers' lockers take every crop it grows): a field's worth a review from the plots lying untended under the last
/// plan, of the crop with most of them that earns less, to the one its own workshop draws for (`own`: per day and what
/// an item earns there, [`crate::workshop::input_demand`] for its own people alone) and grows too little of, up to the
/// plots the draw and the crop's feed want (not as the mod). Seed 8675309's Meadowfell grew its cows' wheat and none for
/// its cookies, its lockers buying every beetroot, and met one comfort of five for 90 days. Only untended plots: the
/// plan shares the Farmers among the crops by what each earns, and turning tended ones took Ryegate's carrots, potatoes
/// and beetroot, which its people buy at its lockers, and a beetroot field the day it was bought, all for wheat.
pub fn rotate_for_workshop(content: &Content, farm: &mut Farm, own: &BTreeMap<GoodId, (f64, f64)>, wants: &Wants) {
    let plots = plots(farm);
    let have = |g: GoodId| plots.get(&g).copied().unwrap_or(0) as i64;
    let rotation: Vec<GoodId> = ROTATION.iter().filter_map(|n| content.good(n)).collect();
    let best_each = |c: GoodId| uses_of(content, c, wants).iter().map(|u| u.each).fold(0.0, f64::max);
    let mut to: Option<(GoodId, i64, f64)> = None;
    for &c in &rotation {
        let Some(&(per_day, each)) = own.get(&c).filter(|d| d.0 > 0.0) else { continue };
        let short = ((wants.need(c) + per_day) / YIELD_PER_PLOT_PER_DAY).ceil() as i64 - have(c);
        if short > 0 && to.is_none_or(|(_, _, e)| each > e) {
            to = Some((c, short, each));
        }
    }
    let Some((to, short, to_each)) = to else { return };
    let untended = |c: GoodId| ((farm.idle.get(&c).copied().unwrap_or(0.0) / YIELD_PER_PLOT_PER_DAY) as i64).min(have(c));
    let mut from: Option<(GoodId, i64)> = None;
    for &c in rotation.iter().filter(|&&c| c != to && untended(c) > 0 && best_each(c) < to_each) {
        if from.is_none_or(|(_, n)| untended(c) > n) {
            from = Some((c, untended(c)));
        }
    }
    let Some((from, spare)) = from else { return };
    let n = (content.dials.rotate_plots as i64).min(spare).min(short);
    if n > 0 {
        *farm.turned.entry(from).or_insert(0) -= n;
        *farm.turned.entry(to).or_insert(0) += n;
    }
}

/// What a crop sells as at a locker (`Fields.SOLD_AS`): paper for cane (where the host has paper), else itself.
pub fn sold_as(content: &Content, crop: GoodId) -> GoodId {
    let name = content.goods[crop].name.as_str();
    SOLD_AS.iter().find(|(c, _)| *c == name).and_then(|(_, s)| content.good(s)).unwrap_or(crop)
}

/// The field crops this host has (`Fields.CROPS`), in the mod's order.
pub fn field_crops(content: &Content) -> Vec<GoodId> {
    CROPS.iter().filter_map(|n| content.good(n)).collect()
}

/// The [`ROTATION`] crops content has (`Territory.ROTATION`): what forward sales and rents are of.
pub fn rotation(content: &Content) -> Vec<GoodId> {
    ROTATION.iter().filter_map(|n| content.good(n)).collect()
}

/// Can a town plant this crop now (`Fields.allowed`): anything while it trades; while it grows or builds, food crops,
/// and cane where its own workshop or its lockers want it once its food is in hand: the cane, or the paper it sells as
/// (the mod's asked only for cane: seed 8675309's Redfell, its Cartographers taking 768 paper a day, planted none
/// while it grew and bought its cane off Wheatfell's wagons).
pub fn allowed(content: &Content, crop: GoodId, wants: &Wants, food_points: f64, food_only: bool) -> bool {
    !food_only || content.goods[crop].name != "sugar_cane"
        || wanted(content, crop, wants, false) > 0.0 && food_points >= 2.0 * FOOD_PAR_POINTS
}

/// Items a day the town wants of a crop (`Fields.wanted`): as itself, and, while it trades, as what a locker takes
/// for it.
pub fn wanted(content: &Content, crop: GoodId, wants: &Wants, food_only: bool) -> f64 {
    let mut want = wants.demand(crop);
    let sold = sold_as(content, crop);
    if sold != crop && !food_only {
        want += wants.demand(sold);
    }
    want
}

/// What a new field should grow (`Fields.choose`): feed the herds need and the town doesn't grow enough of first;
/// else the crop whose sale (as itself, or as paper for cane) the town's demand most exceeds what it grows, by worth;
/// carrots if nothing's wanted but it's short of food; else nothing.
pub fn choose(content: &Content, wants: &Wants, grown: &BTreeMap<GoodId, f64>, food_points: f64, food_only: bool)
        -> Option<GoodId> {
    let all = field_crops(content);
    let crops: Vec<GoodId> = all.iter().copied().filter(|&c| allowed(content, c, wants, food_points, food_only)).collect();
    let grown_of = |g: GoodId| grown.get(&g).copied().unwrap_or(0.0);
    if let Some(&c) = crops.iter().find(|&&c| wants.need(c) > grown_of(c)) {
        return Some(c);
    }
    let (mut best, mut best_value) = (None, 0.0);
    for &crop in &crops {
        let already: f64 = all.iter().filter(|&&c| sold_as(content, c) == sold_as(content, crop)).map(|&c| grown_of(c)).sum();
        // (Fed, a growing town counts what its lockers take for a crop too: cane for its Cartographers' paper.)
        let fed = food_points >= 2.0 * FOOD_PAR_POINTS;
        let value = (wanted(content, crop, wants, food_only && !fed) - already) * content.goods[sold_as(content, crop)].value;
        if value > best_value {
            best = Some(crop);
            best_value = value;
        }
    }
    if best.is_none() && food_points < 2.0 * FOOD_PAR_POINTS {
        best = content.good("carrot");
    }
    best
}

/// The plots a 9 x 9 field of a crop gives (`Fields.plant`): farmland round a water block; cane along three water
/// channels; berry bushes on every block.
pub fn field_plots(content: &Content, crop: GoodId) -> u32 {
    let water = match content.goods[crop].name.as_str() {
        "sugar_cane" => FIELD_SIZE * 3,
        "sweet_berries" => 0,
        _ => 1,
    };
    FIELD_SIZE * FIELD_SIZE - water
}

/// What the warehouse keeps loose of a crop beside its store (`Territory.loose`): its shelf, and at least two stacks.
pub fn loose(content: &Content, uses: &[f64], good: GoodId) -> f64 {
    LOOSE_AT_LEAST.max(content.dials.shelf_days * uses[good])
}

/// Does the town grow more of a store's crops than it keeps (`Territory.wantsStore`): twice what it keeps loose, or a
/// stack of harvest waiting?
pub fn wants_store(content: &Content, farm: &Farm, stock: &[f64], uses: &[f64], store: Store) -> bool {
    store.goods(content).into_iter().any(|g| {
        stock[g] >= 2.0 * loose(content, uses, g)
            || farm.pending.get(&g).copied().unwrap_or(0.0) >= content.goods[g].stack as f64
    })
}

/// The store the town has built that banks `good`, if any (`Territory.storeFor`).
pub fn store_for(content: &Content, farm: &Farm, good: GoodId) -> Option<Store> {
    farm.stores.iter().copied().find(|s| s.goods(content).contains(&good))
}

/// What's banked in a store, of all it keeps.
pub fn in_store(content: &Content, farm: &Farm, store: Store) -> f64 {
    store.goods(content).iter().map(|g| farm.banked.get(g).copied().unwrap_or(0.0)).sum()
}

/// With a store (`Territory.bank`): crops beyond what the warehouse keeps loose go into it, and it tops the
/// warehouse back up as it's drawn down. An (s, S) store with s = S = [`loose`]: the warehouse is brought back to
/// its level whenever it's below, from the store, and what's above it is banked.
pub fn bank(content: &Content, farm: &mut Farm, stock: &mut [f64], uses: &[f64]) {
    for store in farm.stores.clone() {
        for g in store.goods(content) {
            let keep = loose(content, uses, g);
            if stock[g] > keep {
                let room = store.holds() - in_store(content, farm, store);
                let n = (stock[g] - keep).min(room).floor();
                if n > 0.0 {
                    stock[g] -= n;
                    *farm.banked.entry(g).or_insert(0.0) += n;
                }
                continue;
            }
            let n = (keep - stock[g]).min(farm.banked.get(&g).copied().unwrap_or(0.0).floor()).floor();
            if n > 0.0 {
                stock[g] += n;
                *farm.banked.get_mut(&g).unwrap() -= n;
            }
        }
    }
}

/// What the treasury could still borrow of its households' savings (`Territory.depositsOnCall`): its share of all
/// they have, in purse and lent it, less what it has borrowed, as far as their purse goes (rounded down to the
/// thousandth).
pub fn on_call(content: &Content, purse: Fixed, owed: f64) -> Fixed {
    let purse_ = purse.to_f64();
    Fixed::floor(0.0f64.max(purse_.min(content.dials.deposit_share * (purse_ + owed) - owed))).min(purse.at_least_zero())
}

/// Adds produce, and the Farmers' wages for it, for the time since the farm last accrued, at the pace the town's
/// mood sets; then moves the Farmers' share toward the share of the harvest that sold, times how scarce hands are
/// (`Territory.accrue`).
pub fn accrue(content: &Content, farm: &mut Farm, mood: f64, now: u64) {
    if let Some(last) = farm.last {
        let days = MAX_DAYS.min(now.saturating_sub(last) as f64 / content.day_ticks as f64);
        let worked = days * yield_factor(mood);
        for (&g, &rate) in &farm.per_day {
            *farm.pending.entry(g).or_insert(0.0) += rate * worked;
            *farm.fresh.entry(g).or_insert(0.0) += rate * worked;
        }
        farm.wage_owed += wage_per_day(content, farm) * days;
        if days > 0.0 {
            let weight = 1.0 - crate::fixed::halved(days, SALES_HALF_LIFE_DAYS);
            farm.sales_per_day += (farm.sales_since / days - farm.sales_per_day) * weight;
            farm.value_per_day += (value_per_day(content, &farm.per_day) - farm.value_per_day) * weight;
            farm.sales_since = 0.0;
            if farm.value_per_day > 0.0 {
                let floor = content.dials.min_labor_share.min(MAX_LABOR_SHARE);
                let target = (farm.sales_per_day / farm.value_per_day * farm.tightness).clamp(floor, MAX_LABOR_SHARE);
                farm.labor_share += (target - farm.labor_share) * (1.0 - crate::fixed::halved(days, SHARE_HALF_LIFE_DAYS));
            }
        }
    }
    farm.last = Some(now);
}

/// The Farmers' plan made again from the farm's plots and the town's wants (`afterCensus`): the plan, what lies
/// untended and fallow, the spare hands, how scarce hands are, and the freeholds still standing.
pub fn replan(content: &Content, farm: &mut Farm, farmers: &[u8], uses: &[f64], fed: bool) {
    let wants = Wants::of(farm, uses);
    let mut idle = BTreeMap::new();
    let (per_day, fallow) = plan(content, &plots(farm), farmers, &wants, farm.hands, fed, &mut idle);
    let tended = per_day.values().sum::<f64>() / YIELD_PER_PLOT_PER_DAY;
    let capacity = capacity(farmers, farm.hands);
    let untended = idle.values().sum::<f64>() / YIELD_PER_PLOT_PER_DAY;
    farm.spare_plots = (capacity - tended.round()).max(0.0) as u32;
    farm.tightness = tightness(capacity, tended, untended);
    farm.freeholds = farm.freeholds.min((tended + untended).round() as u32);
    farm.per_day = per_day;
    farm.idle = idle;
    farm.fallow = fallow;
}

/// The levels of a town's Farmers at home (adults, not away).
pub fn farmers(content: &Content, roster: Option<&crate::roster::Roster>) -> Vec<u8> {
    let Some(farmer) = content.trade("farmer") else { return Vec::new() };
    roster.map_or_else(Vec::new, |r| {
        r.hands.iter().filter(|h| h.trade == farmer && !h.is_baby() && !h.away).map(|h| h.level).collect()
    })
}

/// The town's crop sales since the farm last read them, from its cash book: what its lockers took for crops
/// (`recordCropSale`, as the lockers note it).
pub fn crop_sales(content: &Content, book: &crate::cash_book::CashBook, town: usize, farm: &mut Farm, now: u64) -> f64 {
    locker_sales(content, book, town, (&mut farm.sales_day, &mut farm.sales_seen), now, true)
}

/// What a town's lockers took for its crops (`crops`) or for its other goods since `mark` (the day last read, and that
/// day's total then) was last moved on, from its cash book; `mark` moved on to now.
pub fn locker_sales(content: &Content, book: &crate::cash_book::CashBook, town: usize, mark: (&mut u64, &mut f64), now: u64,
        crops: bool) -> f64 {
    use crate::cash_book::{Account, Reason};
    let (sales_day, sales_seen) = mark;
    let today = now / content.day_ticks.max(1);
    let on = |day: u64| -> f64 {
        book.books.get(&Account::Treasury(town)).map_or(0.0, |b| {
            content.goods.iter().enumerate().filter(|(_, g)| g.crop == crops)
                .map(|(i, _)| b.get(&(day, Reason::Sold(i))).copied().unwrap_or_default()).sum::<Fixed>().to_f64()
        })
    };
    let mut since = on(*sales_day) - *sales_seen;
    if today != *sales_day {
        since += on(today);
        *sales_day = today;
    }
    *sales_seen = on(today);
    since.max(0.0)
}

/// A town's farm as a host sees it.
#[derive(Clone, Debug)]
pub struct FarmView {
    pub farm: Farm,
    pub wage_per_day: f64,
    pub deposits: f64,
}

impl crate::society::Society {
    fn farm_entity(&mut self, town: crate::messages::TownId) -> Entity {
        let e = self.towns[town];
        if !self.world.get::<Farm>(e).is_some_and(|f| f.set_up) {
            let now = self.now();
            let content = self.world.resource::<Content>();
            let mut farm = Farm { set_up: true, last: Some(now), sales_day: now / content.day_ticks.max(1), ..Default::default() };
            // Crop sales start counting from now: what its lockers sold before the farm was set up isn't its.
            crop_sales(content, self.world.resource::<crate::cash_book::CashBook>(), town, &mut farm, now);
            self.world.entity_mut(e).insert(farm);
        }
        e
    }

    /// Settles the farm under its old plan, then makes its plan again (the mod's `afterCensus`).
    fn farm_census(&mut self, town: crate::messages::TownId) {
        let e = self.farm_entity(town);
        let now = self.now();
        let content = self.world.resource::<Content>().clone();
        let mood = self.world.get::<crate::state::People>(e).map_or(0.0, |p| p.mood);
        let farmers = farmers(&content, self.world.get::<crate::roster::Roster>(e));
        let uses = self.world.get::<crate::state::Trades>(e).unwrap().uses.clone();
        let fed = crate::people::food_points(&content, self.world.get::<crate::state::Stock>(e).unwrap())
            >= 2.0 * FOOD_PAR_POINTS;
        let mut farm = self.world.get_mut::<Farm>(e).unwrap();
        accrue(&content, &mut farm, mood, now);
        replan(&content, &mut farm, &farmers, &uses, fed);
    }

    /// A town's plots of a crop in the ground (its fields, as the host counts them): its Farmers' plan is made again.
    pub fn set_plots(&mut self, town: crate::messages::TownId, good: GoodId, plots: u32) {
        let e = self.farm_entity(town);
        self.world.get_mut::<Farm>(e).unwrap().plots.insert(good, plots);
        self.farm_census(town);
    }

    /// Plots turned over on the books to (above 0) or from (below 0) a crop: its Farmers' plan is made again.
    pub fn turn_plots(&mut self, town: crate::messages::TownId, good: GoodId, plots: i64) {
        let e = self.farm_entity(town);
        *self.world.get_mut::<Farm>(e).unwrap().turned.entry(good).or_insert(0) += plots;
        self.farm_census(town);
    }

    /// What a town's villagers would buy of a good, items a day, beyond its trades' use (the mod's `recordDemand`):
    /// its Farmers' plan is made again.
    pub fn set_farm_demand(&mut self, town: crate::messages::TownId, good: GoodId, per_day: f64) {
        let e = self.farm_entity(town);
        self.world.get_mut::<Farm>(e).unwrap().demand.insert(good, per_day);
        self.farm_census(town);
    }

    /// The town's Farmers' plan made again now, as a census would.
    pub fn farm_review(&mut self, town: crate::messages::TownId) {
        self.farm_census(town);
    }

    /// A host's locker at the town sold crops for `emeralds` (`Territory.recordCropSale`); the core's own lockers'
    /// sales are read from the cash book.
    pub fn record_crop_sale(&mut self, town: crate::messages::TownId, emeralds: f64) {
        let e = self.farm_entity(town);
        self.world.get_mut::<Farm>(e).unwrap().sales_since += emeralds;
    }

    /// A store finished for the town (the works' doing; a host's, until they come).
    pub fn build_store(&mut self, town: crate::messages::TownId, store: Store) {
        let e = self.farm_entity(town);
        let mut farm = self.world.get_mut::<Farm>(e).unwrap();
        if !farm.stores.contains(&store) {
            farm.stores.push(store);
        }
    }

    /// Does the town grow more of a store's crops than it keeps (`Territory.wantsStore`)?
    pub fn wants_store(&self, town: crate::messages::TownId, store: Store) -> bool {
        let e = self.world.entity(self.towns[town]);
        let Some(farm) = e.get::<Farm>().filter(|f| f.set_up) else { return false };
        wants_store(self.content(), farm, &e.get::<crate::state::Stock>().unwrap().0,
            &e.get::<crate::state::Trades>().unwrap().uses, store)
    }

    /// A town's farm, if it's been set up, as it stood at the last look.
    pub fn farm(&self, town: crate::messages::TownId) -> Option<FarmView> {
        let e = self.world.entity(self.towns[town]);
        e.get::<Farm>().filter(|f| f.set_up).map(|f| FarmView {
            farm: f.clone(),
            wage_per_day: wage_per_day(self.content(), f),
            deposits: e.get::<Deposits>().map_or(0.0, |d| d.owed),
        })
    }

    /// What the town has banked of a crop in its stores.
    pub fn banked(&self, town: crate::messages::TownId, good: GoodId) -> f64 {
        self.world.get::<Farm>(self.towns[town]).and_then(|f| f.banked.get(&good).copied()).unwrap_or(0.0)
    }

    /// What the treasury could still borrow of its households' savings (`Territory.depositsOnCall`).
    pub fn deposits_on_call(&self, town: crate::messages::TownId) -> Fixed {
        let e = self.world.entity(self.towns[town]);
        on_call(self.content(), e.get::<crate::state::People>().unwrap().purse, e.get::<Deposits>().map_or(0.0, |d| d.owed))
    }

    /// What the treasury owes its households of their savings, with interest.
    pub fn deposits(&self, town: crate::messages::TownId) -> f64 {
        self.world.get::<Deposits>(self.towns[town]).map_or(0.0, |d| d.owed)
    }

    /// The treasury borrows up to `emeralds` of its households' savings, as far as they're on call
    /// (`Territory.borrowDeposits`): owed them back with interest. Returns what came in.
    pub fn borrow_deposits(&mut self, town: crate::messages::TownId, emeralds: impl Into<Fixed>) -> Fixed {
        let emeralds = emeralds.into();
        let e = self.towns[town];
        let now = self.now();
        if self.world.get::<Deposits>(e).is_none() {
            self.world.entity_mut(e).insert(Deposits::default());
        }
        self.world.resource_scope(|world, content: Mut<Content>| {
            world.resource_scope(|world, mut ledger: Mut<crate::state::Ledger>| {
                world.resource_scope(|world, mut book: Mut<crate::cash_book::CashBook>| {
                    let mut town_ = world.entity_mut(e);
                    let mut deposits = *town_.get::<Deposits>().unwrap();
                    let mut money = town_.get::<crate::state::Money>().unwrap().0;
                    let mut purse = town_.get::<crate::state::People>().unwrap().purse;
                    let got = borrow(&content, &mut ledger, &mut book, town, &mut money, &mut purse, &mut deposits,
                        emeralds, now);
                    town_.get_mut::<crate::state::Money>().unwrap().0 = money;
                    town_.get_mut::<crate::state::People>().unwrap().purse = purse;
                    *town_.get_mut::<Deposits>().unwrap() = deposits;
                    got
                })
            })
        })
    }
}

/// The treasury borrows up to `emeralds` of its households' savings, as far as [`on_call`]: from their purse into its
/// money (posted "deposits"), owed them with interest from now. Returns what came in.
#[allow(clippy::too_many_arguments)]
pub fn borrow(content: &Content, ledger: &mut crate::state::Ledger, book: &mut crate::cash_book::CashBook, town: usize,
        money: &mut Fixed, purse: &mut Fixed, deposits: &mut Deposits, emeralds: Fixed, now: u64) -> Fixed {
    use crate::cash_book::{post, Reason, Side};
    let take = emeralds.min(on_call(content, *purse, deposits.owed));
    if take <= Fixed::from_millis(10) {
        return Fixed::ZERO;
    }
    if deposits.owed <= 0.01 {
        deposits.last = now; // interest runs from now
    }
    post(ledger, book, Side::households(town, purse), Side::treasury(town, money), take,
        Reason::Other("deposits".into()), now, content.day_ticks);
    deposits.owed += take.to_f64();
    take
}
