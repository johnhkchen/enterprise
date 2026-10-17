//! Territory: a town claims its home hex and grows by culture to two rings; a held hex stays with its owner; guards
//! eat and are paid or desert; hands stand watch for their food; wagons pay tolls to the posted land they cross.

use enterprise_core::{Content, Good, Hex, Society};

const DAY: u64 = 24_000;
const BREAD: usize = 0;

fn content() -> Content {
    Content::new(vec![Good::new("bread", 0.2, 64).food(5.0), Good::new("wheat", 0.05, 64)], DAY)
}

/// A town of five free villagers standing in the hex at (x, z), its mood held at 0 (as a fresh holding's is), so its
/// census value is 5 and its culture factor 1.
fn town_of_five(s: &mut Society, name: &str, x: f64, z: f64) -> usize {
    let t = s.add_town(name, 0.0);
    s.set_people(t, 5.0, 5.0, 5.0);
    s.place_town(t, x, z);
    t
}

fn society() -> Society {
    let mut s = Society::new(content());
    s.set_dial("villagers_per_want", 1e300).unwrap();
    s
}

// parity: TerritoryTests.homeHexFirstThenGrowthWithCulture
#[test]
fn the_home_hex_first_then_growth_with_culture() {
    let mut s = society();
    let t = town_of_five(&mut s, "Meadowworth", 3.0, 3.0);
    // Its first count is a day's culture: home free, 5 culture buys the next hex (5 x 1 held).
    assert!(s.claim(t));
    let home = Hex::at(3.0, 3.0);
    assert_eq!(s.holding(t).hexes.len(), 2, "home plus one");
    assert_eq!(s.holding(t).hexes[0], home, "the home hex is the bell's");
    assert_eq!(s.holding(t).culture, 0.0);
    s.advance(2 * DAY);
    // 10 more culture buys a third hex (5 x 2 held).
    assert_eq!(s.holding(t).hexes.len(), 3, "a third hex after two more days");
}

#[test]
fn a_town_claims_its_home_at_its_first_look_without_being_asked() {
    let mut s = society();
    let t = town_of_five(&mut s, "Meadowworth", 3.0, 3.0);
    s.advance(DAY / 2);
    assert_eq!(s.holding(t).hexes[0], Hex::at(3.0, 3.0));
    assert_eq!(s.hex_owner(Hex::at(3.0, 3.0)), Some(t));
    assert_eq!(s.holding(t).hexes.len(), 2, "a first count is a day's culture, which buys a hex");
}

#[test]
fn culture_follows_mood_confinement_and_works() {
    use enterprise_core::territory::culture_factor;
    assert_eq!(culture_factor(0.0), 1.0);
    assert_eq!(culture_factor(2.0), 1.5, "a quarter more a point");
    assert_eq!(culture_factor(10.0), 2.0, "at most double");
    assert_eq!(culture_factor(-10.0), 0.25, "at least a quarter");
    // Five villagers, two confined (a tenth each): 3.2 a day; with a wall (2), a library (3) and a caravanserai (1).
    let mut s = society();
    let t = town_of_five(&mut s, "Meadowworth", 3.0, 3.0);
    s.set_confined(t, 2);
    s.claim(t);
    assert!((s.holding(t).culture - 3.2).abs() < 1e-9, "a first count: {}", s.holding(t).culture);
    s.advance(DAY);
    assert_eq!(s.holding(t).hexes.len(), 2, "6.4 buys one hex (5)");
    assert!((s.holding(t).culture - 1.4).abs() < 1e-9, "{}", s.holding(t).culture);
    s.set_culture_extras(t, 6.0);
    s.advance(3 * DAY);
    assert_eq!(s.holding(t).hexes.len(), 3, "1.4 + 2 x 9.2 = 19.8 buys a second (10), not a third (15)");
    assert!((s.holding(t).culture - 9.8).abs() < 1e-9, "{}", s.holding(t).culture);
}

// parity: TerritoryTests.growthStopsAtTwoRings
#[test]
fn growth_stops_at_two_rings() {
    let mut s = society();
    let t = town_of_five(&mut s, "Meadowworth", 3.0, 3.0);
    s.claim(t);
    s.advance(1000 * DAY);
    let home = Hex::at(3.0, 3.0);
    let h = s.holding(t);
    assert_eq!(h.hexes.len(), 19, "19 hexes at most");
    assert!(h.hexes.iter().all(|x| x.distance(home) <= 2), "all within two rings");
    let mut distinct = h.hexes.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(distinct.len(), 19);
}

#[test]
fn the_rings_are_a_dial() {
    let mut s = society();
    s.set_dial("max_rings", 1.0).unwrap();
    let t = town_of_five(&mut s, "Meadowworth", 3.0, 3.0);
    s.claim(t);
    s.advance(1000 * DAY);
    assert_eq!(s.holding(t).hexes.len(), 7, "home and the first ring");
}

// parity: TerritoryTests.aHeldHexStaysWithItsOwner
#[test]
fn a_held_hex_stays_with_its_owner() {
    let mut s = society();
    let t = town_of_five(&mut s, "Meadowworth", 3.0, 3.0);
    let rival = s.add_town("Rival", 0.0);
    let home = Hex::at(3.0, 3.0);
    for hex in home.neighbors() {
        assert!(s.claim_hex(rival, hex), "the rival already holds the whole first ring");
    }
    s.claim(t);
    s.advance(1000 * DAY);
    assert_eq!(s.holding(t).hexes, [home], "hemmed in, it keeps only its home hex");
    assert!(!s.claim_hex(t, home.neighbors()[0]), "and can't take theirs");
}

/// A town of five with its home and one more hex, `bread` bread and `emeralds` emeralds, `guards` on its posts.
fn garrison(bread: f64, emeralds: f64, guards: usize) -> (Society, usize) {
    let mut s = society();
    let t = s.add_town("Farm", emeralds);
    s.set_people(t, 0.0, 0.0, 0.0);
    s.place_town(t, 3.0, 3.0);
    s.claim(t);
    s.claim_hex(t, Hex::at(3.0, 3.0).neighbors()[0]);
    s.claim_hex(t, Hex::at(3.0, 3.0).neighbors()[1]);
    s.claim_hex(t, Hex::at(3.0, 3.0).neighbors()[2]);
    s.deliver(t, BREAD, bread);
    s.set_guards(t, guards);
    (s, t)
}

// parity: TerritoryTests.fedAndPaidGuardsStay
#[test]
fn fed_and_paid_guards_stay() {
    let (mut s, t) = garrison(64.0, 64.0, 2);
    s.advance(5 * DAY);
    assert_eq!(s.deserted(t), 0, "fed and paid, nobody deserts");
    assert_eq!(s.guards(t), 2);
    // 2 guards x 6 food x 5 days = 60 food points, 12 bread.
    assert_eq!(s.town(t).stock[BREAD], 64.0 - 12.0, "they ate 12 bread");
    assert_eq!(s.town(t).money, 64.0 - 5.0, "and were paid 2 x 0.5 x 5");
}

// parity: TerritoryTests.unfedGuardsDesert
#[test]
fn unfed_guards_desert() {
    let (mut s, t) = garrison(0.0, 64.0, 4);
    s.advance(3 * DAY);
    assert!(s.guards(t) < 4 && s.deserted(t) > 0, "hungry guards desert: {} left", s.guards(t));
    assert_eq!(s.guards(t) as u32 + s.deserted(t), 4);
}

#[test]
fn unpaid_guards_desert_unless_their_post_takes_tolls() {
    // No money and nothing coming in: the guards go.
    let (mut s, t) = garrison(64.0, 0.0, 3);
    s.advance(2 * DAY);
    assert_eq!(s.guards(t), 0, "unpaid and earning nothing, they go");
    // The same, with tolls taken lately and within three days' pay owed: they wait.
    let (mut s, t) = garrison(640.0, 0.0, 3);
    s.world.resource_mut::<enterprise_core::territory::Territory>().holdings.get_mut(&t).unwrap().toll_today = 1.0;
    s.advance(2 * DAY);
    assert_eq!(s.guards(t), 3, "on credit, up to 3 days x 3 guards x 0.5");
    s.advance(10 * DAY);
    assert_eq!(s.guards(t), 0, "but not for ever");
}

#[test]
fn the_guards_posted_first_stay_longest() {
    // Four guards are owed 2 a day; with 1 to pay them, half stay: the first two posted, by q then r.
    let (mut s, t) = garrison(64.0, 1.0, 4);
    let mut posts = s.holding(t).hexes;
    posts.sort_by_key(|x| (x.q, x.r));
    assert_eq!(s.holding(t).posts, posts, "posted by q, then r");
    s.advance(DAY);
    assert_eq!(s.holding(t).posts, posts[..2], "the earliest posted stay");
    assert_eq!(s.deserted(t), 2);
}

#[test]
fn guards_eat_a_full_meal_or_nothing() {
    // 2 bread (10 points) against the 24 four guards eat a day: the warehouse feeds none, and keeps its bread.
    let (mut s, t) = garrison(2.0, 64.0, 4);
    s.advance(DAY);
    assert_eq!(s.guards(t), 0, "all four go");
    assert_eq!(s.town(t).stock[BREAD], 2.0, "and the bread stays");
}

// parity: LabourTests.handsStandWatchForTheirFood
#[test]
fn hands_stand_watch_for_their_food() {
    let (mut s, t) = garrison(10.0, 10.0, 1);
    s.set_roster(t, &[("nitwit", 1)]);
    s.advance(2 * DAY);
    assert_eq!(s.militia(t), 1, "the Nitwit stands watch");
    assert_eq!(s.town(t).money, 10.0, "unpaid");
    assert!(s.town(t).stock[BREAD] < 10.0, "but fed");
    assert_eq!(s.guards(t), 1, "and stays");
    // A hand with a trade doesn't.
    let (mut s, t) = garrison(10.0, 10.0, 1);
    s.set_roster(t, &[("farmer", 1)]);
    s.advance(2 * DAY);
    assert_eq!(s.militia(t), 0);
    assert_eq!(s.town(t).money, 10.0 - 1.0, "paid in full, 0.5 x 2 days");
}

// parity: TerritoryTests.caravansPayTollsOnlyToLandTheyCross
#[test]
fn caravans_pay_tolls_only_to_land_they_cross() {
    let mut s = society();
    // A route running 1,000 blocks east; two other settlements hold hexes on the way.
    let from = s.add_town("From", 0.0);
    let to = s.add_town("To", 0.0);
    let first = s.add_town("First", 0.0);
    let second = s.add_town("Second", 0.0);
    let off = s.add_town("Off", 0.0);
    let path = Hex::line(Hex::at(0.0, 0.0), Hex::at(1000.0, 0.0));
    s.claim_hex(first, Hex::at(300.0, 0.0));
    s.claim_hex(first, Hex::at(380.0, 0.0)); // two hexes, one toll
    s.claim_hex(second, Hex::at(700.0, 0.0));
    s.claim_hex(off, Hex::at(500.0, 900.0));
    s.set_guards(first, 2); // every hex they hold is guarded: the full 5% is collected
    s.set_guards(second, 1);
    s.set_guards(off, 1);
    let due = s.tolls_for(from, to, &path, 1000.0);
    assert_eq!(due, [(first, 50.0), (second, 47.0)], "5% to the first (50), then 5% of the rest to the second (47)");
    assert_eq!(due.iter().map(|(_, t)| t).sum::<f64>(), 97.0);
    // The route's own ends charge nothing.
    s.claim_hex(from, Hex::at(150.0, 0.0));
    s.set_guards(from, 1);
    assert_eq!(s.tolls_for(from, to, &path, 1000.0), due);
    // Unguarded land takes none; a town's own rate and fee are its own.
    s.set_guards(first, 0);
    assert_eq!(s.tolls_for(from, to, &path, 1000.0), [(second, 50.0)]);
    s.set_toll(second, 0.1);
    s.set_passage_fee(second, 3.0);
    assert_eq!(s.tolls_for(from, to, &path, 1000.0), [(second, 103.0)]);
    assert_eq!(s.tolls_for(from, to, &path, 10.0), [(second, 4.0)]);
    assert_eq!(s.tolls_for(from, to, &path, 2.0), [(second, 2.0)], "never more than it carries");
    s.set_toll(second, 9.0);
    assert_eq!(s.toll_rate(second), 0.25, "at most a quarter");
}

#[test]
fn a_wagon_home_pays_the_posts_on_its_road_out_of_its_purse() {
    let mut s = Society::new(Content::new(vec![Good::new("wheat", 0.05, 64), Good::new("bread", 0.2, 64).food(5.0)], DAY));
    s.content_mut().dials.standing_orders = false;
    let farm = s.add_town("Wheatfield", 20.0);
    let mill = s.add_town("Hayford", 500.0);
    let keeper = s.add_town("Tollgate", 100.0);
    s.place_town(farm, 0.0, 0.0);
    s.place_town(mill, 1000.0, 0.0);
    s.set_trade(farm, 0, 128.0, 0.0);
    s.set_trade(mill, 0, 0.0, 64.0);
    let route = s.add_road(farm, mill, 2.0).expect("a road");
    let path = s.route_path(route).unwrap();
    // The keeper holds two hexes on the road, guarded and fed.
    assert!(s.claim_hex(keeper, path[4]) && s.claim_hex(keeper, path[5]));
    s.set_guards(keeper, 2);
    s.deliver(keeper, 1, 1000.0);
    s.set_toll(keeper, 0.1);
    let start = s.totals();
    for day in 1..=60 {
        s.advance(day * DAY);
        s.conserved(&start).unwrap_or_else(|e| panic!("day {day}: {e}"));
        s.solvent().unwrap_or_else(|e| panic!("day {day}: {e}"));
    }
    assert!(s.route(route).trips > 5, "wagons came home");
    assert!(s.holding(keeper).tolls_taken > 0.0, "the keeper took tolls");
    let book = s.cash_book(keeper, 60);
    assert!(book.iter().any(|(_, why, amount)| why == "toll" && *amount > 0.0), "in its cash book: {book:?}");
    assert_eq!(s.guards(keeper), 2, "and paid and fed its guards");
    // The ends take none from their own wagons.
    assert_eq!(s.holding(farm).tolls_taken + s.holding(mill).tolls_taken, 0.0);
}

#[test]
fn a_town_keeps_its_land_and_culture_in_the_save() {
    let mut s = society();
    let t = town_of_five(&mut s, "Meadowworth", 3.0, 3.0);
    s.claim(t);
    s.advance(40 * DAY);
    s.set_toll(t, 0.12);
    let before = s.holding(t);
    assert!(before.hexes.len() > 3);
    let bytes = s.save();
    let loaded = Society::load(&bytes).unwrap();
    assert_eq!(loaded.holding(t), before);
    assert_eq!(loaded.hex_owner(before.hexes[2]), Some(t));
}
