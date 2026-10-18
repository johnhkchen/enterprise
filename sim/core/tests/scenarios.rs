//! Scenarios (`sim/scenarios/NAME.json`): scripts both sides of the boundary play. A scenario is a list of steps, each a
//! request and the response the core gives to it; a host plays them in order against one society, kept between steps,
//! and every answer must be the expected one. The same files run natively (this test), under Node (the test built for
//! `wasm32-wasip1`, `just scenarios-wasm`) and in the JVM through Chicory (`binding-test`), so the core, its
//! WebAssembly build and the Java that hosts it are held to one set of answers.
//!
//! A step holds exactly one of
//! - `request`: a [`Request`] in JSON (the readable twin of the bytes sent);
//! - `request_hex`: the bytes of a request, in hex, for what JSON can't say (a NaN) or the core must refuse unread;
//! - `load`: `"saved"`, a `Load` of the save the last `Save` step gave (a save is too big to write in a file);
//!
//! and its expected answer, one of
//! - `response`: the [`Response`] in JSON;
//! - `saved`: for a `Save`, the save's format and schema, its blob's length and FNV-64 (the blob is the core's own
//!   bytes, which move with its rules as `replay_digest.txt` does; its `core` string moves with the build).
//!
//! The scripts are written here, in Rust, so each step is built from the wire's own types and its answer comes from the
//! core; `just wire-fixtures` writes the files (`WIRE_SCENARIOS=write`) and the diff is what changed in the
//! behaviour. This test plays the files, and checks they are what the scripts write now.

use enterprise_core::wire::*;
use enterprise_core::{Fixed, Society};
use serde::{Deserialize, Serialize};

/// Where the scenarios are, from the crate's folder (Cargo's working folder for its tests, natively and under Node's WASI).
const DIR: &str = if cfg!(target_family = "wasm") { "/sim/scenarios" } else { "../scenarios" };
const DAY: u64 = 24_000;

#[derive(Serialize, Deserialize)]
struct Scenario {
    name: String,
    about: String,
    steps: Vec<Step>,
}

#[derive(Default, Serialize, Deserialize)]
struct Step {
    note: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    request: Option<Request>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    request_hex: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    load: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    response: Option<Response>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    saved: Option<SavedDigest>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct SavedDigest {
    format: u32,
    schema: u64,
    blob_len: usize,
    blob_fnv: String,
}

fn fnv(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h = (h ^ *b as u64).wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len() / 2).map(|i| u8::from_str_radix(&text[2 * i..2 * i + 2], 16).expect("hex")).collect()
}

/// The host a scenario plays against: the society it keeps and the last save it was given.
#[derive(Default)]
struct Host {
    society: Option<Society>,
    saved: Option<SaveFile>,
}

/// What a step came to: the response, or for a save its digest.
enum Seen {
    Response(Response),
    Saved(SavedDigest),
}

impl Host {
    /// Plays a step's request as a host does, as bytes through [`handle_bytes`].
    fn play(&mut self, step: &Step) -> Seen {
        let bytes = match (&step.request, &step.request_hex, &step.load) {
            (Some(r), None, None) => postcard::to_allocvec(r).expect("a request serializes"),
            (None, Some(h), None) => unhex(h),
            (None, None, Some(what)) => {
                assert_eq!(what, "saved", "{}: load takes \"saved\"", step.note);
                let save = self.saved.clone().unwrap_or_else(|| panic!("{}: no save to load yet", step.note));
                postcard::to_allocvec(&Request::new(Command::Load { save })).unwrap()
            }
            _ => panic!("{}: a step has exactly one of request, request_hex, load", step.note),
        };
        let out = handle_bytes(&mut self.society, &bytes);
        match postcard::from_bytes::<Response>(&out).expect("the core's response reads") {
            Response::Saved { save } => {
                let seen = SavedDigest { format: save.format, schema: save.schema, blob_len: save.blob.len(), blob_fnv: fnv(&save.blob) };
                self.saved = Some(save);
                Seen::Saved(seen)
            }
            r => Seen::Response(r),
        }
    }
}

/// Plays a scenario and says every step that didn't give its expected answer.
fn play(scenario: &Scenario) -> Vec<String> {
    let mut host = Host::default();
    let mut wrong = Vec::new();
    for (i, step) in scenario.steps.iter().enumerate() {
        let said = format!("{} step {i} ({})", scenario.name, step.note);
        match (host.play(step), &step.response, &step.saved) {
            (Seen::Response(got), Some(want), None) => {
                if &got != want {
                    wrong.push(format!("{said}: wanted {want:?}, got {got:?}"));
                }
            }
            (Seen::Saved(got), None, Some(want)) => {
                if &got != want {
                    wrong.push(format!("{said}: wanted a save of {want:?}, got {got:?}"));
                }
            }
            (_, want, saved) => wrong.push(format!("{said}: answered another kind than expected ({want:?} / {saved:?})")),
        }
    }
    wrong
}

/// A scenario being written: each step is played as it's added, and its answer written beside it.
struct Script {
    scenario: Scenario,
    host: Host,
}

impl Script {
    fn new(name: &str, about: &str) -> Script {
        Script { scenario: Scenario { name: name.into(), about: about.into(), steps: Vec::new() }, host: Host::default() }
    }

    fn add(&mut self, mut step: Step) -> Option<Response> {
        match self.host.play(&step) {
            Seen::Response(r) => step.response = Some(r),
            Seen::Saved(d) => step.saved = Some(d),
        }
        let response = step.response.clone();
        self.scenario.steps.push(step);
        response
    }

    /// A command as this core builds requests.
    fn say(&mut self, note: &str, command: Command) -> Response {
        self.add(Step { note: note.into(), request: Some(Request::new(command)), ..Step::default() }).expect("a response")
    }

    /// A whole request as given (another ABI or schema than the core's).
    fn say_request(&mut self, note: &str, request: Request) -> Response {
        self.add(Step { note: note.into(), request: Some(request), ..Step::default() }).expect("a response")
    }

    fn say_bytes(&mut self, note: &str, bytes: &[u8]) -> Response {
        self.add(Step { note: note.into(), request_hex: Some(hex(bytes)), ..Step::default() }).expect("a response")
    }

    fn apply(&mut self, note: &str, inputs: Vec<Input>) -> Vec<Output> {
        match self.say(note, Command::Apply { inputs }) {
            Response::Done { outputs, .. } => outputs,
            r => panic!("{note}: {r:?}"),
        }
    }

    fn ask(&mut self, note: &str, queries: Vec<Query>) -> Vec<Answer> {
        match self.say(note, Command::Ask { queries }) {
            Response::Done { answers, .. } => answers,
            r => panic!("{note}: {r:?}"),
        }
    }

    fn create(&mut self, note: &str) {
        let r = self.say(note, Command::Create { pack: pack(), seed: 7 });
        assert_eq!(r, Response::Done { outputs: vec![], answers: vec![] });
    }

    fn save(&mut self, note: &str) {
        self.add(Step { note: note.into(), request: Some(Request::new(Command::Save)), ..Step::default() });
    }

    fn load(&mut self, note: &str) {
        let at = self.scenario.steps.len();
        self.add(Step { note: note.into(), load: Some("saved".into()), ..Step::default() });
        assert_eq!(self.scenario.steps[at].response, Some(Response::Done { outputs: vec![], answers: vec![] }), "{note}");
    }

    fn refused(&mut self, note: &str, r: Response) -> Refusal {
        match r {
            Response::Refused { why } => why,
            r => panic!("{note}: not refused: {r:?}"),
        }
    }
}

fn pack() -> ContentPack {
    let mut wheat = PackGood::new("minecraft:wheat", 0.05, 64);
    wheat.flags = vec![GoodFlag::Crop];
    let mut bread = PackGood::new("minecraft:bread", 0.2, 64);
    bread.food = 5.0;
    bread.saturation = 6.0;
    let mut book = PackGood::new("minecraft:book", 0.5, 64);
    book.comfort = Some("reading".into());
    let mut beef = PackGood::new("minecraft:cooked_beef", 0.3, 64);
    beef.food = 8.0;
    beef.saturation = 12.8;
    beef.flags = vec![GoodFlag::Meat];
    ContentPack { day_ticks: DAY, goods: vec![wheat, bread, book, beef] }
}

const WHEAT: usize = 0;
const BREAD: usize = 1;
const BOOK: usize = 2;
const BEEF: usize = 3;

fn at(kind: PointKind, x: i32, z: i32) -> Point {
    Point { kind, dim: "minecraft:overworld".into(), x, y: -60, z }
}

fn counts(of: &[(usize, f64)]) -> Vec<Count> {
    of.iter().map(|&(good, count)| Count { good, count }).collect()
}

fn stock_of(town: TownId) -> Vec<Query> {
    vec![Query::Stock { town }, Query::Points]
}

/// A town created, deliveries, a stocktake, its stock asked, saved, loaded, asked again.
fn stocktake() -> Scenario {
    let mut s = Script::new("stocktake", "A town at its desk takes a delivery and two stocktakes, is saved and loaded, and answers the same.");
    s.create("a society from a pack of four goods");
    s.apply("a town at its desk, a locker for it, some money paid in", vec![
        Input::AddTown { name: "Ashford".into(), desk: at(PointKind::Desk, 27, 27) },
        Input::AddPoint { point: at(PointKind::Locker, 30, 27), town: 0 },
        Input::Pay { town: 0, money: Fixed::emeralds(40) },
    ]);
    s.apply("a player delivers wheat and books", vec![
        Input::Deliver { town: 0, good: WHEAT, count: 40.0 },
        Input::Deliver { town: 0, good: BOOK, count: 3.0 },
    ]);
    s.ask("its stock and points", stock_of(0));
    s.apply("a stocktake at the desk counts from nothing", vec![
        Input::Stocktake { point: at(PointKind::Desk, 27, 27), counts: counts(&[(WHEAT, 30.0), (BREAD, 5.0)]) },
    ]);
    s.apply("the locker is counted apart, a good listed twice is summed", vec![
        Input::Stocktake { point: at(PointKind::Locker, 30, 27), counts: counts(&[(BREAD, 1.0), (BREAD, 2.0)]) },
    ]);
    s.apply("a player took 20 wheat out and put 2 books in; three days pass", vec![
        Input::Stocktake { point: at(PointKind::Desk, 27, 27), counts: counts(&[(WHEAT, 10.0), (BREAD, 5.0), (BOOK, 2.0)]) },
        Input::Advance { to: 3 * DAY },
    ]);
    let before = s.ask("the stock after the stocktakes", stock_of(0));
    s.save("saved");
    s.load("loaded");
    let after = s.ask("asked again after the load", stock_of(0));
    assert_eq!(before, after, "a society loaded answers as the one saved did");
    s.apply("it runs on: five more days", vec![Input::Advance { to: 8 * DAY }]);
    s.ask("and answers", stock_of(0));
    s.scenario
}

/// Every kind of happening and question, accepted.
fn inputs() -> Scenario {
    let mut s = Script::new("inputs", "Every happening the host can hand the core, and every question, each accepted.");
    s.create("a society from a pack of four goods");
    s.apply("two towns at their desks", vec![
        Input::AddTown { name: "Ashford".into(), desk: at(PointKind::Desk, 27, 27) },
        Input::AddTown { name: "Br\u{f6}ly".into(), desk: at(PointKind::Desk, 600, -40) },
    ]);
    s.apply("a point of each kind, placed", vec![
        Input::AddPoint { point: at(PointKind::Locker, 30, 27), town: 0 },
        Input::AddPoint { point: at(PointKind::Shop, 24, 27), town: 0 },
        Input::AddPoint { point: at(PointKind::Banker, 27, 30), town: 0 },
        Input::AddPoint { point: at(PointKind::Depot, 603, -40), town: 1 },
    ]);
    s.apply("money and goods from outside", vec![
        Input::Pay { town: 0, money: Fixed::emeralds(100) },
        Input::Pay { town: 1, money: Fixed::from_millis(2500) },
        Input::Deliver { town: 0, good: BREAD, count: 64.0 },
        Input::Deliver { town: 1, good: BEEF, count: 20.0 },
    ]);
    s.apply("a buying shop, a selling shop, a shop for the Merchant's own", vec![
        Input::Order { town: 0, good: WHEAT, lot: 16.0, price: 1.0, purse: 50.0 },
        Input::Stall { town: 0, good: BOOK, lot: 1.0, price: 2.0, stock: 8.0 },
        Input::OwnShop { town: 1 },
    ]);
    s.apply("every point counted", vec![
        Input::Stocktake { point: at(PointKind::Desk, 27, 27), counts: counts(&[(BREAD, 60.0), (BOOK, 8.0)]) },
        Input::Stocktake { point: at(PointKind::Locker, 30, 27), counts: counts(&[(WHEAT, 12.0)]) },
        Input::Stocktake { point: at(PointKind::Shop, 24, 27), counts: counts(&[]) },
        Input::Stocktake { point: at(PointKind::Banker, 27, 30), counts: counts(&[]) },
        Input::Stocktake { point: at(PointKind::Depot, 603, -40), counts: counts(&[(BEEF, 20.0)]) },
    ]);
    s.apply("a player buys from a town's own shop", vec![Input::Buy { town: 1, good: BEEF, count: 2.0 }]);
    s.apply("a point broken", vec![Input::RemovePoint { point: at(PointKind::Shop, 24, 27) }]);
    s.apply("twenty days pass", vec![Input::Advance { to: 20 * DAY }]);
    s.ask("every question", vec![Query::Stock { town: 0 }, Query::Stock { town: 1 }, Query::Points]);
    s.scenario
}

/// Every way the core refuses, each leaving the society as it was.
fn refusals() -> Scenario {
    let mut s = Script::new("refusals", "Every refusal, each a message and nothing applied.");
    let ask_one = |s: &mut Script, note: &str| s.ask(note, vec![Query::Stock { town: 0 }]);
    let r = s.say("a question before there is a society", Command::Ask { queries: vec![Query::Points] });
    assert_eq!(s.refused("no society", r), Refusal::NoSociety);
    let r = s.say("a save before there is a society", Command::Save);
    assert_eq!(s.refused("no society", r), Refusal::NoSociety);
    let mut bad = pack();
    bad.day_ticks = 0;
    let r = s.say("a pack with a day of no ticks", Command::Create { pack: bad, seed: 1 });
    assert!(matches!(s.refused("pack", r), Refusal::Pack { .. }));
    let r = s.say_request("built against another ABI", Request { abi: 0, schema: SCHEMA, command: Command::Save });
    assert_eq!(s.refused("abi", r), Refusal::Abi { core: ABI, request: 0 });
    let r = s.say_request("built against another schema", Request { abi: ABI, schema: u64::MAX, command: Command::Save });
    assert_eq!(s.refused("schema", r), Refusal::Schema { core: SCHEMA, request: u64::MAX });
    let r = s.say_bytes("bytes that are no request", &[0xff, 0xff, 0xff]);
    assert!(matches!(s.refused("unreadable", r), Refusal::Unreadable { .. }));
    let r = s.say_bytes("no bytes at all", &[]);
    assert!(matches!(s.refused("unreadable", r), Refusal::Unreadable { .. }));
    let whole = postcard::to_allocvec(&Request::new(Command::Create { pack: pack(), seed: 7 })).unwrap();
    let r = s.say_bytes("a request cut short", &whole[..whole.len() - 3]);
    assert!(matches!(s.refused("unreadable", r), Refusal::Unreadable { .. }));

    s.create("a society");
    s.apply("a town with some wheat", vec![
        Input::AddTown { name: "Ashford".into(), desk: at(PointKind::Desk, 27, 27) },
        Input::Deliver { town: 0, good: WHEAT, count: 40.0 },
    ]);
    let kept = ask_one(&mut s, "its stock");

    let r = s.say("a save of another format", Command::Load {
        save: SaveFile { format: u32::MAX, schema: SCHEMA, core: "other".into(), blob: vec![1, 2, 3] },
    });
    assert!(matches!(s.refused("save format", r), Refusal::SaveFormat { .. }));
    let r = s.say("a save the core can't read", Command::Load {
        save: SaveFile { format: enterprise_core::save::FORMAT, schema: SCHEMA, core: "other".into(), blob: vec![1, 2, 3] },
    });
    assert!(matches!(s.refused("save", r), Refusal::Save { .. }));
    assert_eq!(ask_one(&mut s, "a refused load keeps the society"), kept);

    let refuse_apply = |s: &mut Script, note: &str, inputs: Vec<Input>, at: u32| {
        let r = s.say(note, Command::Apply { inputs });
        match s.refused(note, r) {
            Refusal::Input { at: got, .. } => assert_eq!(got, at, "{note}"),
            r => panic!("{note}: {r:?}"),
        }
    };
    refuse_apply(&mut s, "a delivery to a town that isn't there, after one that would be fine", vec![
        Input::Deliver { town: 0, good: WHEAT, count: 5.0 },
        Input::Deliver { town: 9, good: WHEAT, count: 5.0 },
    ], 1);
    refuse_apply(&mut s, "a good that isn't there", vec![Input::Deliver { town: 0, good: 99, count: 1.0 }], 0);
    refuse_apply(&mut s, "a count below nothing", vec![Input::Buy { town: 0, good: WHEAT, count: -1.0 }], 0);
    refuse_apply(&mut s, "money below nothing", vec![Input::Pay { town: 0, money: Fixed::from_millis(-1) }], 0);
    refuse_apply(&mut s, "a stocktake of a point that isn't there", vec![
        Input::Stocktake { point: at(PointKind::Locker, 1, 1), counts: vec![] },
    ], 0);
    refuse_apply(&mut s, "a stocktake naming another kind than the point standing there", vec![
        Input::Stocktake { point: at(PointKind::Shop, 27, 27), counts: vec![] },
    ], 0);
    refuse_apply(&mut s, "a point where one stands already", vec![Input::AddPoint { point: at(PointKind::Locker, 27, 27), town: 0 }], 0);
    refuse_apply(&mut s, "a town whose desk isn't a desk", vec![
        Input::AddTown { name: "Nowhere".into(), desk: at(PointKind::Locker, 5, 5) },
    ], 0);
    refuse_apply(&mut s, "an advance past a hundred years", vec![Input::Advance { to: u64::MAX }], 0);
    // NaN and infinity are no JSON: those requests go as bytes.
    for (note, input) in [
        ("a count of NaN, as bytes", Input::Deliver { town: 0, good: WHEAT, count: f64::NAN }),
        ("a price of infinity, as bytes", Input::Order { town: 0, good: WHEAT, lot: 1.0, price: f64::INFINITY, purse: 1.0 }),
    ] {
        let bytes = postcard::to_allocvec(&Request::new(Command::Apply { inputs: vec![input] })).unwrap();
        let r = s.say_bytes(note, &bytes);
        assert!(matches!(s.refused(note, r), Refusal::Input { at: 0, .. }), "{note}");
    }
    let r = s.say("a question about a town that isn't there", Command::Ask { queries: vec![Query::Points, Query::Stock { town: 4 }] });
    assert!(matches!(s.refused("query", r), Refusal::Query { at: 1, .. }));
    assert_eq!(ask_one(&mut s, "nothing of any refused request applied"), kept);
    s.scenario
}

fn scenarios() -> Vec<Scenario> {
    vec![stocktake(), inputs(), refusals()]
}

fn text(scenario: &Scenario) -> String {
    serde_json::to_string_pretty(scenario).expect("a scenario is JSON") + "\n"
}

fn path(name: &str) -> String {
    format!("{DIR}/{name}.json")
}

fn read(name: &str) -> Scenario {
    let file = path(name);
    let text = std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("{file}: {e} (`just wire-fixtures` writes it)"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{file}: {e}"))
}

#[test]
fn the_scenarios_play_as_their_files_say() {
    if std::env::var("WIRE_SCENARIOS").as_deref() == Ok("write") {
        std::fs::create_dir_all(DIR).unwrap();
        for s in scenarios() {
            std::fs::write(path(&s.name), text(&s)).expect("scenario written");
        }
        return;
    }
    let mut wrong = Vec::new();
    for name in ["stocktake", "inputs", "refusals"] {
        let scenario = read(name);
        assert!(!scenario.steps.is_empty(), "{name}: no steps");
        wrong.extend(play(&scenario));
    }
    assert!(wrong.is_empty(), "the core's answers differ from the scenarios' ({} steps; if the behaviour moved on purpose, \
        `just wire-fixtures` writes the new answers):\n{}", wrong.len(), wrong.join("\n"));
}

/// The files are what the scripts here write now: a step added or a rule moved shows as a diff in the files.
#[test]
fn the_files_are_what_the_scripts_write() {
    if std::env::var("WIRE_SCENARIOS").as_deref() == Ok("write") {
        return;
    }
    for s in scenarios() {
        let want = std::fs::read_to_string(path(&s.name)).unwrap_or_else(|e| panic!("{}: {e}", path(&s.name)));
        assert!(text(&s) == want, "{}: the script writes other text than the file has; `just wire-fixtures` writes it", path(&s.name));
    }
}

/// A scenario shows each happening and question the wire has (a new one is added to its list here, and to a script).
#[test]
fn every_message_appears_in_a_scenario() {
    let mut inputs = std::collections::BTreeSet::new();
    let mut commands = std::collections::BTreeSet::new();
    let mut queries = std::collections::BTreeSet::new();
    let mut refusals = std::collections::BTreeSet::new();
    let variant = |v: serde_json::Value| match v {
        serde_json::Value::String(s) => s,
        serde_json::Value::Object(o) => o.keys().next().cloned().unwrap_or_default(),
        v => panic!("{v}"),
    };
    for scenario in scenarios() {
        for step in &scenario.steps {
            let Some(Request { command, .. }) = &step.request else { continue };
            commands.insert(variant(serde_json::to_value(command).unwrap()));
            match command {
                Command::Apply { inputs: is } => inputs.extend(is.iter().map(|i| variant(serde_json::to_value(i).unwrap()))),
                Command::Ask { queries: qs } => queries.extend(qs.iter().map(|q| variant(serde_json::to_value(q).unwrap()))),
                _ => {}
            }
            if let Some(Response::Refused { why }) = &step.response {
                refusals.insert(variant(serde_json::to_value(why).unwrap()));
            }
        }
        for step in &scenario.steps {
            if step.load.is_some() {
                commands.insert("Load".to_string());
            }
            if let Some(Response::Refused { why }) = &step.response {
                refusals.insert(variant(serde_json::to_value(why).unwrap()));
            }
        }
    }
    let set = |names: &[&str]| names.iter().map(|s| s.to_string()).collect::<std::collections::BTreeSet<_>>();
    assert_eq!(inputs, set(&["Advance", "AddPoint", "AddTown", "Buy", "Deliver", "Order", "OwnShop", "Pay", "RemovePoint", "Stall", "Stocktake"]));
    assert_eq!(commands, set(&["Apply", "Ask", "Create", "Load", "Save"]));
    assert_eq!(queries, set(&["Points", "Stock"]));
    assert_eq!(refusals, set(&["Abi", "Input", "NoSociety", "Pack", "Query", "Save", "SaveFormat", "Schema", "Unreadable"]));
}
