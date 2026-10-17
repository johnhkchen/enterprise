//! The atlas and books (`atlas.rs`, `rules/atlas.rs`), pinned as the mod's caravan tests pin them: the test network's
//! hub and far end four blocks apart (a point of food a leg, 80 ticks on the road), bread at six an emerald for 5
//! points, paper at 24 an emerald; the hub's Merchant at its desk and a squad of trained hands (level 2:
//! `CaravanTests.squadMember`); the far end's lockers take paper up to a stack (its par).

use enterprise_core::atlas::{Entry, Kind, Why};
use enterprise_core::{Content, Good, Society};

const DAY: u64 = 24_000;
/// A look: the core looks every 1,200 ticks.
const LOOK: u64 = 1_200;
const PAPER: usize = 0;
const BREAD: usize = 1;
const WHEAT: usize = 2;
const CANE: usize = 3;
const COOKIE: usize = 4;
const BOOK: usize = 5;
const HUB: usize = 0;
const DEST: usize = 1;

/// The mod's goods for these tests (`Prices`): paper 24 an emerald, bread 6 (5 food points), wheat 20, sugar cane 24,
/// cookies 18 (treats), books 4 (reading).
fn content() -> Content {
    let c = Content::new(Vec::new(), DAY);
    let (treats, reading) = (c.comfort("treats").unwrap(), c.comfort("reading").unwrap());
    Content::new(vec![Good::new("paper", 1.0 / 24.0, 64), Good::new("bread", 1.0 / 6.0, 64).food(5.0),
        Good::new("wheat", 1.0 / 20.0, 64), Good::new("sugar_cane", 1.0 / 24.0, 64),
        Good::new("cookie", 1.0 / 18.0, 64).comfort(treats), Good::new("book", 1.0 / 4.0, 16).comfort(reading)], DAY)
}

/// The mod's test network (`CaravanTests.network`): a hub with 128 paper, `bread` and `emeralds`, its Merchant at the
/// desk and a squad of `squad` (trained at level 2: no locker); a far end four blocks away holding `dest_spurs` (16
/// an emerald) and two bread, whose lockers take paper up to a stack (its par).
fn network(bread: f64, emeralds: f64, dest_spurs: f64, squad: &[&str]) -> Society {
    let mut s = Society::new(content());
    s.content_mut().dials.standing_orders = false;
    let hub = s.add_town("Hub", emeralds);
    let dest = s.add_town("Dest", dest_spurs / 16.0);
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

fn whys(s: &Society, town: usize) -> String {
    s.whys(town).into_iter().map(|w| w.line).collect::<Vec<_>>().join("\n")
}

fn tally(s: &Society, town: usize, line: &str) -> Option<(u64, f64)> {
    s.whys(town).into_iter().find(|w| w.line == line).map(|w| (w.times, w.items))
}

// parity: CaravanTests.aMerchantNotesWhyGoodsGoOrStay
#[test]
fn a_merchant_notes_why_goods_go_or_stay() {
    let mut s = network(70.0, 5.0, 192.0, &["librarian"]);
    s.content_mut().dials.wagon_slots = 4; // `pickCargo(..., 4, t)`: four stacks
    // The far end's lockers take wheat (its bakers), sugar cane (cookies) and paper, a stack of each its par.
    s.record_capacity(DEST, 1, &[(PAPER, 64.0 / 5.0), (WHEAT, 64.0 / 5.0), (CANE, 64.0 / 5.0)]);
    for g in [WHEAT, CANE] {
        s.set_trade(DEST, g, 0.0, 1e-9);
    }
    // The far end holds more wheat than its par, as last seen; wants cane, which the hub hasn't got; and paper, which
    // it has.
    s.record_stocktake(DEST, &[(WHEAT, 500.0)], Some(0));
    s.deliver(HUB, WHEAT, 64.0);
    s.deliver(HUB, PAPER, 32.0);
    s.advance(LOOK);
    let sent: Vec<(usize, f64)> = s.books(HUB).iter().filter(|e| e.kind == Kind::Sent)
        .map(|e| (e.good.unwrap(), e.count)).collect();
    assert_eq!(sent, vec![(PAPER, 64.0)], "it sends the paper (the far end's par) and keeps the wheat");
    assert_eq!(s.town(HUB).stock[WHEAT], 62.0, "the wheat stays home, but for the team's fodder");
    let told = whys(&s, HUB);
    assert!(told.contains("Dest | wheat | not sent: they hold their fill"), "why the wheat stayed: {told}");
    assert!(told.contains("Dest | sugar_cane | not sent: we have none"), "why no cane went: {told}");
    assert!(told.contains("Dest | paper | sent: a trade it has switched on takes it"), "why the paper went: {told}");
    assert_eq!(tally(&s, HUB, "Dest | wheat | not sent: they hold their fill (a trade it has switched on takes it)"),
        Some((1, 500.0)), "once, the 500 they held");
    assert!(told.contains("Dest | - | went: on its day"), "and why it went: {told}");
    // A reason in a wagon's log, handed in at home, goes in the tally, not the pages.
    let lines = s.books(HUB).len();
    let reading = s.content().comfort("reading").unwrap();
    s.file(HUB, vec![Entry::why(0, DEST, Some(BOOK), Why::UnsoldFull(Some(reading)), 12.0)]);
    assert_eq!(s.books(HUB).len(), lines, "no page for a reason");
    assert_eq!(tally(&s, HUB, "Dest | book | unsold: they hold their fill of reading"), Some((1, 12.0)),
        "the log's reason is tallied: {}", whys(&s, HUB));
}

// parity: CaravanTests.aTownShortOfComfortsCommissionsItsOwnCaravan
#[test]
fn a_town_short_of_comforts_commissions_its_own_caravan() {
    let mut s = network(2.0, 5.0, 192.0, &["librarian"]);
    s.set_people(DEST, 5.0, 5.0, 5.0); // the destination's town: five villagers
    s.pay(DEST, 30.0);
    s.advance(DAY); // a day without comforts
    assert!(s.town(DEST).people.comfort_met.iter().all(|&m| m < 1.0), "short of every kind");
    assert_eq!(s.known(DEST), vec![HUB], "the hub's wagon has been: both ends know each other");
    assert_eq!(s.consider_commission(DEST), None, "it knows no market with treats yet");
    s.record_stocktake(HUB, &[(COOKIE, 64.0)], Some(DAY));
    s.learn(DEST, HUB); // its caravans have been there
    let routes = s.route_count();
    let route = s.consider_commission(DEST).expect("it sends to the market it knows has treats");
    assert_eq!((route, s.route_ends(route)), (routes, (DEST, HUB)));
    assert_eq!(s.consider_commission(DEST), None, "a town with a route of its own commissions no more");
}

#[test]
fn a_town_commissions_only_with_money_to_spare_and_a_known_stock() {
    let mut s = network(2.0, 5.0, 192.0, &["librarian"]);
    s.set_people(DEST, 5.0, 5.0, 5.0);
    s.advance(DAY);
    s.record_stocktake(HUB, &[(COOKIE, 64.0)], Some(DAY));
    s.pay(DEST, 7.5 - s.town(DEST).money); // under the 8 to spare beyond its reserve
    assert_eq!(s.consider_commission(DEST), None, "not 8 emeralds to spare");
    s.pay(DEST, 1.0);
    assert!(s.consider_commission(DEST).is_some(), "8.5: enough");
    let mut s = network(2.0, 5.0, 192.0, &["librarian"]);
    s.set_people(DEST, 5.0, 5.0, 5.0);
    s.pay(DEST, 30.0);
    s.advance(DAY);
    s.record_stocktake(HUB, &[(PAPER, 64.0)], Some(DAY));
    assert_eq!(s.consider_commission(DEST), None, "the hub's last stocktake holds no comfort it lacks");
    s.record_stocktake(HUB, &[(BOOK, 1.0)], None); // a hub's own, as its wagon left: any stocktake counts
    assert!(s.consider_commission(DEST).is_some(), "a book: it's short of reading too");
}

#[test]
fn towns_commission_at_their_reviews() {
    // A hub that sends no wagons (its stocktake stays as counted), and a town of five within word of mouth.
    let mut s = Society::new(content());
    let hub = s.add_town("Hub", 5.0);
    let town = s.add_town("Town", 30.0);
    s.place_town(hub, 0.0, 0.0);
    s.place_town(town, 300.0, 0.0);
    s.set_people(town, 5.0, 5.0, 5.0);
    s.record_stocktake(hub, &[(COOKIE, 64.0)], Some(0));
    s.advance(LOOK);
    assert_eq!(s.route_count(), 0, "the first look only starts the clock");
    s.advance(LOOK + 3 * DAY);
    assert_eq!(s.route_count(), 1, "at its review three days on it commissions one");
    assert_eq!(s.route_ends(0), (town, hub));
    assert!(s.route_path(0).is_some(), "a road over the map, both being placed");
    s.advance(LOOK + 9 * DAY);
    assert_eq!(s.route_count(), 1, "and no more");
}

// parity: CaravanTests.bothMerchantsKeepBooksOfATrip
#[test]
fn both_merchants_keep_books_of_a_trip() {
    let mut s = network(2.0, 5.0, 640.0, &["librarian"]);
    s.advance(LOOK);
    s.advance(2 * LOOK);
    s.advance(3 * LOOK);
    assert!(s.route(0).trips == 1, "home");
    let home = s.books(HUB);
    let there = s.books(DEST);
    assert!(home.iter().any(|e| e.kind == Kind::Sent && e.good == Some(PAPER) && e.with == Some(DEST)),
        "home's books: paper sent out {home:?}");
    assert!(home.iter().any(|e| e.kind == Kind::Sold && e.good == Some(PAPER) && e.money > 0.0 && e.with == Some(DEST)),
        "and, from the wagon's log, paper sold there: {home:?}");
    assert!(there.iter().any(|e| e.kind == Kind::Bought && e.good == Some(PAPER) && e.money > 0.0 && e.with == Some(HUB)),
        "the far Merchant's: paper bought {there:?}");
    let sold = home.iter().find(|e| e.kind == Kind::Sold).unwrap();
    assert_eq!(s.last_price(HUB, DEST, PAPER), Some(sold.money / sold.count), "home knows what paper fetched there");
    assert_eq!(s.last_price(HUB, DEST, BREAD), None);
}

// parity-partial: CaravanTests.aLibrarianBringsBackTheBooks (the far end here uses its paper, so the wagon has a load
// on its next day: with no stocktake the core reads its stock as it is, where the mod reads none and sends its par)
#[test]
fn a_librarian_brings_back_the_books() {
    let mut s = network(70.0, 5.0, 192.0, &["cartographer"]);
    s.set_trade(DEST, PAPER, 0.0, 64.0 / 5.0); // its people use paper: a stack lasts its five days
    s.advance(LOOK);
    s.advance(2 * LOOK);
    assert!(s.stocktake(DEST).is_none_or(|t| t.when.is_none()), "no Librarian: nobody counted their shelves");
    s.advance(3 * LOOK);
    let librarian = s.add_hand(HUB, "librarian", 2);
    s.set_offers(HUB, librarian, Vec::new());
    let day = LOOK + 4 * DAY; // the wagon's next day
    s.advance(day);
    assert!(s.crew(0).crew.iter().any(|(t, _)| t == "librarian"), "the books are stale: she goes along {:?}", s.crew(0));
    s.advance(day + 2 * LOOK);
    assert!(s.stocktake(DEST).is_some_and(|t| t.when.is_some()), "with one, the books come home");
}

#[test]
fn a_home_with_a_roster_plans_on_the_last_stocktake_while_fresh() {
    let mut s = network(70.0, 5.0, 192.0, &["librarian"]);
    // Last seen holding its par of paper: nothing goes while that's fresh.
    s.record_stocktake(DEST, &[(PAPER, 64.0)], Some(0));
    s.advance(LOOK);
    assert_eq!(s.route(0).carried, 0.0, "they hold their fill, as last seen");
    assert!(whys(&s, HUB).contains("Dest | paper | not sent: they hold their fill"), "{}", whys(&s, HUB));
    // Three days on the stocktake is stale: home plans on the far end as it is (none held), and wants a Librarian.
    s.advance(3 * DAY + 2 * LOOK);
    assert_eq!(s.books(HUB).iter().filter(|e| e.kind == Kind::Sent).map(|e| e.count).sum::<f64>(), 64.0,
        "a stack goes once the books are stale");
}

#[test]
fn a_home_without_a_roster_plans_on_the_far_end_as_it_is() {
    let mut s = network(70.0, 5.0, 192.0, &["librarian"]);
    s.set_roster(HUB, &[]); // no hands: today's one-mouth wagon
    s.record_stocktake(DEST, &[(PAPER, 64.0)], Some(0));
    s.advance(LOOK);
    assert_eq!(s.route(0).carried, 64.0, "a stack, as the far end holds none");
    s.advance(2 * LOOK);
    let take = s.stocktake(DEST).expect("counted at the visit");
    assert_eq!((take.when, take.stock[PAPER]), (Some(2 * LOOK), 64.0), "after the sale");
}

#[test]
fn a_wagon_counts_what_others_carry_to_the_far_end() {
    // No road food (a bare road's money only), and a two-day road: the first wagon is still on its way when the
    // second plans.
    let mut c = content();
    c.goods[BREAD].food = 0.0;
    let mut s = Society::new(c);
    s.content_mut().dials.standing_orders = false;
    let hub = s.add_town("Hub", 5.0);
    let dest = s.add_town("Dest", 12.0);
    s.record_capacity(dest, 1, &[(PAPER, 64.0 / 5.0)]);
    s.set_trade(dest, PAPER, 0.0, 1e-9);
    s.deliver(hub, PAPER, 256.0);
    s.set_roster(hub, &[("merchant", 1), ("librarian", 2), ("cartographer", 2), ("cartographer", 2)]);
    for i in 1..4 {
        s.set_offers(hub, i, Vec::new());
    }
    s.add_route(hub, dest, 2.0, 0.0, 4.0);
    s.advance(LOOK);
    assert_eq!(s.route(0).carried, 64.0, "the first takes the far end's par");
    s.add_route(hub, dest, 2.0, 0.0, 4.0);
    s.advance(2 * LOOK);
    assert_eq!(s.route(1).carried, 0.0, "the second counts the first's stack on its way there");
    assert_eq!(tally(&s, hub, "Dest | paper | not sent: they hold their fill (a trade it has switched on takes it)")
        .map(|t| t.1), Some(64.0), "{}", whys(&s, hub));
}

#[test]
fn a_wagon_counts_what_others_carry_crated_to_the_far_end() {
    // As above, with a far-end par of three stacks: the first wagon's paper goes crated (`rules/cargo.rs::ship`
    // crates a good for sale of two stacks or more), and the second counts the crates too.
    let mut c = content();
    c.goods[BREAD].food = 0.0;
    let mut s = Society::new(c);
    s.content_mut().dials.standing_orders = false;
    let hub = s.add_town("Hub", 50.0);
    let dest = s.add_town("Dest", 12.0);
    s.record_capacity(dest, 1, &[(PAPER, 192.0 / 5.0)]);
    s.set_trade(dest, PAPER, 0.0, 1e-9);
    s.deliver(hub, PAPER, 1024.0);
    s.set_roster(hub, &[("merchant", 1), ("librarian", 2), ("cartographer", 2), ("cartographer", 2)]);
    for i in 1..4 {
        s.set_offers(hub, i, Vec::new());
    }
    s.add_route(hub, dest, 2.0, 0.0, 4.0);
    s.advance(LOOK);
    assert_eq!(s.route(0).carried, 192.0, "the first takes the far end's par");
    assert_eq!(s.town(hub).stock[PAPER], 1024.0 - 192.0, "gone aboard");
    s.add_route(hub, dest, 2.0, 0.0, 4.0);
    s.advance(2 * LOOK);
    assert_eq!(s.route(1).carried, 0.0, "the second counts the first's crates on their way there");
    assert_eq!(tally(&s, hub, "Dest | paper | not sent: they hold their fill (a trade it has switched on takes it)")
        .map(|t| t.1), Some(192.0), "{}", whys(&s, hub));
}

#[test]
fn a_wagon_leaving_takes_home_s_stocktake_and_both_ends_learn_each_other() {
    let mut s = network(70.0, 5.0, 192.0, &["librarian"]);
    assert!(s.known(HUB).is_empty() && s.known(DEST).is_empty(), "unplaced and never travelled: they know nobody");
    s.advance(LOOK);
    let take = s.stocktake(HUB).expect("home's, as its wagon left");
    assert_eq!(take.when, None, "with no time: it makes no books fresh");
    assert_eq!(take.stock[PAPER], 64.0, "the paper left after the load");
    assert_eq!((s.known(HUB), s.known(DEST)), (vec![DEST], vec![HUB]), "both ends of the road");
}

/// Two towns far apart on open ground, and a third whose land the road between them crosses.
fn three_towns() -> Society {
    let mut s = Society::new(Content::new(vec![Good::new("paper", 1.0, 64)], DAY));
    s.content_mut().dials.standing_orders = false;
    let hub = s.add_town("Hub", 50.0);
    let dest = s.add_town("Dest", 12.0);
    let side = s.add_town("Side", 0.0);
    s.place_town(hub, 0.0, 0.0);
    s.place_town(dest, 2000.0, 0.0);
    s.place_town(side, 1000.0, 1000.0);
    s.record_capacity(dest, 1, &[(PAPER, 64.0 / 5.0)]);
    s.set_trade(dest, PAPER, 0.0, 1e-9);
    s.deliver(hub, PAPER, 128.0);
    s
}

#[test]
fn towns_whose_land_a_road_crosses_learn_both_its_ends() {
    let mut s = three_towns();
    let (hub, dest, side) = (0, 1, 2);
    assert!(s.known(side).is_empty() && s.known(hub).is_empty(), "nobody within word of mouth");
    let route = s.add_road(hub, dest, 4.0).expect("a road over open ground");
    let mid = s.route_path(route).unwrap()[10];
    assert!(s.claim_hex(side, mid), "the side town holds a hex on the road");
    let (ticks, _) = s.route_road(route);
    s.advance(LOOK);
    assert!(s.known(side).is_empty(), "not till the wagon's home");
    s.advance(LOOK + 3 * ticks);
    assert_eq!(s.route(route).trips, 1, "{:?} {:?}", s.road_food(route), whys(&s, hub));
    assert_eq!(s.known(side), vec![hub, dest], "both ends, nearest first");
}

// parity-partial: CaravanTests.caravansAreCommissionedAtTheDesk
#[test]
fn caravans_are_commissioned_at_the_desk() {
    let mut s = three_towns();
    let (hub, dest, side) = (0, 1, 2);
    let near = s.add_town("Near", 5.0);
    let away = s.add_town("Away", 5.0);
    s.place_town(near, 300.0, 0.0);
    s.place_town(away, 6000.0, 0.0);
    assert_eq!(s.known(hub), vec![near], "the desk knows its neighbour by word of mouth");
    let route = s.commission(hub, near).expect("Send caravans: a route");
    assert_eq!(s.route_ends(route), (hub, near));
    assert!(s.commission(hub, near).is_err(), "one route there is enough");
    assert!(s.commission(hub, dest).is_err(), "only markets it knows");
    s.learn(hub, dest);
    s.learn(hub, away);
    assert_eq!(s.known(hub), vec![near, dest, away], "nearest first");
    assert!(s.commission(hub, away).is_err(), "6,000 blocks: beyond a leg's food");
    assert!(s.commission(hub, dest).is_ok(), "2,000 blocks: within it");
    assert!(s.commission(side, side).is_err());
}

#[test]
fn word_of_mouth_carries_further_from_a_caravanserai() {
    let mut s = three_towns();
    let (hub, _, side) = (0, 1, 2);
    let mid = s.add_town("Mid", 5.0);
    s.place_town(mid, 600.0, 0.0);
    assert!(!s.known(hub).contains(&mid), "600 blocks: beyond word of mouth");
    let caravanserai = enterprise_core::works::What::Hall(enterprise_core::works::Kind::Caravanserai);
    s.start_project(mid, caravanserai);
    s.finish_project(mid, caravanserai);
    assert_eq!(s.known(hub), vec![mid], "heard of from 800 with a caravanserai");
    assert!(!s.known(side).contains(&mid), "about 1,080 blocks from the side town: beyond even that");
}

#[test]
fn books_go_back_ten_days_sixty_with_a_librarian_a_year_with_a_library() {
    let kept = |librarian: bool, library: bool| -> Vec<u64> {
        let mut s = network(2.0, 5.0, 192.0, &[]);
        if librarian {
            s.add_hand(HUB, "librarian", 1);
        }
        if library {
            let library = enterprise_core::works::What::Hall(enterprise_core::works::Kind::Library);
            s.start_project(HUB, library);
            s.finish_project(HUB, library);
        }
        s.advance(100 * DAY);
        let line = |day: u64| Entry::new(day, Kind::Shop, DEST, PAPER, 1.0, 0.1);
        s.file(HUB, vec![line(0), line(39), line(41), line(89), line(99)]);
        s.books(HUB).iter().filter(|e| e.kind == Kind::Shop).map(|e| e.day).collect()
    };
    assert_eq!(kept(false, false), vec![99], "a Merchant alone: ten days");
    assert_eq!(kept(true, false), vec![41, 89, 99], "a Librarian in town: sixty");
    assert_eq!(kept(false, true), vec![0, 39, 41, 89, 99], "a library: a year");
}

#[test]
fn what_the_far_end_wont_buy_is_noted_in_the_log_and_tallied_at_home() {
    let mut s = network(70.0, 5.0, 0.0, &["librarian"]); // the far end has no money
    s.advance(LOOK);
    assert_eq!(s.route(0).carried, 64.0);
    s.advance(3 * LOOK);
    assert_eq!(s.route(0).trips, 1);
    assert_eq!(tally(&s, HUB, "Dest | paper | unsold: they can't pay"), Some((1, 64.0)), "{}", whys(&s, HUB));
    assert!(s.books(HUB).iter().all(|e| e.kind != Kind::Sold), "nothing sold there");
    assert!(s.books(HUB).iter().any(|e| e.kind == Kind::Home && e.count == 64.0), "it all came home");
}

#[test]
fn a_route_that_stays_home_says_why_once_a_day() {
    let mut s = network(70.0, 5.0, 192.0, &[]); // nobody to lead a squad
    s.advance(DAY - LOOK);
    assert_eq!(tally(&s, HUB, "Dest | - | no trip: no squad at home"), Some((1, 0.0)), "{}", whys(&s, HUB));
    s.advance(2 * DAY - LOOK);
    assert_eq!(tally(&s, HUB, "Dest | - | no trip: no squad at home").map(|t| t.0), Some(2), "a day, a line");
}
