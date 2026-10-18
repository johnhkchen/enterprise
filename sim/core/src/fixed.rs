//! Money as fixed-point integers, and the few curves the rules need, so a replay comes out the same on every host:
//! natively, and in WebAssembly in Java (`docs/core-math.md`, fixed-point arithmetic; `docs/worktrees.md`, pain 6).
//!
//! **Money** is a [`Fixed`]: a whole number of thousandths of an emerald, in an `i64`. Every balance (a treasury, a
//! households' purse, a wagon's purse), every amount posted (`cash_book::post`), every debt and budget is one, so
//! money adds and subtracts exactly and the books balance to the thousandth (`check.rs` compares without a
//! tolerance). Rates stay `f64` (a good's worth an item, a share, trust, mood): money times a rate is rounded once
//! into a `Fixed` where it becomes money, by the rule the mod rounds that kind of sum by. The mod pays in whole spurs
//! at the mint's rate (`money/Treasury.java`): a payer's withdrawal for a price or a debt is rounded up to the spur
//! (`Treasury.withdraw`'s `Math.ceil(rest * rate)`; `Caravans` `Math.ceil(taken * each * rate)`), a share paid out is
//! rounded down (`Households.payIn`'s `Math.floor`, a toll's `Math.floor`). Here the unit is a thousandth, not a spur
//! (the core's money is emerald-worth; spurs are the binding's), and the same rule holds: [`Fixed::ceil`] for what a
//! payer owes for goods, food, debts and costs, [`Fixed::floor`] for shares paid out (wages, commissions, tolls), and
//! always no more than the payer holds.
//!
//! **Why not `f64` money:** `+ - * /` and `sqrt` are correctly rounded on every IEEE host, WebAssembly and Java
//! included, so `f64` replays to the bit; but sums of fractions drift with their order, and a balance that's
//! 0.1 + 0.2 isn't 0.3. In thousandths it is.
//!
//! **The curves** (`exp`, `ln`, `pow`, the half-lives' [`halved`], `hypot`) aren't correctly rounded in any libm, and
//! a native libm and WebAssembly's (or Java's `StrictMath`) may differ in the last bit. So they're computed here from
//! `+ - * /` alone (and a float's bits), which every IEEE host computes alike: the same bits natively and in
//! WebAssembly. A decay (trust's half-lives, the mint's pressure, mood) is computed in `f64` by these and the money it
//! moves rounded once into a `Fixed` at the boundary: the curve is a rate, the money is counted.

use std::fmt;
use std::iter::Sum;
use std::ops::{Add, AddAssign, Neg, Sub, SubAssign};

use serde::{Deserialize, Serialize};

/// Thousandths in an emerald.
pub const SCALE: i64 = 1000;

/// An amount of money: thousandths of an emerald.
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Fixed(i64);

/// How an `f64` figure rounds into thousandths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Round {
    Down,
    Up,
    Nearest,
}

/// `x` (in emeralds) in thousandths, rounded as `how`; a figure within a hair of a whole thousandth is that
/// thousandth, whichever way it rounds (`0.29 * 100.0` is `28.999999999999996` in binary: 29 emeralds, not 28.999).
fn thousandths(x: f64, how: Round) -> i64 {
    debug_assert!(!x.is_nan(), "money from NaN");
    let m = x * SCALE as f64;
    let nearest = m.round();
    let hair = 1e-6f64.max(nearest.abs() * 1e-13);
    let r = if (m - nearest).abs() <= hair {
        nearest
    } else {
        match how {
            Round::Down => m.floor(),
            Round::Up => m.ceil(),
            Round::Nearest => nearest,
        }
    };
    r as i64
}

impl Fixed {
    pub const ZERO: Fixed = Fixed(0);
    /// One emerald.
    pub const ONE: Fixed = Fixed(SCALE);
    /// The most there is (a sum no one can pay: an infinite cost, a cap that isn't one).
    pub const MAX: Fixed = Fixed(i64::MAX);

    /// So many thousandths of an emerald.
    pub const fn from_millis(millis: i64) -> Fixed {
        Fixed(millis)
    }

    /// So many whole emeralds.
    pub const fn emeralds(n: i64) -> Fixed {
        Fixed(n * SCALE)
    }

    /// Its thousandths.
    pub const fn millis(self) -> i64 {
        self.0
    }

    /// A figure in emeralds rounded down to the thousandth: a share paid out.
    pub fn floor(x: f64) -> Fixed {
        Fixed(thousandths(x, Round::Down))
    }

    /// A figure in emeralds rounded up to the thousandth: what a payer owes for a price, a debt or a cost.
    pub fn ceil(x: f64) -> Fixed {
        Fixed(thousandths(x, Round::Up))
    }

    /// A figure in emeralds to the nearest thousandth (half away from 0): a host's figure, a tally.
    pub fn round(x: f64) -> Fixed {
        Fixed(thousandths(x, Round::Nearest))
    }

    /// In emeralds, for the rates it's weighed against: the nearest `f64` to its thousandths over 1,000, so
    /// `Fixed::round(12.345).to_f64() == 12.345`.
    pub fn to_f64(self) -> f64 {
        self.0 as f64 / SCALE as f64
    }

    /// So much of it (`r` of it), rounded down: a share paid out.
    pub fn mul_floor(self, r: f64) -> Fixed {
        Fixed::floor(self.to_f64() * r)
    }

    /// So much of it, rounded up: a payer's part of a cost.
    pub fn mul_ceil(self, r: f64) -> Fixed {
        Fixed::ceil(self.to_f64() * r)
    }

    /// So much of it, to the nearest thousandth.
    pub fn mul_round(self, r: f64) -> Fixed {
        Fixed::round(self.to_f64() * r)
    }

    /// `n` of it (whole times).
    pub fn times(self, n: i64) -> Fixed {
        Fixed(self.0.checked_mul(n).expect("money overflowed"))
    }

    /// How many of `per` it makes, as a rate (`per` 0: none).
    pub fn ratio(self, per: Fixed) -> f64 {
        if per.0 == 0 { 0.0 } else { self.0 as f64 / per.0 as f64 }
    }

    pub fn checked_add(self, other: Fixed) -> Option<Fixed> {
        self.0.checked_add(other.0).map(Fixed)
    }

    pub fn checked_sub(self, other: Fixed) -> Option<Fixed> {
        self.0.checked_sub(other.0).map(Fixed)
    }

    /// Never below nothing.
    pub fn at_least_zero(self) -> Fixed {
        self.max(Fixed::ZERO)
    }

    pub fn abs(self) -> Fixed {
        Fixed(self.0.checked_abs().expect("money overflowed"))
    }

    pub fn is_zero(self) -> bool {
        self.0 == 0
    }

    pub fn is_positive(self) -> bool {
        self.0 > 0
    }

    pub fn is_negative(self) -> bool {
        self.0 < 0
    }
}

impl Add for Fixed {
    type Output = Fixed;
    fn add(self, other: Fixed) -> Fixed {
        self.checked_add(other).expect("money overflowed")
    }
}

impl Sub for Fixed {
    type Output = Fixed;
    fn sub(self, other: Fixed) -> Fixed {
        self.checked_sub(other).expect("money overflowed")
    }
}

impl Neg for Fixed {
    type Output = Fixed;
    fn neg(self) -> Fixed {
        Fixed(self.0.checked_neg().expect("money overflowed"))
    }
}

impl AddAssign for Fixed {
    fn add_assign(&mut self, other: Fixed) {
        *self = *self + other;
    }
}

impl SubAssign for Fixed {
    fn sub_assign(&mut self, other: Fixed) {
        *self = *self - other;
    }
}

impl Sum for Fixed {
    fn sum<I: Iterator<Item = Fixed>>(iter: I) -> Fixed {
        iter.fold(Fixed::ZERO, |a, b| a + b)
    }
}

impl<'a> Sum<&'a Fixed> for Fixed {
    fn sum<I: Iterator<Item = &'a Fixed>>(iter: I) -> Fixed {
        iter.fold(Fixed::ZERO, |a, b| a + *b)
    }
}

/// A host's figure in emeralds, to the nearest thousandth (`add_town("A", 100.0)`).
impl From<f64> for Fixed {
    fn from(x: f64) -> Fixed {
        Fixed::round(x)
    }
}

/// Whole emeralds.
impl From<i32> for Fixed {
    fn from(n: i32) -> Fixed {
        Fixed::emeralds(n as i64)
    }
}

impl From<i64> for Fixed {
    fn from(n: i64) -> Fixed {
        Fixed::emeralds(n)
    }
}

impl From<u32> for Fixed {
    fn from(n: u32) -> Fixed {
        Fixed::emeralds(n as i64)
    }
}

/// Compared with a figure in emeralds exactly: `Fixed::round(12.345) == 12.345`, and `Fixed::round(0.3) > 0.2999`.
impl PartialEq<f64> for Fixed {
    fn eq(&self, other: &f64) -> bool {
        self.to_f64() == *other
    }
}

impl PartialOrd<f64> for Fixed {
    fn partial_cmp(&self, other: &f64) -> Option<std::cmp::Ordering> {
        self.to_f64().partial_cmp(other)
    }
}

/// In emeralds, to the thousandth: `12.345`, `-0.005`, `3`.
impl fmt::Display for Fixed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.0 < 0 { "-" } else { "" };
        let a = self.0.unsigned_abs();
        let (whole, part) = (a / SCALE as u64, a % SCALE as u64);
        if part == 0 {
            write!(f, "{sign}{whole}")
        } else {
            let digits = format!("{part:03}");
            write!(f, "{sign}{whole}.{}", digits.trim_end_matches('0'))
        }
    }
}

impl fmt::Debug for Fixed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

// The curves, from + - * / alone.

const LN2: f64 = std::f64::consts::LN_2;
/// ln 2 in two parts (fdlibm's): the high part's low bits are 0, so `k * LN2_HI` is exact for any exponent `k`.
const LN2_HI: f64 = 6.931_471_803_691_238_164_90e-1;
const LN2_LO: f64 = 1.908_214_929_270_587_700_02e-10;

/// 2^k for a whole `k` in the normal range, from its bits.
fn two_to(k: i64) -> f64 {
    debug_assert!((-1022..=1023).contains(&k));
    f64::from_bits(((k + 1023) as u64) << 52)
}

/// `y` times 2^k, through the subnormals and up to the largest exponent.
fn scaled(y: f64, k: i64) -> f64 {
    if k > 1023 {
        y * two_to(1023) * two_to(k - 1023)
    } else if k < -1022 {
        y * two_to(-1022) * two_to((k + 1022).max(-1022))
    } else {
        y * two_to(k)
    }
}

/// e^r for |r| up to about 0.35: its series to r^13 (the next term is under half a unit in the last place), by
/// Horner's rule.
fn exp_small(r: f64) -> f64 {
    let mut p = 1.0;
    for n in (1..=13).rev() {
        p = 1.0 + p * r / n as f64;
    }
    p
}

/// e^x, the same bits on every IEEE host (within two units in the last place of the true value).
pub fn exp(x: f64) -> f64 {
    if x.is_nan() {
        return x;
    }
    if x > 709.782_712_893_384 {
        return f64::INFINITY;
    }
    if x < -745.133_219_101_941_1 {
        return 0.0;
    }
    // x = k ln 2 + r, |r| <= ln 2 / 2 (Cody and Waite).
    let k = (x / LN2).round();
    let r = (x - k * LN2_HI) - k * LN2_LO;
    scaled(exp_small(r), k as i64)
}

/// 2^x, the same bits on every IEEE host: 2^k exactly for its whole part, e^(f ln 2) for the rest.
pub fn exp2(x: f64) -> f64 {
    if x.is_nan() {
        return x;
    }
    if x >= 1024.0 {
        return f64::INFINITY;
    }
    if x < -1075.0 {
        return 0.0;
    }
    let k = x.round();
    let f = x - k; // exact: |f| <= 1/2
    scaled(exp_small(f * LN2), k as i64)
}

/// The natural log of `x`, the same bits on every IEEE host: x = m 2^e with m in [√½, √2), and ln m by its
/// series in s = (m - 1) / (m + 1).
pub fn ln(x: f64) -> f64 {
    if x.is_nan() || x < 0.0 {
        return f64::NAN;
    }
    if x == 0.0 {
        return f64::NEG_INFINITY;
    }
    if x.is_infinite() {
        return x;
    }
    let (mut bits, mut e) = (x.to_bits(), 0i64);
    if bits >> 52 == 0 {
        // Subnormal: scaled up by 2^54 first.
        bits = (x * two_to(54)).to_bits();
        e = -54;
    }
    e += ((bits >> 52) & 0x7ff) as i64 - 1023;
    let mut m = f64::from_bits((bits & 0x000f_ffff_ffff_ffff) | 0x3ff0_0000_0000_0000);
    if m > std::f64::consts::SQRT_2 {
        m *= 0.5;
        e += 1;
    }
    let f = m - 1.0; // exact
    let s = f / (2.0 + f);
    let s2 = s * s;
    // ln m = 2 atanh s = 2s (1 + s^2/3 + s^4/5 + ...); |s| < 0.172, so s^24 is under a unit in the last place.
    let mut p = 0.0;
    for k in (1..=12).rev() {
        p = 1.0 / (2 * k + 1) as f64 + s2 * p;
    }
    let ln_m = 2.0 * s + 2.0 * s * (s2 * p);
    let e = e as f64;
    e * LN2_HI + (ln_m + e * LN2_LO)
}

/// `base` to the power `x` (`base` above 0; 0 to a positive power is 0), the same bits on every IEEE host:
/// e^(x ln base).
pub fn pow(base: f64, x: f64) -> f64 {
    if x == 0.0 || base == 1.0 {
        return 1.0;
    }
    if base == 0.0 {
        return if x > 0.0 { 0.0 } else { f64::INFINITY };
    }
    exp(x * ln(base))
}

/// What's left of something halving every `half_life` after `elapsed` (in the same units): 0.5^(elapsed /
/// half_life), the decays' curve (trust, the mint's pressure, mood, a farm's sales).
pub fn halved(elapsed: f64, half_life: f64) -> f64 {
    exp2(-elapsed / half_life)
}

/// `base` to a whole power, by squaring: the same bits on every host.
pub fn powi(base: f64, n: i32) -> f64 {
    let mut result = 1.0;
    let mut b = base;
    let mut e = n.unsigned_abs();
    while e > 0 {
        if e & 1 == 1 {
            result *= b;
        }
        b *= b;
        e >>= 1;
    }
    if n < 0 { 1.0 / result } else { result }
}

/// The length of (x, y): √(x² + y²), correctly rounded wherever x² + y² is exact (whole blocks), as libm's `hypot`
/// isn't on every host.
pub fn hypot(x: f64, y: f64) -> f64 {
    (x * x + y * y).sqrt()
}
