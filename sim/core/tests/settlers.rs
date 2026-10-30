//! Settlers (`caravan/Expeditions.java`): a town grown past growing sends a party to a village nobody trades with, or
//! to found a town on open ground; the party is off the world on the road, and the new market opens with the capital
//! and a road home.


use enterprise_core::cash_book::Account;
use enterprise_core::settlers::{capital, Kind};
use enterprise_core::{Content, Good, Hex, Society};

const DAY: u64 = 24_000;
const BREAD: usize = 0;

fn content() -> Content {
    // Raw iron for the hills' worth to settlers (`complement`).
    Content::new(vec![Good::new("bread", 0.2, 64).food(5.0), Good::new("raw_iron", 1.0, 64)], DAY)
}

fn at(s: &mut Society, t: usize, hex: Hex) {
    s.place_town(t, hex.center_x(), hex.center_z());
}

/// A town of ten at the origin, five of them idle, with 150 bread (750 food points, past twice the par of 320) and
/// `money`; its Merchant, a Farmer and three of other trades make up the rest: a Librarian and a Cartographer, the
/// leads its road to a settled village wants, so it trains none (`squad.rs`), and a Mason, so its works take none on
/// (`works.rs`).
fn home(s: &mut Society, money: f64) -> usize {
    let t = s.add_town("Home", money);
    s.set_roster(t, &[("merchant", 2), ("farmer", 1), ("librarian", 1), ("cartographer", 1), ("mason", 1), ("none", 1), ("none", 1),
        ("none", 1), ("nitwit", 1), ("none", 1)]);
    s.set_people(t, 10.0, 12.0, 12.0);
    s.deliver(t, BREAD, 150.0);
    at(s, t, Hex::new(0, 0));
    t
}

fn village(s: &mut Society, hex: Hex) -> usize {
    let v = s.add_village(hex.center_x(), hex.center_z());
    assert_eq!(s.town_hex(v), Some(hex));
    v
}

fn world(money: f64) -> (Society, usize) {
    let mut s = Society::new(content());
    s.content_mut().dials.budget_share = 1.0;
    // No earnings from outside (`hand_wage`): these pin the town's own money as the mod's.
    s.content_mut().dials.hand_wage = 0.0;
    // The mod's spacing (`Expeditions.MIN_HEXES`, `MAX_HEXES`), as its tests pin it.
    s.content_mut().dials.min_hexes = 4;
    s.content_mut().dials.max_hexes = 8;
    let t = home(&mut s, money);
    (s, t)
}

/// The books balance: money and goods conserved since `before`.
fn conserved(s: &mut Society, before: &enterprise_core::check::Totals) {
    s.conserved(before).unwrap();
    s.solvent().unwrap();
}

#[test]
fn the_capital_is_half_the_spare_between_the_least_and_the_dial() {
    // `Expeditions.capital`: max(least, min(jumpstart, spare / 2)); the least 10 for a village, 16 for a new town.
    assert_eq!(capital(Kind::Village, 30.0, enterprise_core::Fixed::from(10.0)), 10.0, "spare 10: the least, 10");
    assert_eq!(capital(Kind::Village, 30.0, enterprise_core::Fixed::from(40.0)), 20.0, "half of 40");
    assert_eq!(capital(Kind::Village, 30.0, enterprise_core::Fixed::from(100.0)), 30.0, "up to the jumpstart");
    assert_eq!(capital(Kind::Found, 42.0, enterprise_core::Fixed::from(20.0)), 16.0, "a new town's least");
    assert_eq!(capital(Kind::Found, 42.0, enterprise_core::Fixed::from(100.0)), 42.0);
    assert_eq!(capital(Kind::Found, 42.0, enterprise_core::Fixed::from(60.0)), 30.0);
}

#[test]
fn a_hosts_settlers_town_is_asked_of_the_host_until_its_desk_stands() {
    // A host's town (its lockers the host's) with nobody's village near: its party founds a town on open ground,
    // which is the host's too, and is asked for (`Found`) at every look until the host says where its desk stands.
    use enterprise_core::wire::{self, Command, Input, Output, Point, PointKind, Request, Response};
    let (mut s, home) = world(100.0);
    s.host_places_lockers(home);
    s.advance(2 * DAY);
    let founded = s.settled();
    assert_eq!(founded.len(), 1, "a town founded: {:?}", s.parties());
    let t = founded[0];
    let found = |s: &Society| s.show_founded().into_iter().filter(|o| matches!(o, Output::Found { town, .. } if *town as usize == t)).count();
    assert_eq!(found(&s), 1);
    let hex = s.town_hex(t).unwrap();
    assert!(matches!(&s.show_founded()[0], Output::Found { x, z, .. }
        if (*x as f64 - hex.center_x()).abs() < 1.0 && (*z as f64 - hex.center_z()).abs() < 1.0), "at its hex's centre");
    assert!(s.lockers(t).is_some_and(|l| l.placed.is_some()), "a host's town: its lockers where the host places them");
    let mut host = Some(s);
    let desk = Point { kind: PointKind::Desk, dim: "minecraft:overworld".into(), x: hex.center_x() as i32, y: 64, z: hex.center_z() as i32 };
    match wire::handle(&mut host, Request::new(Command::Apply { inputs: vec![Input::AddPoint { point: desk, town: t as u32 }] })) {
        Response::Done { outputs, .. } => assert!(!outputs.iter().any(|o| matches!(o, Output::Found { .. })), "built: {outputs:?}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(found(host.as_ref().unwrap()), 0, "not asked again");
}

#[test]
fn a_village_the_host_found_is_settled_and_its_desk_asked_of_the_host() {
    // A host's town, and a village the host found 5 hexes off (`Input::Village`): its Merchant makes a market of it,
    // a settlement of the host's asked for (`Found`) where the village stands, named as a settlement.
    use enterprise_core::wire::{self, Command, Input, Output, Request, Response};
    let (s, home) = world(100.0);
    let mut host = Some(s);
    host.as_mut().unwrap().host_places_lockers(home);
    let at = Hex::new(5, 0);
    match wire::handle(&mut host, Request::new(Command::Apply {
            inputs: vec![Input::Village { x: at.center_x() as i32, z: at.center_z() as i32 }] })) {
        Response::Done { .. } => {}
        other => panic!("{other:?}"),
    }
    let mut s = host.unwrap();
    let v = s.town_count() - 1;
    assert_eq!(s.town_hex(v), Some(at), "a village where the host found it");
    assert!(s.village_seen(at.center_x() + 20.0, at.center_z()).is_none(), "found again: the same village");
    s.advance(2 * DAY);
    assert_eq!(s.settled(), vec![v], "settled by a Merchant: {:?}", s.parties());
    let found: Vec<Output> = s.show_founded();
    assert!(matches!(&found[..], [Output::Found { town, name, .. }] if *town as usize == v && !name.starts_with("Village")),
        "its desk asked of the host, a settlement's name: {found:?}");
    assert!(s.roster(v).merchant.is_some(), "its Merchant among its villagers");
}

#[test]
fn a_party_finds_its_ground_as_explored_and_comes_home_from_water() {
    // No village: a party sets out to found a town on open ground, its way and its spot said to explore.
    let (mut s, _) = world(100.0);
    let out = s.advance(DAY / 4);
    let party = s.parties()[0].clone();
    assert_eq!(party.kind, Kind::Found);
    let explored: Vec<Hex> = out.iter().filter_map(|o| match o {
        enterprise_core::Output::Explore { hexes } => Some(hexes.iter().map(|h| Hex::new(h.q, h.r)).collect::<Vec<_>>()),
        _ => None,
    }).flatten().collect();
    assert!(explored.contains(&party.target), "its spot explored: {explored:?}");
    // The host's survey finds the spot under water (it was guessed dry): they come home, no town founded.
    s.set_ground(party.target, 0.9, 0.0);
    s.advance(3 * DAY);
    assert!(s.settled().is_empty(), "no town on the water: {:?}", s.settled());
    assert!(s.parties().iter().all(|p| p.target != party.target), "home again");
}

// parity-partial: ExpeditionTests.aMerchantSettlesAVillageNobodyTradesWith (the bell, beds and desk placed in the world are Minecraft's: the capital, the route home and the roster are pinned)
#[test]
fn a_merchant_settles_a_village_nobody_trades_with() {
    // 14 emeralds in all, the floor: 4 for the desk, 10 of capital. The Java test settles by hand (the bell and desk are
    // Minecraft's); here the capital lands in the new market's treasury and a route opens home.
    let (mut s, home) = world(14.0);
    s.content_mut().dials.meal_points = 0.0; // no meals: the stock as put
    let v = village(&mut s, Hex::new(5, 0));
    let before = s.totals();
    assert!(s.raising(home), "grown past growing, nothing sent yet");
    assert_eq!(s.roster(v).merchant, None, "a village nobody trades with");
    s.advance(DAY / 4);
    let parties = s.parties();
    assert_eq!(parties.len(), 1, "a party left at the first look");
    assert_eq!((parties[0].kind, parties[0].hands, parties[0].capital, parties[0].bread), (Kind::Village, 1, enterprise_core::Fixed::from(10.0), 64.0));
    assert!(!s.raising(home), "and none while it's on the road");
    assert!(s.town(home).money.abs() < 1e-9, "14 left the treasury: the desk 4 and the capital 10");
    // Off the world on the road: the hand is off the old town's roll, the capital off the books, the bread gone.
    assert_eq!(s.roster(home).residents(), 9);
    assert_eq!(s.town(home).people.villagers, 9.0);
    assert!((s.town(home).stock[BREAD] - 86.0).abs() < 1e-9, "64 bread carried");
    assert_eq!(s.route_count(), 0, "no route yet");
    conserved(&mut s, &before);
    // 5 hexes at 80 blocks, 20 ticks a block: 8,000 ticks.
    assert_eq!(parties[0].arrives, 1200 + 8000);
    s.advance(DAY);
    assert!(s.parties().is_empty(), "arrived");
    assert_eq!(s.settled(), vec![v]);
    assert!((s.town(v).stock[BREAD] - 64.0).abs() < 1e-9, "and the bread");
    let r = s.roster(v);
    assert!(r.merchant.is_some_and(|m| s.content().trades[r.hands[m].trade].merchant), "with its Merchant");
    assert_eq!(r.residents(), 4);
    assert_eq!(s.route_count(), 2, "a road there and its own road home");
    let route = s.route_path(0).expect("a road over the map");
    assert_eq!((route.first(), route.last()), (Some(&Hex::new(0, 0)), Some(&Hex::new(5, 0))));
    let back = s.route_path(1).expect("its own road home");
    assert_eq!((back.first(), back.last()), (Some(&Hex::new(5, 0)), Some(&Hex::new(0, 0))));
    conserved(&mut s, &before);
    let book = s.account_book(Account::Treasury(v), 2);
    assert!(book.iter().any(|(_, what, n)| what == "settling" && *n == 10.0), "{book:?}");
}

#[test]
fn a_party_goes_once_in_eight_days_at_most() {
    let (mut s, home) = world(200.0);
    s.content_mut().dials.meal_points = 0.0; // no meals: the stock as put
    let v1 = village(&mut s, Hex::new(5, 0));
    let v2 = village(&mut s, Hex::new(-6, 0));
    s.deliver(home, BREAD, 150.0); // enough bread for two parties to take 64 each and still be fed
    let before = s.totals();
    s.advance(8 * DAY - 1);
    assert_eq!(s.settled(), vec![v1], "one in the first eight days: the nearest village");
    assert_eq!(s.last_party(home), Some(1200));
    assert!(s.town(home).money == (200.0 - 34.0), "4 for the desk, the jumpstart's 30");
    s.advance(8 * DAY);
    assert_eq!(s.parties().len(), 1, "the second left on day 8");
    assert_eq!(s.last_party(home), Some(8 * DAY));
    // The nearest village is a market now: the second party makes one of the farther village (any village in reach,
    // not only the nearest: John's "several villages naturally generated ... would've made excellent settlement
    // candidates"), as it adds as much as open ground would.
    assert_eq!(s.parties()[0].kind, Kind::Village);
    s.advance(10 * DAY);
    let settled = s.settled();
    assert_eq!(settled, vec![v1, v2]);
    // The jumpstart's 30 landed in its treasury (spent from there: a stall, homes, its own road's crew).
    assert!(s.cash_book(v1, 30).iter().any(|(_, what, e)| what == "settling" && *e == 30.0), "{:?}", s.cash_book(v1, 30));
    conserved(&mut s, &before);
}

#[test]
fn a_town_not_grown_or_not_fed_or_without_hands_or_money_sends_nobody() {
    // Under 8 villagers.
    let (mut s, t) = world(100.0);
    s.content_mut().dials.meal_points = 0.0; // no meals: the stock as put
    village(&mut s, Hex::new(5, 0));
    s.set_roster(t, &[("merchant", 1), ("none", 1), ("none", 1), ("none", 1), ("none", 1), ("none", 1), ("none", 1)]);
    s.advance(3 * DAY);
    assert!(s.parties().is_empty() && s.settled().is_empty(), "7 villagers is still growing");
    s.add_hand(t, "none", 1);
    assert!(s.raising(t), "8 is grown");
    // Short of food: under twice the par of 320.
    let mut s2 = Society::new(content());
    let t2 = home(&mut s2, 100.0);
    s2.world.get_mut::<enterprise_core::state::Stock>(s2.world.resource::<enterprise_core::state::Towns>().0[t2]).unwrap().0[BREAD] = 127.0;
    village(&mut s2, Hex::new(5, 0));
    s2.advance(3 * DAY);
    assert!(s2.parties().is_empty() && !s2.raising(t2), "635 food points is under 640");
    // No idle hand: every hand has a trade.
    let (mut s, t) = world(100.0);
    village(&mut s, Hex::new(5, 0));
    s.set_roster(t, &[("merchant", 1), ("farmer", 1), ("farmer", 1), ("cleric", 1), ("mason", 1), ("mason", 1),
        ("cleric", 1), ("farmer", 1)]);
    s.advance(3 * DAY);
    assert!(s.parties().is_empty(), "nobody waiting for work");
    // Only Nitwits idle: they never go (`Labour.NONE` only).
    let (mut s, t) = world(100.0);
    village(&mut s, Hex::new(5, 0));
    s.set_roster(t, &[("merchant", 1), ("farmer", 1), ("farmer", 1), ("cleric", 1), ("mason", 1), ("mason", 1),
        ("cleric", 1), ("nitwit", 1), ("nitwit", 1)]);
    s.advance(3 * DAY);
    assert!(s.parties().is_empty(), "Nitwits don't settle");
    // The lead is the first hand of no trade, never a Nitwit before it.
    let (mut s, t) = world(100.0);
    village(&mut s, Hex::new(5, 0));
    s.advance(DAY / 4);
    assert_eq!(s.parties().len(), 1);
    let names: Vec<&str> = s.roster(t).hands.iter().map(|h| s.content().trades[h.trade].name.as_str()).collect();
    assert_eq!(names.iter().filter(|n| **n == "nitwit").count(), 1, "the Nitwit stays home: {names:?}");
    // No Merchant: a grown, fed village sends nobody (only a market does).
    let (mut s, t) = world(100.0);
    village(&mut s, Hex::new(5, 0));
    s.set_roster(t, &[("farmer", 1), ("farmer", 1), ("cleric", 1), ("mason", 1), ("none", 1), ("none", 1), ("none", 1),
        ("none", 1), ("none", 1)]);
    s.advance(3 * DAY);
    assert!(s.parties().is_empty(), "no Merchant, no market, no settlers");
    // Money short of 14.
    let (mut s, _) = world(13.0);
    village(&mut s, Hex::new(5, 0));
    s.advance(3 * DAY);
    assert!(s.parties().is_empty(), "13 emeralds can't settle a village");
}

#[test]
fn the_households_savings_stand_behind_a_party() {
    // 5 in the treasury, 60 saved at home: half of it on call (`deposit_share`), 35 to spare; a village takes the set-up
    // of 4 and half the rest, 15.5, and the households lend all 19.5 of it.
    let (mut s, t) = world(5.0);
    s.post(Account::Outside, Account::Households(t), 60.0, enterprise_core::cash_book::Reason::PaidIn);
    village(&mut s, Hex::new(5, 0));
    s.advance(DAY / 4);
    assert_eq!(s.parties().len(), 1, "the savings sent it");
    assert_eq!(s.parties()[0].capital, 15.5);
    assert!((s.deposits(t) - 19.5).abs() < 0.05, "lent to the treasury, with its interest since: {}", s.deposits(t));
    assert!(s.town(t).money == 5.0 && s.town(t).people.purse == 40.5);
}

#[test]
fn the_accountant_may_hold_the_party_back() {
    // Half the free money a day: 40 free, 20 to spend today; a village takes at least 14 and here 4 + 18 = 22.
    let (mut s, _) = world(40.0);
    s.content_mut().dials.budget_share = 0.5;
    village(&mut s, Hex::new(5, 0));
    s.advance(DAY / 4);
    assert!(s.parties().is_empty(), "22 is more than the day's 20");
    assert!(!s.held_back(0, 1).is_empty(), "noted held back");
}

#[test]
fn a_village_too_near_or_too_far_is_no_candidate() {
    for hex in [Hex::new(3, 0), Hex::new(9, 0)] {
        let (mut s, _) = world(100.0);
        let v = village(&mut s, hex);
        s.advance(3 * DAY);
        assert!(!s.settled().contains(&v) && s.roster(v).merchant.is_none(), "{hex:?}");
        assert!(s.parties().iter().all(|p| p.kind == Kind::Found), "a town is founded instead: {hex:?}");
    }
    // Not 4 hexes clear of another market: the village is beside one.
    let (mut s, _) = world(100.0);
    let v = village(&mut s, Hex::new(5, 0));
    let other = s.add_town("Other", 10.0);
    s.set_roster(other, &[("merchant", 1)]);
    at(&mut s, other, Hex::new(8, 0));
    s.advance(3 * DAY);
    assert!(s.parties().iter().all(|p| p.target != s.town_hex(v).unwrap() || p.kind == Kind::Found));
    assert!(!s.settled().contains(&v));
}

#[test]
fn with_no_village_a_town_is_founded_on_open_ground() {
    let (mut s, home) = world(100.0);
    let before = s.totals();
    s.advance(DAY / 4);
    let parties = s.parties();
    assert_eq!(parties.len(), 1);
    // The flattest, driest ground, then the nearest, then the first direction (east): 5 hexes out.
    assert_eq!((parties[0].kind, parties[0].target, parties[0].hands), (Kind::Found, Hex::new(5, 0), 2));
    // 18 for the set-up and half of the 82 beyond it, 41, under the dial's 42.
    assert!(parties[0].capital == 41.0, "{}", parties[0].capital);
    assert!(s.town(home).money == (100.0 - 18.0 - 41.0));
    assert_eq!(s.roster(home).residents(), 8, "two hands away");
    conserved(&mut s, &before);
    s.advance(DAY);
    assert_eq!(s.town_count(), 2, "the new town");
    let new = s.settled()[0];
    assert_eq!(s.town_hex(new), Some(Hex::new(5, 0)));
    assert!(s.town(new).money == 41.0);
    let r = s.roster(new);
    let names: Vec<&str> = r.hands.iter().map(|h| s.content().trades[h.trade].name.as_str()).collect();
    assert_eq!(names, ["merchant", "farmer"], "a Merchant and its Farmer");
    assert_eq!(r.merchant, Some(0));
    assert_eq!(s.town(new).people.beds, 4.0, "two cottages");
    assert!(s.town(new).people.land > 4.0, "and room for more: open ground");
    assert_eq!(s.route_count(), 2, "roads both ways");
    conserved(&mut s, &before);
}

#[test]
fn open_ground_is_dry_and_flat_enough_and_a_party_needs_two_hands_and_34() {
    let (mut s, _) = world(100.0);
    // East, 5 out, is a lake; 7 out east is hills; the other directions are open: the party goes to the next best.
    s.set_ground(Hex::new(5, 0), 0.6, 0.0);
    s.set_ground(Hex::new(7, 0), 0.0, 20.0);
    s.advance(DAY / 4);
    let p = &s.parties()[0];
    assert_eq!(p.target, Hex::new(5, -5), "the next direction, 5 out");
    // With one idle hand a town can't found one (it takes a Merchant and a settler).
    let (mut s, t) = world(100.0);
    s.set_roster(t, &[("merchant", 1), ("farmer", 1), ("farmer", 1), ("cleric", 1), ("mason", 1), ("mason", 1),
        ("cleric", 1), ("farmer", 1), ("none", 1)]);
    s.advance(3 * DAY);
    assert!(s.parties().is_empty(), "one hand: no new town");
    // With 33 to spare, too.
    let (mut s, _) = world(33.0);
    s.advance(3 * DAY);
    assert!(s.parties().is_empty(), "33 is under the 34 a new town costs");
}

#[test]
fn a_grown_town_with_nobody_idle_sends_workers_it_has_others_of() {
    // Seed 1717: towns with savings to found sent nobody, their hands all at work. The least practised go, each trade
    // keeping one, the town keeping eight.
    let (mut s, t) = world(100.0);
    s.set_roster(t, &[("merchant", 2), ("farmer", 3), ("farmer", 1), ("farmer", 2), ("cleric", 1), ("cleric", 2),
        ("mason", 1), ("mason", 2), ("librarian", 1), ("cartographer", 1)]);
    s.advance(DAY / 4);
    let parties = s.parties();
    assert_eq!((parties.len(), parties[0].kind, parties[0].hands), (1, Kind::Found, 2), "{parties:?}");
    let r = s.roster(t);
    assert_eq!(r.residents(), 8);
    let count = |name: &str| r.hands.iter().filter(|h| s.content().trades[h.trade].name == name).count();
    assert_eq!((count("merchant"), count("farmer"), count("cleric"), count("mason")), (1, 2, 1, 2), "a Farmer and a Cleric");
    assert!(r.hands.iter().filter(|h| s.content().trades[h.trade].name == "farmer").all(|h| h.level >= 2),
        "the Novice Farmer went");
    // Nine: one to spare, and a new town takes two.
    let (mut s, t) = world(100.0);
    s.set_roster(t, &[("merchant", 2), ("farmer", 3), ("farmer", 1), ("farmer", 2), ("cleric", 1), ("cleric", 2),
        ("mason", 1), ("mason", 2), ("librarian", 1)]);
    s.advance(3 * DAY);
    assert!(s.parties().is_empty(), "{:?}", s.parties());
}

#[test]
fn open_ground_off_the_spokes_is_found() {
    // Every hex 5 to 7 out along the six ways is a lake: the ground between them is still open.
    let (mut s, _) = world(100.0);
    for (dq, dr) in enterprise_core::hex::DIRECTIONS {
        for d in 5..=7 {
            s.set_ground(Hex::new(d * dq, d * dr), 0.6, 0.0);
        }
    }
    s.advance(DAY / 4);
    let p = &s.parties()[0];
    let d = Hex::new(0, 0).distance(p.target);
    assert!((5..=7).contains(&d) && p.target.q != 0 && p.target.r != 0 && p.target.q != -p.target.r, "{:?}", p.target);
}

#[test]
fn no_way_through_and_nobody_goes() {
    let (mut s, _) = world(100.0);
    let v = village(&mut s, Hex::new(5, 0));
    // A wall of cliffs round the village, too sheer for llamas and dry for a boat: no road to it.
    for n in Hex::new(5, 0).neighbors() {
        s.set_ground(n, 0.0, 1000.0);
    }
    s.advance(3 * DAY);
    assert!(s.settled().is_empty() && !s.parties().iter().any(|p| p.kind == Kind::Village && p.target == s.town_hex(v).unwrap()), "{:?}", s.parties());
}

#[test]
fn a_party_goes_by_llama_over_the_steeps_and_by_boat_over_the_water() {
    // A wall of lake round the village: a wagon has no way, a boat sails it.
    let (mut s, _) = world(100.0);
    village(&mut s, Hex::new(5, 0));
    for n in Hex::new(5, 0).neighbors() {
        s.set_ground(n, 1.0, 0.0);
    }
    s.advance(DAY / 4);
    assert_eq!(s.parties().len(), 1, "by boat");
    // Rough ground round it, too rough for a wagon, not for llamas.
    let (mut s, _) = world(100.0);
    village(&mut s, Hex::new(5, 0));
    let rough = (enterprise_core::map::Carrier::Wagon.gait().cliff + enterprise_core::map::Carrier::Llama.gait().cliff) / 2.0;
    for n in Hex::new(5, 0).neighbors() {
        s.set_ground(n, 0.0, rough);
    }
    s.advance(DAY / 4);
    assert_eq!(s.parties().len(), 1, "by llama");
    // A new town past the same rough ring round home: founded, not turned back on arrival.
    let (mut s, t) = world(100.0);
    for n in s.town_hex(t).unwrap().neighbors() {
        s.set_ground(n, 0.0, rough);
    }
    s.advance(DAY / 4);
    assert_eq!(s.parties().len(), 1, "by llama");
    assert_eq!(s.parties()[0].kind, Kind::Found);
    s.advance(3 * DAY);
    assert_eq!(s.settled().len(), 1, "founded where only llamas go");
}

#[test]
fn a_party_whose_village_found_a_merchant_comes_home_with_its_capital() {
    let (mut s, home) = world(100.0);
    let v = village(&mut s, Hex::new(5, 0));
    let before = s.totals();
    s.advance(DAY / 4);
    assert_eq!(s.parties().len(), 1);
    // Meanwhile someone else's Merchant settles in the village.
    s.add_hand(v, "merchant", 1);
    s.advance(2 * DAY);
    assert!(s.settled().is_empty(), "no place to settle");
    assert_eq!(s.roster(home).residents(), 10, "the hand is home");
    assert!(s.town(home).money == 96.0, "with the capital, less the desk's 4: {}", s.town(home).money);
    conserved(&mut s, &before);
}

#[test]
fn a_party_on_the_road_survives_a_save() {
    let (mut s, _) = world(100.0);
    let v = village(&mut s, Hex::new(5, 0));
    s.advance(DAY / 4);
    assert_eq!(s.parties().len(), 1);
    let blob = s.save();
    let mut loaded = Society::load(&blob).unwrap();
    assert_eq!(loaded.parties(), s.parties());
    s.advance(2 * DAY);
    loaded.advance(2 * DAY);
    assert_eq!(loaded.settled(), vec![v]);
    assert!(loaded.save() == s.save(), "the same world");
}

#[test]
fn the_dials_are_the_mods_and_move_the_rule() {
    let (mut s, _) = world(100.0);
    assert_eq!(s.dial("settling_capital"), Some(30.0));
    assert_eq!(s.dial("founding_capital"), Some(42.0));
    assert_eq!(s.dial("days_between"), Some(8.0));
    assert_eq!(s.dial("min_hexes"), Some(4.0));
    assert_eq!(s.dial("max_hexes"), Some(8.0));
    s.set_dial("settling_capital", 12.0).unwrap();
    village(&mut s, Hex::new(5, 0));
    s.advance(DAY / 4);
    assert_eq!(s.parties()[0].capital, 12.0, "the jumpstart is the dial");
    // Villages out to 8 hexes only: max_hexes 4 would keep this one out of reach.
    let (mut s, _) = world(100.0);
    s.set_dial("max_hexes", 4.0).unwrap();
    village(&mut s, Hex::new(5, 0));
    s.advance(DAY / 4);
    assert!(s.parties().iter().all(|p| p.kind != Kind::Village));
}

#[test]
fn settle_looks_at_one_town_now() {
    let (mut s, home) = world(100.0);
    village(&mut s, Hex::new(5, 0));
    assert!(s.settle(home));
    assert!(!s.settle(home), "not again while the party is on the road");
}

#[test]
fn claimed_ground_is_not_open() {
    // `openGround` passes over a hex someone holds: east 5 out is claimed, so the party goes the next way, 5 out.
    let (mut s, home) = world(100.0);
    assert!(s.claim_hex(home, Hex::new(5, 0)));
    s.advance(DAY / 4);
    assert_eq!(s.parties()[0].target, Hex::new(5, -5), "{:?}", s.parties());
}

#[test]
fn a_road_home_with_no_way_is_owed_till_a_ship_can_sail_it() {
    // A lake round the village: settled by boat, but no road home a wagon, llama or ship can take yet.
    let (mut s, home) = world(100.0);
    let v = village(&mut s, Hex::new(5, 0));
    for n in Hex::new(5, 0).neighbors() {
        s.set_ground(n, 1.0, 0.0);
    }
    s.advance(3 * DAY);
    assert_eq!(s.settled(), vec![v]);
    let roads = |s: &Society| s.route_count();
    assert_eq!(roads(&s), 0, "no way home");
    // Harbours laid at both, on the lake: the road home opens by ship the next day.
    s.set_port(home, Hex::new(4, 0).center_x(), Hex::new(4, 0).center_z());
    s.set_port(v, Hex::new(6, 0).center_x(), Hex::new(6, 0).center_z());
    s.advance(5 * DAY);
    assert_eq!(roads(&s), 2, "the roads both ways, owed, opened");
    assert_eq!((s.route_carrier(0), s.route_carrier(1)), (enterprise_core::map::Carrier::Ship, enterprise_core::map::Carrier::Ship));
}

#[test]
fn settlers_found_where_the_land_adds_most_to_the_towns_portfolio() {
    // Hills round (7, 0), plains everywhere else (the home's land): the party founds among the hills, for the ore its
    // home's land can't give, though open plains lie nearer.
    let (mut s, _) = world(100.0);
    let hills = Hex::new(7, 0);
    for h in std::iter::once(hills).chain(hills.neighbors()) {
        s.set_land(h, enterprise_core::map::Land::Hills);
    }
    s.advance(DAY / 4);
    let party = &s.parties()[0];
    assert_eq!((party.kind, party.target), (Kind::Found, hills), "{:?}", s.parties());
}
