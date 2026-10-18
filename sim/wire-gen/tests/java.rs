//! The generated Java against the core's own bytes (`just wire-javac`; ignored by `cargo test`, as it runs a JDK).
//!
//! First the generated folder compiles alone (`javac --release 21`, every lint an error), as `chicory-host`'s Gradle
//! build will compile it. Then `tests/java/WireCheck.java` reads what Rust writes here: every wire type's samples at
//! their edges (0 and the largest numbers, NaN, -0.0, the empty and the long, multi-byte UTF-8, a length's varint at
//! its boundaries) must decode and encode again to the same bytes; bytes postcard refuses (cut short, a varint too
//! long or past its range, an option tag of 2, an unknown variant, a string not UTF-8, a length past the bytes left)
//! must throw; and every number type the codec has, with its bytes from postcard, both ways.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command as Run;

use enterprise_core::wire::*;
use enterprise_core::Fixed;
use serde::de::DeserializeOwned;
use serde::Serialize;

fn hex(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "-".to_string();
    }
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The lines WireCheck reads.
#[derive(Default)]
struct Samples(String);

impl Samples {
    /// A value that must cross: Java decodes its bytes and encodes them again, the same.
    fn ok<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(&mut self, ty: &str, value: &T) {
        let bytes = postcard::to_allocvec(value).unwrap();
        // Read back to the same bytes (not compared as values: NaN isn't equal to itself in Rust).
        assert_eq!(postcard::to_allocvec(&postcard::from_bytes::<T>(&bytes).unwrap()).unwrap(), bytes);
        writeln!(self.0, "OK {ty} {}", hex(&bytes)).unwrap();
    }

    /// Bytes postcard refuses as a `T`: Java must refuse them too.
    fn bad<T: DeserializeOwned + std::fmt::Debug>(&mut self, ty: &str, bytes: &[u8]) {
        assert!(postcard::from_bytes::<T>(bytes).is_err(), "postcard read {ty} from {}", hex(bytes));
        writeln!(self.0, "BAD {ty} {}", hex(bytes)).unwrap();
    }

    /// A whole `T` and a byte more: postcard's `from_bytes` ignores what's left, Java's `fromBytes` refuses it.
    fn trailing<T: DeserializeOwned + std::fmt::Debug>(&mut self, ty: &str, bytes: &[u8]) {
        let (_, rest) = postcard::take_from_bytes::<T>(bytes).unwrap();
        assert!(!rest.is_empty());
        writeln!(self.0, "BAD {ty} {}", hex(bytes)).unwrap();
    }

    fn prim<T: Serialize>(&mut self, kind: &str, value: &str, v: T) {
        writeln!(self.0, "PRIM {kind} {value} {}", hex(&postcard::to_allocvec(&v).unwrap())).unwrap();
    }

    fn bad_prim<T: DeserializeOwned + std::fmt::Debug>(&mut self, kind: &str, bytes: &[u8]) {
        assert!(postcard::from_bytes::<T>(bytes).is_err(), "postcard read a {kind} from {}", hex(bytes));
        writeln!(self.0, "BADPRIM {kind} {}", hex(bytes)).unwrap();
    }
}

fn point(kind: PointKind, dim: &str, x: i32, y: i32, z: i32) -> Point {
    Point { kind, dim: dim.to_string(), x, y, z }
}

fn pack() -> ContentPack {
    let mut wheat = PackGood::new("minecraft:wheat", 0.05, 64);
    wheat.flags = vec![GoodFlag::Crop, GoodFlag::Meat];
    let mut book = PackGood::new("minecraft:book", 0.5, u32::MAX);
    book.comfort = Some("reading".into());
    book.food = f64::MIN_POSITIVE;
    book.saturation = 5e-324;
    ContentPack { day_ticks: u64::MAX, goods: vec![wheat, book] }
}

fn inputs() -> Vec<Input> {
    let desk = point(PointKind::Desk, "minecraft:overworld", i32::MIN, -64, i32::MAX);
    vec![
        Input::Advance { to: 0 },
        Input::Advance { to: u64::MAX },
        Input::Advance { to: 1 << 63 },
        Input::Deliver { town: usize::MAX, good: 0, count: -0.0 },
        Input::Deliver { town: 127, good: 128, count: f64::INFINITY },
        Input::Pay { town: 0, money: Fixed::from_millis(i64::MIN) },
        Input::Pay { town: 0, money: Fixed::MAX },
        Input::Pay { town: 0, money: Fixed::from_millis(-1) },
        Input::Order { town: 1, good: 2, lot: 16.0, price: 0.1, purse: f64::MAX },
        Input::Stall { town: 1, good: 2, lot: 1.0, price: f64::MIN_POSITIVE, stock: 1e300 },
        Input::OwnShop { town: 16_383 },
        Input::Buy { town: 16_384, good: 0, count: 3.5 },
        Input::AddTown { name: String::new(), desk: desk.clone() },
        Input::AddTown { name: "Ünïcødé 町 🐝".into(), desk: desk.clone() },
        Input::AddTown { name: "x".repeat(300), desk: desk.clone() },
        Input::AddPoint { point: point(PointKind::Depot, "a", 0, 0, 0), town: 3 },
        Input::RemovePoint { point: point(PointKind::Banker, "minecraft:the_nether", -1, 1, -1) },
        Input::Stocktake { point: desk.clone(), counts: vec![] },
        Input::Stocktake {
            point: desk,
            counts: (0..1000).map(|i| Count { good: i, count: i as f64 * 0.5 }).collect(),
        },
    ]
}

fn outputs() -> Vec<Output> {
    vec![
        Output::WagonLeft { route: 0, stacks: u32::MAX, worth: 12.5 },
        Output::WagonSold { route: 1, sold: 64.0, takings: Fixed::from_millis(-5) },
        Output::WagonHome { route: usize::MAX, takings: Fixed::ZERO },
        Output::OrderFilled { town: 2, order: 3, good: 4, lots: 0, count: 1.0, spurs: 0.25 },
        Output::Signal { town: 2, good: 1, count: 9.0, spurs_each: f64::NAN },
    ]
}

fn refusals() -> Vec<Refusal> {
    vec![
        Refusal::Unreadable { why: "garbled".into() },
        Refusal::Abi { core: 1, request: u32::MAX },
        Refusal::Schema { core: u64::MAX, request: 0 },
        Refusal::NoSociety,
        Refusal::Pack { why: "no goods".into() },
        Refusal::SaveFormat { core: 1, save: 2 },
        Refusal::Save { why: String::new() },
        Refusal::Input { at: 7, why: "no town 9".into() },
        Refusal::Query { at: 0, why: "no town 9".into() },
    ]
}

fn save(len: usize) -> SaveFile {
    SaveFile { format: 1, schema: SCHEMA, core: CORE.to_string(), blob: (0..len).map(|i| (i * 7) as u8).collect() }
}

fn samples() -> Samples {
    let mut s = Samples::default();
    for kind in [PointKind::Desk, PointKind::Locker, PointKind::Shop, PointKind::Banker, PointKind::Depot] {
        s.ok("PointKind", &kind);
    }
    s.ok("GoodFlag", &GoodFlag::Crop);
    s.ok("GoodFlag", &GoodFlag::Meat);
    s.ok("Point", &point(PointKind::Shop, "", 0, 0, 0));
    s.ok("Count", &Count { good: 0, count: 0.0 });
    s.ok("TownPoint", &TownPoint { point: point(PointKind::Locker, "d", 1, 2, 3), town: 5 });
    for g in &pack().goods {
        s.ok("PackGood", g);
    }
    s.ok("ContentPack", &pack());
    s.ok("ContentPack", &ContentPack { day_ticks: 24_000, goods: vec![] });
    for len in [0, 1, 127, 128, 16_383, 16_384, 70_000] {
        s.ok("SaveFile", &save(len));
    }
    for input in inputs() {
        s.ok("Input", &input);
    }
    for output in outputs() {
        s.ok("Output", &output);
    }
    for why in refusals() {
        s.ok("Refusal", &why.clone());
        s.ok("Response", &Response::Refused { why });
    }
    s.ok("Query", &Query::Stock { town: 0 });
    s.ok("Query", &Query::Points);
    let answers = vec![
        Answer::Stock { town: 3, stock: (0..500).map(|i| i as f64 / 3.0).collect() },
        Answer::Stock { town: 0, stock: vec![] },
        Answer::Points { points: vec![TownPoint { point: point(PointKind::Desk, "o", 1, 2, 3), town: 0 }] },
    ];
    for a in &answers {
        s.ok("Answer", a);
    }
    s.ok("Response", &Response::Done { outputs: outputs(), answers: answers.clone() });
    s.ok("Response", &Response::Done { outputs: vec![], answers: vec![] });
    s.ok("Response", &Response::Saved { save: save(300) });
    let commands = vec![
        Command::Create { pack: pack(), seed: u64::MAX },
        Command::Load { save: save(129) },
        Command::Save,
        Command::Apply { inputs: inputs() },
        Command::Apply { inputs: vec![] },
        Command::Ask { queries: vec![Query::Points, Query::Stock { town: 1 << 40 }] },
    ];
    for c in &commands {
        s.ok("Command", c);
        s.ok("Request", &Request::new(c.clone()));
    }
    s.ok("Request", &Request { abi: u32::MAX, schema: 0, command: Command::Save });

    // Refused by postcard, so by Java: every cut of a request.
    let whole = postcard::to_allocvec(&Request::new(Command::Ask { queries: vec![Query::Stock { town: 300 }] })).unwrap();
    for cut in 0..whole.len() {
        s.bad::<Request>("Request", &whole[..cut]);
    }
    let mut more = whole.clone();
    more.push(0);
    s.trailing::<Request>("Request", &more);
    // An unknown variant.
    s.bad::<Input>("Input", &[11]);
    s.bad::<PointKind>("PointKind", &[5]);
    s.bad::<Command>("Command", &[0x80, 0x01]);
    s.bad::<Command>("Command", &[0xff, 0xff, 0xff, 0xff, 0x0f]);
    // A u64 varint of eleven bytes, and one whose tenth byte holds more than its last bit.
    let mut long = vec![0];
    long.extend([0x80; 10]);
    long.push(0x00);
    s.bad::<Input>("Input", &long);
    let mut past = vec![0];
    past.extend([0xff; 9]);
    past.push(0x02);
    s.bad::<Input>("Input", &past);
    let mut max = vec![0];
    max.extend([0xff; 9]);
    max.push(0x01);
    s.ok("Input", &postcard::from_bytes::<Input>(&max).unwrap());
    // A u32 (the ABI) of six bytes, and one past its range in its fifth.
    s.bad::<Request>("Request", &[0x80, 0x80, 0x80, 0x80, 0x80, 0x00, 1, 2]);
    s.bad::<Request>("Request", &[0xff, 0xff, 0xff, 0xff, 0x10, 1, 2]);
    // An option's tag of 2 (a good's comfort).
    let mut good = postcard::to_allocvec(&PackGood::new("a", 1.0, 1)).unwrap();
    let tag = 1 + 1 + 8 + 1 + 8 + 8;
    assert_eq!(good[tag], 0);
    good[tag] = 2;
    s.bad::<PackGood>("PackGood", &good);
    // A town's name that isn't UTF-8: a lone continuation byte, and an overlong encoding.
    for name in [vec![0x80], vec![0xc0, 0xaf], vec![0xed, 0xa0, 0x80]] {
        let mut bytes = vec![7, name.len() as u8];
        bytes.extend(name);
        bytes.extend(postcard::to_allocvec(&point(PointKind::Desk, "o", 0, 0, 0)).unwrap());
        s.bad::<Input>("Input", &bytes);
    }
    // A list's length past the bytes left.
    s.bad::<Answer>("Answer", &[0, 0, 0xff, 0xff, 0xff, 0xff, 0x07]);
    s.bad::<Answer>("Answer", &[0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0]);

    // Every number type, with postcard's bytes.
    for v in [false, true] {
        s.prim("bool", &v.to_string(), v);
    }
    for v in [0u8, 1, 127, 128, 255] {
        s.prim("u8", &v.to_string(), v);
    }
    for v in [0i8, 1, -1, i8::MIN, i8::MAX] {
        s.prim("i8", &v.to_string(), v);
    }
    for v in [0u16, 127, 128, 16_383, 16_384, u16::MAX] {
        s.prim("u16", &v.to_string(), v);
    }
    for v in [0i16, 1, -1, 63, -64, 64, i16::MIN, i16::MAX] {
        s.prim("i16", &v.to_string(), v);
    }
    for v in [0u32, 127, 128, (1 << 28) - 1, 1 << 28, u32::MAX] {
        s.prim("u32", &v.to_string(), v);
    }
    for v in [0i32, 1, -1, i32::MIN, i32::MAX, -1_000_000] {
        s.prim("i32", &v.to_string(), v);
    }
    for v in [0u64, 127, 128, 1 << 56, (1 << 63) - 1, 1 << 63, u64::MAX] {
        s.prim("u64", &v.to_string(), v);
    }
    for v in [0i64, 1, -1, i64::MIN, i64::MAX, -1_000_000_000_000] {
        s.prim("i64", &v.to_string(), v);
    }
    for v in [0.0f32, -0.0, 1.5, f32::NAN, f32::INFINITY, f32::MIN_POSITIVE, f32::from_bits(1)] {
        s.prim("f32", &format!("{:08x}", v.to_bits()), v);
    }
    for v in [0.0f64, -0.0, 0.1, f64::NAN, f64::NEG_INFINITY, f64::MAX, f64::from_bits(1), f64::from_bits(0x7ff8_dead_beef_0001)] {
        s.prim("f64", &format!("{:016x}", v.to_bits()), v);
    }
    for v in ["", "a", "Ünïcødé", "町", "🐝", &"y".repeat(200)] {
        s.prim("string", &hex(v.as_bytes()), v.to_string());
    }
    s.bad_prim::<bool>("bool", &[2]);
    s.bad_prim::<u16>("u16", &[0xff, 0xff, 0x04]);
    s.bad_prim::<u16>("u16", &[0x80, 0x80, 0x80, 0x00]);
    s.bad_prim::<i16>("i16", &[0xff, 0xff, 0x04]);
    s.bad_prim::<u32>("u32", &[0xff, 0xff, 0xff, 0xff, 0x10]);
    s.bad_prim::<i32>("i32", &[0xff, 0xff, 0xff, 0xff, 0x10]);
    s.bad_prim::<u64>("u64", &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x02]);
    s.bad_prim::<i64>("i64", &[0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x00]);
    s.bad_prim::<f64>("f64", &[0, 0, 0, 0, 0, 0, 0]);
    s.bad_prim::<f32>("f32", &[0, 0, 0]);
    s.bad_prim::<String>("string", &[3, b'a', b'b']);
    s.bad_prim::<String>("string", &[1, 0xff]);
    s
}

fn run(cmd: &mut Run) -> String {
    let out = cmd.output().unwrap_or_else(|e| panic!("running {cmd:?}: {e} (a JDK on the PATH?)"));
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "{cmd:?} failed:\n{text}");
    text
}

fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("enterprise-wire-javac-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn java_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "java")).collect();
    files.sort();
    files
}

#[test]
#[ignore = "runs javac and java: `just wire-javac`"]
fn the_generated_java_compiles_alone_and_reads_and_writes_the_cores_bytes() {
    let root = enterprise_wire_gen::repo_root();
    let generated = java_files(&root.join(enterprise_wire_gen::JAVA_DIR));
    assert!(!generated.is_empty(), "no generated Java: `just wire-gen`");
    let dir = scratch();
    let classes = dir.join("classes");
    // The generated folder alone, as Gradle will compile it.
    let out = run(Run::new("javac").args(["--release", "21", "-Xlint:all", "-Werror", "-d"]).arg(&classes)
        .args(&generated));
    println!("javac: {} generated files compile alone{}", generated.len(),
        if out.trim().is_empty() { String::new() } else { format!(":\n{out}") });
    // The check, on the core's bytes.
    let harness = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/java/WireCheck.java");
    run(Run::new("javac").args(["--release", "21", "-Xlint:all", "-Werror", "-cp"]).arg(&classes).arg("-d")
        .arg(&classes).arg(&harness));
    let file = dir.join("samples.txt");
    std::fs::write(&file, samples().0).unwrap();
    let out = run(Run::new("java").arg("-cp").arg(&classes).arg("WireCheck").arg(&file));
    print!("{out}");
    assert!(out.contains("WireCheck: all"), "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}
