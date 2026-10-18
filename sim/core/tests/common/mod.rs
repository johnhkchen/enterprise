//! The random worlds' generator, shared by the tests that run them (`random_worlds.rs`, `save.rs`...): the base world
//! below, then each slice's part of it, a file of its own in `slices/` listed in `slices.rs` (see there).
#![allow(dead_code)]

use enterprise_core::{Content, Good, Hex, Incoterm, Society};

pub const DAY: u64 = 24_000;

/// xorshift64*: the same seed, the same world.
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    pub fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.unit()
    }

    /// A stream of its own for a slice (by its name) in a world (by its seed), on a day (0: making the world): a slice
    /// added or changed leaves every other's draws as they were.
    pub fn of(seed: u64, slice: &str, day: u64) -> Rng {
        // FNV-1a over the name, then mixed with the seed and the day.
        let mut h: u64 = 0xCBF2_9CE4_8422_2325;
        for b in slice.bytes() {
            h = (h ^ b as u64).wrapping_mul(0x0100_0000_01B3);
        }
        let mut r = Rng((h ^ seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ day.wrapping_mul(0xC2B2_AE3D_27D4_EB4F)) | 1);
        r.next();
        r
    }
}

/// The emeralds each of a world's sellers sells to the mint each day (`random_worlds.rs`, `sim/host --sweep`): a few
/// towns, a stack or two each.
pub fn sellers(seed: u64) -> Vec<u64> {
    let mut r = Rng(seed.wrapping_mul(0xD1B5_4A32_D192_ED03) | 1);
    (0..r.below(4)).map(|_| (1 + r.below(4)) * 64 + r.below(64)).collect()
}

/// Runs `f` for each seed across the machine's cores (`std::thread::scope`, no dependency) and returns the results in
/// the seeds' order. Each thread takes the next seed not yet taken; a seed that panics stops the rest from starting
/// and fails the call with its seed named (its own message first, as the default hook prints it).
pub fn each_seed<T: Send>(seeds: impl IntoIterator<Item = u64>, f: impl Fn(u64) -> T + Sync) -> Vec<T> {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    let seeds: Vec<u64> = seeds.into_iter().collect();
    // (`SEED_THREADS=1` runs them one after another, to time a world or read one's prints in order.)
    let threads = std::env::var("SEED_THREADS").ok().and_then(|v| v.parse().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()))
        .clamp(1, seeds.len().max(1));
    let (next, failed) = (AtomicUsize::new(0), AtomicBool::new(false));
    let mut done: Vec<(usize, T)> = Vec::with_capacity(seeds.len());
    let mut first_failure: Option<(u64, String)> = None;
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| scope.spawn(|| {
                let mut mine = Vec::new();
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= seeds.len() || failed.load(Ordering::Relaxed) {
                        return Ok(mine);
                    }
                    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(seeds[i]))) {
                        Ok(t) => mine.push((i, t)),
                        Err(e) => {
                            failed.store(true, Ordering::Relaxed);
                            let msg = e.downcast_ref::<String>().cloned()
                                .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                                .unwrap_or_else(|| "a panic".into());
                            return Err((i, msg));
                        }
                    }
                }
            }))
            .collect();
        for w in workers {
            match w.join().expect("a seed's thread") {
                Ok(mine) => done.extend(mine),
                // The lowest seed's failure, whichever thread met it first.
                Err((i, msg)) => if first_failure.as_ref().is_none_or(|(s, _)| seeds[i] < *s) {
                    first_failure = Some((seeds[i], msg));
                },
            }
        }
    });
    if let Some((seed, msg)) = first_failure {
        panic!("seed {seed} failed: {msg}");
    }
    done.sort_by_key(|(i, _)| *i);
    done.into_iter().map(|(_, t)| t).collect()
}

/// The list of slices (`slices.rs`) made into calls: each slice's `extend` when a world is made, its `daily` each day
/// (the host's doings, after the day's run) and its `check` after it (its invariants), each with a stream of its own.
macro_rules! slices {
    ($($name:ident),* $(,)?) => {
        $(pub mod $name;)*

        /// Every slice's part of a new world, in the list's order.
        pub fn extend(s: &mut Society, seed: u64) {
            $($name::extend(s, &mut Rng::of(seed, stringify!($name), 0));)*
        }

        /// Every slice's doings for a day, after the day has run.
        pub fn daily(s: &mut Society, seed: u64, day: u64) {
            $($name::daily(s, &mut Rng::of(seed, stringify!($name), day), day);)*
        }

        /// Every slice's invariants, checked at the end of a day.
        pub fn check(s: &Society, day: u64) -> Result<(), String> {
            $($name::check(s, day).map_err(|e| format!("{}: {e}", stringify!($name)))?;)*
            Ok(())
        }
    };
}

pub mod slices;

pub fn world(seed: u64) -> Society {
    let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let n_goods = 2 + r.below(6) as usize;
    let goods: Vec<Good> = (0..n_goods)
        .map(|i| {
            let g = Good::new(&format!("good{i}"), r.range(0.02, 1.0), [16, 64][r.below(2) as usize]);
            match r.below(4) {
                0 => g.comfort(r.below(5) as usize),
                1 => g.food(r.range(1.0, 8.0)),
                _ => g,
            }
        })
        .collect();
    let mut s = Society::new(Content::new(goods, DAY));
    let n_towns = 2 + r.below(6) as usize;
    for t in 0..n_towns {
        s.add_town(&format!("town{t}"), r.range(0.0, 100.0));
        let villagers = r.below(20) as f64;
        s.set_people(t, villagers, villagers + r.below(4) as f64, villagers + r.below(20) as f64);
        // Its villagers as a roster: random squad trades and levels.
        let hands: Vec<(&str, u8)> = (0..villagers as usize)
            .map(|_| (enterprise_core::roster::SQUAD[r.below(enterprise_core::roster::SQUAD.len() as u64) as usize],
                1 + r.below(5) as u8))
            .collect();
        s.set_roster(t, &hands);
        for g in 0..n_goods {
            match r.below(3) {
                0 => s.set_trade(t, g, r.range(0.0, 200.0), 0.0),
                1 => s.set_trade(t, g, 0.0, r.range(0.0, 60.0)),
                _ => {}
            }
            if r.below(3) == 0 {
                s.deliver(t, g, r.range(0.0, 500.0).floor());
            }
        }
    }
    for _ in 0..r.below(3 * n_towns as u64) + 1 {
        let (a, b) = (r.below(n_towns as u64) as usize, r.below(n_towns as u64) as usize);
        if a != b {
            s.add_route(a, b, r.range(0.2, 3.0), r.range(0.0, 3.0), r.range(1.0, 8.0));
        }
    }
    for _ in 0..r.below(4) {
        let (a, b) = (r.below(n_towns as u64) as usize, r.below(n_towns as u64) as usize);
        if a != b {
            let terms = [Incoterm::Exw, Incoterm::Cpt, Incoterm::Dap, Incoterm::Ddp][r.below(4) as usize];
            s.draft(a, b, r.below(n_goods as u64) as usize, (1 + r.below(8)) as f64 * 16.0, r.range(0.02, 2.0),
                r.below(10) * DAY, (1 + r.below(4)) * DAY, 1 + r.below(10) as u32, terms, r.below(2) == 0);
        }
    }
    // A 12 x 12 patch of hexes: lakes, marsh, hills and cliffs, a few bridges; the towns placed on it, some joined by
    // roads planned over it (a road with no way through isn't built).
    for q in 0..12 {
        for rr in 0..12 {
            let hex = Hex::new(q, rr);
            let water = match r.below(5) {
                0 => 1.0,
                1 => r.range(0.0, 0.5),
                _ => 0.0,
            };
            let relief = if r.below(5) == 0 { r.range(0.0, 40.0) } else { 0.0 };
            s.set_ground(hex, water, relief);
            if water > 0.5 && r.below(4) == 0 {
                s.bridge(hex);
            }
        }
    }
    for t in 0..n_towns {
        let hex = Hex::new(r.below(12) as i32, r.below(12) as i32);
        s.place_town(t, hex.center_x() + r.range(-20.0, 20.0), hex.center_z() + r.range(-20.0, 20.0));
    }
    for _ in 0..r.below(2 * n_towns as u64) + 1 {
        let (a, b) = (r.below(n_towns as u64) as usize, r.below(n_towns as u64) as usize);
        if a != b {
            s.add_road(a, b, r.range(1.0, 8.0));
        }
    }
    slices::extend(&mut s, seed);
    s
}
