//! Firms, a player's businesses (John, 2026-11-03, "How to Add Society Systems to the Sim": "Start a firm as a thin
//! wrapper around what a merchant or smith already does, then let it hire"; step 3, "Firms, plus 2–3 processing jobs
//! with threshold and range"; 2026-11-04, "Allow capital investments for higher quality goods", "Baker, Weaver, Tanner.
//! Higher quality goods", "Town hands of its trade"; and `docs/firms-plan.md`, slice 1).
//!
//! A firm is a household's capital put into a workshop of one of the [`KINDS`]: a bakery, a weaver's or a tannery. It
//! works the town's materials as its own workshop would, but each batch comes out at a grade (`persons::Grade`: seconds
//! to masterwork), Fundamentals × Morale: the skill and talents of the hand that made it, times how its shop is running
//! (its head, its crew's spirits, an inspiration). Its hands are hands of its trade it hires away from the town's
//! wages; past `firm_hands_unled` of them its owners appoint a head, the most skilled at its process, by a mandate of
//! the firm's (`charters.rs`). It opens where enough people live within its range (the kind's threshold and range, the
//! guide's central places), its town wants what it makes, and a hand of its trade would make more than it earns now;
//! it's wound up once it hasn't paid its hands in full for `firm_broke_days`.
//!
//! A firm holds its owners and their shares, a treasury (on the ledger, `Account::Firm`), its employees on wages, what
//! it has made by grade, what befell its hands, and its reputation (slice 3). Its claims (`property.rs`, owned by
//! [`Party::Firm`]) are for later.

use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::charters::Party;
use crate::content::{Content, GoodId};
use crate::fixed::Fixed;
use crate::persons::{Event, Grade, Work};
use crate::messages::TownId;
use crate::roster::{HandId, TradeId};
use crate::workshop::Conversion;

pub type FirmId = u32;

/// A step of a kind of firm: its work, what it makes, and the recipes it makes it by (the first its town has the
/// inputs for first): each its inputs by name and how many it makes.
pub type StepDef = (Work, &'static str, &'static [(&'static [(&'static str, f64)], f64)]);

/// A tannery's tanning, and a bootmaker's first step: four rabbit hides tanned into a leather (the game's recipe), or a
/// hide cured (a cow's leather, which the town holds plain, into graded leather).
const TANNING: StepDef = (Work::Tanning, "leather", &[(&[("rabbit_hide", 4.0)], 1.0), (&[("leather", 1.0)], 1.0)]);

/// The kinds of firm: name, the trade it hires, its steps in turn (the last makes what it sells; an earlier one makes
/// what the next works, `docs/firms-plan.md` slice 2), the people within its range it needs (threshold) and that range
/// in blocks, and the capital its workshop takes, in emeralds (the guide's starting values: a baker 40 and 150; the
/// others the smith's 300 blocks, and its 60 people cut to 50: seed 1717's towns are 55 at most, and no Shepherd's or
/// Leatherworker's had 60 within its range all year). A tannery sells its leather to its town, where a bootmaker buys
/// it before tanning its own.
/// An ironworks' smelting, and the iron chain's first step (`docs/firms-plan.md` slice 4): eight raw iron and a coal
/// (or a charcoal) make eight ingots (`services::RAW_PER_COAL`).
const SMELTING: StepDef = (Work::Smelting, "iron_ingot", &[(&[("raw_iron", 8.0), ("coal", 1.0)], 8.0),
    (&[("raw_iron", 8.0), ("charcoal", 1.0)], 8.0)]);

#[allow(clippy::type_complexity)]
pub const KINDS: [(&str, &str, &[StepDef], f64, f64, f64); 7] = [
    ("bakery", "farmer", &[(Work::Baking, "bread", &[(&[("wheat", 3.0)], 1.0)])], 40.0, 150.0, 30.0),
    ("weaver", "shepherd", &[(Work::Weaving, "white_carpet", &[(&[("white_wool", 2.0)], 3.0)])], 50.0, 300.0, 40.0),
    ("tannery", "leatherworker", &[TANNING], 50.0, 300.0, 40.0),
    ("bootmaker", "leatherworker", &[TANNING, (Work::Leatherwork, "leather_boots", &[(&[("leather", 4.0)], 1.0)])],
        50.0, 300.0, 40.0),
    // The iron chain: an ironworks' ingots, graded, to the smiths', whose kit (`kit.rs`) counts as its grade.
    ("ironworks", "armorer", &[SMELTING], 50.0, 300.0, 60.0),
    ("bladesmith", "weaponsmith", &[(Work::Smithing, "iron_sword", &[(&[("iron_ingot", 2.0)], 1.0)])], 50.0, 300.0, 50.0),
    ("toolmaker", "toolsmith", &[(Work::Smithing, "iron_hoe", &[(&[("iron_ingot", 2.0)], 1.0)])], 50.0, 300.0, 50.0),
];

/// A step of a kind as the host's content has it: its work, and its recipes the host has the goods of.
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub work: Work,
    pub output: GoodId,
    pub recipes: Vec<Conversion>,
}

/// A kind of firm as the host's content has it: its trade, its steps, and the recipe its hands are judged by (its last
/// step's last: a bootmaker's sewing, a tannery's curing). `None` where the host lacks the trade or the last step's goods.
#[derive(Clone, Debug, PartialEq)]
pub struct Kind {
    pub name: &'static str,
    pub trade: TradeId,
    pub steps: Vec<Step>,
    pub conversion: Conversion,
    pub work: Work,
    pub threshold: f64,
    pub range: f64,
    pub capital: f64,
}

impl Kind {
    /// The step that makes this good (an earlier step's output, the next one's material), if one does.
    pub fn step_of(&self, g: GoodId) -> Option<usize> {
        self.steps.iter().position(|s| s.output == g)
    }
}

/// Each kind the host's content has.
pub fn kinds(content: &Content) -> Vec<Kind> {
    KINDS.iter().filter_map(|&(name, trade, steps, threshold, range, capital)| {
        let trade = content.trade(trade)?;
        let steps: Vec<Option<Step>> = steps.iter().map(|&(work, output, recipes)| {
            let output = content.good(output)?;
            let recipes: Vec<Conversion> = recipes.iter().filter_map(|&(inputs, out)| {
                let inputs = inputs.iter().map(|&(g, n)| content.good(g).map(|g| (g, n))).collect::<Option<Vec<_>>>()?;
                Some(Conversion { trades: vec![trade], inputs, output, out })
            }).collect();
            (!recipes.is_empty()).then_some(Step { work, output, recipes })
        }).collect();
        // (Without its last step's goods there's no such kind; an earlier step the host can't do, its materials are
        // bought.)
        let last = steps.last()?.clone()?;
        let steps: Vec<Step> = steps.into_iter().flatten().collect();
        let (conversion, work) = (last.recipes.last()?.clone(), last.work);
        Some(Kind { name, trade, steps, conversion, work, threshold, range, capital })
    }).collect()
}

/// The names of a conversion's inputs, bare (a material talent's).
pub fn ingredients(content: &Content, c: &Conversion) -> Vec<String> {
    c.inputs.iter().map(|&(g, _)| content.goods[g].name.rsplit(':').next().unwrap_or_default().to_string()).collect()
}

/// How many more of its graded pieces a town wants of a firm: what its own workshop keeps it at of the output
/// (`workshop::wanted`'s pars: a comfort's rations for `comfort_days`, food at twice the food par, else a locker's
/// par), less the graded pieces it holds (`People::graded`). Its people use the best pieces first, so a firm keeps
/// their stock up whatever else the town holds (meat enough, its people still eat bread, a business's first), and the
/// town's workshop makes plain only what the firm can't.
pub fn graded_wanted(content: &Content, people: &crate::state::People, output: GoodId, locker_par: f64) -> f64 {
    graded_wanted_for(content, people, output, locker_par, 0.0)
}

/// [`graded_wanted`], a material its town's businesses use kept at what they would use too (`needed`).
pub fn graded_wanted_for(content: &Content, people: &crate::state::People, output: GoodId, locker_par: f64,
        needed: f64) -> f64 {
    let g = &content.goods[output];
    let par = if g.comfort.is_some() {
        crate::people::comfort_par(content, people) / content.comfort_worth(output).max(1e-9)
    } else if g.food > 0.0 {
        2.0 * crate::workshop::FOOD_PAR_POINTS / g.food.max(1e-9)
    } else {
        locker_par.max(needed)
    };
    let held: f64 = people.graded.get(&output).map_or(0.0, |of| of.iter().sum());
    (par - held).max(0.0).floor()
}

/// What a point of fundamentals is worth to a piece, a plain piece's being 1: the span from seconds (a score of 3) to
/// a masterwork (30), on the whole.
pub fn worth_per_point(d: &crate::content::Dials) -> f64 {
    (Grade::Masterwork.worth(d) - Grade::Seconds.worth(d)) / 27.0
}

/// What its town's businesses bid for each grade above Plain of each material they work, a piece (John: "Buyer bids
/// its gain"): what a grade of it adds to their own pieces, its two points of fundamentals a grade, the best bid of
/// them. A graded piece of a material no business works is worth its grade, as any other.
pub fn bids<'a>(content: &Content, kinds: impl IntoIterator<Item = &'a Kind>) -> BTreeMap<GoodId, f64> {
    let mut bids = BTreeMap::new();
    for k in kinds {
        let c = &k.conversion;
        for &(g, per) in c.inputs.iter().filter(|i| i.0 != c.output) {
            let bid = c.out * content.goods[c.output].value * worth_per_point(&content.dials) * 2.0 / per.max(1e-9);
            let b = bids.entry(g).or_insert(0.0_f64);
            *b = b.max(bid);
        }
    }
    bids
}

/// What a piece of `g` at `grade` sells for in a town whose businesses bid `bids`: its grade's worth, or what they bid
/// for it if more (seconds at their worth).
pub fn price(content: &Content, bids: &BTreeMap<GoodId, f64>, g: GoodId, grade: Grade) -> f64 {
    let value = content.goods[g].value;
    let bid = bids.get(&g).map_or(0.0, |b| value + b * (grade.index() as f64 - 1.0));
    (value * grade.worth(&content.dials)).max(if grade > Grade::Plain { bid } else { 0.0 })
}

/// The best grade a station of this tier lets a piece be: a first one's Fine, a second's or better a masterwork.
pub fn ceiling(tier: u8) -> Grade {
    if tier >= 2 { Grade::Masterwork } else { Grade::Fine }
}

/// How much faster a station of this tier works than a first one.
pub fn throughput(d: &crate::content::Dials, tier: u8) -> f64 {
    1.0 + d.station_throughput * (tier.max(1) - 1) as f64
}

/// What a station of this tier costs a kind of business (its capital for each tier).
pub fn station_price(kind: &Kind, tier: u8) -> f64 {
    kind.capital * tier as f64
}

/// What a batch at `grade` is worth less its inputs, in emeralds: what a hand's batch adds.
pub fn added(content: &Content, bids: &BTreeMap<GoodId, f64>, c: &Conversion, grade: Grade) -> f64 {
    c.out * price(content, bids, c.output, grade) - crate::workshop::inputs_worth(content, c)
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Firm {
    pub id: FirmId,
    pub town: TownId,
    /// Its kind's name ([`KINDS`]).
    pub name: String,
    /// Its owners and each one's share (they add up to 1).
    pub owners: Vec<(Party, f64)>,
    pub treasury: Fixed,
    /// Its hands on wages, and what each is paid a day.
    pub employees: Vec<(HandId, Fixed)>,
    /// What it makes: its output good, by name.
    pub recipe: Option<String>,
    /// What its name does for it (`docs/firms-plan.md` slice 3): earned by its pieces' grades, a little of it fading
    /// each day. Past `house_reputation` it's a House.
    pub reputation: f64,
    pub founded: u64,
    /// The days running it hasn't paid its hands in full (it's wound up after `firm_broke_days`).
    pub broke_days: u32,
    /// The pieces it has made, all told, and by grade (`Grade::index`).
    #[serde(default)]
    pub made: f64,
    #[serde(default)]
    pub grades: [f64; 5],
    /// The batches it has made, all told (each one's draw its own).
    #[serde(default)]
    pub batches: u64,
    /// The day it last reckoned (its hiring, wages and owners' takings once a day).
    #[serde(default)]
    pub reckoned: Option<u64>,
    /// What its town wants a day of what it makes, on average (a fifth of the way to each day's), and what it wanted
    /// since the last day's reckoning (at each look, before the firm works): it hires for that, not to fill the town's
    /// stock at once.
    #[serde(default)]
    pub used: f64,
    #[serde(default)]
    pub eaten: f64,
    /// When it last worked, and the batches its hands have done toward the next at each step (worked at each look, a
    /// day's at most carried).
    #[serde(default)]
    pub worked: Option<u64>,
    #[serde(default)]
    pub progress: Vec<f64>,
    /// The graded pieces of its materials it has bought of each other business, all told: its suppliers.
    #[serde(default)]
    pub suppliers: Vec<(FirmId, f64)>,
    /// What it has sold its town since its last reckoning, and the days in a row its town has bought of it.
    #[serde(default)]
    pub sold: f64,
    #[serde(default)]
    pub regular: u32,
    /// Its name as a House, once its reputation made it one (a name, once made, it keeps); a branch has its House's.
    #[serde(default)]
    pub house: Option<String>,
    /// The House it's a branch of, if it is one (owned by that business, in another town).
    #[serde(default)]
    pub branch_of: Option<FirmId>,
    /// Money of its treasury riding with a wagon or waiting for one ([`Carried`]): not its to spend.
    #[serde(default)]
    pub pledged: Fixed,
    /// A branch's takings beyond its working days, for its House: sent home with the House's next wagon to call.
    #[serde(default)]
    pub due: Fixed,
    /// Its steps' stations, by step: each one's tier (none said: the first), whether it's broken (it works again once
    /// repaired, the next day), and the pieces a day its station held back from a better grade (on average, a fifth
    /// of the way to each day's), and since its last reckoning.
    #[serde(default)]
    pub stations: Vec<u8>,
    #[serde(default)]
    pub broken: Vec<bool>,
    #[serde(default)]
    pub held_back: Vec<f64>,
    #[serde(default)]
    pub held_today: Vec<f64>,
    /// Its head, past `firm_hands_unled` hands: one of them, by mandate (`charters.rs`).
    #[serde(default)]
    pub head: Option<Head>,
    /// What befell its hands, latest last (the last [`EVENTS_KEPT`]): the day, the hand, what.
    #[serde(default)]
    pub events: Vec<(u64, HandId, Event)>,
}

/// The events a firm keeps.
pub const EVENTS_KEPT: usize = 40;

/// A firm's head: the hand its owners put over the others, and its mandate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Head {
    pub hand: HandId,
    pub mandate: crate::charters::MandateId,
}

impl Firm {
    pub fn output(&self, content: &Content) -> Option<GoodId> {
        self.recipe.as_deref().and_then(|r| content.good(r))
    }

    /// What more its pieces sell for, its name's premium: a tenth for each `house_reputation`, up to
    /// `house_premium_most`, a House's.
    pub fn premium(&self, d: &crate::content::Dials) -> f64 {
        if self.house.is_none() {
            return 0.0;
        }
        (0.1 * self.reputation / d.house_reputation.max(1e-9)).clamp(0.0, d.house_premium_most)
    }

    /// The share of the hands newly of its trade in its town that come with a talent for its work: its draw.
    pub fn draw(&self, d: &crate::content::Dials) -> f64 {
        if self.house.is_none() {
            return 0.0;
        }
        (d.house_draw_most * self.reputation / (4.0 * d.house_reputation.max(1e-9))).clamp(0.0, d.house_draw_most)
    }

    /// Its station's tier at step `s` (1, the first, where none is said).
    pub fn station(&self, s: usize) -> u8 {
        self.stations.get(s).copied().unwrap_or(1).max(1)
    }

    pub fn is_broken(&self, s: usize) -> bool {
        self.broken.get(s).copied().unwrap_or(false)
    }

    /// The business it has bought most of its materials of.
    pub fn main_supplier(&self) -> Option<FirmId> {
        self.suppliers.iter().max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0))).map(|s| s.0)
    }

    pub fn note(&mut self, day: u64, hand: HandId, event: Event) {
        self.events.push((day, hand, event));
        if self.events.len() > EVENTS_KEPT {
            self.events.remove(0);
        }
    }
}

/// What money a business sends by wagon is for: a House's capital for a branch in the town it goes to, or a branch's
/// takings home to its House.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Errand {
    Branch,
    Home,
}

/// Where money a business sends is: waiting for its route's next wagon, riding with it, or there.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stage {
    Waiting,
    Riding,
    There,
}

/// Money a business sends by its town's wagon (nothing crosses distance without a carrier): on the books its own until
/// it's there, pledged meanwhile (`Firm::pledged`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Carried {
    pub errand: Errand,
    /// The business sending it.
    pub firm: FirmId,
    /// The town it's going to, by the route whose wagon carries it.
    pub town: TownId,
    pub route: usize,
    pub amount: Fixed,
    pub stage: Stage,
    /// The tick it was sent (one riding too long is lost on the road: its pledge freed).
    pub since: u64,
}

/// Every firm, the next firm's id, the firms wound up (with the day), and the money they send by wagon.
#[derive(Resource, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Firms {
    pub firms: Vec<Firm>,
    pub next: FirmId,
    #[serde(default)]
    pub wound_up: Vec<(Firm, u64)>,
    #[serde(default)]
    pub carried: Vec<Carried>,
}

impl Firms {
    /// A wagon left on `route`: the money waiting for it rides with it.
    pub fn wagon_left(&mut self, route: usize) {
        for c in self.carried.iter_mut().filter(|c| c.route == route && c.stage == Stage::Waiting) {
            c.stage = Stage::Riding;
        }
    }

    /// A wagon of `route` got to `town`, from `home`: what it carried there is there, and it takes up what the
    /// branches there have due for their House at `home`, home with it.
    pub fn wagon_at(&mut self, route: usize, town: TownId, home: TownId, now: u64) {
        for c in self.carried.iter_mut().filter(|c| c.route == route && c.stage == Stage::Riding && c.town == town) {
            c.stage = Stage::There;
        }
        let houses: Vec<FirmId> = self.firms.iter().filter(|f| f.town == home).map(|f| f.id).collect();
        for f in self.firms.iter_mut().filter(|f| f.town == town && f.branch_of.is_some_and(|h| houses.contains(&h))
                && f.due.is_positive()) {
            self.carried.push(Carried { errand: Errand::Home, firm: f.id, town: home, route, amount: f.due,
                stage: Stage::Riding, since: now });
            f.pledged += f.due;
            f.due = Fixed::ZERO;
        }
    }
}

impl Firms {
    /// The hands of a town working for its firms.
    pub fn employed(&self, town: TownId) -> BTreeSet<HandId> {
        self.firms.iter().filter(|f| f.town == town).flat_map(|f| f.employees.iter().map(|e| e.0)).collect()
    }

    pub fn get_mut(&mut self, id: FirmId) -> Option<&mut Firm> {
        self.firms.iter_mut().find(|f| f.id == id)
    }
}

impl crate::society::Society {
    pub fn firms(&self) -> Vec<Firm> {
        self.world.resource::<Firms>().firms.clone()
    }

    /// The firms wound up, each with the day.
    pub fn firms_wound_up(&self) -> Vec<(Firm, u64)> {
        self.world.resource::<Firms>().wound_up.clone()
    }
}
