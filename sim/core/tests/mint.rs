//! The mint's rate, its sales and purchases, and its debasement, pinned by the numbers in `MoneyTests` and `WarTests`.

use enterprise_core::mint::{Mint, BUY_SPREAD, DEBASEMENT_SCALE, PRESSURE_FLOOR};
use enterprise_core::{Content, Good, Society};
use serde::{Deserialize, Serialize};

const DAY: u64 = 24_000;

/// Days as ticks, as the host passes them.
fn days(n: f64) -> u64 {
    (n * DAY as f64) as u64
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

// parity: MoneyTests.rateFallsWithSellingAndRecovers
#[test]
fn the_rate_falls_with_selling_and_recovers() {
    let mut mint = Mint::default();
    assert!(close(mint.rate(0, DAY), 16.0), "starts at 16 spurs per emerald");
    let paid = mint.sell(1728, 0, DAY);
    // A shulker's worth at once: 16 x 1728 x ln 2 = 19164, between the opening and closing rates.
    assert_eq!(paid, 19164, "a shulker of emeralds pays 19164 spurs");
    assert!(close(mint.rate(0, DAY), 8.0), "the rate halves after a shulker's worth: {}", mint.rate(0, DAY));
    assert!(close(mint.rate(days(3.0), DAY), 16.0 / 1.5), "half the pressure is gone after 3 days: {}",
        mint.rate(days(3.0), DAY));
}

// parity-partial: MoneyTests.surplusEmeraldsBecomeCoinsInStock
#[test]
fn a_quote_is_what_a_sale_pays_and_changes_nothing() {
    let mut mint = Mint::default();
    mint.sell(500, 0, DAY);
    let before = mint;
    let quoted = mint.quote(36, days(1.0), DAY);
    assert_eq!(mint, before, "a quote changes nothing");
    assert_eq!(mint.sell(36, days(1.0), DAY), quoted, "the sale pays what was quoted");
    assert!(quoted > 0 && quoted < 36 * 16, "priced along the falling rate: {quoted}");
}

// parity-partial: MoneyTests.theMerchantChangesCoinForEmeraldsToPayItsVillagers
#[test]
fn buying_emeralds_costs_a_little_over_the_rate_and_takes_back_pressure() {
    let mut mint = Mint::default();
    let rate = mint.rate(0, DAY);
    let cost = mint.buy_cost(2, 0, DAY);
    assert_eq!(cost, (2.0 * rate * (1.0 + BUY_SPREAD)).ceil() as u64, "two emeralds at the rate plus the spread");
    assert!(cost > (2.0 * rate) as u64, "a little over the rate: {cost}");
    assert_eq!(mint.buy(2, 0, DAY), cost, "buying pays the cost");
    assert!(close(mint.pressure(0, DAY), -2.0), "buying takes two emeralds of pressure back: {}", mint.pressure(0, DAY));
    assert!(mint.rate(0, DAY) > rate, "so the spur fetches more after buying");
}

#[test]
fn buying_takes_pressure_no_lower_than_its_floor() {
    let mut mint = Mint::default();
    mint.buy(5000, 0, DAY);
    assert!(close(mint.pressure(0, DAY), PRESSURE_FLOOR), "pressure stops at -864: {}", mint.pressure(0, DAY));
}

// parity-partial: WarTests.mobilisingTheSovereignMintsAndBuysGoldThroughAShop
#[test]
fn minting_debases_the_spur_and_the_debasement_works_out_over_thirty_days() {
    let mut mint = Mint::default();
    mint.mint(DEBASEMENT_SCALE as u64, 0, DAY);
    assert!(close(mint.debasement(0, DAY), 2.0), "minting 27648 doubles the debasement");
    assert!(close(mint.rate(0, DAY), 32.0), "so an emerald fetches 32 spurs: {}", mint.rate(0, DAY));
    assert!(close(mint.debasement(days(30.0), DAY), 1.5), "half the minting is gone after 30 days: {}",
        mint.debasement(days(30.0), DAY));
    assert!(close(mint.rate(days(30.0), DAY), 24.0), "so the rate is 24 then");
    let paid = mint.quote(1728, 0, DAY);
    assert_eq!(paid, 2 * 19164, "a debased sale pays twice the spurs");
}

#[test]
fn a_sale_then_a_purchase_is_priced_on_the_rate_at_the_time() {
    let mut mint = Mint::default();
    mint.sell(1728, 0, DAY);
    let later = days(1.0);
    let rate = mint.rate(later, DAY);
    assert!(rate > 8.0 && rate < 16.0, "partly recovered after a day: {rate}");
    assert_eq!(mint.buy_cost(10, later, DAY), (10.0 * rate * (1.0 + BUY_SPREAD)).ceil() as u64);
}

#[test]
fn the_mint_is_serializable_and_defaults_to_nothing_sold_and_nothing_minted() {
    fn save_ready<T: Serialize + for<'de> Deserialize<'de> + Default>() {}
    save_ready::<Mint>();
    let mint = Mint::default();
    assert_eq!((mint.pressure, mint.minted), (0.0, 0.0));
    assert!(close(mint.rate(0, DAY), 16.0));
}

#[test]
fn a_society_reads_its_mint_rate_and_sales_do_not_move_money() {
    let goods = vec![Good::new("emerald", 1.0, 64)];
    let mut s = Society::new(Content::new(goods, DAY));
    s.add_town("Emberfold", 100.0);
    assert!(close(s.mint_rate(), 16.0), "a new society starts at 16");
    let before = s.totals();
    let paid = s.sell_emeralds(1728);
    assert_eq!(paid, 19164);
    assert!(close(s.mint_rate(), 8.0), "the society's rate falls with the sale: {}", s.mint_rate());
    s.conserved(&before).expect("selling to the mint leaves the books as they were");
}
