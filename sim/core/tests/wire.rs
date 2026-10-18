//! The wire (`wire/`): a host's one way in. A society created from a content pack, its desks' stocktakes and
//! deliveries booked, its stock and points asked; saved and loaded through `handle`, running on as if never saved;
//! every refusal a message with nothing applied; postcard bytes in and out as `handle` does; and no wire type holding
//! anything but numbers, strings, ids, vectors and other wire types.

mod common;

use common::{world, DAY};
use enterprise_core::wire::*;
use enterprise_core::{Content, Fixed, Good, Society};

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

fn desk(x: i32, z: i32) -> Point {
    Point { kind: PointKind::Desk, dim: "minecraft:overworld".into(), x, y: -60, z }
}

fn at(kind: PointKind, x: i32, z: i32) -> Point {
    Point { kind, ..desk(x, z) }
}

fn call(s: &mut Option<Society>, command: Command) -> Response {
    handle(s, Request::new(command))
}

fn created() -> Option<Society> {
    let mut s = None;
    assert_eq!(call(&mut s, Command::Create { pack: pack(), seed: 7 }), Response::Done { outputs: vec![], answers: vec![] });
    s
}

fn apply(s: &mut Option<Society>, inputs: Vec<Input>) -> Vec<Output> {
    match call(s, Command::Apply { inputs }) {
        Response::Done { outputs, answers } if answers.is_empty() => outputs,
        r => panic!("applied: {r:?}"),
    }
}

fn ask(s: &mut Option<Society>, queries: Vec<Query>) -> Vec<Answer> {
    match call(s, Command::Ask { queries }) {
        Response::Done { outputs, answers } if outputs.is_empty() => answers,
        r => panic!("asked: {r:?}"),
    }
}

fn stock(s: &mut Option<Society>, town: TownId) -> Vec<f64> {
    match &ask(s, vec![Query::Stock { town }])[..] {
        [Answer::Stock { town: t, stock }] if *t == town => stock.clone(),
        a => panic!("stock: {a:?}"),
    }
}

fn refusal(r: Response) -> Refusal {
    match r {
        Response::Refused { why } => why,
        r => panic!("not refused: {r:?}"),
    }
}

fn counts(of: &[(usize, f64)]) -> Vec<Count> {
    of.iter().map(|&(good, count)| Count { good, count }).collect()
}

#[test]
fn a_society_from_a_pack_takes_a_delivery_and_says_its_stock() {
    let mut s = created();
    let out = apply(&mut s, vec![
        Input::AddTown { name: "Ashford".into(), desk: desk(27, 27) },
        Input::Deliver { town: 0, good: WHEAT, count: 40.0 },
        Input::Deliver { town: 0, good: BOOK, count: 3.0 },
    ]);
    assert!(out.is_empty());
    assert_eq!(stock(&mut s, 0), vec![40.0, 0.0, 3.0, 0.0]);
    assert_eq!(ask(&mut s, vec![Query::Points]), vec![Answer::Points { points: vec![TownPoint { point: desk(27, 27), town: 0 }] }]);
    let society = s.as_ref().unwrap();
    assert_eq!(society.town(0).name, "Ashford");
    assert_eq!(society.town_hex(0), Some(enterprise_core::Hex::at(27.0, 27.0)), "the town stands at its desk");
    assert_eq!(society.town(0).money, Fixed::ZERO, "no money until it's paid some");
}

#[test]
fn a_pack_makes_the_content_the_core_would_for_its_goods() {
    let content = Content::from_pack(&pack()).unwrap();
    let goods = vec![
        Good::new("minecraft:wheat", 0.05, 64).crop(),
        Good::new("minecraft:bread", 0.2, 64).food(5.0).saturation(6.0),
        Good::new("minecraft:book", 0.5, 64).comfort(Content::COMFORTS.iter().position(|k| *k == "reading").unwrap()),
        Good::new("minecraft:cooked_beef", 0.3, 64).food(8.0).saturation(12.8).meat(),
    ];
    assert_eq!(format!("{content:?}"), format!("{:?}", Content::new(goods, DAY)));
}

#[test]
fn a_stocktake_books_what_the_chests_gained_and_lost_since_the_last() {
    let mut s = created();
    apply(&mut s, vec![
        Input::AddTown { name: "Ashford".into(), desk: desk(27, 27) },
        Input::AddPoint { point: at(PointKind::Locker, 30, 27), town: 0 },
    ]);
    let before = s.as_mut().unwrap().totals();
    // The first stocktake counts from nothing.
    apply(&mut s, vec![Input::Stocktake { point: desk(27, 27), counts: counts(&[(WHEAT, 30.0), (BREAD, 5.0)]) }]);
    assert_eq!(stock(&mut s, 0), vec![30.0, 5.0, 0.0, 0.0]);
    // A player took 20 wheat out and put 2 books in; bread is as it was.
    apply(&mut s, vec![Input::Stocktake { point: desk(27, 27), counts: counts(&[(WHEAT, 10.0), (BREAD, 5.0), (BOOK, 2.0)]) }]);
    assert_eq!(stock(&mut s, 0), vec![10.0, 5.0, 2.0, 0.0]);
    // A locker's chests are counted apart from the desk's, for the same town; a good listed twice is summed.
    apply(&mut s, vec![Input::Stocktake { point: at(PointKind::Locker, 30, 27), counts: counts(&[(BREAD, 1.0), (BREAD, 2.0)]) }]);
    assert_eq!(stock(&mut s, 0), vec![10.0, 8.0, 2.0, 0.0]);
    // The town has used 6 wheat the chests still show; then the chests are emptied: only what it holds is taken.
    assert_eq!(s.as_mut().unwrap().withdraw(0, WHEAT, 6.0), 6.0);
    apply(&mut s, vec![Input::Stocktake { point: desk(27, 27), counts: counts(&[(BREAD, 5.0), (BOOK, 2.0)]) }]);
    assert_eq!(stock(&mut s, 0), vec![0.0, 8.0, 2.0, 0.0]);
    let society = s.as_mut().unwrap();
    society.conserved(&before).unwrap();
    society.solvent().unwrap();
    // A point broken and placed again counts from nothing again.
    apply(&mut s, vec![
        Input::RemovePoint { point: at(PointKind::Locker, 30, 27) },
        Input::AddPoint { point: at(PointKind::Locker, 30, 27), town: 0 },
        Input::Stocktake { point: at(PointKind::Locker, 30, 27), counts: counts(&[(BREAD, 3.0)]) },
    ]);
    assert_eq!(stock(&mut s, 0), vec![0.0, 11.0, 2.0, 0.0]);
    s.as_mut().unwrap().conserved(&before).unwrap();
}

#[test]
fn points_are_answered_in_their_places_order_with_their_towns() {
    let mut s = created();
    apply(&mut s, vec![
        Input::AddTown { name: "B".into(), desk: desk(500, 0) },
        Input::AddTown { name: "A".into(), desk: desk(-500, 0) },
        Input::AddPoint { point: at(PointKind::Banker, 502, 0), town: 0 },
        Input::AddPoint { point: Point { dim: "minecraft:the_nether".into(), ..at(PointKind::Depot, 0, 0) }, town: 1 },
    ]);
    let Answer::Points { points } = &ask(&mut s, vec![Query::Points])[0] else { panic!() };
    let seen: Vec<(PointKind, &str, i32, TownId)> = points.iter().map(|p| (p.point.kind, p.point.dim.as_str(), p.point.x, p.town)).collect();
    assert_eq!(seen, vec![
        (PointKind::Desk, "minecraft:overworld", -500, 1),
        (PointKind::Desk, "minecraft:overworld", 500, 0),
        (PointKind::Banker, "minecraft:overworld", 502, 0),
        (PointKind::Depot, "minecraft:the_nether", 0, 1),
    ]);
}

/// A world saved and loaded through the wire runs on as the one kept, to the bit.
fn saved_and_loaded_runs_on_alike(mut kept: Option<Society>, days: std::ops::RangeInclusive<u64>) {
    let Response::Saved { save } = call(&mut kept, Command::Save) else { panic!("saved") };
    assert_eq!((save.format, save.schema, save.core.as_str()), (enterprise_core::save::FORMAT, SCHEMA, CORE));
    let mut loaded = None;
    assert_eq!(call(&mut loaded, Command::Load { save }), Response::Done { outputs: vec![], answers: vec![] });
    let n = kept.as_ref().unwrap().town_count();
    let queries: Vec<Query> = (0..n).map(|town| Query::Stock { town }).chain([Query::Points]).collect();
    assert_eq!(ask(&mut loaded, queries.clone()), ask(&mut kept, queries.clone()));
    for day in days {
        let advance = vec![Input::Advance { to: day * DAY }];
        assert_eq!(apply(&mut loaded, advance.clone()), apply(&mut kept, advance), "day {day}: the same outputs");
        assert_eq!(ask(&mut loaded, queries.clone()), ask(&mut kept, queries.clone()), "day {day}: the same answers");
    }
    let (Response::Saved { save: a }, Response::Saved { save: b }) = (call(&mut kept, Command::Save), call(&mut loaded, Command::Save))
        else { panic!("saved") };
    assert!(a == b, "the same save at the end");
}

#[test]
fn a_society_saved_and_loaded_through_handle_runs_on_as_never_saved() {
    let mut s = created();
    apply(&mut s, vec![
        Input::AddTown { name: "Ashford".into(), desk: desk(27, 27) },
        Input::AddTown { name: "Bly".into(), desk: desk(600, 40) },
        Input::Pay { town: 0, money: Fixed::emeralds(40) },
        Input::Stocktake { point: desk(27, 27), counts: counts(&[(WHEAT, 64.0), (BREAD, 12.0)]) },
        Input::Advance { to: 3 * DAY },
    ]);
    saved_and_loaded_runs_on_alike(s, 4..=20);
    // And a whole random world, every slice's state in it, with wagons on the road.
    let mut w = world(77);
    w.advance(30 * DAY);
    saved_and_loaded_runs_on_alike(Some(w), 31..=60);
}

#[test]
fn a_request_for_a_society_before_there_is_one_is_refused() {
    let mut s = None;
    for command in [Command::Save, Command::Apply { inputs: vec![Input::Advance { to: DAY }] }, Command::Ask { queries: vec![Query::Points] }] {
        assert_eq!(refusal(call(&mut s, command)), Refusal::NoSociety);
    }
    assert!(s.is_none());
}

#[test]
fn a_request_built_against_another_abi_or_schema_is_refused() {
    let mut s = created();
    let command = Command::Ask { queries: vec![Query::Points] };
    assert_eq!(refusal(handle(&mut s, Request { abi: ABI + 1, schema: SCHEMA, command: command.clone() })),
        Refusal::Abi { core: ABI, request: ABI + 1 });
    assert_eq!(refusal(handle(&mut s, Request { abi: ABI, schema: SCHEMA ^ 1, command: command.clone() })),
        Refusal::Schema { core: SCHEMA, request: SCHEMA ^ 1 });
    // Before a society, too: the envelope is checked first.
    assert_eq!(refusal(handle(&mut None, Request { abi: ABI, schema: SCHEMA ^ 1, command })),
        Refusal::Schema { core: SCHEMA, request: SCHEMA ^ 1 });
}

#[test]
fn a_bad_pack_is_refused_with_everything_wrong_with_it_and_the_society_kept() {
    let mut bad = pack();
    bad.day_ticks = 0;
    bad.goods[1].name = "minecraft:wheat".into();
    bad.goods[2].worth = f64::NAN;
    bad.goods[2].comfort = Some("gambling".into());
    bad.goods[3].stack = 0;
    bad.goods[3].food = -1.0;
    bad.goods.push(PackGood::new(" ", 1.0, 1));
    let mut s = created();
    apply(&mut s, vec![Input::AddTown { name: "Ashford".into(), desk: desk(27, 27) }]);
    let Refusal::Pack { why } = refusal(call(&mut s, Command::Create { pack: bad, seed: 1 })) else { panic!("a pack refusal") };
    for wrong in ["a day of no ticks", "good 1 (\"minecraft:wheat\"): named as another is", "good 2 (\"minecraft:book\"): worth NaN",
        "a comfort the core doesn't know (\"gambling\"", "good 3 (\"minecraft:cooked_beef\"): a stack of none", "food points -1",
        "good 4 (\" \"): no name"] {
        assert!(why.contains(wrong), "{wrong:?} in {why:?}");
    }
    assert_eq!(s.as_ref().unwrap().town_count(), 1, "the society there is kept");
    let empty = ContentPack { day_ticks: DAY, goods: vec![] };
    assert_eq!(refusal(call(&mut None, Command::Create { pack: empty, seed: 1 })), Refusal::Pack { why: "no goods".into() });
}

#[test]
fn a_save_of_another_format_or_none_at_all_is_refused_and_the_society_kept() {
    let mut s = created();
    apply(&mut s, vec![Input::AddTown { name: "Ashford".into(), desk: desk(27, 27) }]);
    let Response::Saved { save } = call(&mut s, Command::Save) else { panic!("saved") };
    let newer = SaveFile { format: enterprise_core::save::FORMAT + 1, ..save.clone() };
    assert_eq!(refusal(call(&mut s, Command::Load { save: newer })),
        Refusal::SaveFormat { core: enterprise_core::save::FORMAT, save: enterprise_core::save::FORMAT + 1 });
    let junk = SaveFile { blob: b"not a save".to_vec(), ..save.clone() };
    assert!(matches!(refusal(call(&mut s, Command::Load { save: junk })), Refusal::Save { .. }));
    let cut = SaveFile { blob: save.blob[..save.blob.len() / 2].to_vec(), ..save };
    assert!(matches!(refusal(call(&mut s, Command::Load { save: cut })), Refusal::Save { .. }));
    assert_eq!(s.as_ref().unwrap().town_count(), 1, "the society there is kept");
}

#[test]
fn a_look_naming_what_isnt_there_is_refused_whole() {
    let mut s = created();
    apply(&mut s, vec![Input::AddTown { name: "Ashford".into(), desk: desk(27, 27) }]);
    let refused = |s: &mut Option<Society>, inputs: Vec<Input>| refusal(call(s, Command::Apply { inputs }));
    let deliver = Input::Deliver { town: 0, good: WHEAT, count: 5.0 };
    let cases: Vec<(Input, &str)> = vec![
        (Input::Deliver { town: 1, good: WHEAT, count: 1.0 }, "no town 1"),
        (Input::Deliver { town: 0, good: 4, count: 1.0 }, "no good 4"),
        (Input::Deliver { town: 0, good: WHEAT, count: f64::NAN }, "a count of NaN"),
        (Input::Buy { town: 0, good: WHEAT, count: -1.0 }, "a count of -1"),
        (Input::Pay { town: 0, money: Fixed::from_millis(-1) }, "a payment of"),
        (Input::Order { town: 0, good: BREAD, lot: 1.0, price: f64::INFINITY, purse: 0.0 }, "a price of inf"),
        (Input::OwnShop { town: 9 }, "no town 9"),
        (Input::AddTown { name: "Bly".into(), desk: at(PointKind::Locker, 0, 0) }, "a town's desk that's a Locker"),
        (Input::AddTown { name: "Bly".into(), desk: desk(27, 27) }, "a point already stands"),
        (Input::AddPoint { point: Point { kind: PointKind::Shop, ..desk(27, 27) }, town: 0 }, "a point already stands"),
        (Input::AddPoint { point: at(PointKind::Shop, 1, 1), town: 3 }, "no town 3"),
        (Input::RemovePoint { point: desk(1, 1) }, "no point at"),
        (Input::Stocktake { point: desk(1, 1), counts: vec![] }, "no point at"),
        (Input::Stocktake { point: desk(27, 27), counts: counts(&[(7, 1.0)]) }, "no good 7"),
    ];
    for (input, why) in cases {
        // A good delivery first: refused with the rest, so nothing of the look is applied.
        match refused(&mut s, vec![deliver.clone(), input.clone()]) {
            Refusal::Input { at: 1, why: w } if w.contains(why) => {}
            r => panic!("{input:?}: {r:?}, not {why:?}"),
        }
    }
    assert_eq!(stock(&mut s, 0), vec![0.0; 4], "nothing applied");
    // Checked in order, as applied: a town added is there for what follows it, a point removed isn't.
    apply(&mut s, vec![
        Input::AddTown { name: "Bly".into(), desk: desk(600, 0) },
        Input::Deliver { town: 1, good: BREAD, count: 2.0 },
        Input::AddPoint { point: at(PointKind::Depot, 600, 5), town: 1 },
        Input::Stocktake { point: at(PointKind::Depot, 600, 5), counts: counts(&[(BREAD, 1.0)]) },
    ]);
    assert_eq!(stock(&mut s, 1), vec![0.0, 3.0, 0.0, 0.0]);
    assert!(matches!(refused(&mut s, vec![
        Input::RemovePoint { point: at(PointKind::Depot, 600, 5) },
        Input::Stocktake { point: at(PointKind::Depot, 600, 5), counts: vec![] },
    ]), Refusal::Input { at: 1, .. }));
    assert!(matches!(refused(&mut s, vec![
        Input::AddTown { name: "Cray".into(), desk: desk(-600, 0) },
        Input::AddTown { name: "Dun".into(), desk: desk(-600, 0) },
    ]), Refusal::Input { at: 1, .. }));
    assert_eq!(s.as_ref().unwrap().town_count(), 2);
}

#[test]
fn a_question_about_what_isnt_there_is_refused() {
    let mut s = created();
    apply(&mut s, vec![Input::AddTown { name: "Ashford".into(), desk: desk(27, 27) }]);
    assert_eq!(refusal(call(&mut s, Command::Ask { queries: vec![Query::Points, Query::Stock { town: 1 }] })),
        Refusal::Query { at: 1, why: "no town 1".into() });
}

#[test]
fn bytes_in_and_out_are_handle_in_postcard() {
    let (mut a, mut b) = (None, None);
    let requests = vec![
        Request::new(Command::Create { pack: pack(), seed: 3 }),
        Request::new(Command::Apply { inputs: vec![
            Input::AddTown { name: "Ashford".into(), desk: desk(27, 27) },
            Input::Stocktake { point: desk(27, 27), counts: counts(&[(WHEAT, 9.0)]) },
            Input::Advance { to: 2 * DAY },
        ] }),
        Request::new(Command::Ask { queries: vec![Query::Stock { town: 0 }, Query::Points] }),
        Request::new(Command::Ask { queries: vec![Query::Stock { town: 5 }] }),
        Request::new(Command::Save),
    ];
    for request in requests {
        let bytes = handle_bytes(&mut a, &postcard::to_allocvec(&request).unwrap());
        let response: Response = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(response, handle(&mut b, request));
    }
    let garbled: Response = postcard::from_bytes(&handle_bytes(&mut a, &[0xff, 0xff, 0xff])).unwrap();
    assert!(matches!(garbled, Response::Refused { why: Refusal::Unreadable { .. } }), "{garbled:?}");
    let truncated = &postcard::to_allocvec(&Request::new(Command::Save)).unwrap()[..2];
    let garbled: Response = postcard::from_bytes(&handle_bytes(&mut a, truncated)).unwrap();
    assert!(matches!(garbled, Response::Refused { why: Refusal::Unreadable { .. } }), "{garbled:?}");
}

#[test]
fn money_crosses_as_its_thousandths_in_an_i64() {
    for millis in [0, 1, -1, 1500, -1500, i64::MAX, i64::MIN] {
        assert_eq!(postcard::to_allocvec(&Fixed::from_millis(millis)).unwrap(), postcard::to_allocvec(&millis).unwrap());
    }
}

#[test]
fn the_host_names_the_same_messages_from_messages() {
    let input: enterprise_core::messages::Input = Input::Advance { to: 1 };
    let _: enterprise_core::Input = input;
    let output: enterprise_core::messages::Output = Output::WagonHome { route: 0, takings: Fixed::ZERO };
    let _: enterprise_core::Output = output;
}

// What a wire type may hold: numbers, strings, the core's ids and money (`Fixed`: an i64, serde-transparent), vectors
// and options of these, and other wire types. Until `wire-gen` lands (and its registry is walked instead), by the
// source: every type `wire/` defines is listed below and derives serde both ways, and its fields name only these.

/// Every wire type.
const WIRE_TYPES: [&str; 16] = ["Request", "Command", "Response", "Refusal", "Input", "Count", "Output", "Query",
    "Answer", "TownPoint", "ContentPack", "PackGood", "GoodFlag", "Point", "PointKind", "SaveFile"];

const ALLOWED: [&str; 18] = ["u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64", "usize", "f64", "bool", "String",
    "Vec", "Option", "TownId", "RouteId", "GoodId", "Fixed"];

fn both_ways<T: serde::Serialize + serde::de::DeserializeOwned>() {}

#[test]
#[cfg_attr(target_os = "wasi", ignore = "reads the source, which WASI under Node can't reach")]
fn every_wire_type_is_listed_and_crosses_both_ways() {
    both_ways::<Request>();
    both_ways::<Command>();
    both_ways::<Response>();
    both_ways::<Refusal>();
    both_ways::<Input>();
    both_ways::<Count>();
    both_ways::<Output>();
    both_ways::<Query>();
    both_ways::<Answer>();
    both_ways::<TownPoint>();
    both_ways::<ContentPack>();
    both_ways::<PackGood>();
    both_ways::<GoodFlag>();
    both_ways::<Point>();
    both_ways::<PointKind>();
    both_ways::<SaveFile>();
    let mut defined: Vec<String> = wire_items().into_iter().map(|i| i.name).collect();
    defined.sort();
    let mut listed: Vec<String> = WIRE_TYPES.iter().map(|s| s.to_string()).collect();
    listed.sort();
    assert_eq!(defined, listed, "every type wire/ defines, and only those, in WIRE_TYPES");
}

#[test]
#[cfg_attr(target_os = "wasi", ignore = "reads the source, which WASI under Node can't reach")]
fn no_wire_type_holds_a_state_type() {
    let mut wrong = Vec::new();
    for item in wire_items() {
        if item.attributes.contains("serde(") {
            wrong.push(format!("{} ({}): a serde attribute (the generator traces plain shapes only)", item.name, item.file));
        }
        if item.body.contains('(') || item.body.contains('#') {
            wrong.push(format!("{} ({}): a tuple field or an attribute inside (name every field)", item.name, item.file));
        }
        for ty in field_types(&item.body) {
            if ty.contains("::") {
                wrong.push(format!("{}: a field of {ty} (by a path: name wire types plainly)", item.name));
            }
            for ident in ty.split(|c: char| !(c.is_alphanumeric() || c == '_')).filter(|w| !w.is_empty()) {
                if !ALLOWED.contains(&ident) && !WIRE_TYPES.contains(&ident) {
                    wrong.push(format!("{}: a field of {ty}: {ident} isn't a number, a string, an id or a wire type", item.name));
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn the_wire_check_catches_a_state_type() {
    let body = "Stock { town: TownId, people: People },\n    Other { held: Vec<crate::state::Stock> },";
    let types = field_types(body);
    assert_eq!(types, vec!["TownId", "People", "Vec<crate::state::Stock>"]);
}

struct Item {
    file: String,
    name: String,
    attributes: String,
    body: String,
}

/// Every `pub struct` and `pub enum` in `src/wire/`, its attributes and its body (comments taken out).
fn wire_items() -> Vec<Item> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src/wire");
    let mut files: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
    files.sort();
    let mut items = Vec::new();
    for path in files {
        let text: String = std::fs::read_to_string(&path).unwrap().lines()
            .map(|l| l.split("//").next().unwrap()).collect::<Vec<_>>().join("\n");
        let file = path.file_name().unwrap().to_string_lossy().to_string();
        let mut attributes = String::new();
        let mut rest = text.as_str();
        while let Some(line_end) = rest.find('\n').or(if rest.is_empty() { None } else { Some(rest.len()) }) {
            let line = rest[..line_end].trim();
            let kind = ["pub struct ", "pub enum "].into_iter().find(|k| line.starts_with(k));
            if let Some(kind) = kind {
                let name: String = line[kind.len()..].chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                let open = rest.find('{').expect("a body");
                let mut depth = 0;
                let mut close = open;
                for (i, c) in rest[open..].char_indices() {
                    match c {
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                close = open + i;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                items.push(Item { file: file.clone(), name, attributes: std::mem::take(&mut attributes),
                    body: rest[open + 1..close].to_string() });
                rest = &rest[close + 1..];
                continue;
            }
            if line.starts_with("#[") {
                attributes.push_str(line);
            } else if !line.is_empty() {
                attributes.clear();
            }
            rest = &rest[(line_end + 1).min(rest.len())..];
        }
    }
    items
}

/// The types a body's fields name: after each `name:`, up to the `,` or `}` that ends it.
fn field_types(body: &str) -> Vec<String> {
    let chars: Vec<char> = body.chars().collect();
    let mut types = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == ':' && chars.get(i + 1) != Some(&':') && (i == 0 || chars[i - 1] != ':') {
            let mut depth = 0;
            let mut j = i + 1;
            while j < chars.len() {
                match chars[j] {
                    '<' => depth += 1,
                    '>' => depth -= 1,
                    ',' | '}' if depth == 0 => break,
                    ':' if depth == 0 && chars.get(j + 1) == Some(&':') => j += 1,
                    _ => {}
                }
                j += 1;
            }
            types.push(chars[i + 1..j].iter().collect::<String>().trim().to_string());
            i = j;
        } else {
            i += 1;
        }
    }
    types
}
