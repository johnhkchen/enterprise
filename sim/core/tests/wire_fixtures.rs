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
        point(PointKind::Market, -2_400, 64, 27),
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
            dials: pack_dials(), packages: pack_packages() },
        ContentPack { day_ticks: 1, goods: vec![], comforts: vec![], trades: vec![], recipes: vec![], animals: vec![], dials: vec![],
            packages: vec![] },
    ]
}

fn pack_packages() -> Vec<PackPackage> {
    vec![
        PackPackage { name: "loose".into(), slots: 0, tare: 0.0, emeralds: 0.0, returnable: false },
        PackPackage { name: "crate".into(), slots: 9, tare: 0.5, emeralds: 0.125, returnable: true },
        PackPackage { name: "Br\u{f6}ly \u{1f33e}".into(), slots: u32::MAX, tare: f64::MAX, emeralds: f64::MIN_POSITIVE, returnable: false },
        PackPackage { name: String::new(), slots: 1, tare: -0.5, emeralds: -1.0, returnable: true },
    ]
}

fn pallet_views() -> Vec<PalletView> {
    vec![
        PalletView { id: 0, at: Place::Town { town: 0 }, since: 0, shipper: 0, consignee: 1, from: String::new(), to: String::new(),
            contract: None, paid: false, issued: 0, sealed: false, intact: false, digest: 0, packages: vec![], items: 0,
            weight: 0.0, shown: None, arriving: vec![] },
        PalletView { id: 12, at: Place::Town { town: 2 }, since: 24_000, shipper: 2, consignee: 1, from: "Ashford".into(),
            to: "Br\u{f6}ly \u{1f33e}".into(), contract: Some(7), paid: true, issued: 23_000, sealed: true, intact: true,
            digest: 0xcbf2_9ce4_8422_2325, packages: vec![PackageLine { kind: 1, good: 3, count: 576.0 },
                PackageLine { kind: 1, good: 3, count: 48.0 }, PackageLine { kind: 0, good: 0, count: 0.5 }],
            items: 3, weight: 9.75, shown: Some(point(PointKind::Desk, 27, -60, 27)), arriving: vec![2, 0] },
        PalletView { id: u64::MAX, at: Place::Container { container: u64::MAX }, since: u64::MAX, shipper: BIGGEST_ID,
            consignee: BIGGEST_ID, from: "x".repeat(300), to: "\u{1f33e}".repeat(40), contract: Some(BIGGEST_ID), paid: false,
            issued: u64::MAX, sealed: true, intact: false, digest: u64::MAX, packages: (0..40)
            .map(|i| PackageLine { kind: i % 4, good: i, count: i as f64 * 9.0 }).collect(), items: u32::MAX, weight: f64::MAX,
            shown: Some(point(PointKind::Desk, i32::MIN, i32::MAX, i32::MIN)), arriving: (0..40).collect() },
    ]
}

fn manifest_lines() -> Vec<ManifestLine> {
    let v = pallet_views();
    vec![
        ManifestLine { pallet: v[0].clone(), drop: None, container: None },
        ManifestLine { pallet: v[1].clone(), drop: Some(1), container: Some(0) },
        ManifestLine { pallet: v[2].clone(), drop: Some(BIGGEST_ID), container: Some(u64::MAX) },
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
    all.extend(counts().into_iter().map(|counts| Input::Stocktake { point: point(PointKind::Desk, 27, -60, 27), counts, free: 27 }));
    all.push(Input::Stocktake { point: point(PointKind::Depot, 31, -60, 27), counts: vec![], free: 0 });
    all.push(Input::Stocktake { point: point(PointKind::Desk, 0, 0, 0), counts: vec![], free: u32::MAX });
    all.extend([
        Input::Unmoved { point: point(PointKind::Desk, 27, -60, 27), good: 0, count: 64 },
        Input::Unmoved { point: point(PointKind::Depot, 31, -60, 27), good: 2, count: -1 },
        Input::Unmoved { point: point(PointKind::Desk, i32::MIN, i32::MIN, i32::MIN), good: BIGGEST_ID, count: i64::MIN },
        Input::Unmoved { point: point(PointKind::Desk, 0, 0, 0), good: 1, count: i64::MAX },
        Input::Cashcount { point: point(PointKind::Desk, 27, -60, 27), emeralds: 0, coins: 0, bank: None, coin_room: false, writes: false },
        Input::Cashcount { point: point(PointKind::Desk, 27, -60, 27), emeralds: 100, coins: 4_096, bank: Some(19_164),
            coin_room: true, writes: true },
        Input::Cashcount { point: point(PointKind::Desk, i32::MIN, i32::MIN, i32::MIN), emeralds: u64::MAX, coins: u64::MAX,
            bank: Some(u64::MAX), coin_room: true, writes: false },
        Input::Cashcount { point: point(PointKind::Desk, 0, 0, 0), emeralds: 3, coins: 1, bank: Some(0), coin_room: false, writes: true },
        Input::CashUnmoved { point: point(PointKind::Desk, 27, -60, 27), emeralds: 64, coins: 0, bank: 0 },
        Input::CashUnmoved { point: point(PointKind::Desk, 27, -60, 27), emeralds: -2, coins: -4_096, bank: -1 },
        Input::CashUnmoved { point: point(PointKind::Desk, i32::MAX, i32::MAX, i32::MAX), emeralds: i64::MIN, coins: i64::MAX,
            bank: i64::MIN },
        Input::Ground { hex: hex(0, 0), water: 0.0, relief: 0.0, land: LandKind::Plains, height: None, river: false },
        Input::Ground { hex: hex(-3, 7), water: 0.5625, relief: 31.0, land: LandKind::Water, height: Some(-64.0), river: true },
        Input::Ground { hex: hex(i32::MIN, i32::MAX), water: 1.0, relief: f64::MAX, land: LandKind::Hills, height: Some(f64::MAX), river: false },
        Input::Crops { hex: hex(0, 0), chunk_x: 0, chunk_z: -1, counts: vec![] },
        Input::Crops { hex: hex(2, -1), chunk_x: i32::MIN, chunk_z: i32::MAX, counts: vec![count(3, 80.0), count(BIGGEST_ID, 1.0)] },
        Input::Survey { town: 0, bell_x: 27, bell_z: 24 },
        Input::Survey { town: BIGGEST_ID, bell_x: i32::MIN, bell_z: i32::MAX },
        Input::Switch { hand: 3, offer: 0, good: 2, on: true },
        Input::Switch { hand: u32::MAX, offer: u32::MAX, good: BIGGEST_ID, on: false },
        Input::Locker { hand: 3, at: Some(point(PointKind::Locker, 30, -60, 27)) },
        Input::Locker { hand: 3, at: None },
        Input::Locker { hand: u32::MAX, at: Some(point(PointKind::Locker, i32::MIN, i32::MAX, i32::MIN)) },
        Input::Route { from: 0, to: 1, send: true },
        Input::Route { from: BIGGEST_ID, to: 0, send: false },
        Input::Pack { town: 0, consignee: 1, goods: vec![] },
        Input::Pack { town: 2, consignee: 0, goods: vec![count(0, 1200.0), count(3, 0.5)] },
        Input::Pack { town: BIGGEST_ID, consignee: BIGGEST_ID, goods: counts().remove(2) },
        Input::Stowed { point: point(PointKind::Desk, 27, -60, 27), pallet: 12 },
        Input::Stowed { point: point(PointKind::Desk, i32::MIN, i32::MIN, i32::MIN), pallet: u64::MAX },
    ]);
    all.extend(vendor_reads().into_iter().enumerate().map(|(i, read)| Input::Vendor {
        point: point(PointKind::Shop, 30 + i as i32, -60, 20), read }));
    all.extend([
        Input::Board { point: point(PointKind::Shop, 27, -60, 28) },
        Input::Board { point: point(PointKind::Shop, i32::MIN, i32::MAX, i32::MIN) },
        Input::DeedShop { rent: 0, point: Some(point(PointKind::Shop, 21, -60, 33)) },
        Input::DeedShop { rent: u32::MAX, point: None },
        Input::DeedBrought { town: 0, rent: 3 },
        Input::DeedBrought { town: BIGGEST_ID, rent: u32::MAX },
        Input::OffMap { name: "the front".into(), at: point(PointKind::Market, 27, 64, -1_473), sells: false, realm: String::new(),
            spurs: 0, partners: vec![] },
        Input::OffMap { name: "the Coast".into(), at: point(PointKind::Market, -2_400, 64, 27), sells: true, realm: "coast".into(),
            spurs: 20_000, partners: vec![0, 1, 2] },
        Input::OffMap { name: "x".repeat(300), at: point(PointKind::Market, i32::MIN, i32::MAX, i32::MIN), sells: true,
            realm: "x".repeat(300), spurs: u64::MAX, partners: vec![BIGGEST_ID; 40] },
        Input::Capital { town: 0, spurs_per_day: 16 },
        Input::Capital { town: BIGGEST_ID, spurs_per_day: u64::MAX },
        Input::Mobilise { town: 0, spurs_per_day: 480 },
        Input::Mobilise { town: BIGGEST_ID, spurs_per_day: u64::MAX },
        Input::Declare { theatre: "faction".into() },
        Input::Declare { theatre: "x".repeat(300) },
        Input::Peace,
        Input::CrownShop { point: point(PointKind::Shop, 27, -60, 17) },
        Input::CrownShop { point: point(PointKind::Shop, i32::MIN, i32::MAX, i32::MIN) },
        Input::Port { town: 2, x: -620, z: -351 },
        Input::Port { town: BIGGEST_ID, x: i32::MIN, z: i32::MAX },
    ]);
    all.extend(works().into_iter().enumerate().map(|(i, work)| Input::Built { town: i as TownNo, work, share: 0.25 * i as f64 }));
    all.push(Input::Built { town: BIGGEST_ID, work: Work::Wall, share: 1.0 });
    all
}

fn vendor_reads() -> Vec<VendorRead> {
    vec![
        VendorRead { mode: VendorMode::Idle, good: None, deed: None, lot: 0, price: 0, stock: 0, room: 0, purse: 0, coins: 0,
            touches: false },
        VendorRead { mode: VendorMode::Buy, good: Some(1), deed: None, lot: 16, price: 16, stock: 0, room: 54, purse: 40, coins: 0,
            touches: false },
        VendorRead { mode: VendorMode::Sell, good: Some(3), deed: None, lot: 4, price: 12, stock: 8, room: 0, purse: 0, coins: 24,
            touches: false },
        VendorRead { mode: VendorMode::Sell, good: Some(2), deed: None, lot: 16, price: 9, stock: 64, room: 0, purse: 0, coins: 9,
            touches: true },
        VendorRead { mode: VendorMode::Buy, good: None, deed: Some(7), lot: 1, price: 480, stock: 1, room: 26, purse: 960, coins: 0,
            touches: false },
        VendorRead { mode: VendorMode::Sell, good: Some(BIGGEST_ID), deed: Some(u32::MAX), lot: u32::MAX, price: u32::MAX,
            stock: u32::MAX, room: u32::MAX, purse: u64::MAX, coins: u64::MAX, touches: true },
    ]
}

fn shop_lines() -> Vec<ShopLine> {
    let line = |role, good, rent| ShopLine { point: point(PointKind::Shop, 33, -60, 20), role, good, lot: 16.0, price: 9.0, purse: 8.0,
        open: 20.0, delivered: 32.0, paid: 32.0, stock: 48.0, rent };
    vec![line(ShopRole::Order, Some(0), None), line(ShopRole::Stall, Some(1), None), line(ShopRole::Own, Some(2), None),
        line(ShopRole::Board, None, None), line(ShopRole::Deed, None, Some(4)), line(ShopRole::Idle, Some(BIGGEST_ID), Some(u32::MAX))]
}

fn deed_lines() -> Vec<DeedLine> {
    vec![
        DeedLine { rent: 0, farm: 2, farm_name: "Ashford".into(), crop: 4, quantity: 64.0, ask: 1_290, bid: 1_056, bearer: false, accrued: 0.0,
            point: Some(point(PointKind::Shop, 21, -60, 33)) },
        DeedLine { rent: 3, farm: 1, farm_name: String::new(), crop: 0, quantity: 32.0, ask: 512, bid: 400, bearer: true, accrued: 12.5, point: None },
        DeedLine { rent: u32::MAX, farm: BIGGEST_ID, farm_name: "\u{1f33e}".repeat(40), crop: BIGGEST_ID, quantity: f64::MAX, ask: u32::MAX, bid: 0, bearer: true,
            accrued: f64::MAX, point: Some(point(PointKind::Shop, i32::MIN, i32::MAX, i32::MIN)) },
    ]
}

fn hex(q: i32, r: i32) -> HexAt {
    HexAt { q, r }
}

fn works() -> Vec<Work> {
    vec![
        Work::Bridge { hex: hex(1, 2), from: hex(0, 2), to: hex(2, 2) },
        Work::Bridge { hex: hex(i32::MAX, i32::MIN), from: hex(i32::MIN, 0), to: hex(0, i32::MAX) },
        Work::Wall,
        Work::Hall { kind: "watchtower".into() },
        Work::Hall { kind: String::new() },
        Work::Field { crop: 0 },
        Work::Field { crop: BIGGEST_ID },
        Work::Berth,
    ]
}

const LANDS: [LandKind; 7] = [LandKind::Plains, LandKind::Woods, LandKind::Taiga, LandKind::Desert, LandKind::Shore,
    LandKind::Hills, LandKind::Water];

fn work_views() -> Vec<WorkView> {
    works().into_iter().enumerate()
        .map(|(i, work)| WorkView { work, laid: i as f64 / 6.0, done: i % 2 == 0, shown: EDGE_F64[i % EDGE_F64.len()] })
        .collect()
}

fn ground_views() -> Vec<GroundView> {
    LANDS.iter().enumerate()
        .map(|(i, &land)| GroundView { hex: hex(i as i32, -(i as i32)), water: i as f64 / 7.0, relief: EDGE_F64[i], land,
            bridged: i % 3 == 0, height: if i % 2 == 0 { Some(-EDGE_F64[i]) } else { None }, river: i % 4 == 1 })
        .collect()
}

fn outputs() -> Vec<Output> {
    vec![
        Output::WagonLeft { route: 0, stacks: 0, worth: 0.0 },
        Output::WagonLeft { route: BIGGEST_ID, stacks: u32::MAX, worth: 1234.5 },
        Output::WagonSold { route: 2, sold: 96.0, takings: Fixed::emeralds(12) },
        Output::WagonSold { route: 2, sold: 0.0, takings: Fixed::from_millis(-1) },
        Output::WagonHome { route: 1, takings: Fixed::MAX },
        Output::OrderFilled { town: 0, order: 3, good: 1, lots: 2, count: 32.0, spurs: 80.0, point: None },
        Output::OrderFilled { town: BIGGEST_ID, order: BIGGEST_ID, good: BIGGEST_ID, lots: u32::MAX, count: f64::MAX, spurs: 0.0,
            point: Some(point(PointKind::Shop, i32::MIN, i32::MAX, i32::MIN)) },
        Output::OrderFilled { town: 1, order: 0, good: 0, lots: 1, count: 16.0, spurs: 16.0, point: Some(point(PointKind::Shop, 33, -60, 20)) },
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
        Output::Moved { point: point(PointKind::Desk, 27, -60, 27), good: 0, count: 10 },
        Output::Moved { point: point(PointKind::Depot, 31, -60, 27), good: 3, count: -64 },
        Output::Moved { point: point(PointKind::Desk, i32::MAX, i32::MAX, i32::MAX), good: BIGGEST_ID, count: i64::MAX },
        Output::Moved { point: point(PointKind::Depot, -5, -64, 9), good: 1, count: i64::MIN },
        Output::Cash { point: point(PointKind::Desk, 27, -60, 27), emeralds: 12, coins: 0, bank: 0 },
        Output::Cash { point: point(PointKind::Desk, 27, -60, 27), emeralds: -36, coins: 574, bank: 0 },
        Output::Cash { point: point(PointKind::Desk, 0, 0, 0), emeralds: -100, coins: 0, bank: 19_164 },
        Output::Cash { point: point(PointKind::Desk, i32::MAX, i32::MAX, i32::MAX), emeralds: i64::MIN, coins: i64::MAX, bank: i64::MIN },
        Output::Build { town: 0, work: Work::Wall, share: 0.25 },
        Output::Build { town: BIGGEST_ID, work: works().remove(1), share: 1.0 },
        Output::Build { town: 2, work: Work::Hall { kind: "caravanserai".into() }, share: 0.0 },
        Output::Build { town: 1, work: Work::Field { crop: 4 }, share: 1.0 },
        Output::HandAway { town: 0, hand: 2, tag: "6f1c2a3e-0000-4000-8000-00000000002a".into() },
        Output::HandAway { town: BIGGEST_ID, hand: u32::MAX, tag: "\u{1f33e}".repeat(40) },
        Output::HandBack { town: 0, hand: 2, tag: "6f1c2a3e-0000-4000-8000-00000000002a".into(), trade: "merchant".into(),
            level: 1, xp: 1 },
        Output::HandBack { town: BIGGEST_ID, hand: u32::MAX, tag: String::new(), trade: "x".repeat(300), level: u8::MAX,
            xp: u32::MAX },
        Output::StallBought { point: point(PointKind::Shop, 33, -60, 14), good: 1, lots: 2, count: 8.0, spurs: 24.0 },
        Output::StallBought { point: point(PointKind::Shop, i32::MIN, i32::MAX, i32::MIN), good: BIGGEST_ID, lots: u32::MAX,
            count: f64::MAX, spurs: f64::MAX },
        Output::Shelf { point: point(PointKind::Shop, 27, -60, 28), good: Some(2), lot: 16, price: 9, add: 64, clear: false },
        Output::Shelf { point: point(PointKind::Shop, 28, -60, 28), good: Some(0), lot: 8, price: 5, add: -3, clear: true },
        Output::Shelf { point: point(PointKind::Shop, 29, -60, 28), good: None, lot: 0, price: 0, add: 0, clear: true },
        Output::Shelf { point: point(PointKind::Shop, i32::MAX, 0, i32::MAX), good: Some(BIGGEST_ID), lot: u32::MAX, price: u32::MAX,
            add: i64::MIN, clear: false },
        Output::Deed { point: point(PointKind::Shop, 21, -60, 33), rent: 0, selling: true, price: 1_290, fund: 0 },
        Output::Deed { point: point(PointKind::Shop, 21, -60, 33), rent: 0, selling: false, price: 1_056, fund: 1_100 },
        Output::Deed { point: point(PointKind::Shop, i32::MIN, i32::MAX, i32::MIN), rent: u32::MAX, selling: false, price: u32::MAX,
            fund: u64::MAX },
        Output::DeedPaid { town: 0, rent: 3, good: 4, count: 12, owed: 0 },
        Output::DeedPaid { town: BIGGEST_ID, rent: u32::MAX, good: BIGGEST_ID, count: u32::MAX, owed: u32::MAX },
        Output::BuildShop { town: 0 },
        Output::BuildShop { town: BIGGEST_ID },
        Output::WarDeclared { levies: 0 },
        Output::WarDeclared { levies: u32::MAX },
        Output::PeaceMade { home: 24 },
        Output::PeaceMade { home: u32::MAX },
        Output::BuildCrownShop { town: 0 },
        Output::BuildCrownShop { town: BIGGEST_ID },
        Output::CrownShop { point: point(PointKind::Shop, 27, -60, 17), good: 5, lot: 8, price: 1_200, fund: 2_400, taken: 3 },
        Output::CrownShop { point: point(PointKind::Shop, i32::MIN, i32::MAX, i32::MIN), good: BIGGEST_ID, lot: u32::MAX,
            price: u32::MAX, fund: u64::MAX, taken: u32::MAX },
        Output::CrownShopDown { point: point(PointKind::Shop, 27, -60, 17) },
        Output::CrownShopDown { point: point(PointKind::Shop, i32::MIN, i32::MAX, i32::MIN) },
        Output::Found { town: 7, name: "Newfound 3".into(), x: 1640, z: -360 },
        Output::Found { town: u32::MAX, name: String::new(), x: i32::MIN, z: i32::MAX },
        Output::Trail { hex: hex(3, -2), from: hex(2, -2), to: hex(4, -2), width: 2, path: vec![hex(2, -2), hex(3, -2), hex(4, -2)], llama: false },
        Output::Trail { hex: hex(i32::MIN, i32::MAX), from: hex(i32::MIN, i32::MAX), to: hex(i32::MAX, i32::MIN), width: u32::MAX, path: vec![], llama: true },
        Output::Stall { town: 1, hand: 7, tag: "1d0c5a3e-0000-4000-8000-000000000007".into(), trade: "farmer".into() },
        Output::Stall { town: u32::MAX, hand: u32::MAX, tag: String::new(), trade: "é".into() },
        Output::Explore { hexes: vec![hex(2, -2), hex(3, -2), hex(4, -2)] },
        Output::Explore { hexes: vec![] },
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
        Query::Hands { town: 0 }, Query::Hands { town: BIGGEST_ID }, Query::Chests { point: point(PointKind::Desk, 27, -60, 27) },
        Query::Chests { point: point(PointKind::Depot, i32::MIN, 0, i32::MAX) }, Query::Cash { point: point(PointKind::Desk, 27, -60, 27) },
        Query::Cash { point: point(PointKind::Desk, i32::MIN, i32::MIN, i32::MAX) }, Query::Mint,
        Query::Land { town: 0 },
        Query::Land { town: BIGGEST_ID }, Query::Ground { hexes: vec![] },
        Query::Ground { hexes: (0..60).map(|i| hex(i - 30, 30 - i)).collect() },
        Query::Lockers { town: 0 }, Query::Lockers { town: BIGGEST_ID }, Query::Routes { town: 0 },
        Query::Routes { town: BIGGEST_ID }, Query::Pallets { town: 0 }, Query::Pallets { town: BIGGEST_ID },
        Query::Pallet { id: 0 }, Query::Pallet { id: u64::MAX }, Query::Stow { point: point(PointKind::Desk, 27, -60, 27), free: 27 },
        Query::Stow { point: point(PointKind::Desk, i32::MAX, i32::MIN, i32::MAX), free: u32::MAX }, Query::Manifest { route: 0 },
        Query::Manifest { route: BIGGEST_ID },
        Query::CashBook { town: 0, days: 10 },
        Query::CashBook { town: BIGGEST_ID, days: u32::MAX }, Query::Books { town: 0 }, Query::Books { town: BIGGEST_ID },
        Query::Whys { town: 0 }, Query::Whys { town: BIGGEST_ID }, Query::Trust, Query::Contracts { town: None },
        Query::Contracts { town: Some(2) }, Query::Contracts { town: Some(BIGGEST_ID) }, Query::Territory { town: 0 },
        Query::Territory { town: BIGGEST_ID }, Query::War, Query::RouteBooks,
        Query::Shops { town: 0 }, Query::Shops { town: BIGGEST_ID }, Query::Deeds { town: 0 }, Query::Deeds { town: BIGGEST_ID },
        Query::Realms, Query::Metals]
}

fn offer_views() -> Vec<OfferView> {
    vec![
        OfferView { offer: 0, good: 4, sells: false, count: 24, emeralds: 1, price: 19, uses: 16, used: 2, xp: 2, level: 1, demand: 0,
            inflated: false, on: true, training: false, procured: false },
        OfferView { offer: 3, good: 9, sells: true, count: 1, emeralds: 9, price: 9, uses: 12, used: 0, xp: 1, level: 1, demand: -6,
            inflated: false, on: false, training: true, procured: true },
        OfferView { offer: u32::MAX, good: BIGGEST_ID, sells: true, count: u32::MAX, emeralds: u32::MAX, price: u32::MAX,
            uses: u8::MAX, used: u8::MAX, xp: u32::MAX, level: u8::MAX, demand: i32::MIN, inflated: true, on: true, training: true,
            procured: true },
        OfferView { offer: 1, good: 0, sells: false, count: 0, emeralds: 0, price: 1, uses: 0, used: 0, xp: 0, level: 0,
            demand: i32::MAX, inflated: true, on: false, training: false, procured: false },
    ]
}

fn locker_views() -> Vec<LockerView> {
    vec![
        LockerView { hand: 2, managed: true, client: true, at: Some(point(PointKind::Locker, 30, -60, 27)), worth: 1.5,
            offers: offer_views() },
        LockerView { hand: 5, managed: false, client: false, at: None, worth: 0.0, offers: vec![] },
        LockerView { hand: u32::MAX, managed: true, client: false, at: Some(point(PointKind::Locker, i32::MAX, 0, i32::MIN)),
            worth: f64::MAX, offers: (0..40).map(|i| OfferView { offer: i, good: i % 7, sells: i % 2 == 0, count: i, emeralds: 1 + i,
                price: 1 + i / 2, uses: (i % 17) as u8, used: 0, xp: i, level: 1 + (i % 5) as u8, demand: i as i32 - 20,
                inflated: i % 3 == 0, on: i % 4 == 0, training: false, procured: i % 5 == 0 }).collect() },
    ]
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
        Answer::Chests { point: point(PointKind::Desk, 27, -60, 27), town: 0, held: vec![], free: 0, short: vec![] },
        Answer::Chests { point: point(PointKind::Depot, 31, -60, 27), town: 2, held: vec![40.0, 0.0, 3.0, 64.0], free: 27,
            short: vec![0.0, 2.0] },
        Answer::Chests { point: point(PointKind::Desk, i32::MAX, i32::MAX, i32::MAX), town: BIGGEST_ID, held: EDGE_F64.to_vec(),
            free: u32::MAX, short: (0..200).map(|i| i as f64).collect() },
        Answer::Cash { point: point(PointKind::Desk, 27, -60, 27), town: 0, treasury: Fixed::ZERO, emeralds: 0, coins: 0,
            coin_worth: Fixed::ZERO, bank: None, bank_worth: Fixed::ZERO, reserve: 0 },
        Answer::Cash { point: point(PointKind::Desk, 27, -60, 27), town: 2, treasury: Fixed::from_millis(1_234_567), emeralds: 64,
            coins: 4_096, coin_worth: Fixed::emeralds(256), bank: Some(19_164), bank_worth: Fixed::emeralds(1_728), reserve: 64 },
        Answer::Cash { point: point(PointKind::Desk, i32::MAX, i32::MAX, i32::MAX), town: BIGGEST_ID, treasury: Fixed::MAX,
            emeralds: u64::MAX, coins: u64::MAX, coin_worth: Fixed::from_millis(i64::MIN), bank: Some(u64::MAX),
            bank_worth: Fixed::MAX, reserve: u64::MAX },
        Answer::Mint { rate: 16.0, pressure: 0.0, minted: 0.0, debased: 1.0 },
        Answer::Mint { rate: 8.0, pressure: 1728.0, minted: 27648.5, debased: 1.5 },
        Answer::Mint { rate: f64::MAX, pressure: f64::MIN_POSITIVE, minted: 0.1, debased: 1e-300 },
        Answer::Land { town: 0, hexes: vec![], culture: 0.0, next_hex: 0.0, plots: vec![], works: vec![] },
        Answer::Land { town: 3, hexes: vec![hex(0, 0), hex(1, 0), hex(0, 1)], culture: 12.75, next_hex: 30.0,
            plots: vec![count(0, 80.0), count(5, 54.0)], works: work_views() },
        Answer::Land { town: BIGGEST_ID, hexes: (0..19).map(|i| hex(i, -i)).collect(), culture: f64::MAX,
            next_hex: f64::MIN_POSITIVE, plots: counts().remove(2), works: vec![] },
        Answer::Ground { ground: vec![] },
        Answer::Ground { ground: ground_views() },
        Answer::Lockers { town: 0, limit: 0, placed: false, lockers: vec![] },
        Answer::Lockers { town: 1, limit: 4, placed: true, lockers: locker_views() },
        Answer::Lockers { town: BIGGEST_ID, limit: u32::MAX, placed: true, lockers: (0..60).map(|i| LockerView { hand: 1 + i,
            managed: i < 10, client: i < 4, at: None, worth: i as f64 * 0.25, offers: vec![] }).collect() },
        Answer::Routes { town: 0, routes: vec![], markets: vec![], squad: vec![] },
        Answer::Routes { town: 1, routes: route_lines(), markets: market_lines(), squad: vec!["merchant".into(), "librarian".into()] },
        Answer::Routes { town: BIGGEST_ID, routes: vec![], markets: (0..60).map(|i| MarketLine { town: i, desk: None,
            blocks: Some(i * 40), settlement: i % 2 == 0, reachable: i < 30, routed: i % 7 == 0 }).collect(), squad: vec!["x".repeat(300)] },
        Answer::Pallets { town: 0, held: vec![], inbound: vec![], received: 0, lost: 0, lost_goods: vec![] },
        Answer::Pallets { town: 1, held: pallet_views()[..2].to_vec(), inbound: pallet_views()[2..].to_vec(), received: 3, lost: 1,
            lost_goods: vec![count(3, 576.0)] },
        Answer::Pallets { town: BIGGEST_ID, held: (0..30).map(|i| PalletView { id: i, ..pallet_views().remove(1) }).collect(),
            inbound: vec![], received: u64::MAX, lost: u64::MAX, lost_goods: counts().remove(2) },
        Answer::Pallet { id: 0, pallet: None },
        Answer::Pallet { id: 12, pallet: Some(pallet_views().remove(1)) },
        Answer::Pallet { id: u64::MAX, pallet: Some(pallet_views().remove(2)) },
        Answer::Stow { point: point(PointKind::Desk, 27, -60, 27), pallets: vec![] },
        Answer::Stow { point: point(PointKind::Desk, i32::MIN, 0, i32::MAX), pallets: pallet_views() },
        Answer::Manifest { route: 0, lines: vec![], superbill: false },
        Answer::Manifest { route: 3, lines: manifest_lines()[..1].to_vec(), superbill: false },
        Answer::Manifest { route: BIGGEST_ID, lines: manifest_lines(), superbill: true },
        Answer::CashBook { town: 0, name: String::new(), today: 0, lines: vec![], held: vec![], totals: vec![], held_totals: vec![],
            received: Fixed::ZERO, paid: Fixed::ZERO },
        Answer::CashBook { town: 1, name: "Brindle".into(), today: 12, lines: book_lines(), held: book_lines().split_off(1),
            totals: for_totals(), held_totals: for_totals().split_off(1), received: Fixed::from_millis(12_500),
            paid: Fixed::from_millis(3_000) },
        Answer::CashBook { town: BIGGEST_ID, name: "x".repeat(300), today: u64::MAX, lines: vec![BookLine { day: u64::MAX,
            what: "x".repeat(300), amount: Fixed::from_millis(i64::MIN) }], held: vec![], totals: vec![ForTotal {
            what: "x".repeat(300), amount: Fixed::from_millis(i64::MIN) }], held_totals: vec![], received: Fixed::MAX,
            paid: Fixed::MAX },
        Answer::Books { town: 0, name: String::new(), keeps: 0, lines: vec![], whys: vec![] },
        Answer::Books { town: 1, name: "Brindle".into(), keeps: 30, lines: summary_lines(), whys: why_views() },
        Answer::Books { town: BIGGEST_ID, name: "x".repeat(300), keeps: u64::MAX, lines: (0..60).map(|i| SummaryLine {
            kind: "sold".into(), with: format!("town {i}"), good: "wheat".into(), count: i as f64, money: Fixed::MAX }).collect(),
            whys: why_views() },
        Answer::Whys { town: 0, name: String::new(), whys: vec![] },
        Answer::Whys { town: 1, name: "Brindle".into(), whys: why_views() },
        Answer::Whys { town: BIGGEST_ID, name: "x".repeat(300), whys: (0..60).map(|i| WhyView { line: "x".repeat(300),
            times: u64::MAX - i, items: f64::MAX, last: u64::MAX }).collect() },
        Answer::Trust { records: vec![] },
        Answer::Trust { records: trust_lines() },
        Answer::Contracts { contracts: vec![], rents: vec![] },
        Answer::Contracts { contracts: contract_lines(), rents: rent_lines() },
        territory(0),
        territory(1),
        territory(2),
        Answer::War { phase: String::new(), theatre: String::new(), days: 0.0, capital: None, treasury: Fixed::ZERO,
            treasury_spurs: 0.0, minted: 0.0, contracts: 0, delivered: 0.0, serving: 0, waiting: 0, raised: 0, fallen: 0, bills: 0.0,
            rate: 16.0, debasement: 1.0, levies: 0, hired: 0, awarded: 0, depot: vec![], unmet: vec![], towns: vec![],
            issued: vec![], eaten: 0.0, heartiness: 0.0 },
        Answer::War { phase: "war".into(), theatre: "faction".into(), days: 12.5, capital: Some(0), treasury: Fixed::from_millis(500_000),
            treasury_spurs: 8000.0, minted: 1000.0, contracts: 3, delivered: 96.0, serving: 40, waiting: 6, raised: 50, fallen: 4,
            bills: 12.5, rate: 15.8, debasement: 0.97, levies: 2, hired: 1, awarded: 5, depot: name_counts(),
            unmet: vec!["minecraft:iron_ingot".into()], towns: war_towns(), issued: name_counts(), eaten: 900.0, heartiness: 0.4 },
        Answer::War { phase: "x".repeat(300), theatre: "x".repeat(300), days: f64::MAX, capital: Some(BIGGEST_ID),
            treasury: Fixed::MAX, treasury_spurs: f64::MAX, minted: f64::MAX, contracts: u32::MAX, delivered: f64::MAX,
            serving: u32::MAX, waiting: u32::MAX, raised: u32::MAX, fallen: u32::MAX, bills: f64::MAX, rate: f64::MIN_POSITIVE,
            debasement: f64::MIN_POSITIVE, levies: u32::MAX, hired: u32::MAX, awarded: u32::MAX,
            depot: (0..60).map(|i| NameCount { name: "x".repeat(300), count: i as f64 }).collect(), unmet: vec!["x".repeat(300); 40],
            towns: war_towns(), issued: vec![], eaten: f64::MAX, heartiness: 1.0 },
        Answer::RouteBooks { rate: 16.0, routes: vec![] },
        Answer::RouteBooks { rate: 15.5, routes: route_books() },
        Answer::Shops { town: 0, shops: vec![], wanted: vec![], signals: vec![], listings: vec![],
            tally: TallyLine { lots_sold: 0.0, spurs_from_sales: 0.0, lots_bought: 0.0, spurs_on_purchases: 0.0, spurs_from_own_shops: 0.0 } },
        Answer::Shops { town: 1, shops: shop_lines(), wanted: vec![count(0, 20.0), count(3, 2.5)], signals: vec![
            SignalLine { good: 3, count: 20.0, spurs_each: 3.0, why: "a player's order".into() },
            SignalLine { good: BIGGEST_ID, count: f64::MAX, spurs_each: 0.0, why: "x".repeat(300) }],
            listings: vec![ListingLine { good: 2, lot: 16, spurs: 9 }, ListingLine { good: BIGGEST_ID, lot: u32::MAX, spurs: u32::MAX }],
            tally: TallyLine { lots_sold: 2.0, spurs_from_sales: 48.0, lots_bought: 2.0, spurs_on_purchases: 24.0,
                spurs_from_own_shops: 9.0 } },
        Answer::Deeds { town: 0, hub: false, deeds: vec![] },
        Answer::Deeds { town: BIGGEST_ID, hub: true, deeds: deed_lines() },
        Answer::Realms { realms: vec![] },
        Answer::Realms { realms: realm_lines() },
        Answer::Metals { desk: None, phase: "peace".into(), metals: vec![] },
        Answer::Metals { desk: Some(0), phase: "war".into(), metals: metal_lines() },
        Answer::Metals { desk: Some(BIGGEST_ID), phase: "x".repeat(300), metals: vec![MetalLine { good: BIGGEST_ID,
            name: "x".repeat(300), held: f64::MAX, target: f64::MAX, coming: f64::MAX, premium: f64::MIN_POSITIVE }; 12] },
    ]
}

fn realm_lines() -> Vec<RealmLine> {
    vec![
        RealmLine { town: 7, name: "the Coast".into(), trips: 12, partners: 3, sold_spurs: 4_800.0, bought_spurs: 2_300.5,
            customs_spurs: 230.0, made: 1_840.0, in_hand_spurs: 21_400.25 },
        RealmLine { town: BIGGEST_ID, name: "x".repeat(300), trips: u32::MAX, partners: u32::MAX, sold_spurs: f64::MAX,
            bought_spurs: f64::MAX, customs_spurs: f64::MAX, made: f64::MAX, in_hand_spurs: f64::MIN_POSITIVE },
    ]
}

fn metal_lines() -> Vec<MetalLine> {
    vec![
        MetalLine { good: 3, name: "minecraft:iron_ingot".into(), held: 120.0, target: 384.0, coming: 64.0, premium: 1.35 },
        MetalLine { good: 8, name: "minecraft:gold_ingot".into(), held: 0.0, target: 96.0, coming: 0.0, premium: 1.5 },
    ]
}

fn book_lines() -> Vec<BookLine> {
    vec![
        BookLine { day: 11, what: "sold wheat".into(), amount: Fixed::from_millis(12_500) },
        BookLine { day: 12, what: "paid wages".into(), amount: Fixed::from_millis(-3_000) },
        BookLine { day: 12, what: String::new(), amount: Fixed::ZERO },
    ]
}

fn summary_lines() -> Vec<SummaryLine> {
    vec![
        SummaryLine { kind: "sent".into(), with: "Alder".into(), good: "wheat".into(), count: 64.0, money: Fixed::ZERO },
        SummaryLine { kind: "sold".into(), with: "the town".into(), good: "-".into(), count: 0.5, money: Fixed::from_millis(-1) },
    ]
}

fn why_views() -> Vec<WhyView> {
    vec![
        WhyView { line: "Alder | wheat | sent: they're short".into(), times: 9, items: 576.0, last: 11 },
        WhyView { line: String::new(), times: 0, items: 0.0, last: 0 },
    ]
}

fn trust_lines() -> Vec<TrustLine> {
    vec![
        TrustLine { a: 0, b: 1, parties: "Brindle and Alder".into(), trust: 0.5, kept: 3.5, missed: 0.25 },
        TrustLine { a: BIGGEST_ID - 1, b: BIGGEST_ID, parties: "x".repeat(300), trust: f64::MIN_POSITIVE, kept: f64::MAX,
            missed: f64::MAX },
    ]
}

fn contract_lines() -> Vec<ContractLine> {
    vec![
        ContractLine { id: 0, seller: 0, buyer: 1, seller_name: "Brindle".into(), buyer_name: "Alder".into(), state: "active".into(),
            incoterm: "DAP".into(), copies: 2, what: "64 wheat".into(), delivered: 128.0, shipped: 64.0,
            paid: Fixed::from_millis(24_000), advanced: Fixed::ZERO, missed: 1, buyer_missed: 0, trust: 0.6,
            history: vec!["day 1: drafted, 64 wheat 10 time(s) every 2 day(s), DAP".into(), String::new()] },
        ContractLine { id: u32::MAX, seller: BIGGEST_ID, buyer: BIGGEST_ID, seller_name: "x".repeat(300), buyer_name: String::new(),
            state: "broken".into(), incoterm: "EXW".into(), copies: u8::MAX, what: "a levy of 12 soldiers".into(), delivered: f64::MAX,
            shipped: f64::MIN_POSITIVE, paid: Fixed::MAX, advanced: Fixed::from_millis(i64::MIN), missed: u32::MAX,
            buyer_missed: u32::MAX, trust: 1.0, history: vec!["x".repeat(300); 12] },
    ]
}

fn for_totals() -> Vec<ForTotal> {
    vec![
        ForTotal { what: "paid in".into(), amount: Fixed::from_millis(12_500) },
        ForTotal { what: "wages".into(), amount: Fixed::from_millis(-3_000) },
        ForTotal { what: String::new(), amount: Fixed::ZERO },
    ]
}

fn name_counts() -> Vec<NameCount> {
    vec![
        NameCount { name: "minecraft:wheat".into(), count: 4.5 },
        NameCount { name: String::new(), count: 0.0 },
        NameCount { name: "x".repeat(300), count: f64::MAX },
    ]
}

fn project_lines() -> Vec<ProjectLine> {
    vec![
        ProjectLine { what: "bridge on 2,-1".into(), progress: 0.5, done: false, wages: Fixed::from_millis(8_000) },
        ProjectLine { what: "wall".into(), progress: 1.0, done: true, wages: Fixed::MAX },
        ProjectLine { what: String::new(), progress: 0.0, done: false, wages: Fixed::from_millis(i64::MIN) },
    ]
}

fn war_towns() -> Vec<WarTown> {
    vec![
        WarTown { town: 0, name: "Brindle".into(), raised: 12, fallen: 1, deserted: 0, days: 300, hungry: 4, strength: 220.5,
            kit: 32.0, earned: Fixed::from_millis(40_000), trust: 0.7 },
        WarTown { town: BIGGEST_ID, name: "x".repeat(300), raised: u32::MAX, fallen: u32::MAX, deserted: u32::MAX, days: u32::MAX,
            hungry: u32::MAX, strength: f64::MAX, kit: f64::MIN_POSITIVE, earned: Fixed::from_millis(i64::MIN), trust: 1.0 },
    ]
}

fn rent_lines() -> Vec<RentLine> {
    vec![
        RentLine { id: 0, farm: 0, holder: 1, farm_name: "Brindle".into(), holder_name: "Alder".into(), quantity: 16.0,
            crop: "minecraft:wheat".into(), capital: Fixed::from_millis(6_400), state: "active".into(), coupons: 3,
            delivered: 32.0, arrears: 0.0, history: vec!["day 2: bought for 6.4 emeralds, 16 minecraft:wheat a season".into()] },
        RentLine { id: u32::MAX, farm: BIGGEST_ID, holder: BIGGEST_ID, farm_name: "x".repeat(300), holder_name: String::new(),
            quantity: f64::MAX, crop: String::new(), capital: Fixed::MAX, state: "defaulted".into(), coupons: u32::MAX,
            delivered: f64::MAX, arrears: f64::MIN_POSITIVE, history: vec![] },
    ]
}

fn forward_lines() -> Vec<ForwardLine> {
    vec![
        ForwardLine { good: "minecraft:wheat".into(), per_day: 12.5, price: 0.055 },
        ForwardLine { good: "x".repeat(300), per_day: f64::MAX, price: f64::MIN_POSITIVE },
    ]
}

fn route_books() -> Vec<RouteBook> {
    vec![
        RouteBook { route: 0, from: 0, to: 1, from_desk: Some(point(PointKind::Desk, 1002, -60, 1002)), to_desk: None, blocks: 300,
            trips: 4, sold_items: 128.0, sales_spurs: 90.5, service_spurs: 0.0, bought_items: 16.0, purchase_spurs: 12.0,
            food_spurs: 4.5, tolls_spurs: 1.0, home_spurs: 77.0, food_eaten: 80.0, short_rations: 1, delivered: 64.0 },
        RouteBook { route: u32::MAX, from: BIGGEST_ID, to: BIGGEST_ID, from_desk: None,
            to_desk: Some(point(PointKind::Desk, i32::MIN, 0, i32::MAX)), blocks: u32::MAX, trips: u32::MAX, sold_items: f64::MAX,
            sales_spurs: f64::MAX, service_spurs: f64::MIN_POSITIVE, bought_items: f64::MAX, purchase_spurs: f64::MAX,
            food_spurs: f64::MAX, tolls_spurs: f64::MAX, home_spurs: f64::MAX, food_eaten: f64::MAX, short_rations: u32::MAX,
            delivered: f64::MAX },
    ]
}

/// A territory report, empty (0), ordinary (1) or with every number and string at its widest (2).
fn territory(which: u8) -> Answer {
    match which {
        0 => Answer::Territory { town: 0, name: String::new(), rate: 16.0, hexes: vec![], culture: 0.0, next_hex: 0.0, farms: vec![],
            idle: vec![], sold_forward: vec![], wage_per_day: 0.0, labor_share: 0.0, crop_sales_per_day: 0.0, wages_paid: Fixed::ZERO,
            wages_unpaid: Fixed::ZERO, harvested: 0.0, deposits: 0.0, focus: String::new(), purse: Fixed::ZERO, credit: Fixed::ZERO,
            freeholds: 0, tightness: 1.0, treasury: Fixed::ZERO, reserve: 0, mood: 0.0, villagers: 0.0, beds: 0.0, comforts: vec![],
            yield_factor: 1.0, culture_factor: 1.0, bridges: vec![], wall: None, buildings: vec![], hunting: 0.0, armed: 0.0,
            homes_built: 0, born: 0, toll_rate: 0.0, passage_fee: 0.0, toll_income: 0.0, tolls_taken: Fixed::ZERO,
            tolls_owed: Fixed::ZERO, guards: 0, posts: vec![], traffic: 0.0, deserted: 0, militia: 0, trades: 0 },
        1 => Answer::Territory { town: 1, name: "Brindle".into(), rate: 15.5, hexes: vec![hex(0, 0), hex(1, -1)], culture: 31.5,
            next_hex: 40.0, farms: name_counts(), idle: name_counts().split_off(1), sold_forward: forward_lines(), wage_per_day: 2.5,
            labor_share: 0.25, crop_sales_per_day: 6.0, wages_paid: Fixed::from_millis(12_000), wages_unpaid: Fixed::from_millis(500),
            harvested: 640.0, deposits: 3.5, focus: "grow".into(), purse: Fixed::from_millis(20_000), credit: Fixed::from_millis(1_000),
            freeholds: 3, tightness: 1.2, treasury: Fixed::from_millis(150_250), reserve: 64, mood: 1.5, villagers: 9.0, beds: 10.0,
            comforts: name_counts(), yield_factor: 1.15, culture_factor: 1.375, bridges: project_lines(),
            wall: Some(project_lines().remove(0)), buildings: project_lines(), hunting: 0.5, armed: 1.0, homes_built: 2, born: 4,
            toll_rate: 0.05, passage_fee: 2.0, toll_income: 31.0, tolls_taken: Fixed::from_millis(40_000), tolls_owed: Fixed::ZERO,
            guards: 2, posts: vec![hex(2, 0)], traffic: 410.0, deserted: 1, militia: 1, trades: 120 },
        _ => Answer::Territory { town: BIGGEST_ID, name: "x".repeat(300), rate: f64::MAX, hexes: (0..60).map(|i| hex(i, -i)).collect(),
            culture: f64::MAX, next_hex: f64::MAX, farms: name_counts(), idle: name_counts(), sold_forward: forward_lines(),
            wage_per_day: f64::MAX, labor_share: 1.0, crop_sales_per_day: f64::MAX, wages_paid: Fixed::MAX,
            wages_unpaid: Fixed::from_millis(i64::MIN), harvested: f64::MAX, deposits: f64::MAX, focus: "x".repeat(300),
            purse: Fixed::MAX, credit: Fixed::from_millis(i64::MIN), freeholds: u32::MAX, tightness: f64::MIN_POSITIVE,
            treasury: Fixed::from_millis(i64::MIN), reserve: u64::MAX, mood: -5.0, villagers: f64::MAX, beds: f64::MAX,
            comforts: name_counts(), yield_factor: 0.5, culture_factor: 0.25, bridges: project_lines(),
            wall: Some(project_lines().remove(1)), buildings: project_lines(), hunting: 1.0, armed: 1.5, homes_built: u32::MAX,
            born: u32::MAX, toll_rate: 0.25, passage_fee: f64::MAX, toll_income: f64::MAX, tolls_taken: Fixed::MAX,
            tolls_owed: Fixed::MAX, guards: u32::MAX, posts: (0..40).map(|i| hex(-i, i)).collect(), traffic: f64::MAX,
            deserted: u32::MAX, militia: u32::MAX, trades: u64::MAX },
    }
}

fn wagon_lines() -> Vec<WagonLine> {
    vec![
        WagonLine { out: true, arrives: 2000, crew: vec![2, 5], stacks: 27, porter: false },
        WagonLine { out: false, arrives: 0, crew: vec![], stacks: 0, porter: true },
        WagonLine { out: false, arrives: u64::MAX, crew: (0..40).map(|i| u32::MAX - i).collect(), stacks: u32::MAX, porter: false },
    ]
}

fn route_lines() -> Vec<RouteLine> {
    vec![
        RouteLine { route: 0, from: 1, to: 2, blocks: 40, days: 1.0 / 30.0, every_days: 4.0, open: true, on_call: false,
            trips: 3, sold: 12.5, wagon: wagon_lines().into_iter().next(), status: "on the way out".into() },
        RouteLine { route: 1, from: 0, to: 1, blocks: 0, days: 0.0, every_days: 0.0, open: false, on_call: true, trips: 0,
            sold: 0.0, wagon: None, status: String::new() },
        RouteLine { route: BIGGEST_ID, from: BIGGEST_ID, to: BIGGEST_ID, blocks: u32::MAX, days: f64::MAX,
            every_days: f64::MIN_POSITIVE, open: true, on_call: false, trips: u32::MAX, sold: f64::MAX,
            wagon: wagon_lines().pop(), status: "home: no trip: ".to_string() + &"x".repeat(300) },
    ]
}

fn market_lines() -> Vec<MarketLine> {
    vec![
        MarketLine { town: 2, desk: Some(point(PointKind::Desk, 67, -60, 27)), blocks: Some(40), settlement: true, reachable: true, routed: true },
        MarketLine { town: 0, desk: None, blocks: None, settlement: false, reachable: false, routed: false },
        MarketLine { town: BIGGEST_ID, desk: Some(point(PointKind::Desk, i32::MIN, i32::MAX, i32::MIN)), blocks: Some(u32::MAX),
            settlement: false, reachable: false, routed: true },
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
    point_kind_fixtures: PointKind = vec![PointKind::Desk, PointKind::Locker, PointKind::Shop, PointKind::Banker, PointKind::Depot, PointKind::Market];
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
    locker_view_fixtures: LockerView = locker_views();
    offer_view_fixtures: OfferView = offer_views();
    resident_fixtures: Resident = residents();
    good_flag_fixtures: GoodFlag = vec![GoodFlag::Crop, GoodFlag::Meat];
    save_file_fixtures: SaveFile = saves();
    fixed_fixtures: Fixed = fixeds();
    hex_at_fixtures: HexAt = vec![hex(0, 0), hex(-1, 1), hex(i32::MIN, i32::MAX)];
    land_kind_fixtures: LandKind = LANDS.to_vec();
    route_line_fixtures: RouteLine = route_lines();
    wagon_line_fixtures: WagonLine = wagon_lines();
    market_line_fixtures: MarketLine = market_lines();
    book_line_fixtures: BookLine = book_lines();
    summary_line_fixtures: SummaryLine = summary_lines();
    why_view_fixtures: WhyView = why_views();
    contract_line_fixtures: ContractLine = contract_lines();
    trust_line_fixtures: TrustLine = trust_lines();
    for_total_fixtures: ForTotal = for_totals();
    name_count_fixtures: NameCount = name_counts();
    project_line_fixtures: ProjectLine = project_lines();
    war_town_fixtures: WarTown = war_towns();
    rent_line_fixtures: RentLine = rent_lines();
    forward_line_fixtures: ForwardLine = forward_lines();
    route_book_fixtures: RouteBook = route_books();
    work_fixtures: Work = works();
    work_view_fixtures: WorkView = work_views();
    ground_view_fixtures: GroundView = ground_views();
    pack_package_fixtures: PackPackage = pack_packages();
    pallet_view_fixtures: PalletView = pallet_views();
    manifest_line_fixtures: ManifestLine = manifest_lines();
    vendor_read_fixtures: VendorRead = vendor_reads();
    vendor_mode_fixtures: VendorMode = vec![VendorMode::Idle, VendorMode::Buy, VendorMode::Sell];
    shop_role_fixtures: ShopRole = vec![ShopRole::Order, ShopRole::Stall, ShopRole::Own, ShopRole::Board, ShopRole::Deed, ShopRole::Idle];
    shop_line_fixtures: ShopLine = shop_lines();
    realm_line_fixtures: RealmLine = realm_lines();
    metal_line_fixtures: MetalLine = metal_lines();
    deed_line_fixtures: DeedLine = deed_lines();
    listing_line_fixtures: ListingLine = vec![ListingLine { good: 2, lot: 16, spurs: 9 },
        ListingLine { good: BIGGEST_ID, lot: u32::MAX, spurs: u32::MAX }];
    signal_line_fixtures: SignalLine = vec![SignalLine { good: 3, count: 20.0, spurs_each: 3.0, why: "a player's order".into() },
        SignalLine { good: BIGGEST_ID, count: f64::MAX, spurs_each: 0.0, why: "x".repeat(300) }];
    tally_line_fixtures: TallyLine = vec![
        TallyLine { lots_sold: 0.0, spurs_from_sales: 0.0, lots_bought: 0.0, spurs_on_purchases: 0.0, spurs_from_own_shops: 0.0 },
        TallyLine { lots_sold: 2.0, spurs_from_sales: 48.0, lots_bought: 2.0, spurs_on_purchases: 24.0, spurs_from_own_shops: 9.0 }];
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
        Input::Unmoved { .. } => 19,
        Input::Cashcount { .. } => 20,
        Input::CashUnmoved { .. } => 21,
        Input::Ground { .. } => 22,
        Input::Crops { .. } => 23,
        Input::Survey { .. } => 24,
        Input::Built { .. } => 25,
        Input::Switch { .. } => 26,
        Input::Locker { .. } => 27,
        Input::Route { .. } => 28,
        Input::Pack { .. } => 29,
        Input::Stowed { .. } => 30,
        Input::Vendor { .. } => 31,
        Input::Board { .. } => 32,
        Input::DeedShop { .. } => 33,
        Input::DeedBrought { .. } => 34,
        Input::OffMap { .. } => 35,
        Input::Capital { .. } => 36,
        Input::Mobilise { .. } => 37,
        Input::Declare { .. } => 38,
        Input::Peace => 39,
        Input::CrownShop { .. } => 40,
        Input::Port { .. } => 41,
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
        Output::Moved { .. } => 13,
        Output::Cash { .. } => 14,
        Output::Build { .. } => 15,
        Output::HandAway { .. } => 16,
        Output::HandBack { .. } => 17,
        Output::StallBought { .. } => 18,
        Output::Shelf { .. } => 19,
        Output::Deed { .. } => 20,
        Output::DeedPaid { .. } => 21,
        Output::BuildShop { .. } => 22,
        Output::WarDeclared { .. } => 23,
        Output::PeaceMade { .. } => 24,
        Output::BuildCrownShop { .. } => 25,
        Output::CrownShop { .. } => 26,
        Output::CrownShopDown { .. } => 27,
        Output::Found { .. } => 28,
        Output::Trail { .. } => 29,
        Output::Stall { .. } => 30,
        Output::Explore { .. } => 31,
    }
}

fn query_place(q: &Query) -> usize {
    match q {
        Query::Stock { .. } => 0,
        Query::Points => 1,
        Query::Goods => 2,
        Query::Dials => 3,
        Query::Hands { .. } => 4,
        Query::Chests { .. } => 5,
        Query::Cash { .. } => 6,
        Query::Mint => 7,
        Query::Land { .. } => 8,
        Query::Ground { .. } => 9,
        Query::Lockers { .. } => 10,
        Query::Routes { .. } => 11,
        Query::Pallets { .. } => 12,
        Query::Pallet { .. } => 13,
        Query::Stow { .. } => 14,
        Query::Manifest { .. } => 15,
        Query::CashBook { .. } => 16,
        Query::Books { .. } => 17,
        Query::Whys { .. } => 18,
        Query::Trust => 19,
        Query::Contracts { .. } => 20,
        Query::Territory { .. } => 21,
        Query::War => 22,
        Query::RouteBooks => 23,
        Query::Shops { .. } => 24,
        Query::Deeds { .. } => 25,
        Query::Realms => 26,
        Query::Metals => 27,
    }
}

fn answer_place(a: &Answer) -> usize {
    match a {
        Answer::Stock { .. } => 0,
        Answer::Points { .. } => 1,
        Answer::Goods { .. } => 2,
        Answer::Dials { .. } => 3,
        Answer::Hands { .. } => 4,
        Answer::Chests { .. } => 5,
        Answer::Cash { .. } => 6,
        Answer::Mint { .. } => 7,
        Answer::Land { .. } => 8,
        Answer::Ground { .. } => 9,
        Answer::Lockers { .. } => 10,
        Answer::Routes { .. } => 11,
        Answer::Pallets { .. } => 12,
        Answer::Pallet { .. } => 13,
        Answer::Stow { .. } => 14,
        Answer::Manifest { .. } => 15,
        Answer::CashBook { .. } => 16,
        Answer::Books { .. } => 17,
        Answer::Whys { .. } => 18,
        Answer::Trust { .. } => 19,
        Answer::Contracts { .. } => 20,
        Answer::Territory { .. } => 21,
        Answer::War { .. } => 22,
        Answer::RouteBooks { .. } => 23,
        Answer::Shops { .. } => 24,
        Answer::Deeds { .. } => 25,
        Answer::Realms { .. } => 26,
        Answer::Metals { .. } => 27,
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
    covers("Input", &inputs(), 42, input_place);
    covers("Output", &outputs(), 32, output_place);
    covers("Query", &queries(), 28, query_place);
    covers("Answer", &answers(), 28, answer_place);
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
    for (i, k) in [PointKind::Desk, PointKind::Locker, PointKind::Shop, PointKind::Banker, PointKind::Depot, PointKind::Market].iter().enumerate() {
        assert_eq!(postcard::to_allocvec(k).unwrap(), vec![i as u8], "{k:?}");
    }
    for (i, p) in [Place::Town { town: 0 }, Place::Custody { town: 0 }, Place::Wagon { route: 0 }, Place::Container { container: 0 }]
        .iter().enumerate() {
        assert_eq!(postcard::to_allocvec(p).unwrap()[0] as usize, i, "{p:?}");
    }
    for (i, f) in [GoodFlag::Crop, GoodFlag::Meat].iter().enumerate() {
        assert_eq!(postcard::to_allocvec(f).unwrap(), vec![i as u8], "{f:?}");
    }
    for (i, l) in LANDS.iter().enumerate() {
        assert_eq!(postcard::to_allocvec(l).unwrap(), vec![i as u8], "{l:?}");
    }
    for (w, i) in works().iter().zip([0, 0, 1, 2, 2, 3, 3]) {
        assert_eq!(postcard::to_allocvec(w).unwrap()[0], i, "{w:?}");
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
