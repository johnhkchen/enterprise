//! Services (`services.rs`, `rules/services.rs`), pinned as the mod's service, metal and caravan tests pin them: a
//! town of five, its trades at level 2 (`ServiceTests.town`), goods at the mod's prices (`Prices.java`: items per
//! emerald, so values are their inverses), and the caravan tests' network (the hub and its far end four blocks apart,
//! the hub's Merchant at its desk and a squad of trained hands).

use enterprise_core::services::{kit_share, Where};
use enterprise_core::{Content, Good, Society};

const DAY: u64 = 24_000;
/// A look: the core looks every 1,200 ticks.
const LOOK: u64 = 1_200;

const WHEAT: usize = 0;
const PAPER: usize = 1;
const LEATHER: usize = 2;
const BOOK: usize = 3;
const RAW_IRON: usize = 4;
const COAL: usize = 5;
const IRON: usize = 6;
const HOE: usize = 7;
const PICKAXE: usize = 8;
const BREAD: usize = 9;
const WOOL: usize = 10;
const STICK: usize = 11;
const BANNER: usize = 12;
const SWORD: usize = 13;

fn content() -> Content {
    Content::new(vec![
        Good::new("wheat", 1.0 / 20.0, 64),
        Good::new("paper", 1.0 / 24.0, 64),
        Good::new("leather", 1.0 / 6.0, 64),
        Good::new("book", 1.0 / 4.0, 64).comfort(1),
        Good::new("raw_iron", 1.0 / 6.0, 64),
        Good::new("coal", 1.0 / 12.0, 64),
        Good::new("iron_ingot", 1.0 / 3.0, 64),
        Good::new("iron_hoe", 1.0, 1),
        Good::new("iron_pickaxe", 1.5, 1),
        Good::new("bread", 1.0 / 6.0, 64).food(5.0),
        Good::new("white_wool", 1.0 / 16.0, 64),
        Good::new("stick", 1.0 / 32.0, 64),
        Good::new("white_banner", 1.0, 16).comfort(3),
        Good::new("iron_sword", 1.0, 1),
    ], DAY)
}

/// A town of five (`ServiceTests.town`): residents of these trades at level 2, the rest waiting for work.
fn town(s: &mut Society, name: &str, trades: &[&str]) -> usize {
    let t = s.add_town(name, 0.0);
    let mut hands: Vec<(&str, u8)> = trades.iter().map(|&tr| (tr, 2)).collect();
    while hands.len() < 5 {
        hands.push(("none", 1));
    }
    s.set_roster(t, &hands);
    t
}

/// A town of five with no roster of its own (the mod's census only): no trade at home.
fn bare(s: &mut Society, name: &str, money: f64) -> usize {
    let t = s.add_town(name, money);
    s.set_people(t, 5.0, 5.0, 5.0);
    t
}

fn stock(s: &Society, t: usize, g: usize) -> f64 {
    s.town(t).stock[g]
}

// parity: ServiceTests.everyTradeSaysWhatItDoes
#[test]
fn every_trade_says_what_it_does() {
    let s = Society::new(content());
    assert!(s.services().iter().all(|v| !v.does.trim().is_empty()), "each service says what it does");
    for trade in ["merchant", "cartographer", "librarian", "shepherd", "leatherworker", "fisherman", "cleric", "farmer",
        "toolsmith", "armorer", "weaponsmith"] {
        assert!(!s.services_of(trade).is_empty(), "{trade} does something");
    }
    // Each recipe at home, and abroad for a town without the trade.
    let names: Vec<String> = s.services().into_iter().map(|v| v.name).collect();
    for n in ["bookbinding", "bookbinding_abroad", "smelting", "smelting_abroad", "hoes", "hoes_abroad", "banners"] {
        assert!(names.contains(&n.to_string()), "{n} is registered: {names:?}");
    }
    let swap = s.services().into_iter().find(|v| v.name == "map_swap").unwrap();
    assert_eq!((swap.trade.as_str(), swap.where_), ("cartographer", Where::Abroad));
}

#[test]
fn a_recipe_needs_every_good_it_uses() {
    let s = Society::new(Content::new(vec![Good::new("paper", 1.0 / 24.0, 64), Good::new("book", 0.25, 64)], DAY));
    let names: Vec<String> = s.services().into_iter().map(|v| v.name).collect();
    assert!(!names.iter().any(|n| n.starts_with("bookbinding")), "no leather, no bookbinding: {names:?}");
    assert!(names.contains(&"blessing".to_string()), "a Cleric blesses whatever the goods");
}

// parity: ServiceTests.aClericAtHomeBlessesItsTown
#[test]
fn a_cleric_at_home_blesses_its_town() {
    let mut s = Society::new(content());
    let t = town(&mut s, "Town", &["cleric"]);
    assert_eq!(s.blessed_until(t), None);
    s.advance(LOOK);
    assert!(s.blessed_until(t).is_some_and(|u| u > LOOK), "blessed while the Cleric is home: {:?}", s.blessed_until(t));
    assert_eq!(s.blessed_until(t), Some(LOOK + DAY), "a day at a time");
    // While blessed, its mood heads half a point higher.
    let target = s.mood_target(t);
    s.advance(DAY);
    assert!(s.blessed_until(t).unwrap() >= DAY, "and again while it stays");
    let mut plain = Society::new(content());
    let p = town(&mut plain, "Plain", &["librarian"]);
    plain.advance(LOOK);
    assert!((target - plain.mood_target(p) - 0.5).abs() < 1e-9, "+0.5 where its mood heads: {target}");
}

// parity: ServiceTests.aLibrarianBindsBooksWhileReadingIsShort
#[test]
fn a_librarian_binds_books_while_reading_is_short() {
    for workshop in [true, false] {
        let mut s = Society::new(content());
        if !workshop {
            s.content_mut().conversions.clear(); // the bookbinding service alone, not the workshop's conversion
        }
        let t = town(&mut s, "Town", &["librarian"]);
        s.deliver(t, PAPER, 9.0);
        s.deliver(t, LEATHER, 5.0);
        s.advance(LOOK);
        assert_eq!(stock(&s, t, BOOK), 3.0, "three books from nine paper");
        assert_eq!(stock(&s, t, LEATHER), 2.0, "and three leather");
        assert_eq!(stock(&s, t, PAPER), 0.0);
        if !workshop {
            assert_eq!(s.times_done("bookbinding", t, t), 1, "the Librarian bound them: {:?}", s.done(t));
        }
    }
}

#[test]
fn a_librarian_binds_no_more_than_reading_wants() {
    let mut s = Society::new(content());
    s.content_mut().conversions.clear();
    let t = town(&mut s, "Town", &["librarian"]);
    s.deliver(t, PAPER, 90.0);
    s.deliver(t, LEATHER, 30.0);
    s.deliver(t, BOOK, 10.0); // five of six days' reading held: 3 emeralds' par, 2.5 held
    s.advance(LOOK);
    assert_eq!(stock(&s, t, BOOK), 12.0, "up to its par: two more books");
}

// parity: ServiceTests.aVisitingFarmerBringsInTheUntendedHarvest
#[test]
fn a_visiting_farmer_brings_in_the_untended_harvest() {
    let mut s = Society::new(content());
    let home = town(&mut s, "Home", &["merchant", "farmer"]);
    let at = bare(&mut s, "Town", 0.0);
    s.set_plots(at, WHEAT, 80); // 80 plots no one tends
    assert_eq!(s.farm(at).unwrap().farm.idle.get(&WHEAT).copied(), Some(40.0), "40 wheat a day lie in the fields");
    let route = s.add_route(home, at, 1.0, 0.0, 1.0);
    assert!(!s.service_wanted("harvest_help", home, at, 1, true, 0.0, Some(route)),
        "home's own fields can't spare its Farmer");
    s.set_idle(home, &[], 32); // a Farmer's plots to spare at home
    assert!(s.service_wanted("harvest_help", home, at, 1, true, 0.0, Some(route)),
        "fields no one tends: a Farmer is wanted there");
    let done = s.do_service("harvest_help", home, at, 1, 10.0, None).unwrap();
    assert_eq!(s.owed_produce(at).get(&WHEAT).copied(), Some(16.0), "a Novice tends 32 plots for a day: 16 wheat");
    s.advance(LOOK);
    assert_eq!(stock(&s, at, WHEAT), 16.0, "handed over with the harvest");
    assert!(done.fee > 0.0 && done.fee < 16.0 / 20.0, "for a farmhand's share of its worth: {}", done.fee);
    assert!((done.fee - 16.0 / 20.0 * 0.5).abs() < 1e-12, "half its worth, the farmhands' share");
}

#[test]
fn a_farmer_helps_only_as_far_as_the_town_can_pay() {
    let mut s = Society::new(content());
    let home = town(&mut s, "Home", &["farmer"]);
    let at = bare(&mut s, "Town", 0.0);
    s.set_plots(at, WHEAT, 80);
    assert!(!s.service_wanted("harvest_help", home, at, 1, false, 0.0, None), "nothing to pay with");
    let done = s.do_service("harvest_help", home, at, 2, 0.2, None).unwrap();
    assert_eq!(done.fee, 0.2, "all it can pay");
    assert!((s.owed_produce(at)[&WHEAT] - 32.0 * 0.2 / 0.8).abs() < 1e-9, "and the harvest that buys: 8 of 32 wheat");
}

// parity: ServiceTests.aMapSwapTeachesBothTowns
#[test]
fn a_map_swap_teaches_both_towns() {
    let mut s = Society::new(content());
    let at = bare(&mut s, "Town", 0.0);
    let home = town(&mut s, "Home", &["cartographer"]);
    let ours = s.add_town("Ours", 0.0);
    let theirs = s.add_town("Theirs", 0.0);
    s.learn(home, ours);
    s.learn(at, theirs);
    assert!(s.service_wanted("map_swap", home, at, 1, false, 10.0, None), "they know different markets: worth a swap");
    let done = s.do_service("map_swap", home, at, 1, 10.0, None).unwrap();
    assert!(s.known(at).contains(&ours), "they learn of ours");
    assert!(done.heard.contains(&theirs), "and we carry home theirs: {:?}", done.heard);
    assert!(!s.known(home).contains(&theirs), "not before the Cartographer is home");
    assert_eq!(done.fee, 0.0, "each taught the other two: nobody pays");
    assert!(s.service_wanted("map_swap", home, at, 1, false, 10.0, None), "till our copy is home, home knows less");
}

#[test]
fn the_town_that_learns_more_pays_a_quarter_a_market() {
    let mut s = Society::new(content());
    let at = bare(&mut s, "Town", 0.0);
    let home = town(&mut s, "Home", &["cartographer"]);
    for name in ["A", "B", "C"] {
        let m = s.add_town(name, 0.0);
        s.learn(home, m);
    }
    let done = s.do_service("map_swap", home, at, 1, 10.0, None).unwrap();
    assert_eq!(done.fee, 0.25 * 3.0, "it learns home and three more, and teaches only itself");
    assert_eq!(done.heard, vec![at], "our copy of its map: itself");
}

// parity-partial: MetalTests.anArmorerSmeltsAndAToolsmithForgesHoes (the hoe share reckoned from the stock here, not a
// territory kit share kept at each touch: mines-and-kit's)
#[test]
fn an_armorer_smelts_and_a_toolsmith_forges_hoes() {
    let mut s = Society::new(content());
    let t = town(&mut s, "Town", &["toolsmith", "armorer", "farmer", "farmer"]);
    s.deliver(t, RAW_IRON, 16.0);
    s.deliver(t, COAL, 2.0);
    s.advance(LOOK);
    assert_eq!(stock(&s, t, HOE), 2.0, "a hoe for each farmer");
    assert_eq!(stock(&s, t, IRON), 12.0, "of the 16 iron smelted");
    assert_eq!(stock(&s, t, PICKAXE), 0.0, "no mine, no miners: no pickaxes");
    assert_eq!(kit_share(stock(&s, t, HOE), 2), 1.0, "every farmer has tools");
    s.advance(2 * LOOK);
    assert_eq!((stock(&s, t, HOE), stock(&s, t, IRON)), (2.0, 12.0), "and no more while they have them");
}

// parity: MetalTests.aVisitingArmorerSmeltsForATownWithoutOne
#[test]
fn a_visiting_armorer_smelts_for_a_town_without_one() {
    let mut s = Society::new(content());
    let at = bare(&mut s, "Town", 0.0);
    let home = town(&mut s, "Home", &["armorer"]);
    s.deliver(at, RAW_IRON, 16.0);
    s.deliver(at, COAL, 2.0);
    assert!(s.service_wanted("smelting_abroad", home, at, 1, false, 5.0, None), "ore and no Armorer: wanted");
    let done = s.do_service("smelting_abroad", home, at, 1, 5.0, None).unwrap();
    assert_eq!(stock(&s, at, IRON), 16.0, "all 16 smelted");
    assert!((done.fee - 0.32).abs() < 1e-9, "for 0.02 an ingot: {}", done.fee);
    // A town with an Armorer of its own smelts for itself.
    assert!(!s.service_wanted("smelting_abroad", at, home, 1, false, 5.0, None));
}

#[test]
fn what_a_town_buys_abroad_is_what_it_can_pay_for() {
    let mut s = Society::new(content());
    let at = bare(&mut s, "Town", 0.0);
    let home = town(&mut s, "Home", &["armorer"]);
    s.deliver(at, RAW_IRON, 16.0);
    s.deliver(at, COAL, 2.0);
    let done = s.do_service("smelting_abroad", home, at, 1, 0.2, None).unwrap();
    assert_eq!(stock(&s, at, IRON), 8.0, "one batch of eight for 0.16: two cost 0.32");
    assert!((done.fee - 0.16).abs() < 1e-9);
}

#[test]
fn a_master_weaves_banners_where_a_novice_cant() {
    let mut s = Society::new(content());
    let at = bare(&mut s, "Town", 0.0);
    let home = town(&mut s, "Home", &["shepherd"]);
    s.deliver(at, WOOL, 12.0);
    s.deliver(at, STICK, 2.0);
    assert!(!s.service_wanted("banners_abroad", home, at, 4, false, 5.0, None), "banners take a master");
    let done = s.do_service("banners_abroad", home, at, 5, 5.0, None).unwrap();
    assert_eq!(stock(&s, at, BANNER), 2.0, "two banners from 12 wool and 2 sticks");
    assert!((done.fee - 0.2).abs() < 1e-9);
}

#[test]
fn kit_is_made_for_what_a_town_has_contracted_to_sell_too() {
    let mut s = Society::new(content());
    let t = town(&mut s, "Town", &["weaponsmith"]);
    let buyer = bare(&mut s, "Buyer", 0.0);
    s.deliver(t, IRON, 20.0);
    let id = s.draft(t, buyer, SWORD, 3.0, 1.0, 10 * DAY, DAY, 1, enterprise_core::Incoterm::Ddp, true);
    s.advance(LOOK);
    assert_eq!(stock(&s, t, SWORD), 3.0, "no guards: only the three it owes {:?}", s.contract(id).state);
    assert_eq!(stock(&s, t, IRON), 14.0);
}

/// The mod's caravan test network (`CaravanTests.network`): a hub with 128 paper, `bread` and `emeralds`, its Merchant
/// at the desk and a squad of `squad` (trained at level 2: no locker); a far end of five four blocks away holding
/// `dest_spurs` (16 an emerald) and two bread, whose lockers take paper up to a stack.
fn network(bread: f64, emeralds: f64, dest_spurs: f64, squad: &[&str]) -> Society {
    let mut s = Society::new(content());
    s.content_mut().dials.standing_orders = false;
    let hub = s.add_town("Hub", emeralds);
    let dest = s.add_town("Dest", dest_spurs / 16.0);
    s.set_people(dest, 5.0, 5.0, 5.0);
    s.record_capacity(dest, 1, &[(PAPER, 64.0 / 5.0)]);
    s.set_trade(dest, PAPER, 0.0, 1e-9);
    s.deliver(hub, PAPER, 128.0);
    if bread > 0.0 {
        s.deliver(hub, BREAD, bread);
    }
    s.deliver(dest, BREAD, 2.0);
    let mut hands: Vec<(&str, u8)> = vec![("merchant", 1)];
    hands.extend(squad.iter().map(|&t| (t, 2)));
    s.set_roster(hub, &hands);
    for i in 1..hands.len() {
        s.set_offers(hub, i, Vec::new()); // trained: it keeps its trade without a workstation
    }
    s.add_route(hub, dest, 4.0 / 1200.0, 0.0, 4.0);
    s
}

// parity: CaravanTests.aVisitingClericBlessesATownWithoutOne
#[test]
fn a_visiting_cleric_blesses_a_town_without_one() {
    let mut s = network(70.0, 5.0, 192.0, &["librarian", "cleric"]);
    s.pay(1, 10.0); // enough to pay for a blessing
    let trust = s.trust(0, 1);
    s.advance(LOOK);
    assert!(s.crew(0).crew.iter().any(|(t, _)| t == "cleric"), "a town without a Cleric: the Cleric goes too {:?}",
        s.crew(0));
    s.advance(2 * LOOK);
    assert!(s.blessed_until(1).is_some_and(|u| u > LOOK + DAY), "the town is blessed for days: {:?}", s.blessed_until(1));
    assert_eq!(s.blessed_until(1), Some(2 * LOOK + 4 * DAY), "two days and a day a level");
    let (_, _, _, paid) = s.route_services(0);
    assert_eq!(paid, 2.0, "and paid for it");
    assert!(s.trust(0, 1) > trust, "trust rises with a paid service");
    assert!(s.cash_book(1, 1).iter().any(|(_, why, n)| why == "service: blessing" && *n == -2.0),
        "out of its treasury: {:?}", s.cash_book(1, 1));
    s.advance(3 * LOOK);
    assert_eq!(s.route(0).trips, 1, "home");
    assert!(s.roster(0).hands.iter().all(|h| !h.away), "the Cleric's home too");
}

#[test]
fn a_cleric_stays_home_from_a_town_with_its_own() {
    let mut s = network(70.0, 5.0, 192.0, &["librarian", "cleric"]);
    s.set_roster(1, &[("cleric", 1), ("none", 1), ("none", 1), ("none", 1), ("none", 1)]);
    s.advance(LOOK);
    assert!(!s.crew(0).crew.iter().any(|(t, _)| t == "cleric"), "they bless themselves: {:?}", s.crew(0));
}

// parity-partial: CaravanTests.aShepherdSellsSheepWhereThePastureIsShort (the herd counted, not penned: pens and the
// animals let out by the bell are Minecraft's)
#[test]
fn a_shepherd_sells_sheep_where_the_pasture_is_short() {
    let mut s = network(2.0, 5.0, 192.0, &["shepherd"]);
    s.deliver(0, WHEAT, 64.0); // their feed
    assert!(s.set_herd(0, "sheep", 7.0));
    s.advance(LOOK);
    assert_eq!(s.wagon_herd(0), vec!["sheep"; 3], "she drives 3 sheep");
    assert!((4.0..5.0).contains(&s.herds(0).count_of(0)), "out of home's flock, its four to breed from kept: {:?}", s.herds(0));
    s.advance(2 * LOOK);
    let (_, _, sold, paid) = s.route_services(0);
    assert_eq!(sold, 3, "the town buys all three: its five want ten");
    assert!(paid == 3.0 * 0.75, "at a bargained price, between half and all of one each: {paid}");
    assert!((s.herds(1).count_of(0) - 3.0).abs() < 1e-6, "and they're its sheep now");
}

/// A Shepherd leaves the town its breeding stock: a flock of three stays home (a flock is seeded once).
#[test]
fn a_shepherd_leaves_the_breeding_stock_home() {
    let mut s = network(2.0, 5.0, 192.0, &["shepherd"]);
    s.deliver(0, WHEAT, 64.0);
    assert!(s.set_herd(0, "sheep", 3.0));
    s.advance(LOOK);
    assert!(s.wagon_herd(0).is_empty(), "none to spare: {:?}", s.wagon_herd(0));
    assert!(s.herds(0).count_of(0) >= 3.0, "the flock is home: {:?}", s.herds(0));
}

#[test]
fn unsold_animals_come_home() {
    let mut s = network(2.0, 5.0, 192.0, &["shepherd"]);
    s.deliver(0, WHEAT, 64.0);
    s.set_people(1, 1.0, 1.0, 1.0); // a town of one wants two
    assert!(s.set_herd(0, "sheep", 7.0));
    s.advance(LOOK);
    let kept = s.herds(0).count_of(0);
    s.advance(2 * LOOK);
    assert_eq!(s.route_services(0).2, 2, "two sold");
    s.advance(3 * LOOK);
    assert_eq!(s.route(0).trips, 1);
    let home = s.herds(0).count_of(0);
    assert!(home >= kept + 1.0, "the third is home with the four kept (and breeding, fed): {:?}", s.herds(0));
}

/// Two towns far apart on open ground (a road of 24 hexes), and a third a hex off the road, beyond word of mouth and
/// a Cartographer's charting of either.
fn road_and_side(squad: &[&str]) -> (Society, usize) {
    let mut s = Society::new(Content::new(vec![Good::new("glass", 0.5, 64)], DAY));
    s.content_mut().dials.standing_orders = false;
    let hub = s.add_town("Hub", 50.0);
    let dest = s.add_town("Dest", 400.0);
    let side = s.add_town("Side", 0.0);
    for (t, hex) in [(hub, enterprise_core::Hex::new(0, 0)), (dest, enterprise_core::Hex::new(24, 0)),
        (side, enterprise_core::Hex::new(12, 1))] {
        s.place_town(t, hex.center_x(), hex.center_z());
    }
    s.record_capacity(dest, 1, &[(0, 64.0 / 5.0)]);
    s.set_trade(dest, 0, 0.0, 64.0 / 5.0); // its people use a stack in five days
    s.deliver(hub, 0, 1024.0);
    let mut hands: Vec<(&str, u8)> = vec![("merchant", 1)];
    hands.extend(squad.iter().map(|&t| (t, 2)));
    s.set_roster(hub, &hands);
    for i in 1..hands.len() {
        s.set_offers(hub, i, Vec::new());
    }
    let route = s.add_road(hub, dest, 4.0).expect("a road over open ground");
    (s, route)
}

// parity-partial: CaravanTests.aCartographerBringsHomeAMapAndTheTownsOnTheWay (the towns along the road known at home;
// the map item is Minecraft's)
#[test]
fn a_cartographer_brings_home_the_towns_on_the_way() {
    let (mut s, route) = road_and_side(&["cartographer"]);
    let (hub, side) = (0, 2);
    s.advance(LOOK);
    assert!(s.crew(route).arrives.is_some(), "the wagon's away: {:?}", s.crew(route));
    assert!(!s.known(hub).contains(&side), "nobody's told the hub of it");
    let (ticks, _) = s.route_road(route);
    s.advance(LOOK + ticks + LOOK);
    assert!(s.route_services(route).0.is_some(), "the road is mapped");
    assert_eq!(s.route_services(route).1, 1, "one town along it");
    assert!(!s.known(hub).contains(&side), "not till the Cartographer's home");
    s.advance(LOOK + 2 * ticks + 2 * LOOK);
    assert_eq!(s.route(route).trips, 1);
    assert!(s.known(hub).contains(&side), "and the hub knows the town it saw: {:?}", s.known(hub));
}

#[test]
fn a_mapped_road_wants_no_cartographer_for_ten_days() {
    let (mut s, route) = road_and_side(&["merchant", "cartographer"]);
    assert!(s.service_wanted("survey", 0, 1, 2, true, 0.0, Some(route)), "never mapped");
    // Look by look for 30 days: each trip's crew as it leaves, and when the road was mapped.
    let mut trips: Vec<(u64, bool)> = Vec::new();
    let mut maps: Vec<u64> = Vec::new();
    let mut out = false;
    for look in 1..=(30 * DAY / LOOK) {
        s.advance(look * LOOK);
        let c = s.crew(route);
        if c.arrives.is_some() && !out {
            trips.push((look * LOOK, c.crew.iter().any(|(t, _)| t == "cartographer")));
        }
        out = c.arrives.is_some();
        if let Some(m) = s.route_services(route).0.filter(|m| maps.last() != Some(m)) {
            maps.push(m);
        }
    }
    assert!(trips.len() >= 4, "trips: {trips:?}");
    assert!(trips[0].1, "never mapped: she goes {trips:?}");
    assert!(trips.iter().any(|t| !t.1), "while it's mapped and both know the same, she stays home: {trips:?}");
    assert!(maps.len() >= 2, "mapped again once stale: {maps:?}");
    assert!(maps.windows(2).all(|w| w[1] - w[0] > 10 * DAY), "never sooner than ten days: {maps:?}");
    assert!(s.known(1).contains(&2), "and her map swap taught the far end the town she saw: {:?}", s.known(1));
}

#[test]
fn a_cartographer_at_home_charts_the_country_around() {
    let mut s = Society::new(content());
    let t = town(&mut s, "Town", &["cartographer"]);
    let near = s.add_town("Near", 0.0);
    let far = s.add_town("Far", 0.0);
    s.place_town(t, 0.0, 0.0);
    s.place_town(near, 700.0, 0.0);
    s.place_town(far, 1500.0, 0.0);
    assert!(!s.known(t).contains(&near), "beyond word of mouth");
    s.advance(LOOK);
    assert!(s.known(t).contains(&near), "charted: within 800 blocks");
    assert!(!s.known(t).contains(&far), "but not 1,500");
    assert_eq!(s.times_done("charting", t, t), 1);
    s.advance(5 * DAY);
    assert_eq!(s.times_done("charting", t, t), 1, "every ten days");
    s.advance(11 * DAY);
    assert_eq!(s.times_done("charting", t, t), 2);
}

#[test]
fn services_switched_off_do_nothing() {
    let mut s = Society::new(content());
    s.set_dial("services", 0.0).unwrap();
    let t = town(&mut s, "Town", &["cleric", "armorer"]);
    s.deliver(t, RAW_IRON, 16.0);
    s.deliver(t, COAL, 2.0);
    s.advance(LOOK);
    assert_eq!((s.blessed_until(t), stock(&s, t, IRON)), (None, 0.0));
}

#[test]
fn the_services_ledger_survives_a_save() {
    let mut s = Society::new(content());
    let t = town(&mut s, "Town", &["cleric"]);
    s.advance(LOOK);
    let bytes = s.save();
    let back = Society::load(&bytes).unwrap();
    assert_eq!(back.done(t), s.done(t));
    assert_eq!(back.blessed_until(t), s.blessed_until(t));
}
