//! What the run saw: each wagon's comings and goings (from the core's outputs), each town's cash book day by day, the
//! CSV of one row per town and per route a day (the study's column names where the core has the same quantity), the
//! day's line, and the end-of-run report.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Write as _;

use enterprise_core::cash_book::Account;
use enterprise_core::state::RouteState;
use enterprise_core::works::What;
use enterprise_core::{Output, Society, TownId};

use crate::world::Study;

/// A route's wagons as the outputs told them.
#[derive(Clone, Debug, Default)]
pub struct RouteTrack {
    pub left: u32,
    pub stacks_out: u64,
    pub full: u32,
    pub worth_out: f64,
    pub arrived: u32,
    pub sold_items: f64,
    pub sold_stacks: f64,
    pub takings_far: f64,
    pub home: u32,
    pub stacks_home: u64,
    pub takings_home: f64,
    /// The wagons' purse book, the whole run: emeralds into and out of it by what for (the purse home sent with it, the
    /// far end's payments, road food, tolls, what it bought, what it brought home).
    pub cash: BTreeMap<String, (f64, f64)>,
}

/// A town's cash book, the whole run: emeralds in and out by what for; and its lockers' trades (each pays an emerald).
#[derive(Clone, Debug, Default)]
pub struct TownTrack {
    pub cash: BTreeMap<String, (f64, f64)>,
    pub trades: f64,
    pub opening: f64,
}

#[derive(Default)]
pub struct Tracker {
    pub routes: Vec<RouteTrack>,
    pub towns: Vec<TownTrack>,
    pub events: usize,
    csv: Option<std::io::BufWriter<std::fs::File>>,
    pub rows: usize,
}

/// Every route's state, by its number (`Society::route_state`).
pub fn route_states(s: &mut Society) -> Vec<RouteState> {
    (0..s.route_count()).map(|r| s.route_state(r).1.clone()).collect()
}

pub fn route_label(s: &Society, route: usize) -> String {
    let (a, b) = s.route_ends(route);
    format!("{}->{}", s.town(a).name, s.town(b).name)
}

const TOWN_COLUMNS: &str = "day,kind,name,mint_rate,treasury,purse,hexes,guards,posts,toll_pct,tolls_taken,traffic,deserted,\
farm_share_pct,harvested,trades,mood,villagers,beds,homes,born,wall_pct,bridges,culture,focus,stock_worth";
const ROUTE_COLUMNS: &str = "blocks,trips,sold_items,sales_spurs,service_spurs,bought_items,purchase_spurs,food_spurs,\
home_spurs,purse_spurs,stacks_out,stacks_home,sold_stacks,food_eaten,short_rations,delivered,tolls";

impl Tracker {
    pub fn new(csv: Option<&str>) -> Result<Tracker, String> {
        let csv = match csv {
            Some(path) => {
                let mut w = std::io::BufWriter::new(std::fs::File::create(path).map_err(|e| format!("{path}: {e}"))?);
                writeln!(w, "{TOWN_COLUMNS},{ROUTE_COLUMNS}").map_err(|e| e.to_string())?;
                Some(w)
            }
            None => None,
        };
        Ok(Tracker { csv, ..Default::default() })
    }

    /// Room for every route and town: a town's opening balance is what it held before the first day (a town opened
    /// later opens with nothing: its money comes in through its books).
    pub fn fit(&mut self, s: &Society, opening: bool) {
        self.routes.resize(s.route_count(), RouteTrack::default());
        while self.towns.len() < s.town_count() {
            let t = self.towns.len();
            self.towns.push(TownTrack { opening: if opening { s.town(t).money.to_f64() } else { 0.0 }, ..Default::default() });
        }
    }

    /// The day's outputs and books, after the core has run it.
    pub fn day(&mut self, st: &mut Study, day: u64, out: &[Output]) {
        self.fit(&st.s, false);
        self.events += out.len();
        let content = st.s.content().clone();
        let states = route_states(&mut st.s);
        for o in out {
            match *o {
                Output::WagonLeft { route, stacks, worth } => {
                    let r = &mut self.routes[route as usize];
                    r.left += 1;
                    r.stacks_out += stacks as u64;
                    r.worth_out += worth;
                    if stacks >= content.dials.wagon_slots {
                        r.full += 1;
                    }
                }
                Output::WagonSold { route, sold, takings } => {
                    let r = &mut self.routes[route as usize];
                    r.arrived += 1;
                    r.sold_items += sold;
                    r.takings_far += takings.to_f64();
                    if let Some(w) = &states[route as usize].wagon {
                        r.sold_stacks += w.sold_here.iter().enumerate()
                            .map(|(g, n)| n / content.goods[g].stack.max(1) as f64).sum::<f64>();
                    }
                }
                Output::WagonHome { route, takings } => {
                    let r = &mut self.routes[route as usize];
                    r.home += 1;
                    r.takings_home += takings.to_f64();
                    r.stacks_home += states[route as usize].last_home.unwrap_or(0) as u64;
                }
                _ => {}
            }
        }
        self.read_books(&st.s, day.saturating_sub(1));
        if self.csv.is_some() {
            self.write_rows(st, day, &states);
        }
    }

    /// Each town's cash book for one whole day (a day's last postings land at the next day's first tick, so the host
    /// reads yesterday's after today's run, and the last day's at the end).
    pub fn read_books(&mut self, s: &Society, day: u64) {
        for r in 0..s.route_count() {
            for (_, label, v) in s.account_book(Account::Wagon(r), 3).into_iter().filter(|(d, _, _)| *d == day) {
                let v = v.to_f64();
                let e = self.routes[r].cash.entry(label).or_default();
                if v >= 0.0 { e.0 += v } else { e.1 -= v }
            }
        }
        for t in 0..s.town_count() {
            for (_, label, v) in s.cash_book(t, 3).into_iter().filter(|(d, _, _)| *d == day) {
                let v = v.to_f64();
                let e = self.towns[t].cash.entry(label.clone()).or_default();
                if v >= 0.0 { e.0 += v } else { e.1 -= v }
                if label.starts_with("lockers: sold") && v > 0.0 {
                    self.towns[t].trades += v;
                }
            }
        }
    }

    fn write_rows(&mut self, st: &Study, day: u64, states: &[RouteState]) {
        let s = &st.s;
        let rate = s.mint_rate();
        let content = s.content();
        let mut lines = String::new();
        let blank_route = ",".repeat(ROUTE_COLUMNS.matches(',').count());
        for (name, t) in &st.named {
            let v = s.town(*t);
            let h = s.holding(*t);
            let farm = s.farm(*t);
            let works = s.works(*t);
            let bridges = works.projects.iter().filter(|p| p.done && matches!(p.what, What::Bridge { .. })).count();
            let worth: f64 = v.stock.iter().zip(&content.goods).map(|(n, g)| n * g.value).sum();
            let posts: Vec<String> = h.posts.iter().map(|p| format!("{} {}", p.q, p.r)).collect();
            let _ = writeln!(lines, "{day},town,{name},{rate:.3},{:.3},{:.3},{},{},{},{:.1},{:.1},{:.1},{},{},{:.0},{:.0},{:.3},{},{},{},{},{:.0},{},{:.2},{},{:.2},{blank_route}",
                v.money, v.people.purse, h.hexes.len(), s.guards(*t), posts.join(";"),
                100.0 * s.toll_rate(*t), h.tolls_taken, h.traffic_per_day, h.deserted,
                farm.as_ref().map_or(String::new(), |f| format!("{:.1}", 100.0 * f.farm.labor_share)),
                farm.as_ref().map_or(0.0, |f| f.farm.harvested), s.locker_trades(*t) as f64, v.people.mood,
                v.people.villagers, v.people.beds, v.people.homes_built, v.people.born, 100.0 * s.walled(*t), bridges,
                h.culture, farm.as_ref().map_or(String::new(), |f| format!("{:?}", f.farm.focus)), worth);
        }
        let blank_town = ",".repeat(TOWN_COLUMNS.matches(',').count() - 3);
        for (r, state) in states.iter().enumerate() {
            let tr = &self.routes[r];
            let _ = writeln!(lines, "{day},route,{},{rate:.3}{blank_town},{},{},{:.0},{:.1},{:.1},{:.0},{:.1},{:.1},{:.1},{:.1},{},{},{:.2},{:.1},{},{:.0},{:.1}",
                route_label(s, r), s.route_road_blocks(r), state.trips, state.sold, tr.takings_far * rate,
                state.service_paid.to_f64() * rate, state.bought, state.purchases.to_f64() * rate, state.food_paid.to_f64() * rate,
                tr.takings_home * rate, tr.cash.get("wagon purse").map_or(0.0, |c| c.0) * rate, tr.stacks_out, tr.stacks_home, tr.sold_stacks, state.food_eaten,
                state.short_rations, state.delivered, s.tolls_paid(r).to_f64() * rate);
        }
        self.rows += lines.lines().count();
        if let Some(w) = self.csv.as_mut() {
            let _ = w.write_all(lines.as_bytes());
        }
    }

    pub fn finish(&mut self) {
        if let Some(w) = self.csv.as_mut() {
            let _ = w.flush();
        }
    }
}

/// The day's line, as the study prints it: each town's treasury, mood, villagers, hexes, guards, toll and trades.
pub fn day_line(st: &Study, _tr: &Tracker, day: u64) -> String {
    let s = &st.s;
    let towns: Vec<String> = st.named.iter().filter(|(n, _)| n != "hub" && n != "spring").map(|(name, t)| {
        let v = s.town(*t);
        format!("{name} {:.0}em {:+.1} {}v {}h {}g {:.0}% {}t", v.money, v.people.mood, v.people.villagers,
            s.holding(*t).hexes.len(), s.guards(*t), 100.0 * s.toll_rate(*t), s.locker_trades(*t))
    }).collect();
    format!("day {day:>4}: {} | hub {:.0}em | mint {:.1}", towns.join(" | "), s.town(st.hub).money, s.mint_rate())
}

/// The end-of-run report, from the core's state and what the run tracked.
pub fn end_report(st: &mut Study, tr: &Tracker, days: u64) -> String {
    let states = route_states(&mut st.s);
    let s = &st.s;
    let content = s.content();
    let mut o = String::new();
    let _ = writeln!(o, "\n== Towns on day {days} (money in emeralds; mint {:.2} spurs to the emerald)", s.mint_rate());
    let towns: Vec<(String, TownId)> = (0..s.town_count()).map(|t| (s.town(t).name, t)).collect();
    for (name, t) in &towns {
        let v = s.town(*t);
        let h = s.holding(*t);
        let worth: f64 = v.stock.iter().zip(&content.goods).map(|(n, g)| n * g.value).sum();
        let mut top: Vec<(usize, f64)> = v.stock.iter().enumerate().filter(|(_, n)| **n >= 1.0)
            .map(|(g, n)| (g, n * content.goods[g].value)).collect();
        top.sort_by(|a, b| b.1.total_cmp(&a.1));
        let top: Vec<String> = top.iter().take(5).map(|&(g, _)| format!("{:.0} {}", v.stock[g], content.goods[g].name)).collect();
        let roster = s.roster(*t);
        let mut trades: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        for hand in &roster.hands {
            trades.entry(content.trades[hand.trade].name.clone()).or_default().push(hand.level);
        }
        let hands: Vec<String> = trades.iter().map(|(n, l)| if l.len() == 1 { format!("{n} {}", l[0]) }
            else { format!("{}x{n} {:?}", l.len(), l) }).collect();
        let _ = writeln!(o, "{name:>18}: {:>8.1} em (opened {:.0}), purse {:.1}; {} villagers, {} beds, mood {:+.2}; {} hexes, {} guards, toll {:.0}%, traffic {:.0} spurs/day; {} locker trades, {:.0} em of desk sales",
            v.money, tr.towns.get(*t).map_or(0.0, |x| x.opening), v.people.purse, v.people.villagers, v.people.beds,
            v.people.mood, h.hexes.len(), s.guards(*t), 100.0 * s.toll_rate(*t), h.traffic_per_day,
            s.locker_trades(*t), tr.towns.get(*t).map_or(0.0, |x| x.trades));
        let _ = writeln!(o, "{:>18}  hands: {}", "", hands.join(", "));
        if let Some(f) = s.farm(*t) {
            let plots: Vec<String> = f.farm.plots.iter().map(|(g, n)| format!("{n} {}", content.goods[*g].name)).collect();
            let grow: Vec<String> = f.farm.per_day.iter().filter(|(_, n)| **n > 0.0)
                .map(|(g, n)| format!("{n:.1} {}", content.goods[*g].name)).collect();
            let _ = writeln!(o, "{:>18}  farm: plots {}, grows {} a day, {} fallow; labour share {:.0}%, {:.0} harvested, wages {:.1} paid / {:.1} unpaid, focus {:?}",
                "", plots.join(" "), if grow.is_empty() { "nothing".into() } else { grow.join(", ") }, f.farm.fallow,
                100.0 * f.farm.labor_share, f.farm.harvested, f.farm.wages_paid, f.farm.wages_unpaid, f.farm.focus);
        }
        let _ = writeln!(o, "{:>18}  stock worth {:.0} em: {}", "", worth, top.join(", "));
        let herds = s.herds(*t);
        let animals: Vec<String> = herds.count.iter().enumerate().filter(|(_, n)| **n > 0.0)
            .map(|(k, n)| format!("{n:.1} {}", content.animals[k].name)).collect();
        if !animals.is_empty() {
            let _ = writeln!(o, "{:>18}  herds: {}", "", animals.join(", "));
        }
        let works = s.works(*t);
        if !works.projects.is_empty() {
            let p: Vec<String> = works.projects.iter().map(|p| format!("{:?} {:.0}/{}{}", p.what, p.laid, p.blocks_needed,
                if p.done { " done" } else { "" })).collect();
            let _ = writeln!(o, "{:>18}  works: {}", "", p.join("; "));
        }
        if s.is_hub(*t) {
            let hub = s.hub(*t);
            let _ = writeln!(o, "{:>18}  freight hub: {} crates held, book {:?}", "", hub.custody.len(), hub.book);
        }
        if let Some(m) = s.mine(*t) {
            let _ = writeln!(o, "{:>18}  mine: {m:?}", "");
        }
    }
    let army = s.army();
    if !army.records.is_empty() || army.front.is_some() {
        let _ = writeln!(o, "\n== The army: {} at the front, {} waiting, {} walking home; {:.0} food points eaten", army.soldiers.len(),
            army.barracks.len(), army.walking.len(), army.eaten);
        for (t, r) in &army.records {
            let _ = writeln!(o, "{:>18}: raised {}, fallen {}, deserted {}, {} soldier-days ({} hungry), pay {:.1}",
                s.town(*t).name, r.raised, r.fallen, r.deserted, r.days, r.hungry, r.pay);
        }
    }
    for t in st.realms.iter().copied().chain(st.overseas) {
        if let Some(r) = s.realm(t) {
            let _ = writeln!(o, "{:>18}: realm made {:.0} goods, paid {:.1} em of customs", s.town(t).name, r.goods_made, r.customs_paid);
        }
    }
    if st.overseas.is_some() {
        let ex = s.exchange();
        let held: Vec<String> = ex.held.iter().map(|(g, n)| format!("{n:.0} {}", content.goods[*g].name)).collect();
        let _ = writeln!(o, "{:>18}: the metal exchange, phase {:?}, held {}", "", ex.phase, held.join(", "));
    }
    let _ = writeln!(o, "\n== Routes (a trip: out with its load, sold at the far end, home with what it bought)");
    let _ = writeln!(o, "{:>22} {:>6} {:>5} | {:>7} {:>5} {:>8} {:>7} {:>7} | {:>7} {:>7} {:>6} | {:>6} {:>6}",
        "route", "blocks", "trips", "stk out", "full", "sold it", "sold st", "em far", "stk hm", "bought", "em hm", "food", "paid");
    for (r, state) in states.iter().enumerate() {
        let t = &tr.routes[r];
        let per = |x: f64, n: u32| if n == 0 { 0.0 } else { x / n as f64 };
        let _ = writeln!(o, "{:>22} {:>6} {:>5} | {:>7.2} {:>4.0}% {:>8.1} {:>7.2} {:>7.2} | {:>7.2} {:>7.1} {:>6.2} | {:>6.0} {:>6.1}",
            route_label(s, r), s.route_road_blocks(r), state.trips, per(t.stacks_out as f64, t.left),
            100.0 * per(t.full as f64, t.left), per(t.sold_items, t.arrived), per(t.sold_stacks, t.arrived),
            per(t.takings_far, t.arrived), per(t.stacks_home as f64, t.home), per(state.bought, t.home.max(state.trips)),
            per(t.takings_home, t.home), state.food_eaten, state.food_paid);
    }
    let _ = writeln!(o, "  (stk out: stacks a wagon left with; full: share of wagons that left with every slot taken; sold it/st/em far: \
items, stacks and emeralds sold a visit at the far end; stk hm: stacks a wagon brought home; bought: items bought at the \
far end a trip; em hm: the purse a wagon brought home; food: points eaten, paid: emeralds paid far ends for food)");
    for r in 0..states.len() {
        let t = &tr.routes[r];
        if t.cash.is_empty() {
            continue;
        }
        let mut lines: Vec<(&String, &(f64, f64))> = t.cash.iter().collect();
        lines.sort_by(|a, b| (b.1.0 + b.1.1).total_cmp(&(a.1.0 + a.1.1)));
        let parts: Vec<String> = lines.iter().take(8).map(|(l, (i, x))| match (*i > 0.005, *x > 0.005) {
            (true, true) => format!("{l} +{i:.1}/-{x:.1}"),
            (true, false) => format!("{l} +{i:.1}"),
            _ => format!("{l} -{x:.1}"),
        }).collect();
        let _ = writeln!(o, "{:>22}  purse: {}", route_label(s, r), parts.join("; "));
    }
    let (left, out, home, back): (u32, u64, u32, u64) = tr.routes.iter()
        .fold((0, 0, 0, 0), |a, t| (a.0 + t.left, a.1 + t.stacks_out, a.2 + t.home, a.3 + t.stacks_home));
    let sold: f64 = tr.routes.iter().map(|t| t.sold_stacks).sum();
    let arrived: u32 = tr.routes.iter().map(|t| t.arrived).sum();
    let _ = writeln!(o, "{:>22}: {left} wagons left, {:.2} stacks out a trip, {:.2} stacks sold a trip, {:.2} stacks home a trip ({} slots a wagon)",
        "all routes", out as f64 / left.max(1) as f64, sold / arrived.max(1) as f64, back as f64 / home.max(1) as f64,
        content.dials.wagon_slots);
    let (mut documents, mut ladings, mut superbills, mut fees) = (0u64, 0u64, 0u64, 0.0);
    for t in 0..s.town_count() {
        let p = s.paperwork(t);
        documents += p.documents;
        ladings += p.by_kind.get(enterprise_core::cargo::LADING).copied().unwrap_or(0);
        superbills += p.by_kind.get(enterprise_core::cargo::SUPERBILL).copied().unwrap_or(0);
        fees += p.scribe_fees.to_f64();
    }
    let _ = writeln!(o, "{:>22}: {documents} documents drawn ({ladings} bills of lading, {superbills} superbills), {fees:.1} emeralds of scribes' fees, {} pallets packed, {} consolidations",
        "paper", s.pallets().next, s.pallets().next_container);
    let _ = writeln!(o, "\n== Why wagons went or stayed, and goods went or didn't (each Merchant's books, most often first)");
    for (name, t) in &towns {
        let whys = s.whys(*t);
        if whys.is_empty() {
            continue;
        }
        let _ = writeln!(o, "{name}:");
        // The wagons' and the goods' reasons first; the far ends' comfort shortages (a line a kind a market) after.
        let short = |w: &&enterprise_core::atlas::WhyLine| w.line.contains("they're short of");
        let (comforts, rest): (Vec<_>, Vec<_>) = whys.iter().partition(short);
        for w in rest.iter().take(14) {
            let _ = writeln!(o, "  {:>6}x {:>9.0} items  {} (last day {})", w.times, w.items, w.line, w.last);
        }
        if rest.len() > 14 {
            let _ = writeln!(o, "  ... and {} more", rest.len() - 14);
        }
        if let Some(w) = comforts.first() {
            let _ = writeln!(o, "  {:>6}x {:>9.0} items  {} (last day {}), and {} more comfort shortages", w.times, w.items,
                w.line, w.last, comforts.len() - 1);
        }
    }
    let _ = writeln!(o, "\n== Cash books, the whole run (emeralds into and out of each treasury, by what for)");
    for (name, t) in &towns {
        let Some(tt) = tr.towns.get(*t) else { continue };
        let mut lines: Vec<(&String, &(f64, f64))> = tt.cash.iter().collect();
        lines.sort_by(|a, b| (b.1.0 + b.1.1).total_cmp(&(a.1.0 + a.1.1)));
        let (i, x): (f64, f64) = tt.cash.values().fold((0.0, 0.0), |a, v| (a.0 + v.0, a.1 + v.1));
        let money = s.town(*t).money.to_f64();
        let _ = writeln!(o, "{name}: opened {:.1}, in {i:.1}, out {x:.1}, now {money:.1} (unexplained {:+.2})",
            tt.opening, money - tt.opening - i + x);
        let parts: Vec<String> = lines.iter().take(10).map(|(l, (i, x))| {
            match (*i > 0.005, *x > 0.005) {
                (true, true) => format!("{l} +{i:.1}/-{x:.1}"),
                (true, false) => format!("{l} +{i:.1}"),
                _ => format!("{l} -{x:.1}"),
            }
        }).collect();
        if !parts.is_empty() {
            let _ = writeln!(o, "    {}", parts.join("; "));
        }
    }
    o
}
