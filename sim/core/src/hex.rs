//! A hex of the world grid, in axial coordinates (pointy-top): the ledger's unit of place. Ownership, tolls and route
//! lengths are per hex. A hex is [`ACROSS`] blocks across its flat sides, about the core of a naturally generated
//! village.
//!
//! Ported exactly from the mod's `hex/Hex.java` (`at`, `centerX`, `centerZ`, `s`, `distance`, `neighbors`, `line`,
//! `round`): the same rounding (Java's `Math.round`, half up), the same direction order (east first), the same nudge
//! off the edges in `line`, so a host and the mod agree on every hex.

use serde::{Deserialize, Serialize};

/// Blocks across a hex, flat side to flat side.
pub const ACROSS: f64 = 80.0;

/// Centre to corner.
pub fn size() -> f64 {
    ACROSS / 3f64.sqrt()
}

/// The six neighbours' offsets, east first, then round anticlockwise as the mod has them.
pub const DIRECTIONS: [(i32, i32); 6] = [(1, 0), (1, -1), (0, -1), (-1, 0), (-1, 1), (0, 1)];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Hex {
    pub q: i32,
    pub r: i32,
}

/// Java's `Math.round`: the nearest whole number, halves up (toward positive infinity, unlike Rust's `round`).
fn java_round(x: f64) -> i64 {
    (x + 0.5).floor() as i64
}

impl Hex {
    pub const fn new(q: i32, r: i32) -> Self {
        Hex { q, r }
    }

    /// The hex holding the point (x, z) in blocks. (The mod's `at(BlockPos)` adds half a block to each first.)
    pub fn at(x: f64, z: f64) -> Hex {
        let q = (3f64.sqrt() / 3.0 * x - z / 3.0) / size();
        let r = (2.0 / 3.0 * z) / size();
        Hex::round(q, r)
    }

    pub fn center_x(&self) -> f64 {
        size() * 3f64.sqrt() * (self.q as f64 + self.r as f64 / 2.0)
    }

    pub fn center_z(&self) -> f64 {
        size() * 1.5 * self.r as f64
    }

    pub fn s(&self) -> i32 {
        -self.q - self.r
    }

    /// Steps between two hexes.
    pub fn distance(&self, other: Hex) -> i32 {
        ((self.q - other.q).abs() + (self.r - other.r).abs() + (self.s() - other.s()).abs()) / 2
    }

    /// The six around this one, east first.
    pub fn neighbors(&self) -> [Hex; 6] {
        DIRECTIONS.map(|(dq, dr)| Hex::new(self.q + dq, self.r + dr))
    }

    /// Every hex on the straight line from `a` to `b`, both included.
    pub fn line(a: Hex, b: Hex) -> Vec<Hex> {
        let n = a.distance(b);
        // Nudge off the exact edges so ties round the same way every time.
        let (aq, ar) = (a.q as f64 + 1e-6, a.r as f64 + 1e-6);
        let (bq, br) = (b.q as f64 + 1e-6, b.r as f64 + 1e-6);
        (0..=n)
            .map(|i| {
                let t = if n == 0 { 0.0 } else { i as f64 / n as f64 };
                Hex::round(aq + (bq - aq) * t, ar + (br - ar) * t)
            })
            .collect()
    }

    /// Cube rounding: round all three coordinates, then fix the one that moved most.
    fn round(q: f64, r: f64) -> Hex {
        let s = -q - r;
        let (mut rq, mut rr, rs) = (java_round(q), java_round(r), java_round(s));
        let dq = (rq as f64 - q).abs();
        let dr = (rr as f64 - r).abs();
        let ds = (rs as f64 - s).abs();
        if dq > dr && dq > ds {
            rq = -rr - rs;
        } else if dr > ds {
            rr = -rq - rs;
        }
        Hex::new(rq as i32, rr as i32)
    }
}

impl std::fmt::Display for Hex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {})", self.q, self.r)
    }
}
