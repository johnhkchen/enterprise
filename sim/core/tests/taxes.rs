//! Taxes (`taxes.rs`): a town withholds its income tax at the source, of its wages and its villagers' earnings.

use enterprise_core::{Content, Good, Society};

const DAY: u64 = 24_000;

#[test]
fn a_town_withholds_its_income_tax_of_wages_and_earnings_at_the_source() {
    let mut s = Society::new(Content::new(vec![Good::new("bread", 0.2, 64).food(5.0)], DAY));
    let t = s.add_town("Taxford", 100.0);
    s.set_people(t, 4.0, 4.0, 4.0);
    s.set_roster(t, &[("merchant", 1), ("farmer", 1), ("none", 1), ("none", 1)]);
    let before = s.totals();
    s.advance(10 * DAY);
    // What came into the households' purses, and what of it went back to the treasury.
    let book = s.account_book(enterprise_core::cash_book::Account::Households(t), 30);
    let sum = |what: &str| book.iter().filter(|(_, w, _)| w == what).map(|(_, _, n)| n.to_f64()).sum::<f64>();
    let (wages, earned, tax) = (sum("wages"), sum("earned"), -sum("income tax"));
    assert!(earned > 0.0 && tax > 0.0, "its villagers earned, and the treasury took its tax: {book:?}");
    let rate = s.content().dials.income_tax;
    // Each posting's tax rounded down to the thousandth.
    assert!((tax - rate * (wages + earned)).abs() <= 0.001 * book.len() as f64 + 1e-6, "{rate} of {}: {tax}", wages + earned);
    s.conserved(&before).unwrap();
}
