//! What the network study's world (`sim/host`, `dev/experiments/network_study.py`) showed the core got wrong, each
//! pinned by a small world as the mod's caravan tests pin it: the test network's hub and far end four blocks apart (a
//! point of food a leg), paper at 24 an emerald, bread at six an emerald for 5 points.

use enterprise_core::state::{Towns, Trades};
use enterprise_core::{Content, Good, Society};

const DAY: u64 = 24_000;
/// A look: the core looks every 1,200 ticks.
const LOOK: u64 = 1_200;
const PAPER: usize = 0;
const BREAD: usize = 1;
const WHEAT: usize = 2;
const PUMPKIN: usize = 3;
const STICK: usize = 4;
const HUB: usize = 0;
const DEST: usize = 1;

/// A town's `Trades`, read off its entity.
fn trades(s: &Society, t: usize) -> Trades {
    s.world.get::<Trades>(s.world.resource::<Towns>().0[t]).unwrap().clone()
}

fn content() -> Content {
    Content::new(vec![Good::new("paper", 1.0 / 24.0, 64), Good::new("bread", 1.0 / 6.0, 64).food(5.0),
        Good::new("wheat", 1.0 / 20.0, 64), Good::new("pumpkin", 1.0 / 6.0, 64), Good::new("stick", 1.0 / 32.0, 64)], DAY)
}

/// The mod's test network (`CaravanTests.network`): a hub with `paper` and bread for the road, 5 emeralds for the
/// purse; a far end four blocks away with `dest_money` and two bread, its villagers `hands` (a roster), every buying
/// offer of theirs switched on as the Merchant's screen would (`autoswitch selling`: the trades of the hand's level
/// and below, the ones the villager has), but those of the trades in `off`.
fn network(paper: f64, dest_money: f64, hands: &[(&str, u8)], off: &[&str]) -> Society {
    let mut s = Society::new(content());
    s.content_mut().dials.standing_orders = false;
    let hub = s.add_town("Hub", 5.0);
    let dest = s.add_town("Dest", dest_money);
    s.set_roster(dest, hands);
    let roster = s.roster(dest);
    let lockers = s.lockers(dest).expect("a roster's lockers");
    for (h, locker) in lockers.hands.iter().enumerate() {
        let trade = &s.content().trades[locker.trade].name;
        if off.contains(&trade.as_str()) {
            continue;
        }
        for (o, offer) in locker.offers.iter().enumerate() {
            if !offer.sells && offer.level <= roster.hands[h].level {
                s.switch(dest, h, o, true);
            }
        }
    }
    s.deliver(hub, PAPER, paper);
    s.deliver(hub, BREAD, 2.0);
    s.deliver(dest, BREAD, 2.0);
    s.add_route(hub, dest, 4.0 / 1200.0, 0.0, 4.0);
    s
}

// parity-partial: CaravanTests.caravanSellsItsCargoAndBuysItsFood (the far end's paper wanted through a trade it has switched on, as the Java fixture's is: no Merchant manages it, so a stack's par; its road food in road_food.rs)
#[test]
fn a_wagon_brings_what_a_trade_switched_on_there_takes() {
    // The far end's Librarian buys paper (its trade switched on, `setEnabled`), and nobody there uses paper: the
    // warehouse's par for a good a switched-on trade takes is a stack (`Warehouse.par(desk, item, true)`, MIN_PAR),
    // with no Merchant managing the locker to record its capacity.
    let mut s = network(128.0, 12.0, &[("librarian", 1)], &[]);
    assert_eq!(trades(&s, DEST).uses[PAPER], 0.0, "nobody there uses paper");
    s.advance(LOOK);
    assert!(s.route(0).on_the_road, "it leaves");
    assert_eq!(s.route(0).carried, 64.0, "with the 64 paper the far end will take (its par, one stack)");
    assert_eq!(s.town(HUB).stock[PAPER], 64.0, "the rest of the paper stays home");
    s.advance(3 * LOOK);
    assert_eq!(s.town(DEST).stock[PAPER], 64.0, "the far end bought paper up to its par of a stack");
    assert_eq!(s.route(0).sold, 64.0);
}

#[test]
fn a_managed_lockers_par_is_its_capacity_and_any_other_switched_on_trade_a_stack() {
    // A Merchant of level 1 manages two lockers, the Librarian's and the Cartographer's (the first in its order): their
    // paper trades take 24 paper an emerald 16 times a restock each (a level-1 Merchant brings no reputation), 768 a
    // day, five days of which is the par, 3,840. The Farmer's locker is beyond the Merchant's limit: its wheat trade, switched on, is
    // wanted all the same ("a trade it has switched on takes it", every workstation's), at a stack's par. Its pumpkin
    // trade is a level-2 Farmer's: a level-1 Farmer has none to switch on. The Fletcher's sticks are switched off.
    let mut s = network(200.0, 100.0,
        &[("merchant", 1), ("librarian", 1), ("cartographer", 1), ("farmer", 1), ("fletcher", 1)], &["fletcher"]);
    s.deliver(HUB, WHEAT, 100.0);
    s.deliver(HUB, PUMPKIN, 64.0);
    s.deliver(HUB, STICK, 64.0);
    s.advance(LOOK);
    assert_eq!(s.capacity(DEST, PAPER), 768.0, "the managed lockers take 768 paper a day");
    let mut cargo = s.wagon_cargo(0).expect("a wagon out");
    for crate_ in s.wagon_shipments(0).0 {
        cargo[crate_.good] += crate_.count; // (a good of two stacks or more rides crated)
    }
    assert_eq!(cargo[PAPER], 200.0, "all the paper: the far end's par is 3,840");
    assert_eq!(cargo[WHEAT], 64.0, "a stack of wheat, for the Farmer's trade its Merchant doesn't manage");
    assert_eq!((cargo[PUMPKIN], cargo[STICK]), (0.0, 0.0), "no pumpkin (no trade there has it) nor sticks (switched off)");
    s.advance(3 * LOOK);
    assert_eq!(s.route(0).sold, 264.0, "the far end bought it all: its paper and its stack of wheat");
}


/// What a wagon left with, loose and crated (a good of two stacks or more rides crated: `rules/cargo.rs`).
fn aboard(s: &Society, route: usize) -> Vec<f64> {
    let mut cargo = s.wagon_cargo(route).expect("a wagon out");
    for crate_ in s.wagon_shipments(route).0 {
        cargo[crate_.good] += crate_.count;
    }
    cargo
}

/// A far end like the study's settlements: a Merchant of level 1 managing its Librarian's and Cartographer's paper
/// trades (a par of 3,840), its Farmer's wheat trade switched on beyond its limit (a stack's par).
const SETTLEMENT: &[(&str, u8)] = &[("merchant", 1), ("librarian", 1), ("cartographer", 1), ("farmer", 1)];

#[test]
fn no_more_of_one_good_than_640_nor_than_its_share_of_the_slots() {
    // `pickCargo`: of each good no more than `WANT_PER_ITEM` (640), nor than the slots left over the goods that could
    // go, 64 to a slot: a wagon's 27 slots over paper and wheat is 13 slots each, so 640 of the paper, though the far
    // end's par is 3,840 and the hub holds 2,000.
    let mut s = network(2000.0, 100.0, SETTLEMENT, &[]);
    s.deliver(HUB, WHEAT, 100.0);
    s.advance(LOOK);
    let cargo = aboard(&s, 0);
    assert_eq!((cargo[PAPER], cargo[WHEAT]), (640.0, 64.0), "640 paper (WANT_PER_ITEM) and a stack of wheat (its par)");
    // Four slots over the same two goods: two each (`max(1, slots / goods) * 64`), 128 of the paper, where it would
    // have filled every slot before the wheat's turn.
    let mut s = network(2000.0, 100.0, SETTLEMENT, &[]);
    s.content_mut().dials.wagon_slots = 4;
    s.deliver(HUB, WHEAT, 100.0);
    s.advance(LOOK);
    let cargo = aboard(&s, 0);
    assert_eq!((cargo[PAPER], cargo[WHEAT]), (128.0, 64.0), "two slots of paper, and the wheat");
}

#[test]
fn a_wagon_is_full_only_of_what_the_far_end_could_pay_for() {
    // `saleLoad`: what makes a wagon full is the stacks the far end could pay for when a wagon last sold there
    // (`farPurse`: its money beyond its payroll, within its day's budget). A far end that could pay for a stack
    // doesn't fill a wagon with paper: the wagon waits for its day (every four days), where a far end with the money
    // for it would have the next one the day after.
    let run = |dest_money: f64| {
        let mut s = network(5000.0, dest_money, &[("merchant", 1), ("librarian", 1), ("cartographer", 1)], &[]);
        s.content_mut().dials.wagon_slots = 8; // full at 6 stacks; 512 paper (a good's share of 8 slots) fills it
        s.deliver(DEST, BREAD, 64.0); // the far end feeds every wagon for the road home
        assert_eq!(s.route_state(0).1.far_purse, None, "never sold there");
        s.advance(LOOK);
        assert!(s.route(0).on_the_road, "the first wagon goes on its day");
        s.advance(3 * LOOK);
        assert!(s.route(0).sold > 0.0, "it sold there");
        let purse = s.route_state(0).1.far_purse.expect("the far end's purse, as its wagon saw it");
        s.advance(DAY + 3 * LOOK); // the next day's wagon there and home again, if one went
        (purse, s.route(0).trips, s.route_state(0).1.stayed)
    };
    let (purse, trips, stayed) = run(2.0);
    assert!(purse.to_f64() < 2.0, "the far end could spend {purse} on goods");
    assert_eq!(trips, 1, "no wagon the next day: 8 stacks of paper, but it could pay for one");
    assert_eq!(stayed, Some(enterprise_core::road::Stay::WaitingForALoad));
    let (purse, trips, _) = run(500.0);
    assert!(purse.to_f64() > 100.0, "a far end with money: {purse}");
    assert_eq!(trips, 2, "the next day's wagon, full of what it can pay for, goes");
}

#[test]
fn a_route_sends_one_wagon_a_day() {
    // `Caravans.poll` 172: a route dispatches once `DEPARTURE_INTERVAL` (a day) has passed since its last wagon left.
    // Four blocks away, the wagon is home within three looks, full again (a far end with money), and waits out the
    // day all the same.
    let mut s = network(5000.0, 500.0, &[("merchant", 1), ("librarian", 1), ("cartographer", 1)], &[]);
    s.content_mut().dials.wagon_slots = 8;
    s.deliver(DEST, BREAD, 64.0);
    s.advance(LOOK);
    assert!(s.route(0).on_the_road, "the first wagon goes");
    s.advance(DAY - LOOK);
    assert_eq!(s.route(0).trips, 1, "home within the day, and no second wagon that day");
    assert!(!s.route(0).on_the_road);
    assert_eq!(s.route_state(0).1.stayed, Some(enterprise_core::road::Stay::Home), "home, as the mod's route says");
    s.advance(DAY + LOOK);
    assert!(s.route(0).on_the_road, "the next goes a day after the first left");
}

#[test]
fn a_load_is_weighed_against_its_road_food_at_its_goods_worth() {
    // `saleLoad` values the load at each good's worth (`Prices.emeraldsEach`), not at the price the Merchants will
    // settle: four blocks' road food, a point each way for the crew and a point each way for the team (0.2 emeralds),
    // is paid by five sheets of paper (a 24th of an emerald each, 0.21), though they'd fetch three quarters of that.
    let mut s = network(5.0, 12.0, &[("librarian", 1)], &[]);
    s.advance(LOOK);
    assert!(s.route(0).on_the_road, "five sheets go");
    let mut s = network(4.0, 12.0, &[("librarian", 1)], &[]);
    s.advance(LOOK);
    assert!(!s.route(0).on_the_road, "four don't pay their road");
    assert_eq!(s.route_state(0).1.stayed, Some(enterprise_core::road::Stay::WaitingForALoad));
}

/// Crops at their locker prices, bread, cookies (a treat) the Farmer's workshop makes of wheat and cane, and paper
/// (the Librarian's and Cartographer's trades).
fn farm_content() -> Content {
    Content::new(vec![
        Good::new("wheat", 1.0 / 20.0, 64).crop(),
        Good::new("carrot", 1.0 / 22.0, 64).crop().food(3.0),
        Good::new("sugar_cane", 1.0 / 24.0, 64).crop(),
        Good::new("bread", 1.0 / 6.0, 64).food(5.0),
        Good::new("cookie", 1.0 / 18.0, 64).comfort(0),
        Good::new("paper", 1.0 / 24.0, 64),
    ], DAY)
}
const F_WHEAT: usize = 0;
const F_CARROT: usize = 1;
const F_CANE: usize = 2;
const F_BREAD: usize = 3;

/// A settlement like the study's: `hands`, 81 plots of `crop`, a shulker of bread (fed: its fields of a crop nobody
/// wants lie fallow), 64 emeralds, every buying trade of its lockers switched on; a day run.
fn farm_town(hands: &[(&str, u8)], crop: usize) -> Society {
    let mut s = Society::new(farm_content());
    let t = s.add_town("Furrow", 64.0);
    s.set_roster(t, hands);
    let villagers = s.roster(t).residents() as f64;
    s.set_people(t, villagers, villagers + 1.0, villagers + 1.0);
    s.deliver(t, F_BREAD, 27.0 * 64.0);
    s.set_plots(t, crop, 81);
    let roster = s.roster(t);
    let lockers = s.lockers(t).unwrap();
    for (h, locker) in lockers.hands.iter().enumerate() {
        for (o, offer) in locker.offers.iter().enumerate() {
            if !offer.sells && offer.level <= roster.hands[h].level {
                s.switch(t, h, o, true);
            }
        }
    }
    s.advance(DAY + LOOK);
    s
}

fn fallow(s: &Society) -> u32 {
    s.farm(0).expect("a farm").farm.fallow
}

#[test]
fn a_farm_grows_what_the_lockers_its_merchant_manages_would_buy() {
    // `CensusTaker.noteDemand`: each buying trade of a villager at a locker the Merchant manages, switched on or not,
    // twice its cost times its uses a day, is the village's demand, and the Farmers tend what it wants
    // (`Territory.afterCensus`). A level-1 Farmer the Merchant manages buys carrots, 22 for an emerald 16 times a
    // restock (704 a day), and wheat, 20 for an emerald (640): wanted, its Novice's 32 plots are all worked, the wheat
    // it plants a field of first (it earns more an item).
    let s = farm_town(&[("merchant", 1), ("farmer", 1)], F_CARROT);
    let farm = s.farm(0).unwrap().farm;
    assert_eq!((farm.census.get(&F_CARROT), farm.census.get(&F_WHEAT)), (Some(&704.0), Some(&640.0)),
        "the census's demand: {:?}", farm.census);
    assert_eq!(farm.per_day.values().sum::<f64>(), 16.0, "a Novice's 32 plots worked: {:?}", farm.per_day);
    // A Merchant of level 1 manages two lockers, the Librarian's and the Cartographer's: the Farmer's carrots aren't
    // the village's demand, and its plots lie fallow, the town fed (`Territory.plan` 1066: "no one wants it").
    let s = farm_town(&[("merchant", 1), ("librarian", 1), ("cartographer", 1), ("farmer", 1)], F_CARROT);
    let farm = s.farm(0).unwrap().farm;
    assert_eq!(farm.census.get(&F_CARROT), None, "the Farmer's locker isn't managed: {:?}", farm.census);
    assert_eq!(farm.per_day.get(&F_CARROT), None, "no carrots grown: {:?}", farm.per_day);
    assert!(fallow(&s) > 0, "its carrot plots lie fallow (but those its workshop's wheat turned over)");
}

#[test]
fn a_farm_grows_what_its_own_workshop_would_draw() {
    // `Workshop.inputDemand` (`afterCensus` 757-759): the Farmer bakes cookies of wheat and cane for its people's
    // treats, so the town wants wheat for them though no locker it manages buys any; its wheat plots are worked.
    // (Its Librarian and Cartographer fill the Merchant's two lockers.)
    let s = farm_town(&[("merchant", 1), ("librarian", 1), ("cartographer", 1), ("farmer", 1)], F_WHEAT);
    let farm = s.farm(0).unwrap().farm;
    assert!(farm.draws.get(&F_WHEAT).is_some_and(|d| d.0 > 0.0), "the workshop draws wheat: {:?}", farm.draws);
    assert!(farm.draws.get(&F_CANE).is_some_and(|d| d.0 > 0.0), "and cane");
    assert!(farm.per_day.get(&F_WHEAT).is_some_and(|n| *n > 0.0), "its wheat plots are worked: {:?}", farm.per_day);
}

#[test]
fn equal_roads_across_open_ground_go_the_mods_way() {
    // `RoutePlanner.plan`'s open set is Java's `PriorityQueue`, compared on the guess alone: of equally short paths
    // it's the heap's sifting that picks, not the order queued (which sent the core's hub->S3 road east first along
    // q, missing the toll town between). The study's hub desk (1002, 1002) to S3's (1851, 1851), as the mod finds
    // their hexes (`Hex.at(BlockPos)`: the block's middle), at road food of 16 points a hex, a 30th of an emerald each.
    let map = enterprise_core::Map::default();
    let (hub, s3) = (enterprise_core::Hex::at(1002.5, 1002.5), enterprise_core::Hex::at(1851.5, 1851.5));
    let path = map.plan(hub, s3, enterprise_core::map::per_hex(1.0 / 30.0), 0.0, &Default::default(),
        &Default::default(), false, &Default::default()).expect("a way");
    let hexes: Vec<(i32, i32)> = path.hexes.iter().map(|h| (h.q, h.r)).collect();
    assert_eq!(hexes, vec![(5, 15), (6, 15), (6, 16), (6, 17), (6, 18), (6, 19), (7, 19), (8, 19), (9, 19), (9, 20),
        (9, 21), (9, 22), (9, 23), (9, 24), (9, 25), (9, 26), (10, 26), (10, 27)],
        "the mod's road, by the toll town's hexes (7,19), (8,19) and (9,19)");
}

#[test]
fn a_town_grows_into_the_hexes_nearest_its_bell() {
    // `Territory.grow` 842: the next hex is the unclaimed neighbour nearest the bell's block, then the lowest q, then
    // r. The study's toll town T: its desk at (1400, 1403), its bell at (1400, 1400), in hex (8, 20); the six about it
    // are all but equally far from the hex's centre (the core ranked them by that, so floating-point noise chose).
    let mut s = Society::new(content());
    let t = s.add_town("T", 0.0);
    s.place_town(t, 1400.5, 1403.5);
    s.place_bell(t, 1400.0, 1400.0);
    let home = s.town_hex(t).unwrap();
    assert_eq!((home.q, home.r), (8, 20));
    let mut territory = s.territory().clone();
    territory.claim(t, home);
    territory.holdings.get_mut(&t).unwrap().culture = 1e9;
    territory.grow(t, home, 1.0, 2);
    let order: Vec<(i32, i32)> = territory.holdings[&t].hexes.iter().take(9).map(|h| (h.q, h.r)).collect();
    assert_eq!(order, vec![(8, 20), (7, 20), (7, 21), (8, 19), (6, 21), (8, 21), (7, 19), (9, 19), (6, 20)],
        "the mod's order (T's growth in the Java study: (8,19) on day 6, (7,19) on day 21, (9,19) on day 28)");
}

#[test]
fn a_road_is_as_long_as_the_straight_way_between_desks_and_its_detours() {
    // `Route.distance` (`Route.java` 191-205): the straight distance between the desks, rounded up, and a hex's width
    // for each hex the path adds over the straight line of hexes. The study's hub (1002, 1002) to S3 (1851, 1851):
    // 1,201 blocks, as the Java study's CSV has it, not the 17 steps of 80 (1,360) the core counted.
    let mut s = Society::new(content());
    let hub = s.add_town("Hub", 0.0);
    let s3 = s.add_town("S3", 0.0);
    let s1 = s.add_town("S1", 0.0);
    s.place_town(hub, 1002.5, 1002.5);
    s.place_town(s3, 1851.5, 1851.5);
    s.place_town(s1, 1302.5, 1002.5);
    let to_s3 = s.add_road(hub, s3, 4.0).expect("a road");
    assert_eq!(s.route_road_blocks(to_s3), 1201);
    let to_s1 = s.add_road(hub, s1, 4.0).expect("a road");
    assert_eq!(s.route_road_blocks(to_s1), 300, "S1's 300, the CSV's (4 steps of 80: 320)");
    // Water on the straight line: the road goes round, a hex's width for each hex it adds.
    let mut s = Society::new(content());
    let (a, b) = (s.add_town("A", 0.0), s.add_town("B", 0.0));
    s.place_town(a, 0.0, 0.0);
    s.place_town(b, 600.0, 0.0);
    let line = enterprise_core::Hex::line(s.town_hex(a).unwrap(), s.town_hex(b).unwrap());
    s.set_ground(line[line.len() / 2], 1.0, 0.0);
    let path = s.plan_road(a, b).expect("a way round");
    assert!(path.hexes.len() > line.len(), "a detour");
    let r = s.add_road(a, b, 4.0).unwrap();
    assert_eq!(s.route_road_blocks(r) as f64, 600.0 + ((path.hexes.len() - line.len()) as f64 * 80.0).round());
}

#[test]
fn the_hosts_reads_a_routes_tolls_and_a_towns_locker_trades() {
    // `Society::tolls_paid`: what a route's wagons paid the posts on their road, all told (its purse book's tolls), as
    // the posts' owner took it; `Society::locker_trades`: the trades a town's managed lockers made.
    let mut s = Society::new(Content::new(vec![Good::new("wheat", 0.05, 64), Good::new("bread", 0.2, 64).food(5.0)], DAY));
    s.content_mut().dials.standing_orders = false;
    s.content_mut().dials.toll_reviews = false;
    let farm = s.add_town("Wheatfield", 20.0);
    let mill = s.add_town("Hayford", 500.0);
    let keeper = s.add_town("Tollgate", 100.0);
    s.place_town(farm, 0.0, 0.0);
    s.place_town(mill, 1000.0, 0.0);
    s.set_trade(farm, 0, 128.0, 0.0);
    s.set_trade(mill, 0, 0.0, 128.0);
    let route = s.add_road(farm, mill, 2.0).expect("a road");
    let path = s.route_path(route).unwrap();
    assert!(s.claim_hex(keeper, path[4]) && s.claim_hex(keeper, path[5]));
    s.set_guards(keeper, 2);
    s.deliver(keeper, 1, 1000.0);
    s.set_toll(keeper, 0.1);
    assert_eq!(s.tolls_paid(route), enterprise_core::Fixed::ZERO);
    s.advance(30 * DAY);
    let paid = s.tolls_paid(route);
    assert!(paid.is_positive() && paid == s.holding(keeper).tolls_taken, "{paid} paid, as the keeper took");
    assert_eq!(s.route_state(route).1.trips, s.route(route).trips);

    let mut s = network(200.0, 100.0, &[("merchant", 1), ("librarian", 1)], &[]);
    assert_eq!(s.locker_trades(DEST), 0);
    s.advance(2 * DAY + LOOK); // a whole day's two restocks since its clock started (`LockerCatchUp`)
    assert!(s.locker_trades(DEST) >= 8, "the Librarian's paper trades: {}", s.locker_trades(DEST));
    assert_eq!(s.locker_trades(HUB), 0, "no roster, no lockers");
}
