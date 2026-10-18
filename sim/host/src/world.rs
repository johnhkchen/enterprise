//! The network study's world (`dev/experiments/network_study.py`, its `setup`, `build_river`, `build_ridge`,
//! `build_coast`, and the testbench's `network/hub`, `network/settlement`, `network/squad` and `network/spring`), built
//! through the core's host calls only, and the host's doings each day (the spring's refill, the war's days).

use enterprise_core::crown::Theatre;
use enterprise_core::{Hex, Society, TownId};

use crate::flags::Flags;
use crate::goods::{self, DAY};

/// The hub's corner (its desk two blocks in), and the spring's, behind it.
pub const HUB: (f64, f64) = (1000.0, 1000.0);
pub const SPRING: (f64, f64) = (900.0, 860.0);
/// The settlements' corners (their desks seven blocks in).
const SETTLEMENTS: [(&str, f64, f64); 5] =
    [("S1", 1295.0, 995.0), ("S2", 995.0, 1595.0), ("S3", 1844.0, 1844.0), ("S4", 4995.0, 995.0), ("T", 1393.0, 1396.0)];
/// East of the ridge (`--ridge`): towns left to find their own trade.
const EAST: [(&str, f64, f64); 3] = [("E1", 1650.0, 1350.0), ("E2", 1300.0, 1750.0), ("E3", 2050.0, 1300.0)];
/// The hub's routes, by settlement.
const ROUTED: [&str; 4] = ["S1", "S2", "S3", "S4"];
/// The river (`build_river`): T's row of hexes, the hexes along it, the two by T left dry.
const RIVER_ROW: i32 = 20;
const RIVER_Q: std::ops::Range<i32> = -8..25;
const BRIDGES: [i32; 2] = [7, 8];
/// The ridge (`build_ridge`): hexes whose centre lies within `RIDGE_HALF` of the line x + z = `RIDGE` are mountain.
const RIDGE: f64 = 2789.0;
const RIDGE_HALF: f64 = 70.0;
/// The relief the study raises a ridge hex by: 40 blocks of stone, one chunk left low.
const RIDGE_RELIEF: f64 = 40.0;
/// A bread shulker, and any full shulker of one good: 27 stacks.
const SHULKER: f64 = 27.0 * 64.0;
/// What a realm opens with (`/enterprise realms open`: 20,000 spurs), in emeralds.
const REALM_MONEY: f64 = 20_000.0 / 16.0;

/// The study's world and who's who in it.
pub struct Study {
    pub s: Society,
    pub flags: Flags,
    pub hub: TownId,
    /// The study's own towns (settlements, the hub, the spring once open), by its names, in the order added.
    pub named: Vec<(String, TownId)>,
    pub spring: Option<TownId>,
    pub realms: Vec<TownId>,
    pub front: Option<TownId>,
    /// `--people-use`: the towns whose people use what their lockers would take.
    pub served: Vec<TownId>,
    pub overseas: Option<TownId>,
    /// Lines saying what the core can't do yet, printed once at setup.
    pub missing: Vec<String>,
    /// Routes the study asked for and what came of it (printed at setup, as the study prints `route add`).
    pub setup_log: Vec<String>,
}

fn desk(corner: (f64, f64)) -> (f64, f64) {
    (corner.0 + 7.0, corner.1 + 7.0)
}

fn hub_desk() -> (f64, f64) {
    (HUB.0 + 2.0, HUB.1 + 2.0)
}

/// Does a box of blocks overlap a town's ground (`overlaps_town`: its corner less 2 to its corner and 29)?
fn overlaps_town(towns: &[(f64, f64)], x0: f64, z0: f64, x1: f64, z1: f64) -> bool {
    towns.iter().any(|c| x0 <= c.0 + 29.0 && x1 >= c.0 - 2.0 && z0 <= c.1 + 29.0 && z1 >= c.1 - 2.0)
}

impl Study {
    pub fn town(&self, name: &str) -> Option<TownId> {
        self.named.iter().find(|(n, _)| n == name).map(|&(_, t)| t)
    }

    fn good(&self, name: &str) -> usize {
        self.s.content().good(name).unwrap_or_else(|| panic!("the host has no {name}"))
    }

    fn deliver(&mut self, town: TownId, name: &str, count: f64) {
        let g = self.good(name);
        self.s.deliver(town, g, count);
    }

    /// Tops a good up to `count` in a town's warehouse (an `item replace` into its chest).
    fn top_up(&mut self, town: TownId, name: &str, count: f64) {
        let g = self.good(name);
        let held = self.s.town(town).stock[g];
        if held < count {
            self.s.deliver(town, g, count - held);
        }
    }

    /// The beds, and the land its cottages may spread on, of a town whose roster is set.
    fn house(&mut self, town: TownId, beds: f64) {
        let villagers = self.s.roster(town).residents() as f64;
        self.s.set_people(town, villagers, beds, enterprise_core::settlers::OPEN_LAND);
    }

    /// A route as `enterprise route add` makes one: refused where the road is too long for a leg's food.
    fn route(&mut self, from: TownId, to: TownId, label: &str) -> Option<usize> {
        let blocks = self.s.plan_road(from, to).map(|p| p.blocks());
        if let Some(b) = blocks {
            if enterprise_core::road::road_food(self.s.content())
                && !enterprise_core::road::in_range(self.s.content(), b.round() as u32) {
                self.setup_log.push(format!("route {label}: refused, {b:.0} blocks is more than a wagon's food carries"));
                return None;
            }
        }
        match self.s.open_route(from, to) {
            Some(r) => {
                let (ticks, food) = self.s.route_road(r);
                let how = if self.s.route_path(r).is_some() { "road" } else { "bare route" };
                self.setup_log.push(format!("route {label}: #{r}, a {how} of {} blocks, {:.1} days each way, {food:.2} emeralds of road food a round trip",
                    self.s.route_road_blocks(r), ticks as f64 / DAY as f64));
                Some(r)
            }
            None => {
                self.setup_log.push(format!("route {label}: no way through"));
                None
            }
        }
    }

    /// The study's world, set up as `setup` leaves it before day 1.
    pub fn build(flags: Flags, seed: u64) -> Study {
        let mut s = Society::new(goods::content());
        s.seed(seed);
        let mut st = Study { s, flags, hub: 0, named: Vec::new(), spring: None, realms: Vec::new(), front: None, served: Vec::new(),
            overseas: None, missing: Vec::new(), setup_log: Vec::new() };
        let f = st.flags.clone();
        let mut corners: Vec<(&str, f64, f64)> = SETTLEMENTS.to_vec();
        if f.ridge {
            corners.extend(EAST);
        }
        let town_corners: Vec<(f64, f64)> = corners.iter().map(|&(_, x, z)| (x, z)).collect();
        // The ground first, as the study floods and raises it before anything is built.
        if f.river {
            st.build_river(RIVER_ROW, &BRIDGES, &town_corners);
        } else if f.river_bridge {
            st.build_river(RIVER_ROW + 1, &[], &town_corners);
        }
        if f.ridge {
            st.build_coast();
            st.build_ridge();
        }
        st.build_hub();
        for &(name, x, z) in &corners {
            st.build_settlement(name, (x, z));
        }
        st.build_extras();
        // The hub's routes, then the ridge's and the freight hub's.
        let hub = st.hub;
        for name in ROUTED {
            if f.freight && name == "S3" {
                continue;
            }
            let t = st.town(name).unwrap();
            st.route(hub, t, &format!("hub -> {name}"));
        }
        let (s2, s3) = (st.town("S2").unwrap(), st.town("S3").unwrap());
        if f.ridge {
            st.route(s2, s3, "S2 -> S3");
        }
        if f.freight {
            st.s.make_hub(s2);
            st.route(s2, hub, "S2 -> hub");
            st.route(s3, s2, "S3 -> S2");
            st.route(s2, s3, "S2 -> S3");
        }
        if f.people_use {
            st.people_use_served();
        }
        if f.realms {
            st.open_realms();
        }
        if let Some(war) = f.war {
            st.build_capital(war);
        }
        st.note_missing();
        st
    }

    /// The hub (`network/hub` and `network/squad`): a desk with its Merchant, 18 shulkers of trade goods and 9 of
    /// bread, 64 emeralds for its caravans' purses, its squad (a trained Merchant, Cartographer, Librarian and
    /// Shepherd) and four sheep.
    fn build_hub(&mut self) {
        let f = self.flags.clone();
        let hub = self.s.add_town("hub", 64.0);
        self.hub = hub;
        self.named.push(("hub".into(), hub));
        let (x, z) = hub_desk();
        self.s.place_town(hub, x, z);
        let desk_level = if f.master || f.war.is_some() { 5 } else { 1 };
        let mut hands: Vec<(&str, u8)> =
            vec![("merchant", desk_level), ("merchant", 2), ("cartographer", 1), ("librarian", 1), ("shepherd", 1)];
        if f.fleet {
            hands.extend([("fisherman", 1), ("fisherman", 1), ("fisherman", 1), ("leatherworker", 1)]);
        }
        if f.services {
            hands.extend([("cleric", 2), ("farmer", 2)]);
        }
        if f.metal {
            hands.extend([("armorer", 2), ("toolsmith", 2)]);
        }
        if f.war.is_some() {
            hands.extend([("librarian", 5), ("cleric", 3)]);
        }
        if f.ridge {
            hands.extend([("fisherman", 3), ("fisherman", 3)]); // the port's captains (`build_coast`)
        }
        self.s.set_roster(hub, &hands);
        self.no_workstations(hub);
        // No bell and no beds: the hub isn't a settlement (it is at war: a bell and four beds, and the coast's two).
        let mut beds = 0.0;
        if f.war.is_some() {
            beds += 4.0;
        }
        if f.ridge {
            beds += 2.0;
        }
        let villagers = self.s.roster(hub).residents() as f64;
        // Beds for all, and no land to build on: the hub's people neither crowd nor grow.
        let beds = if beds > 0.0 { beds } else { villagers };
        self.s.set_people(hub, villagers, beds, beds);
        if f.war.is_some() {
            self.s.claim(hub);
        }
        for name in ["paper", "paper", "paper", "stick", "stick", "wheat", "potato", "carrot", "beetroot", "pumpkin",
                "melon", "flint", "glass_pane", "book", "string", "feather", "ink_sac", "compass"] {
            self.deliver(hub, name, SHULKER);
        }
        self.deliver(hub, "bread", 9.0 * SHULKER);
        if f.comforts {
            self.comforts(hub);
        }
        self.s.set_herd(hub, "sheep", 4.0);
    }

    /// A squad's hands are summoned with `Xp:1`, which keeps their trade without a workstation: no workstation, no
    /// locker chest, so no locker trades (the core gives every trained hand its trade's offers; none, here).
    fn no_workstations(&mut self, town: TownId) {
        let n = self.s.lockers(town).map_or(0, |l| l.hands.len());
        for h in 0..n {
            self.s.set_offers(town, h, Vec::new());
        }
    }

    /// A shulker of comforts (`network/comforts`).
    fn comforts(&mut self, town: TownId) {
        for (name, n) in COMFORTS {
            self.deliver(town, name, n);
        }
    }

    /// A settlement (`network/settlement`): the test market's Merchant and four locker villagers (a Librarian,
    /// Cartographer, Fletcher and Farmer, each at level 1), a bed each in the pen and a starter cottage of two with a
    /// spare hand in it, a 9 by 9 carrot field, a shulker of bread and 64 emeralds; its selling trades switched on
    /// (`autoswitch selling`: every trade that pays emeralds for goods).
    fn build_settlement(&mut self, name: &str, corner: (f64, f64)) {
        let t = self.s.add_town(name, 64.0);
        self.named.push((name.into(), t));
        let (x, z) = desk(corner);
        self.s.place_town(t, x, z);
        self.s.set_roster(t, &[("merchant", 1), ("librarian", 1), ("cartographer", 1), ("fletcher", 1), ("farmer", 1),
            ("none", 1)]);
        self.house(t, 7.0);
        self.deliver(t, "bread", SHULKER);
        let carrot = self.good("carrot");
        self.s.set_plots(t, carrot, 81);
        self.s.claim(t);
    }

    /// What the flags add to the settlements once they stand (`--trades`, `--metal`, `--ridge`'s pass town).
    fn build_extras(&mut self) {
        let f = self.flags.clone();
        let (s1, t) = (self.town("S1").unwrap(), self.town("T").unwrap());
        if f.trades {
            for (town, hands) in [(s1, &[("nitwit", 1), ("nitwit", 1), ("mason", 3), ("cleric", 3), ("fletcher", 3),
                    ("farmer", 5)][..]), (t, &[("nitwit", 1), ("nitwit", 1)][..])] {
                for &(trade, level) in hands {
                    self.s.add_hand(town, trade, level);
                }
                let beds = self.s.town(town).people.beds + hands.len() as f64;
                self.house(town, beds);
            }
        }
        if f.metal {
            self.s.add_hand(s1, "toolsmith", 2);
            for trade in ["toolsmith", "armorer", "weaponsmith"] {
                self.s.add_hand(t, trade, 2);
            }
            for town in [s1, t] {
                let beds = self.s.town(town).people.beds;
                self.house(town, beds);
            }
        }
        if f.ridge {
            // The pass town's carrying trade: Leatherworkers (packs), a Shepherd, idle hands, a carrying house's capital.
            for (trade, level) in [("leatherworker", 3), ("leatherworker", 3), ("shepherd", 3), ("nitwit", 1),
                    ("nitwit", 1), ("nitwit", 1)] {
                self.s.add_hand(t, trade, level);
            }
            let beds = self.s.town(t).people.beds + 6.0;
            self.house(t, beds);
            self.s.pay(t, 64.0);
        }
        // The selling trades switched on, after every hand has come.
        let names: Vec<(String, TownId)> = self.named.iter().filter(|(n, _)| n != "hub").cloned().collect();
        for &(_, town) in &names {
            self.autoswitch(town);
        }
    }

    /// `--people-use`, once the routes stand: each town a route serves keeps its people's uses in place of its lockers.
    fn people_use_served(&mut self) {
        let mut served: Vec<TownId> = (0..self.s.route_count()).map(|r| self.s.route_ends(r).1)
            .filter(|&t| self.named.iter().any(|(n, x)| *x == t && n != "hub" && n != "spring")).collect();
        served.sort();
        served.dedup();
        for &t in &served {
            self.people_use(t);
        }
        self.served = served;
    }

    /// A hand of a town served by `--people-use` that took a new trade has its trade's offers again: none, again.
    fn clear_offers(&mut self) {
        for t in self.served.clone() {
            let Some(lockers) = self.s.lockers(t) else { continue };
            for (h, l) in lockers.hands.iter().enumerate() {
                if !l.offers.is_empty() {
                    self.s.set_offers(t, h, Vec::new());
                }
            }
        }
    }

    /// `--people-use`: what the town's managed lockers would take a day, switched on, as what its people use, and its
    /// lockers left with no offers (so its Merchant's review switches none on and sets none of it aside).
    fn people_use(&mut self, town: TownId) {
        let caps = self.s.lockers(town).map_or_else(Vec::new, |l| l.capacity);
        let n = self.s.lockers(town).map_or(0, |l| l.hands.len());
        for h in 0..n {
            self.s.set_offers(town, h, Vec::new());
        }
        for (g, cap) in caps.into_iter().enumerate() {
            if cap > 0.0 {
                self.s.set_trade(town, g, 0.0, cap);
            }
        }
    }

    /// `enterprise autoswitch <desk> selling`: every offer that pays emeralds for goods, on: the offers each villager
    /// has now (`MerchantView`'s trades: those of its level and below), not those it'll unlock as it levels.
    fn autoswitch(&mut self, town: TownId) {
        let Some(lockers) = self.s.lockers(town) else { return };
        let roster = self.s.roster(town);
        for (h, locker) in lockers.hands.iter().enumerate() {
            if locker.switched_on() {
                continue;
            }
            let level = roster.hands.get(h).map_or(0, |hand| hand.level);
            for (o, offer) in locker.offers.iter().enumerate() {
                if !offer.sells && offer.level <= level {
                    self.s.switch(town, h, o, true);
                }
            }
        }
    }

    /// The river (`build_river`): the surface of each hex in `row` flooded in strips inside the hex, strips that would
    /// run under a town left dry, the `bridges` left dry altogether.
    fn build_river(&mut self, row: i32, bridges: &[i32], towns: &[(f64, f64)]) {
        let strips = [(0.0, 22.0, 39.0), (23.0, 30.0, 26.0), (31.0, 38.0, 12.0)];
        let mut flooded = 0;
        for q in RIVER_Q {
            if bridges.contains(&q) {
                continue;
            }
            let hex = Hex::new(q, row);
            let (cx, cz) = (hex.center_x().round(), hex.center_z().round());
            let mut rects = Vec::new();
            for (low, high, half) in strips {
                let spans: Vec<(f64, f64)> =
                    if low == 0.0 { vec![(cz - high, cz + high)] } else { vec![(cz + low, cz + high), (cz - high, cz - low)] };
                for (z0, z1) in spans {
                    if !overlaps_town(towns, cx - half, z0, cx + half, z1) {
                        rects.push((cx - half, z0, cx + half, z1));
                    }
                }
            }
            // The share of the hex's blocks under water, counted block by block.
            let (mut wet, mut all) = (0u32, 0u32);
            for bx in (cx - 50.0) as i32..=(cx + 50.0) as i32 {
                for bz in (cz - 50.0) as i32..=(cz + 50.0) as i32 {
                    let (x, z) = (bx as f64, bz as f64);
                    if Hex::at(x + 0.5, z + 0.5) != hex {
                        continue;
                    }
                    all += 1;
                    if rects.iter().any(|&(x0, z0, x1, z1)| x >= x0 && x <= x1 && z >= z0 && z <= z1) {
                        wet += 1;
                    }
                }
            }
            self.s.set_ground(hex, wet as f64 / all.max(1) as f64, 0.0);
            flooded += 1;
        }
        self.setup_log.push(format!("river: {flooded} hexes of row {row} flooded, dry at q {bridges:?}"));
    }

    /// The coast (`build_coast`): the sea west of the capital, each hex whose centre lies in x 440 to 950, z 520 to
    /// 1,480 flooded; its two Fishermen are in the hub's roster.
    fn build_coast(&mut self) {
        let mut flooded = 0;
        for q in -30..60 {
            for r in -10..60 {
                let hex = Hex::new(q, r);
                let (cx, cz) = (hex.center_x().round(), hex.center_z().round());
                if (440.0..=950.0).contains(&cx) && (520.0..=1480.0).contains(&cz) {
                    self.s.set_ground(hex, 1.0, 0.0);
                    flooded += 1;
                }
            }
        }
        self.setup_log.push(format!("coast: {flooded} hexes of sea west of the capital"));
    }

    /// The ridge (`build_ridge`): each hex near the line x + z = 2,789 (centre within 600 to 2,200 in x) raised, all but
    /// T's, which is a ravine crossed by its one bridge.
    fn build_ridge(&mut self) {
        let t = SETTLEMENTS[4];
        let (tx, tz) = desk((t.1, t.2));
        let pass = Hex::at(tx + 0.5, tz + 0.5);
        let mut raised = 0;
        for q in -30..60 {
            for r in -10..60 {
                let hex = Hex::new(q, r);
                let (cx, cz) = (hex.center_x().round(), hex.center_z().round());
                if (cx + cz - RIDGE).abs() > RIDGE_HALF || !(600.0..=2200.0).contains(&cx) || hex == pass {
                    continue;
                }
                self.s.set_ground(hex, 0.0, RIDGE_RELIEF);
                raised += 1;
            }
        }
        self.s.set_ground(pass, 0.6, 0.0);
        self.s.bridge(pass);
        self.setup_log.push(format!("ridge: {raised} hexes raised, the pass at T ({pass}) a ravine with one bridge"));
    }

    /// The realms beyond the border (`--realms`): the Coast over the sea west of the capital, trading with it; the March
    /// to the east, with E3 and E1.
    fn open_realms(&mut self) {
        let (hx, hz) = hub_desk();
        let coast = self.s.add_coast(hx - 2400.0, hz, REALM_MONEY);
        let march = self.s.add_march(3400.0, 1350.0, REALM_MONEY);
        self.realms = vec![coast, march];
        let hub = self.hub;
        self.partner(coast, hub, "the Coast -> hub");
        let partners: Vec<TownId> = ["E3", "E1"].iter().filter_map(|n| self.town(n)).collect();
        if partners.is_empty() {
            self.missing.push("--realms: the March trades with E3 and E1, which stand only with --ridge: it has no partner".into());
        }
        for p in partners {
            let name = self.s.town(p).name;
            self.partner(march, p, &format!("the March -> {name}"));
        }
    }

    fn partner(&mut self, realm: TownId, partner: TownId, label: &str) {
        let r = self.s.open_route(realm, partner);
        self.s.learn(realm, partner);
        self.s.learn(partner, realm);
        let line = match r {
            Some(r) => format!("route {label}: #{r}, {} blocks", self.s.route_road_blocks(r)),
            None => format!("route {label}: no way"),
        };
        self.setup_log.push(line);
    }

    /// The sovereign's capital at the hub (`--war`): its court (a master Merchant, Librarian and Cleric, in the
    /// hub's roster), its metal exchange with the market across the sea, named the capital with 500 spurs a day of
    /// peacetime revenue (`enterprise war capital <desk> 500`: the crown, `crown.rs`); S2 promoted with a barracks (four
    /// beds). The crown opens its own front when it declares war.
    fn build_capital(&mut self, _war: (u64, u64, u64)) {
        let hub = self.hub;
        self.overseas = self.s.open_exchange_overseas(hub, REALM_MONEY);
        self.s.name_capital(hub, 500);
        let s2 = self.town("S2").unwrap();
        let beds = self.s.town(s2).people.beds + 4.0;
        self.house(s2, beds);
    }

    /// What the flags ask for that the core can't do yet: one line each.
    fn note_missing(&mut self) {
        let f = &self.flags;
        let mut m = Vec::new();
        m.push("world: the settlements' bells, beds and fields are the host's numbers (no blocks): 7 beds, 81 plots of carrots, land for 40 beds".to_string());
        if f.fleet {
            m.push("--fleet: boats across water (the fleet's sailing and fishing) aren't in the core: the Fishermen sit in the hub's roster; the Leatherworker's packs are".into());
        }
        if f.services {
            m.push("--services: the Cleric's and Farmer's services abroad run; mending, hunting abroad and master services are the core's gaps (docs/core-progress.md)".into());
        }
        if f.metal {
            m.push("--metal: mines are dug by the core's Toolsmiths; blast smelting abroad and mending/armoury services are missing".into());
        }
        if f.trades {
            m.push("--trades: the temple and watchtower aren't started by the host (no site); flower beds are a flag the host doesn't set".into());
        }
        if f.master {
            m.push("--master: the hub's Merchant is a master, but the study puts no players' shops round its desk, so it brokers nothing".into());
        }
        if f.war.is_some() {
            m.push("--war: the crown runs it (`crown.rs`): the binding's gold shop and the requisitions' buying vendors aren't the core's (Numismatics blocks)".into());
        }
        if f.ridge {
            m.push("--ridge: llamas (the pass town's pen) aren't among the core's animals; the ridge is relief 40 on each hex, the pass a ravine (60% water) with a map bridge".into());
        }
        if f.inject.is_some() {
            m.push("--inject: the spring is a town whose warehouse the host tops up each day".into());
        }
        self.missing = m.into_iter().chain(std::mem::take(&mut self.missing)).collect();
    }

    /// The host's doings at the start of a day, before the core runs it (`--inject`, `--war`).
    pub fn morning(&mut self, day: u64) {
        let f = self.flags.clone();
        if f.people_use {
            self.clear_offers();
        }
        if let Some(inject) = f.inject {
            if day == inject {
                self.open_spring();
            } else if day > inject {
                self.refill_spring();
            }
        }
        if let Some((m, w, p)) = f.war {
            let hub = self.hub;
            if day == m {
                // `enterprise war mobilise`: 1,000 spurs a day minted, the stockpile requisitioned.
                self.s.mobilise(hub, 1000);
                self.setup_log.push(format!("day {day}: mobilised"));
            }
            if day == w {
                let levies = self.s.declare(Theatre::Faction);
                self.front = self.s.sovereign().front;
                self.setup_log.push(format!("day {day}: war declared on a rival faction, {levies} towns levied"));
            }
            if day == p {
                let home = self.s.peace();
                self.setup_log.push(format!("day {day}: peace, {home} sent home"));
            }
        }
    }

    /// The spring opens (`open_spring`): a desk with its Merchant and squad behind the hub, 64 emeralds, its
    /// warehouse filled, routes to S1, S2 and S3.
    fn open_spring(&mut self) {
        let spring = self.s.add_town("spring", 0.0);
        self.s.pay(spring, 64.0);
        self.named.push(("spring".into(), spring));
        self.spring = Some(spring);
        self.s.place_town(spring, SPRING.0 + 2.0, SPRING.1 + 2.0);
        self.s.set_roster(spring, &[("merchant", 1), ("merchant", 2), ("cartographer", 1), ("librarian", 1), ("shepherd", 1)]);
        self.no_workstations(spring);
        let v = self.s.roster(spring).residents() as f64;
        self.s.set_people(spring, v, v, v);
        self.s.set_herd(spring, "sheep", 4.0);
        self.refill_spring();
        for name in ["S1", "S2", "S3"] {
            let t = self.town(name).unwrap();
            self.route(spring, t, &format!("spring -> {name}"));
        }
    }

    /// The spring's day's refill (`network/spring_refill`): its chests' slots set again, so each good topped up.
    fn refill_spring(&mut self) {
        let Some(spring) = self.spring else { return };
        for (name, shulkers) in [("paper", 3.0), ("stick", 2.0), ("wheat", 1.0), ("potato", 1.0), ("carrot", 1.0),
                ("beetroot", 1.0), ("bread", 2.0)] {
            self.top_up(spring, name, shulkers * SHULKER);
        }
        if self.flags.comforts {
            for (name, n) in COMFORTS {
                self.top_up(spring, name, n);
            }
            self.top_up(spring, "book", 64.0);
        }
    }
}

/// A shulker of comforts (`network/comforts`).
const COMFORTS: [(&str, f64); 15] = [("cookie", 256.0), ("pumpkin_pie", 128.0), ("honey_bottle", 32.0), ("cake", 1.0),
    ("leather_boots", 4.0), ("leather_leggings", 4.0), ("leather_helmet", 2.0), ("white_carpet", 64.0),
    ("red_carpet", 64.0), ("candle", 64.0), ("flower_pot", 64.0), ("lantern", 16.0), ("painting", 16.0),
    ("yellow_carpet", 64.0), ("blue_carpet", 64.0)];
