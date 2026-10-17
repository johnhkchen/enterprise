//! The random worlds' generator, shared by the tests that run them (`random_worlds.rs`, `save.rs`).
#![allow(dead_code)]

use enterprise_core::{Content, Good, Incoterm, Society};

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
}

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
    s
}
