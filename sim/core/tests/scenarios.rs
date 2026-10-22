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
//! A `request` and a `response` each have their postcard twin beside them, in hex (`request_postcard`,
//! `response_postcard`): the bytes a host sends and the bytes it gets back, so a host plays a scenario with its own
//! generated decoders and no JSON reader of the wire's types (the Java host, `binding/`'s `ScenariosTest`). This test
//! checks every twin is its JSON's bytes.
//!
//! A refusal matches by its kind and numbers; its `why` is prose and isn't compared ([`without_prose`]).
//!
//! `people` shows a child born (`Born`), `chests` the chests' moves (`Moved`), `money` the till's (`Cash`) and
//! `lockers` a locker's trades levelling its villager (`HandLevel`), `wagons` a route started by its host and its crew
//! away and back (`WagonLeft`, `HandAway`, `HandBack`); the others need a master Merchant or a party of settlers, and
//! the wire has no input that makes either. Their bytes are pinned by the
//! golden files meanwhile
//! (`tests/wire_fixtures.rs`).
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
    request_postcard: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    request_hex: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    load: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    response: Option<Response>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    response_postcard: Option<String>,
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

/// A refusal's `why` is prose for people (it holds a library's error text and the core's own wording), so a host
/// compares a refusal by its kind and its numbers, never its words: the Java host does the same.
fn without_prose(r: &Response) -> Response {
    let mut r = r.clone();
    if let Response::Refused { why } = &mut r {
        match why {
            Refusal::Unreadable { why } | Refusal::Pack { why } | Refusal::Save { why } | Refusal::Input { why, .. }
            | Refusal::Query { why, .. } => why.clear(),
            Refusal::Abi { .. } | Refusal::Schema { .. } | Refusal::NoSociety | Refusal::SaveFormat { .. } => {}
        }
    }
    r
}

/// Plays a scenario and says every step that didn't give its expected answer.
fn play(scenario: &Scenario) -> Vec<String> {
    let mut host = Host::default();
    let mut wrong = Vec::new();
    for (i, step) in scenario.steps.iter().enumerate() {
        let said = format!("{} step {i} ({})", scenario.name, step.note);
        if step.request_postcard != step.request.as_ref().map(|r| hex(&postcard::to_allocvec(r).unwrap())) {
            wrong.push(format!("{said}: request_postcard isn't the request's bytes"));
        }
        if step.response_postcard != step.response.as_ref().map(|r| hex(&postcard::to_allocvec(r).unwrap())) {
            wrong.push(format!("{said}: response_postcard isn't the response's bytes"));
        }
        match (host.play(step), &step.response, &step.saved) {
            (Seen::Response(got), Some(want), None) => {
                if without_prose(&got) != without_prose(want) {
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
            Seen::Response(r) => {
                step.response_postcard = Some(hex(&postcard::to_allocvec(&r).unwrap()));
                step.response = Some(r);
            }
            Seen::Saved(d) => step.saved = Some(d),
        }
        step.request_postcard = step.request.as_ref().map(|r| hex(&postcard::to_allocvec(r).unwrap()));
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
    ContentPack::new(DAY, vec![wheat, bread, book, beef])
}

const WHEAT: GoodNo = 0;
const BREAD: GoodNo = 1;
const BOOK: GoodNo = 2;
const BEEF: GoodNo = 3;

fn at(kind: PointKind, x: i32, z: i32) -> Point {
    Point { kind, dim: "minecraft:overworld".into(), x, y: -60, z }
}

fn counts(of: &[(GoodNo, f64)]) -> Vec<Count> {
    of.iter().map(|&(good, count)| Count { good, count }).collect()
}

fn stock_of(town: TownNo) -> Vec<Query> {
    vec![Query::Stock { town }, Query::Points]
}

/// A town created, deliveries, a stocktake, its stock asked, saved, loaded, asked again.
fn stocktake() -> Scenario {
    let mut s = Script::new("stocktake", "A town at its desk takes a delivery and two stocktakes, is saved and loaded, and answers the same. \
        A delivery handed to the town outside its chests is added to what the desk's first count finds, and shown in \
        the desk's chests at the end of that look (Moved): its next count finds it there and books it no more.");
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
    s.apply("a stocktake at the desk counts from nothing: its 30 wheat and 5 bread are added to the 40 wheat and 3 books \
        delivered, which are moved into its chests", vec![
        Input::Stocktake { point: at(PointKind::Desk, 27, 27), counts: counts(&[(WHEAT, 30.0), (BREAD, 5.0)]), free: 27 },
    ]);
    s.apply("the locker is counted apart, a good listed twice is summed", vec![
        Input::Stocktake { point: at(PointKind::Locker, 30, 27), counts: counts(&[(BREAD, 1.0), (BREAD, 2.0)]), free: 27 },
    ]);
    s.apply("a player took 20 wheat out of the desk and put 2 books in (stock 70 wheat to 50); three days pass", vec![
        Input::Stocktake { point: at(PointKind::Desk, 27, 27), counts: counts(&[(WHEAT, 50.0), (BREAD, 5.0), (BOOK, 5.0)]), free: 25 },
        Input::Advance { to: 3 * DAY },
    ]);
    let before = s.ask("the stock after the stocktakes", stock_of(0));
    s.save("saved");
    s.load("loaded");
    let after = s.ask("asked again after the load", stock_of(0));
    assert_eq!(before, after, "a society loaded answers as the one saved did");
    s.ask("the goods by number, as the save keeps them (what a host maps its items by after a load)", vec![Query::Goods]);
    s.apply("it runs on: five more days", vec![Input::Advance { to: 8 * DAY }]);
    s.ask("and answers", stock_of(0));
    s.scenario
}

/// A desk's and a depot's chests showing their town's stock (`chests.rs`): what the core moved in or out since they were
/// counted, at the end of each look that counts them; what the host couldn't do, back; a desk broken and put back.
fn chests() -> Scenario {
    let mut s = Script::new("chests", "A town's stock shown in its desk's and depot's chests, at the end of each look that \
        counted them: the stack its master Merchant puts on its own shop's shelf is moved out of the desk's chests, and \
        books delivered to the town are moved into the chests' free slots, the desk's first. What didn't fit (a hopper \
        filled the slot first) comes back as Unmoved and waits on the town's books until a count shows room. A desk \
        broken takes its chests' goods with it, and put back where it stood is the same town, its chests counted in \
        once. Saved and loaded, the chests are as the core took them.");
    let desk = at(PointKind::Desk, 27, 27);
    let depot = at(PointKind::Depot, 31, 27);
    let held = |s: &mut Script, note: &str| s.ask(note, vec![
        Query::Stock { town: 0 }, Query::Points, Query::Chests { point: desk.clone() }, Query::Chests { point: depot.clone() },
    ]);
    s.create("a society from a pack of four goods");
    s.apply("a town at its desk, a depot of its, its master Merchant and its own shop", vec![
        Input::AddTown { name: "Ashford".into(), desk: desk.clone() },
        Input::AddPoint { point: depot.clone(), town: 0 },
        Input::AddHand { town: 0, trade: "merchant".into(), level: 5, tag: "6f1c2a3e-0000-4000-8000-00000000002a".into() },
        Input::OwnShop { town: 0 },
    ]);
    s.apply("the desk's full chests and the depot's empty one counted from nothing: nothing to move", vec![
        Input::Stocktake { point: desk.clone(), counts: counts(&[(WHEAT, 300.0), (BREAD, 10.0)]), free: 0 },
        Input::Stocktake { point: depot.clone(), counts: counts(&[]), free: 1 },
    ]);
    let moved = s.apply("a day: its master Merchant puts a stack of its biggest surplus on its shop's shelf, out of the desk's chests", vec![
        Input::Stocktake { point: desk.clone(), counts: counts(&[(WHEAT, 300.0), (BREAD, 10.0)]), free: 0 },
        Input::Stocktake { point: depot.clone(), counts: counts(&[]), free: 1 },
        Input::Advance { to: DAY },
    ]);
    assert_eq!(moved, vec![Output::Moved { point: desk.clone(), good: WHEAT, count: -64 }]);
    let moved = s.apply("128 books delivered to the town, the chests counted: a stack into each free slot, the desk's first", vec![
        Input::Deliver { town: 0, good: BOOK, count: 128.0 },
        Input::Stocktake { point: desk.clone(), counts: counts(&[(WHEAT, 236.0), (BREAD, 10.0)]), free: 1 },
        Input::Stocktake { point: depot.clone(), counts: counts(&[]), free: 1 },
    ]);
    assert_eq!(moved, vec![
        Output::Moved { point: desk.clone(), good: BOOK, count: 64 },
        Output::Moved { point: depot.clone(), good: BOOK, count: 64 },
    ]);
    let moved = s.apply("a hopper filled the desk's slot first: the host couldn't put its stack in, and says so before it counts \
        again; no room is counted, so nothing moves", vec![
        Input::Unmoved { point: desk.clone(), good: BOOK, count: 64 },
        Input::Stocktake { point: desk.clone(), counts: counts(&[(WHEAT, 236.0), (BREAD, 10.0)]), free: 0 },
        Input::Stocktake { point: depot.clone(), counts: counts(&[(BOOK, 64.0)]), free: 0 },
    ]);
    assert_eq!(moved, vec![]);
    held(&mut s, "the town holds the 128 books, its chests (as the core takes them) 64: the rest wait on its books");
    let moved = s.apply("a player takes the bread (the town's no more) and the hopper's cobblestone out: the books that waited go in", vec![
        Input::Stocktake { point: desk.clone(), counts: counts(&[(WHEAT, 236.0)]), free: 2 },
        Input::Stocktake { point: depot.clone(), counts: counts(&[(BOOK, 64.0)]), free: 0 },
    ]);
    assert_eq!(moved, vec![Output::Moved { point: desk.clone(), good: BOOK, count: 64 }]);
    s.apply("the desk broken: its chests' goods go with them", vec![Input::RemovePoint { point: desk.clone() }]);
    s.apply("and put back where it stood: the same town, its chests counted in once", vec![
        Input::AddTown { name: "Newton".into(), desk: desk.clone() },
        Input::Stocktake { point: desk.clone(), counts: counts(&[(WHEAT, 236.0), (BOOK, 64.0)]), free: 1 },
    ]);
    let before = held(&mut s, "its stock, points and chests");
    s.save("saved");
    s.load("loaded");
    let after = held(&mut s, "asked again after the load");
    assert_eq!(before, after, "a society loaded answers as the one saved did");
    s.scenario
}

/// Every kind of happening and question, accepted.
/// A desk's money (`till.rs`): its chests and Blaze Banker counted, the surplus sold to the mint for coin, the treasury shown
/// in them, and the mint asked.
fn money() -> Scenario {
    let mut s = Script::new("money", "A town's money in its desk's chests and its Blaze Banker. The host counts them with \
        each look (Cashcount): what they hold beyond what the core took them to is money paid in, and a town with no trade \
        that costs emeralds sells all its emeralds to the mint for coin, into the Banker's account if one touches the desk \
        (Cash). The mint's rate falls with the selling and is asked (Mint). What the host couldn't write comes back as \
        CashUnmoved and is shown again. \
        A desk broken takes its money with it, and put back where it stood counts it in once. Saved and loaded, the till \
        is as the core took it.");
    let desk = at(PointKind::Desk, 27, 27);
    let till = |s: &mut Script, note: &str| s.ask(note, vec![Query::Cash { point: desk.clone() }, Query::Mint]);
    s.create("a society from a pack of four goods");
    s.apply("a town at its desk", vec![Input::AddTown { name: "Ashford".into(), desk: desk.clone() }]);
    s.apply("the desk's chests count 40 emeralds and 8 spurs of coin, its Banker's account 160, and the host only counts: \
        the money is paid in, the mint untouched", vec![
        Input::Stocktake { point: desk.clone(), counts: counts(&[]), free: 20 },
        Input::Cashcount { point: desk.clone(), emeralds: 40, coins: 8, bank: Some(160), coin_room: true, writes: false },
    ]);
    till(&mut s, "40 emeralds, 8 spurs worth half an emerald and 160 spurs worth ten: 50.5 in the treasury");
    let cash = s.apply("the host writes now: no trade of the town's costs emeralds, so all 40 are sold to the mint for coin, \
        into the Banker's account", vec![
        Input::Stocktake { point: desk.clone(), counts: counts(&[]), free: 20 },
        Input::Cashcount { point: desk.clone(), emeralds: 40, coins: 8, bank: Some(160), coin_room: true, writes: true },
    ]);
    assert!(matches!(cash[..], [Output::Cash { emeralds: -40, coins: 0, bank: 632, .. }]), "{cash:?}");
    till(&mut s, "the Banker's account holds 792 spurs standing for 50 emeralds, and the mint's rate has fallen");
    let again = s.apply("the host could take only 30 of the 40 emeralds sold out of the chests (a hopper): it says so, and counts \
        again; the ten left are the coin's already, so they are taken out once more", vec![
        Input::CashUnmoved { point: desk.clone(), emeralds: -10, coins: 0, bank: 0 },
        Input::Stocktake { point: desk.clone(), counts: counts(&[]), free: 20 },
        Input::Cashcount { point: desk.clone(), emeralds: 10, coins: 8, bank: Some(792), coin_room: true, writes: true },
    ]);
    assert!(matches!(again[..], [Output::Cash { emeralds: -10, coins: 0, bank: 0, .. }]), "{again:?}");
    till(&mut s, "the treasury is the same: nothing was paid in, and nothing is in the chests twice");
    s.apply("the desk broken: its money goes with it", vec![Input::RemovePoint { point: desk.clone() }]);
    s.apply("and put back where it stood: the same town, its money counted in once", vec![
        Input::AddTown { name: "Newton".into(), desk: desk.clone() },
        Input::Stocktake { point: desk.clone(), counts: counts(&[]), free: 20 },
        Input::Cashcount { point: desk.clone(), emeralds: 5, coins: 0, bank: None, coin_room: true, writes: false },
    ]);
    let before = till(&mut s, "five emeralds in the treasury");
    s.save("saved");
    s.load("loaded");
    let after = till(&mut s, "asked again after the load");
    assert_eq!(before, after, "a society loaded answers as the one saved did");
    s.scenario
}

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
        Input::Stocktake { point: at(PointKind::Desk, 27, 27), counts: counts(&[(BREAD, 60.0), (BOOK, 8.0)]), free: 27 },
        Input::Stocktake { point: at(PointKind::Locker, 30, 27), counts: counts(&[(WHEAT, 12.0)]), free: 27 },
        Input::Stocktake { point: at(PointKind::Shop, 24, 27), counts: counts(&[]), free: 27 },
        Input::Stocktake { point: at(PointKind::Banker, 27, 30), counts: counts(&[]), free: 27 },
        Input::Stocktake { point: at(PointKind::Depot, 603, -40), counts: counts(&[(BEEF, 20.0)]), free: 27 },
    ]);
    s.apply("what the host couldn't move at the desk, said before its next stocktake: there's room now, so it goes in again \
        (the locker's wheat the Merchant carried there is in the desk's chests now)", vec![
        Input::Unmoved { point: at(PointKind::Desk, 27, 27), good: BREAD, count: 4 },
        Input::Stocktake { point: at(PointKind::Desk, 27, 27), counts: counts(&[(WHEAT, 12.0), (BREAD, 120.0), (BOOK, 8.0)]), free: 24 },
    ]);
    s.apply("a player buys from a town's own shop", vec![Input::Buy { town: 1, good: BEEF, count: 2.0 }]);
    s.apply("a point broken", vec![Input::RemovePoint { point: at(PointKind::Shop, 24, 27) }]);
    s.apply("a player's pallet inputs name pallets that aren't there (a stale view): accepted, nothing happens", vec![
        Input::PalletBrought { town: 1, id: 41 },
        Input::PackageBrought { town: 1, pallet: 41, package: 0 },
        Input::PackageLost { pallet: 41, package: 0 },
    ]);
    s.apply("dials set for the world, one a switch, one put back to the world's start", vec![
        Input::SetDial { name: "harvest_window_days".into(), value: Some(6.0) },
        Input::SetDial { name: "bargain".into(), value: Some(0.0) },
        Input::SetDial { name: "harvest_window_days".into(), value: None },
    ]);
    s.apply("twenty days pass", vec![Input::Advance { to: 20 * DAY }]);
    s.ask("every question", vec![Query::Stock { town: 0 }, Query::Stock { town: 1 }, Query::Points, Query::Goods,
        Query::Chests { point: at(PointKind::Desk, 27, 27) }]);
    s.scenario
}

/// The pack of all the game brings: weights, kinds of comfort, trades, recipes, animals and dials.
fn full_pack() -> ContentPack {
    let mut p = pack();
    let mut iron = PackGood::new("minecraft:iron_ingot", 0.25, 64);
    iron.weight = 4.0;
    let mut cane = PackGood::new("minecraft:sugar_cane", 0.04, 64);
    cane.weight = 0.5;
    let paper = PackGood::new("minecraft:paper", 0.04, 64);
    p.goods.extend([iron, cane, paper]);
    p.comforts = vec!["clothing".into(), "reading".into()];
    p.trades = ["merchant", "farmer", "librarian", "shepherd", "none", "nitwit"].map(String::from).to_vec();
    p.recipes = vec![
        PackRecipe { trades: vec!["farmer".into()], inputs: vec![PackAmount { good: "minecraft:wheat".into(), count: 3.0 }],
            output: "minecraft:bread".into(), makes: 1.0 },
        PackRecipe { trades: vec!["librarian".into()], inputs: vec![PackAmount { good: "minecraft:sugar_cane".into(), count: 1.0 }],
            output: "minecraft:paper".into(), makes: 1.0 },
    ];
    p.animals = vec![PackAnimal { name: "cow".into(), feed: vec!["minecraft:wheat".into()], feed_per_day: 0.25,
        meat: Some("minecraft:cooked_beef".into()), meat_each: 2.0, wool: None, hide: None }];
    p.dials = vec![PackDial { name: "birth_mood".into(), value: 0.5 }, PackDial { name: "wagon_slots".into(), value: 30.0 }];
    p
}

/// A society from a pack with everything a game brings, its dials asked, set, kept by a save and put back.
fn content() -> Scenario {
    let mut s = Script::new("content", "A pack with all a game brings (kinds of comfort, trades, recipes, animals, dials, weights) makes a society \
        whose dials start where the pack set them. A dial is set for the world, kept by a save, and reset to the pack's own, not the core's. \
        A pack that names what it lacks is refused with everything wrong with it.");
    let r = s.say("a society from the pack", Command::Create { pack: full_pack(), seed: 7 });
    assert_eq!(r, Response::Done { outputs: vec![], answers: vec![] });
    let r = s.say("a pack whose recipe, animal, trade and dial name what isn't there: refused, the society kept", Command::Create { pack: {
        let mut bad = full_pack();
        bad.recipes[0].output = "minecraft:glass".into();
        bad.animals[0].feed = vec!["minecraft:hay".into()];
        bad.trades.retain(|t| t != "merchant");
        bad.dials.push(PackDial { name: "no_such_dial".into(), value: 1.0 });
        bad
    }, seed: 1 });
    assert!(matches!(s.refused("pack", r), Refusal::Pack { .. }));
    s.ask("the goods by number", vec![Query::Goods]);
    let dials = s.ask("every dial, the pack's two set", vec![Query::Dials]);
    let Answer::Dials { dials } = &dials[0] else { panic!("dials") };
    let named = |name: &str| dials.iter().find(|d| d.name == name).unwrap_or_else(|| panic!("no dial {name}")).clone();
    assert_eq!((named("birth_mood").value, named("birth_mood").initial), (0.5, 0.5));
    assert_eq!((named("wagon_slots").value, named("wagon_slots").initial), (30.0, 30.0));
    s.apply("a town, then three dials set: a fraction, a count rounded, a switch", vec![
        Input::AddTown { name: "Ashford".into(), desk: at(PointKind::Desk, 27, 27) },
        Input::SetDial { name: "birth_mood".into(), value: Some(0.9) },
        Input::SetDial { name: "wagon_slots".into(), value: Some(12.4) },
        Input::SetDial { name: "bargain".into(), value: Some(0.0) },
    ]);
    s.save("saved with its dials");
    s.load("loaded");
    let kept = s.ask("the dials after the load", vec![Query::Dials]);
    let Answer::Dials { dials } = &kept[0] else { panic!("dials") };
    let named = |name: &str| dials.iter().find(|d| d.name == name).unwrap_or_else(|| panic!("no dial {name}")).clone();
    assert_eq!((named("birth_mood").value, named("birth_mood").initial), (0.9, 0.5), "kept, and the pack's start with it");
    assert_eq!(named("wagon_slots").value, 12.0);
    s.apply("two dials reset: the pack's setting comes back, not the core's", vec![
        Input::SetDial { name: "birth_mood".into(), value: None },
        Input::SetDial { name: "wagon_slots".into(), value: None },
    ]);
    let back = s.ask("the dials again", vec![Query::Dials]);
    let Answer::Dials { dials } = &back[0] else { panic!("dials") };
    let named = |name: &str| dials.iter().find(|d| d.name == name).unwrap_or_else(|| panic!("no dial {name}")).clone();
    assert_eq!((named("birth_mood").value, named("wagon_slots").value, named("bargain").value), (0.5, 30.0, 0.0));
    s.scenario
}

fn hands_of(town: TownNo) -> Vec<Query> {
    vec![Query::Hands { town }]
}

/// The ids of a town's hands, in its roster's order, from an answer to [`hands_of`].
fn ids(answers: &[Answer]) -> Vec<u32> {
    match &answers[0] {
        Answer::Hands { hands, .. } => hands.iter().map(|h| h.hand).collect(),
        a => panic!("{a:?}"),
    }
}

/// Villagers joining towns as hands with ids; one lost from the middle of a roster, the rest keeping theirs through a
/// save and a load; the refusals of a hand's happenings.
fn hands() -> Scenario {
    let mut s = Script::new("hands", "A town's villagers join it as hands, each given an id that a host ties to its own \
        name for it (a villager's UUID, here a tag) and finds again by asking. A hand lost from the middle of a roster \
        leaves the others' ids as they were, and the ids and tags come back the same from a save; places in the roster \
        move, ids don't. The desk's Merchant manages two lockers at Novice.");
    s.create("a society from a pack of four goods");
    s.apply("two towns at their desks", vec![
        Input::AddTown { name: "Ashford".into(), desk: at(PointKind::Desk, 27, 27) },
        Input::AddTown { name: "Brent".into(), desk: at(PointKind::Desk, 600, -40) },
    ]);
    s.ask("a town with no roster has no hands, and its desk no Merchant", hands_of(0));
    s.apply("a Merchant takes the desk; a farmer, a librarian and a villager with no trade come to live at Ashford, and a \
        mason at Brent (the trades by the host's names, with or without its namespace)", vec![
        Input::AddHand { town: 0, trade: "enterprise:merchant".into(), level: 1, tag: "villager-m".into() },
        Input::AddHand { town: 0, trade: "minecraft:farmer".into(), level: 2, tag: "villager-f".into() },
        Input::AddHand { town: 0, trade: "librarian".into(), level: 1, tag: "villager-l".into() },
        Input::AddHand { town: 1, trade: "mason".into(), level: 3, tag: "villager-s".into() },
        Input::AddHand { town: 0, trade: "none".into(), level: 1, tag: String::new() },
    ]);
    let before = ids(&s.ask("Ashford's hands, each with its id and tag", hands_of(0)));
    assert_eq!(before, vec![1, 2, 3, 5], "ids in the order they joined, across towns");
    s.ask("and Brent's", hands_of(1));
    s.apply("the farmer dies (by its id: its place is second)", vec![Input::HandLost { hand: before[1] }]);
    let after = ids(&s.ask("the others keep their ids; the librarian is second now", hands_of(0)));
    assert_eq!(after, vec![1, 3, 5]);
    s.save("saved");
    s.load("loaded");
    let loaded = ids(&s.ask("asked after the load: the same ids and tags", hands_of(0)));
    assert_eq!(loaded, after, "ids survive a save");
    s.apply("a weaponsmith comes: a new id, never one given before", vec![
        Input::AddHand { town: 0, trade: "weaponsmith".into(), level: 1, tag: "villager-f2".into() },
        Input::Advance { to: 2 * DAY },
    ]);
    s.ask("its hands two days on", hands_of(0));

    let refuse = |s: &mut Script, note: &str, inputs: Vec<Input>, at: u32| {
        let r = s.say(note, Command::Apply { inputs });
        match s.refused(note, r) {
            Refusal::Input { at: got, .. } => assert_eq!(got, at, "{note}"),
            r => panic!("{note}: {r:?}"),
        }
    };
    let kept = s.ask("the hands as they stand", hands_of(0));
    refuse(&mut s, "a hand for a town that isn't there", vec![
        Input::AddHand { town: 9, trade: "farmer".into(), level: 1, tag: String::new() },
    ], 0);
    refuse(&mut s, "a hand of a trade the core hasn't", vec![
        Input::AddHand { town: 0, trade: "minecraft:baker".into(), level: 1, tag: String::new() },
    ], 0);
    refuse(&mut s, "a hand of level 0, after one of level 5", vec![
        Input::AddHand { town: 0, trade: "cleric".into(), level: 5, tag: String::new() },
        Input::AddHand { town: 0, trade: "cleric".into(), level: 0, tag: String::new() },
    ], 1);
    refuse(&mut s, "a hand called what another is called", vec![
        Input::AddHand { town: 1, trade: "farmer".into(), level: 1, tag: "villager-l".into() },
    ], 0);
    refuse(&mut s, "two hands called alike in one look", vec![
        Input::AddHand { town: 1, trade: "farmer".into(), level: 1, tag: "villager-x".into() },
        Input::AddHand { town: 0, trade: "farmer".into(), level: 1, tag: "villager-x".into() },
    ], 1);
    refuse(&mut s, "a hand lost that was lost already", vec![Input::HandLost { hand: before[1] }], 0);
    refuse(&mut s, "a hand lost twice in one look", vec![
        Input::HandLost { hand: after[2] },
        Input::HandLost { hand: after[2] },
    ], 1);
    let r = s.say("the hands of a town that isn't there", Command::Ask { queries: vec![Query::Hands { town: 0 }, Query::Hands { town: 7 }] });
    assert!(matches!(s.refused("query", r), Refusal::Query { at: 1, .. }));
    assert_eq!(s.ask("nothing of any refused request applied", hands_of(0)), kept);
    s.apply("a tag freed by its hand's death is free again: a villager of the same name comes", vec![
        Input::AddHand { town: 1, trade: "farmer".into(), level: 1, tag: "villager-f".into() },
    ]);
    let brent = ids(&s.ask("Brent's hands", hands_of(1)));
    refuse(&mut s, "a villager called as a living hand is, before that hand is lost in the same look", vec![
        Input::AddHand { town: 1, trade: "farmer".into(), level: 2, tag: "villager-f".into() },
        Input::HandLost { hand: brent[1] },
    ], 0);
    s.apply("the hand lost first, then a villager of its name comes, in one look", vec![
        Input::HandLost { hand: brent[1] },
        Input::AddHand { town: 1, trade: "farmer".into(), level: 2, tag: "villager-f".into() },
    ]);
    s.ask("Brent's hands: the new one with a new id", hands_of(1));
    s.scenario
}

fn resident(tag: &str, trade: &str, level: u8, baby: bool) -> Resident {
    Resident { tag: tag.into(), trade: trade.into(), level, baby }
}

/// A town's census as its roster's source: the villagers seen join as hands, a child is born in the core and named by
/// the host, a villager unseen for ten days has left; the refusals of a census's happenings.
fn people() -> Scenario {
    let mut s = Script::new("people", "A town's census is its roster's source: the villagers the host saw living there \
        join as hands under their tags (one of a trade the core hasn't isn't counted), its beds are the world's, and a \
        child born in the core comes out as Born, a hand with no villager that the host spawns and names (TagHand). A \
        villager no census has seen for ten days has left.");
    s.create("a society from a pack of four goods");
    s.apply("a town at its desk, with bread, where children come easily (the dials)", vec![
        Input::AddTown { name: "Ashford".into(), desk: at(PointKind::Desk, 27, 27) },
        Input::Deliver { town: 0, good: BREAD, count: 64.0 },
        Input::SetDial { name: "birth_mood".into(), value: Some(-100.0) },
        Input::SetDial { name: "birth_cost".into(), value: Some(0.0) },
    ]);
    s.apply("its first census: four beds, its Merchant, a farmer, a child, and a baker the core has no trade for", vec![
        Input::Census { town: 0, beds: 4, residents: vec![
            resident("villager-m", "enterprise:merchant", 1, false),
            resident("villager-f", "minecraft:farmer", 2, false),
            resident("villager-c", "minecraft:none", 1, true),
            resident("villager-b", "minecraft:baker", 1, false),
        ] },
    ]);
    s.ask("its hands, by the host's tags", hands_of(0));
    let out = s.apply("five days on: a child born, with no villager yet", vec![Input::Advance { to: 5 * DAY }]);
    let born: Vec<u32> = out.iter().filter_map(|o| match o {
        Output::Born { hand, .. } => Some(*hand),
        _ => None,
    }).collect();
    assert_eq!(born.len(), 1, "{out:?}");
    s.ask("the child is a hand with no tag", hands_of(0));
    s.apply("the host spawns a villager for it and names it; its next census sees it as that hand", vec![
        Input::TagHand { hand: born[0], tag: "villager-n".into() },
        Input::Census { town: 0, beds: 4, residents: vec![
            resident("villager-m", "enterprise:merchant", 1, false),
            resident("villager-f", "minecraft:farmer", 2, false),
            resident("villager-c", "minecraft:none", 1, true),
            resident("villager-n", "minecraft:none", 1, true),
        ] },
    ]);
    s.ask("four hands, the child named", hands_of(0));
    s.apply("eleven days on, a census that doesn't see the farmer: it has left", vec![
        Input::Advance { to: 16 * DAY + 1 },
        Input::Census { town: 0, beds: 4, residents: vec![
            resident("villager-m", "enterprise:merchant", 1, false),
            resident("villager-c", "minecraft:none", 1, false),
            resident("villager-n", "minecraft:none", 1, true),
        ] },
    ]);
    s.ask("three hands", hands_of(0));
    let r = s.say("a census of a town that isn't there", Command::Apply { inputs: vec![
        Input::Census { town: 4, beds: 1, residents: vec![] },
    ] });
    assert!(matches!(s.refused("census", r), Refusal::Input { at: 0, .. }));
    let r = s.say("a hand named with no tag", Command::Apply { inputs: vec![Input::TagHand { hand: born[0], tag: String::new() }] });
    assert!(matches!(s.refused("tag", r), Refusal::Input { at: 0, .. }));
    s.apply("a hand that isn't there named, and a name another hand has: ignored, not refused", vec![
        Input::TagHand { hand: 99, tag: "villager-z".into() },
        Input::TagHand { hand: born[0], tag: "villager-m".into() },
    ]);
    s.ask("the hands as they were", hands_of(0));
    s.scenario
}

fn land_pack() -> ContentPack {
    let mut p = pack();
    let mut carrot = PackGood::new("minecraft:carrot", 1.0 / 22.0, 64);
    carrot.food = 3.0;
    carrot.flags = vec![GoodFlag::Crop];
    p.goods.push(carrot);
    p
}

const CARROT: GoodNo = 4;

fn hex_at(q: i32, r: i32) -> HexAt {
    HexAt { q, r }
}

/// The host's land into the core: a surveyed hex's ground; a town's fields as the crops counted in its hexes; a field
/// its rules plant asked of the host (Build) at each look that surveys it until the host says it's laid (Built).
fn land() -> Scenario {
    let mut s = Script::new("land", "The host's land into the core. A hex's ground as the host surveyed it is the map's         (asked back as Ground). A town's fields are the crops the host counted in the hexes it holds, chunk by chunk, set         on its farm at each Survey (which sets the farm up). Its level-3 Farmer has hands to spare, so the town plants a         field of carrots: at the end of every look that surveys the town it's asked of the host (Build) until the host         lays it, counts its chunk and says so (Built); its plots are counted once.");
    s.say("a society from a pack with carrots", Command::Create { pack: land_pack(), seed: 7 });
    s.apply("a town at its desk (in hex 0, 0), money paid in, a level-3 Farmer, its ground surveyed: a lake to the east", vec![
        Input::AddTown { name: "Ashford".into(), desk: at(PointKind::Desk, 8, 8) },
        Input::Pay { town: 0, money: Fixed::emeralds(200) },
        Input::AddHand { town: 0, trade: "minecraft:farmer".into(), level: 3, tag: "villager-f".into() },
        Input::Ground { hex: hex_at(0, 0), water: 0.0, relief: 2.0, land: LandKind::Plains },
        Input::Ground { hex: hex_at(1, 0), water: 0.875, relief: 1.0, land: LandKind::Water },
    ]);
    s.ask("the ground of its hex, the lake's, and one never surveyed", vec![
        Query::Ground { hexes: vec![hex_at(0, 0), hex_at(1, 0), hex_at(0, 1)] },
    ]);
    s.apply("its first survey: 20 wheat plots counted in one chunk of its hex, 500 in a hex nobody holds", vec![
        Input::Crops { hex: hex_at(0, 0), chunk_x: 0, chunk_z: 0, counts: counts(&[(WHEAT, 20.0)]) },
        Input::Crops { hex: hex_at(2, 2), chunk_x: 12, chunk_z: 12, counts: counts(&[(WHEAT, 500.0)]) },
        Input::Survey { town: 0, bell_x: 8, bell_z: 4 },
    ]);
    s.ask("its land: the 20 wheat plots its fields", vec![Query::Land { town: 0 }]);
    let mut day = 0;
    let field = loop {
        day += 1;
        assert!(day <= 5, "no field planted in five days");
        let out = s.apply(&format!("day {day}: a look that surveys the town"), vec![
            Input::Advance { to: day * DAY },
            Input::Survey { town: 0, bell_x: 8, bell_z: 4 },
        ]);
        if let Some(Output::Build { work, .. }) = out.iter().find(|o| matches!(o, Output::Build { .. })) {
            break work.clone();
        }
    };
    assert_eq!(field, Work::Field { crop: CARROT });
    s.ask("the field of carrots is the farm's before the host lays it", vec![Query::Land { town: 0 }]);
    s.apply("a look whose host couldn't lay it: asked again", vec![Input::Survey { town: 0, bell_x: 8, bell_z: 4 }]);
    s.apply("the host lays it, counts its chunk and says so: not asked again", vec![
        Input::Crops { hex: hex_at(0, 0), chunk_x: 1, chunk_z: 0, counts: counts(&[(CARROT, 80.0)]) },
        Input::Built { town: 0, work: field, share: 1.0 },
        Input::Survey { town: 0, bell_x: 8, bell_z: 4 },
    ]);
    s.ask("its land: 20 wheat and 80 carrot plots, counted once", vec![Query::Land { town: 0 }]);
    let r = s.say("ground that isn't a number", Command::Apply { inputs: vec![
        Input::Ground { hex: hex_at(0, 0), water: -0.5, relief: 0.0, land: LandKind::Plains },
    ] });
    assert!(matches!(s.refused("ground", r), Refusal::Input { at: 0, .. }));
    let r = s.say("a hall the core has no name for", Command::Apply { inputs: vec![
        Input::Built { town: 0, work: Work::Hall { kind: "palace".into() }, share: 1.0 },
    ] });
    assert!(matches!(s.refused("hall", r), Refusal::Input { at: 0, .. }));
    let r = s.say("the land of a town that isn't there", Command::Ask { queries: vec![Query::Land { town: 3 }] });
    assert!(matches!(s.refused("land", r), Refusal::Query { at: 0, .. }));
    s.save("saved");
    s.load("loaded");
    s.ask("its land as saved", vec![Query::Land { town: 0 }]);
    s.scenario
}

/// The lockers of a town its host places (`locker_host.rs`): a hand's locker standing at a workstation's chests, the
/// Merchant's screen switching its offers, the chests emptied into the desk, and the refusals.
fn lockers() -> Scenario {
    let mut s = Script::new("lockers", "A town founded by its host has its lockers where the host places them: a \
        Librarian with no locker isn't one the Merchant manages, and once its locker stands at a workstation (a Locker \
        point, tied to it) it is. The locker's chests are counted with the town's: the paper a player left there is the \
        town's, carried to the desk's chests at once (Moved out of the locker, into the desk). The screen switches the \
        Librarian's paper trade on; two days on its locker has sold the town's paper, two trades a restock while the \
        paper lasts, and the Librarian and the Merchant earned their XP (HandLevel). Untied, the locker isn't managed. \
        A locker at a point that isn't one is refused; a switch for a hand or an offer that isn't there is ignored. \
        Saved and loaded, the lockers are as they were.");
    const PAPER: GoodNo = 6;
    let desk = at(PointKind::Desk, 27, 27);
    let locker = at(PointKind::Locker, 25, 27);
    let lockers_of = |s: &mut Script, note: &str| s.ask(note, vec![Query::Lockers { town: 0 }]);
    let r = s.say("a society from a pack with paper and Librarians", Command::Create { pack: full_pack(), seed: 7 });
    assert_eq!(r, Response::Done { outputs: vec![], answers: vec![] });
    s.apply("a town at its desk, its Merchant and a Librarian, the desk's empty chests counted", vec![
        Input::AddTown { name: "Ashford".into(), desk: desk.clone() },
        Input::AddHand { town: 0, trade: "merchant".into(), level: 1, tag: "villager-m".into() },
        Input::AddHand { town: 0, trade: "minecraft:librarian".into(), level: 1, tag: "villager-l".into() },
        Input::Stocktake { point: desk.clone(), counts: vec![], free: 27 },
    ]);
    let ids = ids(&s.ask("its hands", hands_of(0)));
    let librarian = ids[1];
    let a = lockers_of(&mut s, "its lockers: the host places them, and the Librarian has none yet, so isn't managed");
    let paper_offer = match &a[0] {
        Answer::Lockers { placed: true, lockers, .. } if lockers.len() == 1 && !lockers[0].managed && lockers[0].at.is_none() => {
            lockers[0].offers.iter().find(|o| o.good == PAPER && !o.sells).expect("a paper offer").offer
        }
        a => panic!("{a:?}"),
    };
    let moved = s.apply("its locker placed at the Librarian's workstation, and a look's count: 64 paper a player left in the \
        locker are the town's, carried to the desk's chests", vec![
        Input::AddPoint { point: locker.clone(), town: 0 },
        Input::Locker { hand: librarian, at: Some(locker.clone()) },
        Input::Stocktake { point: desk.clone(), counts: vec![], free: 27 },
        Input::Stocktake { point: locker.clone(), counts: counts(&[(PAPER, 64.0)]), free: 26 },
    ]);
    assert_eq!(moved, vec![
        Output::Moved { point: locker.clone(), good: PAPER, count: -64 },
        Output::Moved { point: desk.clone(), good: PAPER, count: 64 },
    ]);
    let a = lockers_of(&mut s, "the Librarian's locker stands there, and the Merchant manages it");
    assert!(matches!(&a[0], Answer::Lockers { lockers, .. } if lockers[0].managed && lockers[0].at == Some(locker.clone())), "{a:?}");
    s.apply("the screen switches its paper trade on, and a stale switch (a hand that isn't there, another good) is ignored", vec![
        Input::Switch { hand: librarian, offer: paper_offer, good: PAPER, on: true },
        Input::Switch { hand: 99, offer: 0, good: PAPER, on: true },
        Input::Switch { hand: librarian, offer: paper_offer, good: WHEAT, on: false },
    ]);
    let out = s.apply("two days: the locker trades the town's paper, its chests counted as the look ends", vec![
        Input::Stocktake { point: desk.clone(), counts: counts(&[(PAPER, 64.0)]), free: 26 },
        Input::Stocktake { point: locker.clone(), counts: vec![], free: 27 },
        Input::Advance { to: 2 * DAY },
    ]);
    assert!(out.iter().any(|o| matches!(o, Output::HandLevel { hand, .. } if *hand == librarian)), "{out:?}");
    assert!(out.iter().any(|o| matches!(o, Output::Moved { point, good: PAPER, count } if *point == desk && *count < 0)), "{out:?}");
    lockers_of(&mut s, "its offers, the paper trade used");
    s.save("saved");
    s.load("loaded");
    lockers_of(&mut s, "the same after the load");
    s.apply("the locker's chest broken: untied, then its point removed", vec![
        Input::Locker { hand: librarian, at: None },
        Input::RemovePoint { point: locker.clone() },
    ]);
    let a = lockers_of(&mut s, "no locker, not managed");
    assert!(matches!(&a[0], Answer::Lockers { lockers, .. } if !lockers[0].managed && lockers[0].at.is_none()), "{a:?}");
    let r = s.say("a locker at a desk: refused", Command::Apply { inputs: vec![
        Input::Locker { hand: librarian, at: Some(Point { kind: PointKind::Locker, ..desk.clone() }) },
    ] });
    assert!(matches!(s.refused("a desk", r), Refusal::Input { at: 0, .. }));
    let r = s.say("a locker where no point stands: refused", Command::Apply { inputs: vec![
        Input::Locker { hand: librarian, at: Some(locker.clone()) },
    ] });
    assert!(matches!(s.refused("nowhere", r), Refusal::Input { at: 0, .. }));
    let r = s.say("the lockers of a town that isn't there", Command::Ask { queries: vec![Query::Lockers { town: 3 }] });
    assert!(matches!(s.refused("no town", r), Refusal::Query { at: 0, .. }));
    s.scenario
}

/// A town's wagons through the wire (`wagon_host.rs`): a route started and stopped by its host, its crew away and back.
fn wagons() -> Scenario {
    let mut s = Script::new("wagons", "A town's wagons, started and stopped by its host (the Merchant's screen, the route \
        commands): Hub's host starts wagons to Far, forty blocks off, which holds books Hub's people are short of. At the \
        next look its wagon leaves (WagonLeft) and the hand that leads it, the villager with no trade Hub's Merchant takes \
        on as its travelling Merchant, goes away with it (HandAway: the host stows its villager). The route shows the \
        wagon on its way out with its crew by hand; on its way home; and home, its crew back (HandBack, with the trade it \
        was taken on for, for the host's villager let out by the desk). Stopped, the route is wound down and no wagon \
        leaves; started again, it's the same route, open. A route to itself, or from a town that isn't there, is \
        refused; one to a far end beyond a leg's food opens nothing. Saved and loaded, the routes are as they were.");
    const BREAD: GoodNo = 1;
    let hub = at(PointKind::Desk, 27, 27);
    let far = at(PointKind::Desk, 67, 27);
    let distant = at(PointKind::Desk, 27, 9_027);
    let routes_of = |s: &mut Script, note: &str| s.ask(note, vec![Query::Routes { town: 0 }]);
    let r = s.say("a society from a pack with books and Merchants", Command::Create { pack: full_pack(), seed: 7 });
    assert_eq!(r, Response::Done { outputs: vec![], answers: vec![] });
    s.apply("two towns forty blocks apart, Hub with its Merchant and a villager of no trade, Far with its Merchant and \
        books; bread and money in both", vec![
        Input::AddTown { name: "Hub".into(), desk: hub },
        Input::AddTown { name: "Far".into(), desk: far },
        Input::AddTown { name: "Distant".into(), desk: distant },
        Input::AddHand { town: 0, trade: "merchant".into(), level: 1, tag: "villager-hub".into() },
        Input::AddHand { town: 0, trade: "none".into(), level: 1, tag: "villager-lead".into() },
        Input::AddHand { town: 1, trade: "merchant".into(), level: 1, tag: "villager-far".into() },
        Input::Deliver { town: 0, good: BREAD, count: 128.0 },
        Input::Deliver { town: 1, good: BREAD, count: 128.0 },
        Input::Deliver { town: 1, good: BOOK, count: 64.0 },
        Input::Pay { town: 0, money: Fixed::from(50.0) },
        Input::Pay { town: 1, money: Fixed::from(50.0) },
    ]);
    let lead = ids(&s.ask("Hub's hands", hands_of(0)))[1];
    let a = routes_of(&mut s, "Hub has no route yet, and knows Far (Distant is beyond word of mouth); its squad at home");
    assert!(matches!(&a[0], Answer::Routes { routes, markets, .. } if routes.is_empty() && markets.len() == 1 && markets[0].routed == false), "{a:?}");
    s.apply("Hub's host starts wagons to Far, and to Distant (nine thousand blocks: beyond a leg's food)", vec![
        Input::Route { from: 0, to: 1, send: true },
        Input::Route { from: 0, to: 2, send: true },
    ]);
    let a = routes_of(&mut s, "a route to Far, home; none to Distant");
    assert!(matches!(&a[0], Answer::Routes { routes, markets, .. } if routes.len() == 1 && routes[0].to == 1
        && routes[0].open && routes[0].wagon.is_none() && markets[0].routed), "{a:?}");
    let out = s.apply("a look: the wagon leaves to shop for books, led by the hand taken on as its Merchant", vec![
        Input::Advance { to: 1_200 },
    ]);
    assert!(out.iter().any(|o| matches!(o, Output::WagonLeft { route: 0, .. })), "{out:?}");
    assert!(out.contains(&Output::HandAway { town: 0, hand: lead, tag: "villager-lead".into() }), "{out:?}");
    let a = routes_of(&mut s, "its wagon on the way out, its crew the lead");
    assert!(matches!(&a[0], Answer::Routes { routes, .. } if routes[0].wagon.as_ref().is_some_and(|w| w.out && w.crew == vec![lead])), "{a:?}");
    s.apply("a look: at the far end, and on its way home", vec![Input::Advance { to: 2_400 }]);
    routes_of(&mut s, "on its way home");
    let out = s.apply("a look: home, its crew back as Hub's Merchant on the road", vec![Input::Advance { to: 3_600 }]);
    assert!(out.iter().any(|o| matches!(o, Output::WagonHome { route: 0, .. })), "{out:?}");
    assert!(out.iter().any(|o| matches!(o, Output::HandBack { town: 0, hand, tag, trade, .. }
        if *hand == lead && tag == "villager-lead" && trade == "merchant")), "{out:?}");
    routes_of(&mut s, "home, one trip made");
    s.apply("the host stops Hub's wagons to Far", vec![Input::Route { from: 0, to: 1, send: false }]);
    let a = routes_of(&mut s, "the route wound down");
    assert!(matches!(&a[0], Answer::Routes { routes, markets, .. } if !routes[0].open && !markets[0].routed), "{a:?}");
    let out = s.apply("five days: no wagon leaves", vec![Input::Advance { to: 5 * DAY }]);
    assert!(!out.iter().any(|o| matches!(o, Output::WagonLeft { .. } | Output::HandAway { .. })), "{out:?}");
    s.apply("started again", vec![Input::Route { from: 0, to: 1, send: true }]);
    let a = routes_of(&mut s, "the same route, open");
    assert!(matches!(&a[0], Answer::Routes { routes, .. } if routes.len() == 1 && routes[0].open), "{a:?}");
    s.save("saved");
    s.load("loaded");
    routes_of(&mut s, "the same after the load");
    let r = s.say("a route from Hub to itself: refused", Command::Apply { inputs: vec![Input::Route { from: 0, to: 0, send: true }] });
    assert!(matches!(s.refused("to itself", r), Refusal::Input { at: 0, .. }));
    let r = s.say("a route to a town that isn't there: refused", Command::Apply { inputs: vec![
        Input::Route { from: 0, to: 1, send: false },
        Input::Route { from: 0, to: 9, send: true },
    ] });
    assert!(matches!(s.refused("no town", r), Refusal::Input { at: 1, .. }));
    let a = routes_of(&mut s, "nothing of the refused look applied: the route still open");
    assert!(matches!(&a[0], Answer::Routes { routes, .. } if routes[0].open), "{a:?}");
    let r = s.say("the routes of a town that isn't there", Command::Ask { queries: vec![Query::Routes { town: 9 }] });
    assert!(matches!(s.refused("no town", r), Refusal::Query { at: 0, .. }));
    s.scenario
}

fn pallets() -> Scenario {
    let mut s = Script::new("pallets", "A town's pallets as its host shows them (pallet_host.rs). Pallets made with two \
        dials raised so one holds three crates. A player has Far's Merchant pack paper for Mid (a pallet of three crates) \
        and for Hub (one crate): pallets in Far's warehouse (PalletPacked), a bill each. Asked what to put in Far's desk's \
        chests with two slots free, the core offers only Hub's pallet: a chest takes a pallet whole or not at all; with \
        four, both, and the host says it put them there (Stowed). A player carries Mid's pallet to Hub, a town on the way \
        that isn't its consignee: it waits there, packed (PalletMoved), shown nowhere until Hub's chests take it. Two of \
        its three crates reach Mid: at Mid's next look the pallet is received short, its seal broken, Mid taking what came \
        and the third crate a loss in the books (PalletOpened; the registry's lost goods). Hub's wagon to Far fetches the \
        crate packed for Hub, sealed as it leaves (on the manifest, one line, no superbill), and Hub receives it home.");
    const PAPER: GoodNo = 6;
    let (hub, far, mid) = (at(PointKind::Desk, 27, 27), at(PointKind::Desk, 67, 27), at(PointKind::Desk, 47, 27));
    let r = s.say("a society from a pack with paper and Merchants", Command::Create { pack: full_pack(), seed: 7 });
    assert_eq!(r, Response::Done { outputs: vec![], answers: vec![] });
    s.apply("three towns, Hub, Far and Mid; Hub with its Merchant and a villager of no trade to lead its wagon, Far with its \
        Merchant, books and paper; bread and money in each; pallets of up to 27 stacks", vec![
        Input::AddTown { name: "Hub".into(), desk: hub.clone() },
        Input::AddTown { name: "Far".into(), desk: far.clone() },
        Input::AddTown { name: "Mid".into(), desk: mid.clone() },
        Input::AddHand { town: 0, trade: "merchant".into(), level: 1, tag: "villager-hub".into() },
        Input::AddHand { town: 0, trade: "none".into(), level: 1, tag: "villager-lead".into() },
        Input::AddHand { town: 1, trade: "merchant".into(), level: 1, tag: "villager-far".into() },
        Input::Deliver { town: 0, good: BREAD, count: 128.0 },
        Input::Deliver { town: 1, good: BREAD, count: 128.0 },
        Input::Deliver { town: 1, good: BOOK, count: 64.0 },
        Input::Deliver { town: 1, good: PAPER, count: 1300.0 },
        Input::Pay { town: 0, money: Fixed::from(50.0) },
        Input::Pay { town: 1, money: Fixed::from(50.0) },
        Input::Pay { town: 2, money: Fixed::from(10.0) },
        Input::SetDial { name: "pallet_slots".into(), value: Some(27.0) },
        Input::SetDial { name: "pallet_weight".into(), value: Some(27.0) },
    ]);
    let out = s.apply("Far packs 1,200 paper for Mid and 100 for Hub", vec![
        Input::Pack { town: 1, consignee: 2, goods: counts(&[(PAPER, 1200.0)]) },
        Input::Pack { town: 1, consignee: 0, goods: counts(&[(PAPER, 100.0)]) },
    ]);
    let ids: Vec<u64> = out.iter().filter_map(|o| match o { Output::PalletPacked { id, .. } => Some(*id), _ => None }).collect();
    assert_eq!(ids.len(), 2, "{out:?}");
    let (for_mid, for_hub) = (ids[0], ids[1]);
    s.ask("Far's pallets", vec![Query::Pallets { town: 1 }]);
    let a = s.ask("what Far's chests take with two slots free: only Hub's, whole", vec![Query::Stow { point: far.clone(), free: 2 }]);
    assert!(matches!(&a[0], Answer::Stow { pallets, .. } if pallets.len() == 1 && pallets[0].id == for_hub), "{a:?}");
    s.ask("with four: both", vec![Query::Stow { point: far.clone(), free: 4 }]);
    s.apply("the host put both in Far's chests", vec![
        Input::Stowed { point: far.clone(), pallet: for_mid },
        Input::Stowed { point: far.clone(), pallet: for_hub },
    ]);
    let a = s.ask("nothing more to put there", vec![Query::Stow { point: far.clone(), free: 27 }]);
    assert!(matches!(&a[0], Answer::Stow { pallets, .. } if pallets.is_empty()), "{a:?}");
    let out = s.apply("a player carries Mid's pallet to Hub's desk: it waits there, packed", vec![
        Input::PalletBrought { town: 0, id: for_mid },
    ]);
    assert_eq!(out, vec![Output::PalletMoved { id: for_mid, to: Place::Town { town: 0 } }]);
    s.ask("Hub holds it, shown nowhere yet; Mid's inbound", vec![Query::Pallets { town: 0 }, Query::Pallets { town: 2 },
        Query::Stow { point: hub.clone(), free: 27 }]);
    s.apply("and on to Mid with two of its three crates", vec![
        Input::PackageBrought { town: 2, pallet: for_mid, package: 0 },
        Input::PackageBrought { town: 2, pallet: for_mid, package: 2 },
    ]);
    let out = s.apply("Mid's next look: received short", vec![Input::Advance { to: 1_200 }]);
    assert!(out.contains(&Output::PalletOpened { id: for_mid, at: Place::Town { town: 0 } }), "{out:?}");
    let a = s.ask("Mid's paper, and the crate that didn't come lost in the books", vec![Query::Stock { town: 2 },
        Query::Pallets { town: 2 }, Query::Pallet { id: for_mid }]);
    assert!(matches!(&a[0], Answer::Stock { stock, .. } if stock[PAPER as usize] == 576.0 + 48.0), "{a:?}");
    assert!(matches!(&a[1], Answer::Pallets { lost_goods, .. } if lost_goods == &counts(&[(PAPER, 576.0)])), "{a:?}");
    s.apply("Hub's host starts wagons to Far", vec![Input::Route { from: 0, to: 1, send: true }]);
    let mut fetched = false;
    for look in 2..=4u64 {
        let out = s.apply("a look", vec![Input::Advance { to: look * 1_200 }]);
        fetched |= out.contains(&Output::PalletMoved { id: for_hub, to: Place::Wagon { route: 0 } });
        let a = s.ask("Hub's wagon's manifest", vec![Query::Manifest { route: 0 }]);
        if fetched && look == 3 {
            assert!(matches!(&a[0], Answer::Manifest { lines, superbill: false, .. } if lines.len() == 1 && lines[0].pallet.sealed), "{a:?}");
        }
    }
    assert!(fetched, "Hub's wagon fetched its crate");
    let a = s.ask("home: Hub's paper, every pallet gone", vec![Query::Stock { town: 0 }, Query::Pallets { town: 0 }]);
    assert!(matches!(&a[0], Answer::Stock { stock, .. } if stock[PAPER as usize] == 100.0), "{a:?}");
    s.save("saved");
    s.load("loaded");
    s.ask("the same after the load", vec![Query::Pallets { town: 1 }]);
    let r = s.say("a pallet stowed at a point that isn't a desk: refused", Command::Apply { inputs: vec![
        Input::Stowed { point: at(PointKind::Desk, 5, 5), pallet: 0 },
    ] });
    assert!(matches!(s.refused("no desk", r), Refusal::Input { at: 0, .. }));
    let r = s.say("goods packed for the town itself: refused", Command::Apply { inputs: vec![
        Input::Pack { town: 1, consignee: 1, goods: vec![] },
    ] });
    assert!(matches!(s.refused("itself", r), Refusal::Input { at: 0, .. }));
    let r = s.say("the manifest of a route that isn't there", Command::Ask { queries: vec![Query::Manifest { route: 9 }] });
    assert!(matches!(s.refused("no route", r), Refusal::Query { at: 0, .. }));
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
    let mut bad = pack();
    bad.recipes.push(PackRecipe { trades: vec!["baker".into()], inputs: vec![], output: "minecraft:glass".into(), makes: 0.0 });
    let r = s.say("a pack with a recipe for trades and goods it hasn't", Command::Create { pack: bad, seed: 1 });
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
    // A newer core's world (the host leaves its file as it is and runs without a society), and a format older than
    // this core's migrations reach (it has none yet: format 0 never was).
    let format = enterprise_core::save::FORMAT;
    let r = s.say("a save of the next format, from a newer core", Command::Load {
        save: SaveFile { format: format + 1, schema: SCHEMA ^ 1, core: "a newer core".into(), blob: vec![1, 2, 3] },
    });
    assert_eq!(s.refused("save format", r), Refusal::SaveFormat { core: format, save: format + 1 });
    let r = s.say("a save of a format older than the core brings forward", Command::Load {
        save: SaveFile { format: 0, schema: SCHEMA, core: "an older core".into(), blob: vec![1, 2, 3] },
    });
    assert_eq!(s.refused("save format", r), Refusal::SaveFormat { core: format, save: 0 });
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
    refuse_apply(&mut s, "a pallet brought to a town that isn't there", vec![Input::PalletBrought { town: 9, id: 0 }], 0);
    refuse_apply(&mut s, "a package brought to a town that isn't there", vec![
        Input::PackageBrought { town: 9, pallet: 0, package: 0 },
    ], 0);
    refuse_apply(&mut s, "a good that isn't there", vec![Input::Deliver { town: 0, good: 99, count: 1.0 }], 0);
    refuse_apply(&mut s, "a count below nothing", vec![Input::Buy { town: 0, good: WHEAT, count: -1.0 }], 0);
    refuse_apply(&mut s, "money below nothing", vec![Input::Pay { town: 0, money: Fixed::from_millis(-1) }], 0);
    refuse_apply(&mut s, "a stocktake of a point that isn't there", vec![
        Input::Stocktake { point: at(PointKind::Locker, 1, 1), counts: vec![], free: 27 },
    ], 0);
    refuse_apply(&mut s, "a stocktake naming another kind than the point standing there", vec![
        Input::Stocktake { point: at(PointKind::Shop, 27, 27), counts: vec![], free: 27 },
    ], 0);
    refuse_apply(&mut s, "a point where one stands already", vec![Input::AddPoint { point: at(PointKind::Locker, 27, 27), town: 0 }], 0);
    refuse_apply(&mut s, "a town whose desk isn't a desk", vec![
        Input::AddTown { name: "Nowhere".into(), desk: at(PointKind::Locker, 5, 5) },
    ], 0);
    refuse_apply(&mut s, "an advance past a hundred years", vec![Input::Advance { to: u64::MAX }], 0);
    refuse_apply(&mut s, "a dial the core hasn't, after one it has", vec![
        Input::SetDial { name: "shelf_days".into(), value: Some(7.0) },
        Input::SetDial { name: "no_such_dial".into(), value: Some(1.0) },
    ], 1);
    refuse_apply(&mut s, "a count dial set below nothing", vec![Input::SetDial { name: "wagon_slots".into(), value: Some(-1.0) }], 0);
    refuse_apply(&mut s, "a dial that isn't there put back", vec![Input::SetDial { name: "no_such_dial".into(), value: None }], 0);
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
    vec![stocktake(), inputs(), content(), refusals(), hands(), people(), chests(), money(), land(), lockers(), wagons(), pallets()]
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
    for name in ["stocktake", "inputs", "content", "refusals", "hands", "people", "chests", "money", "land", "lockers", "wagons", "pallets"] {
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
    let mut outputs = std::collections::BTreeSet::new();
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
        }
        for step in &scenario.steps {
            if step.load.is_some() {
                commands.insert("Load".to_string());
            }
            match &step.response {
                Some(Response::Refused { why }) => {
                    refusals.insert(variant(serde_json::to_value(why).unwrap()));
                }
                Some(Response::Done { outputs: os, .. }) => outputs.extend(os.iter().map(|o| variant(serde_json::to_value(o).unwrap()))),
                _ => {}
            }
        }
    }
    let set = |names: &[&str]| names.iter().map(|s| s.to_string()).collect::<std::collections::BTreeSet<_>>();
    assert_eq!(inputs, set(&["Advance", "AddPoint", "AddTown", "Buy", "Deliver", "Order", "OwnShop", "PackageBrought", "PackageLost", "PalletBrought", "Pay", "RemovePoint", "SetDial", "Stall", "Stocktake", "AddHand", "HandLost", "Census", "TagHand", "Unmoved", "Cashcount", "CashUnmoved", "Ground", "Crops", "Survey", "Built", "Switch", "Locker", "Route", "Pack", "Stowed"]));
    assert_eq!(commands, set(&["Apply", "Ask", "Create", "Load", "Save"]));
    assert_eq!(queries, set(&["Dials", "Goods", "Points", "Stock", "Hands", "Chests", "Cash", "Mint", "Land", "Ground", "Lockers", "Routes", "Pallets", "Pallet", "Stow", "Manifest"]));
    assert_eq!(refusals, set(&["Abi", "Input", "NoSociety", "Pack", "Query", "Save", "SaveFormat", "Schema", "Unreadable"]));
    // A child born, the chests' moves, the till's, a field to lay, a locker's trades levelling its villager and the
    // Merchant, a wagon's trip with its crew away and back, and pallets packed, moved and opened (`Pack`): the others
    // need a master Merchant (OrderFilled, Signal) or a party of settlers or the rules taking a hand (Settled, Departed,
    // HandGone), and no wire input makes any. When one does, a scenario shows each and its name goes here.
    assert_eq!(outputs, set(&["Born", "Cash", "Moved", "Build", "HandLevel", "WagonLeft", "WagonSold", "WagonHome",
        "HandAway", "HandBack", "PalletPacked", "PalletMoved", "PalletOpened"]));
}
