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

/// A farm town making wheat and a mill town whose people use it, the farm's wagon every two days; each holding three
/// hexes of open ground.
fn farm_and_mill() -> (Society, usize, usize) {
    let mut s = Society::new(Content::new(vec![Good::new("wheat", 0.05, 64)], DAY));
    s.content_mut().dials.standing_orders = false;
    let farm = s.add_town("Wheatfield", 100.0);
    let mill = s.add_town("Hayford", 200.0);
    s.set_trade(farm, 0, 128.0, 0.0);
    s.set_trade(mill, 0, 0.0, 64.0);
    s.add_route(farm, mill, 1.0, 0.5, 2.0);
    for (t, q) in [(farm, 0), (mill, 10)] {
        for r in 0..3 {
            s.claim_hex(t, enterprise_core::Hex::new(q, r));
        }
    }
    (s, farm, mill)
}

#[test]
fn the_capitals_wagon_collects_the_land_tax_of_the_towns_it_calls_at() {
    let (mut s, farm, mill) = farm_and_mill();
    s.name_capital(farm, 0);
    let before = s.totals();
    s.advance(20 * DAY);
    let (owed, paid) = s.land_tax(mill);
    let rent = 3.0 * s.content().dials.land_tax;
    assert!(paid.to_f64() > 10.0 * rent, "the mill paid its three hexes' rent as the capital's wagon called: {paid}, owing {owed}");
    assert!(owed.to_f64() <= 3.0 * rent + 1e-6, "little left owing: {owed}");
    let (highways, _) = s.highways();
    assert!(highways.to_f64() > 0.0 || s.highways().1 > 0, "for the king's highways");
    assert_eq!(s.land_tax(farm).1.to_f64(), 0.0, "the capital owes none of its own");
    s.conserved(&before).unwrap();
}

#[test]
fn a_towns_wagon_carries_its_land_tax_to_the_capital() {
    let (mut s, farm, mill) = farm_and_mill();
    s.name_capital(mill, 0);
    s.advance(20 * DAY);
    let (_, paid) = s.land_tax(farm);
    assert!(paid.to_f64() > 0.0, "the farm's wagon paid at the capital: {:?}", s.land_tax(farm));
    let book = s.cash_book(mill, 30);
    assert!(book.iter().any(|(_, w, n)| w == "land tax" && n.to_f64() > 0.0), "into the capital's treasury: {book:?}");
}

#[test]
fn a_town_off_the_capitals_roads_owes_nothing() {
    let (mut s, farm, mill) = farm_and_mill();
    let far = s.add_town("Faraway", 50.0);
    s.claim_hex(far, enterprise_core::Hex::new(30, 0));
    s.name_capital(farm, 0);
    s.advance(10 * DAY);
    assert_eq!(s.land_tax(far), (enterprise_core::Fixed::ZERO, enterprise_core::Fixed::ZERO), "no road, no carrier, no tax");
    let _ = mill;
}

#[test]
fn a_town_takes_vat_on_the_comforts_its_households_buy() {
    let mut goods: Vec<Good> = Content::COMFORTS.iter().enumerate()
        .map(|(k, name)| Good::new(&format!("{name}_good"), 0.05, 64).comfort(k)).collect();
    goods.push(Good::new("wheat", 0.05, 64));
    let mut s = Society::new(Content::new(goods, DAY));
    s.content_mut().dials.standing_orders = false;
    let t = s.add_town("Meadowworth", 40.0);
    s.set_people(t, 8.0, 8.0, 8.0);
    s.set_trade(t, 5, 128.0, 0.0);
    for k in 0..5 {
        s.deliver(t, k, 500.0);
    }
    let before = s.totals();
    s.advance(5 * DAY);
    let book = s.account_book(enterprise_core::cash_book::Account::Households(t), 30);
    let sum = |what: &str| -book.iter().filter(|(_, w, _)| w == what).map(|(_, _, n)| n.to_f64()).sum::<f64>();
    let (comforts, vat) = (sum("comforts"), sum("VAT"));
    assert!(comforts > 0.0 && vat > 0.0, "they bought their comforts and paid VAT on them: {book:?}");
    let rate = s.content().dials.vat;
    // Their purse spent on the comforts first, the VAT as far as what's left goes.
    assert!(vat <= rate * comforts + 1e-6 && vat >= 0.5 * rate * comforts, "{rate} of {comforts}, as far as their purse went: {vat}");
    s.conserved(&before).unwrap();
}

#[test]
fn a_crowded_hex_slows_wagons_and_a_paved_one_carries_more() {
    let mut map = enterprise_core::map::Map::default();
    let h = enterprise_core::Hex::new(0, 0);
    let day = DAY;
    assert_eq!(map.crowding(h, 0, day), 0.0, "an empty hex isn't crowded");
    // Six crossings a day, kept up: three times what a trail carries freely.
    let kept = 6.0 * enterprise_core::map::WEAR_HALF_LIFE_DAYS / std::f64::consts::LN_2;
    map.wear(&[h], kept, 0, day);
    assert!((map.crossings_a_day(h, 0, day) - 6.0).abs() < 1e-9);
    assert_eq!(map.crowding(h, 0, day), 1.0, "crowded fully");
    assert!((map.pace(&[h], 0, day) - 2.0).abs() < 1e-9, "twice the time");
    map.surface.insert(h, enterprise_core::map::PAVED);
    assert!((map.crowding(h, 0, day) - 0.0).abs() < 1e-9, "paved, it carries eight a day freely");
    assert!((map.pace(&[h], 0, day) - enterprise_core::map::PAVED_PACE).abs() < 1e-9);
}

#[test]
fn a_wagon_pays_the_owner_of_crowded_land_it_crosses() {
    // The farm's wagons to the mill over Crowdham's land, its middle hex crowded by others' traffic.
    let goods = vec![Good::new("wheat", 0.05, 64)];
    let mut s = Society::new(Content::new(goods, DAY));
    s.content_mut().dials.standing_orders = false;
    let farm = s.add_town("Wheatfield", 100.0);
    let mill = s.add_town("Hayford", 200.0);
    let crowd = s.add_town("Crowdham", 10.0);
    let (a, b) = (enterprise_core::Hex::new(0, 0), enterprise_core::Hex::new(6, 0));
    s.place_town(farm, a.center_x(), a.center_z());
    s.place_town(mill, b.center_x(), b.center_z());
    s.place_town(crowd, 999_000.5, 999_000.5);
    s.set_trade(farm, 0, 128.0, 0.0);
    s.set_trade(mill, 0, 0.0, 64.0);
    s.deliver(farm, 0, 640.0);
    let road = s.add_road(farm, mill, 1.0).expect("a road");
    let middle: Vec<_> = s.route_path(road).unwrap().into_iter().filter(|h| h.distance(a) >= 2 && h.distance(b) >= 2).collect();
    for &h in &middle {
        s.claim_hex(crowd, h);
    }
    let money = s.town(crowd).money.to_f64();
    for d in 1..=10 {
        s.wear_hexes(&middle, 20.0);
        s.advance(d * DAY);
    }
    let fees: f64 = s.cash_book(crowd, 30).iter().filter(|(_, w, _)| w == "congestion fees").map(|(_, _, n)| n.to_f64()).sum();
    assert!(s.route(road).trips >= 1 && fees > 0.0, "Crowdham took its congestion fees: {} trips, {fees}", s.route(road).trips);
    assert!(s.town(crowd).money.to_f64() >= money + fees - 1e-6);
}
