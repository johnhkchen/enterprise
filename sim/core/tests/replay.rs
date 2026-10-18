//! The replay: a script of 200 days, played the same on every host. Its outputs (each day's messages, what a host sees
//! of every town and route, the mint's rate) and the world's save at the end are digested, and the digest checked
//! against the one committed in `tests/replay_digest.txt`. Native and WebAssembly builds must both match it: money is
//! fixed-point and the curves are computed from `+ - * /` alone (`fixed.rs`), so nothing in a run depends on the
//! host's libm. How to run it under wasm32-wasip1 is in `sim/README.md`.
//!
//! The script is the random worlds' generator (`common/`) for a few seeds, every slice's part in it, with the sellers'
//! emeralds sold to the mint each day and each slice's daily doings: every mechanism the core has, on one thread. And
//! the curves themselves (`fixed::exp`, `ln`, `exp2`, `pow`, `halved`, `powi`, `hypot`) over a grid, to the bit: a
//! last-bit difference the worlds would round away still shows there.
//!
//! When the rules change what a world does, the digest changes: `REPLAY_DIGEST=write cargo test --release --test
//! replay` writes the new one (natively), to commit with the change that moved it.

mod common;

use common::{sellers, world, DAY};

const SEEDS: [u64; 4] = [3, 77, 1234, 2026];
const DAYS: u64 = 200;

/// FNV-1a, 64 bits: a digest with no dependency, the same on every host (bytes in, a number out).
struct Fnv(u64);

impl Fnv {
    fn new() -> Self {
        Fnv(0xCBF2_9CE4_8422_2325)
    }
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 = (self.0 ^ *b as u64).wrapping_mul(0x0100_0000_01B3);
        }
    }
    fn line(&mut self, text: &str) {
        self.write(text.as_bytes());
        self.write(b"\n");
    }
}

/// One seed's run, digested: the script's every output in order, then its save.
fn play(seed: u64) -> (u64, usize) {
    let mut h = Fnv::new();
    let mut s = world(seed);
    let daily = sellers(seed);
    for day in 1..=DAYS {
        for &emeralds in &daily {
            s.sell_emeralds(emeralds);
        }
        let out = s.advance(day * DAY);
        common::slices::daily(&mut s, seed, day);
        h.line(&format!("day {day}: {out:?}"));
        for t in 0..s.town_count() {
            h.line(&format!("{:?}", s.town(t)));
        }
        h.line(&format!("mint {:?}", s.mint_rate()));
    }
    for route in 0..s.route_count() {
        h.line(&format!("{:?}", s.route(route)));
    }
    let blob = s.save();
    h.write(&blob);
    (h.0, blob.len())
}

/// The curves over a grid of the rules' ranges, every result's bits.
fn curves() -> u64 {
    use enterprise_core::fixed;
    let mut h = Fnv::new();
    for i in 0..4096 {
        let t = i as f64 / 4096.0;
        let x = -40.0 + 80.0 * t;
        let y = 1e-6 + 1e4 * t * t;
        for v in [fixed::exp(x), fixed::exp2(x), fixed::ln(y), fixed::pow(0.75, 60.0 * t), fixed::pow(1.002, 60.0 * t),
                fixed::halved(60.0 * t, 3.0), fixed::powi(15.0 / 16.0, i % 40), fixed::hypot(x, y)] {
            h.write(&v.to_bits().to_le_bytes());
        }
    }
    h.0
}

fn digest() -> String {
    let mut lines = vec![format!("curves: {:016x}", curves())];
    for seed in SEEDS {
        let (d, bytes) = play(seed);
        lines.push(format!("seed {seed}: {d:016x} ({bytes} bytes saved)"));
    }
    lines.join("\n") + "\n"
}

#[test]
fn the_replay_comes_out_as_its_committed_digest() {
    let got = digest();
    if std::env::var("REPLAY_DIGEST").as_deref() == Ok("write") {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/replay_digest.txt");
        std::fs::write(path, &got).expect("the digest written");
        println!("wrote {path}:\n{got}");
        return;
    }
    let want = include_str!("replay_digest.txt");
    assert_eq!(got, want, "the replay's digest moved (on {}): a change to the rules writes the new one with \
        `REPLAY_DIGEST=write cargo test --release --test replay`; the same build on another host should never move it",
        std::env::consts::ARCH);
}
