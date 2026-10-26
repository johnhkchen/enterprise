//! A run, traced: a journal the mod wrote (`binding/core/Journal`: every request that changed its society, in order)
//! played back through this core, and what happened written into SQLite. The core is deterministic, so the replay is the
//! run; what a host never asked about can be asked now.
//!
//!     cargo run --release -p enterprise-sim -- --trace world/enterprise-journal/20261023-180903.ejl run.db
//!     sqlite3 run.db "select day, good, count from stock join towns using (day, town) where name = 'Haywick'"
//!
//! The tables (`day` is the society's day; a town, route or hand is the core's number for it):
//! - `requests`: each request replayed, its command, the tick after it, and its inputs and outputs counted.
//! - `inputs`, `outputs`: each input the mod sent and each output the core gave back, by kind, with its town, route
//!   and good when it names one, and the whole of it as JSON.
//! - Each day as the first request to reach it left it (the last day as the run ended): `towns` (name, people, beds, mood, money, mood target, comforts met, paid share), `comforts` (each kind met), `herds` (each kind's count and the pastures), `hands` (each town's roster), `farms` (each crop's plots, plan, idle yield, demand, census, workshop draws and needs) and `farm_towns` (focus, fields planted), `works` (each project's progress, done, the works' mood), `stock` (each good a town
//!   holds), `routes` (trips, what's sold and carried, whether a wagon is out, why it stayed, the far end's last purse),
//!   `lockers` (each hand's offers: placed, managed, switched on), `whys` (each town's reasons, tallied so far: a
//!   day's are the difference from the day before's).
//! - Each plan a route decided, as it decided it: `plans` (its load's worth against the road's cost, full, on its day,
//!   whether it went) and `plan_whys` (why each good went or stayed in it), the reasons of a wagon that stayed home too,
//!   which the core's own tally keeps only for one that went.
//! - Each wagon's trip, once it's home: `trips` (what it took out: stacks, worth, purse; what it sold there and took
//!   for it; what it bought there and paid; the items on pallets it carried, either way: contract goods and crated
//!   sales). A trip's worth out counts only what it took to sell (and advances it paid): what it brought home is here.
//!   A wagon that left before the journal began has only its route, its homecoming and its takings.
//! - At the end of the run: `cash` (each town's cash book, every line).
//!
//! A journal written by a core of another wire schema is refused: replay it with the core that wrote it (`git log` for
//! the commit whose `sim/wire/registry.yaml` it matches).

use std::collections::BTreeMap;
use std::path::Path;

use enterprise_core::state::{Route, RouteState};
use enterprise_core::wire::{self, Answer, Command, Query, Request, Response};
use enterprise_core::Society;
use rusqlite::{params, Connection};
use serde_json::Value;

/// A journal's first bytes (`Journal.MAGIC`).
pub const MAGIC: &[u8] = b"ENTJRNL1";

/// What a trace came to.
#[derive(Debug, Default)]
pub struct Traced {
    pub requests: usize,
    pub refused: usize,
    pub days: u64,
    pub towns: usize,
    pub routes: usize,
}

/// A journal's requests, in order (a last one cut short left out).
pub fn read(bytes: &[u8]) -> Result<Vec<&[u8]>, String> {
    let rest = bytes.strip_prefix(MAGIC).ok_or("not an Enterprise journal (no ENTJRNL1 at its start)")?;
    let mut out = Vec::new();
    let mut at = 0;
    while at + 4 <= rest.len() {
        let len = u32::from_be_bytes(rest[at..at + 4].try_into().unwrap()) as usize;
        if at + 4 + len > rest.len() {
            break;
        }
        out.push(&rest[at + 4..at + 4 + len]);
        at += 4 + len;
    }
    Ok(out)
}

/// The journal at `journal` replayed into a new database at `db` (one there already is replaced).
pub fn trace_file(journal: &Path, db: &Path) -> Result<Traced, String> {
    let bytes = std::fs::read(journal).map_err(|e| format!("{}: {e}", journal.display()))?;
    if db.exists() {
        std::fs::remove_file(db).map_err(|e| format!("{}: {e}", db.display()))?;
    }
    let mut conn = Connection::open(db).map_err(|e| e.to_string())?;
    trace(&read(&bytes)?, &mut conn)
}

const SCHEMA_SQL: &str = "
create table requests (seq integer, command text, tick integer, day integer, inputs integer, outputs integer, refused text);
create table inputs (seq integer, i integer, tick integer, day integer, kind text, town integer, route integer, good text, json text);
create table outputs (seq integer, i integer, tick integer, day integer, kind text, town integer, route integer, good text, json text);
create table goods (good integer, name text, crop integer, value real);
create table towns (day integer, town integer, name text, villagers real, beds real, mood real, money real, target real, comforts real, paid_share real, land real);
create table comforts (day integer, town integer, kind text, met real);
create table herds (day integer, town integer, animal text, count real, pastures integer);
create table hands (day integer, town integer, hand integer, trade text, level integer, away integer);
create table farms (day integer, town integer, crop text, plots integer, per_day real, demand real, idle real, census real, draws real, needs real);
create table works (day integer, town integer, what text, progress real, done integer, mood real);
create table farm_towns (day integer, town integer, focus text, fields_planted integer, fallow integer, spare_plots integer, harvested real);
create table stock (day integer, town integer, good text, count real);
create table routes (day integer, route integer, from_town integer, to_town integer, blocks integer, trips integer,
    sold real, carried real, bought real, paid real, on_road integer, stayed text, far_purse real);
create table trips (route integer, from_town integer, to_town integer, left_tick integer, left_day integer, home_tick integer,
    home_day integer, stacks integer, worth real, purse real, sold real, takings real, bought real, paid real,
    pallet_items real);
create table plans (tick integer, day integer, route integer, worth real, cost real, purse real, full integer, on_its_day integer,
    pressing integer, went integer, stacks integer);
create table plan_whys (tick integer, day integer, route integer, good text, why text, count real);
create table lockers (day integer, town integer, hand integer, managed integer, client integer, placed text, offer integer,
    good text, sells integer, count integer, emeralds integer, uses integer, used integer, level integer, \"on\" integer);
create table whys (day integer, town integer, name text, line text, times integer, items real, last integer);
create table cash (town integer, name text, day integer, what text, amount real);
create table paths (route integer, step integer, q integer, r integer);
create table trails (q integer, r integer, width integer, wear real);
";

/// Replays `requests` into `conn`, writing the tables above.
pub fn trace(requests: &[&[u8]], conn: &mut Connection) -> Result<Traced, String> {
    let sql = |e: rusqlite::Error| e.to_string();
    conn.execute_batch(SCHEMA_SQL).map_err(sql)?;
    let tx = conn.transaction().map_err(sql)?;
    let mut society: Option<Society> = None;
    let mut traced = Traced::default();
    let mut snapped: Option<u64> = None;
    let mut asking: Option<(u32, u64)> = None;
    let mut planned: BTreeMap<usize, u64> = BTreeMap::new();
    let mut out_on_road: BTreeMap<usize, Trip> = BTreeMap::new();
    let mut pallets: BTreeMap<u64, f64> = BTreeMap::new();
    for (seq, bytes) in requests.iter().enumerate() {
        let request: Request = postcard::from_bytes(bytes).map_err(|e| format!("request {seq} unreadable: {e}"))?;
        if request.schema != wire::SCHEMA {
            return Err(format!("request {seq} is of wire schema {:016x}, this core's is {:016x}: replay it with the core \
                that wrote it", request.schema, wire::SCHEMA));
        }
        asking = Some((request.abi, request.schema));
        let (command, inputs) = match &request.command {
            Command::Create { .. } => ("Create", Vec::new()),
            Command::Load { .. } => ("Load", Vec::new()),
            Command::Save => ("Save", Vec::new()),
            Command::Apply { inputs } => ("Apply", inputs.iter().map(|i| serde_json::to_value(i).unwrap()).collect()),
            Command::Ask { .. } => ("Ask", Vec::new()),
        };
        let before = society.as_ref().map_or(0, Society::now);
        let response = wire::handle(&mut society, request);
        let Some(s) = society.as_ref() else {
            return Err(format!("request {seq} ({command}) left no society"));
        };
        let day_ticks = s.content().day_ticks.max(1);
        let goods: Vec<String> = s.content().goods.iter().map(|g| bare(&g.name)).collect();
        for (i, v) in inputs.iter().enumerate() {
            row(&tx, "inputs", seq, i, before, before / day_ticks, v, &goods)?;
        }
        let (outputs, refused) = match &response {
            Response::Done { outputs, .. } => (outputs.iter().map(|o| serde_json::to_value(o).unwrap()).collect(), None),
            Response::Refused { why } => (Vec::new(), Some(format!("{why:?}"))),
            Response::Saved { .. } => (Vec::new(), None),
        };
        let now = s.now();
        for (i, v) in outputs.iter().enumerate() {
            row(&tx, "outputs", seq, i, now, now / day_ticks, v, &goods)?;
        }
        plans(&tx, society.as_mut().unwrap(), &mut planned, &goods)?;
        trips(&tx, society.as_mut().unwrap(), &outputs, &mut out_on_road, &mut pallets)?;
        traced.refused += refused.is_some() as usize;
        tx.execute("insert into requests values (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![seq, command, now, now / day_ticks, inputs.len(), outputs.len(), refused]).map_err(sql)?;
        traced.requests += 1;
        // The day's state, once a request reaches it (and the first's, whatever its day).
        let day = now / day_ticks;
        if snapped != Some(day) {
            snapshot(&tx, &mut society, day, asking.unwrap())?;
            snapped = Some(day);
        }
    }
    let Some(s) = society.as_ref() else { return Err("the journal has no requests".into()) };
    let day = s.now() / s.content().day_ticks.max(1);
    // The last day as the run ended, in place of its first look's.
    for table in ["towns", "comforts", "herds", "hands", "farms", "farm_towns", "works", "stock", "routes", "lockers", "whys"] {
        tx.execute(&format!("delete from {table} where day = ?1"), params![day]).map_err(|e| e.to_string())?;
    }
    snapshot(&tx, &mut society, day, asking.unwrap())?;
    finals(&tx, &mut society, asking.unwrap())?;
    let s = society.as_ref().unwrap();
    traced.days = day;
    traced.towns = s.town_count();
    traced.routes = s.route_count();
    tx.commit().map_err(sql)?;
    Ok(traced)
}

/// An input or output: its kind (the variant), the town, route and good it names, if any, and the whole as JSON.
#[allow(clippy::too_many_arguments)]
fn row(tx: &rusqlite::Transaction, table: &str, seq: usize, i: usize, tick: u64, day: u64, v: &Value, goods: &[String])
        -> Result<(), String> {
    let (kind, inner) = match v {
        Value::Object(m) if m.len() == 1 => {
            let (k, inner) = m.iter().next().unwrap();
            (k.clone(), Some(inner))
        }
        Value::String(k) => (k.clone(), None),
        other => (other.to_string(), None),
    };
    let field = |names: &[&str]| -> Option<u64> {
        let inner = inner?;
        names.iter().find_map(|n| inner.get(*n).and_then(Value::as_u64))
    };
    let good = field(&["good"]).and_then(|g| goods.get(g as usize).cloned());
    tx.execute(&format!("insert into {table} values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"),
        params![seq, i, tick, day, kind, field(&["town", "from"]), field(&["route"]), good, v.to_string()])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Each town, its stock, each route and its latest plan's whys, and each town's lockers, as they are now.
fn snapshot(tx: &rusqlite::Transaction, society: &mut Option<Society>, day: u64, (abi, schema): (u32, u64))
        -> Result<(), String> {
    let sql = |e: rusqlite::Error| e.to_string();
    let s = society.as_mut().expect("a society");
    let goods: Vec<String> = s.content().goods.iter().map(|g| bare(&g.name)).collect();
    // The goods by number, once (a work's or an output's good is its number: `Field { crop }`).
    let listed: i64 = tx.query_row("select count(*) from goods", [], |r| r.get(0)).map_err(sql)?;
    if listed == 0 {
        for (i, g) in s.content().goods.iter().enumerate() {
            tx.execute("insert into goods values (?1, ?2, ?3, ?4)", params![i, bare(&g.name), g.crop, g.value]).map_err(sql)?;
        }
    }
    for t in 0..s.town_count() {
        let v = s.town(t);
        let target = s.mood_target(t);
        tx.execute("insert into towns values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![day, t, v.name, v.people.villagers, v.people.beds, v.people.mood, v.money.to_f64(), target,
                v.people.comfort_met.iter().sum::<f64>(), v.people.paid_share, v.people.land]).map_err(sql)?;
        for h in s.roster(t).hands {
            let trade = s.content().trades.get(h.trade).map(|x| x.name.clone()).unwrap_or_default();
            tx.execute("insert into hands values (?1, ?2, ?3, ?4, ?5, ?6)", params![day, t, h.id, trade, h.level, h.away])
                .map_err(sql)?;
        }
        if let Some(f) = s.farm(t) {
            let f = f.farm;
            tx.execute("insert into farm_towns values (?1, ?2, ?3, ?4, ?5, ?6, ?7)", params![day, t, format!("{:?}", f.focus),
                f.fields_planted, f.fallow, f.spare_plots, f.harvested]).map_err(sql)?;
            let crops: std::collections::BTreeSet<usize> = f.plots.keys().chain(f.per_day.keys()).chain(f.demand.keys())
                .chain(f.idle.keys()).chain(f.census.keys()).chain(f.draws.keys()).chain(f.needs.keys()).copied().collect();
            for g in crops {
                let get = |m: &BTreeMap<usize, f64>| m.get(&g).copied().unwrap_or(0.0);
                tx.execute("insert into farms values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)", params![day, t, goods[g],
                    f.plots.get(&g).copied().unwrap_or(0), get(&f.per_day), get(&f.demand), get(&f.idle), get(&f.census),
                    f.draws.get(&g).map_or(0.0, |d| d.0), get(&f.needs)]).map_err(sql)?;
            }
        }
        let works = s.works(t);
        for p in &works.projects {
            tx.execute("insert into works values (?1, ?2, ?3, ?4, ?5, ?6)", params![day, t, format!("{:?}", p.what),
                p.progress(), p.done, works.mood()]).map_err(sql)?;
        }
        let herds = s.herds(t);
        for (a, n) in herds.count.iter().enumerate() {
            let animal = s.content().animals.get(a).map(|x| x.name.clone()).unwrap_or_default();
            tx.execute("insert into herds values (?1, ?2, ?3, ?4, ?5)", params![day, t, animal, n, herds.pastures]).map_err(sql)?;
        }
        for (k, met) in v.people.comfort_met.iter().enumerate() {
            let kind = s.content().comforts.get(k).cloned().unwrap_or_default();
            tx.execute("insert into comforts values (?1, ?2, ?3, ?4)", params![day, t, kind, met]).map_err(sql)?;
        }
        for (g, n) in v.stock.iter().enumerate() {
            if n.abs() >= 1e-9 {
                tx.execute("insert into stock values (?1, ?2, ?3, ?4)", params![day, t, goods[g], n]).map_err(sql)?;
            }
        }
    }
    let mut q = s.world.query::<(&Route, &RouteState)>();
    let mut routes: BTreeMap<usize, (Route, RouteState)> = BTreeMap::new();
    for (r, st) in q.iter(&s.world) {
        routes.insert(r.seq, (*r, st.clone()));
    }
    for (seq, (r, st)) in routes {
        tx.execute("insert into routes values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![day, seq, r.from_id, r.to_id, r.blocks, st.trips, st.sold, st.carried, st.bought,
                st.purchases.to_f64(), st.wagon.is_some(),
                st.stayed.map(|w| format!("{w:?}")), st.far_purse.map(|p| p.to_f64())]).map_err(sql)?;
    }
    let towns = s.town_count() as u32;
    let mut queries: Vec<Query> = (0..towns).map(|town| Query::Lockers { town }).collect();
    queries.extend((0..towns).map(|town| Query::Whys { town }));
    for answer in ask(society, abi, schema, queries)? {
        if let Answer::Whys { town, name, whys } = &answer {
            for w in whys {
                tx.execute("insert into whys values (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![day, town, name, w.line, w.times, w.items, w.last]).map_err(sql)?;
            }
        }
        let Answer::Lockers { town, lockers, .. } = answer else { continue };
        for l in lockers {
            let placed = l.at.as_ref().map(|p| format!("{} {} {}", p.x, p.y, p.z));
            for o in &l.offers {
                tx.execute("insert into lockers values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                    params![day, town, l.hand, l.managed, l.client, placed, o.offer, goods.get(o.good as usize).cloned(),
                        o.sells, o.count, o.emeralds, o.uses, o.used, o.level, o.on]).map_err(sql)?;
            }
        }
    }
    Ok(())
}

/// Each plan decided since the last request: once each (by the tick it was decided).
fn plans(tx: &rusqlite::Transaction, s: &mut Society, planned: &mut BTreeMap<usize, u64>, goods: &[String])
        -> Result<(), String> {
    let sql = |e: rusqlite::Error| e.to_string();
    let day_ticks = s.content().day_ticks.max(1);
    let mut q = s.world.query::<(&Route, &RouteState)>();
    for (r, st) in q.iter(&s.world) {
        let Some(p) = st.last_plan.as_ref() else { continue };
        if planned.get(&r.seq) == Some(&p.at) {
            continue;
        }
        planned.insert(r.seq, p.at);
        tx.execute("insert into plans values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![p.at, p.at / day_ticks, r.seq, p.worth, p.cost, p.purse, p.full, p.on_its_day, p.pressing, p.went,
                p.stacks])
            .map_err(sql)?;
        for (good, why, count) in &p.whys {
            tx.execute("insert into plan_whys values (?1, ?2, ?3, ?4, ?5, ?6)",
                params![p.at, p.at / day_ticks, r.seq, good.and_then(|g| goods.get(g).cloned()), format!("{why:?}"), count])
                .map_err(sql)?;
        }
    }
    Ok(())
}

/// A wagon on the road, from its `WagonLeft`: its route's counters as it left, to tell its trip's share at `WagonHome`.
struct Trip {
    /// None: it left before the journal began (a server started with a wagon out): what it took out is unknown.
    left: Option<u64>,
    stacks: u32,
    worth: f64,
    purse: f64,
    sold: f64,
    bought: f64,
    paid: f64,
    takings: f64,
    /// Items on the pallets put on this wagon while it was out (or as it left): contract goods, crated sales.
    pallet_items: f64,
}

/// Each wagon's trip, written when it's home: what it took out (stacks, worth, purse), sold there and took for it, and
/// bought there and paid (its route's counters, home less leaving).
fn trips(tx: &rusqlite::Transaction, s: &mut Society, outputs: &[Value], out: &mut BTreeMap<usize, Trip>,
        pallets: &mut BTreeMap<u64, f64>) -> Result<(), String> {
    let now = s.now();
    let day_ticks = s.content().day_ticks.max(1);
    let mut q = s.world.query::<(&Route, &RouteState)>();
    let routes: BTreeMap<usize, (Route, RouteState)> = q.iter(&s.world).map(|(r, st)| (r.seq, (*r, st.clone()))).collect();
    let field = |v: &Value, kind: &str, f: &str| v.get(kind).and_then(|o| o.get(f)).cloned();
    let fixed = |v: Option<Value>| v.and_then(|v| v.as_f64()).unwrap_or(0.0) / 1000.0;
    let blank = || Trip { left: None, stacks: 0, worth: 0.0, purse: 0.0, sold: 0.0, bought: 0.0, paid: 0.0, takings: 0.0,
        pallet_items: 0.0 };
    for v in outputs {
        if let Some(id) = field(v, "PalletPacked", "id").and_then(|i| i.as_u64()) {
            let items = field(v, "PalletPacked", "packages").and_then(|p| p.as_array().cloned()).unwrap_or_default().iter()
                .filter_map(|p| p.get("count").and_then(Value::as_f64)).sum();
            pallets.insert(id, items);
        } else if let Some(route) = field(v, "PalletMoved", "to").and_then(|to| to.get("Wagon").and_then(|w| w.get("route"))
                .and_then(Value::as_u64)) {
            let id = field(v, "PalletMoved", "id").and_then(|i| i.as_u64()).unwrap_or(u64::MAX);
            out.entry(route as usize).or_insert_with(blank).pallet_items += pallets.get(&id).copied().unwrap_or(0.0);
        } else if let Some(route) = field(v, "WagonLeft", "route").and_then(|r| r.as_u64()) {
            let Some((_, st)) = routes.get(&(route as usize)) else { continue };
            // (Pallets loaded as it left come in the same look's outputs, before or after this.)
            let loaded = out.get(&(route as usize)).map_or(0.0, |t| t.pallet_items);
            out.insert(route as usize, Trip { left: Some(now), pallet_items: loaded,
                stacks: field(v, "WagonLeft", "stacks").and_then(|n| n.as_u64()).unwrap_or(0) as u32,
                worth: field(v, "WagonLeft", "worth").and_then(|n| n.as_f64()).unwrap_or(0.0),
                purse: st.last_plan.as_ref().map_or(0.0, |p| p.purse),
                sold: st.sold, bought: st.bought, paid: st.purchases.to_f64(), takings: 0.0 });
        } else if let Some(route) = field(v, "WagonSold", "route").and_then(|r| r.as_u64()) {
            let t = out.entry(route as usize).or_insert_with(blank);
            t.takings += fixed(field(v, "WagonSold", "takings"));
        } else if let Some(route) = field(v, "WagonHome", "route").and_then(|r| r.as_u64()) {
            let Some((r, st)) = routes.get(&(route as usize)) else { continue };
            let t = out.remove(&(route as usize));
            // (One that left before the journal: only its takings are known.)
            let known = t.as_ref().filter(|t| t.left.is_some());
            tx.execute("insert into trips values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                params![r.seq, r.from_id, r.to_id, known.map(|t| t.left), known.map(|t| t.left.unwrap() / day_ticks), now,
                    now / day_ticks, known.map(|t| t.stacks), known.map(|t| t.worth), known.map(|t| t.purse),
                    known.map(|t| st.sold - t.sold), t.as_ref().map_or(0.0, |t| t.takings), known.map(|t| st.bought - t.bought),
                    known.map(|t| st.purchases.to_f64() - t.paid), t.as_ref().map_or(0.0, |t| t.pallet_items)])
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Each town's whole cash book, each road's path over the hexes and the trails its wagons wore, at the run's end.
fn finals(tx: &rusqlite::Transaction, society: &mut Option<Society>, (abi, schema): (u32, u64)) -> Result<(), String> {
    let sql = |e: rusqlite::Error| e.to_string();
    {
        let s = society.as_ref().expect("a society");
        for route in 0..s.route_count() {
            for (step, hex) in s.route_path(route).unwrap_or_default().into_iter().enumerate() {
                tx.execute("insert into paths values (?1, ?2, ?3, ?4)", params![route, step, hex.q, hex.r]).map_err(sql)?;
            }
        }
        let (map, now, day) = (s.map(), s.now(), s.content().day_ticks);
        for &hex in &map.trails {
            let wear = map.wear.get(&hex).map_or(0.0, |w| w.at(now, day));
            tx.execute("insert into trails values (?1, ?2, ?3, ?4)",
                params![hex.q, hex.r, map.widths.get(&hex).copied().unwrap_or(0), wear]).map_err(sql)?;
        }
    }
    let towns = society.as_ref().expect("a society").town_count() as u32;
    // (Every day there is: the book keeps them all.)
    let queries: Vec<Query> = (0..towns).map(|town| Query::CashBook { town, days: 1_000_000 }).collect();
    for answer in ask(society, abi, schema, queries)? {
        match answer {
            Answer::CashBook { town, name, lines, .. } => {
                for l in lines {
                    tx.execute("insert into cash values (?1, ?2, ?3, ?4, ?5)",
                        params![town, name, l.day, l.what, l.amount.to_f64()]).map_err(sql)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Asks the society, as the host would (asking changes nothing).
fn ask(society: &mut Option<Society>, abi: u32, schema: u64, queries: Vec<Query>) -> Result<Vec<Answer>, String> {
    match wire::handle(society, Request { abi, schema, command: Command::Ask { queries } }) {
        Response::Done { answers, .. } => Ok(answers),
        other => Err(format!("the core wouldn't answer: {other:?}")),
    }
}

fn bare(name: &str) -> String {
    name.strip_prefix("minecraft:").unwrap_or(name).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flags::Flags;
    use crate::goods::DAY;
    use crate::world::Study;
    use enterprise_core::wire::{Input, SaveFile};

    /// The study's world, two days in, journaled as the mod writes it (a `Load`, then looks), and traced: the replay
    /// ends where the same looks run natively do, and its tables say so.
    #[test]
    fn a_journal_traced_ends_where_the_run_did() {
        let argv: Vec<String> = ["12"].iter().map(|a| a.to_string()).collect();
        let mut st = Study::build(Flags::parse(&argv).unwrap(), 1);
        st.s.advance(2 * DAY);
        let look = DAY / 20;
        let now = st.s.now();
        let mut requests = vec![Request::new(Command::Load { save: SaveFile::of(&mut st.s) })];
        for k in 1..=10 * 20 {
            requests.push(Request::new(Command::Apply { inputs: vec![Input::Advance { to: now + k * look }] }));
        }
        let mut journal = MAGIC.to_vec();
        for r in &requests {
            let b = r.to_bytes();
            journal.extend((b.len() as u32).to_be_bytes());
            journal.extend(b);
        }
        journal.extend([0, 0, 1]); // a last request cut short: left out
        // The same looks, natively.
        let mut ran: Option<Society> = None;
        for r in &requests {
            assert!(matches!(wire::handle(&mut ran, r.clone()), Response::Done { .. }));
        }
        let ran = ran.unwrap();

        let mut conn = Connection::open_in_memory().unwrap();
        let t = trace(&read(&journal).unwrap(), &mut conn).unwrap();
        assert_eq!((t.requests, t.refused), (requests.len(), 0));
        assert_eq!(t.days, ran.now() / DAY);
        let last = t.days as i64;
        for town in 0..ran.town_count() {
            let money: f64 = conn.query_row("select money from towns where day = ?1 and town = ?2", params![last, town],
                |r| r.get(0)).unwrap();
            assert_eq!(money, ran.town(town).money.to_f64(), "town {town}'s money");
        }
        for route in 0..ran.route_count() {
            let trips: u32 = conn.query_row("select trips from routes where day = ?1 and route = ?2", params![last, route],
                |r| r.get(0)).unwrap();
            assert_eq!(trips, ran.route(route).trips, "route {route}'s trips");
        }
        let count = |q: &str| -> i64 { conn.query_row(q, [], |r| r.get(0)).unwrap() };
        assert!(count("select count(*) from routes where trips > 0") > 0, "no wagon went");
        assert_eq!(count("select count(*) from inputs where kind = 'Advance'"), 200);
        assert!(count("select count(*) from outputs where kind = 'WagonLeft'") > 0, "no WagonLeft output");
        assert!(count("select count(distinct day) from towns") >= 10, "a snapshot a day");
        assert_eq!(count("select count(*) from towns") as usize,
            count("select count(distinct day) from towns") as usize * ran.town_count(), "a town once a day");
        assert!(count("select count(*) from stock") > 0 && count("select count(*) from cash") > 0);
        assert!(count("select count(*) from whys") > 0, "whys {}", count("select count(*) from whys"));
        assert!(count("select count(*) from plans where went = 1") > 0 && count("select count(*) from plans where went = 0") > 0,
            "plans that went and stayed");
        assert!(count("select count(*) from plan_whys") > 0, "no plan's reasons");
        assert_eq!(count("select count(*) from trips"), count("select count(*) from outputs where kind = 'WagonHome'"),
            "a row a trip home");
        let out = count("select count(*) from routes where on_road = 1 and day = (select max(day) from routes)");
        assert_eq!(count("select count(*) from trips where left_tick is not null"),
            count("select count(*) from outputs where kind = 'WagonLeft'") - out, "a known start a trip left and home");
        assert!(count("select count(*) from trips where sold > 0") > 0, "no trip sold anything");
    }

    #[test]
    fn a_stranger_is_refused() {
        assert!(read(b"not a journal").is_err());
        assert_eq!(read(MAGIC).unwrap().len(), 0);
    }
}
