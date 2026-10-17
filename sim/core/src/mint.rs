//! The mint: the spur's rate against emeralds, as the mod's `money/Mint.java` plays it. The rate starts at
//! [`BASE_SPURS_PER_EMERALD`] spurs an emerald and falls as emeralds are sold to the mint (selling pressure), and
//! recovers as that pressure decays. Coin minted with nothing behind it (a sovereign paying for a war) debases the
//! spur, and that works its way out over [`DEBASEMENT_HALF_LIFE_DAYS`].
//!
//! Computed on touch, as the mod does: one pressure number and the tick it was last brought up to date, and the same
//! for the minted coin. Java keeps one such pair per faction; the core has one faction (the commons), so one pair each.
//! Times are ticks; the half-lives are days, converted with the host's `day_ticks` passed in by the caller.
//!
//! The core's money is emerald-worth, so the mint changes what a sale is paid in (coin), not what the town holds in
//! worth; see `Society::sell_emeralds`. The mod's `MoneyTests.rateFallsWithSellingAndRecovers` pins the curve.

use serde::{Deserialize, Serialize};

use bevy_ecs::prelude::Resource;

/// Spurs per emerald when nobody has been selling.
pub const BASE_SPURS_PER_EMERALD: f64 = 16.0;

/// Emeralds of recent selling that halve the rate: one full shulker box.
pub const PRESSURE_SCALE: f64 = 1728.0;

/// Selling pressure halves every three in-game days.
pub const HALF_LIFE_DAYS: f64 = 3.0;

/// Spurs minted without backing that double the spurs an emerald fetches.
pub const DEBASEMENT_SCALE: f64 = BASE_SPURS_PER_EMERALD * PRESSURE_SCALE;

/// Minted coin's debasement works its way out over this many days.
pub const DEBASEMENT_HALF_LIFE_DAYS: f64 = 30.0;

/// Buying emeralds back costs this much over the rate.
pub const BUY_SPREAD: f64 = 0.05;

/// Buying takes back selling pressure, but no lower than minus half its scale.
pub const PRESSURE_FLOOR: f64 = -PRESSURE_SCALE / 2.0;

/// The mint's selling pressure and minted coin, each with the tick it was last brought up to date.
#[derive(Resource, Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Mint {
    pub pressure: f64,
    pub pressure_at: u64,
    pub minted: f64,
    pub minted_at: u64,
}

/// Decays `value`, last set at `at`, to `now`, halving every `half_life` ticks.
fn decayed(value: f64, at: u64, now: u64, half_life: f64) -> f64 {
    let elapsed = now.saturating_sub(at) as f64;
    value * 0.5f64.powf(elapsed / half_life)
}

/// Spurs for `emeralds` sold at selling pressure `pressure`: the integral of base / (1 + p / scale) from p to p + n,
/// rounded down.
fn spurs_for(pressure: f64, emeralds: u64) -> u64 {
    if emeralds == 0 {
        return 0;
    }
    let paid = BASE_SPURS_PER_EMERALD * PRESSURE_SCALE
        * ((PRESSURE_SCALE + pressure + emeralds as f64) / (PRESSURE_SCALE + pressure)).ln();
    paid.floor() as u64
}

impl Mint {
    fn half_life(day_ticks: u64) -> f64 {
        HALF_LIFE_DAYS * day_ticks as f64
    }

    fn debasement_half_life(day_ticks: u64) -> f64 {
        DEBASEMENT_HALF_LIFE_DAYS * day_ticks as f64
    }

    /// Selling pressure now: recent emeralds sold, decayed by elapsed time.
    pub fn pressure(&self, now: u64, day_ticks: u64) -> f64 {
        decayed(self.pressure, self.pressure_at, now, Self::half_life(day_ticks))
    }

    /// Spurs minted without backing, as they stand now (decayed).
    pub fn minted(&self, now: u64, day_ticks: u64) -> f64 {
        decayed(self.minted, self.minted_at, now, Self::debasement_half_life(day_ticks))
    }

    /// How far the spur is debased: 1 when nothing's been minted, 2 when it fetches half as much.
    pub fn debasement(&self, now: u64, day_ticks: u64) -> f64 {
        1.0 + self.minted(now, day_ticks) / DEBASEMENT_SCALE
    }

    /// Spurs per emerald for the next emerald sold.
    pub fn rate(&self, now: u64, day_ticks: u64) -> f64 {
        BASE_SPURS_PER_EMERALD * self.debasement(now, day_ticks) / (1.0 + self.pressure(now, day_ticks) / PRESSURE_SCALE)
    }

    /// Spurs paid for `emeralds` sold now, priced along the falling rate. Changes nothing.
    pub fn quote(&self, emeralds: u64, now: u64, day_ticks: u64) -> u64 {
        (spurs_for(self.pressure(now, day_ticks), emeralds) as f64 * self.debasement(now, day_ticks)).floor() as u64
    }

    /// Sells `emeralds` to the mint: returns the spurs paid and adds the selling pressure.
    pub fn sell(&mut self, emeralds: u64, now: u64, day_ticks: u64) -> u64 {
        let pressure = self.pressure(now, day_ticks);
        let spurs = self.quote(emeralds, now, day_ticks);
        self.pressure = pressure + emeralds as f64;
        self.pressure_at = now;
        spurs
    }

    /// Spurs it would take to buy `emeralds` from the mint now. Changes nothing.
    pub fn buy_cost(&self, emeralds: u64, now: u64, day_ticks: u64) -> u64 {
        (emeralds as f64 * self.rate(now, day_ticks) * (1.0 + BUY_SPREAD)).ceil() as u64
    }

    /// Buys `emeralds` from the mint (to pay villagers in): returns the spurs it cost. Buying takes back selling
    /// pressure, so the spur fetches less, and pressure goes no lower than [`PRESSURE_FLOOR`].
    pub fn buy(&mut self, emeralds: u64, now: u64, day_ticks: u64) -> u64 {
        let spurs = self.buy_cost(emeralds, now, day_ticks);
        self.pressure = (self.pressure(now, day_ticks) - emeralds as f64).max(PRESSURE_FLOOR);
        self.pressure_at = now;
        spurs
    }

    /// A sovereign mints `spurs` to pay with: coin with nothing behind it, which debases the spur.
    pub fn mint(&mut self, spurs: u64, now: u64, day_ticks: u64) {
        self.minted = self.minted(now, day_ticks) + spurs as f64;
        self.minted_at = now;
    }
}
