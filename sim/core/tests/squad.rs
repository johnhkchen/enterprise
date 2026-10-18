//! Squads (`squad.rs`, `rules/squad.rs`), pinned as the mod's caravan tests pin them: the test network's hub and far
//! end four blocks apart (a point of food a leg, 80 ticks on the road), bread at six an emerald for 5 points, paper at
//! 24 an emerald; the hub's Merchant at its desk and a squad of trained hands (level 2, no locker of their own:
//! `CaravanTests.squadMember`).

use enterprise_core::cash_book::Account;
use enterprise_core::road::Stay;
use enterprise_core::roster::Hand;
use enterprise_core::squad::{self, available, eligible, muster, Needs};
use enterprise_core::{Content, Good, Incoterm, Roster, Society};

const DAY: u64 = 24_000;
/// A look: the core looks every 1,200 ticks.
const LOOK: u64 = 1_200;
const PAPER: usize = 0;
const BREAD: usize = 1;
const SADDLE: usize = 2;
/// Three more goods a far end's lockers may take: of one good a wagon takes no more than 640, nor than its share of the
/// slots (`Caravans.pickCargo`), so a load to fill a squad's 36 stacks is four goods' worth (`plan_sale`).
const MORE: [usize; 3] = [3, 4, 5];
const HUB: usize = 0;
const DEST: usize = 1;

fn content() -> Content {
    Content::new(vec![Good::new("paper", 1.0 / 24.0, 64), Good::new("bread", 1.0 / 6.0, 64).food(5.0),
        Good::new("saddle", 6.0, 1), Good::new("ink_sac", 1.0 / 24.0, 64), Good::new("feather", 1.0 / 24.0, 64),
        Good::new("string", 1.0 / 24.0, 64)], DAY)
}

/// The mod's test network (`CaravanTests.network`): a hub with 128 paper, `bread` and `emeralds`, its Merchant at the
/// desk and a squad of `squad` (trained at level 2: no locker; `none` and `nitwit` as they come); a far end four blocks
/// away holding `dest_spurs` (16 an emerald) and two bread, whose lockers take paper up to a stack (its par).
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
    hands.extend(squad.iter().map(|&t| (t, if t == "none" || t == "nitwit" { 1 } else { 2 })));
    s.set_roster(hub, &hands);
    for i in 1..hands.len() {
        s.set_offers(hub, i, Vec::new()); // trained: it keeps its trade without a workstation
    }
    s.add_route(hub, dest, 4.0 / 1200.0, 0.0, 4.0);
    s
}

/// The hands who could go from the hub, and its roster.
fn home(s: &Society) -> (Roster, Vec<usize>) {
    let roster = s.roster(HUB);
    let lockers = s.lockers(HUB);
    let avail = available(s.content(), &roster, lockers.as_ref());
    (roster, avail)
}

fn trades(s: &Society, roster: &Roster, hands: &[usize]) -> Vec<String> {
    hands.iter().map(|&i| s.content().trades[roster.hands[i].trade].name.clone()).collect()
}

fn away(s: &Society, town: usize) -> usize {
    s.roster(town).hands.iter().filter(|h| h.away).count()
}

// parity: CaravanTests.extraMembersGoOnlyWhereTheyHelp
#[test]
fn extra_members_go_only_where_they_help() {
    let s = network(2.0, 5.0, 192.0, &["cartographer", "shepherd"]);
    let c = s.content();
    let (roster, avail) = home(&s);
    assert_eq!(muster(c, &roster, &avail, &Needs::of(c, false, false, false), 3).len(), 1,
        "nothing to map, no animals: the lead goes alone");
    let animals = Needs::of(c, false, false, true);
    let squad = muster(c, &roster, &avail, &animals, 3);
    assert_eq!(trades(&s, &roster, &squad), ["cartographer", "shepherd"], "animals to drive: the Shepherd comes too");
    assert_eq!(muster(c, &roster, &avail, &animals, 0).len(), 1, "unless she's another route's lead");
}

// parity: CaravanTests.aClericGoesAlongButNeverLeads
#[test]
fn a_cleric_goes_along_but_never_leads() {
    let s = network(2.0, 5.0, 192.0, &["cleric"]);
    let (roster, avail) = home(&s);
    assert_eq!(trades(&s, &roster, &avail), ["cleric"], "she could go");
    assert!(muster(s.content(), &roster, &avail, &Needs::all(s.content()), 3).is_empty(),
        "a Cleric alone doesn't drive the wagon");
}

// parity: CaravanTests.aSquadIsNeverMoreThanFour
#[test]
fn a_squad_is_never_more_than_four() {
    let s = network(2.0, 5.0, 192.0, &["cartographer", "librarian", "shepherd", "leatherworker", "fisherman", "fisherman"]);
    let (roster, avail) = home(&s);
    let squad = muster(s.content(), &roster, &avail, &Needs::all(s.content()), 9);
    assert_eq!(squad.len(), squad::MAX, "four at most: {:?}", trades(&s, &roster, &squad));
    assert_eq!(trades(&s, &roster, &squad), ["fisherman", "cartographer", "librarian", "shepherd"],
        "by water a Fisherman leads, then the rest in the squad's order");
}

// parity: CaravanTests.babiesAndTheDesksMerchantStayHome
#[test]
fn babies_and_the_desks_merchant_stay_home() {
    let c = content();
    let cartographer = c.trade("cartographer").unwrap();
    let none = c.trade("none").unwrap();
    let mut baby = Hand::new(&c, cartographer, 2);
    baby.baby_until = Some(DAY);
    let roster = Roster::new(&c, vec![Hand::new(&c, c.trade("merchant").unwrap(), 1), baby, Hand::new(&c, none, 1)]);
    assert!(!eligible(&c, &roster, None, 1), "no babies on the road");
    let avail = available(&c, &roster, None);
    assert_eq!(avail, vec![2], "only the untrained adult could go");
    let needs = Needs { porters: 2, ..Default::default() };
    assert!(muster(&c, &roster, &avail, &needs, 3).is_empty(), "the untrained go only as porters, and never alone");
    assert!(!eligible(&c, &roster, None, 0) && !avail.contains(&0), "and not the Merchant who works the desk");
}

#[test]
fn a_hand_working_a_locker_stays_home() {
    // A Librarian whose paper-buying offer the Merchant runs is the market's trade: it doesn't go.
    let mut s = network(2.0, 5.0, 192.0, &["librarian"]);
    let c = s.content().clone();
    let (roster, avail) = home(&s);
    assert_eq!(trades(&s, &roster, &avail), ["librarian"]);
    let paper = enterprise_core::Offer::buys(PAPER, 24, 1, 16, 2, 0.05);
    s.set_offers(HUB, 1, vec![paper]);
    s.switch(HUB, 1, 0, true);
    let lockers = s.lockers(HUB).unwrap();
    assert!(lockers.managed.contains(&1));
    assert!(available(&c, &s.roster(HUB), Some(&lockers)).is_empty(), "working its locker: not in the squad");
}

// parity: CaravanTests.noSquadNoCaravan
#[test]
fn no_squad_no_caravan() {
    let mut s = network(2.0, 5.0, 192.0, &[]);
    s.advance(LOOK);
    assert!(!s.route(0).on_the_road, "nobody to send, nobody goes");
    assert_eq!(s.road_food(0).stayed, Some(Stay::NoSquad), "the route says so");
    assert_eq!(s.town(HUB).stock[PAPER], 128.0, "the paper stays home");
}

// parity: CaravanTests.aTownShortOfCrewTakesOnAHand
#[test]
fn a_town_short_of_crew_takes_on_a_hand() {
    let mut s = network(2.0, 8.0, 192.0, &["none"]);
    // (A town that can't pay to train one: it signs the hand on instead, as the poll does in the mod's test.)
    s.set_dial("training_emeralds", 1000.0).unwrap();
    s.advance(LOOK);
    let (trade, level, xp, away) = s.hand(HUB, 1).unwrap();
    assert_eq!((trade.as_str(), level), ("merchant", 1), "the Merchant takes the hand on as a travelling Merchant");
    assert!(xp > 0, "and it keeps the trade");
    assert!(s.route(0).on_the_road, "it leads the caravan");
    assert!(away, "off home while on the road");
    assert_eq!(s.crew(0).crew, vec![("merchant".to_string(), 1)]);
    let book = s.cash_book(HUB, 1);
    assert!(book.iter().any(|(_, why, v)| why == "signing" && *v == -2.0), "2 emeralds to sign on: {book:?}");
    assert_eq!(s.roster(HUB).merchant, Some(0), "the desk's Merchant is still the desk's");
    s.advance(4 * LOOK);
    assert!(!s.route(0).on_the_road, "home");
    let (trade, _, _, away) = s.hand(HUB, 1).unwrap();
    assert!(trade == "merchant" && !away, "home again, still a travelling Merchant");
}

// parity: CaravanTests.aTownWithARouteTrainsItsSquad
#[test]
fn a_town_with_a_route_trains_its_squad() {
    let mut s = network(2.0, 20.0, 192.0, &["none"]);
    let pupil = 1;
    let money = s.town(HUB).money;
    assert_eq!(s.train_squad(HUB), Some(pupil), "the unemployed adult is trained");
    let (trade, level, xp, _) = s.hand(HUB, pupil).unwrap();
    assert!(trade == "merchant" && level == 1 && xp > 0, "as a Merchant first, and keeps the trade: {trade} {xp}");
    assert!(money - s.town(HUB).money == 4.0, "for 4 emeralds");
    let book = s.cash_book(HUB, 1);
    assert!(book.iter().any(|(_, why, v)| why == "training" && *v == -4.0), "{book:?}");
    // A lead a route and a spare, at most four: the next learns the next lead trade, then it has enough.
    s.add_hand(HUB, "none", 1);
    s.add_hand(HUB, "none", 1);
    assert_eq!(s.train_squad(HUB), Some(2));
    assert_eq!(s.hand(HUB, 2).unwrap().0, "cartographer", "the trade the squad lacks next");
    assert_eq!(s.train_squad(HUB), None, "two leads for one route: enough");
    // A town with no route trains nobody.
    let mut s = network(2.0, 20.0, 192.0, &["none"]);
    let lone = s.add_town("Lone", 20.0);
    s.set_roster(lone, &[("merchant", 1), ("none", 1)]);
    assert_eq!(s.train_squad(lone), None);
}

#[test]
fn a_town_trains_once_a_day_in_its_review() {
    let mut s = network(2.0, 20.0, 192.0, &["none", "none", "none"]);
    s.advance(LOOK);
    let roster = s.roster(HUB);
    let named: Vec<String> = (1..4).map(|i| s.hand(HUB, i).unwrap().0).collect();
    assert_eq!(named, ["merchant", "none", "none"], "one a day: {roster:?}");
    assert!(s.route(0).on_the_road, "and the trained Merchant leads the day's wagon");
    s.advance(DAY + LOOK);
    assert_eq!(s.hand(HUB, 2).unwrap().0, "cartographer", "the next day, the next lead");
    assert_eq!(s.hand(HUB, 3).unwrap().0, "none", "and the idle wait for the squad: no stall while it's short");
    s.advance(2 * DAY + LOOK);
    let trainings = s.cash_book(HUB, 3).iter().filter(|(_, why, _)| why == "training").count();
    assert_eq!(trainings, 2, "a lead and a spare for one route: no more (the last idle hand may find work now)");
}

// parity-partial: CaravanTests.theSquadLeavesTheWorldAndComesBack (why both go is set by hand, `want_trades`, where the mod
// works it out from the far town's missing books and unmapped road; the entities leaving the world are Minecraft's)
#[test]
fn the_squad_leaves_home_and_comes_back() {
    // 70 bread (350 food points): food to spare past the caravans' 320-point par, for a second member. What the trip
    // wants of them (books, mapping: `Services.wantedAbroad`) is the host's to say here.
    let mut s = network(70.0, 5.0, 192.0, &["cartographer", "librarian"]);
    s.want_trades(0, &["cartographer", "librarian"]);
    assert_eq!(s.food_points(HUB), 350.0);
    s.advance(LOOK);
    let crew = s.crew(0);
    assert_eq!(crew.crew.len(), 2, "both go: {crew:?}");
    assert_eq!(away(&s, HUB), 2, "both are away from home while on the road");
    assert_eq!(s.food_points(HUB), 345.0, "both eat: 2 food points, one bread");
    assert_eq!(s.road_food(0).food_a_leg, 2.0);
    s.advance(4 * LOOK);
    assert!(!s.route(0).on_the_road, "home");
    assert_eq!(away(&s, HUB), 0, "both are back");
    let named: Vec<String> = (1..3).map(|i| s.hand(HUB, i).unwrap().0).collect();
    assert_eq!(named, ["cartographer", "librarian"], "still in their trades");
    assert_eq!(s.road_food(0).food_eaten, 4.0, "two mouths, both ways");
}

#[test]
fn one_more_mouth_goes_only_when_home_can_feed_it() {
    // Two bread (10 points) is short of the caravans' 320-point par: the lead goes alone, though both are wanted.
    let mut s = network(2.0, 5.0, 192.0, &["cartographer", "librarian"]);
    s.want_trades(0, &["cartographer", "librarian"]);
    s.advance(LOOK);
    assert_eq!(s.crew(0).crew, vec![("cartographer".to_string(), 2)], "the lead alone");
    assert_eq!(away(&s, HUB), 1);
}

#[test]
fn every_other_waiting_route_keeps_a_lead() {
    // Two routes out of the hub and two leads: the first doesn't take the second's lead as an extra.
    let mut s = network(70.0, 10.0, 192.0, &["cartographer", "librarian"]);
    let far = s.add_town("Far", 12.0);
    s.record_capacity(far, 1, &[(PAPER, 64.0 / 5.0)]);
    s.set_trade(far, PAPER, 0.0, 1e-9);
    s.deliver(far, BREAD, 2.0);
    s.add_route(HUB, far, 4.0 / 1200.0, 0.0, 4.0);
    s.want_trades(0, &["librarian"]);
    s.advance(LOOK);
    assert_eq!(s.crew(0).crew, vec![("cartographer".to_string(), 2)], "the first route's lead, no extra");
    assert_eq!(s.crew(1).crew, vec![("librarian".to_string(), 2)], "the second keeps its lead");
}

#[test]
fn a_route_that_stays_keeps_no_hands_from_the_next() {
    // The hub's first route goes to a town that buys nothing: it stays. Its lead isn't kept back for it (`dispatch`
    // musters a route as it leaves), so the second route, to Dest, goes with the Cartographer and nobody is hired.
    let mut s = Society::new(content());
    s.content_mut().dials.standing_orders = false;
    let hub = s.add_town("Hub", 5.0);
    let nowhere = s.add_town("Nowhere", 0.0);
    let dest = s.add_town("Dest", 192.0 / 16.0);
    s.record_capacity(dest, 1, &[(PAPER, 64.0 / 5.0)]);
    s.set_trade(dest, PAPER, 0.0, 1e-9);
    s.deliver(hub, PAPER, 128.0);
    s.deliver(hub, BREAD, 2.0);
    s.deliver(dest, BREAD, 2.0);
    s.set_roster(hub, &[("merchant", 1), ("cartographer", 2)]);
    s.set_offers(hub, 1, Vec::new());
    s.add_route(hub, nowhere, 4.0 / 1200.0, 0.0, 4.0);
    s.add_route(hub, dest, 4.0 / 1200.0, 0.0, 4.0);
    s.advance(LOOK);
    assert!(!s.route(0).on_the_road, "nothing Nowhere wants: the first route stays");
    assert_eq!(s.road_food(0).stayed, Some(Stay::WaitingForALoad), "for a load, not for want of a squad");
    assert!(s.route(1).on_the_road, "the second goes");
    assert_eq!(s.road_food(1).stayed, None);
    assert_eq!(s.crew(1).crew, vec![("cartographer".to_string(), 2)], "with the Cartographer");
    assert_eq!(s.roster(hub).hands.len(), 2, "nobody taken on");
    assert!(!s.cash_book(hub, 1).iter().any(|(_, why, _)| why == "signing" || why == "training"), "nobody paid for");
}

#[test]
fn no_merchant_at_the_desk_no_caravan_and_no_training() {
    // A roster with no Merchant at its desk (`Caravans.dispatch` 210: "no Merchant at the hub").
    let mut s = network(2.0, 20.0, 192.0, &["cartographer", "none"]);
    s.set_roster(HUB, &[("cartographer", 2), ("none", 1)]);
    s.set_offers(HUB, 0, Vec::new());
    assert_eq!(s.roster(HUB).merchant, None);
    s.advance(LOOK);
    assert!(!s.route(0).on_the_road, "nobody sends it");
    assert_eq!(s.road_food(0).stayed, Some(Stay::NoMerchant));
    assert_eq!(s.hand(HUB, 1).unwrap().0, "none", "and nobody's commissioned to train");
    assert_eq!(s.train_squad(HUB), None);
}

#[test]
fn a_crew_home_frees_its_own_hands_only() {
    // Two porters alike in every way, out with two wagons; a hand at home leaves the roster meanwhile (a settler).
    let c = content();
    let none = c.trade("none").unwrap();
    let mut roster = Roster::new(&c, vec![Hand::new(&c, c.trade("merchant").unwrap(), 1), Hand::new(&c, none, 1),
        Hand::new(&c, none, 1), Hand::new(&c, none, 1)]);
    let first = squad::stamp(&mut roster, 2).unwrap();
    let second = squad::stamp(&mut roster, 3).unwrap();
    assert_ne!(first.id, second.id, "each known by its own id");
    enterprise_core::settlers::take_hand(&mut roster, 1);
    squad::release(&mut roster, std::slice::from_ref(&second));
    assert_eq!(roster.hands.iter().map(|h| h.away).collect::<Vec<_>>(), [false, true, false],
        "the second wagon's porter is home, the first's still out");
    squad::release(&mut roster, std::slice::from_ref(&second));
    assert!(roster.hands[1].away, "and its crew home again frees nobody else");
    squad::release(&mut roster, &[first]);
    assert!(roster.hands.iter().all(|h| !h.away));
}

// parity: CaravanTests.aLeatherworkersPacksCarryMore
#[test]
fn a_leatherworkers_packs_carry_more() {
    let mut s = network(70.0, 5.0, 192.0, &["librarian", "leatherworker"]);
    s.deliver(HUB, PAPER, 30.0 * 64.0); // 32 stacks of paper for a town that buys paper
    s.advance(LOOK);
    let crew = s.crew(0);
    assert_eq!((crew.crew.len(), crew.slots), (2, 36), "with packs: 36 stacks, {crew:?}");
}

#[test]
fn a_squad_with_fewer_slots_than_planned_leaves_the_last_of_the_sale_home() {
    // Two routes, one Leatherworker: both plan for her packs (36 stacks: nine of each of four goods), the first takes
    // her, and the second, with the wagon's 27, leaves the rest of its load home.
    let mut s = network(70.0, 10.0, 192.0, &["cartographer", "librarian", "leatherworker"]);
    let far = s.add_town("Far", 12.0);
    s.set_trade(far, PAPER, 0.0, 1e-9);
    s.deliver(far, BREAD, 2.0);
    s.add_route(HUB, far, 4.0 / 1200.0, 0.0, 4.0);
    let goods = [PAPER, MORE[0], MORE[1], MORE[2]];
    for t in [DEST, far] {
        // Room on their shelves for 40 stacks of each.
        s.record_capacity(t, 1, &goods.map(|g| (g, 40.0 * 64.0 / 5.0)));
    }
    s.deliver(HUB, PAPER, 25.0 * 64.0 - 128.0);
    for g in MORE {
        s.deliver(HUB, g, 25.0 * 64.0);
    }
    // Both far ends' books fresh (copied at 0, holding no paper, as they are): no trip wants the Librarian along
    // (`atlas.rs`: stale books would take her as the first route's extra, before the Leatherworker).
    for t in [DEST, far] {
        s.record_stocktake(t, &[], Some(0));
    }
    s.advance(LOOK);
    let (first, second) = (s.crew(0), s.crew(1));
    assert_eq!(first.slots, 36, "the first with her packs: {first:?}");
    assert_eq!(second.slots, 27, "the second without: {second:?}");
    assert_eq!((s.route(0).carried, s.route(1).carried), (36.0 * 64.0, 27.0 * 64.0),
        "each a full load for its slots: the second planned 36 stacks and left 9 home");
    let left: f64 = goods.iter().map(|&g| s.town(HUB).stock[g]).sum();
    assert_eq!(left, (100.0 - 36.0 - 27.0) * 64.0);
}

// parity: CaravanTests.aPorterCarriesOutAndWalksHome
#[test]
fn a_porter_carries_out_and_walks_home() {
    let mut s = network(70.0, 5.0, 192.0, &["librarian", "nitwit"]);
    s.deliver(HUB, PAPER, 30.0 * 64.0);
    s.advance(LOOK);
    let crew = s.crew(0);
    assert_eq!((crew.crew.len(), crew.slots), (2, 36), "a porter: 36 stacks, {crew:?}");
    assert_eq!(away(&s, HUB), 2);
    s.advance(2 * LOOK); // there, at 1,280
    let crew = s.crew(0);
    assert_eq!((crew.crew.len(), crew.walkers), (1, 1), "the load delivered, the porter walks home: {crew:?}");
    assert_eq!(crew.walker_home_at, Some(2 * LOOK + 80), "a leg's walk");
    assert_eq!(crew.slots, 27, "the wagon's own slots left");
    s.advance(3 * LOOK);
    let crew = s.crew(0);
    assert_eq!(crew.walkers, 0, "and is home");
    assert_eq!(away(&s, HUB), 0, "by the desk");
    assert_eq!(crew.porters_sent, 1);
    assert_eq!(s.road_food(0).food_eaten, 3.0, "two mouths out, one home");
}

#[test]
fn a_porter_whose_load_is_still_crated_aboard_stays() {
    // What's aboard counts its crates (`Crates.stacks`): a far end too poor to buy leaves the load crated as lots
    // (`rules/cargo.rs`), more than the wagon holds without the porter's pack, so the porter stays with it; home
    // again, the stacks it brought back count the crates too (`lastHome`).
    let mut s = network(70.0, 5.0, 16.0, &["librarian", "nitwit"]);
    // Shelf room for 40 stacks of each of four goods, one emerald to pay: a load of nine stacks of each.
    s.record_capacity(DEST, 1, &[(PAPER, 40.0 * 64.0 / 5.0), (MORE[0], 512.0), (MORE[1], 512.0), (MORE[2], 512.0)]);
    s.deliver(HUB, PAPER, 30.0 * 64.0);
    for g in MORE {
        s.deliver(HUB, g, 10.0 * 64.0);
    }
    s.advance(LOOK);
    assert_eq!(s.crew(0).crew.len(), 2);
    s.advance(2 * LOOK); // there
    let crew = s.crew(0);
    assert_eq!((crew.crew.len(), crew.walkers, crew.slots), (2, 0, 36), "the porter's back still needed: {crew:?}");
    s.advance(3 * LOOK); // home, and off again
    let crew = s.crew(0);
    assert!(crew.last_home.is_some_and(|h| h > 27), "the crates it brought home counted: {crew:?}");
}

// parity: CaravanTests.aSaddledCaravanRidesFaster
#[test]
fn a_saddled_caravan_rides_faster() {
    let mut s = network(2.0, 5.0, 192.0, &["librarian"]);
    s.deliver(HUB, SADDLE, 1.0);
    s.advance(LOOK);
    let crew = s.crew(0);
    assert_eq!(crew.arrives, Some(LOOK + 60), "a third faster: there at 60 ticks, not 80");
    assert!(crew.saddled && (crew.saddle_wear - 0.1).abs() < 1e-12, "a tenth of the saddle worn");
    s.advance(2 * LOOK);
    assert_eq!(s.road_food(0).home_at, Some(2 * LOOK + 60), "and home again as fast");
    // Without a saddle, 80.
    let mut s = network(2.0, 5.0, 192.0, &["librarian"]);
    s.advance(LOOK);
    assert_eq!(s.crew(0).arrives, Some(LOOK + 80));
}

#[test]
fn a_saddled_wagon_takes_its_crates_from_home() {
    // A saddled wagon gets there sooner (`leg_ticks`); crating as it leaves (`rules/cargo.rs` `ship`) still knows it
    // for one just gone: the crate it planned leaves home's warehouse, not rides on and stays too.
    let mut s = network(2.0, 5.0, 192.0, &["librarian"]);
    s.deliver(HUB, SADDLE, 1.0);
    let id = s.draft(HUB, DEST, PAPER, 64.0, 1.0 / 24.0, 0, DAY, 1, Incoterm::Ddp, true);
    s.deliver_shipment(HUB, enterprise_core::cargo::Shipment { good: PAPER, count: 64.0, shipper: HUB,
        consignee: Some(DEST), contract: Some(id), settled: false, copy: true });
    assert_eq!(s.shipments(HUB).held.len(), 1, "crated at home");
    let start = s.totals();
    s.advance(LOOK);
    assert!(s.route(0).on_the_road && s.crew(0).saddled, "the wagon went, saddled");
    assert!(s.shipments(HUB).held.is_empty(), "the crate left home's warehouse");
    assert!(s.wagon_shipments(0).0.iter().any(|c| c.contract == Some(id) && c.count == 64.0), "and rides aboard");
    s.conserved(&start).unwrap();
}

#[test]
fn a_saddle_wears_out_on_its_eleventh_trip() {
    let c = content();
    let mut state = enterprise_core::state::RouteState::default();
    let mut ledger = enterprise_core::state::Ledger { used: vec![0.0; 3], ..Default::default() };
    let mut stock = vec![0.0, 0.0, 1.0];
    let trips = (1..=20).take_while(|_| enterprise_core::rules::squad::saddle(&c, &mut ledger, &mut state, &mut stock)).count();
    assert_eq!(trips, 11, "a tenth a trip (ten tenths sum a hair short of one): used up on the eleventh");
    assert_eq!((stock[SADDLE], ledger.used[SADDLE]), (0.0, 1.0), "into the ledger's used");
    assert!(!enterprise_core::rules::squad::saddle(&c, &mut ledger, &mut state, &mut stock), "none left: no saddle");
}

/// The mod's test: a first trip home with nothing left, then 32 paper owed in two days. Returns the society, the
/// contract and the tick its wagon went.
fn owed_by_porter() -> (Society, usize, u64) {
    let mut s = network(8.0, 5.0, 192.0, &["librarian"]);
    s.advance(LOOK);
    s.advance(4 * LOOK);
    assert!(!s.route(0).on_the_road, "its first trip done");
    assert_eq!(s.crew(0).last_home, Some(0), "all of it sold: nothing came home");
    let t = s.now();
    let id = s.draft(HUB, DEST, PAPER, 32.0, 0.5, t + 2 * DAY, DAY, 1, Incoterm::Dap, true);
    s.advance(t + 2 * DAY);
    assert!(!s.route(0).on_the_road && s.contract(id).delivered == 0.0, "due, with days of grace yet: it waits");
    // The mod's last day it can wait: a day before the installment's grace runs out (`nextDeadline - 24000`).
    let last = t + 2 * DAY + (enterprise_core::contracts::GRACE_DAYS as u64) * DAY - DAY;
    for look in 1..=(2 * DAY / LOOK) {
        s.advance(t + 2 * DAY + look * LOOK);
        if s.route(0).on_the_road {
            let went = s.now();
            assert!(went <= last && went + DAY > last, "it goes on the day it can't wait, {last}: went at {went}");
            return (s, id, went);
        }
    }
    panic!("it never went");
}

// parity: CaravanTests.goodsOwedWaitTillTheyCantAndASmallLoadGoesByPorter
#[test]
fn the_day_it_cant_wait_a_small_load_goes_by_porter() {
    let (mut s, id, went) = owed_by_porter();
    let crew = s.crew(0);
    assert!(crew.porter && crew.slots == 9, "the day it can't wait, it goes; half a stack on a porter's back: {crew:?}");
    assert_eq!(crew.crew.len(), 1);
    s.advance(went + 2 * LOOK);
    assert_eq!(s.contract(id).delivered, 32.0, "delivered");
    assert_eq!(s.contract(id).missed, 0, "in time");
}

#[test]
fn a_porter_run_has_no_team_to_feed() {
    let (s, _, went) = owed_by_porter();
    let day = went / DAY;
    let road = |s: &Society, d: u64| -> enterprise_core::Fixed {
        s.cash_book(HUB, 30).iter().filter(|(on, why, _)| *on == d && why == "road food").map(|(_, _, v)| -*v).sum()
    };
    assert!(road(&s, 0) == 0.1, "the first trip's wagon bought its team's fodder, 2 points: {}", road(&s, 0));
    assert_eq!(road(&s, day), 0.0, "the porter's trip: its food from home's bread, and no team");
}

#[test]
fn a_town_without_a_roster_sends_its_one_mouth_wagon() {
    let mut s = Society::new(content());
    let hub = s.add_town("Hub", 5.0);
    let dest = s.add_town("Dest", 12.0);
    s.record_capacity(dest, 1, &[(PAPER, 64.0 / 5.0)]);
    s.set_trade(dest, PAPER, 0.0, 1e-9);
    s.deliver(hub, PAPER, 128.0);
    s.deliver(hub, BREAD, 2.0);
    s.add_route(hub, dest, 4.0 / 1200.0, 0.0, 4.0);
    s.advance(LOOK);
    assert!(s.route(0).on_the_road, "no roster, no squad wanted");
    let crew = s.crew(0);
    assert!(crew.crew.is_empty() && !crew.porter, "{crew:?}");
    assert_eq!(s.road_food(0).food_a_leg, 1.0, "one mouth");
}

#[test]
fn a_crew_on_the_road_survives_a_save() {
    let mut s = network(70.0, 5.0, 192.0, &["librarian", "nitwit"]);
    s.deliver(HUB, PAPER, 30.0 * 64.0);
    s.advance(LOOK);
    let blob = s.save();
    let mut loaded = Society::load(&blob).unwrap();
    assert!(loaded.unsaved().is_empty(), "{:?}", loaded.unsaved());
    for to in [2 * LOOK, 3 * LOOK, 5 * LOOK] {
        s.advance(to);
        loaded.advance(to);
        assert_eq!(s.crew(0), loaded.crew(0));
        assert_eq!(s.roster(HUB), loaded.roster(HUB));
    }
    let start = s.totals();
    s.advance(10 * DAY);
    s.conserved(&start).unwrap();
    s.solvent().unwrap();
    assert_eq!(s.balance(Account::Wagon(0)).is_some(), s.route(0).on_the_road);
}

