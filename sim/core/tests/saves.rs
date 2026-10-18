//! Old saves and the way they are brought forward (`save/migrate/`, `docs/binding-plan.md` 1.4):
//!
//! - the template: a hand-made format 0 (a field renamed and moved) read through a chain of one step, to be copied
//!   when the first real migration lands;
//! - a save of a newer format, or of one the chain doesn't reach, is refused and nothing is guessed;
//! - the golden saves in `tests/saves/`: each `vN.blob` is a real save of format N (written by the core of that
//!   format), and must load in today's core and run on to the digest committed beside it (`vN.digest`). None yet: the
//!   first lands with the first format bump after the first release (`save/migrate/mod.rs` says how).
//!
//! `SAVES=blob cargo test --release --test saves` writes `saves/vN.blob` for today's format from a fixed world (run it
//! before the bump); `SAVES=digest` writes every golden's digest (after a migration lands, and whenever the rules
//! move, as `replay_digest.txt` is).

mod common;

use common::{world, DAY};
use enterprise_core::save::{self, migrate, Resources, SavedRoute, SavedTown};
use enterprise_core::wire::{handle, Command, Refusal, Request, Response, SaveFile, ABI, CORE, SCHEMA};
use enterprise_core::Society;
use serde::{Deserialize, Serialize};

/// A blob's header, magic then format, then `payload`.
fn blob_of(format: u32, payload: &[u8]) -> Vec<u8> {
    let mut b = save::MAGIC.to_vec();
    b.extend_from_slice(&format.to_le_bytes());
    b.extend_from_slice(payload);
    b
}

/// Format 0, as if the core had once saved: the same parts, a town list that was called `settlements` and came first.
/// (Postcard has no names, so a rename is a move in the bytes; a frozen type keeps the old order.) The template for a
/// `save/migrate/vN.rs`: only what changed is copied, here the container; the parts are today's.
mod v0 {
    use super::*;

    #[derive(Serialize, Deserialize)]
    pub struct Saved0 {
        pub settlements: Vec<SavedTown>,
        pub routes: Vec<SavedRoute>,
        pub resources: Resources,
    }

    /// The step from format 0 to 1: read as it was, written as it is.
    pub fn run(payload: &[u8]) -> Result<Vec<u8>, String> {
        let old: Saved0 = postcard::from_bytes(payload).map_err(|e| e.to_string())?;
        let now = save::Saved { resources: old.resources, towns: old.settlements, routes: old.routes };
        postcard::to_allocvec(&now).map_err(|e| e.to_string())
    }

    /// A save as format 0 wrote it (the test's stand-in for a golden file).
    pub fn payload_of(s: &mut Society) -> Vec<u8> {
        let now = save::saved(&mut s.world);
        postcard::to_allocvec(&Saved0 { settlements: now.towns, routes: now.routes, resources: now.resources }).unwrap()
    }
}

/// Everything a host can see of a society, exactly.
fn seen(s: &Society) -> String {
    let towns: Vec<String> = (0..s.town_count()).map(|t| format!("{:?}", s.town(t))).collect();
    let routes: Vec<String> = (0..s.route_count()).map(|r| format!("{:?}", s.route(r))).collect();
    format!("{towns:?}\n{routes:?}\n{:?}\n{:?}", s.contracts(), s.bills())
}

fn fnv(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h = (h ^ *b as u64).wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

#[test]
fn a_hand_made_format_0_is_brought_forward_by_a_chain_of_one_step() {
    let chain = [migrate::Step { from: 0, run: v0::run }];
    assert!(migrate::is_whole(&chain, 0, 1), "one step, from 0, ends at 1");
    common::each_seed([3, 42, 99], |seed| {
        let mut kept = world(seed);
        kept.advance(40 * DAY);
        let old = v0::payload_of(&mut kept);
        assert_ne!(old, postcard::to_allocvec(&save::saved(&mut kept.world)).unwrap(), "seed {seed}: format 0 is another shape");

        // The chain brings it to today's payload, byte for byte what today's core would have saved.
        let payload = migrate::upgrade_with(&chain, 0, 1, 0, &old).expect("the chain runs");
        assert_eq!(blob_of(1, &payload), kept.save(), "seed {seed}: the same blob as never having been format 0");
        // The load path does the same from the whole old blob: header off, payload through the chain.
        let through_load = save::upgraded_with(&chain, 0, 1, &blob_of(0, &old)).expect("an old blob loads");
        assert_eq!(postcard::to_allocvec(&through_load).unwrap(), payload, "seed {seed}: upgraded() reads the chain's payload");
        // ... and the society it loads runs on as the one that was never old.
        let mut loaded = Society::load(&blob_of(1, &payload)).unwrap();
        assert_eq!(seen(&loaded), seen(&kept), "seed {seed}: loaded as saved");
        for day in 41..=60 {
            assert_eq!(kept.advance(day * DAY), loaded.advance(day * DAY), "seed {seed}, day {day}");
        }
        assert_eq!(seen(&loaded), seen(&kept), "seed {seed}: the same world at day 60");

        // A broken old payload is a broken save, not a panic and not a guess.
        let cut = &old[..old.len() / 2];
        assert!(matches!(migrate::upgrade_with(&chain, 0, 1, 0, cut), Err(migrate::Unreadable::Broken { from: 0, .. })));
    });
}

#[test]
fn this_core_reads_its_own_format_and_refuses_the_ones_it_cant_bring_forward() {
    let mut s = world(7);
    s.advance(10 * DAY);
    let blob = s.save();
    assert_eq!(save::format_of(&blob), Ok(save::FORMAT));
    assert!(Society::load(&blob).is_ok());
    assert!(migrate::reads(save::FORMAT));

    // A newer core's save: refused with the formats in the words, not misread as ours.
    let payload = &blob[8..];
    let newer = Society::load(&blob_of(save::FORMAT + 1, payload)).err().expect("a newer format is refused");
    assert!(newer.contains(&format!("format {}", save::FORMAT + 1)) && newer.contains("newer"), "{newer}");
    // Older than the chain goes (there is no format 0 step): refused too.
    let older = Society::load(&blob_of(0, payload)).err().expect("a format with no migration is refused");
    assert!(older.contains("format 0") && older.contains("older"), "{older}");
    assert!(!migrate::reads(0) && !migrate::reads(save::FORMAT + 1) && !migrate::reads(u32::MAX));
}

#[test]
fn a_load_of_another_format_is_refused_by_its_numbers_and_the_society_kept() {
    let mut s = Some(world(5));
    s.as_mut().unwrap().advance(5 * DAY);
    let before = seen(s.as_ref().unwrap());
    let Response::Saved { save: file } = handle(&mut s, Request::new(Command::Save)) else { panic!("saved") };
    assert_eq!((file.format, file.schema, file.core.as_str()), (save::FORMAT, SCHEMA, CORE));
    let load = |s: &mut Option<Society>, save: SaveFile| match handle(s, Request::new(Command::Load { save })) {
        Response::Refused { why } => why,
        r => panic!("not refused: {r:?}"),
    };
    for format in [0, save::FORMAT + 1, u32::MAX] {
        let why = load(&mut s, SaveFile { format, ..file.clone() });
        assert_eq!(why, Refusal::SaveFormat { core: save::FORMAT, save: format });
    }
    // A save filed as one format whose blob says another is a broken save.
    let why = load(&mut s, SaveFile { blob: blob_of(save::FORMAT + 1, &file.blob[8..]), ..file.clone() });
    assert!(matches!(why, Refusal::Save { .. }), "{why:?}");
    assert_eq!(seen(s.as_ref().unwrap()), before, "the society there is kept");
    // And a request built against another ABI or schema is refused before it is looked at, with a save in it too.
    let command = Command::Load { save: file };
    assert_eq!(match handle(&mut s, Request { abi: ABI, schema: SCHEMA ^ 1, command: command.clone() }) {
        Response::Refused { why } => why,
        r => panic!("{r:?}"),
    }, Refusal::Schema { core: SCHEMA, request: SCHEMA ^ 1 });
    assert_eq!(seen(s.as_ref().unwrap()), before);
}

// ---- the golden saves ----

fn saves_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/saves")
}

/// The days a golden save is run on before its digest is taken.
const RUN_ON_DAYS: u64 = 30;

/// What a golden save comes to: loaded, run on [`RUN_ON_DAYS`] days, everything a host can see, and the blob it saves.
fn digest_of(blob: &[u8]) -> Result<String, String> {
    let mut s = Society::load(blob)?;
    let day = s.now() / DAY;
    for d in day + 1..=day + RUN_ON_DAYS {
        s.advance(d * DAY);
    }
    Ok(format!("{} {}\n", fnv(seen(&s).as_bytes()), fnv(&s.save())))
}

/// Every golden save: its format (from its name) and path.
fn goldens() -> Vec<(u32, std::path::PathBuf)> {
    let mut all: Vec<(u32, std::path::PathBuf)> = std::fs::read_dir(saves_dir()).unwrap().filter_map(|e| {
        let path = e.ok()?.path();
        let name = path.file_name()?.to_str()?;
        let n = name.strip_prefix('v')?.strip_suffix(".blob")?.parse().ok()?;
        Some((n, path))
    }).collect();
    all.sort();
    all
}

#[test]
fn the_golden_saves_load_and_run_on_to_their_digests() {
    let mode = std::env::var("SAVES").unwrap_or_default();
    if mode == "blob" {
        // Today's format, from a fixed world: written by the core that is about to be left behind.
        let mut s = world(7);
        s.advance(40 * DAY);
        std::fs::write(saves_dir().join(format!("v{}.blob", save::FORMAT)), s.save()).unwrap();
    }
    let mut wrong = Vec::new();
    for (n, path) in goldens() {
        let blob = std::fs::read(&path).unwrap();
        if save::format_of(&blob) != Ok(n) {
            wrong.push(format!("{}: its header says format {:?}, its name {n}", path.display(), save::format_of(&blob)));
            continue;
        }
        if !migrate::reads(n) {
            wrong.push(format!("{}: this core can't read format {n}: a migration is missing from migrate::CHAIN", path.display()));
            continue;
        }
        let digest = match digest_of(&blob) {
            Ok(d) => d,
            Err(why) => {
                wrong.push(format!("{}: doesn't load: {why}", path.display()));
                continue;
            }
        };
        let file = path.with_extension("digest");
        if mode == "digest" {
            std::fs::write(&file, &digest).unwrap();
        } else if mode != "blob" && std::fs::read_to_string(&file).ok().as_deref() != Some(digest.as_str()) {
            wrong.push(format!("{}: run on {RUN_ON_DAYS} days it comes to {} (SAVES=digest writes it)", file.display(), digest.trim()));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn a_golden_save_digest_is_the_same_every_time_it_is_taken() {
    let mut s = world(7);
    s.advance(40 * DAY);
    let blob = s.save();
    assert_eq!(digest_of(&blob), digest_of(&blob));
    assert!(digest_of(&blob[..blob.len() / 2]).is_err());
}

#[test]
fn every_format_the_core_has_had_has_a_golden_save() {
    // From the first release: a format below today's that this core still reads (a step in the chain) has its golden.
    // (Until then the chain is empty.)
    let have: Vec<u32> = goldens().into_iter().map(|(n, _)| n).collect();
    for step in migrate::CHAIN {
        assert!(have.contains(&step.from), "migrate::CHAIN reads format {} but tests/saves/v{}.blob isn't there", step.from, step.from);
    }
}
