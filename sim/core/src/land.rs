//! The host's land as the core's map and fields, and a town's works as things the host lays in its world
//! (`docs/binding-plan.md` 2, concern 7).
//!
//! The mod surveys its world for the society (`territory/Terrain.java`, `territory/Crops.java`) and lays what a town
//! builds (`territory/Works.java`'s `lay` and `raise`, `Buildings.build`, `Fields.plant`). Here the host does both and
//! the core keeps the rest:
//!
//! - **Ground** (`Input::Ground`): a hex's share under water, its relief and what its land gives, as the host surveyed
//!   them, into the [`Map`](crate::map::Map) caravans plan over, bridges are weighed on and hands gather from. A bridge
//!   the core finished stays.
//! - **Crops** (`Input::Crops`): the plots the host counted in one chunk's part of a hex, kept by chunk ([`LandBook`]),
//!   so a chunk not loaded keeps its last count (as `Territory.cropsIn` keeps it). A town's fields are the plots
//!   counted in the hexes it holds (its home hex before its first count, if nobody holds it) with the fields it planted
//!   that the host hasn't laid yet: set on its farm at its `Survey` ([`Society::set_fields`]), which sets the farm up.
//! - **Survey** (`Input::Survey`): the town's bell (where its land grows from, `Territory.grow`) and that its ground is
//!   loaded this look: at the end of the look its works to lay come out as [`Output::Build`] ([`Society::show_land`]).
//! - **Works shown** (`Output::Build`, `Input::Built`): a bridge or a hall once its masons have finished it, a wall a
//!   quarter at a time as they lay it (`Works.onServerTick`: "a quarter at a time, and the last stretch when done"), a
//!   field the town planted (the farm's review, `Fields.consider`, or a household's, `freehold`) as soon as it's
//!   planted, a home its households built (`people.rs`: two beds, `Growth.buildHome`) as soon as it's paid for. A
//!   home not yet laid still counts its beds: a census adds them to the beds it found ([`LandBook::homes`]). Each is asked for at every look that surveys its town until the host says it has laid it, so nothing
//!   waits in the host and a town unloaded when its work finished is shown it when it loads. Where a hall or a field
//!   goes is the host's to find (`Buildings.site`, `Growth.farmSite`): the core counts their blocks, not their place.
//!
//! A field the town planted is seen in the plots around an `Advance` through the wire ([`Society::note_fields`]: the
//! plots each crop gained, a field's plots each), so the rules that plant stay as they are.
//!
//! **Not as the mod:** the core keeps no wooden pier (`Territory.buildDock`) and no pastures' fences or mine heads
//! (their rules are the core's, `herds.rs` and `kit.rs`, with nothing laid in the host's world); a hall's or a field's
//! site isn't checked by the core, so a host that finds none leaves it asked for. A field is paid for (and its plots
//! are the farm's) when the rules plant it, before the host looks for its site, where `Fields.consider` finds the site
//! first: a field the host finds no room for is paid for, yields on the farm and is asked for at every survey.

use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::content::GoodId;
use crate::hex::Hex;
use crate::messages::TownId;
use crate::society::Society;
use crate::wire::{Count, GroundView, HexAt, Input, LandKind, Output, Work, WorkView};
use crate::works::{What, KINDS};

/// A chunk of the host's world, by its coordinates.
pub type ChunkAt = (i32, i32);

/// What the host has told the core of its land and seen of the towns' works.
#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
pub struct LandBook {
    /// The crop plots last counted by hex, by chunk, by crop.
    pub counted: BTreeMap<Hex, BTreeMap<ChunkAt, BTreeMap<GoodId, u32>>>,
    /// The fields each town planted that the host hasn't laid yet, by crop, in the order planted.
    pub unlaid: BTreeMap<TownId, Vec<GoodId>>,
    /// How much of each of a town's projects the host has shown (0 to 1), by what it is.
    pub shown: BTreeMap<TownId, Vec<(What, f64)>>,
    /// The homes each town built that the host hasn't laid yet.
    pub homes: BTreeMap<TownId, u32>,
    /// The berths each town paid for that the host hasn't laid yet.
    #[serde(default)]
    pub berths: BTreeMap<TownId, u32>,
}

impl LandBook {
    /// The beds of the homes a town built that the host hasn't laid yet: a census can't see them.
    pub fn unlaid_beds(&self, town: TownId) -> f64 {
        2.0 * self.homes.get(&town).copied().unwrap_or(0) as f64
    }

    fn shown(&self, town: TownId, what: &What) -> f64 {
        self.shown.get(&town).and_then(|s| s.iter().find(|(w, _)| w == what)).map_or(0.0, |(_, share)| *share)
    }
}

/// How much of a project the host should show: a bridge or a hall once finished; a wall a quarter at a time as it's
/// laid, and whole when finished.
pub fn due(project: &crate::works::Project) -> f64 {
    if project.done {
        return 1.0;
    }
    match project.what {
        What::Wall => (project.progress() * 4.0).floor() / 4.0,
        _ => 0.0,
    }
}

/// The plots a town's farm had of each crop, for [`Society::note_fields`].
pub type PlotsNow = BTreeMap<TownId, BTreeMap<GoodId, u32>>;

impl Society {
    /// A work as the host is told of it.
    fn work(&self, what: &What) -> Work {
        match *what {
            What::Bridge { hex, from, to } => Work::Bridge { hex: HexAt::of(hex), from: HexAt::of(from), to: HexAt::of(to) },
            What::Wall => Work::Wall,
            What::Hall(kind) => Work::Hall { kind: kind.id().to_string() },
        }
    }

    /// The hexes whose crops are a town's: those it holds, or its home hex before its first count if nobody holds it.
    pub fn field_hexes(&self, town: TownId) -> Vec<Hex> {
        let held = self.holding(town).hexes;
        if !held.is_empty() {
            return held;
        }
        self.town_hex(town).filter(|&h| self.hex_owner(h).is_none()).into_iter().collect()
    }

    /// A town's fields: the plots counted in its hexes, and the fields it planted that the host hasn't laid.
    pub fn fields_of(&self, town: TownId) -> BTreeMap<GoodId, u32> {
        let book = self.world.resource::<LandBook>();
        let mut plots: BTreeMap<GoodId, u32> = BTreeMap::new();
        for hex in self.field_hexes(town) {
            for counts in book.counted.get(&hex).into_iter().flat_map(|c| c.values()) {
                for (&good, &n) in counts {
                    *plots.entry(good).or_insert(0) += n;
                }
            }
        }
        for &crop in book.unlaid.get(&town).into_iter().flatten() {
            *plots.entry(crop).or_insert(0) += crate::farms::field_plots(self.content(), crop);
        }
        plots
    }

    /// A town's farm set to its fields ([`Society::fields_of`]): each crop whose plots moved set anew, and the farm set
    /// up if it wasn't (its Farmers' plan made again either way, as at a census).
    pub fn set_fields(&mut self, town: TownId) {
        let fields = self.fields_of(town);
        let had = self.farm(town).map(|f| f.farm.plots);
        let crops: BTreeSet<GoodId> = fields.keys().chain(had.iter().flat_map(|p| p.keys())).copied().collect();
        let mut moved = false;
        for crop in crops {
            let now = fields.get(&crop).copied().unwrap_or(0);
            let before = had.as_ref().and_then(|p| p.get(&crop)).copied().unwrap_or(0);
            if now != before {
                self.set_plots(town, crop, now);
                moved = true;
            }
        }
        if !moved && had.is_none() {
            self.farm_review(town);
        }
    }

    /// Every set-up farm's plots now, for [`Society::note_fields`] after an `Advance`.
    pub fn plots_now(&self) -> PlotsNow {
        (0..self.town_count()).filter_map(|t| self.farm(t).map(|f| (t, f.farm.plots))).collect()
    }

    /// The fields the rules planted since `before` (`plots_now`), noted as fields for the host to lay: the plots each
    /// crop gained, a field's plots each (`Fields.plant`'s).
    pub fn note_fields(&mut self, before: &PlotsNow) {
        let mut planted: Vec<(TownId, GoodId)> = Vec::new();
        for t in 0..self.town_count() {
            let Some(farm) = self.farm(t) else { continue };
            for (&crop, &n) in &farm.farm.plots {
                let was = before.get(&t).and_then(|p| p.get(&crop)).copied().unwrap_or(0);
                if n > was {
                    let each = crate::farms::field_plots(self.content(), crop).max(1);
                    for _ in 0..(n - was).div_ceil(each) {
                        planted.push((t, crop));
                    }
                }
            }
        }
        let mut book = self.world.resource_mut::<LandBook>();
        for (t, crop) in planted {
            book.unlaid.entry(t).or_default().push(crop);
        }
    }

    /// What the host should lay of each surveyed town's works now (in the order given, each town once): its projects'
    /// due shares beyond what it has shown, in the order they were started, then the fields it planted that the host
    /// hasn't laid.
    pub fn show_land(&self, towns: &[TownId]) -> Vec<Output> {
        let mut seen = BTreeSet::new();
        let mut out = Vec::new();
        let book = self.world.resource::<LandBook>();
        for &t in towns {
            if t >= self.town_count() || !seen.insert(t) {
                continue;
            }
            for p in &self.works(t).projects {
                let share = due(p);
                if share > book.shown(t, &p.what) {
                    out.push(Output::Build { town: crate::wire::no(t), work: self.work(&p.what), share });
                }
            }
            for &crop in book.unlaid.get(&t).into_iter().flatten() {
                out.push(Output::Build { town: crate::wire::no(t), work: Work::Field { crop: crate::wire::no(crop) }, share: 1.0 });
            }
            for _ in 0..book.homes.get(&t).copied().unwrap_or(0) {
                out.push(Output::Build { town: crate::wire::no(t), work: Work::Home, share: 1.0 });
            }
            for _ in 0..self.unlaid_pastures(t) {
                out.push(Output::Build { town: crate::wire::no(t), work: Work::Pasture { grass: 0 }, share: 1.0 });
            }
        }
        out
    }

    /// The berths each town paid for that the host hasn't laid, surveyed or not: the host lays a waterfront's stretch as
    /// its shore loads, not its desk (seed 1717: four towns' berths, paid for while their desks weren't loaded, never
    /// asked of the host).
    pub fn show_berths(&self) -> Vec<Output> {
        let book = self.world.resource::<LandBook>();
        book.berths.iter()
            .flat_map(|(&t, &n)| (0..n).map(move |_| Output::Build { town: crate::wire::no(t), work: Work::Berth, share: 1.0 }))
            .collect()
    }

    /// The host laid `share` of a town's work: a project's shown share rises to it, a field planted is laid (the first
    /// of its crop). A work the town hasn't is a stale view: ignored.
    pub fn laid(&mut self, town: TownId, work: &Work, share: f64) {
        let share = share.clamp(0.0, 1.0);
        if *work == Work::Home || *work == Work::Berth {
            let mut book = self.world.resource_mut::<LandBook>();
            let unlaid = if *work == Work::Home { &mut book.homes } else { &mut book.berths };
            if let Some(n) = unlaid.get_mut(&town) {
                *n = n.saturating_sub(1);
                if *n == 0 {
                    unlaid.remove(&town);
                }
            }
            return;
        }
        if let Work::Pasture { grass } = *work {
            if let Some(mut herds) = self.towns.get(town).and_then(|&e| self.world.get_mut::<crate::herds::Herds>(e)) {
                if herds.unlaid() > 0 {
                    let laid = herds.laid() as usize;
                    herds.grass.truncate(laid);
                    herds.grass.push(grass);
                }
            }
            return;
        }
        if let Work::Field { crop } = *work {
            let mut book = self.world.resource_mut::<LandBook>();
            if let Some(unlaid) = book.unlaid.get_mut(&town) {
                if let Some(i) = unlaid.iter().position(|&c| c == crop as GoodId) {
                    unlaid.remove(i);
                }
                if unlaid.is_empty() {
                    book.unlaid.remove(&town);
                }
            }
            return;
        }
        let Some(what) = self.works(town).projects.iter().map(|p| p.what).find(|w| self.work(w) == *work) else { return };
        let mut book = self.world.resource_mut::<LandBook>();
        let shown = book.shown.entry(town).or_default();
        match shown.iter_mut().find(|(w, _)| *w == what) {
            Some((_, s)) => *s = s.max(share),
            None => shown.push((what, share)),
        }
    }

    /// The pastures a town fenced that the host hasn't laid yet.
    pub fn unlaid_pastures(&self, town: TownId) -> u32 {
        self.towns.get(town).and_then(|&e| self.world.get::<crate::herds::Herds>(e)).map_or(0, |h| h.unlaid())
    }

    /// A town's land, as `Query::Land` answers it.
    pub fn land_view(&self, town: TownId) -> crate::wire::Answer {
        let holding = self.holding(town);
        let book = self.world.resource::<LandBook>();
        let mut works: Vec<WorkView> = self.works(town).projects.iter().map(|p| WorkView {
            work: self.work(&p.what),
            laid: p.progress(),
            done: p.done,
            shown: book.shown(town, &p.what),
        }).collect();
        for &crop in book.unlaid.get(&town).into_iter().flatten() {
            works.push(WorkView { work: Work::Field { crop: crate::wire::no(crop) }, laid: 1.0, done: true, shown: 0.0 });
        }
        for _ in 0..book.homes.get(&town).copied().unwrap_or(0) {
            works.push(WorkView { work: Work::Home, laid: 1.0, done: true, shown: 0.0 });
        }
        for _ in 0..self.unlaid_pastures(town) {
            works.push(WorkView { work: Work::Pasture { grass: 0 }, laid: 1.0, done: true, shown: 0.0 });
        }
        for _ in 0..book.berths.get(&town).copied().unwrap_or(0) {
            works.push(WorkView { work: Work::Berth, laid: 1.0, done: true, shown: 0.0 });
        }
        let plots = self.farm(town).map(|f| f.farm.plots).unwrap_or_default().into_iter()
            .filter(|&(_, n)| n > 0)
            .map(|(g, n)| Count { good: crate::wire::no(g), count: n as f64 }).collect();
        crate::wire::Answer::Land {
            town: crate::wire::no(town),
            hexes: holding.hexes.iter().map(|&h| HexAt::of(h)).collect(),
            culture: holding.culture,
            next_hex: self.territory().next_hex_cost(town, self.content().dials.culture_per_hex),
            plots,
            works,
        }
    }

    /// A hex's ground, as `Query::Ground` answers it.
    pub fn ground_view(&self, hex: HexAt) -> GroundView {
        let g = self.map().ground(hex.hex());
        GroundView { hex, water: g.water_share, relief: g.relief, land: LandKind::of(g.land), bridged: g.bridged,
            height: g.height, river: g.river }
    }

    /// Applies a land happening (`Input::Ground`, `Crops`, `Survey`, `Built`, `Port`); false for any other. The wire checks
    /// them first (`wire::handle`).
    pub(crate) fn apply_land(&mut self, input: &Input) -> bool {
        match input {
            Input::Ground { hex, water, relief, land, height, river } => {
                self.set_ground(hex.hex(), *water, *relief);
                self.set_land(hex.hex(), land.land());
                let mut map = self.world.resource_mut::<crate::map::Map>();
                map.set_height(hex.hex(), *height);
                map.set_river(hex.hex(), *river);
            }
            Input::Crops { hex, chunk_x, chunk_z, counts } => {
                let mut plots: BTreeMap<GoodId, u32> = BTreeMap::new();
                for c in counts {
                    let n = c.count.max(0.0).min(u32::MAX as f64) as u32;
                    if n > 0 {
                        *plots.entry(c.good as GoodId).or_insert(0) += n;
                    }
                }
                let mut book = self.world.resource_mut::<LandBook>();
                let chunks = book.counted.entry(hex.hex()).or_default();
                if plots.is_empty() {
                    chunks.remove(&(*chunk_x, *chunk_z));
                    if chunks.is_empty() {
                        book.counted.remove(&hex.hex());
                    }
                } else {
                    chunks.insert((*chunk_x, *chunk_z), plots);
                }
            }
            Input::Survey { town, bell_x, bell_z } => {
                let t = *town as TownId;
                self.place_bell(t, *bell_x as f64, *bell_z as f64);
                self.set_fields(t);
            }
            Input::Built { town, work, share } => self.laid(*town as TownId, work, *share),
            Input::Port { town, x, z } => self.set_port(*town as TownId, *x as f64 + 0.5, *z as f64 + 0.5),
            Input::Village { x, z } => {
                self.village_seen(*x as f64 + 0.5, *z as f64 + 0.5);
            }
            _ => return false,
        }
        true
    }

    /// Does the hall named `kind` exist (`Kind.id`)?
    pub fn hall_named(kind: &str) -> bool {
        KINDS.iter().any(|k| k.id() == kind)
    }
}
