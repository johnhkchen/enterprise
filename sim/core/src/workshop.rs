//! The workshop: a town's trades turning raw goods into the goods they deal in, on touch. Ported from
//! `territory/Workshop.java`: `CONVERSIONS` (35), `potential` (61), `inputDemand` (104) with `draw`, `gain` (151),
//! `inputsWorth` (156), `convert` (176), `comfortWanted` (219), `wanted` (228), `run` (240); and `territory/Comforts.java`
//! `kindOf` (60) (a good's comfort kind is [`crate::content::Good::comfort`]).
//!
//! A conversion runs only if a hand of one of its trades is on the town's roster. It runs as far as its output is
//! wanted: a comfort up to the town's par less what it holds, in items, and as much again for each route out of the
//! town (what it makes, it can sell); food while the town holds less than twice the food par; anything else up to the
//! par a locker buys it at. It runs whole batches only, the conversion that earns most per emerald of inputs first, and
//! leaves alone the stock set aside for forward sales.
//!
//! **Leontief input-output** (`docs/core-math.md`), in its simplest form: the conversions are a matrix `A` (inputs per
//! unit of output). What a town wants of its comforts is a demand vector `d`; the inputs it draws a day are `A d`, and
//! what its own trades make of those, `A (A d)`: [`input_demand`] is that back-substitution, two steps deep, each row
//! carrying the worth an item earns there.
//!
//! **Not yet as the mod:** a comfort's worth in the ranking is its plain value, not `Desires.reckon`; food wanted
//! divides by the output's own food points (the mod hard-codes 5, bread's and cooked cod's); `run` has no warehouse
//! room limit; the books' `made:` and `could make more: short of` notes aren't kept; and the workshop runs in
//! `Step::Work`, after the touch's comforts and food are used, where the mod's farm touch converts before
//! `enjoyComforts` and `householdsPay` (`Territory.java` 1416 vs 1430), so what it makes is used at the next touch.

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::content::{Content, GoodId};
use crate::people::{comfort_held, comfort_par, food_points};
use crate::roster::{Roster, TradeId};
use crate::society::Society;
use crate::state::{Ledger, People, Stock};

/// A town keeps its food at twice this many points before its bakers rest (`Warehouse.FOOD_PAR_POINTS`).
pub const FOOD_PAR_POINTS: f64 = 320.0;

/// `inputs` make `out` of `output`, done by any of `trades` (the mod's `Conversion`, with any number of inputs).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Conversion {
    pub trades: Vec<TradeId>,
    pub inputs: Vec<(GoodId, f64)>,
    pub output: GoodId,
    pub out: f64,
}

/// The mod's seven conversions (`Workshop.CONVERSIONS`, in its order), then those the game has no recipe for that its
/// pack adds (`VillagerCrafts.EXTRAS`: a Librarian breaks a bookshelf into its three books; a Butcher cooks beef and
/// mutton over a campfire, no coal, as the game's campfire does): trade names, inputs, output, how many it makes. The
/// game's own recipes come with the mod's pack (`VillagerCrafts`), not here.
#[allow(clippy::type_complexity)]
pub const MOD_CONVERSIONS: [(&[&str], &[(&str, f64)], &str, f64); 10] = [
    (&["librarian", "cartographer"], &[("sugar_cane", 1.0)], "paper", 1.0),
    (&["farmer"], &[("wheat", 3.0)], "bread", 1.0),
    (&["shepherd"], &[("white_wool", 2.0)], "white_carpet", 3.0),
    (&["leatherworker"], &[("leather", 4.0)], "leather_boots", 1.0),
    (&["fisherman"], &[("cod", 1.0)], "cooked_cod", 1.0),
    (&["farmer"], &[("wheat", 2.0), ("sugar_cane", 1.0)], "cookie", 4.0),
    (&["librarian"], &[("paper", 3.0), ("leather", 1.0)], "book", 1.0),
    (&["librarian"], &[("bookshelf", 1.0)], "book", 3.0),
    (&["butcher"], &[("beef", 1.0)], "cooked_beef", 1.0),
    (&["butcher"], &[("mutton", 1.0)], "cooked_mutton", 1.0),
];

/// The mod's conversions, matched by name to the host's goods and trades; one the host lacks a good or trade for is
/// skipped.
pub fn mod_conversions(content: &Content) -> Vec<Conversion> {
    MOD_CONVERSIONS.iter().filter_map(|&(trades, inputs, output, out)| conversion(content, trades, inputs, output, out)).collect()
}

/// A conversion by names; `None` if a good or every trade is missing from the host's content.
pub fn conversion(content: &Content, trades: &[&str], inputs: &[(&str, f64)], output: &str, out: f64) -> Option<Conversion> {
    let trades: Vec<TradeId> = trades.iter().filter_map(|t| content.trade(t)).collect();
    let inputs: Option<Vec<(GoodId, f64)>> = inputs.iter().map(|&(g, n)| content.good(g).map(|g| (g, n))).collect();
    let output = content.good(output)?;
    if trades.is_empty() || inputs.as_ref()?.is_empty() {
        return None;
    }
    Some(Conversion { trades, inputs: inputs?, output, out })
}

/// What the town's side hooks tell its workshop: stock set aside for forward sales (the bakers leave the wheat it
/// owes), and the par a locker buys each good up to (0: none does). Every town has one; empty means neither.
#[derive(Component, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Workshop {
    pub aside: Vec<f64>,
    pub locker_par: Vec<f64>,
}

impl Workshop {
    pub fn aside_of(&self, good: GoodId) -> f64 {
        self.aside.get(good).copied().unwrap_or(0.0)
    }

    pub fn par_of(&self, good: GoodId) -> f64 {
        self.locker_par.get(good).copied().unwrap_or(0.0)
    }
}

/// Whole units in `x`, forgiving the dust floats leave.
fn whole(x: f64) -> f64 {
    (x + 1e-9).floor()
}

/// Is a hand of one of the conversion's trades on the roster?
pub fn can_do(conversion: &Conversion, roster: &Roster) -> bool {
    conversion.trades.iter().any(|&t| roster.count(t) > 0)
}

/// What a batch's inputs are worth at their values, in emeralds (`Workshop.inputsWorth`).
pub fn inputs_worth(content: &Content, conversion: &Conversion) -> f64 {
    conversion.inputs.iter().map(|&(g, n)| n * content.goods[g].value).sum()
}

/// What a batch earns for each emerald of its inputs' worth, its output worth `each` apiece (`Workshop.gain`).
pub fn gain(content: &Content, conversion: &Conversion, each: f64) -> f64 {
    conversion.out * each / inputs_worth(content, conversion).max(1e-9)
}

/// What the town's workshop keeps back from its lockers (`lockers.rs`), by good: of each input, what the comforts its
/// trades can make and it is short of would take to bring each up to its par, so its lockers don't buy up the paper
/// its books need: as many batches as the other inputs it holds would make (no leather, no paper kept for books).
/// Nothing of a comfort it has met.
pub fn keeps(content: &Content, roster: &Roster, people: &People, stock: &Stock) -> Vec<f64> {
    let mut keep: Vec<f64> = vec![0.0; content.goods.len()];
    for c in content.conversions.iter().filter(|c| can_do(c, roster)) {
        let Some(kind) = content.goods[c.output].comfort else { continue };
        if people.comfort_met.get(kind).copied().unwrap_or(1.0) >= 1.0 {
            continue;
        }
        let wanted = (comfort_wanted(content, people, stock, kind, c.output) / c.out).ceil();
        for &(g, n) in &c.inputs {
            let others = c.inputs.iter().filter(|&&(h, _)| h != g)
                .map(|&(h, m)| (stock.0.get(h).copied().unwrap_or(0.0) / m).floor()).fold(f64::INFINITY, f64::min);
            keep[g] = keep[g].max(wanted.min(others) * n);
        }
    }
    keep
}

/// How many more of a comfort the town wants: its par less what it holds of the kind, in items (`comfortWanted`).
pub fn comfort_wanted(content: &Content, people: &People, stock: &Stock, kind: usize, output: GoodId) -> f64 {
    let short = comfort_par(content, people) - comfort_held(content, stock, kind);
    if short <= 0.0 { 0.0 } else { (short / content.goods[output].value.max(1e-9) - 1e-9).ceil() }
}

/// How many more of `output` the town wants (`Workshop.potential`'s `wanted`): a comfort up to the town's par, and as
/// much again for each of `routes_out`; food while it holds under twice the food par (a fifth of the points short,
/// as bread and cooked cod are five points apiece); anything else up to the par a locker buys it at, less what it
/// holds.
pub fn wanted(content: &Content, output: GoodId, people: &People, stock: &Stock, routes_out: usize, locker_par: f64) -> f64 {
    let g = &content.goods[output];
    if let Some(kind) = g.comfort {
        return comfort_wanted(content, people, stock, kind, output) * (1 + routes_out) as f64;
    }
    if g.food > 0.0 {
        return whole(((2.0 * FOOD_PAR_POINTS - food_points(content, stock)) / g.food.max(1e-9)).max(0.0));
    }
    if locker_par <= 0.0 {
        return 0.0;
    }
    (locker_par - stock.0[output]).max(0.0).ceil()
}

/// Is it food made of no crop: what a town's herds and nets leave (cooked beef, mutton, cod)?
pub fn by_product_food(content: &Content, conversion: &Conversion) -> bool {
    content.goods[conversion.output].food > 0.0 && conversion.inputs.iter().all(|&(g, _)| !content.goods[g].crop)
}

/// Converts as many whole batches as cover `wanted`, as far as the inputs go; returns the items made. Inputs are
/// `used` and outputs `made` in the ledger (`Workshop.run`).
pub fn run(conversion: &Conversion, wanted: f64, stock: &mut Stock, ledger: &mut Ledger) -> f64 {
    let mut batches = (wanted / conversion.out - 1e-9).ceil();
    for &(g, n) in &conversion.inputs {
        batches = batches.min(whole(stock.0[g] / n));
    }
    if batches <= 0.0 {
        return 0.0;
    }
    for &(g, n) in &conversion.inputs {
        stock.0[g] -= batches * n;
        ledger.used[g] += batches * n;
    }
    let made = batches * conversion.out;
    stock.0[conversion.output] += made;
    ledger.made[conversion.output] += made;
    made
}

/// One conversion's potential this touch: what is wanted of its output (`Workshop.potential`).
pub struct Potential {
    pub conversion: usize,
    pub wanted: f64,
}

/// A town's workshop on touch (`Workshop.convert`): the conversions its roster's trades can do, ranked by what they
/// earn per emerald of inputs, each run as far as its output is wanted. What is wanted is reckoned for all of them
/// before any runs. Returns the items made.
pub fn convert(content: &Content, roster: &Roster, people: &People, stock: &mut Stock, ledger: &mut Ledger,
        routes_out: usize, hooks: Option<&Workshop>, far: Option<&[f64]>) -> f64 {
    let mut ranked: Vec<(Potential, f64)> = content.conversions.iter().enumerate()
        .filter(|(_, c)| can_do(c, roster))
        .map(|(i, c)| {
            let par = hooks.map_or(0.0, |h| h.par_of(c.output));
            // What the far ends want and lack of it, on top of its own lockers' par (`rules/workshop.rs`).
            let far = far.and_then(|f| f.get(c.output)).copied().unwrap_or(0.0);
            let w = if by_product_food(content, c) {
                // Food of what the herds leave (beef, mutton, cod): kept at twice the food par in its own points,
                // whatever else feeds the town, so it's in the bag its people eat from (`people.rs`); not as the mod.
                let held = stock.0[c.output] * content.goods[c.output].food;
                whole(((2.0 * FOOD_PAR_POINTS - held) / content.goods[c.output].food).max(0.0))
            } else {
                wanted(content, c.output, people, stock, routes_out, par + far)
            };
            (Potential { conversion: i, wanted: w }, gain(content, c, content.goods[c.output].value))
        })
        .collect();
    // Most gain first; ties keep the list's order (the sort is stable).
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
    let mut made = 0.0;
    for (p, _) in ranked {
        let c = &content.conversions[p.conversion];
        // Not the crop set aside for its forward sales: the bakers leave the wheat it owes.
        let (first, n) = c.inputs[0];
        let kept = hooks.map_or(0.0, |h| h.aside_of(first));
        let free = stock.0[first] - kept;
        let w = if kept <= 0.0 { p.wanted } else { p.wanted.min(whole(free.max(0.0) / n) * c.out) };
        made += run(c, w, stock, ledger);
    }
    made
}

/// What a town's workshop draws of one good a day, and what an item of it earns there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Demand {
    pub good: GoodId,
    pub per_day: f64,
    pub each: f64,
}

/// What the town's workshop would draw a day of each input to keep up the comforts its trades make, and what an item
/// of it earns there (`Workshop.inputDemand`): each comfort at the town's ration (`villagers * ration`, and as much
/// again for each route out), in the inputs that make it, and back through what its own trades make of those (books
/// from paper, paper from cane). An input earns its share of the output's worth, by the inputs' worth; where two
/// outputs draw on one input it keeps the higher worth. A comfort the town is short of is worth what a short town
/// pays for it (`desires::will_pay`'s eagerness: up to `desire_most` times its worth with none of its kind met, by
/// `met`, the town's comforts met by kind; none given, its worth), so its inputs outrank a locker's crops while its
/// people go without (not as the mod, whose ranking takes the plain worth). Its farmers grow for this as for any
/// demand. By good.
pub fn input_demand(content: &Content, roster: &Roster, villagers: f64, routes_out: usize, met: &[f64]) -> Vec<Demand> {
    let towns = (1 + routes_out) as f64;
    let mut out: Vec<Demand> = Vec::new();
    // The frontier: (conversion, items of its output made a day, worth of one).
    let mut frontier: Vec<(usize, f64, f64)> = Vec::new();
    for (i, c) in content.conversions.iter().enumerate() {
        if let Some(kind) = content.goods[c.output].comfort.filter(|_| can_do(c, roster)) {
            let value = content.goods[c.output].value;
            let eager = 1.0 + (content.dials.desire_most - 1.0) * (1.0 - met.get(kind).copied().unwrap_or(1.0).clamp(0.0, 1.0));
            frontier.push((i, villagers * content.dials.ration * towns / value.max(1e-9), value * eager));
        }
    }
    // Two steps back: A d, then A (A d).
    for _ in 0..2 {
        let mut next = Vec::new();
        for (i, made, each) in frontier {
            let c = &content.conversions[i];
            let batches = made / c.out;
            let worth = inputs_worth(content, c);
            for &(g, n) in &c.inputs {
                let per_day = batches * n;
                let share = c.out * each * content.goods[g].value / worth.max(1e-9);
                match out.iter_mut().find(|d| d.good == g) {
                    Some(d) => {
                        d.per_day += per_day;
                        d.each = d.each.max(share);
                    }
                    None => out.push(Demand { good: g, per_day, each: share }),
                }
                if let Some(j) = content.conversions.iter().position(|m| m.output == g && can_do(m, roster)) {
                    next.push((j, per_day, share));
                }
            }
        }
        frontier = next;
    }
    out.sort_by_key(|d| d.good);
    out
}

impl Society {
    /// Adds a conversion by names (see [`conversion`]); false if the host lacks a good or every trade of it.
    pub fn add_conversion(&mut self, trades: &[&str], inputs: &[(&str, f64)], output: &str, out: f64) -> bool {
        let c = conversion(self.content(), trades, inputs, output, out);
        match c {
            Some(c) => {
                self.content_mut().conversions.push(c);
                true
            }
            None => false,
        }
    }

    /// Sets the mod's seven conversions again from the goods the host has now (after it added goods or trades).
    pub fn rebuild_conversions(&mut self) {
        let list = mod_conversions(self.content());
        self.content_mut().conversions = list;
    }

    fn workshop_hooks(&mut self, town: usize) -> Mut<'_, Workshop> {
        let n = self.content().goods.len();
        let mut hooks = self.world.get_mut::<Workshop>(self.towns[town]).expect("a town has its workshop");
        hooks.aside.resize(n, 0.0);
        hooks.locker_par.resize(n, 0.0);
        hooks
    }

    /// Stock of `good` the town sold forward (harvest-and-rents' hook): its bakers leave this much alone.
    pub fn set_aside(&mut self, town: usize, good: GoodId, amount: f64) {
        self.workshop_hooks(town).aside[good] = amount.max(0.0);
    }

    /// The par a locker buys `good` up to in the town (0: none does), so its workshop makes it up to that.
    pub fn set_locker_par(&mut self, town: usize, good: GoodId, par: f64) {
        self.workshop_hooks(town).locker_par[good] = par.max(0.0);
    }

    /// What the town's workshop would draw of each input a day, and what an item earns there (see [`input_demand`]).
    pub fn input_demand(&self, town: usize) -> Vec<Demand> {
        let e = self.towns[town];
        let roster = self.roster(town);
        let people = self.world.get::<People>(e).expect("a town");
        input_demand(self.content(), &roster, people.villagers, self.routes_out(town), &people.comfort_met)
    }

    /// Routes that start at the town: where its comforts can be sold.
    pub fn routes_out(&self, town: usize) -> usize {
        self.routes.iter().filter(|&&r| self.world.get::<crate::state::Route>(r).is_some_and(|r| r.from_id == town)).count()
    }
}
