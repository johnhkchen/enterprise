//! Squads: who crews a town's wagon (`docs/decisions.md` "Caravans are staffed by squads", "Towns train their squads",
//! "A squad is never more than 4"). One to four trained hands of its home town go with a wagon: a lead to drive it,
//! and each more only when its trade helps this trip (and the town can spare its road food beyond the caravans' par);
//! hands without a trade go as porters when there's more cargo than the wagon and the packs carry, and walk home once
//! it's delivered. The crew leave the roster as `away` while on the road and come back with the wagon. A town whose
//! routes have no lead takes on a hand waiting for work as a travelling Merchant; a town with routes trains one lead
//! a day, up to a lead a route and one spare. Saddles in stock make a wagon ride a third faster.
//!
//! Only for towns that keep a roster of hands ([`crewed`]): a town without one sends its one-mouth wagon as before.
//!
//! Ported from:
//!
//! - `caravan/Squad.java`: `MAX`, `PACK_STACKS`, `SIGNING_EMERALDS`, `Needs` (`wants`, `ALL`, the old three),
//!   `eligible` (130: an adult of a squad trade, or one without a trade, not the desk's Merchant, nor one working a
//!   locker), `available` (208: in `TRADES` order, most experienced first), `order`, `muster` (148), `leads`, `hire`
//!   (229), `porter`, `carries`.
//! - `caravan/Commissions.java`: `train` (99), `squadShort` (81), `TRAINING_EMERALDS`.
//! - `caravan/Caravans.java` `dispatch` 210–215 (no Merchant at the hub, no caravan), 223–303 (route by route as it
//!   leaves: the far end's wanted stacks, porters and packs, extras keeping a lead for every route still without a
//!   caravan, hiring when nobody leads, the porter run, the food trim), 380 (slots), 528–537
//!   (the saddle's pace and wear); `sendPortersHome` (1136); `porterRun` (1299); `arriveHome` (the squad released);
//!   `Caravan.mouths`, `Caravan.capacity`.
//! - `caravan/CaravanLedger.java` `walkHome` (42), `walkersHome`, `arrived`.
//! - `territory/Territory.java` `grow` 1614: the squad trains before anything, and the idle find work
//!   (`Jobs.employ`) only when it trained nobody and isn't short ([`hiring_waits`]).
//!
//! A hand on the road is known by an id its town stamps it with as it leaves ([`stamp`], as the mod knows a villager by
//! its UUID): its place in the roster shifts while it's away, as settlers leave (`settlers::take_hand`).
//!
//! The core's hands have no workstations: a hand "works a locker" when the desk's Merchant manages its locker and one
//! of its offers is switched on (`lockers.rs`); a hand trained or taken on for a squad keeps its trade with no
//! offers, as the mod's keeps it without a workstation. Not here yet: what a trip wants of the squad's services
//! (`Services.wantedAbroad`: stale books, an unmapped road, animals to drive, a blessing...) is a hook a host or a
//! later slice fills ([`Society::want_trades`]); by water (a Fisherman captains a boat) is in [`Needs`] but no route
//! is by water yet; llamas, herds and the fishing fleet's catch; the haggling level is still home's Merchant's
//! (`bargain.rs`), not the squad's best Merchant's (`Squad.haggleLevel`).

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::content::{Content, GoodId};
use crate::lockers::Lockers;
use crate::messages::{RouteId, TownId};
use crate::roster::{Hand, Roster, TradeId, LEADS, NITWIT, NONE, SQUAD};
use crate::society::Society;
use crate::state::*;

/// A squad is never more than this (`Squad.MAX`).
pub const MAX: usize = 4;
pub const FISHERMAN: &str = "fisherman";
pub const LEATHERWORKER: &str = "leatherworker";
pub const SHEPHERD: &str = "shepherd";
pub const LIBRARIAN: &str = "librarian";
pub const CARTOGRAPHER: &str = "cartographer";
/// The good a saddle is, by name (`Items.SADDLE`).
pub const SADDLE: &str = "saddle";

/// The day towns last trained their squads (`Commissions.train`, once a count: here once a day), and the day each town
/// last trained a hand (its hiring waits that day: `Territory.grow`).
#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Squads {
    pub trained: Option<u64>,
    #[serde(default)]
    pub trained_hand: std::collections::BTreeMap<TownId, u64>,
}

/// Does a town's hiring wait for its squad today (`Territory.grow`: `Jobs.employ` only when `Commissions.train` trained
/// nobody and the squad isn't short)? It trained a hand today, or it has `routes` of its own, a Merchant at its desk to
/// train one, and its squad is short of leads.
pub fn hiring_waits(content: &Content, squads: &Squads, town: TownId, today: u64, roster: &Roster,
        lockers: Option<&Lockers>, routes: usize) -> bool {
    squads.trained_hand.get(&town) == Some(&today)
        || crewed(Some(roster)) && roster.merchant.is_some() && squad_short(content, roster, lockers, routes)
}

/// A squad member walking home on its own, a porter whose load is delivered (`CaravanLedger.Walker`): home at
/// `arrives`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Walker {
    pub hand: Hand,
    pub arrives: u64,
}

/// What a trip has a use for beyond its lead (`Squad.Needs`): the trades whose work is wanted there, by water (a
/// Fisherman captains the boat), and porters for cargo beyond the wagon and the packs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Needs {
    pub wanted: Vec<TradeId>,
    pub water: bool,
    pub porters: usize,
}

impl Needs {
    /// The old three (`Needs(books, mapping, animals)`): stale books (a Librarian), an unmapped road (a Cartographer),
    /// animals to drive (a Shepherd).
    pub fn of(content: &Content, books: bool, mapping: bool, animals: bool) -> Self {
        let wanted = [(books, LIBRARIAN), (mapping, CARTOGRAPHER), (animals, SHEPHERD)]
            .into_iter().filter(|(w, _)| *w).filter_map(|(_, t)| content.trade(t)).collect();
        Needs { wanted, water: false, porters: 0 }
    }

    /// Every trade, by water, and a squad's worth of porters (`Needs.ALL`).
    pub fn all(content: &Content) -> Self {
        Needs { wanted: SQUAD.iter().filter_map(|t| content.trade(t)).collect(), water: true, porters: MAX }
    }

    /// Does the trip want a hand of `trade` (`Needs.wants`)?
    pub fn wants(&self, content: &Content, trade: TradeId) -> bool {
        self.wanted.contains(&trade) || self.water && content.trade(FISHERMAN) == Some(trade)
    }
}

fn named(content: &Content, trade: TradeId) -> &str {
    content.trades.get(trade).map_or("", |t| t.name.as_str())
}

/// Where a trade comes in the squad's order (`Squad.order`): the squad trades in `TRADES` order, then Nitwits, then
/// hands with no trade.
pub fn order(content: &Content, trade: TradeId) -> usize {
    let name = named(content, trade);
    SQUAD.iter().position(|t| *t == name).unwrap_or(if name == NITWIT { SQUAD.len() } else { SQUAD.len() + 1 })
}

/// A hand without a trade (a Nitwit, or an adult waiting for work) can carry: a porter (`Squad.porter`).
pub fn porter(content: &Content, trade: TradeId) -> bool {
    content.trades.get(trade).is_some_and(|t| t.idle())
}

/// Does a hand of this trade carry cargo beyond the wagon: a Leatherworker's packs, or a porter's back
/// (`Squad.carries`)?
pub fn carries(content: &Content, trade: TradeId) -> bool {
    named(content, trade) == LEATHERWORKER || porter(content, trade)
}

/// Can a hand of this trade lead a squad (`Squad.LEADS`)?
pub fn leads(content: &Content, trade: TradeId) -> bool {
    content.trades.get(trade).is_some_and(|t| t.leads)
}

/// Does the town keep a roster of hands? Only then are its wagons crewed; a town without sends its one-mouth wagon.
pub fn crewed(roster: Option<&Roster>) -> bool {
    roster.is_some_and(|r| !r.hands.is_empty())
}

/// Does the hand at `i` work a locker: one the desk's Merchant manages, with an offer switched on (the market's trade
/// goes on through it)?
pub fn works_a_locker(lockers: Option<&Lockers>, i: usize) -> bool {
    lockers.is_some_and(|l| l.managed.contains(&i) && l.hands.get(i).is_some_and(|h| h.switched_on()))
}

/// Can the hand at `i` go with a caravan (`Squad.eligible`)? An adult at home of a squad trade or none, not the desk's
/// Merchant, nor one working a locker.
pub fn eligible(content: &Content, roster: &Roster, lockers: Option<&Lockers>, i: usize) -> bool {
    let Some(h) = roster.hands.get(i) else { return false };
    let Some(t) = content.trades.get(h.trade) else { return false };
    !h.is_baby() && !h.away && (t.squad || t.idle()) && roster.merchant != Some(i) && !works_a_locker(lockers, i)
}

/// The hands who could go, in the squad's order, most experienced first, then the roster's order
/// (`Squad.available`).
pub fn available(content: &Content, roster: &Roster, lockers: Option<&Lockers>) -> Vec<usize> {
    let mut out: Vec<usize> = (0..roster.hands.len()).filter(|&i| eligible(content, roster, lockers, i)).collect();
    out.sort_by_key(|&i| (order(content, roster.hands[i].trade), std::cmp::Reverse(roster.hands[i].level), i));
    out
}

/// Of these hands, those who could lead a squad (`Squad.leads`).
pub fn leaders(content: &Content, roster: &Roster, hands: &[usize]) -> Vec<usize> {
    hands.iter().copied().filter(|&i| leads(content, roster.hands[i].trade)).collect()
}

/// Who would go (`Squad.muster`), of `candidates` (as [`available`] orders them): a lead (by water the first
/// Fisherman; else the first hand of a leading trade; none, no squad: a Cleric goes along but never drives the
/// wagon); then one of each trade the trip wants, in `TRADES` order, while the squad is no more than `extras` and
/// less than [`MAX`]; then porters (Nitwits first) up to `porters`, at most `min(MAX, extras + 1)` in all; by water,
/// every Fisherman to spare up to that.
pub fn muster(content: &Content, roster: &Roster, candidates: &[usize], needs: &Needs, extras: usize) -> Vec<usize> {
    let trade = |i: usize| roster.hands[i].trade;
    let mut squad: Vec<usize> = Vec::new();
    let fisherman = content.trade(FISHERMAN);
    if needs.water {
        if let Some(&i) = candidates.iter().find(|&&i| Some(trade(i)) == fisherman) {
            squad.push(i);
        }
    }
    if squad.is_empty() {
        match candidates.iter().find(|&&i| leads(content, trade(i))) {
            Some(&i) => squad.push(i),
            None => return squad,
        }
    }
    for name in SQUAD {
        let Some(t) = content.trade(name) else { continue };
        if squad.len() > extras || squad.len() >= MAX || !needs.wants(content, t) || squad.iter().any(|&i| trade(i) == t) {
            continue;
        }
        if let Some(&i) = candidates.iter().find(|&&i| trade(i) == t) {
            squad.push(i);
        }
    }
    let cap = MAX.min(extras + 1);
    let room = needs.porters.min(cap.saturating_sub(squad.len()));
    let porters: Vec<usize> = candidates.iter().copied()
        .filter(|&i| porter(content, trade(i)) && !squad.contains(&i)).take(room).collect();
    squad.extend(porters);
    if needs.water {
        let room = cap.saturating_sub(squad.len());
        let fleet: Vec<usize> = candidates.iter().copied()
            .filter(|&i| Some(trade(i)) == fisherman && !squad.contains(&i)).take(room).collect();
        squad.extend(fleet);
    }
    squad
}

/// The stacks a wagon with this crew carries (`Caravan.capacity`): `wagon_slots`, and `pack_stacks` more for each
/// Leatherworker and porter; a porter run, one pack.
pub fn capacity(content: &Content, crew: impl IntoIterator<Item = TradeId>, porter_run: bool) -> u32 {
    let d = &content.dials;
    if porter_run {
        return d.pack_stacks;
    }
    d.wagon_slots + d.pack_stacks * crew.into_iter().filter(|&t| carries(content, t)).count() as u32
}

/// Porters a load of `stacks` wants beyond the wagon and, if a Leatherworker is at home, its packs (`dispatch`):
/// `ceil((stacks - wagon_slots - packs) / pack_stacks)`, none for a load the wagon holds.
pub fn porters_needed(content: &Content, stacks: u32, packs: bool) -> usize {
    let d = &content.dials;
    if stacks <= d.wagon_slots || d.pack_stacks == 0 {
        return 0;
    }
    let beyond = stacks as f64 - d.wagon_slots as f64 - if packs { d.pack_stacks as f64 } else { 0.0 };
    (beyond / d.pack_stacks as f64).ceil().max(0.0) as usize
}

/// The goods the far end wants (`Caravans.wantedThere`, as far as the core knows it): what its people use or its
/// lockers take (a trade it has switched on, managed or not: `far_lockers`, with its roster), comforts aside.
pub fn wanted_there(content: &Content, far: &Trades, far_lockers: Option<(&crate::lockers::Lockers, &Roster)>)
        -> Vec<GoodId> {
    let n = content.goods.len();
    let taken = far_lockers.map(|(l, r)| l.taken(content, r, n));
    (0..n)
        .filter(|&g| content.goods[g].comfort.is_none()
            && (far.uses[g] > 0.0 || far.capacity.get(g).is_some_and(|c| *c > 0.0)
                || far.inputs.get(g).is_some_and(|n| *n > 0.0)
                || taken.as_ref().is_some_and(|t| t[g])))
        .collect()
}

/// Stacks of the goods the far end wants that home holds (`Caravans.wantedStacks`: 64 to a stack, whatever the good).
pub fn wanted_stacks(content: &Content, home: &[f64], far: &Trades,
        far_lockers: Option<(&crate::lockers::Lockers, &Roster)>) -> u32 {
    // (`wanted_there`'s goods, asked only of those home holds a whole one of: the rest add no stack.)
    let mut taken: Option<Vec<bool>> = None;
    let mut stacks = 0;
    for g in 0..content.goods.len() {
        let n = (home.get(g).copied().unwrap_or(0.0).max(0.0).floor() / 64.0).ceil() as u32;
        if n == 0 || content.goods[g].comfort.is_some() {
            continue;
        }
        let wanted = far.uses[g] > 0.0 || far.capacity.get(g).is_some_and(|c| *c > 0.0)
            || far.inputs.get(g).is_some_and(|n| *n > 0.0)
            || far_lockers.is_some_and(|(l, r)| taken.get_or_insert_with(|| l.taken(content, r, content.goods.len()))[g]);
        if wanted {
            stacks += n;
        }
    }
    stacks
}

/// The hand a town with routes and nobody to lead them takes on as a travelling Merchant (`Squad.hire`): the first
/// adult waiting for work (not a Nitwit: it can't learn a trade) among those who could go, while the leads at home
/// and those out on the road are fewer than its routes. The caller pays the signing.
pub fn to_hire(content: &Content, roster: &Roster, home: &[usize], routes: usize, leads_out: usize) -> Option<usize> {
    if leaders(content, roster, home).len() + leads_out >= routes {
        return None;
    }
    let none = content.trade(NONE)?;
    home.iter().copied().find(|&i| roster.hands[i].trade == none)
}

/// A hand takes a squad trade (taken on, or trained): a Novice that keeps it with no workstation, so no locker
/// (`setLevel(1)`, `setVillagerXp(max(1, xp))`).
pub fn sign_on(content: &Content, roster: &mut Roster, lockers: Option<&mut Lockers>, i: usize, trade: TradeId) {
    let h = &mut roster.hands[i];
    h.trade = trade;
    h.level = 1;
    h.xp = h.xp.max(1);
    if let Some(l) = lockers {
        l.sync(content, roster);
        if let Some(locker) = l.hands.get_mut(i) {
            locker.offers.clear();
        }
        l.sync(content, roster);
    }
}

/// The leads a town's squad has: those at home who could go, and those away on the road (`Commissions.train`'s and
/// `squadShort`'s count).
pub fn leads_had(content: &Content, roster: &Roster, lockers: Option<&Lockers>) -> Vec<TradeId> {
    let home = available(content, roster, lockers);
    let mut out: Vec<TradeId> = leaders(content, roster, &home).into_iter().map(|i| roster.hands[i].trade).collect();
    out.extend(roster.hands.iter().filter(|h| h.away && !h.is_baby() && leads(content, h.trade)).map(|h| h.trade));
    out
}

/// Does a town with `routes` of its own still lack leads (`Commissions.squadShort`): fewer than one a route and a
/// spare, at most [`MAX`]?
pub fn squad_short(content: &Content, roster: &Roster, lockers: Option<&Lockers>, routes: usize) -> bool {
    routes > 0 && leads_had(content, roster, lockers).len() < MAX.min(routes + 1)
}

/// Whom a town with `routes` trains, and in what (`Commissions.train`): while its squad is short, the first adult at
/// home waiting for work (no trade; not a Nitwit) learns the first trade of `LEADS` the squad lacks (all had: the
/// next in turn). The caller pays the training.
pub fn to_train(content: &Content, roster: &Roster, lockers: Option<&Lockers>, routes: usize) -> Option<(usize, TradeId)> {
    if !squad_short(content, roster, lockers, routes) {
        return None;
    }
    let had = leads_had(content, roster, lockers);
    let none = content.trade(NONE)?;
    let pupil = (0..roster.hands.len()).find(|&i| {
        let h = &roster.hands[i];
        h.trade == none && !h.is_baby() && !h.away && roster.merchant != Some(i)
    })?;
    let trade = LEADS.iter().filter_map(|t| content.trade(t)).find(|t| !had.contains(t))
        .or_else(|| content.trade(LEADS[had.len() % LEADS.len()]))?;
    Some((pupil, trade))
}

/// The hands of a crew back home: each marked at home again in its town's roster, the away hand with the id it left
/// with ([`stamp`]; a hand that left the roster meanwhile frees nobody in its place).
pub fn release(roster: &mut Roster, crew: &[Hand]) {
    for hand in crew {
        if let Some(h) = roster.hands.iter_mut().find(|h| h.away && h.id == hand.id) {
            h.away = false;
        }
    }
}

/// A hand leaves with a caravan: away, with an id none of its town's hands has (the mod knows a villager by its UUID;
/// a hand's place in the roster shifts as others leave it, `settlers::take_hand`). Returns the hand as it left.
pub fn stamp(roster: &mut Roster, i: usize) -> Option<Hand> {
    let id = roster.hands.iter().map(|h| h.id).max().unwrap_or(0) + 1;
    let h = roster.hands.get_mut(i)?;
    h.away = true;
    h.id = id;
    Some(h.clone())
}

/// The ticks a leg takes a wagon, `slow` times its pace's (short rations: 2): a saddled wagon a third faster
/// (`travelTicks / pace`).
pub fn leg_ticks(content: &Content, travel_ticks: u64, saddled: bool, slow: f64) -> u64 {
    if saddled {
        (travel_ticks as f64 / content.dials.saddled_pace * slow) as u64
    } else {
        (travel_ticks as f64 * slow) as u64
    }
}

/// A route's crew as a host sees it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CrewView {
    /// The route's home town.
    pub home: TownId,
    /// Its wagon's crew on the road, by trade and level (empty while it's home, or for a town without a roster).
    pub crew: Vec<(String, u8)>,
    /// The stacks its wagon carries, and whether it's a porter run (one hand on foot with a pack).
    pub slots: u32,
    pub porter: bool,
    pub saddled: bool,
    /// When its wagon gets where it's going, if it's on the road.
    pub arrives: Option<u64>,
    /// Porters walking home from its far end, and when the first gets there.
    pub walkers: usize,
    pub walker_home_at: Option<u64>,
    pub porters_sent: u32,
    pub saddle_wear: f64,
    /// The stacks its wagon brought home last trip (`lastHome`).
    pub last_home: Option<u32>,
}

impl Society {
    /// A route's crew: its wagon's squad on the road, its slots, its porters walking home.
    pub fn crew(&self, route: RouteId) -> CrewView {
        let s = self.world.get::<RouteState>(self.routes[route]).expect("a route");
        let content = self.content();
        let w = s.wagon.as_ref();
        let home = self.world.get::<Route>(self.routes[route]).expect("a route").from_id;
        CrewView {
            home,
            crew: w.map_or_else(Vec::new, |w| w.crew.iter().map(|h| (named(content, h.trade).to_string(), h.level)).collect()),
            slots: w.map_or(0, |w| w.slots),
            porter: w.is_some_and(|w| w.porter),
            saddled: w.is_some_and(|w| w.saddled),
            arrives: w.map(|w| w.arrives),
            walkers: s.walkers.len(),
            walker_home_at: s.walkers.iter().map(|w| w.arrives).min(),
            porters_sent: s.porters_sent,
            saddle_wear: s.saddle_wear,
            last_home: s.last_home,
        }
    }

    /// The trades whose work a route's far end wants this trip (`Services.wantedAbroad`: stale books, an unmapped
    /// road...), by name: a hand of each goes along if the town can spare it.
    pub fn want_trades(&mut self, route: RouteId, trades: &[&str]) {
        let wanted: Vec<TradeId> = trades.iter()
            .map(|t| self.content().trade(t).unwrap_or_else(|| panic!("no trade called {t}"))).collect();
        self.world.get_mut::<RouteState>(self.routes[route]).expect("a route").wanted_trades = wanted;
    }

    /// The town trains a hand for its squad now, as its daily review does (`Commissions.train`): the hand trained, if
    /// one was.
    pub fn train_squad(&mut self, town: TownId) -> Option<usize> {
        let e = self.towns[town];
        let routes = self.routes.iter().filter(|&&r| self.world.get::<Route>(r).is_some_and(|r| r.from_id == town)).count();
        let now = self.now();
        let mut roster = self.world.get::<Roster>(e)?.clone();
        let mut lockers = self.world.get::<Lockers>(e).cloned();
        let money = self.world.get::<Money>(e)?.0;
        let free = self.free(town);
        let content = self.content().clone();
        let mut ledger = std::mem::take(&mut *self.world.resource_mut::<Ledger>());
        let mut book = std::mem::take(&mut *self.world.resource_mut::<crate::cash_book::CashBook>());
        let mut money = Money(money);
        let out = crate::rules::squad::train_town(&content, &mut ledger, &mut book, now, town, routes, free, &mut roster,
            lockers.as_mut(), &mut money);
        *self.world.resource_mut::<Ledger>() = ledger;
        *self.world.resource_mut::<crate::cash_book::CashBook>() = book;
        if out.is_some() {
            let today = now / self.content().day_ticks.max(1);
            self.world.resource_mut::<Squads>().trained_hand.insert(town, today);
        }
        let mut ent = self.world.entity_mut(e);
        ent.insert((roster, money));
        if let Some(l) = lockers {
            ent.insert(l);
        }
        out
    }

    /// The trade a hand of the town has now, by name (and whether it's away).
    pub fn hand(&self, town: TownId, i: usize) -> Option<(String, u8, u32, bool)> {
        let roster = self.world.get::<Roster>(self.towns[town])?;
        let h = roster.hands.get(i)?;
        Some((named(self.content(), h.trade).to_string(), h.level, h.xp, h.away))
    }
}

