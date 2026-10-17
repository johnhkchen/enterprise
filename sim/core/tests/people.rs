//! A town's people, pinned as the mod's growth and comfort tests pin them: wages, comforts, mood, homes, children.

use enterprise_core::{Content, Good, Society};

const DAY: u64 = 24_000;

/// One good of each kind of comfort (worth 0.05 each), wheat the town grows, and bread.
fn content() -> Content {
    let mut goods: Vec<Good> = Content::COMFORTS.iter().enumerate()
        .map(|(k, name)| Good::new(&format!("{name}_good"), 0.05, 64).comfort(k)).collect();
    goods.push(Good::new("wheat", 0.05, 64));
    goods.push(Good::new("bread", 0.2, 64).food(5.0));
    Content::new(goods, DAY)
}

const WHEAT: usize = 5;
const BREAD: usize = 6;

/// A town of `villagers` in `beds`, its land holding `land` beds, growing wheat (its wages come of it), with
/// `comfort_days` days of every comfort in its warehouse, bread, and `money`.
fn town(villagers: f64, beds: f64, land: f64, comfort_days: f64, money: f64) -> Society {
    let mut s = Society::new(content());
    s.content_mut().dials.standing_orders = false;
    let t = s.add_town("Meadowworth", money);
    s.set_people(t, villagers, beds, land);
    s.set_trade(t, WHEAT, 128.0, 0.0);
    for k in 0..5 {
        s.deliver(t, k, (villagers * 0.1 * comfort_days / 0.05).ceil());
    }
    s.deliver(t, BREAD, 64.0);
    s
}

// parity: MoodTests.comfortsLiftTheMood
#[test]
fn comforts_met_lift_mood_toward_their_count_less_the_towns_size() {
    let mut s = town(8.0, 8.0, 8.0, 30.0, 20.0);
    s.advance(20 * DAY);
    let p = s.town(0).people;
    assert!(p.comfort_met.iter().all(|m| (m - 1.0).abs() < 1e-9), "every kind met: {:?}", p.comfort_met);
    // Five kinds met, a point for every eight villagers: heading for 4.
    assert!((p.mood - 4.0).abs() < 0.1, "mood {}", p.mood);
}

// parity-partial: MoodTests.crowdingAndUnpaidWagesSour
#[test]
fn a_town_without_comforts_sinks_and_crowding_sinks_it_further() {
    let mut s = town(10.0, 6.0, 6.0, 0.0, 200.0); // money enough for 20 days of wages
    s.advance(20 * DAY);
    let p = s.town(0).people;
    // Nothing met, 10/8 for its size, half a point for each of four without beds: heading for -3.25.
    assert!((p.mood + 3.25).abs() < 0.1, "mood {}", p.mood);
}

// parity-partial: MoodTests.crowdingAndUnpaidWagesSour
#[test]
fn unpaid_wages_cost_two_points() {
    let mut s = town(8.0, 8.0, 8.0, 30.0, 0.0);
    s.content_mut().dials.labour_share = 100.0; // wages far beyond what the town takes in
    s.advance(20 * DAY);
    let p = s.town(0).people;
    assert!(p.paid_share < 0.2, "barely paid: {}", p.paid_share);
    assert!(p.mood < 2.5, "mood {}", p.mood);
}

// parity: MoodTests.villagersUseTheirRations
#[test]
fn the_households_pay_for_the_comforts_they_use() {
    let mut s = town(8.0, 8.0, 8.0, 30.0, 20.0);
    s.advance(DAY);
    let p = s.town(0).people;
    // A day's wages: half the worth of 128 wheat, 3.2 emeralds; a day's comforts: 8 x 0.1 x 5 kinds, 4 emeralds.
    assert!(p.purse < 3.2, "they spent their wages on comforts: {} left", p.purse);
}

// parity: GrowthTests.aContentTownWithASpareBedHasAChild
#[test]
fn a_content_town_builds_homes_and_has_children() {
    let mut s = town(6.0, 6.0, 40.0, 60.0, 60.0);
    s.deliver(0, BREAD, 640.0);
    s.advance(60 * DAY);
    let p = s.town(0).people;
    assert!(p.homes_built >= 2, "homes as its beds filled: {}", p.homes_built);
    assert!(p.born >= 2 && p.villagers >= 8.0, "children in the new beds: {} born, {} villagers", p.born, p.villagers);
}

#[test]
fn its_land_is_the_most_it_can_grow_to() {
    let mut s = town(6.0, 6.0, 10.0, 200.0, 200.0);
    s.deliver(0, BREAD, 6400.0);
    s.advance(200 * DAY);
    let p = s.town(0).people;
    assert!(p.beds <= 10.0 && p.villagers <= 10.0, "no more than its land holds: {} beds, {} villagers", p.beds, p.villagers);
    assert_eq!(p.beds, 10.0, "but up to it");
}

// parity: CaravanTests.townsBuyComfortsUpToTheirPar
#[test]
fn wagons_bring_comforts_where_they_are_short() {
    let mut s = Society::new(content());
    s.content_mut().dials.standing_orders = false;
    let maker = s.add_town("Hollowwick", 20.0);
    let short = s.add_town("Cloverby", 40.0);
    s.set_people(maker, 4.0, 4.0, 4.0);
    s.set_people(short, 8.0, 8.0, 8.0);
    s.set_trade(maker, 0, 64.0, 0.0); // it makes treats
    s.add_route(maker, short, 1.0, 0.2, 2.0);
    s.advance(20 * DAY);
    let p = s.town(short).people;
    assert!(p.comfort_met[0] > 0.5, "treats reached the town that makes none: {:?}", p.comfort_met);
}

// parity: GrowthTests.anUnhappyTownHasNoChildren
#[test]
fn an_unhappy_town_has_no_children() {
    let mut s = town(6.0, 10.0, 40.0, 0.0, 200.0); // a spare bed, no comforts: its mood sinks below zero
    s.deliver(0, BREAD, 640.0);
    s.advance(60 * DAY);
    assert_eq!(s.town(0).people.born, 0, "no children: mood {}", s.town(0).people.mood);
}
