//! Property-based: random worlds (towns, goods, trades, people, routes, contracts) over hundreds of seeds, every day
//! of each checked for the books balancing and nobody in debt. A small generator of our own, so no dependency.

use enterprise_core::{Content, Good, Incoterm, Output, Society};

const DAY: u64 = 24_000;

/// xorshift64*: the same seed, the same world.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.unit()
    }
}

/// A world, and the emeralds its sellers sell to the mint each day: a few towns, a stack or two each.
fn world(seed: u64) -> (Society, Vec<u64>) {
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
    let sellers = r.below(4);
    let daily = (0..sellers).map(|_| (1 + r.below(4)) * 64 + r.below(64)).collect();
    (s, daily)
}

/// Advances `s` to day `day`, with each seller's emeralds sold to the mint at the day's start. Returns what came out.
fn run_day(s: &mut Society, daily: &[u64], day: u64) -> Vec<Output> {
    for &emeralds in daily {
        s.sell_emeralds(emeralds);
    }
    s.advance(day * DAY)
}

#[test]
fn the_books_balance_and_nobody_goes_into_debt_in_any_world() {
    for seed in 0..300 {
        let (mut s, daily) = world(seed);
        let start = s.totals();
        for day in 1..=120 {
            run_day(&mut s, &daily, day);
            if let Err(e) = s.conserved(&start).and_then(|_| s.solvent()) {
                panic!("seed {seed}, day {day}: {e}");
            }
        }
    }
}

#[test]
fn the_same_seed_gives_the_same_world() {
    for seed in [3, 77, 1234] {
        let ((mut a, da), (mut b, db)) = (world(seed), world(seed));
        let (mut oa, mut ob) = (Vec::new(), Vec::new());
        for day in 1..=90 {
            oa.extend(run_day(&mut a, &da, day));
            ob.extend(run_day(&mut b, &db, day));
        }
        assert_eq!(oa, ob, "seed {seed}: the same wagons");
        for t in 0..a.town_count() {
            assert_eq!(a.town(t).money.to_bits(), b.town(t).money.to_bits(), "seed {seed}, town {t}");
        }
        assert_eq!(a.mint_rate().to_bits(), b.mint_rate().to_bits(), "seed {seed}: the same mint rate");
    }
}
