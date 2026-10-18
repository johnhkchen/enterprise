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
    edge.weight = f64::MAX;
    let mut iron = PackGood::new("minecraft:iron_ingot", 0.25, 64);
    iron.weight = 2.5;
    let mut tiny = PackGood::new("tiny", 1.0, 1);
    tiny.weight = f64::MIN_POSITIVE;
    vec![PackGood::new("a", 1.0, 1), wheat, bread, book, beef, edge, iron, tiny]
}

fn amounts() -> Vec<PackAmount> {
    vec![PackAmount { good: "minecraft:wheat".into(), count: 3.0 }, PackAmount { good: String::new(), count: 0.0 },
        PackAmount { good: "g".repeat(150), count: f64::MAX }]
}

fn recipes() -> Vec<PackRecipe> {
    vec![
        PackRecipe { trades: vec!["farmer".into()], inputs: amounts()[..1].to_vec(), output: "minecraft:bread".into(), makes: 1.0 },
        PackRecipe { trades: vec!["librarian".into(), "cartographer".into()],
            inputs: vec![PackAmount { good: "minecraft:paper".into(), count: 3.0 }, PackAmount { good: "minecraft:leather".into(), count: 1.0 }],
            output: "minecraft:book".into(), makes: 0.5 },
        PackRecipe { trades: vec![], inputs: vec![], output: String::new(), makes: 0.0 },
        PackRecipe { trades: (0..20).map(|i| format!("trade_{i}")).collect(), inputs: amounts(), output: "o".repeat(300), makes: f64::MAX },
    ]
}

fn animals() -> Vec<PackAnimal> {
    vec![
        PackAnimal { name: "sheep".into(), feed: vec!["minecraft:wheat".into()], feed_per_day: 0.25, meat: Some("minecraft:mutton".into()),
            meat_each: 1.5, wool: Some("minecraft:white_wool".into()), hide: None },
        PackAnimal { name: "cow".into(), feed: vec!["minecraft:wheat".into()], feed_per_day: 0.25, meat: Some("minecraft:beef".into()),
            meat_each: 2.0, wool: None, hide: Some("minecraft:leather".into()) },
        PackAnimal { name: "pig".into(), feed: vec!["minecraft:carrot".into(), "minecraft:potato".into(), "minecraft:beetroot".into()],
            feed_per_day: 0.25, meat: Some("minecraft:porkchop".into()), meat_each: 2.0, wool: None, hide: None },
        PackAnimal { name: String::new(), feed: vec![], feed_per_day: 0.0, meat: None, meat_each: 0.0, wool: None, hide: None },
        PackAnimal { name: "n".repeat(130), feed: (0..10).map(|i| format!("feed_{i}")).collect(), feed_per_day: f64::MAX,
            meat: Some(String::new()), meat_each: f64::MIN_POSITIVE, wool: Some("w".into()), hide: Some("h".into()) },
    ]
}

fn pack_dials() -> Vec<PackDial> {
    vec![PackDial { name: "birth_mood".into(), value: 0.5 }, PackDial { name: "wagon_slots".into(), value: 27.0 },
        PackDial { name: "standing_orders".into(), value: 0.0 }, PackDial { name: String::new(), value: -1.0 },
        PackDial { name: "d".repeat(100), value: f64::MAX }]
}

fn dial_views() -> Vec<DialView> {
    vec![
        DialView { name: "shelf_days".into(), value: 5.0, initial: 5.0, does: "Days of what its people use a town's warehouse aims to hold.".into() },
        DialView { name: "birth_mood".into(), value: 0.75, initial: 0.5, does: String::new() },
        DialView { name: "d".repeat(100), value: f64::MAX, initial: 0.0, does: "Mood \u{e9}\u{1f33e}".into() },
    ]
}

fn packs() -> Vec<ContentPack> {
    vec![
        ContentPack::new(24_000, goods()),
        ContentPack::new(1, vec![]),
        ContentPack::new(u64::MAX, goods()[..2].to_vec()),
        ContentPack::new(0, (0..130).map(|i| PackGood::new(&format!("test:good_{i}"), 1.0 + i as f64, 64)).collect()),
        ContentPack { day_ticks: 24_000, goods: goods(), comforts: vec!["reading".into(), "treats".into(), String::new()],
            trades: vec!["merchant".into(), "farmer".into(), "none".into(), "nitwit".into()], recipes: recipes(), animals: animals(),
            dials: pack_dials() },
        ContentPack { day_ticks: 1, goods: vec![], comforts: vec![], trades: vec![], recipes: vec![], animals: vec![], dials: vec![] },
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
        Input::SetDial { name: "harvest_window_days".into(), value: Some(6.0) },
        Input::SetDial { name: "bargain".into(), value: None },
        Input::SetDial { name: String::new(), value: Some(f64::MAX) },
        Input::SetDial { name: "d".repeat(200), value: Some(-0.5) },
        Input::AddHand { town: 0, trade: "enterprise:merchant".into(), level: 1, tag: "6f1c2a3e-0000-4000-8000-00000000002a".into() },
        Input::AddHand { town: BIGGEST_ID, trade: String::new(), level: u8::MAX, tag: String::new() },
        Input::AddHand { town: 1, trade: "farmer".into(), level: 0, tag: "Br\u{f6}ly \u{1f33e}".repeat(20) },
        Input::HandLost { hand: 1 },
        Input::HandLost { hand: u32::MAX },
        Input::Census { town: 0, beds: 0, residents: vec![] },
        Input::Census { town: 1, beds: 4, residents: residents() },
        Input::Census { town: BIGGEST_ID, beds: u32::MAX, residents: (0..60).map(|i| Resident { tag: format!("v{i}"),
            trade: "farmer".into(), level: (i % 6) as u8, baby: i % 7 == 0 }).collect() },
        Input::TagHand { hand: 7, tag: "6f1c2a3e-0000-4000-8000-00000000002b".into() },
        Input::TagHand { hand: u32::MAX, tag: String::new() },
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
        Output::Born { town: 0, hand: 7 },
        Output::Born { town: BIGGEST_ID, hand: u32::MAX },
        Output::Settled { town: 2, hand: 12 },
        Output::Departed { town: 1, hand: 12, tag: "6f1c2a3e-0000-4000-8000-00000000002a".into() },
        Output::Departed { town: BIGGEST_ID, hand: u32::MAX, tag: String::new() },
        Output::HandGone { hand: 3, tag: "Br\u{f6}ly \u{1f33e}".repeat(20) },
        Output::HandLevel { hand: 1, tag: "6f1c2a3e-0000-4000-8000-00000000002a".into(), trade: "merchant".into(), level: 3, xp: 1395 },
        Output::HandLevel { hand: u32::MAX, tag: String::new(), trade: "x".repeat(300), level: u8::MAX, xp: u32::MAX },
    ]
}

fn residents() -> Vec<Resident> {
    vec![
        Resident { tag: "6f1c2a3e-0000-4000-8000-00000000002a".into(), trade: "enterprise:merchant".into(), level: 2, baby: false },
        Resident { tag: "6f1c2a3e-0000-4000-8000-00000000002c".into(), trade: "minecraft:none".into(), level: 1, baby: true },
        Resident { tag: String::new(), trade: String::new(), level: 0, baby: false },
        Resident { tag: "\u{1f33e}".repeat(40), trade: "x".repeat(300), level: u8::MAX, baby: true },
    ]
}

fn queries() -> Vec<Query> {
    vec![Query::Stock { town: 0 }, Query::Stock { town: BIGGEST_ID }, Query::Points, Query::Goods, Query::Dials,
        Query::Hands { town: 0 }, Query::Hands { town: BIGGEST_ID }]
}

fn hand_views() -> Vec<HandView> {
    vec![
        HandView { hand: 1, tag: "6f1c2a3e-0000-4000-8000-00000000002a".into(), trade: "merchant".into(), level: 1, xp: 0,
            baby: false, away: false, merchant: true },
        HandView { hand: 7, tag: String::new(), trade: "none".into(), level: 1, xp: 0, baby: true, away: false, merchant: false },
        HandView { hand: u32::MAX, tag: "\u{1f33e}".repeat(40), trade: "x".repeat(300), level: u8::MAX, xp: u32::MAX,
            baby: false, away: true, merchant: false },
    ]
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
        Answer::Goods { goods: vec![] },
        Answer::Goods { goods: vec!["minecraft:wheat".into(), "".into(), "Br\u{f6}ly \u{1f33e}".into()] },
        Answer::Goods { goods: (0..200).map(|i| format!("enterprise:good_{i}")).collect() },
        Answer::Dials { dials: vec![] },
        Answer::Dials { dials: dial_views() },
        Answer::Dials { dials: (0..220).map(|i| DialView { name: format!("dial_{i}"), value: i as f64 / 8.0, initial: 1.0, does: "x".repeat(i % 9) }).collect() },
        Answer::Hands { town: 0, lockers: 0, hands: vec![] },
        Answer::Hands { town: 2, lockers: 10, hands: hand_views() },
        Answer::Hands { town: BIGGEST_ID, lockers: u32::MAX, hands: (0..150).map(|i| HandView { hand: 1 + i, tag: format!("v{i}"),
            trade: "farmer".into(), level: 1 + (i % 5) as u8, xp: i * 7, baby: i % 9 == 0, away: i % 4 == 0, merchant: i == 3 }).collect() },
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
    pack_amount_fixtures: PackAmount = amounts();
    pack_recipe_fixtures: PackRecipe = recipes();
    pack_animal_fixtures: PackAnimal = animals();
    pack_dial_fixtures: PackDial = pack_dials();
    dial_view_fixtures: DialView = dial_views();
    hand_view_fixtures: HandView = hand_views();
    resident_fixtures: Resident = residents();
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
        Input::SetDial { .. } => 14,
        Input::AddHand { .. } => 15,
        Input::HandLost { .. } => 16,
        Input::Census { .. } => 17,
        Input::TagHand { .. } => 18,
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
        Output::Born { .. } => 8,
        Output::Settled { .. } => 9,
        Output::Departed { .. } => 10,
        Output::HandGone { .. } => 11,
        Output::HandLevel { .. } => 12,
    }
}

fn query_place(q: &Query) -> usize {
    match q {
        Query::Stock { .. } => 0,
        Query::Points => 1,
        Query::Goods => 2,
        Query::Dials => 3,
        Query::Hands { .. } => 4,
    }
}

fn answer_place(a: &Answer) -> usize {
    match a {
        Answer::Stock { .. } => 0,
        Answer::Points { .. } => 1,
        Answer::Goods { .. } => 2,
        Answer::Dials { .. } => 3,
        Answer::Hands { .. } => 4,
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
    covers("Input", &inputs(), 19, input_place);
    covers("Output", &outputs(), 13, output_place);
    covers("Query", &queries(), 5, query_place);
    covers("Answer", &answers(), 5, answer_place);
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
