//! The wire's golden files (`sim/wire/fixtures/`): for each wire type, a list of sample values (every variant, and the
//! edges: zero, negatives, the largest and smallest numbers, empty and long vectors, strings of every width) as
//! `NAME.bin` (the list as postcard: a length, then each value, which is what the Java side decodes and re-encodes to
//! the same bytes) with a readable `NAME.json` twin. The bytes Rust writes must equal the files, the twin must read back
//! as the same values, and every variant of a wire enum must appear in its file. A change to a wire type moves them;
//! `just wire-fixtures` writes the new ones, and the diff is what the Java side must follow.
//!
//! NaN and infinity have no JSON and never equal themselves, so they're not samples: a message holding one is a
//! refusal, and `sim/scenarios/refusals.json` sends one as raw bytes.

use std::fmt::Debug;

use enterprise_core::wire::*;
use enterprise_core::Fixed;
use serde::de::DeserializeOwned;
use serde::Serialize;

/// Where the files are, from the crate's folder (Cargo's working folder for its tests, natively and under Node's WASI).
const DIR: &str = if cfg!(target_family = "wasm") { "/sim/wire/fixtures" } else { "../wire/fixtures" };

fn writing() -> bool {
    std::env::var("WIRE_FIXTURES").as_deref() == Ok("write")
}

/// Checks `name`'s files against `samples` (or writes them).
fn fixture<T: Serialize + DeserializeOwned + PartialEq + Debug>(name: &str, samples: Vec<T>) {
    let bin = postcard::to_allocvec(&samples).expect("samples serialize");
    let json = serde_json::to_string_pretty(&samples).expect("samples are JSON") + "\n";
    let (bin_path, json_path) = (format!("{DIR}/{name}.bin"), format!("{DIR}/{name}.json"));
    if writing() {
        std::fs::create_dir_all(DIR).expect("the fixtures' folder");
        std::fs::write(&bin_path, &bin).expect("bin written");
        std::fs::write(&json_path, &json).expect("json written");
        return;
    }
    let want = std::fs::read(&bin_path).unwrap_or_else(|e| panic!("{bin_path}: {e} (`just wire-fixtures` writes them)"));
    assert!(bin == want, "{name}: the bytes Rust writes differ from {bin_path} ({} against {}): a wire type moved; \
        `just wire-fixtures` writes the new files, and the Java side follows", bin.len(), want.len());
    let text = std::fs::read_to_string(&json_path).unwrap_or_else(|e| panic!("{json_path}: {e}"));
    assert!(json == text, "{name}: the JSON twin differs from {json_path}");
    // Both files read back as the samples, and the samples encode to the same bytes again.
    let from_bin: Vec<T> = postcard::from_bytes(&want).expect("the bin file decodes");
    assert_eq!(from_bin, samples, "{name}: the bin file reads back differently");
    let from_json: Vec<T> = serde_json::from_str(&text).expect("the json file decodes");
    assert_eq!(from_json, samples, "{name}: the json file reads back differently");
    assert!(postcard::to_allocvec(&from_json).unwrap() == want, "{name}: re-encoding the twin gives other bytes");
}

/// The largest id a sample holds. Ids on the wire (`TownNo`, `RouteNo`, `GoodNo`, a count's good, an order's place)
/// are `u32`, so a host and a core of any width read the same bytes: `u32::MAX` is the widest an id is written.
const BIGGEST_ID: u32 = u32::MAX;

const DIM: &str = "minecraft:overworld";

fn point(kind: PointKind, x: i32, y: i32, z: i32) -> Point {
    Point { kind, dim: DIM.into(), x, y, z }
}

fn count(good: GoodNo, count: f64) -> Count {
    Count { good, count }
}

/// Numbers a count or a worth may be: zero, the smallest and largest, and ones that need every digit.
const EDGE_F64: [f64; 9] = [0.0, 1.0, 0.1, 0.05, 63.999, 1e-300, 4503599627370496.0, f64::MAX, f64::MIN_POSITIVE];

fn counts() -> Vec<Vec<Count>> {
    vec![
        vec![],
        vec![count(0, 0.0)],
        EDGE_F64.iter().enumerate().map(|(i, &c)| count(i as GoodNo, c)).collect(),
        vec![count(BIGGEST_ID, 3.0)],
        (0..300).map(|i| count(i % 7, i as f64)).collect(),
    ]
}

fn points() -> Vec<Point> {
    vec![
        point(PointKind::Desk, 27, -60, 27),
        point(PointKind::Locker, 0, 0, 0),
        point(PointKind::Shop, -1, -64, -1),
        point(PointKind::Banker, i32::MAX, 319, i32::MIN),
        point(PointKind::Depot, i32::MIN, i32::MIN, i32::MAX),
        Point { kind: PointKind::Desk, dim: String::new(), x: 1, y: 2, z: 3 },
        Point { kind: PointKind::Locker, dim: "minecraft:the_nether".into(), x: -30_000_000, y: 0, z: 30_000_000 },
        Point { kind: PointKind::Shop, dim: "mod:dimension_\u{e9}\u{4e16}\u{1f33e}".into(), x: 5, y: 5, z: 5 },
    ]
}

fn fixeds() -> Vec<Fixed> {
    vec![Fixed::ZERO, Fixed::ONE, Fixed::from_millis(1), Fixed::from_millis(-1), Fixed::emeralds(-5), Fixed::emeralds(40),
        Fixed::from_millis(i64::MIN), Fixed::MAX]
}

fn goods() -> Vec<PackGood> {
    let mut wheat = PackGood::new("minecraft:wheat", 0.05, 64);
    wheat.flags = vec![GoodFlag::Crop];
    let mut bread = PackGood::new("minecraft:bread", 0.2, 64);
    bread.food = 5.0;
    bread.saturation = 6.0;
    let mut book = PackGood::new("minecraft:book", 0.5, 16);
    book.comfort = Some("reading".into());
    let mut beef = PackGood::new("minecraft:cooked_beef", 0.3, 64);
    beef.food = 8.0;
    beef.saturation = 12.8;
    beef.flags = vec![GoodFlag::Meat, GoodFlag::Crop];
    let mut edge = PackGood::new(&"n".repeat(200), f64::MAX, u32::MAX);
    edge.food = f64::MIN_POSITIVE;
    edge.comfort = Some(String::new());
    vec![PackGood::new("a", 1.0, 1), wheat, bread, book, beef, edge]
}

fn packs() -> Vec<ContentPack> {
    vec![
        ContentPack { day_ticks: 24_000, goods: goods() },
        ContentPack { day_ticks: 1, goods: vec![] },
        ContentPack { day_ticks: u64::MAX, goods: goods()[..2].to_vec() },
        ContentPack { day_ticks: 0, goods: (0..130).map(|i| PackGood::new(&format!("test:good_{i}"), 1.0 + i as f64, 64)).collect() },
    ]
}

fn saves() -> Vec<SaveFile> {
    vec![
        SaveFile { format: 0, schema: 0, core: String::new(), blob: vec![] },
        SaveFile { format: 7, schema: 1, core: "enterprise-core 0.1.0".into(), blob: vec![0, 1, 2, 255] },
        SaveFile { format: u32::MAX, schema: u64::MAX, core: "abc1234".into(), blob: (0..=255u8).cycle().take(700).collect() },
    ]
}

fn inputs() -> Vec<Input> {
    let mut all = vec![
        Input::Advance { to: 0 },
        Input::Advance { to: u64::MAX },
        Input::Deliver { town: 0, good: 0, count: 40.0 },
        Input::Deliver { town: BIGGEST_ID, good: 300, count: f64::MAX },
        Input::Pay { town: 1, money: Fixed::emeralds(40) },
        Input::Pay { town: 0, money: Fixed::from_millis(-1) },
        Input::Pay { town: 0, money: Fixed::MAX },
        Input::Order { town: 0, good: 1, lot: 16.0, price: 2.5, purse: 100.0 },
        Input::Stall { town: 0, good: 2, lot: 1.0, price: 0.0, stock: 64.0 },
        Input::OwnShop { town: 2 },
        Input::Buy { town: 0, good: 3, count: 0.5 },
        Input::AddTown { name: "Ashford".into(), desk: point(PointKind::Desk, 27, -60, 27) },
        Input::AddTown { name: String::new(), desk: point(PointKind::Desk, 0, 0, 0) },
        Input::AddTown { name: "Sm\u{f6}rg\u{e5}sbord \u{1f33e}".into(), desk: point(PointKind::Desk, -5, -64, 9) },
        Input::AddTown { name: "x".repeat(300), desk: point(PointKind::Desk, i32::MAX, i32::MAX, i32::MAX) },
        Input::AddPoint { point: point(PointKind::Locker, 30, -60, 27), town: 0 },
        Input::RemovePoint { point: point(PointKind::Locker, 30, -60, 27) },
        Input::PalletBrought { town: 1, id: 12 },
        Input::PalletBrought { town: BIGGEST_ID, id: u64::MAX },
        Input::PackageBrought { town: 1, pallet: 12, package: 2 },
        Input::PackageBrought { town: 0, pallet: u64::MAX, package: u32::MAX },
        Input::PackageLost { pallet: 12, package: 0 },
        Input::PackageLost { pallet: u64::MAX, package: u32::MAX },
    ];
    all.extend(counts().into_iter().map(|counts| Input::Stocktake { point: point(PointKind::Desk, 27, -60, 27), counts }));
    all
}

fn outputs() -> Vec<Output> {
    vec![
        Output::WagonLeft { route: 0, stacks: 0, worth: 0.0 },
        Output::WagonLeft { route: BIGGEST_ID, stacks: u32::MAX, worth: 1234.5 },
        Output::WagonSold { route: 2, sold: 96.0, takings: Fixed::emeralds(12) },
        Output::WagonSold { route: 2, sold: 0.0, takings: Fixed::from_millis(-1) },
        Output::WagonHome { route: 1, takings: Fixed::MAX },
        Output::OrderFilled { town: 0, order: 3, good: 1, lots: 2, count: 32.0, spurs: 80.0 },
        Output::OrderFilled { town: BIGGEST_ID, order: BIGGEST_ID, good: BIGGEST_ID, lots: u32::MAX, count: f64::MAX, spurs: 0.0 },
        Output::Signal { town: 1, good: 4, count: 12.0, spurs_each: 0.25 },
        Output::PalletPacked { id: 0, at: Place::Town { town: 0 }, packages: vec![] },
        Output::PalletPacked { id: 7, at: Place::Custody { town: 2 }, packages: vec![
            PackageLine { kind: 1, good: 3, count: 576.0 }, PackageLine { kind: 0, good: 0, count: 0.5 },
            PackageLine { kind: u32::MAX, good: BIGGEST_ID, count: f64::MAX }] },
        Output::PalletPacked { id: u64::MAX, at: Place::Wagon { route: BIGGEST_ID }, packages: (0..40)
            .map(|i| PackageLine { kind: i % 4, good: i, count: i as f64 * 9.0 }).collect() },
        Output::PalletMoved { id: 7, to: Place::Wagon { route: 3 } },
        Output::PalletMoved { id: u64::MAX, to: Place::Container { container: u64::MAX } },
        Output::PalletOpened { id: 7, at: Place::Town { town: 1 } },
        Output::PalletOpened { id: 0, at: Place::Custody { town: BIGGEST_ID } },
    ]
}

fn queries() -> Vec<Query> {
    vec![Query::Stock { town: 0 }, Query::Stock { town: BIGGEST_ID }, Query::Points]
}

fn answers() -> Vec<Answer> {
    vec![
        Answer::Stock { town: 0, stock: vec![] },
        Answer::Stock { town: 3, stock: vec![40.0, 0.0, 3.0, 0.0] },
        Answer::Stock { town: BIGGEST_ID, stock: EDGE_F64.to_vec() },
        Answer::Stock { town: 1, stock: (0..200).map(|i| i as f64 * 0.5).collect() },
        Answer::Points { points: vec![] },
        Answer::Points { points: points().into_iter().enumerate().map(|(i, point)| TownPoint { point, town: i as TownNo }).collect() },
        Answer::Points { points: (0..150).map(|i| TownPoint { point: point(PointKind::Locker, i, 0, -i), town: i as u32 % 4 }).collect() },
    ]
}

fn refusals() -> Vec<Refusal> {
    vec![
        Refusal::Unreadable { why: "Hit the end of buffer, expected more data".into() },
        Refusal::Abi { core: 1, request: 0 },
        Refusal::Abi { core: u32::MAX, request: u32::MAX },
        Refusal::Schema { core: 1, request: u64::MAX },
        Refusal::NoSociety,
        Refusal::Pack { why: String::new() },
        Refusal::SaveFormat { core: 7, save: 6 },
        Refusal::Save { why: "a save the core can't load".into() },
        Refusal::Input { at: 0, why: "no town 3".into() },
        Refusal::Input { at: u32::MAX, why: "x".repeat(200) },
        Refusal::Query { at: 1, why: "no town 9".into() },
    ]
}

fn commands() -> Vec<Command> {
    vec![
        Command::Create { pack: packs().remove(0), seed: 7 },
        Command::Create { pack: packs().remove(1), seed: u64::MAX },
        Command::Load { save: saves().remove(1) },
        Command::Save,
        Command::Apply { inputs: vec![] },
        Command::Apply { inputs: inputs() },
        Command::Ask { queries: vec![] },
        Command::Ask { queries: queries() },
    ]
}

fn requests() -> Vec<Request> {
    let mut all: Vec<Request> = commands().into_iter().map(Request::new).collect();
    all.push(Request { abi: 0, schema: 0, command: Command::Save });
    all.push(Request { abi: u32::MAX, schema: u64::MAX, command: Command::Ask { queries: queries() } });
    all
}

fn responses() -> Vec<Response> {
    vec![
        Response::Done { outputs: vec![], answers: vec![] },
        Response::Done { outputs: outputs(), answers: vec![] },
        Response::Done { outputs: vec![], answers: answers() },
        Response::Done { outputs: outputs(), answers: answers() },
        Response::Saved { save: saves().remove(0) },
        Response::Saved { save: saves().remove(2) },
    ]
    .into_iter()
    .chain(refusals().into_iter().map(|why| Response::Refused { why }))
    .collect()
}

macro_rules! fixtures {
    ($($test:ident: $ty:ident = $samples:expr;)*) => {
        $(#[test] fn $test() { fixture::<$ty>(stringify!($ty), $samples); })*
    };
}

fixtures! {
    request_fixtures: Request = requests();
    response_fixtures: Response = responses();
    command_fixtures: Command = commands();
    input_fixtures: Input = inputs();
    output_fixtures: Output = outputs();
    query_fixtures: Query = queries();
    answer_fixtures: Answer = answers();
    refusal_fixtures: Refusal = refusals();
    point_fixtures: Point = points();
    point_kind_fixtures: PointKind = vec![PointKind::Desk, PointKind::Locker, PointKind::Shop, PointKind::Banker, PointKind::Depot];
    place_fixtures: Place = vec![Place::Town { town: 0 }, Place::Custody { town: 1 }, Place::Wagon { route: BIGGEST_ID },
        Place::Container { container: u64::MAX }];
    package_line_fixtures: PackageLine = vec![PackageLine { kind: 0, good: 0, count: 0.0 },
        PackageLine { kind: u32::MAX, good: BIGGEST_ID, count: f64::MAX }];
    count_fixtures: Count = counts().concat();
    town_point_fixtures: TownPoint = points().into_iter().map(|point| TownPoint { point, town: 0 }).collect();
    content_pack_fixtures: ContentPack = packs();
    pack_good_fixtures: PackGood = goods();
    good_flag_fixtures: GoodFlag = vec![GoodFlag::Crop, GoodFlag::Meat];
    save_file_fixtures: SaveFile = saves();
    fixed_fixtures: Fixed = fixeds();
}

// Every variant of a wire enum appears in its file. The matches are exhaustive: a variant added to a wire type is a
// compile error here until it has a place and a sample.

fn input_place(i: &Input) -> usize {
    match i {
        Input::Advance { .. } => 0,
        Input::Deliver { .. } => 1,
        Input::Pay { .. } => 2,
        Input::Order { .. } => 3,
        Input::Stall { .. } => 4,
        Input::OwnShop { .. } => 5,
        Input::Buy { .. } => 6,
        Input::AddTown { .. } => 7,
        Input::AddPoint { .. } => 8,
        Input::RemovePoint { .. } => 9,
        Input::Stocktake { .. } => 10,
        Input::PalletBrought { .. } => 11,
        Input::PackageBrought { .. } => 12,
        Input::PackageLost { .. } => 13,
    }
}

fn output_place(o: &Output) -> usize {
    match o {
        Output::WagonLeft { .. } => 0,
        Output::WagonSold { .. } => 1,
        Output::WagonHome { .. } => 2,
        Output::OrderFilled { .. } => 3,
        Output::Signal { .. } => 4,
        Output::PalletPacked { .. } => 5,
        Output::PalletMoved { .. } => 6,
        Output::PalletOpened { .. } => 7,
    }
}

fn query_place(q: &Query) -> usize {
    match q {
        Query::Stock { .. } => 0,
        Query::Points => 1,
    }
}

fn answer_place(a: &Answer) -> usize {
    match a {
        Answer::Stock { .. } => 0,
        Answer::Points { .. } => 1,
    }
}

fn command_place(c: &Command) -> usize {
    match c {
        Command::Create { .. } => 0,
        Command::Load { .. } => 1,
        Command::Save => 2,
        Command::Apply { .. } => 3,
        Command::Ask { .. } => 4,
    }
}

fn response_place(r: &Response) -> usize {
    match r {
        Response::Done { .. } => 0,
        Response::Saved { .. } => 1,
        Response::Refused { .. } => 2,
    }
}

fn refusal_place(r: &Refusal) -> usize {
    match r {
        Refusal::Unreadable { .. } => 0,
        Refusal::Abi { .. } => 1,
        Refusal::Schema { .. } => 2,
        Refusal::NoSociety => 3,
        Refusal::Pack { .. } => 4,
        Refusal::SaveFormat { .. } => 5,
        Refusal::Save { .. } => 6,
        Refusal::Input { .. } => 7,
        Refusal::Query { .. } => 8,
    }
}

fn covers<T>(what: &str, samples: &[T], variants: usize, place: fn(&T) -> usize) {
    let mut seen = vec![false; variants];
    for s in samples {
        seen[place(s)] = true;
    }
    let missing: Vec<usize> = (0..variants).filter(|&v| !seen[v]).collect();
    assert!(missing.is_empty(), "{what}: no sample of variants {missing:?}");
}

#[test]
fn every_variant_of_every_wire_enum_has_a_sample() {
    covers("Input", &inputs(), 14, input_place);
    covers("Output", &outputs(), 8, output_place);
    covers("Query", &queries(), 2, query_place);
    covers("Answer", &answers(), 2, answer_place);
    covers("Command", &commands(), 5, command_place);
    covers("Response", &responses(), 3, response_place);
    covers("Refusal", &refusals(), 9, refusal_place);
}

/// The places of the variants are what postcard writes, so a reordering moves these bytes: the first byte of each
/// variant's encoding is its place. (The files pin the rest; this names the rule they pin.)
#[test]
fn a_variant_is_named_by_its_place() {
    for (i, input) in inputs().iter().enumerate() {
        let bytes = postcard::to_allocvec(input).unwrap();
        assert_eq!(bytes[0] as usize, input_place(input), "input sample {i}");
    }
    for output in outputs() {
        assert_eq!(postcard::to_allocvec(&output).unwrap()[0] as usize, output_place(&output));
    }
    for r in refusals() {
        assert_eq!(postcard::to_allocvec(&r).unwrap()[0] as usize, refusal_place(&r));
    }
    for q in queries() {
        assert_eq!(postcard::to_allocvec(&q).unwrap()[0] as usize, query_place(&q));
    }
    for a in answers() {
        assert_eq!(postcard::to_allocvec(&a).unwrap()[0] as usize, answer_place(&a));
    }
    for c in commands() {
        assert_eq!(postcard::to_allocvec(&c).unwrap()[0] as usize, command_place(&c));
    }
    for r in responses() {
        assert_eq!(postcard::to_allocvec(&r).unwrap()[0] as usize, response_place(&r));
    }
    for (i, k) in [PointKind::Desk, PointKind::Locker, PointKind::Shop, PointKind::Banker, PointKind::Depot].iter().enumerate() {
        assert_eq!(postcard::to_allocvec(k).unwrap(), vec![i as u8], "{k:?}");
    }
    for (i, p) in [Place::Town { town: 0 }, Place::Custody { town: 0 }, Place::Wagon { route: 0 }, Place::Container { container: 0 }]
        .iter().enumerate() {
        assert_eq!(postcard::to_allocvec(p).unwrap()[0] as usize, i, "{p:?}");
    }
    for (i, f) in [GoodFlag::Crop, GoodFlag::Meat].iter().enumerate() {
        assert_eq!(postcard::to_allocvec(f).unwrap(), vec![i as u8], "{f:?}");
    }
}

/// serde_json writes numbers shortest-round-trip and reads them back exactly for the samples, which is what lets the
/// twin stand for the bytes.
#[test]
fn the_edge_numbers_survive_the_json_twin() {
    for x in EDGE_F64 {
        let back: f64 = serde_json::from_str(&serde_json::to_string(&x).unwrap()).unwrap();
        assert_eq!(back.to_bits(), x.to_bits());
    }
}
