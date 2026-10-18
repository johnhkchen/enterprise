//! Fixed-point money and the curves computed from `+ - * /` alone (`fixed.rs`): thousandths add exactly, round the
//! way the mod rounds each kind of sum, and the curves agree with the platform's libm to a few units in the last place.

use enterprise_core::fixed::{self, Fixed};

/// Units in the last place between two floats of the same sign.
fn ulps(a: f64, b: f64) -> u64 {
    if a == b {
        return 0;
    }
    (a.to_bits() as i64 - b.to_bits() as i64).unsigned_abs()
}

#[test]
fn thousandths_add_exactly() {
    let tenth = Fixed::round(0.1);
    let fifth = Fixed::round(0.2);
    assert_eq!(tenth + fifth, Fixed::round(0.3));
    assert_ne!(0.1 + 0.2, 0.3, "as floats they don't");
    // Ten thousand tenths are a thousand emeralds to the thousandth.
    let sum: Fixed = (0..10_000).map(|_| tenth).sum();
    assert_eq!(sum, Fixed::emeralds(1000));
    assert_eq!(Fixed::emeralds(3) - Fixed::from_millis(1), Fixed::round(2.999));
}

#[test]
fn a_share_rounds_down_a_cost_rounds_up_and_a_hair_off_a_thousandth_is_it() {
    assert_eq!(Fixed::floor(1.2349), Fixed::from_millis(1234));
    assert_eq!(Fixed::ceil(1.2341), Fixed::from_millis(1235));
    assert_eq!(Fixed::round(1.2345), Fixed::from_millis(1235));
    assert_eq!(Fixed::round(-1.2345), Fixed::from_millis(-1235));
    // 0.29 * 100 is 28.999999999999996 in binary: 29 emeralds either way.
    assert_eq!(Fixed::floor(0.29 * 100.0), Fixed::emeralds(29));
    assert_eq!(Fixed::ceil(0.1 + 0.2), Fixed::from_millis(300));
    // A share of a sum, and a payer's part of a cost.
    assert_eq!(Fixed::emeralds(10).mul_floor(1.0 / 3.0), Fixed::from_millis(3333));
    assert_eq!(Fixed::emeralds(10).mul_ceil(1.0 / 3.0), Fixed::from_millis(3334));
    assert_eq!(Fixed::emeralds(10).ratio(Fixed::emeralds(4)), 2.5);
}

#[test]
fn compared_with_a_figure_in_emeralds_exactly() {
    assert_eq!(Fixed::round(12.345), 12.345);
    assert!(Fixed::round(12.345) > 12.344);
    assert!(Fixed::round(12.345) < 12.346);
    assert_eq!(Fixed::from(100.0), Fixed::emeralds(100));
    assert_eq!(Fixed::from(7), Fixed::emeralds(7));
}

#[test]
fn shown_in_emeralds_to_the_thousandth() {
    assert_eq!(Fixed::round(12.345).to_string(), "12.345");
    assert_eq!(Fixed::round(12.3).to_string(), "12.3");
    assert_eq!(Fixed::emeralds(3).to_string(), "3");
    assert_eq!(Fixed::from_millis(-5).to_string(), "-0.005");
    assert_eq!(format!("{:?}", Fixed::from_millis(1050)), "1.05");
}

#[test]
#[should_panic(expected = "money overflowed")]
fn an_overflow_is_caught_not_wrapped() {
    let _ = Fixed::MAX + Fixed::ONE;
}

#[test]
fn saved_as_its_thousandths() {
    let bytes = postcard_round_trip(Fixed::from_millis(-123_456));
    assert_eq!(bytes, Fixed::from_millis(-123_456));
}

fn postcard_round_trip(x: Fixed) -> Fixed {
    // The save is postcard (`save.rs`); a Fixed is its i64.
    let mut s = enterprise_core::Society::new(enterprise_core::Content::new(vec![], 24_000));
    let t = s.add_town("A", x);
    let blob = s.save();
    let loaded = enterprise_core::Society::load(&blob).expect("a save");
    loaded.town(t).money
}

/// The curves against the platform's libm: within a few units in the last place over the ranges the rules use.
#[test]
fn the_curves_agree_with_libm_to_a_few_units_in_the_last_place() {
    let mut worst = (0u64, 0u64, 0u64, 0u64);
    for i in 0..20_000 {
        let t = i as f64 / 20_000.0;
        let x = -40.0 + 80.0 * t;
        worst.0 = worst.0.max(ulps(fixed::exp(x), x.exp()));
        let y = 1e-6 + 1e4 * t * t;
        worst.1 = worst.1.max(ulps(fixed::ln(y), y.ln()));
        let days = 60.0 * t;
        worst.2 = worst.2.max(ulps(fixed::halved(days, 3.0), 0.5f64.powf(days / 3.0)));
        worst.3 = worst.3.max(ulps(fixed::pow(0.75, days), 0.75f64.powf(days)));
    }
    assert!(worst.0 <= 4, "exp: {} ulps", worst.0);
    assert!(worst.1 <= 4, "ln: {} ulps", worst.1);
    assert!(worst.2 <= 4, "halved: {} ulps", worst.2);
    assert!(worst.3 <= 64, "pow: {} ulps", worst.3);
    assert_eq!(fixed::exp(0.0), 1.0);
    assert_eq!(fixed::ln(1.0), 0.0);
    assert_eq!(fixed::halved(3.0, 3.0), 0.5);
    assert_eq!(fixed::halved(0.0, 3.0), 1.0);
    assert_eq!(fixed::pow(2.0, 10.0), 1024.0);
    assert_eq!(fixed::powi(15.0 / 16.0, 2), 225.0 / 256.0);
    assert_eq!(fixed::hypot(3.0, 4.0), 5.0);
    assert!((fixed::ln(f64::MIN_POSITIVE / 8.0) - (f64::MIN_POSITIVE / 8.0).ln()).abs() < 1e-12);
    assert_eq!(fixed::exp(1000.0), f64::INFINITY);
    assert_eq!(fixed::exp(-1000.0), 0.0);
}
